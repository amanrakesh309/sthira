use std::sync::Arc;
use sthira_core::{ArtifactType, Authorship, Engine};
use sthira_desktop::{
    handle_derive_artifact, handle_get_artifact_context, handle_refine_artifact,
    handle_revert_change_set,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

fn setup_test_engine() -> (tempfile::TempDir, Arc<Engine>) {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("chained_synthesis_test.db");
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
async fn test_refine_artifact_updates_content_and_creates_reversible_changeset() {
    let (_tmp, engine) = setup_test_engine();

    // 1. Create a base research note
    let original_content = "# High-Performance Consensus\n\nRaft ensures strong consistency across distributed nodes via leader election.";
    let base_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("High-Performance Consensus"),
            original_content,
            None,
        )
        .expect("Failed to create base artifact");

    // 2. Perform refinement with critique
    let critique = "Focus more on benchmark data, throughput metrics, and Paxos trade-offs.";
    let refined_content = handle_refine_artifact(
        &engine,
        base_art.id.clone(),
        critique.to_string(),
        None,
        false,
    )
    .await
    .expect("Refinement should succeed");

    assert!(
        refined_content.contains(critique),
        "Refined content must address requested critique"
    );

    // 3. Verify DB has updated content
    let updated_art = engine
        .get_artifact(&base_art.id)
        .expect("Must fetch updated artifact");
    assert_eq!(updated_art.content_md, refined_content);

    // 4. Verify a ChangeSet snapshot was created
    let reader = engine.db().reader().unwrap();
    let cs_id: String = reader
        .query_row(
            "SELECT id FROM change_sets WHERE summary LIKE '%Refined%' ORDER BY applied_at DESC LIMIT 1;",
            [],
            |r| r.get(0),
        )
        .expect("ChangeSet snapshot must be recorded for 1-click undo");

    // 5. Test 1-click undo: revert the changeset and verify original content restored
    handle_revert_change_set(&engine, cs_id).expect("Reverting refinement changeset must succeed");

    let restored_art = engine
        .get_artifact(&base_art.id)
        .expect("Must fetch restored artifact");
    assert_eq!(
        restored_art.content_md, original_content,
        "Reverting changeset must restore exact original content"
    );
}

#[tokio::test]
async fn test_derive_artifact_creates_new_note_with_lineage_foreign_key() {
    let (_tmp, engine) = setup_test_engine();

    // 1. Create source research artifact
    let src_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Distributed Storage Findings"),
            "# Distributed Storage Findings\n\n- LSM-trees offer high write throughput.\n- B-Trees offer superior point-lookup latency.",
            None,
        )
        .expect("Failed to create source artifact");

    // 2. Derive an execution plan
    let plan_art_id = handle_derive_artifact(
        &engine,
        src_art.id.clone(),
        "plan_from_research".to_string(),
        None,
        None,
    )
    .await
    .expect("Deriving plan must succeed");

    assert_ne!(plan_art_id, src_art.id, "Derived artifact must be a new entity");

    // 3. Inspect derived artifact properties and lineage foreign keys
    let plan_art = engine
        .get_artifact(&plan_art_id)
        .expect("Must retrieve derived plan artifact");

    assert_eq!(
        plan_art.derived_from_artifact_id,
        Some(src_art.id.clone()),
        "Derived note must store parent artifact ID"
    );
    assert_eq!(
        plan_art.derivation_type,
        Some("plan_from_research".to_string()),
        "Derived note must store derivation type"
    );
    assert!(
        plan_art.title.as_deref().unwrap_or("").contains("Distributed Storage Findings"),
        "Derived note title must reference source title"
    );
    assert!(
        plan_art.content_md.contains("plan_from_research"),
        "Derived note content must reflect derivation type"
    );

    // 4. Test handle_get_artifact_context returns parent derived_from_title
    let ctx = handle_get_artifact_context(&engine, plan_art_id.clone())
        .expect("handle_get_artifact_context must succeed");

    assert_eq!(
        ctx.derived_from_title,
        Some("Distributed Storage Findings".to_string()),
        "ArtifactContext must populate parent title for UI lineage chip"
    );

    // 5. Derive discrete ideas
    let ideas_art_id = handle_derive_artifact(
        &engine,
        src_art.id.clone(),
        "extract_ideas".to_string(),
        None,
        None,
    )
    .await
    .expect("Deriving ideas must succeed");

    let ideas_art = engine.get_artifact(&ideas_art_id).unwrap();
    assert_eq!(ideas_art.derived_from_artifact_id, Some(src_art.id.clone()));
    assert_eq!(ideas_art.derivation_type, Some("extract_ideas".to_string()));

    // 6. Derive task checklist
    let tasks_art_id = handle_derive_artifact(
        &engine,
        src_art.id.clone(),
        "extract_tasks".to_string(),
        None,
        None,
    )
    .await
    .expect("Deriving tasks must succeed");

    let tasks_art = engine.get_artifact(&tasks_art_id).unwrap();
    assert_eq!(tasks_art.derived_from_artifact_id, Some(src_art.id.clone()));
    assert_eq!(tasks_art.derivation_type, Some("extract_tasks".to_string()));
}
