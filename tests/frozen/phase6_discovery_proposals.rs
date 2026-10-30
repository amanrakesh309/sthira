use std::sync::Arc;
use sthira_core::{
    ArtifactType, Authorship, Engine,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::TempDir;

fn setup_test_engine() -> (Engine, Arc<FakeClock>, Arc<DeterministicFakeModel>, TempDir) {
    let dir = TempDir::new().expect("create tempdir failed");
    let db_path = dir.path().join("sthira.db");
    let vault_dir = dir.path().join(".sthira_vault").join("artifacts");
    let clock = Arc::new(FakeClock::new(1000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));
    let engine = Engine::open(&db_path, &vault_dir, clock.clone(), model.clone()).expect("open engine failed");

    // Configure embedding model threshold
    let writer = engine.db().writer();
    writer.execute(
        "INSERT OR REPLACE INTO embedding_models (id, dim, cosine_threshold, calibrated_at)
         VALUES ('test-model', 128, 0.75, 1000);",
        [],
    ).expect("insert embedding model failed");
    drop(writer);

    (engine, clock, model, dir)
}

#[tokio::test]
async fn test_6_1_cross_project_prior_knowledge_real_vectors_and_ablation() {
    let (engine, _clock, model, _dir) = setup_test_engine();

    // 1. Load frozen real precomputed vectors dataset
    let dataset_str = include_str!("../../fixtures/golden/discovery_pairs_100.json");
    let dataset: serde_json::Value = serde_json::from_str(dataset_str).expect("parse dataset failed");
    let pairs = dataset["pairs"].as_array().expect("pairs must be array");

    let pair_0 = &pairs[0];
    let source_text = pair_0["source_text"].as_str().unwrap();
    let target_text = pair_0["target_text"].as_str().unwrap();

    let source_emb: Vec<f32> = pair_0["source_embedding"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect();
    let target_emb: Vec<f32> = pair_0["target_embedding"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect();

    // Project A: Prior knowledge
    let proj_a = engine.create_project("Resilience Architecture", None).expect("create proj a");
    let note_a = engine
        .create_artifact(ArtifactType::Note, Authorship::User, Some("Backoff Pattern"), source_text, None)
        .expect("create note a");
    engine.attach_artifact(&proj_a.id, &note_a.id, "user_explicit", None).expect("attach note a");
    engine
        .store_custom_embedding(&note_a.id, &note_a.content_hash, &source_emb)
        .expect("store custom embedding a");

    // Project B: Current work
    let proj_b = engine.create_project("Webhook Ingestion", None).expect("create proj b");
    let note_b = engine
        .create_artifact(ArtifactType::Note, Authorship::User, Some("Webhook Retries"), target_text, None)
        .expect("create note b");
    engine.attach_artifact(&proj_b.id, &note_b.id, "user_explicit", None).expect("attach note b");
    engine
        .store_custom_embedding(&note_b.id, &note_b.content_hash, &target_emb)
        .expect("store custom embedding b");

    // Register contradiction check response
    let user_prompt = format!("Statement A:\n{}\n\nStatement B:\n{}", target_text, source_text);
    model.register_response(
        user_prompt,
        serde_json::json!({
            "is_true_contradiction": false,
            "scope_difference": false,
            "explanation": "Both documents describe complementary exponential backoff and jitter retry patterns."
        }).to_string(),
    );

    // Run discovery for note_b in Project B
    let proposals = engine
        .run_discovery(&proj_b.id, &note_b.id)
        .await
        .expect("run discovery failed");

    assert_eq!(proposals.len(), 1, "Must propose cross-project link for high similarity match");

    // Verify proposed target is note_a
    let reader = engine.db().reader().expect("get reader");
    let (target_id, kind): (String, String) = reader
        .query_row(
            "SELECT json_extract(payload, '$.target_artifact_id'), kind FROM ai_proposals WHERE id = ?1;",
            rusqlite::params![proposals[0]],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("query proposal");
    assert_eq!(target_id, note_a.id);
    assert_eq!(kind, "prior_knowledge");
    drop(reader);

    // 2. Ablation Run: Random vectors that fall below calibrated threshold (0.75)
    let proj_c = engine.create_project("Unrelated Billing", None).expect("create proj c");
    let note_c = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Monthly Invoices"),
            "Calculate monthly billing statements from invoice line items.",
            None,
        )
        .expect("create note c");
    engine.attach_artifact(&proj_c.id, &note_c.id, "user_explicit", None).expect("attach note c");

    // Generate orthogonal random vector (cosine similarity ~ 0.05)
    let mut random_emb = vec![0.0f32; 128];
    for (i, v) in random_emb.iter_mut().enumerate() {
        *v = if i % 2 == 0 { 0.1 } else { -0.1 };
    }
    let norm: f32 = random_emb.iter().map(|x| x * x).sum::<f32>().sqrt();
    for v in &mut random_emb {
        *v /= norm;
    }
    engine
        .store_custom_embedding(&note_c.id, &note_c.content_hash, &random_emb)
        .expect("store random embedding");

    // Run discovery from note_c
    let ablation_proposals = engine
        .run_discovery(&proj_c.id, &note_c.id)
        .await
        .expect("run ablation discovery failed");

    assert_eq!(
        ablation_proposals.len(),
        0,
        "Ablation with low similarity vector must produce zero proposals (below 0.75 threshold)"
    );
}

#[tokio::test]
async fn test_6_2_contradiction_vs_scope_difference_precision() {
    let (engine, _clock, model, _dir) = setup_test_engine();

    let dataset_str = include_str!("../../fixtures/golden/discovery_pairs_100.json");
    let dataset: serde_json::Value = serde_json::from_str(dataset_str).expect("parse dataset failed");
    let pairs = dataset["pairs"].as_array().expect("pairs must be array");

    let contra_pairs: Vec<_> = pairs
        .iter()
        .filter(|p| p["is_contradiction"].as_bool().unwrap_or(false))
        .collect();
    let scope_pairs: Vec<_> = pairs
        .iter()
        .filter(|p| p["is_scope_difference"].as_bool().unwrap_or(false))
        .collect();

    assert_eq!(contra_pairs.len(), 10, "Dataset must have 10 true contradictions");
    assert_eq!(scope_pairs.len(), 10, "Dataset must have 10 scope differences");

    let mut true_positives = 0;
    let mut false_positives = 0;

    // Evaluate 10 true contradictions
    for p in &contra_pairs {
        let s = p["source_text"].as_str().unwrap();
        let t = p["target_text"].as_str().unwrap();

        let user_prompt = format!("Statement A:\n{}\n\nStatement B:\n{}", s, t);
        model.register_response(
            user_prompt,
            serde_json::json!({
                "is_true_contradiction": true,
                "scope_difference": false,
                "explanation": "Mutually exclusive technical decisions for the same system component."
            }).to_string(),
        );

        let analysis = sthira_core::discovery::analyze_contradiction(&engine, s, t)
            .await
            .expect("analyze failed");

        if analysis.is_true_contradiction && !analysis.scope_difference {
            true_positives += 1;
        } else {
            false_positives += 1;
        }
    }

    // Evaluate 10 scope differences (near-misses)
    for p in &scope_pairs {
        let s = p["source_text"].as_str().unwrap();
        let t = p["target_text"].as_str().unwrap();

        let user_prompt = format!("Statement A:\n{}\n\nStatement B:\n{}", s, t);
        model.register_response(
            user_prompt,
            serde_json::json!({
                "is_true_contradiction": false,
                "scope_difference": true,
                "explanation": "Valid contextual variation across different operating systems, hardware tiers, or release versions."
            }).to_string(),
        );

        let analysis = sthira_core::discovery::analyze_contradiction(&engine, s, t)
            .await
            .expect("analyze failed");

        if analysis.is_true_contradiction && !analysis.scope_difference {
            false_positives += 1;
        }
    }

    let precision = true_positives as f64 / (true_positives + false_positives) as f64;
    assert!(
        precision >= 0.90,
        "Contradiction detection precision must be >= 0.90 (actual: {})",
        precision
    );
    assert_eq!(true_positives, 10);
    assert_eq!(false_positives, 0);
}

#[tokio::test]
async fn test_6_3_permanent_dismissal_and_reversibility() {
    let (engine, clock, model, dir) = setup_test_engine();
    let db_path = dir.path().join("sthira.db");
    let vault_dir = dir.path().join(".sthira_vault").join("artifacts");

    let proj_1 = engine.create_project("Project Alpha", None).expect("create proj 1");
    let proj_2 = engine.create_project("Project Beta", None).expect("create proj 2");

    let note_1 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Alpha Architecture"),
            "Alpha architecture uses message passing over queues.",
            None,
        )
        .expect("create note 1");
    engine.attach_artifact(&proj_1.id, &note_1.id, "user_explicit", None).expect("attach note 1");

    let note_2 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Beta Messaging"),
            "Beta subsystem communicates via asynchronous message queues.",
            None,
        )
        .expect("create note 2");
    engine.attach_artifact(&proj_2.id, &note_2.id, "user_explicit", None).expect("attach note 2");

    // Store high similarity embeddings
    let v1 = vec![0.1f32; 128];
    let v2 = vec![0.1f32; 128];
    engine.store_custom_embedding(&note_1.id, &note_1.content_hash, &v1).expect("store v1");
    engine.store_custom_embedding(&note_2.id, &note_2.content_hash, &v2).expect("store v2");

    // Register contradiction check
    let user_prompt = format!("Statement A:\n{}\n\nStatement B:\n{}", note_2.content_md, note_1.content_md);
    model.register_response(
        user_prompt,
        serde_json::json!({
            "is_true_contradiction": false,
            "scope_difference": false,
            "explanation": "Related messaging concepts"
        }).to_string(),
    );

    // 1. Run discovery -> generates proposal
    let prop_ids = engine.run_discovery(&proj_2.id, &note_2.id).await.expect("discovery failed");
    assert_eq!(prop_ids.len(), 1);
    let prop_id = &prop_ids[0];

    // 2. Dismiss proposal
    engine.decide_proposal(prop_id, "dismissed").expect("dismiss failed");

    let dismissed = engine.list_dismissed_proposals().expect("list dismissed failed");
    assert_eq!(dismissed.len(), 1);
    assert_eq!(dismissed[0].id, *prop_id);

    // 3. Edit both notes
    clock.advance_ms(5000);
    engine
        .update_artifact(&note_1.id, "Alpha architecture uses message passing over high-speed queues.", sthira_core::Actor::User, Some(&proj_1.id))
        .expect("update note 1");
    engine
        .update_artifact(&note_2.id, "Beta subsystem communicates via asynchronous high-speed message queues.", sthira_core::Actor::User, Some(&proj_2.id))
        .expect("update note 2");

    // 4. Restart engine on the same database
    let restarted_engine = Engine::open(&db_path, &vault_dir, clock.clone(), model.clone())
        .expect("restart engine failed");

    // Re-run discovery: proposal must NEVER reappear
    let prop_ids_restarted = restarted_engine
        .run_discovery(&proj_2.id, &note_2.id)
        .await
        .expect("restarted discovery failed");
    assert_eq!(
        prop_ids_restarted.len(),
        0,
        "Dismissed proposal must NEVER reappear after note edits and engine restart"
    );

    // 5. Raw SQL duplicate insert must fail on ux_proposal_live
    let writer = restarted_engine.db().writer();
    let subject_key = format!("link:{}:{}", note_2.id, note_1.id);
    let raw_dup_result = writer.execute(
        "INSERT INTO ai_proposals (id, kind, subject_key, payload, evidence, model_id, state, created_at)
         VALUES ('dup_id', 'prior_knowledge', ?1, '{}', '{}', 'test-model', 'pending', 2000);",
        rusqlite::params![subject_key],
    );
    assert!(
        raw_dup_result.is_err(),
        "Raw SQL duplicate insert must fail on ux_proposal_live partial unique index"
    );
    drop(writer);

    // 6. Test Reversibility: Accept proposal -> Link created -> Revert ChangeSet -> Link removed & Proposal dismissed
    let note_3 = restarted_engine
        .create_artifact(ArtifactType::Note, Authorship::User, Some("Note 3"), "Distributed log replication", None)
        .expect("create note 3");
    restarted_engine.attach_artifact(&proj_1.id, &note_3.id, "user_explicit", None).expect("attach note 3");

    let note_4 = restarted_engine
        .create_artifact(ArtifactType::Note, Authorship::User, Some("Note 4"), "Distributed consensus commit log", None)
        .expect("create note 4");
    restarted_engine.attach_artifact(&proj_2.id, &note_4.id, "user_explicit", None).expect("attach note 4");

    restarted_engine.store_custom_embedding(&note_3.id, &note_3.content_hash, &v1).expect("store v3");
    restarted_engine.store_custom_embedding(&note_4.id, &note_4.content_hash, &v2).expect("store v4");

    let user_prompt_43 = format!("Statement A:\n{}\n\nStatement B:\n{}", note_4.content_md, note_3.content_md);
    model.register_response(
        user_prompt_43,
        serde_json::json!({
            "is_true_contradiction": false,
            "scope_difference": false,
            "explanation": "Related consensus concepts"
        }).to_string(),
    );

    let prop_ids_2 = restarted_engine.run_discovery(&proj_2.id, &note_4.id).await.expect("discovery 2 failed");
    assert_eq!(prop_ids_2.len(), 1);
    let prop_id_2 = &prop_ids_2[0];

    // Accept proposal
    let cs_id = restarted_engine.apply_discovery_link(prop_id_2).expect("apply discovery link failed");

    // Verify link exists
    let reader = restarted_engine.db().reader().expect("get reader");
    let link_exists: bool = reader
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM artifact_links WHERE proposal_id = ?1);",
            rusqlite::params![prop_id_2],
            |r| r.get(0),
        )
        .expect("query link");
    assert!(link_exists, "Artifact link must exist after accepting proposal");
    drop(reader);

    // Revert ChangeSet
    restarted_engine.revert_change_set(&cs_id).expect("revert change set failed");

    // Verify link was removed
    let reader = restarted_engine.db().reader().expect("get reader");
    let link_after_revert: bool = reader
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM artifact_links WHERE proposal_id = ?1);",
            rusqlite::params![prop_id_2],
            |r| r.get(0),
        )
        .expect("query link");
    assert!(!link_after_revert, "Artifact link must be removed after reverting change set");

    // Verify proposal moved to dismissed
    let prop_state: String = reader
        .query_row(
            "SELECT state FROM ai_proposals WHERE id = ?1;",
            rusqlite::params![prop_id_2],
            |r| r.get(0),
        )
        .expect("query proposal state");
    assert_eq!(prop_state, "dismissed", "Proposal must move to dismissed upon revert so it is not nagged back");
}
