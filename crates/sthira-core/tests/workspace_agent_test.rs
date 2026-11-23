//! Real SQLite and loopback model transport; not live-provider quality evidence.
use serde_json::{json, Value};
use std::sync::Arc;
use sthira_core::{
    llm::lifecycle::CancellationToken, shared_memory::*, workspace_agent::*, ArtifactType,
    Authorship, Engine,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
fn open(path: &std::path::Path) -> Engine {
    Engine::open(
        path.join("sthira.db"),
        path.join("vault"),
        Arc::new(FakeClock::new(1000)),
        Arc::new(DeterministicFakeModel::new("unused-fallback")),
    )
    .unwrap()
}
fn configure(engine: &Engine, url: &str) {
    let writer = engine.db().writer();
    writer.execute("INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) VALUES('fixture','Fixture provider','openai_compatible',?1,1,0)",[url]).unwrap();
    writer.execute_batch("INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) VALUES('fixture-model','fixture','agent-fixture','Fixture model',1,1,0); INSERT INTO workflow_settings(workflow,enabled) VALUES('agent',1);").unwrap();
}
fn goal(project: Option<&str>) -> AgentGoal {
    AgentGoal {
        goal: "Read the notes and prepare a reviewable next step".into(),
        project_id: project.map(str::to_owned),
        workspace_memory: false,
        model_id: "fixture-model".into(),
    }
}
async fn request(socket: &mut tokio::net::TcpStream) -> Value {
    let mut bytes = vec![];
    let mut buffer = [0; 4096];
    loop {
        let count = socket.read(&mut buffer).await.unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..end]);
            let length: usize = headers
                .lines()
                .find_map(|l| {
                    l.to_lowercase()
                        .strip_prefix("content-length:")
                        .map(|s| s.trim().parse().unwrap())
                })
                .unwrap();
            if bytes.len() >= end + 4 + length {
                return serde_json::from_slice(&bytes[end + 4..end + 4 + length]).unwrap();
            }
        }
    }
}
async fn respond(socket: &mut tokio::net::TcpStream, action: &Value) {
    let body = json!({"choices":[{"message":{"content":action.to_string()}}]}).to_string();
    let _=socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await;
}
async fn fixture(actions: Vec<Value>) -> (String, tokio::task::JoinHandle<Vec<Value>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let mut requests = vec![];
        for action in actions {
            let (mut socket, _) = listener.accept().await.unwrap();
            let req = request(&mut socket).await;
            assert_eq!(req["model"], "agent-fixture");
            requests.push(req);
            respond(&mut socket, &action).await;
        }
        requests
    });
    (url, task)
}
fn count(engine: &Engine, table: &str) -> i64 {
    engine
        .db()
        .reader()
        .unwrap()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
#[tokio::test]
async fn goal_tools_review_reopen_and_undo_preserve_scope_identity_and_attribution() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Reading", None).unwrap();
    let private = engine.create_project("Unrelated", None).unwrap();
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Reading note"),
            "Original uncertainty.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &page.id, "user_explicit", None)
        .unwrap();
    let hidden = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Reading private"),
            "Do not disclose",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&private.id, &hidden.id, "user_explicit", None)
        .unwrap();
    for (scope, title) in [
        (MemoryScope::Project(project.id.clone()), "Allowed memory"),
        (MemoryScope::Project(private.id), "Private memory"),
        (MemoryScope::Workspace, "Workspace secret"),
    ] {
        engine
            .execute_memory_action(
                &MemoryAccess::owner(),
                MemoryAction::Create {
                    draft: MemoryDraft {
                        scope,
                        title: title.into(),
                        body: "Preserve uncertainty.".into(),
                        source_ids: vec![],
                        pinned: false,
                    },
                },
            )
            .unwrap();
    }
    let (url,server)=fixture(vec![json!({"type":"search","query":"Reading"}),json!({"type":"read_page","artifact_id":page.id}),json!({"type":"propose_edit","artifact_id":page.id,"content_md":"Reviewed uncertainty."}),json!({"type":"propose_create","title":"Next questions","content_md":"# Open questions\n\nStill unresolved."}),json!({"type":"finish","summary":"Two proposals await review.","handoff":"Verify the remaining sources."})]).await;
    configure(&engine, &url);
    let run = engine.create_agent_run(goal(Some(&project.id))).unwrap();
    let run = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(run.status, "review");
    assert_eq!(run.model_name, "Fixture model");
    assert_eq!(run.provider_name, "Fixture provider");
    assert_eq!(run.steps.len(), 5);
    assert_eq!(count(&engine, "artifacts"), 2);
    assert_eq!(
        engine.get_artifact(&page.id).unwrap().content_md,
        page.content_md
    );
    let requests = server.await.unwrap();
    let first = requests[0]["messages"][1]["content"].as_str().unwrap();
    assert!(first.contains("Allowed memory"));
    assert!(!first.contains("Private memory"));
    assert!(!first.contains("Workspace secret"));
    let second = requests[1]["messages"][1]["content"].as_str().unwrap();
    assert!(second.contains("Reading note"));
    assert!(!second.contains("Reading private"));
    let edit = run.steps[2].clone();
    let create = run.steps[3].clone();
    let applied = engine.review_agent_step(&run.id, &edit.id, true).unwrap();
    assert_eq!(applied.steps[2].state, "applied");
    assert_eq!(
        engine.get_artifact(&page.id).unwrap().authorship,
        Authorship::User
    );
    assert!(engine.review_agent_step(&run.id, &edit.id, true).is_err());
    engine.undo_agent_edit(&run.id, &edit.id).unwrap();
    assert_eq!(
        engine.get_artifact(&page.id).unwrap().content_md,
        page.content_md
    );
    engine.review_agent_step(&run.id, &create.id, true).unwrap();
    assert!(engine.review_agent_step(&run.id, &create.id, true).is_err());
    assert_eq!(count(&engine, "artifacts"), 3);
    let current = engine.agent_run(&run.id).unwrap();
    let new_id = current.steps[3].artifact_id.clone().unwrap();
    let created = engine.get_artifact(&new_id).unwrap();
    assert_eq!(created.authorship, Authorship::AiGenerated);
    assert!(created.last_synced_hash.is_some());
    let reader = engine.db().reader().unwrap();
    assert_eq!(
        reader
            .query_row(
                "SELECT actor FROM artifact_revisions WHERE artifact_id=?1",
                [&new_id],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "ai"
    );
    assert_eq!(
        reader
            .query_row(
                "SELECT project_id FROM project_artifacts WHERE artifact_id=?1",
                [&new_id],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        project.id
    );
    drop(reader);
    drop(engine);
    let engine = open(tmp.path());
    let reopened = engine.agent_run(&run.id).unwrap();
    assert_eq!(reopened.status, "completed");
    assert_eq!(
        reopened.handoff.as_deref(),
        Some("Verify the remaining sources.")
    );
    assert_eq!(reopened.steps[2].state, "reverted");
    assert_eq!(count(&engine, "epistemic_nodes"), 0);
}
#[tokio::test]
async fn tool_errors_are_recorded_without_leaking_outside_scope_and_stale_reviews_fail() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let p = engine.create_project("Selected", None).unwrap();
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Scoped"),
            "Original",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&p.id, &page.id, "user_explicit", None)
        .unwrap();
    let hidden = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Hidden"),
            "Private text",
            None,
        )
        .unwrap();
    let (url,server)=fixture(vec![json!({"type":"read_page","artifact_id":hidden.id}),json!({"type":"propose_edit","artifact_id":page.id,"content_md":"Must read first"}),json!({"type":"read_page","artifact_id":page.id}),json!({"type":"propose_edit","artifact_id":page.id,"content_md":"Suggested"}),json!({"type":"finish","summary":"Review the proposal.","handoff":"Source remains uncertain."})]).await;
    configure(&engine, &url);
    let queued = engine.create_agent_run(goal(Some(&p.id))).unwrap();
    let run = engine
        .execute_agent_run(&queued.id, &CancellationToken::new())
        .await
        .unwrap();
    let requests = server.await.unwrap();
    assert!(run.steps[0].result.get("error").is_some());
    assert!(run.steps[1].result.get("error").is_some());
    assert!(!requests
        .iter()
        .any(|r| r.to_string().contains("Private text")));
    assert_eq!(count(&engine, "document_conversation_turns"), 1);
    engine
        .update_artifact(
            &page.id,
            "New owner edit",
            sthira_core::Actor::User,
            Some(&p.id),
        )
        .unwrap();
    assert!(engine
        .review_agent_step(&run.id, &run.steps[3].id, true)
        .is_err());
    assert_eq!(
        engine.get_artifact(&page.id).unwrap().content_md,
        "New owner edit"
    );
    engine.detach_artifact(&p.id, &page.id).unwrap();
    assert!(engine
        .review_agent_step(&run.id, &run.steps[3].id, true)
        .is_err());
    engine
        .review_agent_step(&run.id, &run.steps[3].id, false)
        .unwrap();
    assert_eq!(
        engine.agent_run(&run.id).unwrap().steps[3].state,
        "dismissed"
    );
}
#[tokio::test]
async fn cancellation_drops_inference_and_cannot_queue_or_apply_a_late_write() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = Arc::new(open(tmp.path()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    configure(
        &engine,
        &format!("http://{}/v1", listener.local_addr().unwrap()),
    );
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        request(&mut socket).await;
        started_tx.send(()).unwrap();
        release_rx.await.unwrap();
        respond(
            &mut socket,
            &json!({"type":"propose_create","title":"Late page","content_md":"Must not be saved"}),
        )
        .await;
    });
    let queued = engine.create_agent_run(goal(None)).unwrap();
    let id = queued.id.clone();
    let token = CancellationToken::new();
    let worker_engine = engine.clone();
    let worker_token = token.clone();
    let worker = tokio::spawn(async move {
        worker_engine
            .execute_agent_run(&id, &worker_token)
            .await
            .unwrap()
    });
    started_rx.await.unwrap();
    engine.cancel_agent_run(&queued.id).unwrap();
    token.cancel();
    let stopped = worker.await.unwrap();
    release_tx.send(()).unwrap();
    server.await.unwrap();
    assert_eq!(stopped.status, "cancelled");
    assert!(stopped.steps.is_empty());
    assert_eq!(count(&engine, "artifacts"), 0);
    assert!(engine
        .execute_agent_run(&queued.id, &CancellationToken::new())
        .await
        .is_err());
}
#[tokio::test]
async fn malformed_actions_and_step_budget_fail_honestly_without_creating_pages() {
    for actions in [
        vec![json!({"type":"shell","command":"touch unsafe","author":"owner"})],
        (0..8)
            .map(|_| json!({"type":"search","query":"missing"}))
            .collect(),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let engine = open(tmp.path());
        let (url, server) = fixture(actions).await;
        configure(&engine, &url);
        let queued = engine.create_agent_run(goal(None)).unwrap();
        let run = engine
            .execute_agent_run(&queued.id, &CancellationToken::new())
            .await
            .unwrap();
        server.await.unwrap();
        assert_eq!(run.status, "failed");
        assert!(run.error.is_some());
        assert!(run.summary.is_none());
        assert_eq!(count(&engine, "artifacts"), 0);
    }
}
#[test]
fn restart_marks_unfinished_runs_and_disabled_or_missing_models_never_dispatch() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    assert!(engine.create_agent_run(goal(None)).is_err());
    configure(&engine, "http://127.0.0.1:1/v1");
    let run = engine.create_agent_run(goal(None)).unwrap();
    let mut invalid = goal(None);
    invalid.model_id = "missing".into();
    assert!(engine.create_agent_run(invalid).is_err());
    engine
        .db()
        .writer()
        .execute("UPDATE provider_models SET is_enabled=0", [])
        .unwrap();
    assert!(engine.create_agent_run(goal(None)).is_err());
    drop(engine);
    let engine = open(tmp.path());
    assert_eq!(engine.agent_run(&run.id).unwrap().status, "interrupted");
    assert_eq!(engine.list_agent_runs().unwrap().len(), 1);
    assert_eq!(
        engine
            .db()
            .reader()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        27
    );
}
#[tokio::test]
async fn failed_and_concurrent_create_reviews_consume_the_proposal_once() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = Arc::new(open(tmp.path()));
    let (url,server)=fixture(vec![json!({"type":"propose_create","title":"Reviewed page","content_md":"Saved only once."}),json!({"type":"finish","summary":"One proposal awaits review.","handoff":"No further questions."})]).await;
    configure(&engine, &url);
    let queued = engine.create_agent_run(goal(None)).unwrap();
    let run = engine
        .execute_agent_run(&queued.id, &CancellationToken::new())
        .await
        .unwrap();
    server.await.unwrap();
    let step = run.steps[0].id.clone();
    engine.db().writer().execute_batch("CREATE TRIGGER reject_agent_revision BEFORE INSERT ON artifact_revisions BEGIN SELECT RAISE(ABORT,'fixture revision failure'); END;").unwrap();
    assert!(engine.review_agent_step(&run.id, &step, true).is_err());
    assert_eq!(count(&engine, "artifacts"), 0);
    assert_eq!(engine.agent_run(&run.id).unwrap().steps[0].state, "pending");
    engine
        .db()
        .writer()
        .execute_batch("DROP TRIGGER reject_agent_revision")
        .unwrap();
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let engine = engine.clone();
            let id = run.id.clone();
            let step = step.clone();
            std::thread::spawn(move || engine.review_agent_step(&id, &step, true).is_ok())
        })
        .collect();
    let successes = workers
        .into_iter()
        .filter_map(|w| w.join().ok())
        .filter(|v| *v)
        .count();
    assert_eq!(successes, 1);
    assert_eq!(count(&engine, "artifacts"), 1);
    assert_eq!(count(&engine, "artifact_revisions"), 1);
}
#[test]
fn v20_upgrade_preserves_workflows_notes_and_memory_and_defaults_agent_off() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Existing"),
            "Keep original content",
            None,
        )
        .unwrap();
    engine
        .execute_memory_action(
            &MemoryAccess::owner(),
            MemoryAction::Create {
                draft: MemoryDraft {
                    scope: MemoryScope::Workspace,
                    title: "Existing memory".into(),
                    body: "Keep provenance".into(),
                    source_ids: vec![],
                    pinned: true,
                },
            },
        )
        .unwrap();
    engine.db().writer().execute_batch("INSERT INTO workflow_settings(workflow,enabled,model_id,templates_enabled) VALUES('research',1,'saved-selection',1); DROP TABLE agent_steps; DROP TABLE agent_runs; PRAGMA user_version=20;").unwrap();
    drop(engine);
    for _ in 0..2 {
        let engine = open(tmp.path());
        assert_eq!(count(&engine, "artifacts"), 1);
        assert_eq!(count(&engine, "shared_memories"), 1);
        let reader = engine.db().reader().unwrap();
        assert_eq!(reader.query_row("SELECT model_id FROM workflow_settings WHERE workflow='research' AND enabled=1 AND templates_enabled=1",[],|r|r.get::<_,String>(0)).unwrap(),"saved-selection");
        assert!(!reader.query_row("SELECT EXISTS(SELECT 1 FROM workflow_settings WHERE workflow='agent' AND enabled=1)",[],|r|r.get::<_,bool>(0)).unwrap());
    }
}
#[tokio::test]
async fn disabling_a_provider_stops_further_model_calls_in_an_active_run() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = Arc::new(open(tmp.path()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    configure(
        &engine,
        &format!("http://{}/v1", listener.local_addr().unwrap()),
    );
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        request(&mut socket).await;
        started_tx.send(()).unwrap();
        release_rx.await.unwrap();
        respond(&mut socket, &json!({"type":"search","query":"notes"})).await;
    });
    let queued = engine.create_agent_run(goal(None)).unwrap();
    let id = queued.id.clone();
    let worker_engine = engine.clone();
    let worker = tokio::spawn(async move {
        worker_engine
            .execute_agent_run(&id, &CancellationToken::new())
            .await
            .unwrap()
    });
    started_rx.await.unwrap();
    engine
        .db()
        .writer()
        .execute("UPDATE providers SET is_enabled=0 WHERE id='fixture'", [])
        .unwrap();
    release_tx.send(()).unwrap();
    server.await.unwrap();
    let result = worker.await.unwrap();
    assert_eq!(result.status, "failed");
    assert_eq!(result.steps.len(), 1);
    assert!(result.error.unwrap().contains("disabled or removed"));
}
#[tokio::test]
async fn a_committed_creation_returns_mirror_warning_even_if_warning_bookkeeping_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let (url,server)=fixture(vec![json!({"type":"propose_create","title":"Saved page","content_md":"Database content remains available."}),json!({"type":"finish","summary":"One proposal to review.","handoff":"Check mirror state."})]).await;
    configure(&engine, &url);
    let queued = engine.create_agent_run(goal(None)).unwrap();
    let run = engine
        .execute_agent_run(&queued.id, &CancellationToken::new())
        .await
        .unwrap();
    server.await.unwrap();
    std::fs::remove_dir(engine.vault_dir()).unwrap();
    std::fs::write(engine.vault_dir(), "blocked mirror directory").unwrap();
    engine.db().writer().execute_batch("CREATE TRIGGER reject_warning BEFORE UPDATE OF result_json ON agent_steps BEGIN SELECT RAISE(ABORT,'fixture warning bookkeeping failure'); END;").unwrap();
    let result = engine
        .review_agent_step(&run.id, &run.steps[0].id, true)
        .unwrap();
    assert_eq!(result.steps[0].state, "applied");
    assert!(result.steps[0].result.get("mirror_warning").is_some());
    assert_eq!(count(&engine, "artifacts"), 1);
    assert!(engine
        .review_agent_step(&run.id, &run.steps[0].id, true)
        .is_err());
}

