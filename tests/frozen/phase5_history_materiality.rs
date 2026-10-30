use std::sync::Arc;
use sthira_core::{
    ArtifactType, Attribution, Authorship, Clock, Engine, Materiality, MilestoneType,
};
use sthira_testkit::{FakeClock, DeterministicFakeModel};
use tempfile::TempDir;

fn setup_test_engine() -> (Engine, Arc<FakeClock>, Arc<DeterministicFakeModel>, TempDir) {
    let dir = TempDir::new().expect("create tempdir failed");
    let db_path = dir.path().join("sthira.db");
    let vault_dir = dir.path().join(".sthira_vault").join("artifacts");
    let clock = Arc::new(FakeClock::new(1000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));
    let engine = Engine::open(&db_path, &vault_dir, clock.clone(), model.clone()).expect("open engine failed");
    (engine, clock, model, dir)
}

#[tokio::test]
async fn test_5_1_four_session_pivot_and_attribution() {
    let (engine, clock, model, _dir) = setup_test_engine();

    // Session 1: Cloud PR bot goal
    let proj = engine
        .create_project("PR Review Assistant", None)
        .expect("create project failed");
    engine
        .set_project_purpose(
            &proj.id,
            "Cloud PR bot to review pull requests using cloud LLMs",
        )
        .expect("set purpose failed");

    let arch_note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Architecture Discussion"),
            "Initial architecture discussion for cloud PR bot.",
            None,
        )
        .expect("create arch note failed");
    engine
        .attach_artifact(&proj.id, &arch_note.id, "user_explicit", None)
        .expect("attach note failed");

    // Session 2: 15 typo edits + 1 privacy note
    clock.advance_ms(sthira_core::IDLE_GAP_MS + 5000);

    let typo_texts = [
        "Initial architecture discussion for clud PR bot.",
        "Initial archtecture discussion for clud PR bot.",
        "Initial archtecture discussion for cloud PR bot.",
        "Initial architecture discusion for cloud PR bot.",
        "Initial architecture discusion for clud PR bot.",
        "Initial architecture discussion for cloud PR bot..",
        "Initial architecture discussion for cloud PR bot...",
        "Initial architecture discussion for cloud PR bot!",
        "Initial architecure discussion for cloud PR bot!",
        "Initial architecure discussion for cloud PR bot.",
        "Initial architecture discussion  for cloud PR bot.",
        "Initial architecture  discussion for cloud PR bot.",
        "Initial architecture discussion for cloud  PR bot.",
        "Initial architecture discusion for cloud PR bot!",
        "Initial architecture discussion for cloud PR bot! ",
    ];

    for (i, typo_content) in typo_texts.iter().enumerate() {
        let updated = engine
            .update_artifact(&arch_note.id, typo_content, sthira_core::Actor::User, Some(&proj.id))
            .expect("typo edit failed");
        let mat = sthira_core::materiality::compute_materiality("Initial architecture discussion for cloud PR bot.", &updated.content_md);
        assert_ne!(
            mat,
            Materiality::Substantive,
            "Edit {} should not be substantive",
            i
        );
    }

    let privacy_note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Enterprise Privacy Mandate"),
            "Due to strict enterprise data privacy rules, no proprietary code may ever leave our infrastructure or be sent to cloud LLM APIs. All analysis must run locally on-premise.",
            None,
        )
        .expect("create privacy note failed");
    engine
        .attach_artifact(&proj.id, &privacy_note.id, "user_explicit", None)
        .expect("attach privacy note failed");

    // Register purpose pivot model response
    let arch_note_latest = engine.get_artifact(&arch_note.id).expect("get arch note failed");
    let pivot_prompt = format!(
        "Original Purpose: Cloud PR bot to review pull requests using cloud LLMs\n\nRecent Notes:\n{}\n\n---\n\n{}",
        arch_note_latest.content_md, privacy_note.content_md
    );
    model.register_response(
        pivot_prompt,
        serde_json::json!({
            "shift_detected": true,
            "inferred_new_purpose": "Local on-premise PR bot ensuring data privacy",
            "explanation": "Strict enterprise privacy rules prohibit external cloud APIs"
        })
        .to_string(),
    );

    let prop_id = engine
        .detect_purpose_pivot(&proj.id)
        .await
        .expect("pivot detection failed")
        .expect("should detect purpose pivot");

    // Session 3: Local model decision
    clock.advance_ms(sthira_core::IDLE_GAP_MS + 5000);

    let dec_note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Deployment Decision"),
            "We decided to deploy local open-weights models on-premise for PR review.",
            None,
        )
        .expect("create dec note failed");
    engine
        .attach_artifact(&proj.id, &dec_note.id, "user_explicit", None)
        .expect("attach dec note failed");

    // Register decision extraction
    let dec_user_prompt = format!(
        "Extract knowledge nodes from this document:\n\n{}",
        dec_note.content_md
    );
    model.register_response(
        dec_user_prompt,
        serde_json::json!({
            "nodes": [
                {
                    "node_type": "decision",
                    "statement": "Decision: deploy local open-weights models",
                    "quote": "We decided to deploy local open-weights models on-premise for PR review."
                }
            ]
        })
        .to_string(),
    );

    let _nodes = engine
        .extract_epistemic_nodes(&dec_note.id)
        .await
        .expect("extract dec nodes failed");

    // Verify history before accepting proposal
    let history_before = engine.get_project_history(&proj.id).expect("get history failed");

    // Assert the 15 typo edits are excluded
    let typo_milestones = history_before
        .milestones
        .iter()
        .filter(|m| m.description.contains("clud") || m.description.contains("architecure") || m.description.contains("discusion"))
        .count();
    assert_eq!(typo_milestones, 0, "15 typo edits must be excluded from history");

    // Assert the implicit privacy pivot is labeled AiInterpretation
    let pivot_milestone = history_before
        .milestones
        .iter()
        .find(|m| m.milestone_type == MilestoneType::PurposePivot)
        .expect("pivot milestone should be present");
    assert_eq!(
        pivot_milestone.attribution,
        Attribution::AiInterpretation,
        "Pending purpose pivot must be labeled AiInterpretation"
    );

    let pivot_segment = history_before
        .segments
        .iter()
        .find(|s| s.text.contains("Sthira inferred this shift") || s.attribution == Attribution::AiInterpretation)
        .expect("pivot narrative segment should exist");
    assert_eq!(pivot_segment.attribution, Attribution::AiInterpretation);

    // Session 4: User accepts the purpose pivot proposal
    clock.advance_ms(sthira_core::IDLE_GAP_MS + 5000);
    engine
        .decide_proposal(&prop_id, "accepted")
        .expect("accept proposal failed");

    let history_after = engine.get_project_history(&proj.id).expect("get history after failed");

    // Assert pivot milestone now transitions to UserRecorded
    let accepted_pivot = history_after
        .milestones
        .iter()
        .find(|m| m.milestone_type == MilestoneType::PurposePivot)
        .expect("pivot milestone should exist after acceptance");
    assert_eq!(
        accepted_pivot.attribution,
        Attribution::UserRecorded,
        "Accepted purpose pivot must transition to UserRecorded"
    );

    // Assert the original goal from Session 1 is still preserved in project_purposes
    let reader = engine.db().reader().expect("get reader failed");
    let mut stmt = reader
        .prepare("SELECT purpose, superseded_at FROM project_purposes WHERE project_id = ?1 ORDER BY set_at ASC;")
        .expect("prepare failed");
    let purposes: Vec<(String, Option<i64>)> = stmt
        .query_map(rusqlite::params![proj.id], |r| Ok((r.get(0)?, r.get(1)?)))
        .expect("query failed")
        .filter_map(|r| r.ok())
        .collect();

    assert_eq!(purposes.len(), 2, "project_purposes must retain both original goal and new purpose");
    assert_eq!(purposes[0].0, "Cloud PR bot to review pull requests using cloud LLMs");
    assert!(purposes[0].1.is_some(), "Original goal superseded_at must be recorded");
    assert_eq!(purposes[1].0, "Local on-premise PR bot ensuring data privacy");
    assert!(purposes[1].1.is_none(), "New purpose superseded_at must be NULL");
}

