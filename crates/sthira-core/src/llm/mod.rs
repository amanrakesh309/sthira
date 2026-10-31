pub mod anthropic;
pub mod autodiscover;
pub mod dispatcher;
pub mod models;
pub mod streaming;

pub use anthropic::AnthropicProvider;
pub use autodiscover::{discover_local_ollama_models, sync_discovered_models};
pub use dispatcher::{resolve_model_provider, ResolvedModel};
pub use models::*;
pub use streaming::*;

use crate::{CoreError, Locality, ModelProvider};
use async_trait::async_trait;
use serde_json::Value;

pub struct OllamaProvider {
    model_name: String,
    host: String,
    locality: Locality,
    client: reqwest::Client,
    embed_model: String,
}

impl OllamaProvider {
    pub fn new(model_name: impl Into<String>, locality: Locality) -> Self {
        let host = std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        let embed_model = std::env::var("OLLAMA_EMBED_MODEL")
            .unwrap_or_else(|_| "bge-m3:latest".to_string());

        Self {
            model_name: model_name.into(),
            host: host.trim_end_matches('/').to_string(),
            locality,
            client: reqwest::Client::new(),
            embed_model,
        }
    }

    pub fn with_host(
        model_name: impl Into<String>,
        host: impl Into<String>,
        locality: Locality,
    ) -> Self {
        let embed_model = std::env::var("OLLAMA_EMBED_MODEL")
            .unwrap_or_else(|_| "bge-m3:latest".to_string());

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        Self {
            model_name: model_name.into(),
            host: host.into().trim_end_matches('/').to_string(),
            locality,
            client,
            embed_model,
        }
    }

    pub fn with_embed_model(mut self, embed_model: impl Into<String>) -> Self {
        self.embed_model = embed_model.into();
        self
    }
}

#[async_trait]
impl ModelProvider for OllamaProvider {
    fn id(&self) -> &str {
        &self.model_name
    }

    fn locality(&self) -> Locality {
        self.locality
    }

    async fn generate_json(
        &self,
        schema: &Value,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, CoreError> {
        let url = format!("{}/api/generate", self.host);
        let schema_str = serde_json::to_string(schema).unwrap_or_else(|_| "{}".to_string());
        let concise_example = if schema_str.contains("RawExtractedNode") || schema_str.contains("ExtractionOutput") {
            "\nExpected format example: {\"nodes\": [{\"node_type\": \"decision\", \"statement\": \"...\", \"quote\": \"...\"}]}"
        } else {
            ""
        };
        let augmented_system = format!(
            "{system_prompt}\n\nRespond strictly with valid JSON conforming to the following JSON schema:\n{schema_str}{concise_example}"
        );

        let req_body = serde_json::json!({
            "model": self.model_name,
            "system": augmented_system,
            "prompt": user_prompt,
            "format": "json",
            "stream": false
        });

        let resp = self
            .client
            .post(&url)
            .json(&req_body)
            .send()
            .await
            .map_err(|e| CoreError::Model(format!("Ollama HTTP request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(CoreError::Model(format!(
                "Ollama returned HTTP error {status}: {err_text}"
            )));
        }

        let resp_json: Value = resp
            .json()
            .await
            .map_err(|e| CoreError::Model(format!("Ollama invalid JSON response: {e}")))?;

        let raw_text = resp_json
            .get("response")
            .and_then(|v| v.as_str())
            .unwrap_or("{}");

        let cleaned = clean_json_fences(raw_text);
        Ok(cleaned.to_string())
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, CoreError> {
        let url = format!("{}/api/embed", self.host);
        let req_body = serde_json::json!({
            "model": self.embed_model,
            "input": texts
        });

        let resp = self
            .client
            .post(&url)
            .json(&req_body)
            .send()
            .await
            .map_err(|e| CoreError::Model(format!("Ollama embed request failed: {e}")))?;

        if !resp.status().is_success() {
            let dim = 128;
            let mut fallback = Vec::with_capacity(texts.len());
            for text in texts {
                let mut vec = vec![0.0f32; dim];
                for (i, b) in text.as_bytes().iter().enumerate() {
                    vec[i % dim] += *b as f32 / 255.0;
                }
                let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for v in &mut vec {
                        *v /= norm;
                    }
                }
                fallback.push(vec);
            }
            return Ok(fallback);
        }

        let resp_json: Value = resp
            .json()
            .await
            .map_err(|e| CoreError::Model(format!("Ollama embed invalid response: {e}")))?;

        if let Some(arr) = resp_json.get("embeddings").and_then(|v| v.as_array()) {
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                if let Some(nums) = item.as_array() {
                    let vec: Vec<f32> = nums
                        .iter()
                        .filter_map(|n| n.as_f64().map(|f| f as f32))
                        .collect();
                    out.push(vec);
                }
            }
            return Ok(out);
        }