fn enable_chat(engine: &Engine) {
    engine.db().writer().execute_batch("INSERT INTO workflow_settings(workflow,enabled) VALUES('chat',1) ON CONFLICT(workflow) DO UPDATE SET enabled=1; UPDATE workflow_settings SET enabled=0 WHERE workflow='agent';").unwrap();
}
fn document_goal(page: &str, text: &str, edit: bool) -> DocumentAgentGoal {
    DocumentAgentGoal {
        artifact_id: page.into(),
        text: text.into(),
        model_id: "fixture-model".into(),
        propose_edit: edit,
    }
}
#[tokio::test]
async fn document_agent_preserves_legacy_history_and_restricts_reads_and_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Current document"),
            "An unresolved proposal.",
            None,
        )
        .unwrap();
    let hidden = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Private unrelated"),
            "Secret unrelated content",
            None,
        )
        .unwrap();
    engine
        .document_conversation(&page.id, Some("document"), "Original goal")
        .unwrap();
    engine.db().writer().execute("INSERT INTO document_conversation_turns(id,artifact_id,user_text,reply,basis_content_md,model_name,provider_name,created_at) VALUES('legacy',?1,'Earlier question','Earlier answer',?2,'Earlier model','Earlier provider',0)",rusqlite::params![page.id,page.content_md]).unwrap();
    let (url,server)=fixture(vec![json!({"type":"read_page","artifact_id":hidden.id}),json!({"type":"propose_edit","artifact_id":page.id,"content_md":"Unauthorized edit"}),json!({"type":"propose_create","title":"Unexpected page","content_md":"Must not be created"}),json!({"type":"finish","summary":"The proposal remains unresolved.","handoff":"Review the evidence."})]).await;
    configure(&engine, &url);
    enable_chat(&engine);
    let queued = engine
        .create_document_agent_run(document_goal(&page.id, "Explain this", false))
        .unwrap();
    assert_eq!(queued.document_id.as_deref(), Some(page.id.as_str()));
    assert!(!queued.allow_edits);
    assert!(engine
        .create_document_agent_run(document_goal(&page.id, "Duplicate", false))
        .is_err());
    let pending = engine
        .document_conversation(&page.id, None, "")
        .unwrap()
        .unwrap();
    assert_eq!(pending.turns.len(), 2);
    assert_eq!(
        pending.turns[1].agent_run_id.as_deref(),
        Some(queued.id.as_str())
    );
    assert!(pending.turns[1].reply.is_none());
    let result = engine
        .execute_agent_run(&queued.id, &CancellationToken::new())
        .await
        .unwrap();
    let requests = server.await.unwrap();
    let prompt: Value =
        serde_json::from_str(requests[0]["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(prompt["document"]["artifact_id"], page.id);
    assert!(prompt["history"].to_string().contains("Earlier answer"));
    assert_eq!(
        prompt["history"]["document_origin"]["original_goal"],
        "Original goal"
    );
    assert!(!requests
        .iter()
        .any(|r| r.to_string().contains("Secret unrelated content")));
    assert!(result
        .steps
        .iter()
        .take(3)
        .all(|step| step.result.get("error").is_some()));
    assert_eq!(result.status, "completed");
    assert_eq!(count(&engine, "artifacts"), 2);
    let chat = engine
        .document_conversation(&page.id, None, "")
        .unwrap()
        .unwrap();
    assert_eq!(chat.turns.len(), 2);
    assert_eq!(chat.turns[0].model_name.as_deref(), Some("Earlier model"));
    assert_eq!(
        chat.turns[1].reply.as_deref(),
        Some("The proposal remains unresolved.")
    );
    assert_eq!(chat.turns[1].agent_status.as_deref(), Some("completed"));
    assert_eq!(
        engine.get_artifact(&page.id).unwrap().content_md,
        page.content_md
    );
}
#[tokio::test]
async fn document_edit_proposal_keeps_new_owner_edits_and_requires_finished_run_review() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = Arc::new(open(tmp.path()));
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Draft"),
            "Original saved document",
            None,
        )
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    configure(
        &engine,
        &format!("http://{}/v1", listener.local_addr().unwrap()),
    );
    enable_chat(&engine);
    let (waiting_tx, waiting_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let doc = page.id.clone();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request_body = request(&mut socket).await;
        assert!(request_body.to_string().contains("Original saved document"));
        respond(
            &mut socket,
            &json!({"type":"propose_edit","artifact_id":doc,"content_md":"Proposed document"}),
        )
        .await;
        let (mut socket, _) = listener.accept().await.unwrap();
        request(&mut socket).await;
        waiting_tx.send(()).unwrap();
        release_rx.await.unwrap();
        respond(&mut socket,&json!({"type":"finish","summary":"An edit awaits your review.","handoff":"Keep uncertainty."})).await;
    });
    let queued = engine
        .create_document_agent_run(document_goal(&page.id, "Revise this", true))
        .unwrap();
    let id = queued.id.clone();
    let worker_engine = engine.clone();
    let worker = tokio::spawn(async move {
        worker_engine
            .execute_agent_run(&id, &CancellationToken::new())
            .await
            .unwrap()
    });
    waiting_rx.await.unwrap();
    let chat = engine
        .document_conversation(&page.id, None, "")
        .unwrap()
        .unwrap();
    assert_eq!(chat.turns.len(), 1);
    let turn = chat.turns[0].id.clone();
    assert!(engine.decide_document_edit(&page.id, &turn, true).is_err());
    engine
        .update_artifact(
            &page.id,
            "Newer owner draft",
            sthira_core::Actor::User,
            None,
        )
        .unwrap();
    release_tx.send(()).unwrap();
    server.await.unwrap();
    let finished = worker.await.unwrap();
    assert_eq!(finished.status, "review");
    assert!(engine.decide_document_edit(&page.id, &turn, true).is_err());
    assert_eq!(
        engine.get_artifact(&page.id).unwrap().content_md,
        "Newer owner draft"
    );
    engine.decide_document_edit(&page.id, &turn, false).unwrap();
    assert_eq!(engine.agent_run(&queued.id).unwrap().status, "completed");
    assert_eq!(
        engine
            .document_conversation(&page.id, None, "")
            .unwrap()
            .unwrap()
            .turns
            .len(),
        1
    );
}
#[tokio::test]
async fn followups_inherit_scope_and_include_current_proposal_state_without_reapplying_it() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Scoped project", None).unwrap();
    let (url,server)=fixture(vec![json!({"type":"propose_create","title":"First approach","content_md":"Original proposal."}),json!({"type":"finish","summary":"The first approach is ready.","handoff":"Choose an approach."}),json!({"type":"finish","summary":"Continuing from the accepted first approach.","handoff":"Keep working."})]).await;
    configure(&engine, &url);
    engine.db().writer().execute("INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) VALUES('fixture-b','Second provider','openai_compatible',?1,1,0)",[&url]).unwrap();
    engine.db().writer().execute_batch("INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) VALUES('second-model','fixture-b','agent-fixture','Second model',1,0,0);").unwrap();
    let queued = engine.create_agent_run(goal(Some(&project.id))).unwrap();
    let first = engine
        .execute_agent_run(&queued.id, &CancellationToken::new())
        .await
        .unwrap();
    engine
        .review_agent_step(&first.id, &first.steps[0].id, true)
        .unwrap();
    let next = engine
        .follow_up_agent_run(&first.id, "Use that approach", Some("second-model"))
        .unwrap();
    assert_eq!(next.thread_id, first.thread_id);
    assert_eq!(next.parent_run_id.as_deref(), Some(first.id.as_str()));
    assert_eq!(next.project_id, first.project_id);
    assert_eq!(next.workspace_memory, first.workspace_memory);
    assert_eq!(next.model_name, "Second model");
    assert!(engine
        .follow_up_agent_run(&first.id, "Competing followup", None)
        .is_err());
    let finished = engine
        .execute_agent_run(&next.id, &CancellationToken::new())
        .await
        .unwrap();
    let requests = server.await.unwrap();
    let last: Value =
        serde_json::from_str(requests[2]["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        last["history"]["previous_runs"][0]["proposals"][0]["state"],
        "applied"
    );
    assert!(last["history"].to_string().contains("Original proposal."));
    assert_eq!(count(&engine, "artifacts"), 1);
    assert_eq!(
        engine.agent_run(&first.id).unwrap().model_name,
        "Fixture model"
    );
    drop(engine);
    let reopened = open(tmp.path());
    assert_eq!(
        reopened.agent_run(&finished.id).unwrap().parent_run_id,
        finished.parent_run_id
    );
}
#[test]
fn queued_document_tasks_interrupt_on_reopen_and_capacity_failure_is_atomic() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    configure(&engine, "http://127.0.0.1:1/v1");
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Existing"),
            "Saved content",
            None,
        )
        .unwrap();
    for _ in 0..3 {
        engine.create_agent_run(goal(None)).unwrap();
    }
    enable_chat(&engine);
    assert!(engine
        .create_document_agent_run(document_goal(&page.id, "Over capacity", false))
        .is_err());
    assert_eq!(count(&engine, "document_conversation_turns"), 0);
    let first = engine.list_agent_runs().unwrap()[0].id.clone();
    engine.cancel_agent_run(&first).unwrap();
    let queued = engine
        .create_document_agent_run(document_goal(&page.id, "A saved attempt", false))
        .unwrap();
    drop(engine);
    let engine = open(tmp.path());
    let run = engine.agent_run(&queued.id).unwrap();
    assert_eq!(run.status, "interrupted");
    let chat = engine
        .document_conversation(&page.id, None, "")
        .unwrap()
        .unwrap();
    assert!(chat.turns[0]
        .error
        .as_deref()
        .unwrap()
        .contains("app stopped"));
    let follow = engine
        .create_document_agent_run(document_goal(
            &page.id,
            "Continue after interruption",
            false,
        ))
        .unwrap();
    assert_eq!(follow.parent_run_id.as_deref(), Some(run.id.as_str()));
    assert_eq!(follow.thread_id, run.thread_id);
    assert!(engine
        .list_agent_tasks()
        .unwrap()
        .iter()
        .any(|task| task.id == follow.id && task.status == "queued"));
}
#[test]
fn v21_upgrade_preserves_runs_and_legacy_chat_with_repeatable_thread_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    configure(&engine, "http://127.0.0.1:1/v1");
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Original page"),
            "Untouched prose",
            None,
        )
        .unwrap();
    let mirror = std::fs::read(engine.vault_dir().join(page.file_path.as_ref().unwrap())).unwrap();
    engine
        .document_conversation(&page.id, Some("document"), "Original goal")
        .unwrap();
    engine.db().writer().execute("INSERT INTO document_conversation_turns(id,artifact_id,user_text,reply,basis_content_md,created_at) VALUES('old-turn',?1,'Older question','Older answer',?2,0)",rusqlite::params![page.id,page.content_md]).unwrap();
    let run = engine.create_agent_run(goal(None)).unwrap();
    engine.db().writer().execute_batch("UPDATE agent_runs SET status='completed',summary='Saved result'; DROP INDEX idx_agent_document_turn; DROP INDEX idx_agent_document; DROP INDEX idx_agent_thread; ALTER TABLE agent_runs DROP COLUMN thread_id; ALTER TABLE agent_runs DROP COLUMN parent_run_id; ALTER TABLE agent_runs DROP COLUMN document_id; ALTER TABLE agent_runs DROP COLUMN document_turn_id; ALTER TABLE agent_runs DROP COLUMN allow_edits; ALTER TABLE agent_runs DROP COLUMN revision; PRAGMA user_version=21;").unwrap();
    drop(engine);
    for _ in 0..2 {
        let engine = open(tmp.path());
        let saved = engine.agent_run(&run.id).unwrap();
        assert_eq!(saved.thread_id, run.id);
        assert!(saved.parent_run_id.is_none());
        assert!(saved.document_id.is_none());
        assert_eq!(saved.status, "completed");
        assert_eq!(saved.summary.as_deref(), Some("Saved result"));
        let chat = engine
            .document_conversation(&page.id, None, "")
            .unwrap()
            .unwrap();
        assert_eq!(chat.turns[0].reply.as_deref(), Some("Older answer"));
        assert!(chat.turns[0].agent_run_id.is_none());
        assert_eq!(
            std::fs::read(engine.vault_dir().join(page.file_path.as_ref().unwrap())).unwrap(),
            mirror
        );
    }
}