#[tokio::test]
async fn test_5_2_one_character_semantic_milestone_edits() {
    let (engine, _clock, _model, _dir) = setup_test_engine();

    let proj = engine
        .create_project("VRAM and Architecture Evaluation", None)
        .expect("create project failed");

    // Note 1: VRAM target edit: "VRAM target: 16GB" -> "VRAM target: 8GB"
    let vram_note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("VRAM Target Note"),
            "VRAM target: 16GB",
            None,
        )
        .expect("create vram note failed");
    engine
        .attach_artifact(&proj.id, &vram_note.id, "user_explicit", None)
        .expect("attach note failed");

    let mat_vram = sthira_core::materiality::compute_materiality("VRAM target: 16GB", "VRAM target: 8GB");
    assert_eq!(
        mat_vram,
        Materiality::Substantive,
        "16GB -> 8GB must be recognized as Substantive"
    );

    engine
        .update_artifact(&vram_note.id, "VRAM target: 8GB", sthira_core::Actor::User, Some(&proj.id))
        .expect("update vram note failed");

    // Note 2: Negation edit: "We will use cloud" -> "We will not use cloud"
    let cloud_note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Cloud Strategy Note"),
            "We will use cloud",
            None,
        )
        .expect("create cloud note failed");
    engine
        .attach_artifact(&proj.id, &cloud_note.id, "user_explicit", None)
        .expect("attach cloud note failed");

    let mat_cloud = sthira_core::materiality::compute_materiality("We will use cloud", "We will not use cloud");
    assert_eq!(
        mat_cloud,
        Materiality::Substantive,
        "will use -> will not use must be recognized as Substantive"
    );

    engine
        .update_artifact(&cloud_note.id, "We will not use cloud", sthira_core::Actor::User, Some(&proj.id))
        .expect("update cloud note failed");

    // Assert both substantive edits appear in history
    let history = engine.get_project_history(&proj.id).expect("get history failed");
    let substantive_milestones: Vec<_> = history
        .milestones
        .iter()
        .filter(|m| m.milestone_type == MilestoneType::SubstantiveEdit)
        .collect();

    assert_eq!(
        substantive_milestones.len(),
        2,
        "Both 1-character/negation substantive edits must appear in history milestones"
    );
}

