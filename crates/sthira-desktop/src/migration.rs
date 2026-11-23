use crate::AppState;
use sthira_core::migration::{ImportDraft, ImportRun};
use tauri::State;
#[tauri::command]
pub fn cmd_preview_migration(
    state: State<'_, AppState>,
    draft: ImportDraft,
) -> Result<ImportRun, String> {
    state
        .engine
        .preview_migration(draft)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_apply_migration(
    state: State<'_, AppState>,
    id: String,
    revision: i64,
    paths: Vec<String>,
) -> Result<ImportRun, String> {
    state
        .engine
        .apply_migration(&id, revision, paths)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_list_migrations(state: State<'_, AppState>) -> Result<Vec<ImportRun>, String> {
    state.engine.list_migrations().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_get_migration(state: State<'_, AppState>, id: String) -> Result<ImportRun, String> {
    state.engine.migration_run(&id).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_cancel_migration(
    state: State<'_, AppState>,
    id: String,
    revision: i64,
) -> Result<ImportRun, String> {
    state
        .engine
        .cancel_migration(&id, revision)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_rollback_migration(
    state: State<'_, AppState>,
    id: String,
    revision: i64,
) -> Result<ImportRun, String> {
    state
        .engine
        .rollback_migration(&id, revision)
        .map_err(|e| e.to_string())
}