#[tokio::test]
async fn reviewed_refine_derive_and_titles_use_domain_history_and_reject_stale_competitors() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Workflow review", None).unwrap();
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Original research"),
            "# Original research\n\nMaybe ship next year.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &page.id, "user_explicit", None)
        .unwrap();
    let (url,server)=fixture(vec![
        json!({"type":"read_page","artifact_id":page.id}),
        json!({"type":"propose_refine","artifact_id":page.id,"content_md":"# Original research\n\nShipping next year remains uncertain."}),
        json!({"type":"propose_derive","artifact_id":page.id,"title":"Research execution plan","content_md":"Validate timing before scheduling.","derivation_type":"plan_from_research"}),
        json!({"type":"propose_titles","artifact_id":page.id,"titles":["Uncertain release timing","Research release questions"]}),
        json!({"type":"finish","summary":"Three proposals await review.","handoff":"Timing remains unresolved."}),
    ]).await;
    configure(&engine, &url);
    let run = engine.create_agent_run(goal(Some(&project.id))).unwrap();
    let run = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    server.await.unwrap();
    assert_eq!(run.status, "review");
    assert_eq!(count(&engine, "artifacts"), 1);
    assert_eq!(
        engine.get_artifact(&page.id).unwrap().content_md,
        page.content_md
    );
    let derived = &run.steps[2];
    let approved = engine
        .review_agent_step(&run.id, &derived.id, true)
        .unwrap();
    let child_id = approved.steps[2].artifact_id.as_ref().unwrap();
    let child = engine.get_artifact(child_id).unwrap();
    assert_eq!(
        child.derived_from_artifact_id.as_deref(),
        Some(page.id.as_str())
    );
    assert_eq!(child.derivation_type.as_deref(), Some("plan_from_research"));
    assert_eq!(child.authorship, Authorship::AiGenerated);
    assert!(engine
        .review_agent_step(&run.id, &derived.id, true)
        .is_err());
    assert_eq!(count(&engine, "artifacts"), 2);
    let refined = engine
        .review_agent_step(&run.id, &run.steps[1].id, true)
        .unwrap();
    assert!(refined.steps[1].change_set_id.is_some());
    let updated = engine.get_artifact(&page.id).unwrap();
    assert_eq!(updated.authorship, Authorship::User);
    assert_eq!(updated.title, page.title);
    assert!(engine
        .review_agent_step_with_title(
            &run.id,
            &run.steps[3].id,
            true,
            Some("Uncertain release timing")
        )
        .unwrap_err()
        .to_string()
        .contains("changed"));
    let undone = engine.undo_agent_edit(&run.id, &run.steps[1].id).unwrap();
    assert_eq!(undone.steps[1].state, "reverted");
    assert_eq!(undone.steps[1].result["undone"], true);
    assert_eq!(
        engine.get_artifact(&page.id).unwrap().content_md,
        page.content_md
    );
    assert!(engine.undo_agent_edit(&run.id, &run.steps[1].id).is_err());
}

