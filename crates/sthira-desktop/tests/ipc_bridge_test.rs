use std::sync::Arc;
use sthira_core::{
    router::IntentType,
    Clock, Engine,
};
use sthira_desktop::{
    handle_apply_plan, handle_canvas_ops, handle_decide_proposal, handle_get_artifact_context,
    handle_get_home_state, handle_get_project_history, handle_get_resume_brief,
    handle_get_resume_pane_hint, handle_promote_to_project, handle_revert_change_set,
    handle_route_input, handle_save_artifact, seed_demo_workspace_if_empty, CanvasItemPosition,
    CanvasOpsRequest, handle_synthesize_and_create_research, handle_answer_question,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

#[tokio::test]
async fn test_ipc_bridge_all_12_commands_roundtrip() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    // Seed workspace
    seed_demo_workspace_if_empty(&engine).unwrap();

    // 1. cmd_get_home_state
    let home_state = handle_get_home_state(&engine).expect("handle_get_home_state failed");
    assert!(!home_state.projects.is_empty(), "Expected seeded projects");
    assert!(
        home_state.projects.iter().any(|p| p.name == "Local AI research"),
        "Local AI research project missing"
    );
    assert!(
        home_state.projects.iter().any(|p| p.name == "Webhook platform"),
        "Webhook platform project missing"
    );
    assert!(!home_state.recent_items.is_empty(), "Recent items missing");
    let json_home = serde_json::to_string(&home_state).expect("Failed to serialize HomeStateView");
    assert!(json_home.contains("Local AI research"));

    let local_ai_proj = home_state
        .projects
        .iter()
        .find(|p| p.name == "Local AI research")
        .unwrap();

    // 2. cmd_route_input
    let route_plan = handle_route_input(&engine, "save this thought: test fast note".to_string())
        .await
        .expect("handle_route_input failed");
    assert_eq!(route_plan.intent_type, IntentType::CaptureNote);
    let json_plan = serde_json::to_string(&route_plan).expect("Failed to serialize RouterPlan");
    assert!(json_plan.contains("capture_note"));

    // 3. cmd_apply_plan
    let apply_outcome = handle_apply_plan(&engine, route_plan, true).expect("handle_apply_plan failed");
    assert!(apply_outcome.success);
    let created_art_id = apply_outcome.created_id.expect("Created artifact ID missing");

    // 4. cmd_save_artifact
    let updated_art = handle_save_artifact(
        &engine,
        created_art_id.clone(),
        "# Updated note title\n\nNew verified body content.".to_string(),
        None,
        Some(local_ai_proj.id.clone()),
    )
    .expect("handle_save_artifact update failed");
    assert_eq!(updated_art.id, created_art_id);

    // Save a new artifact from scratch
    let new_art = handle_save_artifact(
        &engine,
        "".to_string(),
        "# Brand new artifact\n\nFresh thoughts.".to_string(),
        None,
        None,
    )
    .expect("handle_save_artifact create failed");
    assert!(!new_art.id.is_empty());

    // 5. cmd_get_artifact_context
    let art_context = handle_get_artifact_context(&engine, created_art_id.clone())
        .expect("handle_get_artifact_context failed");
    assert_eq!(art_context.artifact.id, created_art_id);
    let json_context = serde_json::to_string(&art_context).expect("Serialize ArtifactContext");
    assert!(json_context.contains("Updated note title"));

    // 6. cmd_get_resume_pane_hint
    let pane_hint = handle_get_resume_pane_hint(&engine, local_ai_proj.id.clone())
        .expect("handle_get_resume_pane_hint failed");
    let json_hint = serde_json::to_string(&pane_hint).expect("Serialize ResumePaneHint");
    assert!(!json_hint.is_empty());

    // 7. cmd_get_resume_brief
    let brief = handle_get_resume_brief(&engine, local_ai_proj.id.clone(), 1)
        .await
        .expect("handle_get_resume_brief failed");
    let json_brief = serde_json::to_string(&brief).expect("Serialize ResumeBrief");
    assert!(json_brief.contains(&local_ai_proj.id));

    // 8. cmd_get_project_history
    let history = handle_get_project_history(&engine, local_ai_proj.id.clone())
        .expect("handle_get_project_history failed");
    let json_history = serde_json::to_string(&history).expect("Serialize ProjectHistory");
    assert!(json_history.contains("milestones"));

    // 9. cmd_promote_to_project
    let promoted_proj_id = handle_promote_to_project(
        &engine,
        vec![new_art.id.clone()],
        "Promoted Research Project".to_string(),
    )
    .expect("handle_promote_to_project failed");
    assert!(!promoted_proj_id.is_empty());

    // 10. cmd_canvas_ops: GetCanvas, MoveItem, AddItem, BatchUpdatePositions
    let canvas_resp = handle_canvas_ops(
        &engine,
        CanvasOpsRequest::GetCanvas {
            canvas_id: None,
            project_id: Some(local_ai_proj.id.clone()),
        },
    )
    .expect("handle_canvas_ops GetCanvas failed");
    assert!(canvas_resp.success);
    let canvas = canvas_resp.canvas.expect("Canvas data missing");
    let cid = canvas.id.clone();

    // Add item
    let add_resp = handle_canvas_ops(
        &engine,
        CanvasOpsRequest::AddItem {
            canvas_id: cid.clone(),
            item_kind: "text".to_string(),
            artifact_id: None,
            x: 200.0,
            y: 300.0,
            w: Some(150.0),
            h: Some(80.0),
        },
    )
    .expect("handle_canvas_ops AddItem failed");
    assert!(add_resp.success);

    // Batch update positions with change_set_id
    let batch_resp = handle_canvas_ops(
        &engine,
        CanvasOpsRequest::BatchUpdatePositions {
            canvas_id: cid.clone(),
            positions: vec![CanvasItemPosition {
                item_id: "test-item-1".to_string(),
                x: 250.0,
                y: 350.0,
            }],
        },
    )
    .expect("handle_canvas_ops BatchUpdatePositions failed");
    assert!(batch_resp.success);
    let cs_id = batch_resp.change_set_id.expect("ChangeSetId should be produced");

    // 11. cmd_decide_proposal (simulate dismiss)
    let prop_id = uuid::Uuid::new_v4().to_string();
    {
        let writer = engine.db().writer();
        let now_ms = clock.now_ms();
        writer
            .execute(
                "INSERT INTO ai_proposals (id, kind, subject_key, payload, evidence, model_id, state, created_at)
                 VALUES (?1, 'contradiction', 'art_a|art_b', '{\"source_artifact_id\":\"a\",\"target_artifact_id\":\"b\"}', '{\"reason\":\"test\"}', 'test-model', 'pending', ?2);",
                rusqlite::params![prop_id, now_ms],
            )
            .unwrap();
    }
    let dismissed_id = handle_decide_proposal(&engine, prop_id.clone(), "dismiss".to_string())
        .expect("handle_decide_proposal dismiss failed");
    assert_eq!(dismissed_id, prop_id);

    // 12. cmd_revert_change_set
    let revert_cs_id = handle_revert_change_set(&engine, cs_id)
        .expect("handle_revert_change_set failed");
    assert!(!revert_cs_id.is_empty());

    // 13. Native synthesis uses a deterministic localhost HTTP fixture, never the
    // owner's Ollama endpoint seeded into the provider catalog.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            let n = socket.read(&mut buffer).await.unwrap();
            request.extend_from_slice(&buffer[..n]);
            if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]);
                let length: usize = headers.lines().find_map(|line| line.to_lowercase().strip_prefix("content-length:").map(|value| value.trim().parse().unwrap())).unwrap_or(0);
                if request.len() >= end + 4 + length { break; }
            }
            if n == 0 { break; }
        }
        let content = serde_json::json!({"title":"Research fixture", "purpose":"Evaluate saved research", "document_title":"Research document", "sections":[{"heading":"Findings", "content":"Synthetic model response for IPC testing."}], "hypotheses":[], "open_questions":[]}).to_string();
        let body = serde_json::json!({"choices":[{"message":{"content":content}}]}).to_string();
        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
    });
    {
        let writer = engine.db().writer();
        writer.execute_batch("UPDATE provider_models SET is_default=0;").unwrap();
        writer.execute("INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) VALUES('ipc-fixture','IPC fixture','openai_compatible',?1,1,0)",[format!("http://{address}/v1")]).unwrap();
        writer.execute_batch("INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) VALUES('ipc-model','ipc-fixture','test','Test model',1,1,0);").unwrap();
    }
    sthira_desktop::workflow_settings::save_workflow_setting(&engine, "research", sthira_desktop::workflow_settings::WorkflowSetting { enabled: true, ..Default::default() }).unwrap();
    let res_proj = handle_synthesize_and_create_research(
        &engine,
        "Local specialized model for Kotlin and Java".to_string(),
        None,
    )
    .await
    .expect("handle_synthesize_and_create_research failed");
    server.await.unwrap();
    assert!(!res_proj.project_id.is_empty());
    assert!(!res_proj.artifact_id.is_empty());

    // 14. cmd_answer_question
    let answer = handle_answer_question(
        &engine,
        "How do we create a local model?".to_string(),
    )
    .await
    .expect("handle_answer_question failed");
    assert!(!answer.is_empty());
}

