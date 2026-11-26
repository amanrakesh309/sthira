use super::*;
use rusqlite::OptionalExtension;
fn finish() -> Value {
    json!({"type":"finish","summary":"Review the saved project proposal.","handoff":"Continue after acceptance."})
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
fn current_purpose(engine: &Engine, project: &str) -> Option<(String, String, String)> {
    engine.db().reader().unwrap().query_row("SELECT id,purpose,origin FROM project_purposes WHERE project_id=?1 AND superseded_at IS NULL",[project],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().unwrap()
}
fn page(engine: &Engine) -> sthira_core::Artifact {
    engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Saved research"),
            "Original research with unresolved questions.",
            None,
        )
        .unwrap()
}
fn member(engine: &Engine, project: &str, page: &str) -> bool {
    engine.db().reader().unwrap().query_row("SELECT EXISTS(SELECT 1 FROM project_artifacts WHERE project_id=?1 AND artifact_id=?2 AND detached_at IS NULL)",rusqlite::params![project,page],|r|r.get(0)).unwrap()
}
#[tokio::test]
async fn project_creation_is_reviewed_attributed_duplicate_safe_and_reversible_when_unused() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let run=execute(&engine,vec![json!({"type":"propose_project_create","name":"Release planning","purpose":"Investigate release timing without claiming a confirmed date.","reason":"Keep the planning context separate."}),finish()],None).await;
    assert_eq!(count(&engine, "projects"), 0);
    assert_eq!(count(&engine, "project_purposes"), 0);
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let engine = engine.clone();
            let run = run.id.clone();
            let step = engine.agent_run(&run).unwrap().steps[0].id.clone();
            std::thread::spawn(move || engine.review_agent_step(&run, &step, true).is_ok())
        })
        .collect();
    assert_eq!(
        workers
            .into_iter()
            .filter_map(|w| w.join().ok())
            .filter(|v| *v)
            .count(),
        1
    );
    let saved = engine.agent_run(&run.id).unwrap();
    let id = saved.steps[0].result["project_id"].as_str().unwrap();
    assert_eq!(count(&engine, "projects"), 1);
    assert_eq!(current_purpose(&engine, id).unwrap().2, "ai_inferred");
    assert!(saved.steps[0].change_set_id.is_some());
    assert_eq!(engine.get_project(id).unwrap().name, "Release planning");
    let undone = engine.undo_agent_edit(&run.id, &saved.steps[0].id).unwrap();
    assert_eq!(undone.steps[0].state, "reverted");
    let deleted: bool = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT deleted_at IS NOT NULL FROM projects WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(deleted);
    assert!(engine.undo_agent_edit(&run.id, &saved.steps[0].id).is_err());
}
#[tokio::test]
async fn purpose_updates_keep_history_and_original_authorship_on_undo_and_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let p = engine.create_project("Research planning", None).unwrap();
    let original = engine
        .set_project_purpose(&p.id, "Owner's unresolved research direction.")
        .unwrap();
    let run=execute(&engine,vec![json!({"type":"read_project","project_id":p.id}),json!({"type":"propose_project_purpose","project_id":p.id,"purpose":"Investigate options before choosing a release date.","reason":"Preserve uncertainty while clarifying the next step."}),finish()],Some(&p.id)).await;
    assert_eq!(current_purpose(&engine, &p.id).unwrap().0, original);
    let accepted = engine
        .review_agent_step(&run.id, &run.steps[1].id, true)
        .unwrap();
    assert_eq!(accepted.status, "completed");
    assert_eq!(current_purpose(&engine, &p.id).unwrap().2, "ai_inferred");
    assert_eq!(count(&engine, "project_purposes"), 2);
    engine.undo_agent_edit(&run.id, &run.steps[1].id).unwrap();
    let restored = current_purpose(&engine, &p.id).unwrap();
    assert_eq!(restored.1, "Owner's unresolved research direction.");
    assert_eq!(restored.2, "user_recorded");
    assert_ne!(restored.0, original);
    assert_eq!(count(&engine, "project_purposes"), 3);
    drop(engine);
    let reopened = open(tmp.path());
    assert_eq!(
        reopened.agent_run(&run.id).unwrap().steps[1].state,
        "reverted"
    );
    assert_eq!(current_purpose(&reopened, &p.id).unwrap().1, restored.1);
}
#[tokio::test]
async fn project_metadata_change_and_restore_under_same_clock_rejects_stale_approval() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let p = engine.create_project("Original name", None).unwrap();
    engine
        .set_project_purpose(&p.id, "Original direction.")
        .unwrap();
    let run=execute(&engine,vec![json!({"type":"read_project","project_id":p.id}),json!({"type":"propose_project_purpose","project_id":p.id,"purpose":"Proposed direction.","reason":"Review the direction change."}),finish()],Some(&p.id)).await;
    engine
        .db()
        .writer()
        .execute(
            "UPDATE projects SET name='Changed name' WHERE id=?1",
            [&p.id],
        )
        .unwrap();
    engine
        .db()
        .writer()
        .execute(
            "UPDATE projects SET name='Original name' WHERE id=?1",
            [&p.id],
        )
        .unwrap();
    assert!(engine
        .review_agent_step(&run.id, &run.steps[1].id, true)
        .unwrap_err()
        .to_string()
        .contains("project changed"));
    assert_eq!(
        current_purpose(&engine, &p.id).unwrap().1,
        "Original direction."
    );
    assert_eq!(engine.agent_run(&run.id).unwrap().steps[1].state, "pending");
}
#[tokio::test]
async fn atomic_move_and_undo_preserve_other_memberships_identity_and_newer_body() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let from = engine.create_project("Source project", None).unwrap();
    let to = engine.create_project("Destination", None).unwrap();
    let other = engine.create_project("Shared project", None).unwrap();
    let a = page(&engine);
    engine
        .attach_artifact(&from.id, &a.id, "user_explicit", None)
        .unwrap();
    engine
        .attach_artifact(&other.id, &a.id, "user_explicit", None)
        .unwrap();
    let run=execute(&engine,vec![json!({"type":"read_project","project_id":from.id}),json!({"type":"read_project","project_id":to.id}),json!({"type":"read_page","artifact_id":a.id}),json!({"type":"propose_move","artifact_id":a.id,"from_project_id":from.id,"to_project_id":to.id,"reason":"Move only the selected source membership."}),finish()],None).await;
    assert!(member(&engine, &from.id, &a.id));
    assert!(!member(&engine, &to.id, &a.id));
    engine
        .review_agent_step(&run.id, &run.steps[3].id, true)
        .unwrap();
    assert!(!member(&engine, &from.id, &a.id));
    assert!(member(&engine, &to.id, &a.id));
    assert!(member(&engine, &other.id, &a.id));
    assert_eq!(engine.get_artifact(&a.id).unwrap().version, a.version);
    engine.db().writer().execute("UPDATE artifacts SET content_md='New owner text',content_hash='new',version=version+1 WHERE id=?1",[&a.id]).unwrap();
    engine.undo_agent_edit(&run.id, &run.steps[3].id).unwrap();
    assert!(member(&engine, &from.id, &a.id));
    assert!(!member(&engine, &to.id, &a.id));
    assert!(member(&engine, &other.id, &a.id));
    assert_eq!(
        engine.get_artifact(&a.id).unwrap().content_md,
        "New owner text"
    );
    assert_eq!(
        engine.get_artifact(&a.id).unwrap().authorship,
        Authorship::User
    );
}
#[tokio::test]
async fn failed_move_receipt_rolls_back_both_memberships_counters_and_change_set() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let from = engine.create_project("Source", None).unwrap();
    let to = engine.create_project("Destination", None).unwrap();
    let a = page(&engine);
    engine
        .attach_artifact(&from.id, &a.id, "user_explicit", None)
        .unwrap();
    let run=execute(&engine,vec![json!({"type":"read_project","project_id":from.id}),json!({"type":"read_project","project_id":to.id}),json!({"type":"read_page","artifact_id":a.id}),json!({"type":"propose_move","artifact_id":a.id,"from_project_id":from.id,"to_project_id":to.id,"reason":"Move the selected membership."}),finish()],None).await;
    let counters = count(&engine, "workspace_relation_versions");
    let cs = count(&engine, "change_sets");
    engine.db().writer().execute_batch("CREATE TRIGGER fail_project_review BEFORE UPDATE OF state ON agent_steps BEGIN SELECT RAISE(ABORT,'fixture receipt failure'); END;").unwrap();
    assert!(engine
        .review_agent_step(&run.id, &run.steps[3].id, true)
        .is_err());
    assert!(member(&engine, &from.id, &a.id));
    assert!(!member(&engine, &to.id, &a.id));
    assert_eq!(count(&engine, "workspace_relation_versions"), counters);
    assert_eq!(count(&engine, "change_sets"), cs);
    assert_eq!(engine.agent_run(&run.id).unwrap().steps[3].state, "pending");
}
#[tokio::test]
async fn creation_undo_refuses_project_use_and_purpose_undo_refuses_newer_direction() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let p = engine.create_project("Existing project", None).unwrap();
    engine
        .set_project_purpose(&p.id, "Original purpose.")
        .unwrap();
    let run=execute(&engine,vec![json!({"type":"propose_project_create","name":"New project","purpose":"Tentative direction.","reason":"Review a new workspace."}),json!({"type":"read_project","project_id":p.id}),json!({"type":"propose_project_purpose","project_id":p.id,"purpose":"Suggested purpose.","reason":"Review direction."}),finish()],None).await;
    let created = engine
        .review_agent_step(&run.id, &run.steps[0].id, true)
        .unwrap();
    let id = created.steps[0].result["project_id"].as_str().unwrap();
    let a = page(&engine);
    engine
        .attach_artifact(id, &a.id, "user_explicit", None)
        .unwrap();
    engine.detach_artifact(id, &a.id).unwrap();
    assert!(engine
        .undo_agent_edit(&run.id, &run.steps[0].id)
        .unwrap_err()
        .to_string()
        .contains("used since creation"));
    engine
        .review_agent_step(&run.id, &run.steps[2].id, true)
        .unwrap();
    engine
        .set_project_purpose(&p.id, "Owner's newer direction.")
        .unwrap();
    assert!(engine.undo_agent_edit(&run.id, &run.steps[2].id).is_err());
    assert_eq!(
        current_purpose(&engine, &p.id).unwrap().1,
        "Owner's newer direction."
    );
}
#[tokio::test]
async fn membership_change_and_restore_under_same_clock_blocks_move_undo() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let to = engine.create_project("Destination", None).unwrap();
    let other = engine.create_project("Another project", None).unwrap();
    let a = page(&engine);
    let run=execute(&engine,vec![json!({"type":"read_project","project_id":to.id}),json!({"type":"read_page","artifact_id":a.id}),json!({"type":"propose_move","artifact_id":a.id,"from_project_id":null,"to_project_id":to.id,"reason":"Give the standalone page a project."}),finish()],None).await;
    engine
        .review_agent_step(&run.id, &run.steps[2].id, true)
        .unwrap();
    assert!(member(&engine, &to.id, &a.id));
    engine
        .attach_artifact(&other.id, &a.id, "user_explicit", None)
        .unwrap();
    engine.detach_artifact(&other.id, &a.id).unwrap();
    assert!(engine
        .undo_agent_edit(&run.id, &run.steps[2].id)
        .unwrap_err()
        .to_string()
        .contains("memberships changed"));
    assert!(member(&engine, &to.id, &a.id));
}

