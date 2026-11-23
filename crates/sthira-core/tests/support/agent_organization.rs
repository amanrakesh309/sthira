use super::*;
use rusqlite::OptionalExtension;
use sthira_core::workspace_structure::{EntityKind, EntityRef};
fn page(engine: &Engine, title: &str) -> sthira_core::Artifact {
    engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some(title),
            "Original body with uncertainty.",
            None,
        )
        .unwrap()
}
fn reference(engine: &Engine, source: &str, target: &str) -> String {
    engine
        .create_workspace_reference(
            &EntityRef {
                kind: EntityKind::Artifact,
                id: source.into(),
            },
            &EntityRef {
                kind: EntityKind::Artifact,
                id: target.into(),
            },
        )
        .unwrap()
}
fn member(engine: &Engine, project: &str, page: &str) -> bool {
    engine.db().reader().unwrap().query_row("SELECT EXISTS(SELECT 1 FROM project_artifacts WHERE project_id=?1 AND artifact_id=?2 AND detached_at IS NULL)",rusqlite::params![project,page],|r|r.get(0)).unwrap()
}
async fn execute(engine: &Engine, actions: Vec<Value>, project: Option<&str>) -> AgentRun {
    let (url, server) = fixture(actions).await;
    configure(engine, &url);
    let run = engine.create_agent_run(goal(project)).unwrap();
    let run = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    server.await.unwrap();
    assert_eq!(run.status, "review", "{:?}", run.error);
    run
}
fn finish() -> Value {
    json!({"type":"finish","summary":"Review these organization proposals.","handoff":"No relationship is changed until accepted."})
}
#[tokio::test]
async fn membership_and_reference_review_preserve_identity_content_and_other_relationships() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let p = engine.create_project("Destination", None).unwrap();
    let other = engine.create_project("Keep membership", None).unwrap();
    let a = page(&engine, "Source page");
    let b = page(&engine, "Target page");
    engine
        .attach_artifact(&other.id, &a.id, "user_explicit", None)
        .unwrap();
    let run=execute(&engine,vec![json!({"type":"search_projects","query":"Destination"}),json!({"type":"read_project","project_id":p.id}),json!({"type":"read_page","artifact_id":a.id}),json!({"type":"read_page","artifact_id":b.id}),json!({"type":"propose_membership","artifact_id":a.id,"project_id":p.id,"attached":true,"reason":"This page belongs with the project context."}),json!({"type":"propose_connection","source_id":a.id,"target_id":b.id,"connected":true,"reason":"Keep a directional reference to the supporting page."}),finish()],None).await;
    assert!(!member(&engine, &p.id, &a.id));
    assert_eq!(count(&engine, "workspace_links"), 0);
    engine
        .review_agent_step(&run.id, &run.steps[4].id, true)
        .unwrap();
    assert!(member(&engine, &p.id, &a.id));
    assert!(member(&engine, &other.id, &a.id));
    let accepted = engine
        .review_agent_step(&run.id, &run.steps[5].id, true)
        .unwrap();
    assert_eq!(accepted.status, "completed");
    assert_eq!(count(&engine, "workspace_links"), 1);
    assert_eq!(engine.get_artifact(&a.id).unwrap().version, a.version);
    assert_eq!(engine.get_artifact(&a.id).unwrap().content_md, a.content_md);
    assert_eq!(
        engine.get_artifact(&a.id).unwrap().authorship,
        Authorship::User
    );
    assert!(engine
        .review_agent_step(&run.id, &run.steps[5].id, true)
        .is_err());
    engine.undo_agent_edit(&run.id, &run.steps[5].id).unwrap();
    assert_eq!(count(&engine, "workspace_links"), 0);
    engine.db().writer().execute("UPDATE artifacts SET content_md='Newer owner draft',content_hash='new',version=version+1 WHERE id=?1",[&a.id]).unwrap();
    let undone = engine.undo_agent_edit(&run.id, &run.steps[4].id).unwrap();
    assert_eq!(undone.steps[4].state, "reverted");
    assert!(!member(&engine, &p.id, &a.id));
    assert!(member(&engine, &other.id, &a.id));
    assert_eq!(
        engine.get_artifact(&a.id).unwrap().content_md,
        "Newer owner draft"
    );
    assert!(engine.undo_agent_edit(&run.id, &run.steps[4].id).is_err());
    drop(engine);
    let reopened = open(tmp.path());
    assert_eq!(
        reopened.agent_run(&run.id).unwrap().steps[5].state,
        "reverted"
    );
}
#[tokio::test]
async fn removal_restores_exact_reference_and_keeps_reverse_reference_and_lineage() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let p = engine.create_project("Research", None).unwrap();
    let a = page(&engine, "Research source");
    let b = page(&engine, "Derived result");
    engine
        .attach_artifact(&p.id, &a.id, "user_explicit", None)
        .unwrap();
    engine
        .attach_artifact(&p.id, &b.id, "user_explicit", None)
        .unwrap();
    engine.db().writer().execute("UPDATE artifacts SET derived_from_artifact_id=?1,derivation_type='plan_from_research' WHERE id=?2",rusqlite::params![a.id,b.id]).unwrap();
    let forward = reference(&engine, &a.id, &b.id);
    let reverse = reference(&engine, &b.id, &a.id);
    let run=execute(&engine,vec![json!({"type":"read_page","artifact_id":a.id}),json!({"type":"read_page","artifact_id":b.id}),json!({"type":"read_connections","artifact_id":a.id}),json!({"type":"propose_connection","source_id":a.id,"target_id":b.id,"connected":false,"reason":"Remove only this explicit directional reference."}),finish()],Some(&p.id)).await;
    assert_eq!(
        run.steps[2].result["references"].as_array().unwrap().len(),
        2
    );
    assert_eq!(run.steps[2].result["includes_lineage"], false);
    engine
        .review_agent_step(&run.id, &run.steps[3].id, true)
        .unwrap();
    assert_eq!(count(&engine, "workspace_links"), 1);
    assert_eq!(
        engine
            .get_artifact(&b.id)
            .unwrap()
            .derived_from_artifact_id
            .as_deref(),
        Some(a.id.as_str())
    );
    let cs = engine.agent_run(&run.id).unwrap().steps[3]
        .change_set_id
        .clone()
        .unwrap();
    engine.revert_change_set(&cs).unwrap();
    let ids: Vec<String> = engine
        .db()
        .reader()
        .unwrap()
        .prepare("SELECT id FROM workspace_links ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(ids.contains(&forward));
    assert!(ids.contains(&reverse));
    assert_eq!(
        engine.agent_run(&run.id).unwrap().steps[3].state,
        "reverted"
    );
}
#[tokio::test]
async fn later_same_clock_relationship_changes_invalidate_acceptance_and_undo() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let p = engine.create_project("Destination", None).unwrap();
    let a = page(&engine, "Source page");
    let b = page(&engine, "Target page");
    let run=execute(&engine,vec![json!({"type":"read_project","project_id":p.id}),json!({"type":"read_page","artifact_id":a.id}),json!({"type":"read_page","artifact_id":b.id}),json!({"type":"propose_membership","artifact_id":a.id,"project_id":p.id,"attached":true,"reason":"Add a membership."}),json!({"type":"propose_connection","source_id":a.id,"target_id":b.id,"connected":true,"reason":"Add a reference."}),finish()],None).await;
    engine
        .attach_artifact(&p.id, &a.id, "user_explicit", None)
        .unwrap();
    engine.detach_artifact(&p.id, &a.id).unwrap();
    assert!(!member(&engine, &p.id, &a.id));
    assert!(engine
        .review_agent_step(&run.id, &run.steps[3].id, true)
        .unwrap_err()
        .to_string()
        .contains("relationship changed"));
    engine
        .review_agent_step(&run.id, &run.steps[4].id, true)
        .unwrap();
    let id = reference(&engine, &a.id, &b.id);
    engine.delete_workspace_reference(&id).unwrap();
    reference(&engine, &a.id, &b.id);
    assert!(engine
        .undo_agent_edit(&run.id, &run.steps[4].id)
        .unwrap_err()
        .to_string()
        .contains("changed after acceptance"));
    assert_eq!(count(&engine, "workspace_links"), 1);
}
#[tokio::test]
async fn failed_and_concurrent_organization_acceptance_is_atomic() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let a = page(&engine, "Source page");
    let b = page(&engine, "Target page");
    let run=execute(&engine,vec![json!({"type":"read_page","artifact_id":a.id}),json!({"type":"read_page","artifact_id":b.id}),json!({"type":"propose_connection","source_id":a.id,"target_id":b.id,"connected":true,"reason":"Add a reference."}),finish()],None).await;
    let before = count(&engine, "workspace_relation_versions");
    let cs = count(&engine, "change_sets");
    engine.db().writer().execute_batch("CREATE TRIGGER reject_structure_receipt BEFORE UPDATE OF state ON agent_steps BEGIN SELECT RAISE(ABORT,'fixture receipt failure'); END;").unwrap();
    assert!(engine
        .review_agent_step(&run.id, &run.steps[2].id, true)
        .is_err());
    assert_eq!(count(&engine, "workspace_links"), 0);
    assert_eq!(count(&engine, "change_sets"), cs);
    assert_eq!(count(&engine, "workspace_relation_versions"), before);
    assert_eq!(engine.agent_run(&run.id).unwrap().steps[2].state, "pending");
    engine
        .db()
        .writer()
        .execute_batch("DROP TRIGGER reject_structure_receipt;")
        .unwrap();
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let engine = engine.clone();
            let id = run.id.clone();
            let step = run.steps[2].id.clone();
            std::thread::spawn(move || engine.review_agent_step(&id, &step, true).is_ok())
        })
        .collect();
    assert_eq!(
        workers
            .into_iter()
            .filter_map(|worker| worker.join().ok())
            .filter(|applied| *applied)
            .count(),
        1
    );
    assert_eq!(count(&engine, "workspace_links"), 1);
    assert_eq!(count(&engine, "change_sets"), cs + 1);
}
#[tokio::test]
async fn project_scope_filters_reference_endpoints_and_cannot_expand_via_arguments() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let p = engine.create_project("Visible project", None).unwrap();
    let hidden = engine.create_project("Private project", None).unwrap();
    let a = page(&engine, "Visible page");
    let b = page(&engine, "Private page");
    engine
        .attach_artifact(&p.id, &a.id, "user_explicit", None)
        .unwrap();
    engine
        .attach_artifact(&hidden.id, &b.id, "user_explicit", None)
        .unwrap();
    reference(&engine, &a.id, &b.id);
    let(url,server)=fixture(vec![json!({"type":"search_projects","query":"project"}),json!({"type":"read_project","project_id":hidden.id}),json!({"type":"read_connections","artifact_id":a.id}),json!({"type":"read_page","artifact_id":b.id}),json!({"type":"propose_membership","artifact_id":a.id,"project_id":hidden.id,"attached":true,"reason":"Try a broader scope."}),json!({"type":"finish","summary":"No authorized structural changes.","handoff":"Keep scope restricted."})]).await;
    configure(&engine, &url);
    let run = engine.create_agent_run(goal(Some(&p.id))).unwrap();
    let run = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    server.await.unwrap();
    assert_eq!(run.status, "completed");
    assert_eq!(run.steps[0].result["projects"].as_array().unwrap().len(), 1);
    assert!(!run.steps[0].result.to_string().contains("Private"));
    assert!(run.steps[2].result["references"]
        .as_array()
        .unwrap()
        .is_empty());
    for i in [1, 3, 4] {
        assert!(run.steps[i].result.get("error").is_some());
    }
    assert!(!member(&engine, &hidden.id, &a.id));
}
#[tokio::test]
async fn document_chat_cannot_use_organization_even_with_edit_permission() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let p = engine.create_project("Project", None).unwrap();
    let a = page(&engine, "Saved document");
    let(url,server)=fixture(vec![json!({"type":"search_projects","query":"Project"}),json!({"type":"read_project","project_id":p.id}),json!({"type":"read_connections","artifact_id":a.id}),json!({"type":"propose_membership","artifact_id":a.id,"project_id":p.id,"attached":true,"reason":"Try to broaden the grant."}),json!({"type":"finish","summary":"Document answer only.","handoff":"No organization changes."})]).await;
    configure(&engine, &url);
    engine
        .db()
        .writer()
        .execute(
            "INSERT INTO workflow_settings(workflow,enabled) VALUES('chat',1)",
            [],
        )
        .unwrap();
    let run = engine
        .create_document_agent_run(DocumentAgentGoal {
            artifact_id: a.id.clone(),
            text: "Keep this scoped".into(),
            model_id: "fixture-model".into(),
            propose_edit: true,
        })
        .unwrap();
    let run = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    server.await.unwrap();
    for step in &run.steps[..4] {
        assert!(step.result.get("error").is_some());
    }
    assert!(!member(&engine, &p.id, &a.id));
}
#[test]
fn v22_upgrade_seeds_relationship_revisions_without_changing_existing_content() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let p = engine.create_project("Saved project", None).unwrap();
    let a = page(&engine, "Saved page");
    let b = page(&engine, "Saved target");
    engine
        .attach_artifact(&p.id, &a.id, "user_explicit", None)
        .unwrap();
    let link = reference(&engine, &a.id, &b.id);
    engine.db().writer().execute_batch("DROP TRIGGER track_project_artifacts_insert; DROP TRIGGER track_project_artifacts_update; DROP TRIGGER track_project_artifacts_delete; DROP TRIGGER track_workspace_links_insert; DROP TRIGGER track_workspace_links_update; DROP TRIGGER track_workspace_links_delete; DROP TABLE workspace_relation_versions; PRAGMA user_version=22;").unwrap();
    drop(engine);
    let engine = open(tmp.path());
    assert_eq!(count(&engine, "workspace_relation_versions"), 2);
    assert!(member(&engine, &p.id, &a.id));
    assert_eq!(
        engine.get_artifact(&a.id).unwrap().content_hash,
        a.content_hash
    );
    engine
        .db()
        .writer()
        .execute_batch("PRAGMA user_version=22;")
        .unwrap();
    drop(engine);
    let engine = open(tmp.path());
    assert_eq!(count(&engine, "workspace_relation_versions"), 2);
    assert_eq!(reference(&engine, &a.id, &b.id), link);
    assert!(engine
        .db()
        .reader()
        .unwrap()
        .query_row("PRAGMA foreign_key_check", [], |r| r.get::<_, String>(0))
        .optional()
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn paginated_project_context_does_not_grant_unread_pages_and_detach_is_reversible() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let p = engine.create_project("Reading project", None).unwrap();
    let other = engine.create_project("Shared context", None).unwrap();
    let mut pages = vec![];
    for i in 0..22 {
        let a = page(&engine, &format!("Reading page {i}"));
        engine
            .attach_artifact(&p.id, &a.id, "user_explicit", None)
            .unwrap();
        pages.push(a);
    }
    let a = &pages[0];
    engine
        .attach_artifact(&other.id, &a.id, "user_explicit", None)
        .unwrap();
    let run=execute(&engine,vec![json!({"type":"read_project","project_id":p.id}),json!({"type":"read_project","project_id":p.id,"offset":20}),json!({"type":"propose_membership","artifact_id":a.id,"project_id":p.id,"attached":false,"reason":"Try without reading the page."}),json!({"type":"read_page","artifact_id":a.id}),json!({"type":"propose_membership","artifact_id":a.id,"project_id":p.id,"attached":false,"reason":"Keep the page in its other project only."}),finish()],Some(&p.id)).await;
    assert_eq!(run.steps[0].result["members"].as_array().unwrap().len(), 20);
    assert_eq!(run.steps[0].result["next_offset"], 20);
    assert_eq!(run.steps[1].result["members"].as_array().unwrap().len(), 2);
    assert!(run.steps[1].result["next_offset"].is_null());
    assert!(run.steps[2].result.get("error").is_some());
    engine
        .review_agent_step(&run.id, &run.steps[4].id, true)
        .unwrap();
    assert!(!member(&engine, &p.id, &a.id));
    assert!(member(&engine, &other.id, &a.id));
    assert_eq!(
        engine.get_artifact(&a.id).unwrap().content_hash,
        a.content_hash
    );
    engine.undo_agent_edit(&run.id, &run.steps[4].id).unwrap();
    assert!(member(&engine, &p.id, &a.id));
    assert!(member(&engine, &other.id, &a.id));
}

