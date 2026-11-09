use crate::llm::lifecycle::{classify_reqwest_error, RequestErrorKind, RequestPolicy};
use crate::{CoreError, Locality, ModelProvider};
use async_trait::async_trait;
use serde_json::Value;

pub struct AnthropicProvider {
    model_name: String,
    base_url: String,
    api_key: String,
    client: reqwest::Client,
    policy: RequestPolicy,
}

impl AnthropicProvider {
    pub fn new(model_name: impl Into<String>) -> Self {
        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com/v1".to_string());
        let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
        let policy = RequestPolicy::standard();
        let client = reqwest::Client::builder()
            .connect_timeout(policy.connect_timeout)
            .build()
            .unwrap_or_default();

        Self {
            model_name: model_name.into(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            client,
            policy,
        }
    }

    pub fn with_credentials(
        model_name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        let policy = RequestPolicy::standard();
        let client = reqwest::Client::builder()
            .connect_timeout(policy.connect_timeout)
            .build()
            .unwrap_or_default();

        Self {
            model_name: model_name.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            client,
            policy,
        }
    }

    pub fn with_policy(mut self, policy: RequestPolicy) -> Self {
        self.policy = policy;
        self.client = reqwest::Client::builder()
            .connect_timeout(policy.connect_timeout)
            .build()
            .unwrap_or_default();
        self
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
            .timeout(self.policy.generation_budget)
            .send()
            .await
            .map_err(|e| {
                let kind = classify_reqwest_error(&e, &self.policy);
                CoreError::Request(kind)
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(CoreError::Request(RequestErrorKind::HttpStatus {
                status: status.as_u16(),
                message: err_text,
            }));
        }

        let resp_json: Value = resp.json().await.map_err(|e| {
            if e.is_timeout() {
                CoreError::Request(RequestErrorKind::GenerationDeadline {
                    limit: self.policy.generation_budget,
                })
            } else {
                CoreError::Request(RequestErrorKind::MalformedResponse {
                    details: e.to_string(),
                })
            }
        })?;

        let text = resp_json
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");

        if text.trim().is_empty() {
            return Err(CoreError::Request(RequestErrorKind::EmptyResponse));
        }

        let cleaned = super::clean_json_fences(text);
        Ok(cleaned.to_string())
    }

    async fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, CoreError> {
        Err(CoreError::Model(
            "Embeddings are not directly supported on Anthropic provider; use local Ollama embedding models".to_string(),
        ))
    }
}