#[tokio::test]
async fn titles_require_an_exact_owner_choice_and_preserve_matching_heading_behavior() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Old research title"),
            "# Old research title\n\nPossible release, not confirmed.",
            None,
        )
        .unwrap();
    let (url,server)=fixture(vec![json!({"type":"read_page","artifact_id":page.id}),json!({"type":"propose_titles","artifact_id":page.id,"titles":["Possible release schedule","Open release questions"]}),json!({"type":"finish","summary":"Choose a title.","handoff":"No release confirmed."})]).await;
    configure(&engine, &url);
    let run = engine.create_agent_run(goal(None)).unwrap();
    let run = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    server.await.unwrap();
    let step = &run.steps[1];
    assert!(engine.review_agent_step(&run.id, &step.id, true).is_err());
    assert!(engine
        .review_agent_step_with_title(&run.id, &step.id, true, Some("Forged approved release"))
        .is_err());
    assert_eq!(engine.agent_run(&run.id).unwrap().steps[1].state, "pending");
    let applied = engine
        .review_agent_step_with_title(&run.id, &step.id, true, Some("Open release questions"))
        .unwrap();
    assert_eq!(applied.status, "completed");
    assert_eq!(
        applied.steps[1].result["applied_title"],
        "Open release questions"
    );
    let renamed = engine.get_artifact(&page.id).unwrap();
    assert_eq!(renamed.id, page.id);
    assert_eq!(renamed.authorship, Authorship::User);
    assert_eq!(
        renamed.content_md,
        "# Open release questions\n\nPossible release, not confirmed."
    );
    assert!(engine
        .review_agent_step_with_title(&run.id, &step.id, true, Some("Possible release schedule"))
        .is_err());
}

