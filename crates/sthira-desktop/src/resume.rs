use crate::{state::AppState, workflow_settings};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use sthira_core::{
    db::providers_repo,
    resume::{ResumeModelIdentity, ResumePreferences},
    Engine, ModelProvider, ResumeBrief,
};
use tauri::State;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResumeSettings {
    pub enabled: bool,
    pub preferences: ResumePreferences,
}
pub fn get_settings(engine: &Engine) -> Result<ResumeSettings, String> {
    Ok(ResumeSettings {
        enabled: workflow_settings::get_workflow_settings(engine)?["resume"].enabled,
        preferences: engine
            .resume_preferences()
            .map_err(|error| error.to_string())?,
    })
}
pub fn save_settings(engine: &Engine, settings: ResumeSettings) -> Result<(), String> {
    settings.preferences.validate()?;
    if let Some(id) = settings
        .preferences
        .model_id
        .as_deref()
        .filter(|_| settings.enabled)
    {
        let reader = engine.db().reader().map_err(|error| error.to_string())?;
        if !providers_repo::get_model_entry(&reader, id)
            .map_err(|error| error.to_string())?
            .is_some_and(|(model, provider, _)| model.is_enabled && provider.is_enabled)
        {
            return Err("Choose an available model or the workspace default.".into());
        }
    }
    let mut writer = engine.db().writer();
    let tx = writer.transaction().map_err(|error| error.to_string())?;
    tx.execute("INSERT INTO resume_preferences(singleton_id,payload) VALUES(1,?1) ON CONFLICT(singleton_id) DO UPDATE SET payload=excluded.payload",[serde_json::to_string(&settings.preferences).map_err(|error|error.to_string())?]).map_err(|error|error.to_string())?;
    tx.execute("INSERT INTO workflow_settings(workflow,enabled,model_id,templates_enabled) VALUES('resume',?1,?2,0) ON CONFLICT(workflow) DO UPDATE SET enabled=excluded.enabled,model_id=excluded.model_id,templates_enabled=0",rusqlite::params![settings.enabled,settings.preferences.model_id]).map_err(|error|error.to_string())?;
    tx.commit().map_err(|error| error.to_string())
}

pub struct ResumeSelection {
    pub model: Arc<dyn ModelProvider>,
    pub identity: ResumeModelIdentity,
}
pub fn select_model(
    engine: &Engine,
    settings: &ResumeSettings,
) -> Result<Option<ResumeSelection>, String> {
    select_model_with_policy(engine, settings, sthira_core::llm::lifecycle::RequestPolicy::standard())
}

pub fn select_model_with_policy(
    engine: &Engine,
    settings: &ResumeSettings,
    policy: sthira_core::llm::lifecycle::RequestPolicy,
) -> Result<Option<ResumeSelection>, String> {
    let preferences = &settings.preferences;
    if !settings.enabled || preferences.model_mode == "factual" {
        return Ok(None);
    }
    if preferences.model_mode == "local" {
        return crate::resume_pack::selected_model(engine).map(Some);
    }
    let reader = engine.db().reader().map_err(|error| error.to_string())?;
    let selected = if let Some(id) = &preferences.model_id {
        Some(id.clone())
    } else {
        reader.query_row("SELECT m.id FROM provider_models m JOIN providers p ON p.id=m.provider_id WHERE m.is_default=1 AND m.is_enabled=1 AND p.is_enabled=1 LIMIT 1",[],|row|row.get::<_,String>(0)).optional().map_err(|error|error.to_string())?
    };
    if let Some(id) = selected {
        let (entry, provider, _) = providers_repo::get_model_entry(&reader, &id)
            .map_err(|error| error.to_string())?
            .ok_or("The selected Resume model is no longer configured.")?;
        if !entry.is_enabled || !provider.is_enabled {
            return Err("The selected Resume model is disabled. Showing factual Resume.".into());
        }
        let resolved = sthira_core::llm::dispatcher::resolve_model_provider_with_policy(&reader, Some(&id), engine.model(), policy)
            .map_err(|error| error.to_string())?;
        let basis = serde_json::to_vec(&(
            &provider.id,
            &provider.display_name,
            provider.provider_type,
            provider.base_url,
            &entry.model_identifier,
            &entry.display_name,
        ))
        .map_err(|error| error.to_string())?;
        return Ok(Some(ResumeSelection {
            model: resolved.provider,
            identity: ResumeModelIdentity {
                id,
                name: entry.display_name,
                provider_name: provider.display_name,
                cache_key: format!("{:x}", Sha256::digest(basis)),
            },
        }));
    }
    if preferences.local_fallback && crate::resume_pack::status(engine)?.ready {
        return crate::resume_pack::selected_model(engine).map(Some);
    }
    Ok(None)
}
use rusqlite::OptionalExtension;

