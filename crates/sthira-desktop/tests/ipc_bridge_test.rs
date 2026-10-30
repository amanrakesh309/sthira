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

    // 13. cmd_synthesize_and_create_research
    let res_proj = handle_synthesize_and_create_research(
        &engine,
        "Local specialized model for Kotlin and Java".to_string(),
        None,
    )
    .await
    .expect("handle_synthesize_and_create_research failed");
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
