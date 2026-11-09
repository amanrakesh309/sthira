//! Model selection and reply attribution using isolated SQLite vaults and a loopback provider.
use std::sync::Arc;
use sthira_core::{ArtifactType, Authorship, Engine};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn switching_models_preserves_history_and_snapshots_the_actual_reply_model() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (resume_tx, resume_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut started_tx = Some(started_tx);
        let mut resume_rx = Some(resume_rx);
        for index in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            let header_end = loop {
                let count = socket.read(&mut buffer).await.unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&buffer[..count]);
                if let Some(end) = bytes.windows(4).position(|value| value == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let length: usize = headers
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(|value| value.trim().parse().unwrap())
                        })
                        .unwrap();
                    if bytes.len() >= end + 4 + length {
                        break end;
                    }
                }
            };
            let body: serde_json::Value = serde_json::from_slice(&bytes[header_end + 4..]).unwrap();
            let prompt: serde_json::Value =
                serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
            assert!(prompt["document"]
                .as_str()
                .unwrap()
                .contains("Original saved document"));
            let expected = if index == 0 {
                "identifier-a"
            } else {
                "identifier-b"
            };
            assert_eq!(body["model"], expected);
            assert_eq!(prompt["conversation"].as_array().unwrap().len(), index);
            if index == 1 {
                assert_eq!(
                    prompt["conversation"][0]["assistant"],
                    "Reply from identifier-a"
                );
                started_tx.take().unwrap().send(()).unwrap();
                resume_rx.take().unwrap().await.unwrap();
            }
            let output = serde_json::json!({"reply":format!("Reply from {expected}"),"proposed_content_md":null}).to_string();
            let response =
                serde_json::json!({"choices":[{"message":{"content":output}}]}).to_string();
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",response.len()).as_bytes()).await.unwrap();
        }
    });
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("sthira.db");
    let vault = tmp.path().join("vault");
    let clock = Arc::new(FakeClock::new(1000));
    let model = Arc::new(DeterministicFakeModel::new("test-unused"));
    let engine = Arc::new(Engine::open(&db, &vault, clock.clone(), model.clone()).unwrap());
    {
        let writer = engine.db().writer();
        writer
            .execute_batch("DELETE FROM provider_models; DELETE FROM providers;")
            .unwrap();
        for id in ["a", "b"] {
            writer.execute("INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) VALUES(?1,?2,'openai_compatible',?3,1,0)",rusqlite::params![format!("provider-{id}"),format!("Provider {id}"),format!("http://{address}/v1")]).unwrap();
            writer.execute("INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) VALUES(?1,?2,?3,?4,1,?5,0)",rusqlite::params![format!("model-{id}"),format!("provider-{id}"),format!("identifier-{id}"),format!("Model {id}"),id=="a"]).unwrap();
        }
    }
    let artifact = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Plan"),
            "Original saved document",
            None,
        )
        .unwrap();
    let other = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Other"),
            "Other saved document",
            None,
        )
        .unwrap();
    engine
        .document_conversation(&artifact.id, Some("plan"), "Original goal")
        .unwrap();
    engine
        .document_conversation(&other.id, Some("document"), "")
        .unwrap();
    engine
        .select_conversation_model(&artifact.id, "model-a")
        .unwrap();
    let first = engine
        .discuss_document(&artifact.id, "First question", None, false)
        .await
        .unwrap();
    assert_eq!(first.turns[0].model_id.as_deref(), Some("model-a"));
    assert_eq!(first.turns[0].model_name.as_deref(), Some("Model a"));
    assert_eq!(first.turns[0].provider_name.as_deref(), Some("Provider a"));
    let switched = engine
        .select_conversation_model(&artifact.id, "model-b")
        .unwrap();
    assert_eq!(switched.turns[0].reply, first.turns[0].reply);
    assert_eq!(switched.turns[0].model_name.as_deref(), Some("Model a"));
    assert_eq!(switched.model_id.as_deref(), Some("model-b"));
    assert!(engine
        .document_conversation(&other.id, None, "")
        .unwrap()
        .unwrap()
        .model_id
        .is_none());
    // A second window changes the preference during inference; the reply still belongs to B.
    let running_engine = Arc::clone(&engine);
    let id = artifact.id.clone();
    let request = tokio::spawn(async move {
        running_engine
            .discuss_document(&id, "Follow up", None, false)
            .await
    });
    started_rx.await.unwrap();
    engine
        .select_conversation_model(&artifact.id, "model-a")
        .unwrap();
    resume_tx.send(()).unwrap();
    let second = request.await.unwrap().unwrap();
    server.await.unwrap();
    assert_eq!(second.model_id.as_deref(), Some("model-a"));
    assert_eq!(second.turns[1].model_id.as_deref(), Some("model-b"));
    assert_eq!(second.turns[1].model_name.as_deref(), Some("Model b"));
    assert_eq!(second.turns[1].provider_name.as_deref(), Some("Provider b"));
    engine.db().writer().execute_batch("UPDATE provider_models SET display_name='Renamed model' WHERE id='model-a'; UPDATE providers SET display_name='Renamed provider' WHERE id='provider-a';").unwrap();
    assert!(engine
        .select_conversation_model(&artifact.id, "missing-model")
        .is_err());
    engine
        .db()
        .writer()
        .execute_batch("UPDATE providers SET is_enabled=0 WHERE id='provider-b';")
        .unwrap();
    assert!(engine
        .select_conversation_model(&artifact.id, "model-b")
        .is_err());
    assert_eq!(
        engine
            .document_conversation(&artifact.id, None, "")
            .unwrap()
            .unwrap()
            .model_id
            .as_deref(),
        Some("model-a")
    );
    engine
        .db()
        .writer()
        .execute_batch("UPDATE providers SET is_enabled=1 WHERE id='provider-b';")
        .unwrap();
    engine
        .select_conversation_model(&artifact.id, "model-b")
        .unwrap();
    let default: String = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT id FROM provider_models WHERE is_default=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(default, "model-a");
    assert_eq!(
        engine.get_artifact(&artifact.id).unwrap().content_md,
        "Original saved document"
    );
    drop(engine);
    let reopened = Engine::open(&db, &vault, clock, model).unwrap();
    let chat = reopened
        .document_conversation(&artifact.id, None, "")
        .unwrap()
        .unwrap();
    assert_eq!(chat.model_id.as_deref(), Some("model-b"));
    assert_eq!(chat.turns.len(), 2);
    assert_eq!(chat.turns[0].model_name.as_deref(), Some("Model a"));
    assert_eq!(chat.turns[0].provider_name.as_deref(), Some("Provider a"));
    assert_eq!(chat.turns[1].model_name.as_deref(), Some("Model b"));
}
