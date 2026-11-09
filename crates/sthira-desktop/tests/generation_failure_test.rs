//! IPC handler integration with an isolated vault and local HTTP fixture, not live inference.
use std::sync::Arc;
use sthira_core::{ArtifactType, Authorship, Engine};
use sthira_desktop::{handle_run_research_into_page, handle_save_generated_artifact};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn research_uses_selected_provider_and_preserves_draft_on_model_failure() {
    for succeeds in [true, false] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let n = socket.read(&mut buffer).await.unwrap();
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let len: usize = headers
                        .lines()
                        .find_map(|s| {
                            s.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse().unwrap())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let request = String::from_utf8(bytes).unwrap();
            assert!(request.starts_with("POST /v1/chat/completions "));
            assert!(request.contains("chosen-research-model"));
            assert!(request.contains("Garden planning"));
            let synthesis=serde_json::json!({"synthesis":"Selected model: compare sunlight, soil and a shared watering schedule.","contradictions_and_evolution":[],"open_questions":["Which plot is available?"]}).to_string();
            let body = if succeeds {
                serde_json::json!({"choices":[{"message":{"content":synthesis}}]}).to_string()
            } else {
                "unavailable".into()
            };
            let status = if succeeds {
                "200 OK"
            } else {
                "503 Service Unavailable"
            };
            socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
        });
        let tmp = tempfile::tempdir().unwrap();
        let engine = Engine::open(
            tmp.path().join("sthira.db"),
            tmp.path().join("vault"),
            Arc::new(FakeClock::new(1000)),
            Arc::new(DeterministicFakeModel::new("test-unselected")),
        )
        .unwrap();
        {
            let writer = engine.db().writer();
            writer
                .execute_batch("DELETE FROM provider_models; DELETE FROM providers;")
                .unwrap();
            writer.execute("INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) VALUES('fixture','Research HTTP','openai_compatible',?1,1,0)",[format!("http://{address}/v1")]).unwrap();
            writer.execute_batch("INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) VALUES('chosen-id','fixture','chosen-research-model','Chosen model',1,1,0);").unwrap();
        }
        let artifact = engine
            .create_artifact(
                ArtifactType::Note,
                Authorship::User,
                Some("Garden planning"),
                "# Garden planning\n\nOriginal goal stays here.",
                None,
            )
            .unwrap();
    sthira_desktop::workflow_settings::save_workflow_setting(&engine, "research", sthira_desktop::workflow_settings::WorkflowSetting { enabled: true, ..Default::default() }).unwrap();
        let result = handle_run_research_into_page(
            &engine,
            artifact.id.clone(),
            "Garden planning".into(),
            Some("local_vault_only".into()),
            None,
            Some("chosen-id".into()),
            tauri::ipc::Channel::new(|_| Ok(())),
        )
        .await;
        server.await.unwrap();
        let after = engine.get_artifact(&artifact.id).unwrap();
        if succeeds {
            assert!(result
                .unwrap()
                .synthesized_markdown
                .contains("Selected model: compare sunlight"));
            assert!(after
                .content_md
                .contains("Selected model: compare sunlight"));
            assert_eq!(after.authorship, Authorship::AiGenerated);
            let actor:String=engine.db().reader().unwrap().query_row("SELECT actor FROM artifact_revisions WHERE artifact_id=?1 ORDER BY version DESC LIMIT 1",[&artifact.id],|r|r.get(0)).unwrap();
            assert_eq!(actor, "ai");
        } else {
            assert!(result.is_err());
            assert_eq!(after.content_md, artifact.content_md);
        }
    }
}

#[test]
fn empty_model_output_does_not_replace_a_saved_document() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path().join("vault"),
        Arc::new(FakeClock::new(1000)),
        Arc::new(DeterministicFakeModel::new("test-unused")),
    )
    .unwrap();
    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Goal"),
            "Keep this draft",
            None,
        )
        .unwrap();
    assert!(handle_save_generated_artifact(
        &engine,
        art.id.clone(),
        "".into(),
        "Goal".into(),
        None
    )
    .is_err());
    assert_eq!(
        engine.get_artifact(&art.id).unwrap().content_md,
        "Keep this draft"
    );
}
