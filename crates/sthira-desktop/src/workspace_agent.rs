use crate::AppState;
use sthira_core::workspace_agent::{AgentGoal, AgentRun, AgentTask};
use tauri::State;

/// Native tasks own their engine/token, independently of the invoking view.
/// This returns after queueing; execution never depends on a mounted chat panel.
pub fn start_agent_task(state: AppState, id: String) -> Result<AgentRun, String> {
    let run = state.engine.agent_run(&id).map_err(|e| e.to_string())?;
    if run.status != "queued" {
        return Err("This agent task has already started or stopped.".into());
    }
    let request = format!("workspace-agent:{id}");
    let token = state
        .requests
        .register_unique(&request)
        .ok_or("This agent task is already executing.")?;
    tauri::async_runtime::spawn(async move {
        if let Err(failure) = state.engine.execute_agent_run(&id, &token).await {
            let _ = state.engine.fail_agent_run(&id, &failure.to_string());
        }
        state.requests.unregister(&request);
    });
    Ok(run)
}
#[tauri::command]
pub fn cmd_create_agent_run(
    state: State<'_, AppState>,
    mut goal: AgentGoal,
) -> Result<AgentRun, String> {
    goal.model_id =
        crate::workflow_settings::workflow_model(&state.engine, "agent", Some(goal.model_id))?
            .ok_or("Choose a model.")?;
    if goal.allow_web {
        let reader = state.engine.db().reader().map_err(|e| e.to_string())?;
        let configured = sthira_core::db::search_repo::get_default_search_provider(&reader)
            .map_err(|e| e.to_string())?;
        if configured.is_none() {
            return Err(
                "Web research is allowed, but no enabled search provider is configured. Configure one in Settings → Web Search or turn web research off."
                    .into(),
            );
        }
    }
    state
        .engine
        .create_agent_run(goal)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_follow_up_agent_run(
    state: State<'_, AppState>,
    parent_id: String,
    text: String,
    model_id: Option<String>,
) -> Result<AgentRun, String> {
    let parent = state
        .engine
        .agent_run(&parent_id)
        .map_err(|e| e.to_string())?;
    let selected = crate::workflow_settings::workflow_model(
        &state.engine,
        if parent.document_id.is_some() {
            "chat"
        } else {
            "agent"
        },
        model_id.or(Some(parent.model_id)),
    )?;
    let run = state
        .engine
        .follow_up_agent_run(&parent_id, &text, selected.as_deref())
        .map_err(|e| e.to_string())?;
    start_agent_task(state.inner().clone(), run.id)
}
#[tauri::command]
pub fn cmd_list_agent_runs(state: State<'_, AppState>) -> Result<Vec<AgentRun>, String> {
    state.engine.list_agent_runs().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_list_agent_tasks(state: State<'_, AppState>) -> Result<Vec<AgentTask>, String> {
    state.engine.list_agent_tasks().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_get_agent_run(state: State<'_, AppState>, id: String) -> Result<AgentRun, String> {
    state.engine.agent_run(&id).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_execute_agent_run(state: State<'_, AppState>, id: String) -> Result<AgentRun, String> {
    start_agent_task(state.inner().clone(), id)
}
#[tauri::command]
pub fn cmd_cancel_agent_run(state: State<'_, AppState>, id: String) -> Result<AgentRun, String> {
    let result = state
        .engine
        .cancel_agent_run(&id)
        .map_err(|e| e.to_string())?;
    state.requests.cancel(&format!("workspace-agent:{id}"));
    Ok(result)
}
#[tauri::command]
pub fn cmd_review_agent_step(
    state: State<'_, AppState>,
    run_id: String,
    step_id: String,
    apply: bool,
    title: Option<String>,
) -> Result<AgentRun, String> {
    state
        .engine
        .review_agent_step_with_title(&run_id, &step_id, apply, title.as_deref())
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_undo_agent_edit(
    state: State<'_, AppState>,
    run_id: String,
    step_id: String,
) -> Result<AgentRun, String> {
    state
        .engine
        .undo_agent_edit(&run_id, &step_id)
        .map_err(|e| e.to_string())
}

use sthira_core::workspace_agent::{
    external::ExternalConnection,
    handoffs::{ContinueHandoff, Handoff, HandoffDraft, HandoffView},
};
#[tauri::command]
pub fn cmd_list_handoff_connections(
    state: State<'_, AppState>,
) -> Result<Vec<ExternalConnection>, String> {
    state
        .engine
        .list_external_connections()
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_list_agent_handoffs(
    state: State<'_, AppState>,
    run_id: Option<String>,
) -> Result<Vec<Handoff>, String> {
    state
        .engine
        .list_agent_handoffs(run_id.as_deref())
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_get_agent_handoff(
    state: State<'_, AppState>,
    id: String,
) -> Result<HandoffView, String> {
    state.engine.agent_handoff(&id).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_save_agent_handoff(
    state: State<'_, AppState>,
    draft: HandoffDraft,
) -> Result<HandoffView, String> {
    state
        .engine
        .save_agent_handoff(draft)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn cmd_close_agent_handoff(
    state: State<'_, AppState>,
    id: String,
    expected_revision: i64,
) -> Result<HandoffView, String> {
    state
        .engine
        .close_agent_handoff(&id, expected_revision)
        .map_err(|e| e.to_string())
}
pub fn handle_continue_agent_handoff(
    state: AppState,
    mut input: ContinueHandoff,
) -> Result<AgentRun, String> {
    input.model_id =
        crate::workflow_settings::workflow_model(&state.engine, "agent", Some(input.model_id))?
            .ok_or("Choose an available model.")?;
    let run = state
        .engine
        .continue_agent_handoff(input)
        .map_err(|e| e.to_string())?;
    if run.status == "queued" {
        start_agent_task(state, run.id)
    } else {
        Ok(run)
    }
}
#[tauri::command]
pub fn cmd_continue_agent_handoff(
    state: State<'_, AppState>,
    input: ContinueHandoff,
) -> Result<AgentRun, String> {
    handle_continue_agent_handoff(state.inner().clone(), input)
}