#[tokio::test]
async fn test_native_persistence_and_restart_recovery() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("restart_test.db");
    let vault_dir = tmp.path().join("restart_vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let note_id;
    let proj_id;
    let canvas_id;
    let tracker_id;

    // Session 1: Create entities in first Engine instance
    {
        let engine1 = Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap();
        let proj = engine1.create_project("Restart Verification Project", None).unwrap();
        proj_id = proj.id;
        engine1.set_project_purpose(&proj_id, "Verify that native SQLite & disk recovery survives engine restart").unwrap();

        let note = engine1.create_artifact(
            sthira_core::ArtifactType::Note,
            sthira_core::Authorship::User,
            Some("Persistent Architecture Note"),
            "# Persistent Architecture Note\n\nThis content was created in Session 1.",
            None,
        ).unwrap();
        note_id = note.id;

        engine1.attach_artifact(&proj_id, &note_id, "user_explicit", None).unwrap();

        let canvas = engine1.create_canvas(Some(&proj_id), "Architecture Map").unwrap();
        canvas_id = canvas;

        engine1.add_canvas_item(
            &canvas_id,
            "item-1",
            sthira_core::CanvasItemKind::Artifact,
            Some(&note_id),
            120.0,
            240.0,
            Some(280.0),
            Some(140.0),
            1,
        ).unwrap();

        engine1.move_canvas_item(&canvas_id, "item-1", 150.0, 300.0).unwrap();

        // Add a scheduled topic / tracker
        tracker_id = sthira_desktop::handle_create_scheduled_topic(
            &engine1,
            sthira_desktop::CreateScheduledTopicInput {
                title: "Daily Storage Benchmark".into(),
                prompt_query: "Track disk throughput across write loads".into(),
                topic_type: "research".into(),
                project_id: Some(proj_id.clone()),
                cadence: Some("daily".into()),
                model_id: None,
                search_provider_id: None,
                schedule_time: Some("09:00".into()),
                schedule_weekday: None,
            },
        ).unwrap();

        // engine1 is dropped here when exiting scope, simulating engine shutdown
    }

    // Session 2: Open a completely fresh Engine instance pointing at the exact same db and vault
    {
        let engine2 = Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap();

        // 1. Verify project survived
        let recovered_proj = engine2.get_project(&proj_id).unwrap();
        assert_eq!(recovered_proj.name, "Restart Verification Project");

        // 2. Verify note survived in SQLite
        let recovered_note = engine2.get_artifact(&note_id).unwrap();
        assert_eq!(recovered_note.title.as_deref(), Some("Persistent Architecture Note"));
        assert!(recovered_note.content_md.contains("This content was created in Session 1."));

        // 3. Verify raw Markdown file exists on physical disk in vault_dir
        let mut raw_found = false;
        if let Ok(entries) = std::fs::read_dir(&vault_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().is_some_and(|e| e == "md") {
                    if let Ok(raw) = std::fs::read_to_string(entry.path()) {
                        if raw.contains("This content was created in Session 1.") {
                            raw_found = true;
                            break;
                        }
                    }
                }
            }
        }
        assert!(raw_found, "Raw Markdown file must be physically preserved on disk in vault");

        // 4. Verify exact recovered geometry of canvas item (x, y, w, h, z)
        let canvas_res = sthira_desktop::handle_canvas_ops(
            &engine2,
            sthira_desktop::CanvasOpsRequest::GetCanvas {
                canvas_id: Some(canvas_id.clone()),
                project_id: None,
            },
        ).unwrap();
        let canvas_view = canvas_res.canvas.expect("Canvas must exist after restart");
        let item = canvas_view.items.iter().find(|i| i.item_id == "item-1").expect("item-1 must survive");
        assert_eq!(item.x, 150.0);
        assert_eq!(item.y, 300.0);
        assert_eq!(item.w, Some(280.0));
        assert_eq!(item.h, Some(140.0));
        assert_eq!(item.z, 1);

        // 5. Verify scheduled tracker survived across restart
        let topics = sthira_desktop::handle_list_scheduled_topics(&engine2, None).unwrap();
        let topic = topics.iter().find(|t| t.id == tracker_id).expect("Scheduled tracker must survive");
        assert_eq!(topic.title, "Daily Storage Benchmark");
        assert_eq!(topic.cadence, "daily");
        assert!(topic.is_active);

        // 6. Verify project membership and purpose survived across restart
        let overview = sthira_desktop::handle_get_project_overview(&engine2, proj_id.clone()).unwrap();
        assert_eq!(overview.project.purpose, "Verify that native SQLite & disk recovery survives engine restart");
        assert!(overview.items.iter().any(|item| item.id == note_id && item.roles.contains(&"member".to_string())));
        assert!(overview.items.iter().any(|item| item.id == canvas_id && item.kind == "canvas"));
    }
}
