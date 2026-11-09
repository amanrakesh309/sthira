//! Offline HTTP adapter tests. These do not claim live-provider inference.
use std::sync::Arc;
use sthira_core::{
    engines::streaming_engine::execute_adaptive_stream, llm::ProgressiveStreamChunk,
    models::router::OperationalMode, Engine,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn run_fixture(
    mode: OperationalMode,
    status: &str,
    body: String,
) -> (Result<(), String>, String, bool) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let status = status.to_string();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            let count = socket.read(&mut buffer).await.unwrap();
            if count == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..count]);
            if let Some(end) = request.windows(4).position(|v| v == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]);
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap())
                    })
                    .unwrap_or(0);
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        let request = String::from_utf8(request).unwrap();
        assert!(
            request.starts_with("POST /v1/chat/completions "),
            "{request}"
        );
        assert!(request.contains("selected-model-identifier"));
        let response=format!("HTTP/1.1 {status}\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
        socket.write_all(response.as_bytes()).await.unwrap();
    });
    let tmp = tempfile::tempdir().unwrap();
    let engine = Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path().join("vault"),
        Arc::new(FakeClock::new(1000)),
        Arc::new(DeterministicFakeModel::new("test-unused-fallback")),
    )
    .unwrap();
    {
        let writer = engine.db().writer();
        writer
            .execute_batch("DELETE FROM provider_models; DELETE FROM providers;")
            .unwrap();
        writer.execute("INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) VALUES('fixture','HTTP fixture','openai_compatible',?1,1,0)",[format!("http://{address}/v1")]).unwrap();
        writer.execute_batch("INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) VALUES('selected-id','fixture','selected-model-identifier','Selected model',1,1,0);").unwrap();
    }
    let (tx, mut rx) = tokio::sync::mpsc::channel(128);
    let result = execute_adaptive_stream(
        &engine,
        "Plan a community garden",
        mode,
        Some("selected-id".into()),
        tx,
    )
    .await;
    let mut text = String::new();
    let mut completed = false;
    while let Some(chunk) = rx.recv().await {
        match chunk {
            ProgressiveStreamChunk::TextToken(token) => text.push_str(&token),
            ProgressiveStreamChunk::Complete { .. } => completed = true,
            _ => {}
        }
    }
    server.await.unwrap();
    (result, text, completed)
}
fn event(text: &str) -> String {
    format!(
        "data: {}\n\n",
        serde_json::json!({"choices":[{"delta":{"content":text}}]})
    )
}
#[tokio::test]
async fn selected_model_plan_streams_actual_http_content() {
    let expected="# Garden plan\n\n## First phase\nChoose a shared plot and agree on watering responsibilities.\n";
    let (result, text, complete) = run_fixture(
        OperationalMode::Plan,
        "200 OK",
        event(expected) + "data: [DONE]\n\n",
    )
    .await;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(text, expected);
    assert!(complete);
}
#[tokio::test]
async fn selected_model_research_streams_actual_http_content() {
    let expected =
        "# Garden research\n\nCompare local sunlight and soil conditions before choosing crops.\n";
    let (result, text, complete) = run_fixture(
        OperationalMode::Research,
        "200 OK",
        event(expected) + "data: [DONE]\n\n",
    )
    .await;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(text, expected);
    assert!(complete);
}
#[tokio::test]
async fn provider_failure_never_substitutes_a_simulated_document() {
    let (result, text, complete) = run_fixture(
        OperationalMode::Plan,
        "503 Service Unavailable",
        "unavailable".into(),
    )
    .await;
    assert!(result.unwrap_err().contains("503"));
    assert!(text.is_empty());
    assert!(!complete);
}
#[tokio::test]
async fn interrupted_response_never_reports_completion() {
    let (result, _text, complete) =
        run_fixture(OperationalMode::Plan, "200 OK", event("Partial content")).await;
    assert!(result.is_err());
    assert!(!complete);
}
#[tokio::test]
async fn missing_selected_model_does_not_dispatch_a_default_or_simulated_response() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path().join("vault"),
        Arc::new(FakeClock::new(1000)),
        Arc::new(DeterministicFakeModel::new("test-unused")),
    )
    .unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(128);
    assert!(execute_adaptive_stream(
        &engine,
        "Goal",
        OperationalMode::Plan,
        Some("missing-model".into()),
        tx
    )
    .await
    .is_err());
    assert!(rx.recv().await.is_none());
}
