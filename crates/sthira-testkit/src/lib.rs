use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use sthira_core::{Clock, CoreError, Locality, ModelProvider};

#[derive(Debug)]
pub struct FakeClock {
    now_ms: AtomicI64,
}

impl FakeClock {
    pub fn new(start_ms: i64) -> Self {
        Self {
            now_ms: AtomicI64::new(start_ms),
        }
    }

    pub fn advance(&self, duration: Duration) {
        self.now_ms
            .fetch_add(duration.as_millis() as i64, Ordering::SeqCst);
    }

    pub fn advance_ms(&self, ms: i64) {
        self.now_ms.fetch_add(ms, Ordering::SeqCst);
    }

    pub fn set(&self, ms: i64) {
        self.now_ms.store(ms, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> i64 {
        self.now_ms.load(Ordering::SeqCst)
    }
}

pub struct DeterministicFakeModel {
    id: String,
    locality: Locality,
    canned_json_responses: Mutex<HashMap<String, String>>,
    embedding_dim: usize,
    panic_on_call: bool,
    call_count: std::sync::atomic::AtomicUsize,
}

impl DeterministicFakeModel {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            locality: Locality::Local,
            canned_json_responses: Mutex::new(HashMap::new()),
            embedding_dim: 128,
            panic_on_call: false,
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn with_panic_on_call(mut self) -> Self {
        self.panic_on_call = true;
        self
    }

    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    pub fn reset_call_count(&self) {
        self.call_count.store(0, Ordering::SeqCst);
    }

    pub fn register_response(&self, key: impl Into<String>, resp: impl Into<String>) {
        self.canned_json_responses
            .lock()
            .unwrap()
            .insert(key.into(), resp.into());
    }
}

#[async_trait]
impl ModelProvider for DeterministicFakeModel {
    fn id(&self) -> &str {
        &self.id
    }

    fn locality(&self) -> Locality {
        self.locality
    }

    async fn generate_json(
        &self,
        _schema: &serde_json::Value,
        _system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, CoreError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        if self.panic_on_call {
            panic!("DeterministicFakeModel::generate_json called unexpectedly!");
        }

        let map = self.canned_json_responses.lock().unwrap();
        if let Some(resp) = map.get(user_prompt) {
            return Ok(resp.clone());
        }

        Ok("{}".to_string())
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, CoreError> {
        if self.panic_on_call {
            panic!("DeterministicFakeModel::embed called unexpectedly!");
        }

        let mut res = Vec::with_capacity(texts.len());
        for text in texts {
            let mut vec = vec![0.0f32; self.embedding_dim];
            let bytes = text.as_bytes();
            for (i, b) in bytes.iter().enumerate() {
                vec[i % self.embedding_dim] += *b as f32 / 255.0;
            }
            let norm: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
            if norm > 0.0 {
                for v in &mut vec {
                    *v /= norm;
                }
            }
            res.push(vec);
        }
        Ok(res)
    }
}
