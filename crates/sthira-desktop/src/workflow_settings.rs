use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use sthira_core::Engine;
use tauri::State;

pub const WORKFLOWS: [&str; 6] = ["research", "plan", "draft", "chat", "resume", "agent"];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkflowSetting {
    pub enabled: bool,
    pub model_id: Option<String>,
    pub templates_enabled: bool,
}

pub fn get_workflow_settings(engine: &Engine) -> Result<BTreeMap<String, WorkflowSetting>, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    let mut settings: BTreeMap<String, WorkflowSetting> = WORKFLOWS
        .iter()
        .map(|key| (key.to_string(), WorkflowSetting::default()))
        .collect();
    let mut query = reader
        .prepare("SELECT workflow, enabled, model_id, templates_enabled FROM workflow_settings")
        .map_err(|e| e.to_string())?;
    let rows = query
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                WorkflowSetting {
                    enabled: row.get(1)?,
                    model_id: row.get(2)?,
                    templates_enabled: row.get(3)?,
                },
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (key, value) = row.map_err(|e| e.to_string())?;
        if settings.contains_key(&key) {
            settings.insert(key, value);
        }
    }
    Ok(settings)
}

pub fn save_workflow_setting(
    engine: &Engine,
    workflow: &str,
    setting: WorkflowSetting,
) -> Result<(), String> {
    if !WORKFLOWS.contains(&workflow) {
        return Err("Unknown workflow.".into());
    }
    if setting.templates_enabled && !matches!(workflow, "research" | "plan") {
        return Err("Templates are available for Research and Plan.".into());
    }
    if setting.enabled {
        if let Some(id) = setting.model_id.as_deref() {
            let reader = engine.db().reader().map_err(|e| e.to_string())?;
            let entry = sthira_core::db::providers_repo::get_model_entry(&reader, id)
                .map_err(|e| e.to_string())?;
            if !entry.is_some_and(|(model, provider, _)| model.is_enabled && provider.is_enabled) {
                return Err("Choose an available, enabled model.".into());
            }
        }
    }
    engine.db().writer().execute(
        "INSERT INTO workflow_settings(workflow,enabled,model_id,templates_enabled) VALUES(?1,?2,?3,?4)
         ON CONFLICT(workflow) DO UPDATE SET enabled=excluded.enabled,model_id=excluded.model_id,templates_enabled=excluded.templates_enabled",
        rusqlite::params![workflow, setting.enabled, setting.model_id, setting.templates_enabled],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Check again at native dispatch, before any network request. Reading old content
/// and reviewing already-created proposals never requires enabling generation.
pub fn workflow_model(
    engine: &Engine,
    workflow: &str,
    explicit: Option<String>,
) -> Result<Option<String>, String> {
    let settings = get_workflow_settings(engine)?;
    let setting = settings.get(workflow).ok_or("Unknown workflow.")?;
    if !setting.enabled {
        return Err(format!(
            "Enable {workflow} in Settings → Workflows to continue."
        ));
    }
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    let selected = explicit.or_else(|| setting.model_id.clone());
    let selected = match selected {
        Some(id) => id,
        None => reader.query_row("SELECT m.id FROM provider_models m JOIN providers p ON p.id=m.provider_id WHERE m.is_default=1 AND m.is_enabled=1 AND p.is_enabled=1 LIMIT 1", [], |row| row.get::<_, String>(0))
            .map_err(|_| "Choose a workspace default model in Settings → AI & Models.".to_string())?,
    };
    let entry = sthira_core::db::providers_repo::get_model_entry(&reader, &selected)
        .map_err(|e| e.to_string())?;
    if !entry.is_some_and(|(model, provider, _)| model.is_enabled && provider.is_enabled) {
        return Err(
            "The selected model is unavailable. Choose an enabled model in Settings.".into(),
        );
    }
    Ok(Some(selected))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowTemplateChoice {
    /// inherit | explicit | none | disabled
    pub mode: String,
    pub template: Option<sthira_core::SynthesisTemplate>,
}

/// Resolve the owner's template decision exactly once at dispatch. The returned
/// template object is the immutable record for this generation; later settings or
/// template edits must not be consulted when saving it.
pub fn resolve_workflow_template(
    engine: &Engine,
    workflow: &str,
    explicit: Option<String>,
) -> Result<WorkflowTemplateChoice, String> {
    let settings = get_workflow_settings(engine)?;
    let setting = settings.get(workflow).ok_or("Unknown workflow.")?;
    if !setting.enabled || !setting.templates_enabled {
        return Ok(WorkflowTemplateChoice {
            mode: "disabled".into(),
            template: None,
        });
    }
    if let Some(id) = explicit.as_deref() {
        if id.trim() == "__none__" {
            return Ok(WorkflowTemplateChoice {
                mode: "none".into(),
                template: None,
            });
        }
        let template = engine
            .get_synthesis_template(id)
            .map_err(|e| e.to_string())?
            .ok_or("Template unavailable.")?;
        if template.archived_at.is_some() {
            return Err(
                "This template was archived. Choose another template or no template.".into(),
            );
        }
        if template.target_mode != workflow {
            return Err("This template belongs to another workflow.".into());
        }
        return Ok(WorkflowTemplateChoice {
            mode: "explicit".into(),
            template: Some(template),
        });
    }
    Ok(WorkflowTemplateChoice {
        mode: "inherit".into(),
        template: engine
            .get_default_synthesis_template(workflow)
            .map_err(|e| e.to_string())?,
    })
}

pub fn workflow_template(
    engine: &Engine,
    workflow: &str,
    explicit: Option<String>,
) -> Result<Option<String>, String> {
    Ok(resolve_workflow_template(engine, workflow, explicit)?
        .template
        .map(|template| template.id))
}

pub fn workflow_prompt_with_template(
    raw: String,
    template: Option<&sthira_core::SynthesisTemplate>,
) -> String {
    match template {
        Some(template) => {
            let guidance = if template.guidance.trim().is_empty() {
                String::new()
            } else {
                format!(
                    "\nTemplate guidance (subordinate to evidence and review rules): {}",
                    template.guidance.trim()
                )
            };
            format!("{raw}\n\nUse the following output structure, filling it with findings for this request. Template examples are not evidence:{guidance}\n{}", template.template_skeleton_md)
        }
        None => raw,
    }
}

pub fn workflow_prompt(
    engine: &Engine,
    workflow: &str,
    raw: String,
    template: Option<String>,
) -> Result<String, String> {
    let choice = resolve_workflow_template(engine, workflow, template)?;
    Ok(workflow_prompt_with_template(raw, choice.template.as_ref()))
}

#[tauri::command]
pub fn cmd_get_workflow_settings(
    state: State<'_, AppState>,
) -> Result<BTreeMap<String, WorkflowSetting>, String> {
    get_workflow_settings(&state.engine)
}
#[tauri::command]
pub fn cmd_save_workflow_setting(
    state: State<'_, AppState>,
    workflow: String,
    setting: WorkflowSetting,
) -> Result<(), String> {
    save_workflow_setting(&state.engine, &workflow, setting)
}
