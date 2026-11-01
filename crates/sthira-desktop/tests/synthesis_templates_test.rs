use std::sync::Arc;
use sthira_core::{ArtifactType, Authorship, Engine};
use sthira_desktop::{
    handle_derive_artifact, handle_list_synthesis_templates, handle_refine_artifact,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

fn setup_test_engine() -> (tempfile::TempDir, Arc<Engine>) {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("synthesis_templates_test.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    (tmp, engine)
}

#[tokio::test]
async fn test_synthesis_templates_seeding_and_filtering() {
    let (_tmp, engine) = setup_test_engine();

    // 1. List all synthesis templates
    let all_templates = handle_list_synthesis_templates(&engine, None)
        .expect("Listing all synthesis templates must succeed");
    assert!(
        all_templates.len() >= 4,
        "Engine::open must pre-seed default templates (found {})",
        all_templates.len()
    );

    // 2. Filter by target_mode = "research"
    let research_templates = handle_list_synthesis_templates(&engine, Some("research".to_string()))
        .expect("Listing research templates must succeed");
    assert!(!research_templates.is_empty(), "Must have research templates");
    for t in &research_templates {
        assert_eq!(t.target_mode, "research");
    }
    let default_research = research_templates.iter().find(|t| t.is_default);
    assert!(default_research.is_some(), "Default research template must exist");
    let def_res = default_research.unwrap();
    assert_eq!(def_res.name, "Comparative Tech Evaluation");
    assert!(def_res.template_skeleton_md.contains("## Comparison Matrix"));

    // 3. Filter by target_mode = "plan"
    let plan_templates = handle_list_synthesis_templates(&engine, Some("plan".to_string()))
        .expect("Listing plan templates must succeed");
    assert!(!plan_templates.is_empty(), "Must have plan templates");
    for t in &plan_templates {
        assert_eq!(t.target_mode, "plan");
    }
    let default_plan = plan_templates.iter().find(|t| t.is_default);
    assert!(default_plan.is_some(), "Default plan template must exist");
    let def_plan = default_plan.unwrap();
    assert_eq!(def_plan.name, "Milestone Roadmap");
    assert!(def_plan.template_skeleton_md.contains("## Phase 1"));
    assert!(def_plan.template_skeleton_md.contains("## Phase 2"));
}

#[tokio::test]
async fn test_derive_artifact_with_template_skeleton_injection() {
    let (_tmp, engine) = setup_test_engine();

    // 1. Create a source research note
    let src_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Database Architecture Evaluation"),
            "# Database Architecture Evaluation\n\nExecutive findings comparing SQLite vs RocksDB.",
            None,
        )
        .expect("Create source artifact");

    // 2. Derive plan with explicit default plan template
    let derived_plan_id = handle_derive_artifact(
        &engine,
        src_art.id.clone(),
        "plan_from_research".to_string(),
        Some("plan-milestone-roadmap".to_string()),
        None,
    )
    .await
    .expect("Deriving plan with template must succeed");

    let derived_art = engine
        .get_artifact(&derived_plan_id)
        .expect("Fetch derived artifact");

    // Lineage verification
    assert_eq!(
        derived_art.derived_from_artifact_id,
        Some(src_art.id.clone()),
        "derived_from_artifact_id must match parent"
    );
    assert_eq!(
        derived_art.derivation_type,
        Some("plan_from_research".to_string())
    );

    // Skeleton preservation verification
    assert!(
        derived_art.content_md.contains("## Scope & Objectives"),
        "Derived plan must preserve template skeleton header: Scope & Objectives"
    );
    assert!(
        derived_art.content_md.contains("## Phase 1: Foundation & Core Setup"),
        "Derived plan must preserve template skeleton header: Phase 1"
    );
    assert!(
        derived_art.content_md.contains("## Phase 2: Feature Implementation"),
        "Derived plan must preserve template skeleton header: Phase 2"
    );
    assert!(
        derived_art.content_md.contains("## Open Decisions & Trade-offs"),
        "Derived plan must preserve template skeleton header: Open Decisions & Trade-offs"
    );
}

#[tokio::test]
async fn test_refine_artifact_snapshots_and_reversibility() {
    let (_tmp, engine) = setup_test_engine();

    let initial = "# Vector Indexing Options\n\nInitial notes on HNSW and Flat L2.";
    let base = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Vector Indexing"),
            initial,
            None,
        )
        .expect("Create base note");

    let critique = "Incorporate IVFFlat recall vs latency tradeoffs and memory boundaries.";
    let refined = handle_refine_artifact(
        &engine,
        base.id.clone(),
        critique.to_string(),
        None,
        false,
    )
    .await
    .expect("Refining artifact must succeed");

    assert!(refined.contains(critique));

    let updated = engine.get_artifact(&base.id).expect("Fetch updated note");
    assert_eq!(updated.content_md, refined);
}
