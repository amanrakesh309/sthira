use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use sthira_core::Engine;

pub fn spawn_event_bridge(app_handle: AppHandle, engine: Arc<Engine>) {
    tauri::async_runtime::spawn(async move {
        let mut rx = engine.subscribe();
        while let Ok(event) = rx.recv().await {
            let _ = app_handle.emit("sthira://core-event", &event);
        }
    });
}