#[tokio::test]
async fn test_5_3_typo_storm_zero_extraction() {
    let (engine, _clock, model, _dir) = setup_test_engine();

    let proj = engine
        .create_project("Typo Storm Project", None)
        .expect("create proj failed");

    let base_content = "This is a detailed technical note about database architecture and consistency.";
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("DB Architecture Note"),
            base_content,
            None,
        )
        .expect("create note failed");
    engine
        .attach_artifact(&proj.id, &note.id, "user_explicit", None)
        .expect("attach note failed");

    // Initial extraction run
    let user_prompt = format!("Extract knowledge nodes from this document:\n\n{}", base_content);
    model.register_response(
        user_prompt,
        serde_json::json!({
            "nodes": [
                {
                    "node_type": "finding",
                    "statement": "Database architecture requires consistency",
                    "quote": "This is a detailed technical note about database architecture and consistency."
                }
            ]
        })
        .to_string(),
    );

    let initial_nodes = engine
        .extract_epistemic_nodes(&note.id)
        .await
        .expect("initial extraction failed");
    assert_eq!(initial_nodes.len(), 1);
    assert_eq!(model.call_count(), 1, "Initial extraction must call model once");

    // Reset call count to verify 0 calls during the typo storm
    model.reset_call_count();

    let typo_storm = [
        "This is a detailed techical note about database architecture and consistency.",
        "This is a detailed technical note about database architecture and consistency.",
        "This is a detailed technical note about database archtecture and consistency.",
        "This is a detailed technical note about database architecture and consistency.",
        "This is a detailed technical note about database architecture and consistncy.",
        "This is a detailed technical note about database architecture and consistency.",
        "This is a detailed technical note about databse architecture and consistency.",
        "This is a detailed technical note about database architecture and consistency.",
        "This is a detailed technical note  about database architecture and consistency.",
        "This is a detailed technical note about database architecture and consistency.",
        "This is a detailed technical note about database architecture and consistency..",
        "This is a detailed technical note about database architecture and consistency.",
        "This is a detailed technical note about database architecture and consistency!",
        "This is a detailed technical note about database architecture and consistency.",
        "This is a detailed technical note about database architecture and consistency.",
    ];

    for (idx, typo) in typo_storm.iter().enumerate() {
        engine
            .update_artifact(&note.id, typo, sthira_core::Actor::User, Some(&proj.id))
            .expect("update typo failed");

        let extracted = engine
            .extract_epistemic_nodes(&note.id)
            .await
            .expect("extract typo failed");

        assert_eq!(
            extracted.len(),
            1,
            "Existing nodes must be preserved without re-extraction on typo {}",
            idx
        );
    }

    // Assert 0 LLM extraction runs during the 15 typo edits
    assert_eq!(
        model.call_count(),
        0,
        "Typo storm must produce 0 LLM extraction calls"
    );

    // Assert 0 milestone entries for the typo edits in history
    let history = engine.get_project_history(&proj.id).expect("get history failed");
    let typo_substantive_milestones: Vec<_> = history
        .milestones
        .iter()
        .filter(|m| m.milestone_type == MilestoneType::SubstantiveEdit)
        .collect();

    assert_eq!(
        typo_substantive_milestones.len(),
        0,
        "Typo storm must produce 0 substantive edit milestone entries in history"
    );
}

