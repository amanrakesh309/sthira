use crate::{
    db::providers_repo,
    models::router::ProviderModelEntry,
    CoreError,
};
use rusqlite::Connection;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Option<Vec<OllamaTagItem>>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagItem {
    name: Option<String>,
    model: Option<String>,
    details: Option<OllamaTagDetails>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagDetails {
    context_length: Option<u32>,
    parameter_size: Option<String>,
}


#[derive(Debug, Deserialize)]
struct OpenAIModelItem {
    id: String,
    name: Option<String>,
    context_length: Option<u32>,
    context_window: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModelsResponse {
    data: Option<Vec<AnthropicModelItem>>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModelItem {
    id: String,
    display_name: Option<String>,
}

pub async fn discover_local_ollama_models(
    endpoint: &str,
) -> Result<Vec<ProviderModelEntry>, CoreError> {
    let base = endpoint.trim_end_matches('/');
    let url = format!("{}/api/tags", base);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()
        .map_err(|e| CoreError::Model(format!("Failed to build HTTP client: {e}")))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| CoreError::Model(format!("Ollama connection failed at {url}: {e}")))?;

    if !resp.status().is_success() {
        return Err(CoreError::Model(format!(
            "Ollama /api/tags returned HTTP {}",
            resp.status()
        )));
    }

    let parsed: OllamaTagsResponse = resp
        .json()
        .await
        .map_err(|e| CoreError::Model(format!("Invalid /api/tags JSON: {e}")))?;

    let mut result = Vec::new();
    if let Some(items) = parsed.models {
        for item in items {
            let raw_tag = item.name.or(item.model).unwrap_or_default();
            if raw_tag.is_empty() {
                continue;
            }

            let ctx = item
                .details
                .as_ref()
                .and_then(|d| d.context_length)
                .unwrap_or(8192);

            let param_suffix = item
                .details
                .as_ref()
                .and_then(|d| d.parameter_size.as_ref())
                .map(|p| format!(" ({})", p))
                .unwrap_or_default();

            let clean_name = raw_tag.replace(":latest", "");
            let display_name = format!("{}{}", clean_name, param_suffix);

            result.push(ProviderModelEntry {
                id: format!("ollama:{}", raw_tag),
                provider_id: "ollama".to_string(),
                model_identifier: raw_tag,
                display_name,
                context_window: ctx,
                supports_structured_output: true,
                is_enabled: true,
                is_default: false,
            });
        }
    }

    Ok(result)
}

pub async fn discover_provider_models(
    provider_id: &str,
    provider_type: &str,
    endpoint: &str,
    api_key: Option<&str>,
) -> Result<Vec<ProviderModelEntry>, CoreError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| CoreError::Model(format!("Failed to build HTTP client: {e}")))?;

    match provider_type {
        "ollama" => discover_local_ollama_models(endpoint).await,
        "anthropic" => {
            let base = endpoint.trim_end_matches('/');
            let url = if base.ends_with("/v1") {
                format!("{base}/models")
            } else if base.ends_with("/models") {
                base.to_string()
            } else {
                format!("{base}/v1/models")
            };

            let mut req = client.get(&url).header("anthropic-version", "2023-06-01");
            if let Some(key) = api_key {
                if !key.is_empty() {
                    req = req.header("x-api-key", key);
                }
            }

            let resp = req
                .send()
                .await
                .map_err(|e| CoreError::Model(format!("Anthropic connection failed at {url}: {e}")))?;

            if !resp.status().is_success() {
                return Err(CoreError::Model(format!(
                    "Anthropic /v1/models returned HTTP {}",
                    resp.status()
                )));
            }

            let parsed: AnthropicModelsResponse = resp
                .json()
                .await
                .map_err(|e| CoreError::Model(format!("Invalid Anthropic models JSON: {e}")))?;

            let mut result = Vec::new();
            if let Some(items) = parsed.data {
                for item in items {
                    let disp = item.display_name.unwrap_or_else(|| item.id.clone());
                    result.push(ProviderModelEntry {
                        id: format!("{provider_id}:{}", item.id),
                        provider_id: provider_id.to_string(),
                        model_identifier: item.id,
                        display_name: disp,
                        context_window: 200000,
                        supports_structured_output: true,
                        is_enabled: true,
                        is_default: false,
                    });
                }
            }
            Ok(result)
        }
        _ => {
            // OpenAI-compatible, OpenRouter, Groq, DeepSeek, Mistral, Custom
            let base = endpoint.trim_end_matches('/');
            let url = if base.ends_with("/models") {
                base.to_string()
            } else if base.ends_with("/v1") || base.contains("/v1/") {
                format!("{base}/models")
            } else if base.contains("openai.com")
                || base.contains("deepseek.com")
                || base.contains("openrouter.ai")
                || base.contains("groq.com")
                || base.contains("mistral.ai")
            {
                format!("{base}/v1/models")
            } else {
                format!("{base}/models")
            };

            let mut req = client.get(&url);
            if let Some(key) = api_key {
                if !key.is_empty() {
                    req = req.header("Authorization", format!("Bearer {key}"));
                }
            }
            if base.contains("openrouter.ai") {
                req = req
                    .header("HTTP-Referer", "https://sthira.local")
                    .header("X-Title", "Sthira");
            }

            let resp = req
                .send()
                .await
                .map_err(|e| CoreError::Model(format!("Provider connection failed at {url}: {e}")))?;

            if !resp.status().is_success() {
                return Err(CoreError::Model(format!(
                    "Provider /models returned HTTP {}",
                    resp.status()
                )));
            }

            let parsed_val: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| CoreError::Model(format!("Invalid OpenAI-compatible models JSON: {e}")))?;

            let items: Vec<OpenAIModelItem> = if let Some(data) = parsed_val.get("data") {
                serde_json::from_value(data.clone()).unwrap_or_default()
            } else if parsed_val.is_array() {
                serde_json::from_value(parsed_val).unwrap_or_default()
            } else {
                Vec::new()
            };

            let mut result = Vec::new();
            for item in items {
                let ctx = item.context_length.or(item.context_window).unwrap_or(128000);
                let disp = item.name.unwrap_or_else(|| item.id.clone());
                result.push(ProviderModelEntry {
                    id: format!("{provider_id}:{}", item.id),
                    provider_id: provider_id.to_string(),
                    model_identifier: item.id,
                    display_name: disp,
                    context_window: ctx,
                    supports_structured_output: true,
                    is_enabled: true,
                    is_default: false,
                });
            }
            Ok(result)
        }
    }
}

pub fn sync_discovered_models(
    conn: &Connection,
    models: &[ProviderModelEntry],
    now_ms: i64,
) -> Result<(), CoreError> {
    for model in models {
        providers_repo::upsert_model(conn, model, now_ms)?;
    }
    Ok(())
}