#[tokio::test]
async fn reviewed_memory_preserves_original_author_and_records_agent_editor_and_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Memory review", None).unwrap();
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Source context"),
            "Release date remains uncertain.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &page.id, "user_explicit", None)
        .unwrap();
    let original = match engine
        .execute_memory_action(
            &MemoryAccess::owner(),
            MemoryAction::Create {
                draft: MemoryDraft {
                    scope: MemoryScope::Project(project.id.clone()),
                    title: "Open question".into(),
                    body: "Date undecided.".into(),
                    source_ids: vec![page.id.clone()],
                    pinned: true,
                },
            },
        )
        .unwrap()
    {
        MemoryResult::Entry(e) => e,
        _ => unreachable!(),
    };
    let (url,server)=fixture(vec![
        json!({"type":"read_page","artifact_id":page.id}),
        json!({"type":"propose_memory_create","scope":"workspace","title":"Invalid scope","body":"Cannot publish beyond grants.","source_ids":[]}),
        json!({"type":"propose_memory_create","scope":"project","title":"Planning uncertainty","body":"Confirm the release date before planning.","source_ids":[page.id]}),
        json!({"type":"read_memory","id":original.id}),
        json!({"type":"propose_memory_update","id":original.id,"expected_version":original.version,"title":"Release question","body":"Release timing remains unresolved."}),
        json!({"type":"propose_memory_update","id":original.id,"expected_version":original.version,"title":"Competing release question","body":"An obsolete competing revision."}),
        json!({"type":"finish","summary":"Memory proposals await review.","handoff":"Review before publishing."}),
    ]).await;
    configure(&engine, &url);
    let run = engine.create_agent_run(goal(Some(&project.id))).unwrap();
    let run = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    server.await.unwrap();
    assert!(run.steps[1].result.get("error").is_some());
    assert_eq!(count(&engine, "shared_memories"), 1);
    let approved = engine
        .review_agent_step(&run.id, &run.steps[2].id, true)
        .unwrap();
    let id = approved.steps[2].result["memory_id"].as_str().unwrap();
    let created = match engine
        .execute_memory_action(&MemoryAccess::owner(), MemoryAction::Read { id: id.into() })
        .unwrap()
    {
        MemoryResult::Entry(e) => e,
        _ => unreachable!(),
    };
    assert!(matches!(created.author, MemoryActor::Agent { .. }));
    assert_eq!(created.sources[0].content_hash, page.content_hash);
    assert_eq!(created.scope, MemoryScope::Project(project.id.clone()));
    let approved = engine
        .review_agent_step(&run.id, &run.steps[4].id, true)
        .unwrap();
    assert_eq!(approved.status, "review");
    assert!(engine
        .review_agent_step(&run.id, &run.steps[5].id, true)
        .is_err());
    assert_eq!(engine.agent_run(&run.id).unwrap().steps[5].state, "pending");
    assert_eq!(
        engine
            .review_agent_step(&run.id, &run.steps[5].id, false)
            .unwrap()
            .status,
        "completed"
    );
    let updated = match engine
        .execute_memory_action(
            &MemoryAccess::owner(),
            MemoryAction::Read {
                id: original.id.clone(),
            },
        )
        .unwrap()
    {
        MemoryResult::Entry(e) => e,
        _ => unreachable!(),
    };
    assert_eq!(updated.author, MemoryActor::User);
    assert_eq!(updated.version, 2);
    assert!(updated.pinned);
    assert_eq!(updated.sources[0].content_hash, page.content_hash);
    let history = match engine
        .execute_memory_action(
            &MemoryAccess::owner(),
            MemoryAction::History {
                id: original.id.clone(),
            },
        )
        .unwrap()
    {
        MemoryResult::History(h) => h,
        _ => unreachable!(),
    };
    assert!(matches!(history[0].editor, MemoryActor::Agent { .. }));
    assert!(engine
        .review_agent_step(&run.id, &run.steps[4].id, true)
        .is_err());
    assert_eq!(count(&engine, "shared_memories"), 2);
    drop(engine);
    let reopened = open(tmp.path());
    assert_eq!(
        reopened.agent_run(&run.id).unwrap().steps[4].state,
        "applied"
    );
}

