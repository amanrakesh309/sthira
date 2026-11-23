use std::sync::Arc;
use sthira_core::Engine;
use tauri::{AppHandle, Emitter};

pub fn spawn_event_bridge(app_handle: AppHandle, engine: Arc<Engine>) {
    let external_engine = engine.clone();
    let external_app = app_handle.clone();
    let mut cursor = engine.external_event_cursor().unwrap_or(0);
    tauri::async_runtime::spawn(async move {
        let mut timer = tokio::time::interval(std::time::Duration::from_millis(1000));
        loop {
            timer.tick().await;
            if let Ok(events) = external_engine.external_events_after(cursor) {
                for (seq, event) in events {
                    let _ = external_app.emit("sthira://core-event", &event);
                    cursor = seq;
                }
            }
        }
    });
    tauri::async_runtime::spawn(async move {
        let mut rx = engine.subscribe();
        while let Ok(event) = rx.recv().await {
            let _ = app_handle.emit("sthira://core-event", &event);
        }
    });
}
