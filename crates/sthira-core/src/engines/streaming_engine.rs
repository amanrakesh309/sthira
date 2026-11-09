use crate::{llm::streaming::ProgressiveStreamChunk, Engine, OperationalMode};
use sha2::{Digest, Sha256};

/// Transparent Markdown streaming. Prose is never converted into UI or removed.
#[derive(Default)]
pub struct AdaptiveStreamParser {
    accumulated_full_text: String,
    total_tokens: u32,
}
impl AdaptiveStreamParser {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn feed(&mut self, token: &str, output: &mut Vec<ProgressiveStreamChunk>) {
        if token.is_empty() {
            return;
        }
        self.total_tokens += 1;
        self.accumulated_full_text.push_str(token);
        output.push(ProgressiveStreamChunk::TextToken(token.to_string()));
    }
    pub fn finish(&mut self, output: &mut Vec<ProgressiveStreamChunk>, duration_ms: u64) {
        output.push(ProgressiveStreamChunk::Complete {
            total_tokens: self.total_tokens,
            duration_ms,
            basis_fingerprint: format!(
                "{:x}",
                Sha256::digest(self.accumulated_full_text.as_bytes())
            ),
        });
    }
    pub fn accumulated_text(&self) -> &str {
        &self.accumulated_full_text
    }
    pub fn fallback_count(&self) -> u32 {
        0
    }
}