        Err(CoreError::Model(
            "Ollama embed response missing embeddings field".to_string(),
        ))
    }
}

pub struct OpenAiCompatibleProvider {
    model_name: String,
    base_url: String,
    api_key: String,
    locality: Locality,
    client: reqwest::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new(model_name: impl Into<String>, locality: Locality) -> Self {
        let base_url = std::env::var("OPENAI_API_BASE")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();

        Self {
            model_name: model_name.into(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            locality,
            client: reqwest::Client::new(),
        }
    }

    pub fn with_credentials(
        model_name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        locality: Locality,
    ) -> Self {
        Self {
            model_name: model_name.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            locality,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ModelProvider for OpenAiCompatibleProvider {
    fn id(&self) -> &str {
        &self.model_name
    }

    fn locality(&self) -> Locality {
        self.locality
    }

    async fn generate_json(
        &self,
        schema: &Value,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, CoreError> {
        let url = format!("{}/chat/completions", self.base_url);
        let augmented_system = format!(
            "{}\n\nYou MUST respond ONLY with valid JSON conforming to this schema:\n{}",
            system_prompt, schema
        );

        let req_body = serde_json::json!({
            "model": self.model_name,
            "messages": [
                { "role": "system", "content": augmented_system },
                { "role": "user", "content": user_prompt }
            ],
            "response_format": { "type": "json_object" }
        });

        let mut req = self.client.post(&url).json(&req_body);
        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }
        if self.base_url.contains("openrouter.ai") {
            req = req
                .header("HTTP-Referer", "https://sthira.local")
                .header("X-Title", "Sthira");
        }

        let resp = req
            .send()
            .await
            .map_err(|e| CoreError::Model(format!("OpenAI HTTP request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(CoreError::Model(format!(
                "OpenAI returned HTTP error {status}: {err_text}"
            )));
        }

        let resp_json: Value = resp
            .json()
            .await
            .map_err(|e| CoreError::Model(format!("OpenAI invalid JSON response: {e}")))?;

        let content = resp_json
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|ch| ch.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or("{}");

        let cleaned = clean_json_fences(content);
        Ok(cleaned.to_string())
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, CoreError> {
        let dim = 128;
        let mut fallback = Vec::with_capacity(texts.len());
        for text in texts {
            let mut vec = vec![0.0f32; dim];
            for (i, b) in text.as_bytes().iter().enumerate() {
                vec[i % dim] += *b as f32 / 255.0;
            }
            let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
            if norm > 0.0 {
                for v in &mut vec {
                    *v /= norm;
                }
            }
            fallback.push(vec);
        }
        Ok(fallback)
    }
}

fn clean_json_fences(raw: &str) -> &str {
    let mut text = raw.trim();
    if let Some(end_think) = text.find("</think>") {
        text = text[end_think + 8..].trim();
    }
    if let Some(start) = text.find("```json") {
        let after_fence = &text[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return after_fence[..end].trim();
        }
    }
    if let Some(start) = text.find("```") {
        let after_fence = &text[start + 3..];
        if let Some(end) = after_fence.find("```") {
            return after_fence[..end].trim();
        }
    }
    if let (Some(obj_start), Some(obj_end)) = (text.find('{'), text.rfind('}')) {
        if obj_end > obj_start {
            return &text[obj_start..=obj_end];
        }
    }
    if let (Some(arr_start), Some(arr_end)) = (text.find('['), text.rfind(']')) {
        if arr_end > arr_start {
            return &text[arr_start..=arr_end];
        }
    }
    text
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskTier {
    FastLocal,
    SynthesisPrimary,
    DeepReview,
}

pub struct TieredModelProvider {
    fast_local: std::sync::Arc<dyn ModelProvider>,
    synthesis_primary: std::sync::Arc<dyn ModelProvider>,
    deep_review: std::sync::Arc<dyn ModelProvider>,
}

impl TieredModelProvider {
    pub fn new(
        fast_local: std::sync::Arc<dyn ModelProvider>,
        synthesis_primary: std::sync::Arc<dyn ModelProvider>,
        deep_review: std::sync::Arc<dyn ModelProvider>,
    ) -> Self {
        Self {
            fast_local,
            synthesis_primary,
            deep_review,
        }
    }

    pub fn from_env() -> Self {
        let host = std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());

        let fast_local_model = std::env::var("STHIRA_FAST_LOCAL_MODEL")
            .unwrap_or_else(|_| "llama3.2:latest".to_string());
        let synthesis_model = std::env::var("STHIRA_SYNTHESIS_MODEL")
            .unwrap_or_else(|_| "gemma4:31b-cloud".to_string());
        let deep_review_model = std::env::var("STHIRA_DEEP_REVIEW_MODEL")
            .unwrap_or_else(|_| "gpt-oss:120b-cloud".to_string());

        let fast_local = std::sync::Arc::new(OllamaProvider::with_host(
            fast_local_model,
            &host,
            Locality::Local,
        ));
        let synthesis_primary = std::sync::Arc::new(OllamaProvider::with_host(
            synthesis_model,
            &host,
            Locality::Cloud,
        ));
        let deep_review = std::sync::Arc::new(OllamaProvider::with_host(
            deep_review_model,
            &host,
            Locality::Cloud,
        ));

        Self::new(fast_local, synthesis_primary, deep_review)
    }

    pub fn fast_local(&self) -> std::sync::Arc<dyn ModelProvider> {
        std::sync::Arc::clone(&self.fast_local)
    }

    pub fn synthesis_primary(&self) -> std::sync::Arc<dyn ModelProvider> {
        std::sync::Arc::clone(&self.synthesis_primary)
    }

    pub fn deep_review(&self) -> std::sync::Arc<dyn ModelProvider> {
        std::sync::Arc::clone(&self.deep_review)
    }

    pub async fn generate_for_task(
        &self,
        tier: TaskTier,
        schema: &Value,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, CoreError> {
        match tier {
            TaskTier::FastLocal => {
                match self.fast_local.generate_json(schema, system_prompt, user_prompt).await {
                    Ok(res) => Ok(res),
                    Err(e) => {
                        self.synthesis_primary
                            .generate_json(schema, system_prompt, user_prompt)
                            .await
                            .map_err(|_| e)
                    }
                }
            }
            TaskTier::SynthesisPrimary => {
                match self.synthesis_primary.generate_json(schema, system_prompt, user_prompt).await {
                    Ok(res) => Ok(res),
                    Err(_) => {
                        // Automatic fallback to fast_local if cloud is unreachable
                        self.fast_local.generate_json(schema, system_prompt, user_prompt).await
                    }
                }
            }
            TaskTier::DeepReview => {
                match self.deep_review.generate_json(schema, system_prompt, user_prompt).await {
                    Ok(res) => Ok(res),
                    Err(_) => {
                        match self.synthesis_primary.generate_json(schema, system_prompt, user_prompt).await {
                            Ok(res) => Ok(res),
                            Err(_) => {
                                self.fast_local.generate_json(schema, system_prompt, user_prompt).await
                            }
                        }
                    }
                }
            }
        }
    }
}

#[async_trait]
impl ModelProvider for TieredModelProvider {
    fn id(&self) -> &str {
        "tiered-model-provider"
    }

    fn locality(&self) -> Locality {
        self.synthesis_primary.locality()
    }

    async fn generate_json(
        &self,
        schema: &Value,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, CoreError> {
        let sys_lower = system_prompt.to_lowercase();
        let user_lower = user_prompt.to_lowercase();
        let schema_str = schema.to_string();

        let tier = if sys_lower.contains("router")
            || sys_lower.contains("classify the user")
            || schema_str.contains("RouterPlan")
        {
            TaskTier::FastLocal
        } else if sys_lower.contains("level 2")
            || user_lower.contains("level 2")
            || sys_lower.contains("expanded review")
            || user_lower.contains("deep review")
        {
            TaskTier::DeepReview
        } else {
            TaskTier::SynthesisPrimary
        };

        self.generate_for_task(tier, schema, system_prompt, user_prompt).await
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, CoreError> {
        match self.fast_local.embed(texts).await {
            Ok(v) => Ok(v),
            Err(_) => self.synthesis_primary.embed(texts).await,
        }
    }
}