#[tokio::test]
async fn project_and_document_grants_cannot_be_expanded_by_new_tool_arguments() {
    for document in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let engine = open(tmp.path());
        let p = engine.create_project("Allowed project", None).unwrap();
        let q = engine.create_project("Other project", None).unwrap();
        let a = page(&engine);
        engine
            .attach_artifact(&p.id, &a.id, "user_explicit", None)
            .unwrap();
        let(url,server)=fixture(vec![json!({"type":"read_project","project_id":p.id}),json!({"type":"propose_project_create","name":"Forbidden project","purpose":"Unrequested broader workspace.","reason":"Try to broaden scope."}),json!({"type":"propose_move","artifact_id":a.id,"from_project_id":p.id,"to_project_id":q.id,"reason":"Try to move outside scope."}),json!({"type":"propose_project_purpose","project_id":q.id,"purpose":"Unrequested direction.","reason":"Try to change another project."}),finish()]).await;
        configure(&engine, &url);
        let run = if document {
            engine
                .db()
                .writer()
                .execute(
                    "INSERT INTO workflow_settings(workflow,enabled) VALUES('chat',1)",
                    [],
                )
                .unwrap();
            engine
                .create_document_agent_run(DocumentAgentGoal {
                    artifact_id: a.id.clone(),
                    text: "Edit only this document".into(),
                    model_id: "fixture-model".into(),
                    propose_edit: true,
                })
                .unwrap()
        } else {
            engine.create_agent_run(goal(Some(&p.id))).unwrap()
        };
        let run = engine
            .execute_agent_run(&run.id, &CancellationToken::new())
            .await
            .unwrap();
        server.await.unwrap();
        assert_eq!(run.status, "completed");
        for i in 1..=3 {
            assert!(run.steps[i].result.get("error").is_some());
        }
        assert!(member(&engine, &p.id, &a.id));
        assert!(!member(&engine, &q.id, &a.id));
        assert_eq!(count(&engine, "projects"), 2);
        assert_eq!(count(&engine, "project_purposes"), 0);
    }
}
#[tokio::test]
async fn project_and_purpose_receipt_failures_roll_back_every_domain_write() {
    for create in [true, false] {
        let tmp = tempfile::tempdir().unwrap();
        let engine = open(tmp.path());
        let p = engine.create_project("Original project", None).unwrap();
        engine
            .set_project_purpose(&p.id, "Original direction.")
            .unwrap();
        let action = if create {
            json!({"type":"propose_project_create","name":"New project","purpose":"New direction.","reason":"Review project creation."})
        } else {
            json!({"type":"propose_project_purpose","project_id":p.id,"purpose":"New direction.","reason":"Review direction."})
        };
        let run = execute(
            &engine,
            vec![
                json!({"type":"read_project","project_id":p.id}),
                action,
                finish(),
            ],
            None,
        )
        .await;
        let before = current_purpose(&engine, &p.id).unwrap();
        let events = count(&engine, "session_events");
        let cs = count(&engine, "change_sets");
        let revision: i64 = engine
            .db()
            .reader()
            .unwrap()
            .query_row(
                "SELECT revision FROM workspace_project_versions WHERE project_id=?1",
                [&p.id],
                |r| r.get(0),
            )
            .unwrap();
        engine.db().writer().execute_batch("CREATE TRIGGER reject_project_receipt BEFORE UPDATE OF state ON agent_steps BEGIN SELECT RAISE(ABORT,'fixture project receipt failure'); END;").unwrap();
        assert!(engine
            .review_agent_step(&run.id, &run.steps[1].id, true)
            .is_err());
        assert_eq!(count(&engine, "projects"), 1);
        assert_eq!(count(&engine, "project_purposes"), 1);
        assert_eq!(current_purpose(&engine, &p.id).unwrap(), before);
        assert_eq!(count(&engine, "session_events"), events);
        assert_eq!(count(&engine, "change_sets"), cs);
        assert_eq!(engine.agent_run(&run.id).unwrap().steps[1].state, "pending");
        assert_eq!(
            engine
                .db()
                .reader()
                .unwrap()
                .query_row(
                    "SELECT revision FROM workspace_project_versions WHERE project_id=?1",
                    [&p.id],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            revision
        );
    }
}
#[test]
fn owner_purpose_failure_preserves_the_previous_active_direction_atomically() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let p = engine.create_project("Owner project", None).unwrap();
    engine
        .set_project_purpose(&p.id, "Keep the original direction.")
        .unwrap();
    let before = current_purpose(&engine, &p.id).unwrap();
    engine.db().writer().execute_batch("CREATE TRIGGER reject_purpose BEFORE INSERT ON project_purposes BEGIN SELECT RAISE(ABORT,'fixture purpose failure'); END;").unwrap();
    assert!(engine
        .set_project_purpose(&p.id, "Must not partially supersede.")
        .is_err());
    assert_eq!(current_purpose(&engine, &p.id).unwrap(), before);
}
#[tokio::test]
async fn v23_upgrade_keeps_legacy_pending_reviews_and_project_content_intact() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let p = engine.create_project("Saved project", None).unwrap();
    engine
        .set_project_purpose(&p.id, "Saved owner direction.")
        .unwrap();
    let a = page(&engine);
    let run=execute(&engine,vec![json!({"type":"read_project","project_id":p.id}),json!({"type":"read_page","artifact_id":a.id}),json!({"type":"propose_membership","artifact_id":a.id,"project_id":p.id,"attached":true,"reason":"Old organization proposal."}),finish()],None).await;
    engine.db().writer().execute_batch("DROP TRIGGER version_projects_insert; DROP TRIGGER version_projects_update; DROP TRIGGER version_projects_delete; DROP TRIGGER version_project_purposes_insert; DROP TRIGGER version_project_purposes_update; DROP TRIGGER version_project_purposes_delete; DROP TABLE workspace_project_versions; DROP INDEX idx_workspace_membership_revision_page; PRAGMA user_version=23;").unwrap();
    drop(engine);
    let engine = open(tmp.path());
    assert_eq!(
        current_purpose(&engine, &p.id).unwrap().1,
        "Saved owner direction."
    );
    assert_eq!(
        engine.get_artifact(&a.id).unwrap().content_hash,
        a.content_hash
    );
    engine
        .review_agent_step(&run.id, &run.steps[2].id, true)
        .unwrap();
    assert!(member(&engine, &p.id, &a.id));
    let version: i64 = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT revision FROM workspace_project_versions WHERE project_id=?1",
            [&p.id],
            |r| r.get(0),
        )
        .unwrap();
    engine
        .db()
        .writer()
        .execute_batch("PRAGMA user_version=23;")
        .unwrap();
    drop(engine);
    let engine = open(tmp.path());
    assert_eq!(
        engine
            .db()
            .reader()
            .unwrap()
            .query_row(
                "SELECT revision FROM workspace_project_versions WHERE project_id=?1",
                [&p.id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        version
    );
    assert_eq!(
        engine
            .db()
            .reader()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        30
    );
}
#[tokio::test]
async fn purpose_undo_preserves_the_original_ai_producing_change_set() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let p = engine.create_project("AI direction history", None).unwrap();
    let mut actions = vec![];
    for purpose in ["First AI direction.", "Second AI direction."] {
        actions.extend([json!({"type":"read_project","project_id":p.id}),json!({"type":"propose_project_purpose","project_id":p.id,"purpose":purpose,"reason":"Review the suggested direction."}),finish()]);
    }
    let (url, server) = fixture(actions).await;
    configure(&engine, &url);
    let first = engine.create_agent_run(goal(Some(&p.id))).unwrap();
    let first = engine
        .execute_agent_run(&first.id, &CancellationToken::new())
        .await
        .unwrap();
    let first = engine
        .review_agent_step(&first.id, &first.steps[1].id, true)
        .unwrap();
    let cs = first.steps[1].change_set_id.clone().unwrap();
    let next = engine
        .follow_up_agent_run(&first.id, "Refine the direction", None)
        .unwrap();
    let next = engine
        .execute_agent_run(&next.id, &CancellationToken::new())
        .await
        .unwrap();
    server.await.unwrap();
    engine
        .review_agent_step(&next.id, &next.steps[1].id, true)
        .unwrap();
    engine.undo_agent_edit(&next.id, &next.steps[1].id).unwrap();
    let restored = current_purpose(&engine, &p.id).unwrap();
    assert_eq!(restored.1, "First AI direction.");
    assert_eq!(restored.2, "ai_inferred");
    let link: Option<String> = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT change_set_id FROM project_purposes WHERE id=?1",
            [&restored.0],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(link.as_deref(), Some(cs.as_str()));
}

