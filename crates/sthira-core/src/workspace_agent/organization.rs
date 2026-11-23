//! AE2 organization tools. Persist proposals, never execute model-requested writes.
use super::*;
use crate::workspace_structure::{self as structure, EntityRef, Relation};
use crate::{Actor, EventType, Materiality};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(super) struct ProjectBasis {
    id: String,
    name: String,
    status: String,
    purpose: Option<String>,
}
#[derive(Default)]
pub(super) struct Context {
    projects: HashMap<String, ProjectBasis>,
    project_versions: HashMap<String, i64>,
}
fn project(conn: &Connection, id: &str) -> Result<ProjectBasis, CoreError> {
    conn.query_row("SELECT p.id,p.name,p.status,(SELECT purpose FROM project_purposes WHERE project_id=p.id AND superseded_at IS NULL ORDER BY set_at DESC,id DESC LIMIT 1) FROM projects p WHERE p.id=?1 AND p.deleted_at IS NULL",[id],|r|Ok(ProjectBasis{id:r.get(0)?,name:r.get(1)?,status:r.get(2)?,purpose:r.get(3)?})).optional()?.ok_or_else(||error("The project is no longer available."))
}
pub(super) fn is_proposal(action: &AgentAction) -> bool {
    matches!(
        action,
        AgentAction::ProposeMembership { .. } | AgentAction::ProposeConnection { .. }
    )
}
fn workspace_request(run: &AgentRun) -> Result<(), CoreError> {
    if run.document_id.is_some() {
        return Err(error("Organization tools require a workspace Agent goal. Document chat cannot expand its scope."));
    }
    Ok(())
}
fn project_allowed(run: &AgentRun, id: &str) -> Result<(), CoreError> {
    if run.project_id.as_deref().is_some_and(|p| p != id) {
        return Err(error("Project outside the selected scope."));
    }
    Ok(())
}
pub(super) fn checked_page(
    conn: &Connection,
    run: &AgentRun,
    id: &str,
    read_pages: &HashMap<String, crate::Artifact>,
) -> Result<crate::Artifact, CoreError> {
    let basis = read_pages.get(id).ok_or_else(|| {
        error("Read the complete saved page before proposing an organization change.")
    })?;
    if !page_allowed(conn, run.project_id.as_deref(), id, None)? {
        return Err(error("Page outside the selected scope."));
    }
    let current = Engine::get_artifact_tx(conn, id)?;
    if current.version != basis.version
        || current.content_hash != basis.content_hash
        || current.title != basis.title
    {
        return Err(error("The saved page changed. Read it again."));
    }
    Ok(basis.clone())
}
pub(super) fn require_project(
    conn: &Connection,
    run: &AgentRun,
    id: &str,
    context: &Context,
) -> Result<(), CoreError> {
    workspace_request(run)?;
    project_allowed(run, id)?;
    let basis = context
        .projects
        .get(id)
        .ok_or_else(|| error("Read the project before proposing a project change."))?;
    if &project(conn, id)? != basis
        || context.project_versions.get(id).copied()
            != Some(crate::project_actions::revision(conn, id)?)
    {
        return Err(error("The project changed. Read it again."));
    }
    if basis
        .purpose
        .as_ref()
        .is_some_and(|text| text.chars().count() > 4000)
    {
        return Err(error("This project's purpose exceeds the complete-read limit. Use the project editor instead."));
    }
    Ok(())
}
pub(super) fn record(
    conn: &Connection,
    run: &AgentRun,
    action: &AgentAction,
    pages: &HashMap<String, crate::Artifact>,
    context: &mut Context,
) -> Result<Value, CoreError> {
    workspace_request(run)?;
    match action {
        AgentAction::SearchProjects { query } => {
            if query.chars().count() > 300 {
                return Err(error("Project searches are limited to 300 characters."));
            }
            let mut stmt=conn.prepare("SELECT id,name,status FROM projects WHERE deleted_at IS NULL AND (?1 IS NULL OR id=?1) AND instr(lower(name),lower(?2))>0 ORDER BY last_substantive_at DESC,id LIMIT 20")?;
            let items=stmt.query_map(params![run.project_id,query],|r|Ok(json!({"project_id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"status":r.get::<_,String>(2)?})))?.collect::<Result<Vec<_>,_>>()?;
            Ok(json!({"projects":items,"limit":20}))
        }
        AgentAction::ReadProject { project_id, offset } => {
            project_allowed(run, project_id)?;
            if *offset > 10000 {
                return Err(error("Project pagination limit reached. Narrow the goal."));
            }
            let basis = project(conn, project_id)?;
            let mut stmt=conn.prepare("SELECT a.id,COALESCE(a.title,'Untitled page'),a.type FROM project_artifacts pa JOIN artifacts a ON a.id=pa.artifact_id WHERE pa.project_id=?1 AND pa.detached_at IS NULL AND a.deleted_at IS NULL ORDER BY a.id LIMIT 21 OFFSET ?2")?;
            let mut members=stmt.query_map(params![project_id,*offset as i64],|r|Ok(json!({"artifact_id":r.get::<_,String>(0)?,"title":r.get::<_,String>(1)?,"type":r.get::<_,String>(2)?})))?.collect::<Result<Vec<_>,_>>()?;
            let more = members.len() > 20;
            members.truncate(20);
            let purpose = basis.purpose.as_deref().unwrap_or("");
            let result = json!({"project_id":basis.id,"name":basis.name,"status":basis.status,"purpose":purpose.chars().take(4000).collect::<String>(),"purpose_truncated":purpose.chars().count()>4000,"purpose_origin":crate::project_actions::purpose(conn,project_id)?.map(|p|p.origin),"members":members,"next_offset":if more{Some(offset+20)}else{None},"membership_is_not_a_reference":true});
            context.project_versions.insert(
                project_id.clone(),
                crate::project_actions::revision(conn, project_id)?,
            );
            context.projects.insert(project_id.clone(), basis);
            Ok(result)
        }
        AgentAction::ReadConnections {
            artifact_id,
            offset,
        } => {
            if *offset > 10000 || !page_allowed(conn, run.project_id.as_deref(), artifact_id, None)?
            {
                return Err(error(
                    "Page outside scope or connection pagination limit reached.",
                ));
            }
            // Only explicit page-to-page references visible to BOTH endpoint grants.
            let mut stmt=conn.prepare("SELECT w.id,w.source_artifact_id,w.target_artifact_id,COALESCE(s.title,'Untitled page'),COALESCE(t.title,'Untitled page') FROM workspace_links w JOIN artifacts s ON s.id=w.source_artifact_id JOIN artifacts t ON t.id=w.target_artifact_id WHERE (s.id=?1 OR t.id=?1) AND s.deleted_at IS NULL AND t.deleted_at IS NULL AND (?2 IS NULL OR (EXISTS(SELECT 1 FROM project_artifacts pa WHERE pa.project_id=?2 AND pa.artifact_id=s.id AND pa.detached_at IS NULL) AND EXISTS(SELECT 1 FROM project_artifacts pa WHERE pa.project_id=?2 AND pa.artifact_id=t.id AND pa.detached_at IS NULL))) ORDER BY w.id LIMIT 21 OFFSET ?3")?;
            let mut items=stmt.query_map(params![artifact_id,run.project_id,*offset as i64],|r|Ok(json!({"reference_id":r.get::<_,String>(0)?,"source_id":r.get::<_,String>(1)?,"target_id":r.get::<_,String>(2)?,"source_title":r.get::<_,String>(3)?,"target_title":r.get::<_,String>(4)?,"relationship":"explicit_reference"})))?.collect::<Result<Vec<_>,_>>()?;
            let more = items.len() > 20;
            items.truncate(20);
            Ok(
                json!({"references":items,"next_offset":if more{Some(offset+20)}else{None},"includes_lineage":false,"includes_project_references":false}),
            )
        }
        AgentAction::ProposeMembership {
            artifact_id,
            project_id,
            attached,
            reason,
        } => {
            if !run.allow_edits {
                return Err(error("Organization changes were not enabled."));
            }
            project_allowed(run, project_id)?;
            check_text(reason, 2000)?;
            let page = checked_page(conn, run, artifact_id, pages)?;
            let basis = context
                .projects
                .get(project_id)
                .ok_or_else(|| error("Read the project before proposing membership changes."))?;
            if &project(conn, project_id)? != basis {
                return Err(error("The project changed. Read it again."));
            }
            let relation = structure::membership(conn, project_id, artifact_id)?;
            if relation.present() == *attached {
                return Err(error(
                    "This page already has the requested membership state.",
                ));
            }
            Ok(
                json!({"pages":[page],"project":basis,"relation_revision":structure::revision(conn,&relation.key())?,"relation":relation,"proposal":"Project membership awaiting review; page content and other memberships stay intact."}),
            )
        }
        AgentAction::ProposeConnection {
            source_id,
            target_id,
            connected,
            reason,
        } => {
            if !run.allow_edits || source_id == target_id {
                return Err(error(
                    "Reference changes are not enabled or the endpoints are identical.",
                ));
            }
            check_text(reason, 2000)?;
            let source = checked_page(conn, run, source_id, pages)?;
            let target = checked_page(conn, run, target_id, pages)?;
            let relation = structure::reference(
                conn,
                &EntityRef::page(source_id),
                &EntityRef::page(target_id),
            )?;
            if relation.present() == *connected {
                return Err(error("This explicit reference already has the requested state. Derivation lineage is separate."));
            }
            Ok(
                json!({"pages":[source,target],"relation_revision":structure::revision(conn,&relation.key())?,"relation":relation,"proposal":"Directional page reference awaiting owner review; lineage and membership stay intact."}),
            )
        }
        _ => Err(error("Unsupported organization action.")),
    }
}
fn validate(conn: &Connection, run: &AgentRun, result: &Value) -> Result<Relation, CoreError> {
    workspace_request(run)?;
    if !run.allow_edits {
        return Err(error("Organization changes were not enabled."));
    }
    scope_live(conn, run.project_id.as_deref())?;
    let pages: Vec<crate::Artifact> = serde_json::from_value(result["pages"].clone())?;
    for basis in pages {
        if !page_allowed(conn, run.project_id.as_deref(), &basis.id, None)? {
            return Err(error("A page is unavailable or left the original scope."));
        }
        let current = Engine::get_artifact_tx(conn, &basis.id)?;
        if current.version != basis.version
            || current.content_hash != basis.content_hash
            || current.title != basis.title
        {
            return Err(error(
                "A saved page changed. Ask for a fresh organization proposal.",
            ));
        }
    }
    if let Some(basis) = result.get("project") {
        let basis: ProjectBasis = serde_json::from_value(basis.clone())?;
        project_allowed(run, &basis.id)?;
        if project(conn, &basis.id)? != basis {
            return Err(error("The project changed. Ask for a fresh proposal."));
        }
    }
    let relation: Relation = serde_json::from_value(result["relation"].clone())?;
    relation.validate(conn)?;
    if structure::revision(conn, &relation.key())?
        != result["relation_revision"]
            .as_i64()
            .ok_or_else(|| error("Missing relationship revision."))?
    {
        return Err(error(
            "This relationship changed. Ask for a fresh proposal.",
        ));
    }
    Ok(relation)
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
        .ok_or_else(|| error("Proposal unavailable."))?;
    if step.state != "pending" || !matches!(run.status.as_str(), "review" | "completed") {
        return Err(error("This proposal is no longer eligible for acceptance."));
    }
    let relation = validate(&tx, &run, &step.result)?;
    match (&step.action, &relation) {
        (
            AgentAction::ProposeMembership {
                artifact_id,
                project_id,
                attached,
                ..
            },
            Relation::Membership { .. },
        ) => {
            if *attached {
                structure::attach_tx(&tx, project_id, artifact_id, "user_explicit", None, now)?;
            } else {
                structure::detach_tx(&tx, project_id, artifact_id, now)?;
            }
        }
        (
            AgentAction::ProposeConnection {
                source_id,
                target_id,
                connected,
                ..
            },
            Relation::Reference { link, .. },
        ) => {
            if *connected {
                structure::create_reference_tx(
                    &tx,
                    &EntityRef::page(source_id),
                    &EntityRef::page(target_id),
                    now,
                )?;
            } else {
                structure::delete_reference_tx(
                    &tx,
                    &link
                        .as_ref()
                        .ok_or_else(|| error("Reference unavailable."))?
                        .id,
                )?;
            }
        }
        _ => return Err(error("Invalid organization proposal receipt.")),
    }
    let cs = uuid::Uuid::new_v4().to_string();
    let inverse = json!({"operations":[{"type":"restore_workspace_relation","relation":relation,"expected_revision":structure::revision(&tx,&relation.key())?,"agent_step_id":step.id}]});
    tx.execute("INSERT INTO change_sets(id,actor,summary,inverse_payload,applied_at) VALUES(?1,'ai','Reviewed agent organization change',?2,?3)",params![cs,inverse.to_string(),now])?;
    tx.execute("UPDATE agent_steps SET state='applied',result_json=json_set(result_json,'$.change_set_id',?2) WHERE id=?1 AND state='pending'",params![step_id,cs])?;
    tx.execute("UPDATE agent_runs SET revision=revision+1,updated_at=?2,status=CASE WHEN EXISTS(SELECT 1 FROM agent_steps s LEFT JOIN document_conversation_turns t ON t.id=s.turn_id WHERE s.run_id=agent_runs.id AND COALESCE(t.proposal_state,s.state)='pending') THEN 'review' ELSE 'completed' END WHERE id=?1",params![run_id,now])?;
    let page = match &step.action {
        AgentAction::ProposeMembership { artifact_id, .. } => artifact_id,
        AgentAction::ProposeConnection { source_id, .. } => source_id,
        _ => unreachable!(),
    };
    engine.log_event_tx(
        &tx,
        match &step.action {
            AgentAction::ProposeMembership { project_id, .. } => Some(project_id.as_str()),
            _ => run.project_id.as_deref(),
        },
        Some(page),
        Actor::Ai,
        EventType::ChangeSetApplied,
        Materiality::Substantive,
        json!({"agent_run_id":run.id,"agent_step_id":step.id,"action":step.action}),
        Some(&cs),
        now,
    )?;
    tx.commit()?;
    drop(writer);
    emit(engine, &step.action);
    engine.emit_event(CoreEvent::ChangeSetApplied { id: cs });
    Ok(())
}
pub(super) fn emit(engine: &Engine, action: &AgentAction) {
    match action {
        AgentAction::ProposeMembership { artifact_id, .. } => {
            engine.emit_event(CoreEvent::ArtifactChanged {
                id: artifact_id.clone(),
            })
        }
        AgentAction::ProposeConnection {
            source_id,
            target_id,
            ..
        } => {
            engine.emit_event(CoreEvent::ArtifactChanged {
                id: source_id.clone(),
            });
            engine.emit_event(CoreEvent::ArtifactChanged {
                id: target_id.clone(),
            });
        }
        _ => {}
    }
}