#[tokio::test]
async fn stale_memory_and_source_changes_cannot_consume_review_receipts() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Scoped evidence", None).unwrap();
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Evidence note"),
            "Original evidence.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &page.id, "user_explicit", None)
        .unwrap();
    let (url,server)=fixture(vec![json!({"type":"read_page","artifact_id":page.id}),json!({"type":"propose_memory_create","scope":"project","title":"Evidence memory","body":"Original evidence only.","source_ids":[page.id]}),json!({"type":"propose_derive","artifact_id":page.id,"title":"Evidence checklist","content_md":"Verify original evidence.","derivation_type":"extract_tasks"}),json!({"type":"finish","summary":"Awaiting review.","handoff":"Check evidence."})]).await;
    configure(&engine, &url);
    let run = engine.create_agent_run(goal(Some(&project.id))).unwrap();
    let run = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    server.await.unwrap();
    engine.db().writer().execute("UPDATE artifacts SET content_md='Changed evidence.',version=version+1,content_hash='changed' WHERE id=?1",[&page.id]).unwrap();
    assert!(engine
        .review_agent_step(&run.id, &run.steps[1].id, true)
        .is_err());
    assert!(engine
        .review_agent_step(&run.id, &run.steps[2].id, true)
        .is_err());
    assert_eq!(count(&engine, "shared_memories"), 0);
    assert_eq!(count(&engine, "artifacts"), 1);
    let saved = engine.agent_run(&run.id).unwrap();
    assert_eq!(saved.steps[1].state, "pending");
    assert_eq!(saved.steps[2].state, "pending");
    engine
        .review_agent_step(&run.id, &run.steps[1].id, false)
        .unwrap();
    assert_eq!(count(&engine, "shared_memories"), 0);
}