#[tokio::test]
async fn followups_receive_the_accepted_project_identity_without_replaying_creation() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let(url,server)=fixture(vec![json!({"type":"propose_project_create","name":"Continuity project","purpose":"Investigate open questions.","reason":"Review project creation."}),finish(),finish()]).await;
    configure(&engine, &url);
    let first = engine.create_agent_run(goal(None)).unwrap();
    let first = engine
        .execute_agent_run(&first.id, &CancellationToken::new())
        .await
        .unwrap();
    let saved = engine
        .review_agent_step(&first.id, &first.steps[0].id, true)
        .unwrap();
    let project = saved.steps[0].result["project_id"].as_str().unwrap();
    let next = engine
        .follow_up_agent_run(&first.id, "Continue with the accepted project", None)
        .unwrap();
    engine
        .execute_agent_run(&next.id, &CancellationToken::new())
        .await
        .unwrap();
    let requests = server.await.unwrap();
    let prompt: Value = serde_json::from_str(
        requests.last().unwrap()["messages"][1]["content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        prompt["history"]["previous_runs"][0]["proposals"][0]["project_id"],
        project
    );
    assert_eq!(
        prompt["history"]["previous_runs"][0]["proposals"][0]["state"],
        "applied"
    );
    assert_eq!(count(&engine, "projects"), 1);
}

