use crate::AppState;

#[tauri::command]
pub fn cmd_get_workspace_session_key(state: tauri::State<'_, AppState>) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{:x}",
        Sha256::digest(state.engine.vault_dir().to_string_lossy().as_bytes())
    )
}