#[tokio::test]
async fn document_grants_cannot_be_expanded_by_workflow_tool_arguments() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Private document"),
            "Do not change this.",
            None,
        )
        .unwrap();
    let (url,server)=fixture(vec![json!({"type":"propose_refine","artifact_id":page.id,"content_md":"Forged refinement."}),json!({"type":"propose_titles","artifact_id":page.id,"titles":["Forged new title"]}),json!({"type":"propose_derive","artifact_id":page.id,"title":"Forbidden new page","content_md":"Outside document grant.","derivation_type":"extract_tasks"}),json!({"type":"propose_memory_create","scope":"session","title":"Forbidden memory","body":"Outside document grant.","source_ids":[]}),json!({"type":"finish","summary":"Read-only answer.","handoff":"No changes."})]).await;
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
            artifact_id: page.id.clone(),
            text: "Read only".into(),
            model_id: "fixture-model".into(),
            propose_edit: false,
        })
        .unwrap();
    let run = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    server.await.unwrap();
    for step in &run.steps[..4] {
        assert!(step.result.get("error").is_some());
        assert_eq!(step.state, "done");
    }
    assert_eq!(run.status, "completed");
    assert_eq!(count(&engine, "artifacts"), 1);
    assert_eq!(count(&engine, "shared_memories"), 0);
    assert_eq!(
        engine.get_artifact(&page.id).unwrap().content_md,
        page.content_md
    );
}

