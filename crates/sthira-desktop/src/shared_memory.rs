use crate::AppState;
use sthira_core::{
    shared_memory::{MemoryAccess, MemoryAction, MemoryResult},
    Engine,
};
use tauri::State;

/// Desktop resolves identity itself; the IPC payload cannot supply grants,
/// authorship, timestamps, or a model identity.
pub fn handle_memory_action(engine: &Engine, action: MemoryAction) -> Result<MemoryResult, String> {
    engine
        .execute_memory_action(&MemoryAccess::owner(), action)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_memory_action(
    state: State<'_, AppState>,
    action: MemoryAction,
) -> Result<MemoryResult, String> {
    handle_memory_action(&state.engine, action)
}
