use std::sync::Arc;
use sthira_core::{
    ArtifactType, Authorship, CanvasItemKind, CoreError, Engine,
};

pub fn clean_duplicate_models_on_startup(engine: &Arc<Engine>) -> Result<usize, CoreError> {
    let writer = engine.db().writer();
    sthira_core::db::providers_repo::clean_duplicate_models(&writer)
}

pub fn seed_demo_workspace_if_empty(engine: &Arc<Engine>) -> Result<(), CoreError> {
    // Strip duplicate models on startup/seed
    let _ = clean_duplicate_models_on_startup(engine);

    let reader = engine.db().reader()?;
    let project_count: i64 = reader.query_row("SELECT COUNT(*) FROM projects;", [], |r| r.get(0))?;
    drop(reader);

    if project_count > 0 {
        return Ok(());
    }

    // 1. Seed "Local AI research"
    let p1 = engine.create_project("Local AI research", None)?;
    engine.set_project_purpose(
        &p1.id,
        "Evaluate fast local inference engines vs cloud synthesis models for knowledge extraction.",
    )?;

    let art1 = engine.create_artifact(
        ArtifactType::Note,
        Authorship::User,
        Some("Model performance notes"),
        "# Model performance notes\n\nWe evaluated local 3B parameter models against larger cloud endpoints.\n\nThe 3B local model processes routing requests in under 15ms with 100% intent classification accuracy.\nWe decided to allocate 16GB VRAM for local execution to ensure deterministic responsiveness.\n",
        None,
    )?;
    engine.attach_artifact(&p1.id, &art1.id, "user_explicit", None)?;

    // Insert 5 more artifacts to reach 6 items
    let titles_p1 = [
        "Quantization benchmarks",
        "Ollama latency profiles",
        "Token throughput analysis",
        "Context window scaling",
        "Memory bandwidth constraints",
    ];
    for t in titles_p1 {
        let a = engine.create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some(t),
            &format!("# {}\n\nInitial findings and architecture considerations.\n", t),
            None,
        )?;
        engine.attach_artifact(&p1.id, &a.id, "user_explicit", None)?;
    }

    // 2. Seed "Webhook platform"
    let p2 = engine.create_project("Webhook platform", None)?;
    engine.set_project_purpose(
        &p2.id,
        "High throughput webhook ingestion pipeline with replay guarantees and dead letter queues.",
    )?;

    // Insert 12 items
    for i in 1..=12 {
        let t = format!("Pipeline spec part {}", i);
        let a = engine.create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some(&t),
            &format!("# {}\n\nWebhook architecture and delivery guarantees component.\n", t),
            None,
        )?;
        engine.attach_artifact(&p2.id, &a.id, "user_explicit", None)?;
    }

    // 3. Seed Canvas "Ideas to investigate"
    let canvas_id = engine.create_canvas(Some(&p1.id), "Ideas to investigate")?;
    engine.add_canvas_item(
        &canvas_id,
        &uuid::Uuid::new_v4().to_string(),
        CanvasItemKind::Artifact,
        Some(&art1.id),
        100.0,
        150.0,
        Some(320.0),
        Some(200.0),
        1,
    )?;
    engine.add_canvas_item(
        &canvas_id,
        &uuid::Uuid::new_v4().to_string(),
        CanvasItemKind::Text,
        None,
        460.0,
        150.0,
        Some(260.0),
        Some(120.0),
        2,
    )?;

    // 4. Seed an epistemic node for art1 so Epistemic badges & quotes show in Focus View
    {
        let writer = engine.db().writer();
        let now_ms = engine.clock().now_ms();
        let node_id = uuid::Uuid::new_v4().to_string();
        let content_hash = &art1.content_hash;
        let _ = writer.execute(
            "INSERT INTO epistemic_nodes (
                id, artifact_id, context_project_id, node_type, origin, lifecycle, review_state,
                statement, quote, quote_start, quote_end, quote_hash, basis_content_hash,
                extraction_run_id, model_id, first_seen_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, 'decision', 'user_recorded', 'open', 'confirmed',
                ?4, ?5, ?6, ?7, ?8, ?9,
                'seed-run', 'manual-seed', ?10, ?10
            );",
            rusqlite::params![
                node_id,
                art1.id,
                p1.id,
                "Allocate 16GB VRAM for local execution",
                "We decided to allocate 16GB VRAM for local execution to ensure deterministic responsiveness.",
                187,
                276,
                "seed-quote-hash",
                content_hash,
                now_ms
            ],
        );
    }

    Ok(())
}
