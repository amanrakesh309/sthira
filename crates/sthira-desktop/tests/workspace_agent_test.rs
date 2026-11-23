use std::sync::Arc;
use sthira_core::{workspace_agent::AgentGoal, Engine};
use sthira_desktop::{
    commands::handle_clear_vault_data,
    state::RequestRegistry,
    workflow_settings::{save_workflow_setting, WorkflowSetting},
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
#[test]
fn agent_workflow_is_opt_in_and_reset_removes_saved_runs() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path().join("vault"),
        Arc::new(FakeClock::new(1000)),
        Arc::new(DeterministicFakeModel::new("unused")),
    )
    .unwrap();
    engine.db().writer().execute_batch("INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) VALUES('fixture','Fixture','openai_compatible','http://127.0.0.1:1/v1',1,0); INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) VALUES('model','fixture','fixture-model','Fixture model',1,1,0);").unwrap();
    let goal = || AgentGoal {
        goal: "A saved goal".into(),
        project_id: None,
        workspace_memory: false,
        model_id: "model".into(),
    };
    assert!(engine.create_agent_run(goal()).is_err());
    save_workflow_setting(
        &engine,
        "agent",
        WorkflowSetting {
            enabled: true,
            model_id: Some("model".into()),
            templates_enabled: false,
        },
    )
    .unwrap();
    let run = engine.create_agent_run(goal()).unwrap();
    assert_eq!(run.status, "queued");
    handle_clear_vault_data(&engine).unwrap();
    assert!(engine.agent_run(&run.id).is_err());
    assert!(engine.list_agent_runs().unwrap().is_empty());
}
#[test]
fn duplicate_dispatch_cannot_replace_the_cancellation_token() {
    let registry = RequestRegistry::new();
    let token = registry.register_unique("agent-run").unwrap();
    assert!(registry.register_unique("agent-run").is_none());
    assert!(registry.cancel("agent-run"));
    assert!(token.is_cancelled());
    registry.unregister("agent-run");
    assert!(registry.register_unique("agent-run").is_some());
}

