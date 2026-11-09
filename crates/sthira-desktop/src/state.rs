use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use sthira_core::llm::lifecycle::{CancelOutcome, CancellationToken};
use sthira_core::Engine;

#[derive(Default, Clone)]
pub struct RequestRegistry {
    tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl RequestRegistry {
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register(&self, request_id: &str) -> CancellationToken {
        let token = CancellationToken::new();
        if let Ok(mut map) = self.tokens.lock() {
            map.insert(request_id.to_string(), token.clone());
        }
        token
    }

    pub fn cancel_with_outcome(&self, request_id: &str) -> Option<CancelOutcome> {
        if let Ok(map) = self.tokens.lock() {
            if let Some(token) = map.get(request_id) {
                return Some(token.cancel_with_outcome());
            }
        }
        None
    }

    pub fn cancel(&self, request_id: &str) -> bool {
        self.cancel_with_outcome(request_id) == Some(CancelOutcome::Cancelled)
    }

    pub fn unregister(&self, request_id: &str) {
        if let Ok(mut map) = self.tokens.lock() {
            map.remove(request_id);
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Engine>,
    pub requests: RequestRegistry,
}

impl AppState {
    pub fn new(engine: Arc<Engine>) -> Self {
        Self {
            engine,
            requests: RequestRegistry::new(),
        }
    }
}