#[tokio::test]
async fn test_5_4_milestone_strict_chronological_ordering() {
    let (engine, clock, _model, _dir) = setup_test_engine();

    let proj = engine
        .create_project("Timeline Project", None)
        .expect("create proj failed");
    engine
        .set_project_purpose(&proj.id, "Initial Timeline Goal")
        .expect("set purpose failed");

    clock.advance_ms(1000);
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Progress Note"),
            "Initial content 100",
            None,
        )
        .expect("create note failed");
    engine
        .attach_artifact(&proj.id, &note.id, "user_explicit", None)
        .expect("attach note failed");

    clock.advance_ms(1000);
    engine
        .update_artifact(&note.id, "Initial content 200", sthira_core::Actor::User, Some(&proj.id))
        .expect("substantive update failed");

    clock.advance_ms(1000);
    let writer = engine.db().writer();
    writer.execute(
        "INSERT INTO epistemic_nodes (id, artifact_id, node_type, origin, statement, quote, quote_start, quote_end, quote_hash, basis_content_hash, extraction_run_id, model_id, first_seen_at, updated_at)
         VALUES ('dec_time', ?1, 'decision', 'user_recorded', 'Decision statement', 'Initial content 200', 0, 19, 'qh', 'bh', 'r', 'm', ?2, ?2);",
        rusqlite::params![note.id, clock.now_ms()],
    ).expect("insert dec failed");
    drop(writer);

    let history = engine.get_project_history(&proj.id).expect("get history failed");
    assert!(history.milestones.len() >= 3);

    // Verify chronological order
    for w in history.milestones.windows(2) {
        assert!(
            w[0].timestamp_ms <= w[1].timestamp_ms,
            "Milestones must be strictly sorted by timestamp: {} <= {}",
            w[0].timestamp_ms,
            w[1].timestamp_ms
        );
    }
}