pub async fn execute_adaptive_stream(
    engine: &Engine,
    raw_input: &str,
    mode: OperationalMode,
    model_id: Option<String>,
    sender: tokio::sync::mpsc::Sender<ProgressiveStreamChunk>,
) -> Result<(), String> {
    let start_ms = engine.clock().now_ms();
    let mut parser = AdaptiveStreamParser::new();

    // Check if a real provider is configured in the DB
    let resolved_model_opt = {
        let ro = engine.db().open_read_only().ok();
        ro.and_then(|r| {
            if let Some(ref mid) = model_id {
                crate::db::providers_repo::get_model_entry(&r, mid)
                    .ok()
                    .flatten()
            } else {
                crate::db::providers_repo::get_default_model(&r)
                    .ok()
                    .flatten()
            }
        })
    };

    if resolved_model_opt.is_none() {
        return Err(
            "No model is configured for this run. Select an enabled model in Settings.".into(),
        );
    }
    let mut streamed_live = false;
    let mut stream_completed = false;

    if let Some((model_entry, provider_entry, api_key)) = resolved_model_opt {
        let is_mock = provider_entry.provider_type.starts_with("mock")
            || provider_entry.id.starts_with("mock")
            || model_entry.model_identifier.starts_with("mock")
            || engine.model().id().starts_with("mock")
            || engine.model().id().starts_with("fake");

        if is_mock || !provider_entry.is_enabled || !model_entry.is_enabled {
            return Err("The selected model is unavailable or disabled. Select an enabled model in Settings.".into());
        }
        {
            let role = match mode {
                OperationalMode::Plan => "Write a clear, actionable plan with phases, task lists, prerequisites and open questions.",
                OperationalMode::Research => "Synthesize the supplied evidence with clear comparisons, source citations and uncertainties.",
                _ => "Write a clear, actionable response to the user's directive.",
            };
            let system_prompt = format!(
                "{role} Never claim fresh web searches unless evidence was actually supplied. {}",
                crate::llm::page_blocks::PAGE_BLOCK_INSTRUCTIONS
            );
            let policy = crate::llm::lifecycle::RequestPolicy::standard();
            let client = reqwest::Client::builder()
                .connect_timeout(policy.connect_timeout)
                .build()
                .unwrap_or_default();

            if provider_entry.provider_type == "anthropic" {
                let resolved = {
                    let reader = engine.db().reader().map_err(|e| e.to_string())?;
                    crate::llm::dispatcher::resolve_model_provider_with_policy(
                        &reader,
                        model_id.as_deref(),
                        engine.model(),
                        policy,
                    )
                    .map_err(|e| e.to_string())?
                };
                let schema = serde_json::json!({"type":"object","properties":{"content_md":{"type":"string"}},"required":["content_md"]});
                let response = tokio::time::timeout(
                    policy.workflow_deadline,
                    resolved
                        .provider
                        .generate_json(&schema, &system_prompt, raw_input),
                )
                .await
                .map_err(|_| {
                    format!(
                        "The selected model timed out after {} seconds.",
                        policy.workflow_deadline.as_secs()
                    )
                })?
                .map_err(|e| e.to_string())?;
                let value: serde_json::Value = serde_json::from_str(&response).map_err(|_| {
                    "The selected model returned an unreadable response.".to_string()
                })?;
                let content = value
                    .get("content_md")
                    .and_then(|v| v.as_str())
                    .filter(|v| !v.trim().is_empty())
                    .ok_or("The selected model returned no document.")?;
                let mut chunks = Vec::new();
                parser.feed(content, &mut chunks);
                for chunk in chunks {
                    if sender.send(chunk).await.is_err() {
                        return Err("Generation was interrupted.".into());
                    }
                }
                streamed_live = true;
                stream_completed = true;
            } else if provider_entry.provider_type == "ollama" {
                let url =
                    crate::llm::endpoints::ollama_api_url(&provider_entry.base_url, "generate");
                let req_body = serde_json::json!({
                    "model": model_entry.model_identifier,
                    "system": system_prompt,
                    "prompt": raw_input,
                    "stream": true
                });

                {
                    let workflow_start = tokio::time::Instant::now();
                    let workflow_deadline_at = workflow_start + policy.workflow_deadline;
                    let initial_deadline_at = workflow_start + policy.stream_initial_timeout;

                    let mut request = client.post(&url).json(&req_body);
                    if let Some(key) = api_key.as_deref().filter(|key| !key.is_empty()) {
                        request = request.bearer_auth(key);
                    }

                    let now = tokio::time::Instant::now();
                    if now >= workflow_deadline_at {
                        return Err(format!(
                            "The model exceeded its maximum stream deadline of {} seconds. No generated document was saved.",
                            policy.workflow_deadline.as_secs()
                        ));
                    }
                    let send_timeout = initial_deadline_at.saturating_duration_since(now);
                    let send_res = tokio::time::timeout(send_timeout, request.send()).await;
                    let mut resp = match send_res {
                        Ok(Ok(r)) => r,
                        Ok(Err(e)) => {
                            let kind = crate::llm::lifecycle::classify_reqwest_error(&e, &policy);
                            match kind {
                                crate::llm::lifecycle::RequestErrorKind::ConnectionTimeout {
                                    limit,
                                } => {
                                    return Err(format!(
                                        "Could not connect to the model provider within {} seconds. Check that the service is running and accessible.",
                                        limit.as_secs()
                                    ));
                                }
                                crate::llm::lifecycle::RequestErrorKind::ConnectionFailed {
                                    details,
                                } => {
                                    return Err(format!(
                                        "Could not reach the selected model ({details}). Check its provider connection."
                                    ));
                                }
                                _ => {
                                    return Err("Could not reach the selected model. Check its provider connection.".to_string());
                                }
                            }
                        }
                        Err(_) => {
                            if tokio::time::Instant::now() >= workflow_deadline_at {
                                return Err(format!(
                                    "The model exceeded its maximum stream deadline of {} seconds. No generated document was saved.",
                                    policy.workflow_deadline.as_secs()
                                ));
                            } else {
                                return Err(format!(
                                    "The model did not begin generating within {} seconds. Check provider connection.",
                                    policy.stream_initial_timeout.as_secs()
                                ));
                            }
                        }
                    };

                    if !resp.status().is_success() {
                        return Err(format!(
                            "The selected model returned HTTP {}. Check the model and provider settings.",
                            resp.status()
                        ));
                    }
                    {
                        let mut byte_buffer: Vec<u8> = Vec::new();
                        let mut had_tokens = false;
                        let mut is_first_chunk = true;

                        loop {
                            let now = tokio::time::Instant::now();
                            if now >= workflow_deadline_at {
                                return Err(format!(
                                    "The model exceeded its maximum stream deadline of {} seconds. No generated document was saved.",
                                    policy.workflow_deadline.as_secs()
                                ));
                            }

                            let remaining_overall =
                                workflow_deadline_at.saturating_duration_since(now);
                            let stage_limit = if is_first_chunk {
                                if now >= initial_deadline_at {
                                    return Err(format!(
                                        "The model did not begin generating within {} seconds. Check provider connection.",
                                        policy.stream_initial_timeout.as_secs()
                                    ));
                                }
                                initial_deadline_at.saturating_duration_since(now)
                            } else {
                                policy.stream_inactivity_timeout
                            };

                            let wait_dur = std::cmp::min(stage_limit, remaining_overall);
                            let chunk_res = tokio::time::timeout(wait_dur, resp.chunk()).await;
                            let chunk_bytes = match chunk_res {
                                Ok(Ok(Some(bytes))) => bytes,
                                Ok(Ok(None)) => break,
                                Ok(Err(_)) => {
                                    return Err("The model stream was interrupted. No generated document was saved.".into());
                                }
                                Err(_) => {
                                    let fired_at = tokio::time::Instant::now();
                                    if fired_at >= workflow_deadline_at
                                        || wait_dur == remaining_overall
                                    {
                                        return Err(format!(
                                            "The model exceeded its maximum stream deadline of {} seconds. No generated document was saved.",
                                            policy.workflow_deadline.as_secs()
                                        ));
                                    } else if is_first_chunk {
                                        return Err(format!(
                                            "The model did not begin generating within {} seconds. Check provider connection.",
                                            policy.stream_initial_timeout.as_secs()
                                        ));
                                    } else {
                                        return Err(format!(
                                            "The model stream stalled for over {} seconds without emitting tokens. No generated document was saved.",
                                            policy.stream_inactivity_timeout.as_secs()
                                        ));
                                    }
                                }
                            };
                            is_first_chunk = false;
                            byte_buffer.extend_from_slice(&chunk_bytes);

                            while let Some(newline_pos) =
                                byte_buffer.iter().position(|&b| b == b'\n')
                            {
                                let line_bytes: Vec<u8> =
                                    byte_buffer.drain(..=newline_pos).collect();
                                let line = String::from_utf8_lossy(&line_bytes).trim().to_string();
                                if line.is_empty() {
                                    continue;
                                }

                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                                    if v.get("error").is_some() {
                                        return Err("The model reported a generation error.".into());
                                    }
                                    if v.get("done").and_then(|d| d.as_bool()) == Some(true) {
                                        stream_completed = true;
                                    }
                                    if let Some(tok) = v.get("response").and_then(|t| t.as_str()) {
                                        if !tok.is_empty() {
                                            had_tokens = true;
                                            let mut chunks = Vec::new();
                                            parser.feed(tok, &mut chunks);
                                            for chunk in chunks {
                                                if sender.send(chunk).await.is_err() {
                                                    return Err("Generation delivery was interrupted. No generated document was saved.".into());
                                                }
                                            }
                                            tokio::task::yield_now().await;
                                        }
                                    }
                                    if v.get("error").is_some() {
                                        return Err("The model reported a generation error. Check provider settings.".into());
                                    }
                                    if v.get("done").and_then(|d| d.as_bool()) == Some(true) {
                                        stream_completed = true;
                                        break;
                                    }
                                }
                            }
                        }

                        if !byte_buffer.is_empty() {
                            let line = String::from_utf8_lossy(&byte_buffer).trim().to_string();
                            if !line.is_empty() {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                                    if v.get("error").is_some() {
                                        return Err("The model reported a generation error.".into());
                                    }
                                    if v.get("done").and_then(|d| d.as_bool()) == Some(true) {
                                        stream_completed = true;
                                    }
                                    if let Some(tok) = v.get("response").and_then(|t| t.as_str()) {
                                        if !tok.is_empty() {
                                            had_tokens = true;
                                            let mut chunks = Vec::new();
                                            parser.feed(tok, &mut chunks);
                                            for chunk in chunks {
                                                if sender.send(chunk).await.is_err() {
                                                    return Err("Generation delivery was interrupted. No generated document was saved.".into());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if had_tokens {
                            streamed_live = true;
                        }
                    }
                }
            } else {
                // OpenAI-compatible / cloud endpoints
                let base = provider_entry.base_url.trim_end_matches('/');
                let url = if base.ends_with("/v1") {
                    format!("{base}/chat/completions")
                } else {
                    format!("{base}/v1/chat/completions")
                };
                let req_body = serde_json::json!({
                    "model": model_entry.model_identifier,
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": raw_input}
                    ],
                    "stream": true
                });

                let mut req = client.post(&url).json(&req_body);
                if let Some(key) = api_key.filter(|k| !k.is_empty()) {
                    req = req.header("Authorization", format!("Bearer {}", key));
                }

                {
                    let workflow_start = tokio::time::Instant::now();
                    let workflow_deadline_at = workflow_start + policy.workflow_deadline;
                    let initial_deadline_at = workflow_start + policy.stream_initial_timeout;

                    let now = tokio::time::Instant::now();
                    if now >= workflow_deadline_at {
                        return Err(format!(
                            "The model exceeded its maximum stream deadline of {} seconds. No generated document was saved.",
                            policy.workflow_deadline.as_secs()
                        ));
                    }
                    let send_timeout = initial_deadline_at.saturating_duration_since(now);
                    let send_res = tokio::time::timeout(send_timeout, req.send()).await;
                    let mut resp = match send_res {
                        Ok(Ok(r)) => r,
                        Ok(Err(e)) => {
                            let kind = crate::llm::lifecycle::classify_reqwest_error(&e, &policy);
                            match kind {
                                crate::llm::lifecycle::RequestErrorKind::ConnectionTimeout {
                                    limit,
                                } => {
                                    return Err(format!(
                                        "Could not connect to the model provider within {} seconds. Check that the service is running and accessible.",
                                        limit.as_secs()
                                    ));
                                }
                                crate::llm::lifecycle::RequestErrorKind::ConnectionFailed {
                                    details,
                                } => {
                                    return Err(format!(
                                        "Could not reach the selected model ({details}). Check its provider connection."
                                    ));
                                }
                                _ => {
                                    return Err("Could not reach the selected model. Check its provider connection.".to_string());
                                }
                            }
                        }
                        Err(_) => {
                            if tokio::time::Instant::now() >= workflow_deadline_at {
                                return Err(format!(
                                    "The model exceeded its maximum stream deadline of {} seconds. No generated document was saved.",
                                    policy.workflow_deadline.as_secs()
                                ));
                            } else {
                                return Err(format!(
                                    "The model did not begin generating within {} seconds. Check provider connection.",
                                    policy.stream_initial_timeout.as_secs()
                                ));
                            }
                        }
                    };

                    if !resp.status().is_success() {
                        return Err(format!(
                            "The selected model returned HTTP {}. Check the model and provider settings.",
                            resp.status()
                        ));
                    }
                    {
                        let mut byte_buffer: Vec<u8> = Vec::new();
                        let mut had_tokens = false;
                        let mut is_first_chunk = true;

                        loop {
                            let now = tokio::time::Instant::now();
                            if now >= workflow_deadline_at {
                                return Err(format!(
                                    "The model exceeded its maximum stream deadline of {} seconds. No generated document was saved.",
                                    policy.workflow_deadline.as_secs()
                                ));
                            }

                            let remaining_overall =
                                workflow_deadline_at.saturating_duration_since(now);
                            let stage_limit = if is_first_chunk {
                                if now >= initial_deadline_at {
                                    return Err(format!(
                                        "The model did not begin generating within {} seconds. Check provider connection.",
                                        policy.stream_initial_timeout.as_secs()
                                    ));
                                }
                                initial_deadline_at.saturating_duration_since(now)
                            } else {
                                policy.stream_inactivity_timeout
                            };

                            let wait_dur = std::cmp::min(stage_limit, remaining_overall);
                            let chunk_res = tokio::time::timeout(wait_dur, resp.chunk()).await;
                            let chunk_bytes = match chunk_res {
                                Ok(Ok(Some(bytes))) => bytes,
                                Ok(Ok(None)) => break,
                                Ok(Err(_)) => {
                                    return Err("The model stream was interrupted. No generated document was saved.".into());
                                }
                                Err(_) => {
                                    let fired_at = tokio::time::Instant::now();
                                    if fired_at >= workflow_deadline_at
                                        || wait_dur == remaining_overall
                                    {
                                        return Err(format!(
                                            "The model exceeded its maximum stream deadline of {} seconds. No generated document was saved.",
                                            policy.workflow_deadline.as_secs()
                                        ));
                                    } else if is_first_chunk {
                                        return Err(format!(
                                            "The model did not begin generating within {} seconds. Check provider connection.",
                                            policy.stream_initial_timeout.as_secs()
                                        ));
                                    } else {
                                        return Err(format!(
                                            "The model stream stalled for over {} seconds without emitting tokens. No generated document was saved.",
                                            policy.stream_inactivity_timeout.as_secs()
                                        ));
                                    }
                                }
                            };
                            is_first_chunk = false;
                            byte_buffer.extend_from_slice(&chunk_bytes);

                            while let Some(newline_pos) =
                                byte_buffer.iter().position(|&b| b == b'\n')
                            {
                                let line_bytes: Vec<u8> =
                                    byte_buffer.drain(..=newline_pos).collect();
                                let line = String::from_utf8_lossy(&line_bytes).trim().to_string();
                                if !line.starts_with("data:") {
                                    continue;
                                }
                                let data_str = line.trim_start_matches("data:").trim();
                                if data_str == "[DONE]" {
                                    stream_completed = true;
                                    break;
                                }

                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(data_str) {
                                    if v.get("error").is_some() {
                                        return Err("The model reported a generation error. Check provider settings.".into());
                                    }
                                    if let Some(reason) = v
                                        .pointer("/choices/0/finish_reason")
                                        .and_then(|v| v.as_str())
                                    {
                                        if reason != "stop" {
                                            return Err(format!("The model stopped before completing the document ({reason}). No generated document was saved."));
                                        }
                                        stream_completed = true;
                                    }
                                    if let Some(delta) = v
                                        .get("choices")
                                        .and_then(|c| c.get(0))
                                        .and_then(|c0| c0.get("delta"))
                                        .and_then(|d| d.get("content"))
                                        .and_then(|cnt| cnt.as_str())
                                    {
                                        if !delta.is_empty() {
                                            had_tokens = true;
                                            let mut chunks = Vec::new();
                                            parser.feed(delta, &mut chunks);
                                            for chunk in chunks {
                                                if sender.send(chunk).await.is_err() {
                                                    return Err("Generation delivery was interrupted. No generated document was saved.".into());
                                                }
                                            }
                                            tokio::task::yield_now().await;
                                        }
                                    }
                                }
                            }
                        }

                        let line = String::from_utf8_lossy(&byte_buffer);
                        let data = line.trim().strip_prefix("data:").map(str::trim);
                        if data == Some("[DONE]") {
                            stream_completed = true;
                        } else if let Some(data) = data {
                            let value: serde_json::Value =
                                serde_json::from_str(data).map_err(|_| {
                                    "The model returned an incomplete stream record.".to_string()
                                })?;
                            if value.get("error").is_some() {
                                return Err("The model reported a generation error.".into());
                            }
                            if let Some(reason) = value
                                .pointer("/choices/0/finish_reason")
                                .and_then(|v| v.as_str())
                            {
                                if reason != "stop" {
                                    return Err(format!("The model stopped before completing the document ({reason})."));
                                }
                                stream_completed = true;
                            }
                            if let Some(token) = value
                                .pointer("/choices/0/delta/content")
                                .and_then(|v| v.as_str())
                            {
                                had_tokens |= !token.is_empty();
                                let mut chunks = Vec::new();
                                parser.feed(token, &mut chunks);
                                for chunk in chunks {
                                    if sender.send(chunk).await.is_err() {
                                        return Err("Generation was interrupted.".into());
                                    }
                                }
                            }
                        }
                        if had_tokens {
                            streamed_live = true;
                        }
                    }
                }
            }
        }
    }

    if !streamed_live || !stream_completed {
        return Err("The selected model did not return a complete response. No generated document was saved; your prompt remains available.".into());
    }

    let mut final_chunks = Vec::new();
    let duration_ms = (engine.clock().now_ms().saturating_sub(start_ms)) as u64;
    parser.finish(&mut final_chunks, duration_ms);

    for chunk in final_chunks {
        let _ = sender.send(chunk).await;
    }

    Ok(())
}

/// Generates progressive token stream tailored to operational mode and intent.
pub fn generate_simulated_token_stream(raw_input: &str, mode: OperationalMode) -> Vec<String> {
    // Deterministic test fixture only; production requires an available model.
    match mode {
        OperationalMode::Plan => vec![
            format!("## Plan: {}\n\n", raw_input.trim()),
            "- [ ] Define the goal\n- [ ] Gather evidence\n- [ ] Review the result\n".into(),
        ],
        OperationalMode::Research => vec![
            format!("## Research: {}\n\n", raw_input.trim()),
            "Compare the available evidence and identify open questions.\n".into(),
        ],
        _ => vec![format!("## {}\n", raw_input.trim())],
    }
}