use sthira_core::{workspace_agent::DocumentAgentGoal, ArtifactType, Authorship, CoreEvent};
use sthira_desktop::{workspace_agent::start_agent_task, AppState};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
fn background_fixture() -> (tempfile::TempDir, Arc<Engine>) {
    let tmp = tempfile::tempdir().unwrap();
    let engine = Arc::new(
        Engine::open(
            tmp.path().join("sthira.db"),
            tmp.path().join("vault"),
            Arc::new(FakeClock::new(1000)),
            Arc::new(DeterministicFakeModel::new("unused")),
        )
        .unwrap(),
    );
    (tmp, engine)
}
async fn delayed_provider(
    engine: &Engine,
) -> (
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    engine.db().writer().execute("INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) VALUES('background-fixture','Background fixture','openai_compatible',?1,1,0)",[url]).unwrap();
    engine.db().writer().execute_batch("INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) VALUES('background-model','background-fixture','background-model','Background model',1,1,0); INSERT INTO workflow_settings(workflow,enabled) VALUES('chat',1);").unwrap();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut data = vec![];
        let mut bytes = [0; 4096];
        loop {
            let n = socket.read(&mut bytes).await.unwrap();
            assert!(n > 0);
            data.extend_from_slice(&bytes[..n]);
            if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&data[..end]);
                let length: usize = header
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap())
                    })
                    .unwrap();
                if data.len() >= end + 4 + length {
                    break;
                }
            }
        }
        started_tx.send(()).unwrap();
        release_rx.await.unwrap();
        let output = serde_json::json!({"type":"finish","summary":"Saved answer from the background task.","handoff":"Inspect the unresolved evidence."});
        let body =
            serde_json::json!({"choices":[{"message":{"content":output.to_string()}}]}).to_string();
        let _=socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await;
    });
    (started_rx, release_tx, server)
}
async fn wait_for_stop(
    engine: &Engine,
    id: &str,
    events: &mut tokio::sync::broadcast::Receiver<CoreEvent>,
) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let run = engine.agent_run(id).unwrap();
            if !matches!(run.status.as_str(), "queued" | "running") {
                break;
            }
            events.recv().await.unwrap();
        }
    })
    .await
    .unwrap();
}
async fn wait_for_cleanup(state: &AppState, id: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while state
            .requests
            .is_registered(&format!("workspace-agent:{id}"))
        {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn background_request_outlives_its_calling_state_and_keeps_the_editor_unchanged() {
    let (_tmp, engine) = background_fixture();
    let (started, release, server) = delayed_provider(&engine).await;
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Working page"),
            "Original saved prose",
            None,
        )
        .unwrap();
    let queued = engine
        .create_document_agent_run(DocumentAgentGoal {
            artifact_id: page.id.clone(),
            text: "Explain this page".into(),
            model_id: "background-model".into(),
            propose_edit: false,
        })
        .unwrap();
    let mut events = engine.subscribe();
    let state = AppState::new(engine.clone());
    let snapshot = start_agent_task(state.clone(), queued.id.clone()).unwrap();
    assert_eq!(snapshot.status, "queued");
    drop(state);
    started.await.unwrap();
    engine
        .update_artifact(
            &page.id,
            "Owner kept writing while the task ran",
            sthira_core::Actor::User,
            None,
        )
        .unwrap();
    release.send(()).unwrap();
    server.await.unwrap();
    wait_for_stop(&engine, &queued.id, &mut events).await;
    let chat = engine
        .document_conversation(&page.id, None, "")
        .unwrap()
        .unwrap();
    assert_eq!(
        chat.turns[0].reply.as_deref(),
        Some("Saved answer from the background task.")
    );
    assert_eq!(
        engine.get_artifact(&page.id).unwrap().content_md,
        "Owner kept writing while the task ran"
    );
    assert_eq!(engine.list_agent_tasks().unwrap()[0].status, "completed");
}
#[tokio::test]
async fn global_stop_cancels_the_owned_worker_before_a_late_provider_response() {
    let (_tmp, engine) = background_fixture();
    let (started, release, server) = delayed_provider(&engine).await;
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Unchanged"),
            "Original",
            None,
        )
        .unwrap();
    let run = engine
        .create_document_agent_run(DocumentAgentGoal {
            artifact_id: page.id.clone(),
            text: "Stop this request".into(),
            model_id: "background-model".into(),
            propose_edit: true,
        })
        .unwrap();
    let state = AppState::new(engine.clone());
    start_agent_task(state.clone(), run.id.clone()).unwrap();
    started.await.unwrap();
    engine.cancel_agent_run(&run.id).unwrap();
    state
        .requests
        .cancel(&format!("workspace-agent:{}", run.id));
    wait_for_cleanup(&state, &run.id).await;
    release.send(()).unwrap();
    server.await.unwrap();
    let saved = engine.agent_run(&run.id).unwrap();
    assert_eq!(saved.status, "cancelled");
    assert!(saved.steps.is_empty());
    let turn = &engine
        .document_conversation(&page.id, None, "")
        .unwrap()
        .unwrap()
        .turns[0];
    assert_eq!(turn.error.as_deref(), Some("Stopped by you."));
    assert!(turn.reply.is_none());
    assert_eq!(
        engine.get_artifact(&page.id).unwrap().content_md,
        page.content_md
    );
}
#[tokio::test]
async fn vault_reset_cancels_workers_and_never_restores_old_tasks_or_conversations() {
    let (_tmp, engine) = background_fixture();
    let (started, release, server) = delayed_provider(&engine).await;
    let page = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Disposable reset fixture"),
            "Original",
            None,
        )
        .unwrap();
    let run = engine
        .create_document_agent_run(DocumentAgentGoal {
            artifact_id: page.id,
            text: "A pending request".into(),
            model_id: "background-model".into(),
            propose_edit: false,
        })
        .unwrap();
    let state = AppState::new(engine.clone());
    start_agent_task(state.clone(), run.id.clone()).unwrap();
    started.await.unwrap();
    state.requests.cancel_agent_tasks();
    handle_clear_vault_data(&engine).unwrap();
    wait_for_cleanup(&state, &run.id).await;
    release.send(()).unwrap();
    server.await.unwrap();
    assert!(engine.list_agent_tasks().unwrap().is_empty());
    assert!(engine.agent_run(&run.id).is_err());
    assert_eq!(
        engine
            .db()
            .reader()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM document_conversation_turns",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}
