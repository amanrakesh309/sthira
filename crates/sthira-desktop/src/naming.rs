use crate::{resume, state::AppState};
use serde::{Deserialize, Serialize};
use sthira_core::Engine;
use tauri::State;
#[derive(Clone, Debug, Default, Deserialize)]
pub struct NamingChoice {
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub local: bool,
}
#[derive(Debug, Serialize)]
pub struct TitleSuggestions {
    pub artifact_id: String,
    pub basis_content_hash: String,
    pub basis_title: Option<String>,
    pub titles: Vec<String>,
    pub model_name: String,
    pub provider_name: String,
}
fn selection(engine: &Engine, choice: &NamingChoice) -> Result<resume::ResumeSelection, String> {
    let mut settings = resume::get_settings(engine)?;
    // The explicit Suggest action opts into this naming request, independently of
    // the switch controlling Resume summaries. It does not change that switch.
    settings.enabled = true;
    if choice.local {
        settings.preferences.model_mode = "local".into();
        settings.preferences.model_id = None;
    } else if let Some(id) = &choice.model_id {
        settings.preferences.model_mode = "workspace".into();
        settings.preferences.model_id = Some(id.clone());
    }
    resume::select_model_with_policy(
        engine,
        &settings,
        sthira_core::llm::lifecycle::RequestPolicy::quick_task(),
    )?
    .ok_or("Choose a Resume model in Settings or select a connected model here.".into())
}
pub async fn suggest_titles(
    engine: &Engine,
    artifact_id: &str,
    choice: NamingChoice,
) -> Result<TitleSuggestions, String> {
    let artifact = engine
        .get_artifact(artifact_id)
        .map_err(|e| e.to_string())?;
    if artifact.deleted_at.is_some() {
        return Err("This document was deleted.".into());
    }
    let selected = selection(engine, &choice)?;
    let titles = sthira_core::naming::suggest(selected.model.as_ref(), &artifact.content_md)
        .await
        .map_err(|e| e.to_string())?;
    let current = engine
        .get_artifact(artifact_id)
        .map_err(|e| e.to_string())?;
    if current.deleted_at.is_some()
        || current.content_hash != artifact.content_hash
        || current.title != artifact.title
    {
        return Err("The document changed. Request new title suggestions.".into());
    }
    if selection(engine, &choice)?.identity.cache_key != selected.identity.cache_key {
        return Err("The selected model changed. Request new title suggestions.".into());
    }
    Ok(TitleSuggestions {
        artifact_id: artifact.id,
        basis_content_hash: artifact.content_hash,
        basis_title: artifact.title,
        titles,
        model_name: selected.identity.name,
        provider_name: selected.identity.provider_name,
    })
}
#[tauri::command]
pub async fn cmd_suggest_document_titles(
    artifact_id: String,
    choice: NamingChoice,
    state: State<'_, AppState>,
) -> Result<TitleSuggestions, String> {
    suggest_titles(&state.engine, &artifact_id, choice).await
}

#[tauri::command]
pub fn cmd_apply_document_title(
    artifact_id: String,
    title: String,
    basis_hash: String,
    basis_title: Option<String>,
    state: State<'_, AppState>,
) -> Result<sthira_core::Artifact, String> {
    state
        .engine
        .apply_suggested_title(&artifact_id, &title, &basis_hash, basis_title.as_deref())
}
