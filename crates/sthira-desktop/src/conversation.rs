use crate::state::AppState;
use sthira_core::{conversation::Conversation, Artifact};
use tauri::State;

#[tauri::command]
pub fn cmd_get_document_conversation(
    state: State<'_, AppState>,
    artifact_id: String,
    mode: Option<String>,
    initial_prompt: Option<String>,
    model_id: Option<String>,
) -> Result<Option<Conversation>, String> {
    let conversation = state.engine.document_conversation(
        &artifact_id,
        mode.as_deref(),
        initial_prompt.as_deref().unwrap_or(""),
    )?;
    if model_id.is_some()
        || (conversation
            .as_ref()
            .is_some_and(|c| c.model_name.is_none())
            && matches!(mode.as_deref(), Some("plan" | "research")))
    {
        let resolved = {
            let reader = state.engine.db().reader().map_err(|e| e.to_string())?;
            sthira_core::llm::resolve_model_provider(
                &reader,
                model_id.as_deref(),
                state.engine.model(),
            )
            .map_err(|e| e.to_string())?
        };
        let id = if resolved.provider_id == "default" {
            None
        } else {
            Some(resolved.model_id.as_str())
        };
        state
            .engine
            .remember_conversation_model(&artifact_id, id, &resolved.display_name)?;
    }
    state.engine.document_conversation(&artifact_id, None, "")
}

#[tauri::command]
pub async fn cmd_discuss_document(
    state: State<'_, AppState>,
    artifact_id: String,
    text: String,
    model_id: Option<String>,
    propose_edit: bool,
) -> Result<Conversation, String> {
    let remembered = state
        .engine
        .document_conversation(&artifact_id, None, "")?
        .and_then(|chat| chat.model_id);
    let model_id =
        crate::workflow_settings::workflow_model(&state.engine, "chat", model_id.or(remembered))?;
    state
        .engine
        .discuss_document(&artifact_id, &text, model_id.as_deref(), propose_edit)
        .await
}
#[tauri::command]
pub fn cmd_decide_document_edit(
    state: State<'_, AppState>,
    artifact_id: String,
    turn_id: String,
    apply: bool,
) -> Result<Artifact, String> {
    state
        .engine
        .decide_document_edit(&artifact_id, &turn_id, apply)
}

#[tauri::command]
pub fn cmd_undo_document_edit(
    state: State<'_, AppState>,
    artifact_id: String,
    change_set_id: String,
) -> Result<Artifact, String> {
    state
        .engine
        .undo_document_edit(&artifact_id, &change_set_id)
}

#[tauri::command]
pub fn cmd_select_document_conversation_model(
    state: State<'_, AppState>,
    artifact_id: String,
    model_id: String,
) -> Result<Conversation, String> {
    state
        .engine
        .select_conversation_model(&artifact_id, &model_id)
}