#[tokio::test]
async fn changed_source_pages_and_project_metadata_reject_review_without_mutations() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let p = engine.create_project("Original destination", None).unwrap();
    let a = page(&engine, "Membership page");
    let b = page(&engine, "Reference source");
    let c = page(&engine, "Reference target");
    let run=execute(&engine,vec![json!({"type":"read_project","project_id":p.id}),json!({"type":"read_page","artifact_id":a.id}),json!({"type":"read_page","artifact_id":b.id}),json!({"type":"read_page","artifact_id":c.id}),json!({"type":"propose_membership","artifact_id":a.id,"project_id":p.id,"attached":true,"reason":"Review project membership."}),json!({"type":"propose_connection","source_id":b.id,"target_id":c.id,"connected":true,"reason":"Review the source relationship."}),finish()],None).await;
    engine.db().writer().execute("UPDATE artifacts SET content_md='New owner content',content_hash='changed',version=version+1 WHERE id=?1",[&b.id]).unwrap();
    assert!(engine
        .review_agent_step(&run.id, &run.steps[5].id, true)
        .unwrap_err()
        .to_string()
        .contains("saved page changed"));
    engine
        .db()
        .writer()
        .execute(
            "UPDATE projects SET name='Different destination name' WHERE id=?1",
            [&p.id],
        )
        .unwrap();
    assert!(engine
        .review_agent_step(&run.id, &run.steps[4].id, true)
        .unwrap_err()
        .to_string()
        .contains("project changed"));
    assert!(!member(&engine, &p.id, &a.id));
    assert_eq!(count(&engine, "workspace_links"), 0);
    assert_eq!(engine.agent_run(&run.id).unwrap().steps[5].state, "pending");
    engine
        .review_agent_step(&run.id, &run.steps[4].id, false)
        .unwrap();
    assert!(!member(&engine, &p.id, &a.id));
    assert_eq!(
        engine.get_artifact(&b.id).unwrap().content_md,
        "New owner content"
    );
}
