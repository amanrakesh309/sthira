use crate::{CoreError, Locality, ModelProvider};
use async_trait::async_trait;
use serde_json::Value;

pub struct AnthropicProvider {
    model_name: String,
    base_url: String,
    api_key: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(model_name: impl Into<String>) -> Self {
        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com/v1".to_string());
        let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();

        Self {
            model_name: model_name.into(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            client: reqwest::Client::new(),
        }
    }

    pub fn with_credentials(
        model_name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        Self {
            model_name: model_name.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            client,
        }
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    fn id(&self) -> &str {
        &self.model_name
    }

    fn locality(&self) -> Locality {
        Locality::Cloud
    }

    async fn generate_json(
        &self,
        schema: &Value,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, CoreError> {
        let url = format!("{}/messages", self.base_url);
        let schema_str = serde_json::to_string(schema).unwrap_or_else(|_| "{}".to_string());
        let augmented_system = format!(
            "{}\n\nYou MUST respond ONLY with valid JSON adhering to the following schema:\n{}",
            system_prompt, schema_str
        );

        let req_body = serde_json::json!({
            "model": self.model_name,
            "max_tokens": 4096,
            "system": augmented_system,
            "messages": [
                { "role": "user", "content": user_prompt }
            ]
        });

        let mut req = self
            .client
            .post(&url)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&req_body);

        if !self.api_key.is_empty() {
            req = req.header("x-api-key", &self.api_key);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| CoreError::Model(format!("Anthropic HTTP request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(CoreError::Model(format!(
                "Anthropic returned HTTP error {status}: {err_text}"
            )));
        }

        let resp_json: Value = resp
            .json()
            .await
            .map_err(|e| CoreError::Model(format!("Anthropic invalid JSON response: {e}")))?;

        let text = resp_json
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("{}");

        let cleaned = super::clean_json_fences(text);
        Ok(cleaned.to_string())
    }

    async fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, CoreError> {
        Err(CoreError::Model(
            "Embeddings are not directly supported on Anthropic provider; use local Ollama embedding models".to_string(),
        ))
    }
}
