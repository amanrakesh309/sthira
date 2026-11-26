pub mod external;

use std::time::SystemTime;
use sthira_core::{Clock, CoreError, Locality, ModelProvider};

pub struct SystemClock;
impl Clock for SystemClock {
    fn now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }
}

pub struct CliModel;
#[async_trait::async_trait]
impl ModelProvider for CliModel {
    fn id(&self) -> &str {
        "sthira-cli-local"
    }

    fn locality(&self) -> Locality {
        Locality::Local
    }

    async fn generate_json(
        &self,
        _schema: &serde_json::Value,
        _system_prompt: &str,
        _user_prompt: &str,
    ) -> Result<String, CoreError> {
        Err(CoreError::Model("No generation provider is configured in the CLI. Use the desktop model workflow or an external agent.".into()))
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, CoreError> {
        let dim = 128;
        let mut results = Vec::with_capacity(texts.len());
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
            results.push(vec);
        }
        Ok(results)
    }
}
