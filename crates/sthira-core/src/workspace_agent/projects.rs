//! Reviewed project creation, direction changes and atomic page moves.
use super::*;
use crate::{
    project_actions as projects, workspace_structure as structure, Actor, EventType, Materiality,
};
pub(super) fn is_proposal(action: &AgentAction) -> bool {
    matches!(
        action,
        AgentAction::ProposeProjectCreate { .. }
            | AgentAction::ProposeProjectPurpose { .. }
            | AgentAction::ProposeMove { .. }
    )
}
fn permitted(run: &AgentRun, whole_workspace: bool) -> Result<(), CoreError> {
    if run.document_id.is_some()
        || !run.allow_edits
        || (whole_workspace && run.project_id.is_some())
    {
        return Err(error("This project operation requires an eligible workspace Agent goal. Choose Entire workspace for project creation or moves."));
    }
    Ok(())
}
fn live_members(conn: &Connection, page: &str) -> Result<Vec<String>, CoreError> {
    let mut stmt=conn.prepare("SELECT p.id FROM projects p JOIN project_artifacts pa ON p.id=pa.project_id WHERE pa.artifact_id=?1 AND pa.detached_at IS NULL AND p.deleted_at IS NULL ORDER BY p.id")?;
    let rows = stmt
        .query_map([page], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
pub(super) fn record(
    conn: &Connection,
    run: &AgentRun,
    action: &AgentAction,
    pages: &HashMap<String, crate::Artifact>,
    context: &organization::Context,
) -> Result<Value, CoreError> {
    match action {
        AgentAction::ProposeProjectCreate {
            name,
            purpose,
            reason,
        } => {
            permitted(run, true)?;
            check_text(name, 160)?;
            if name.chars().any(char::is_control) {
                return Err(error("Project names must fit on one line."));
            }
            check_text(reason, 2000)?;
            if purpose.chars().count() > 4000 {
                return Err(error("Project purposes are limited to 4,000 characters."));
            }
            let same_name:i64=conn.query_row("SELECT COUNT(*) FROM projects WHERE deleted_at IS NULL AND lower(trim(name))=lower(trim(?1))",[name],|r|r.get(0))?;
            Ok(
                json!({"proposal":"New project awaiting review; nothing has been created.","same_name_projects":same_name}),
            )
        }
        AgentAction::ProposeProjectPurpose {
            project_id,
            purpose,
            reason,
        } => {
            permitted(run, false)?;
            check_text(purpose, 4000)?;
            check_text(reason, 2000)?;
            organization::require_project(conn, run, project_id, context)?;
            let basis = projects::snapshot(conn, project_id)?;
            if basis
                .purpose
                .as_ref()
                .is_some_and(|p| p.purpose == purpose.trim())
            {
                return Err(error("This project already has that purpose."));
            }
            Ok(
                json!({"project_basis":basis,"proposal":"Project direction awaiting review; it will remain AI-inferred."}),
            )
        }
        AgentAction::ProposeMove {
            artifact_id,
            from_project_id,
            to_project_id,
            reason,
        } => {
            permitted(run, true)?;
            check_text(reason, 2000)?;
            if from_project_id.as_deref() == Some(to_project_id.as_str()) {
                return Err(error("Choose distinct source and destination projects."));
            }
            let page = organization::checked_page(conn, run, artifact_id, pages)?;
            organization::require_project(conn, run, to_project_id, context)?;
            let destination = projects::snapshot(conn, to_project_id)?;
            let members = live_members(conn, artifact_id)?;
            let source = if let Some(id) = from_project_id {
                organization::require_project(conn, run, id, context)?;
                if !members.contains(id) {
                    return Err(error(
                        "The page is not attached to the requested source project.",
                    ));
                }
                Some(projects::snapshot(conn, id)?)
            } else {
                if !members.is_empty() {
                    return Err(error(
                        "This page is not standalone. Specify the source project to move from.",
                    ));
                }
                None
            };
            Ok(
                json!({"pages":[page],"source_project":source,"destination_project":destination,"membership_revisions":projects::memberships(conn,artifact_id)?,"proposal":"Atomic page move awaiting review; other memberships and page content remain intact."}),
            )
        }
        _ => Err(error("Unsupported project operation.")),
    }
}
pub(super) fn apply(engine: &Engine, run_id: &str, step_id: &str) -> Result<(), CoreError> {
    let now = engine.clock().now_ms();
    let mut writer = engine.db().writer();
    let tx = writer.transaction()?;
    super::external::check_run(&tx, run_id)?;
    let run = engine.agent_run_from_conn(&tx, run_id)?;
    let step = run
        .steps
        .iter()
        .find(|s| s.id == step_id)
        .ok_or_else(|| error("Project proposal unavailable."))?;
    if step.state != "pending" || !matches!(run.status.as_str(), "review" | "completed") {
        return Err(error("This proposal is no longer eligible for acceptance."));
    }
    let cs = uuid::Uuid::new_v4().to_string();
    tx.execute("INSERT INTO change_sets(id,actor,summary,inverse_payload,applied_at) VALUES(?1,'ai','Reviewed agent project change','{}',?2)",params![cs,now])?;
    let (inverse, project_id) = match &step.action {
        AgentAction::ProposeProjectCreate { name, purpose, .. } => {
            permitted(&run, true)?;
            let same_name:i64=tx.query_row("SELECT COUNT(*) FROM projects WHERE deleted_at IS NULL AND lower(trim(name))=lower(trim(?1))",[name],|r|r.get(0))?;
            if Some(same_name) != step.result["same_name_projects"].as_i64() {
                return Err(error(
                    "Projects with this name changed. Ask for a fresh creation proposal.",
                ));
            }
            let id = uuid::Uuid::new_v4().to_string();
            projects::create_tx(&tx, &id, name.trim(), None, now)?;
            if !purpose.trim().is_empty() {
                projects::set_purpose_tx(&tx, &id, purpose.trim(), "ai_inferred", Some(&cs), now)?;
            }
            engine.log_event_tx(
                &tx,
                Some(&id),
                None,
                Actor::Ai,
                EventType::ProjectCreated,
                Materiality::Substantive,
                json!({"name":name,"agent_run_id":run.id,"agent_step_id":step.id}),
                Some(&cs),
                now,
            )?;
            let event_seq: i64 = tx.query_row(
                "SELECT COALESCE(MAX(seq),0) FROM session_events WHERE project_id=?1",
                [&id],
                |r| r.get(0),
            )?;
            (
                json!({"type":"remove_unused_agent_project","after":projects::snapshot(&tx,&id)?,"event_seq":event_seq,"agent_step_id":step.id}),
                id,
            )
        }
        AgentAction::ProposeProjectPurpose {
            project_id: id,
            purpose,
            ..
        } => {
            permitted(&run, false)?;
            scope_live(&tx, run.project_id.as_deref())?;
            if run.project_id.as_ref().is_some_and(|p| p != id) {
                return Err(error("Project outside original scope."));
            }
            let basis: projects::ProjectBasis =
                serde_json::from_value(step.result["project_basis"].clone())?;
            projects::check(&tx, &basis)?;
            projects::set_purpose_tx(&tx, id, purpose.trim(), "ai_inferred", Some(&cs), now)?;
            engine.log_event_tx(&tx,Some(id),None,Actor::Ai,EventType::ProjectPurposePivoted,Materiality::Substantive,json!({"purpose":purpose,"origin":"ai_inferred","agent_run_id":run.id,"agent_step_id":step.id}),Some(&cs),now)?;
            (
                json!({"type":"restore_project_purpose","before":basis.purpose,"after":projects::snapshot(&tx,id)?,"agent_step_id":step.id}),
                id.clone(),
            )
        }
        AgentAction::ProposeMove {
            artifact_id,
            from_project_id,
            to_project_id,
            ..
        } => {
            permitted(&run, true)?;
            let page: crate::Artifact = serde_json::from_value(step.result["pages"][0].clone())?;
            let current = Engine::get_artifact_tx(&tx, artifact_id)?;
            if current.deleted_at.is_some()
                || current.version != page.version
                || current.content_hash != page.content_hash
                || current.title != page.title
            {
                return Err(error(
                    "The saved page changed. Ask for a fresh move proposal.",
                ));
            }
            let expected: Vec<(String, i64)> =
                serde_json::from_value(step.result["membership_revisions"].clone())?;
            if projects::memberships(&tx, artifact_id)? != expected {
                return Err(error(
                    "Page memberships changed. Ask for a fresh move proposal.",
                ));
            }
            for value in [
                &step.result["source_project"],
                &step.result["destination_project"],
            ] {
                if !value.is_null() {
                    let basis: projects::ProjectBasis = serde_json::from_value(value.clone())?;
                    projects::check(&tx, &basis)?;
                }
            }
            let mut before = vec![structure::membership(&tx, to_project_id, artifact_id)?];
            if let Some(id) = from_project_id {
                before.push(structure::membership(&tx, id, artifact_id)?);
            }
            projects::move_tx(
                &tx,
                artifact_id,
                from_project_id.as_deref(),
                to_project_id,
                now,
            )?;
            let mut restore = vec![];
            for relation in before {
                let key = relation.key();
                let after_revision = structure::revision(&tx, &key)?;
                let previous = expected
                    .iter()
                    .find(|(k, _)| k == &key)
                    .map(|(_, v)| *v)
                    .unwrap_or(0);
                if previous != after_revision {
                    restore.push(json!({"relation":relation,"expected_revision":after_revision,"agent_step_id":step.id}));
                }
            }
            if let Some(id) = from_project_id {
                engine.log_event_tx(
                    &tx,
                    Some(id),
                    Some(artifact_id),
                    Actor::Ai,
                    EventType::ArtifactDetached,
                    Materiality::Minor,
                    json!({"reason":"reviewed_agent_move","agent_run_id":run.id}),
                    Some(&cs),
                    now,
                )?;
            }
            engine.log_event_tx(
                &tx,
                Some(to_project_id),
                Some(artifact_id),
                Actor::Ai,
                EventType::ArtifactAttached,
                Materiality::Substantive,
                json!({"reason":"reviewed_agent_move","agent_run_id":run.id}),
                Some(&cs),
                now,
            )?;
            (
                json!({"type":"restore_agent_move","artifact_id":artifact_id,"memberships_after":projects::memberships(&tx,artifact_id)?,"relations":restore,"agent_step_id":step.id}),
                to_project_id.clone(),
            )
        }
        _ => return Err(error("Unsupported project proposal.")),
    };
    tx.execute(
        "UPDATE change_sets SET inverse_payload=?2 WHERE id=?1",
        params![cs, json!({"operations":[inverse]}).to_string()],
    )?;
    tx.execute("UPDATE agent_steps SET state='applied',result_json=json_set(result_json,'$.change_set_id',?2,'$.project_id',?3) WHERE id=?1 AND state='pending'",params![step_id,cs,project_id])?;
    tx.execute("UPDATE agent_runs SET revision=revision+1,updated_at=?2,status=CASE WHEN EXISTS(SELECT 1 FROM agent_steps s LEFT JOIN document_conversation_turns t ON t.id=s.turn_id WHERE s.run_id=agent_runs.id AND COALESCE(t.proposal_state,s.state)='pending') THEN 'review' ELSE 'completed' END WHERE id=?1",params![run_id,now])?;
    tx.commit()?;
    drop(writer);
    emit(engine, &step.action);
    engine.emit_event(CoreEvent::ChangeSetApplied { id: cs });
    Ok(())
}
pub(super) fn emit(engine: &Engine, action: &AgentAction) {
    if let AgentAction::ProposeMove { artifact_id, .. } = action {
        engine.emit_event(CoreEvent::ArtifactChanged {
            id: artifact_id.clone(),
        });
    }
}
