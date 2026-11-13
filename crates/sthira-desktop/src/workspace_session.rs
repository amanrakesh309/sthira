use crate::AppState;
use sthira_core::Engine;

pub fn handle_get_workspace_session_key(engine: &Engine) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let instance_id = engine.vault_instance_id().map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    hasher.update(engine.vault_dir().to_string_lossy().as_bytes());
    hasher.update(b"\0");
    hasher.update(instance_id.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

#[tauri::command]
pub fn cmd_get_workspace_session_key(state: tauri::State<'_, AppState>) -> Result<String, String> {
    handle_get_workspace_session_key(&state.engine)
}