#[tokio::test]
async fn workflow_review_rolls_back_receipts_on_domain_failure_and_consumes_derive_once() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Original research title"),
            "Original research content.",
            None,
        )
        .unwrap();
    let (url,server)=fixture(vec![
        json!({"type":"read_page","artifact_id":page.id}),
        json!({"type":"propose_refine","artifact_id":page.id,"content_md":"Refined research content."}),
        json!({"type":"propose_derive","artifact_id":page.id,"title":"Research execution plan","content_md":"Validate evidence.","derivation_type":"plan_from_research"}),
        json!({"type":"propose_titles","artifact_id":page.id,"titles":["Unresolved research questions"]}),
        json!({"type":"propose_memory_create","scope":"session","title":"Open research question","body":"Evidence still needs verification.","source_ids":[page.id]}),
        json!({"type":"finish","summary":"Review the proposals.","handoff":"Keep uncertainty."}),
    ]).await;
    configure(&engine, &url);
    let run = engine.create_agent_run(goal(None)).unwrap();
    let run = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    server.await.unwrap();
    engine.db().writer().execute_batch("CREATE TRIGGER reject_workflow_revision BEFORE INSERT ON artifact_revisions BEGIN SELECT RAISE(ABORT,'fixture document failure'); END; CREATE TRIGGER reject_memory_revision BEFORE INSERT ON shared_memory_revisions BEGIN SELECT RAISE(ABORT,'fixture memory failure'); END;").unwrap();
    for step in &run.steps[1..5] {
        assert!(engine
            .review_agent_step_with_title(
                &run.id,
                &step.id,
                true,
                Some("Unresolved research questions")
            )
            .is_err());
        assert_eq!(
            engine.agent_run(&run.id).unwrap().steps[step.sequence - 1].state,
            "pending"
        );
    }
    assert_eq!(
        engine.get_artifact(&page.id).unwrap().content_md,
        page.content_md
    );
    assert_eq!(count(&engine, "shared_memories"), 0);
    assert_eq!(count(&engine, "artifacts"), 1);
    engine
        .db()
        .writer()
        .execute_batch(
            "DROP TRIGGER reject_workflow_revision; DROP TRIGGER reject_memory_revision;",
        )
        .unwrap();
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let engine = engine.clone();
            let step = run.steps[2].id.clone();
            let run = run.id.clone();
            std::thread::spawn(move || engine.review_agent_step(&run, &step, true).is_ok())
        })
        .collect();
    assert_eq!(
        workers
            .into_iter()
            .filter_map(|w| w.join().ok())
            .filter(|applied| *applied)
            .count(),
        1
    );
    assert_eq!(count(&engine, "artifacts"), 2);
}

#[path = "support/agent_organization.rs"]
mod organization_tests;

#[path = "support/agent_projects.rs"]
mod project_tests;

#[path = "support/agent_handoffs.rs"]
mod handoff_tests;
