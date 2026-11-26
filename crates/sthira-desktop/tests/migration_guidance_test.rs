//! Real SQLite and loopback model transport; not live-provider quality evidence.
use serde_json::{json, Value};
use std::sync::Arc;
use sthira_core::{
    migration::{guidance::*, *},
    Engine,
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
fn preview(engine: &Engine, key: &str, files: Vec<(&str, &str)>) -> ImportRun {
    engine
        .preview_migration(ImportDraft {
            request_id: key.into(),
            source: "obsidian".into(),
            collection: "research".into(),
            project_id: None,
            files: files
                .into_iter()
                .map(|(path, body)| ImportFile {
                    path: path.into(),
                    content: Some(body.into()),
                    problem: None,
                })
                .collect(),
        })
        .unwrap()
}
fn input(engine: &Engine, run: &ImportRun, key: &str, paths: Vec<&str>) -> GuidanceInput {
    let selection = engine
        .review_migration_selection(
            &run.id,
            run.revision,
            paths.into_iter().map(str::to_owned).collect(),
        )
        .unwrap();
    GuidanceInput {
        request_id: key.into(),
        import_id: run.id.clone(),
        revision: run.revision,
        paths: selection.paths,
        basis: selection.basis,
        model_id: "fixture-model".into(),
        instructions: "Suggest cautious mappings.".into(),
        include_excerpts: false,
    }
}
fn advice(path: &str) -> Value {
    json!({"summary":"Review before importing.","mappings":[{"path":path,"choice":"import_as_page","reason":"Preserve the original note as a page.","suggested_group":"Reading"}],"cautions":["Images are not copied."]})
}

use sthira_desktop::{
    commands::handle_clear_vault_data, migration::start_migration_guidance, AppState,
};
#[tokio::test]
async fn migration_worker_survives_the_calling_view_and_reset_clears_saved_guidance() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = Arc::new(open(tmp.path()));
    let (url, server) = fixture(vec![advice("A.md")]).await;
    configure(&engine, &url);
    let run = preview(&engine, "desktop", vec![("A.md", "Preserve the source")]);
    let request = input(&engine, &run, "desktop-guidance", vec!["A.md"]);
    let state = AppState::new(engine.clone());
    let job = start_migration_guidance(state.clone(), request.clone()).unwrap();
    let retried = start_migration_guidance(state.clone(), request).unwrap();
    assert_eq!(retried.id, job.id);
    drop(state);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let value = engine.migration_guidance(&job.id).unwrap();
            if !["queued", "running"].contains(&value.status.as_str()) {
                assert_eq!(value.status, "completed");
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(server.await.unwrap().len(), 1);
    assert_eq!(count(&engine, "artifacts"), 0);
    handle_clear_vault_data(&engine).unwrap();
    assert!(engine.migration_guidance(&job.id).is_err());
    assert!(engine.migration_history(10, None).unwrap().items.is_empty());
}
#[test]
fn migration_guidance_requires_agent_opt_in_and_reset_cancels_its_tokens() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = Arc::new(open(tmp.path()));
    configure(&engine, "http://127.0.0.1:1/v1");
    let run = preview(&engine, "disabled", vec![("A.md", "Text")]);
    let request = input(&engine, &run, "blocked", vec!["A.md"]);
    engine
        .db()
        .writer()
        .execute(
            "UPDATE workflow_settings SET enabled=0 WHERE workflow='agent'",
            [],
        )
        .unwrap();
    let state = AppState::new(engine.clone());
    assert!(start_migration_guidance(state.clone(), request).is_err());
    assert_eq!(count(&engine, "migration_guidance"), 0);
    let token = state
        .requests
        .register_unique("migration-guidance:reset-fixture")
        .unwrap();
    state.requests.cancel_agent_tasks();
    assert!(token.is_cancelled());
}
