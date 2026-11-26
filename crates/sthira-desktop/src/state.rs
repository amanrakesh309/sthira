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
    /// Do not replace a live token when a duplicate agent dispatch arrives.
    pub fn register_unique(&self, request_id: &str) -> Option<CancellationToken> {
        let mut map = self.tokens.lock().ok()?;
        if map.contains_key(request_id) {
            return None;
        }
        let token = CancellationToken::new();
        map.insert(request_id.to_string(), token.clone());
        Some(token)
    }

    pub fn cancel(&self, request_id: &str) -> bool {
        self.cancel_with_outcome(request_id) == Some(CancelOutcome::Cancelled)
    }

    pub fn cancel_agent_tasks(&self) {
        if let Ok(map) = self.tokens.lock() {
            for (id, token) in map.iter() {
                if id.starts_with("workspace-agent:") || id.starts_with("migration-guidance:") {
                    token.cancel();
                }
            }
        }
    }
    pub fn is_registered(&self, request_id: &str) -> bool {
        self.tokens
            .lock()
            .is_ok_and(|map| map.contains_key(request_id))
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
