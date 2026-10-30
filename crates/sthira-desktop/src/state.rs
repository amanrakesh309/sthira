use std::sync::Arc;
use sthira_core::Engine;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Engine>,
}

impl AppState {
    pub fn new(engine: Arc<Engine>) -> Self {
        Self { engine }
    }
}