pub fn read_brief(engine: &Engine, project_id: &str, level: u32) -> Result<ResumeBrief, String> {
    let settings = get_settings(engine)?;
    let selection = select_model(engine, &settings);
    let key = selection
        .as_ref()
        .ok()
        .and_then(|model| model.as_ref())
        .map(|model| model.identity.cache_key.as_str());
    let mut brief = engine
        .read_resume(
            project_id,
            &settings.preferences,
            key,
            level.clamp(1, 2) as i64,
        )
        .map_err(|error| error.to_string())?;
    if let Err(error) = selection {
        if let Some(meta) = &mut brief.generation {
            meta.notice = Some(error);
        }
    }
    Ok(brief)
}
pub async fn generate_brief(engine: &Engine, project_id: &str) -> Result<ResumeBrief, String> {
    let settings = get_settings(engine)?;
    let selection = select_model(engine, &settings);
    let mut factual = engine
        .prepare_resume(project_id, &settings.preferences)
        .map_err(|error| error.to_string())?
        .factual;
    let model = match selection {
        Ok(Some(model)) => model,
        Ok(None) => {
            if let Some(meta) = &mut factual.generation {
                meta.notice = Some(
                    if !settings.enabled {
                        "Model summaries are off. Showing saved facts."
                    } else if settings.preferences.model_mode == "factual" {
                        "Factual Resume is selected."
                    } else {
                        "No Resume model is configured for this request. Showing saved facts."
                    }
                    .into(),
                );
            }
            return Ok(factual);
        }
        Err(error) => {
            if let Some(meta) = &mut factual.generation {
                meta.notice = Some(error);
            }
            return Ok(factual);
        }
    };
    let cache_guard = || {
        get_settings(engine).ok().is_some_and(|current| {
            current.enabled
                && current.preferences == settings.preferences
                && select_model(engine, &current)
                    .ok()
                    .flatten()
                    .is_some_and(|value| value.identity.cache_key == model.identity.cache_key)
        })
    };
    let result = engine
        .synthesize_resume(
            project_id,
            &settings.preferences,
            model.model.as_ref(),
            &model.identity,
            &cache_guard,
        )
        .await;
    // Do not deliver an old preference/model result after a setting changes mid-flight.
    let current = get_settings(engine)?;
    let same_model = select_model(engine, &current)
        .ok()
        .flatten()
        .is_some_and(|value| value.identity.cache_key == model.identity.cache_key);
    if current.preferences != settings.preferences || !current.enabled || !same_model {
        return read_brief(engine, project_id, 1);
    }
    match result {
        Ok(brief) => Ok(brief),
        Err(_) => {
            let mut brief = read_brief(engine, project_id, 1)?;
            if let Some(meta) = &mut brief.generation {
                meta.notice=Some("The model summary could not be completed or verified. Your saved work is unchanged; showing the available Resume. Try refreshing again.".into());
            }
            Ok(brief)
        }
    }
}
#[tauri::command]
pub fn cmd_get_resume_settings(state: State<'_, AppState>) -> Result<ResumeSettings, String> {
    get_settings(&state.engine)
}
#[tauri::command]
pub fn cmd_save_resume_settings(
    settings: ResumeSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    save_settings(&state.engine, settings)
}
#[tauri::command]
pub async fn cmd_generate_resume_brief(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<ResumeBrief, String> {
    generate_brief(&state.engine, &project_id).await
}

#[derive(Clone, Debug, Serialize)]
pub struct ResumeModelView {
    pub id: Option<String>,
    pub name: Option<String>,
    pub provider_name: Option<String>,
    pub available: bool,
    pub notice: Option<String>,
}
pub fn model_view(engine: &Engine, for_naming: bool) -> Result<ResumeModelView, String> {
    let mut settings = get_settings(engine)?;
    if for_naming {
        settings.enabled = true;
    }
    Ok(match select_model(engine, &settings) {
        Ok(Some(selected)) => ResumeModelView {
            id: Some(selected.identity.id),
            name: Some(selected.identity.name),
            provider_name: Some(selected.identity.provider_name),
            available: true,
            notice: None,
        },
        Ok(None) => ResumeModelView {
            id: None,
            name: None,
            provider_name: None,
            available: false,
            notice: Some(
                if !settings.enabled {
                    "Model summaries are off."
                } else {
                    "No model is selected. Factual Resume remains available."
                }
                .into(),
            ),
        },
        Err(error) => ResumeModelView {
            id: settings.preferences.model_id,
            name: None,
            provider_name: None,
            available: false,
            notice: Some(error),
        },
    })
}
#[tauri::command]
pub fn cmd_get_resume_model(
    for_naming: bool,
    state: State<'_, AppState>,
) -> Result<ResumeModelView, String> {
    model_view(&state.engine, for_naming)
}
