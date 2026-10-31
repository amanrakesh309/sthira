#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use sthira_core::{llm::TieredModelProvider, Clock, Engine};
use sthira_desktop::{
    register_commands, seed_demo_workspace_if_empty, spawn_event_bridge,
    spawn_scheduled_topic_scheduler, AppState,
};

struct DesktopClock;
impl Clock for DesktopClock {
    fn now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }
}

fn resolve_vault_path() -> PathBuf {
    if let Ok(p) = std::env::var("STHIRA_VAULT_DIR") {
        PathBuf::from(p)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".sthira").join("vault")
    }
}

fn main() {
    let vault_path = resolve_vault_path();
    let _ = std::fs::create_dir_all(&vault_path);
    let db_path = vault_path.join("sthira.db");

    let clock: Arc<dyn Clock> = Arc::new(DesktopClock);
    let model: Arc<dyn sthira_core::ModelProvider> = Arc::new(TieredModelProvider::from_env());

    let engine = Arc::new(
        Engine::open(&db_path, &vault_path, clock, model).expect("Failed to initialize Sthira Engine"),
    );

    // Strip duplicate models on startup
    let _ = sthira_desktop::clean_duplicate_models_on_startup(&engine);

    // Only seed demo workspace if STHIRA_SEED_DEMO environment variable is set
    if std::env::var("STHIRA_SEED_DEMO").is_ok() {
        let _ = seed_demo_workspace_if_empty(&engine);
    }

    let app_state = AppState::new(Arc::clone(&engine));

    let builder = tauri::Builder::default()
        .manage(app_state)
        .setup(move |app| {
            spawn_event_bridge(app.handle().clone(), engine.clone());
            spawn_scheduled_topic_scheduler(app.handle().clone(), engine);
            Ok(())
        });

    register_commands(builder)
        .run(tauri::generate_context!())
        .expect("error while running sthira desktop application");
}