#[tokio::test]
async fn page_membership_hints_help_resolve_moves_without_expanding_project_or_document_grants() {
    for scope in 0..3 {
        let tmp = tempfile::tempdir().unwrap();
        let engine = open(tmp.path());
        let p = engine.create_project("Granted context", None).unwrap();
        let q = engine.create_project("Other context", None).unwrap();
        let a = page(&engine);
        engine
            .attach_artifact(&p.id, &a.id, "user_explicit", None)
            .unwrap();
        engine
            .attach_artifact(&q.id, &a.id, "user_explicit", None)
            .unwrap();
        let (url, server) = fixture(vec![
            json!({"type":"read_page","artifact_id":a.id}),
            finish(),
        ])
        .await;
        configure(&engine, &url);
        let run = if scope == 2 {
            engine
                .db()
                .writer()
                .execute(
                    "INSERT INTO workflow_settings(workflow,enabled) VALUES('chat',1)",
                    [],
                )
                .unwrap();
            engine
                .create_document_agent_run(DocumentAgentGoal {
                    artifact_id: a.id.clone(),
                    text: "Read this document only".into(),
                    model_id: "fixture-model".into(),
                    propose_edit: false,
                })
                .unwrap()
        } else {
            engine
                .create_agent_run(goal(if scope == 1 { Some(&p.id) } else { None }))
                .unwrap()
        };
        let run = engine
            .execute_agent_run(&run.id, &CancellationToken::new())
            .await
            .unwrap();
        server.await.unwrap();
        let hints = run.steps[0].result["project_memberships"]
            .as_array()
            .unwrap();
        assert_eq!(
            hints.len(),
            match scope {
                0 => 2,
                1 => 1,
                _ => 0,
            }
        );
        if scope == 1 {
            assert_eq!(hints[0]["project_id"], p.id);
            assert!(!run.steps[0].result.to_string().contains("Other context"));
        }
    }
}
