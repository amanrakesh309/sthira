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
    basis: String,
) -> Result<ImportRun, String> {
    let result = state
        .engine
        .apply_reviewed_migration(&id, revision, paths, &basis)
        .map_err(|e| e.to_string())?;
    stop_import_guidance(&state, &id);
    Ok(result)
}
#[tauri::command]
pub fn cmd_list_migrations(
    state: State<'_, AppState>,
    limit: Option<usize>,
    before: Option<HistoryCursor>,
) -> Result<ImportHistory, String> {
    state
        .engine
        .migration_history(limit.unwrap_or(10), before)
        .map_err(|e| e.to_string())
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
    let result = state
        .engine
        .cancel_migration(&id, revision)
        .map_err(|e| e.to_string())?;
    stop_import_guidance(&state, &id);
    Ok(result)
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

use sthira_core::migration::{
    guidance::{GuidanceInput, MigrationGuidance},
    HistoryCursor, ImportHistory, ImportSelection,
};
#[tauri::command]
pub fn cmd_review_migration_selection(
    state: State<'_, AppState>,
    id: String,
    revision: i64,
    paths: Vec<String>,
) -> Result<ImportSelection, String> {
    state
        .engine
        .review_migration_selection(&id, revision, paths)
        .map_err(|e| e.to_string())
}

pub fn start_migration_guidance(
    state: AppState,
    mut input: GuidanceInput,
) -> Result<MigrationGuidance, String> {
    input.model_id = crate::workflow_settings::workflow_model(
        &state.engine,
        "agent",
        if input.model_id.is_empty() {
            None
        } else {
            Some(input.model_id)
        },
    )?
    .ok_or("Choose an available guidance model.")?;
    let (job, created) = state
        .engine
        .queue_migration_guidance(input)
        .map_err(|e| e.to_string())?;
    if created {
        let id = job.id.clone();
        let request = format!("migration-guidance:{id}");
        let token = state
            .requests
            .register_unique(&request)
            .ok_or("This guidance task is already running.")?;
        tauri::async_runtime::spawn(async move {
            let _ = state.engine.execute_migration_guidance(&id, &token).await;
            state.requests.unregister(&request);
        });
    }
    Ok(job)
}
#[tauri::command]
pub fn cmd_start_migration_guidance(
    state: State<'_, AppState>,
    input: GuidanceInput,
) -> Result<MigrationGuidance, String> {
    start_migration_guidance(state.inner().clone(), input)
}
#[tauri::command]
pub fn cmd_get_migration_guidance(
    state: State<'_, AppState>,
    id: String,
) -> Result<MigrationGuidance, String> {
    state
        .engine
        .migration_guidance(&id)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_latest_migration_guidance(
    state: State<'_, AppState>,
    import_id: String,
) -> Result<Option<MigrationGuidance>, String> {
    state
        .engine
        .latest_migration_guidance(&import_id)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_cancel_migration_guidance(
    state: State<'_, AppState>,
    id: String,
) -> Result<MigrationGuidance, String> {
    let result = state
        .engine
        .cancel_migration_guidance(&id)
        .map_err(|e| e.to_string())?;
    state.requests.cancel(&format!("migration-guidance:{id}"));
    Ok(result)
}

fn stop_import_guidance(state: &AppState, id: &str) {
    if let Ok(Some(job)) = state.engine.latest_migration_guidance(id) {
        state
            .requests
            .cancel(&format!("migration-guidance:{}", job.id));
    }
}
