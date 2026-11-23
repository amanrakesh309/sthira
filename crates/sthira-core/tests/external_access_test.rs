use serde_json::{json, Value};
use std::sync::Arc;
use sthira_core::{
    shared_memory::{MemoryAccess, MemoryAction, MemoryDraft, MemoryResult, MemoryScope},
    workspace_agent::external::{ExternalBatch, ExternalGrant},
    ArtifactType, Authorship, CoreEvent, Engine,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
fn open(tmp: &tempfile::TempDir) -> Engine {
    Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path(),
        Arc::new(FakeClock::new(1000)),
        Arc::new(DeterministicFakeModel::new("unused").with_panic_on_call()),
    )
    .unwrap()
}
fn grant(e: &Engine, project: Option<String>, write: bool) -> (String, String) {
    let (c, t) = e
        .grant_external_access(ExternalGrant {
            name: "Test client".into(),
            whole_workspace: project.is_none(),
            project_id: project,
            workspace_memory: false,
            can_propose: write,
            model: "Declared external model".into(),
            provider: "Declared external provider".into(),
        })
        .unwrap();
    (c.id, t)
}
fn batch(key: &str, actions: Vec<Value>) -> ExternalBatch {
    serde_json::from_value(json!({"request_id":key,"goal":"Inspect saved context and prepare reviewed work.","actions":actions})).unwrap()
}
fn page(e: &Engine, text: &str) -> sthira_core::Artifact {
    e.create_artifact(
        ArtifactType::Note,
        Authorship::User,
        Some("Owner source"),
        text,
        None,
    )
    .unwrap()
}
#[test]
fn attach_never_recovers_native_work_or_seeds_another_vault() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(&tmp);
    e.db().writer().execute("INSERT INTO agent_runs(id,goal,workspace_memory,model_id,model_name,provider_id,provider_name,status,created_at,updated_at,thread_id) VALUES('native','Running owner work',0,'m','M','p','P','running',1,1,'native')",[]).unwrap();
    let attached = Engine::attach(
        tmp.path().join("sthira.db"),
        tmp.path(),
        Arc::new(FakeClock::new(2000)),
        Arc::new(DeterministicFakeModel::new("unused")),
    )
    .unwrap();
    assert_eq!(attached.agent_run("native").unwrap().status, "running");
    assert_eq!(e.agent_run("native").unwrap().revision, 1);
    assert!(Engine::attach(
        tmp.path().join("missing.db"),
        tmp.path(),
        Arc::new(FakeClock::new(0)),
        Arc::new(DeterministicFakeModel::new("unused"))
    )
    .is_err());
    assert!(!tmp.path().join("missing.db").exists());
}
#[test]
fn scoped_reads_and_readonly_credentials_cannot_broaden_their_grant() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(&tmp);
    let p = e.create_project("Visible", None).unwrap();
    let hidden = e.create_project("Hidden", None).unwrap();
    let a = page(&e, "Visible body");
    let b = page(&e, "Secret body");
    e.attach_artifact(&p.id, &a.id, "user_explicit", None)
        .unwrap();
    e.attach_artifact(&hidden.id, &b.id, "user_explicit", None)
        .unwrap();
    let (_, token) = grant(&e, Some(p.id.clone()), false);
    let result = e
        .external_workspace(
            &token,
            batch(
                "read",
                vec![
                    json!({"type":"read_page","artifact_id":a.id}),
                    json!({"type":"read_page","artifact_id":b.id}),
                    json!({"type":"read_project","project_id":hidden.id}),
                ],
            ),
        )
        .unwrap();
    assert_eq!(
        result["run"]["steps"][0]["result"]["content_md"],
        "Visible body"
    );
    assert!(result["run"]["steps"][1]["result"]["error"].is_string());
    assert!(!result.to_string().contains("Secret body"));
    assert!(e
        .external_workspace(
            &token,
            batch(
                "denied",
                vec![
                    json!({"type":"propose_create","title":"Unauthorized","content_md":"No write"})
                ]
            )
        )
        .is_err());
    assert!(serde_json::from_value::<ExternalBatch>(
        json!({"request_id":"x","goal":"x","actions":[],"owner":true})
    )
    .is_err());
}
#[test]
fn external_proposals_use_native_approval_attribution_undo_and_retry_receipts() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(&tmp);
    let (_, token) = grant(&e, None, true);
    let input = batch(
        "once",
        vec![
            json!({"type":"propose_project_create","name":"External idea","purpose":"An unresolved direction.","reason":"Separate this work."}),
        ],
    );
    let result = e.external_workspace(&token, input.clone()).unwrap();
    let run: sthira_core::workspace_agent::AgentRun =
        serde_json::from_value(result["run"].clone()).unwrap();
    assert_eq!(run.status, "review");
    assert!(run.model_name.contains("External: Test client"));
    assert_eq!(
        e.db()
            .reader()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        e.external_workspace(&token, input).unwrap()["replayed"],
        true
    );
    assert!(e
        .external_workspace(
            &token,
            batch("once", vec![json!({"type":"search_projects","query":""})])
        )
        .is_err());
    let accepted = e
        .review_agent_step(&run.id, &run.steps[0].id, true)
        .unwrap();
    let project = accepted.steps[0].result["project_id"].as_str().unwrap();
    let origin: String = e
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT origin FROM project_purposes WHERE project_id=?1 AND superseded_at IS NULL",
            [project],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(origin, "ai_inferred");
    assert_eq!(
        e.undo_agent_edit(&run.id, &run.steps[0].id).unwrap().steps[0].state,
        "reverted"
    );
}
#[test]
fn revocation_blocks_existing_connections_and_all_native_approval_boundaries() {
    for action in [
        "propose_create",
        "propose_edit",
        "propose_refine",
        "propose_titles",
        "propose_memory_create",
        "propose_membership",
        "propose_project_purpose",
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let e = open(&tmp);
        let p = e.create_project("Project", None).unwrap();
        let a = page(&e, "Owner words.");
        e.attach_artifact(&p.id, &a.id, "user_explicit", None)
            .unwrap();
        let (id, token) = grant(&e, Some(p.id.clone()), true);
        let proposed = match action {
            "propose_create" => {
                json!({"type":action,"title":"Reviewed page","content_md":"External words."})
            }
            "propose_edit" | "propose_refine" => {
                json!({"type":action,"artifact_id":a.id,"content_md":"Revised words."})
            }
            "propose_titles" => {
                json!({"type":action,"artifact_id":a.id,"titles":["Unresolved research direction"]})
            }
            "propose_memory_create" => {
                json!({"type":action,"scope":"project","title":"Open question","body":"Still uncertain.","source_ids":[a.id]})
            }
            "propose_membership" => {
                json!({"type":action,"project_id":p.id,"artifact_id":a.id,"attached":false,"reason":"Review detachment."})
            }
            _ => {
                json!({"type":action,"project_id":p.id,"purpose":"Explore carefully.","reason":"Clarify direction."})
            }
        };
        let result = e
            .external_workspace(
                &token,
                batch(
                    action,
                    vec![
                        json!({"type":"read_page","artifact_id":a.id}),
                        json!({"type":"read_project","project_id":p.id}),
                        proposed,
                    ],
                ),
            )
            .unwrap();
        let run: sthira_core::workspace_agent::AgentRun =
            serde_json::from_value(result["run"].clone()).unwrap();
        assert_eq!(run.steps[2].state, "pending", "{action}");
        e.revoke_external_access(&id).unwrap();
        assert!(e.external_status(&token).is_err());
        assert!(
            e.review_agent_step_with_title(
                &run.id,
                &run.steps[2].id,
                true,
                if action == "propose_titles" {
                    Some("Unresolved research direction")
                } else {
                    None
                }
            )
            .is_err(),
            "{action}"
        );
        assert_eq!(e.get_artifact(&a.id).unwrap().content_md, "Owner words.");
        assert!(e
            .review_agent_step(&run.id, &run.steps[2].id, false)
            .is_ok());
    }
}
#[test]
fn memory_scope_read_history_and_context_keep_shared_action_rules() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(&tmp);
    let p = e.create_project("Scoped", None).unwrap();
    let a = page(&e, "Evidence");
    e.attach_artifact(&p.id, &a.id, "user_explicit", None)
        .unwrap();
    let MemoryResult::Entry(m) = e
        .execute_memory_action(
            &MemoryAccess::owner(),
            MemoryAction::Create {
                draft: MemoryDraft {
                    scope: MemoryScope::Project(p.id.clone()),
                    title: "Uncertainty".into(),
                    body: "Not yet decided.".into(),
                    source_ids: vec![a.id],
                    pinned: false,
                },
            },
        )
        .unwrap()
    else {
        panic!()
    };
    let (_, token) = grant(&e, Some(p.id), true);
    assert!(e
        .external_memory(&token, MemoryAction::Read { id: m.id.clone() })
        .is_ok());
    assert!(e
        .external_memory(&token, MemoryAction::History { id: m.id.clone() })
        .is_ok());
    assert!(e
        .external_memory(
            &token,
            MemoryAction::Archive {
                id: m.id.clone(),
                expected_version: m.version
            }
        )
        .is_err());
    let (_, other) = grant(&e, Some(e.create_project("Other", None).unwrap().id), false);
    assert!(e
        .external_memory(&other, MemoryAction::Read { id: m.id })
        .is_err());
}
#[test]
fn stale_proposals_do_not_overwrite_owner_edits_and_reset_invalidates_tokens() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(&tmp);
    let a = page(&e, "Original");
    let (_, token) = grant(&e, None, true);
    let result=e.external_workspace(&token,batch("edit",vec![json!({"type":"read_page","artifact_id":a.id}),json!({"type":"propose_refine","artifact_id":a.id,"content_md":"External revision"})])).unwrap();
    let run: sthira_core::workspace_agent::AgentRun =
        serde_json::from_value(result["run"].clone()).unwrap();
    e.db()
        .writer()
        .execute(
            "UPDATE artifacts SET content_md='New owner edit',version=version+1 WHERE id=?1",
            [a.id],
        )
        .unwrap();
    assert!(e
        .review_agent_step(&run.id, &run.steps[1].id, true)
        .is_err());
    Engine::rotate_vault_instance_id_tx(&e.db().writer()).unwrap();
    assert!(e.external_status(&token).is_err());
}
#[test]
fn external_events_are_ordered_replayable_and_contain_no_secrets() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(&tmp);
    let (_, token) = grant(&e, None, false);
    let cursor = e.external_event_cursor().unwrap();
    let result = e
        .external_workspace(
            &token,
            batch("events", vec![json!({"type":"search_projects","query":""})]),
        )
        .unwrap();
    let events = e.external_events_after(cursor).unwrap();
    assert_eq!(events.len(), 1);
    assert!(
        matches!(&events[0].1,CoreEvent::AgentChanged{id} if id==result["run"]["id"].as_str().unwrap())
    );
    assert!(e.external_events_after(events[0].0).unwrap().is_empty());
    assert!(!serde_json::to_string(&events).unwrap().contains(&token));
}
#[test]
fn v24_upgrade_is_additive_idempotent_and_keeps_pending_projects() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(&tmp);
    let p = e.create_project("Retained", None).unwrap();
    let (_, token) = grant(&e, None, false);
    let version = e
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT revision FROM workspace_project_versions WHERE project_id=?1",
            [&p.id],
            |r| r.get::<_, i64>(0),
        )
        .unwrap();
    e.db()
        .writer()
        .execute_batch("PRAGMA user_version=24")
        .unwrap();
    drop(e);
    let e = open(&tmp);
    assert!(e.external_status(&token).is_ok());
    assert_eq!(
        e.db()
            .reader()
            .unwrap()
            .query_row(
                "SELECT revision FROM workspace_project_versions WHERE project_id=?1",
                [p.id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        version
    );
    assert_eq!(
        e.db()
            .reader()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        27
    );
}
#[test]
fn revocation_between_batch_actions_stops_the_next_action_and_preserves_review_barrier() {
    let tmp = tempfile::tempdir().unwrap();
    let e = open(&tmp);
    let a = page(&e, "Original");
    let (id, token) = grant(&e, None, true);
    e.db().writer().execute_batch("CREATE TRIGGER revoke_during_external_request AFTER INSERT ON agent_steps BEGIN UPDATE external_connections SET revoked_at=1001; END;").unwrap();
    assert!(e
        .external_workspace(
            &token,
            batch(
                "revoked-mid-flight",
                vec![
                    json!({"type":"read_page","artifact_id":a.id}),
                    json!({"type":"propose_edit","artifact_id":a.id,"content_md":"Do not apply"})
                ]
            )
        )
        .is_err());
    assert_eq!(e.get_artifact(&a.id).unwrap().content_md, "Original");
    assert_eq!(e.list_agent_runs().unwrap()[0].status, "failed");
    assert_eq!(e.list_agent_runs().unwrap()[0].steps.len(), 1);
    assert!(e
        .list_external_connections()
        .unwrap()
        .iter()
        .find(|c| c.id == id)
        .unwrap()
        .revoked_at
        .is_some());
}
