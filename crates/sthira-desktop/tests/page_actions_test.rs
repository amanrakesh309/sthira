use std::sync::Arc;
use sthira_core::{
    models::objects::{ObjectType, PropertyDefinition, PropertyType},
    ArtifactType, Authorship, Engine,
};
use sthira_desktop::{
    handle_convert_artifact_to_object, handle_create_object_type, handle_create_project,
    handle_create_project_with_artifact, handle_detach_artifact_from_project,
    handle_duplicate_artifact, handle_link_artifact_to_project, handle_move_artifact_to_project,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

fn setup_test_engine() -> (tempfile::TempDir, Arc<Engine>) {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("page_actions_test.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    (tmp, engine)
}

fn make_note(engine: &Engine, title: &str, body: &str) -> String {
    engine
        .create_artifact(ArtifactType::Note, Authorship::User, Some(title), body, None)
        .unwrap()
        .id
}

fn count_active_links(engine: &Engine, project_id: &str, artifact_id: &str) -> i64 {
    let reader = engine.db().reader().unwrap();
    reader
        .query_row(
            "SELECT COUNT(*) FROM project_artifacts
             WHERE project_id = ?1 AND artifact_id = ?2 AND detached_at IS NULL;",
            rusqlite::params![project_id, artifact_id],
            |r| r.get(0),
        )
        .unwrap()
}

fn count_all_artifacts(engine: &Engine) -> i64 {
    let reader = engine.db().reader().unwrap();
    reader
        .query_row("SELECT COUNT(*) FROM artifacts;", [], |r| r.get(0))
        .unwrap()
}

// ---------------------------------------------------------------------------
// 1. cmd_link_artifact_to_project: join-table only, zero content duplication,
//    idempotent on repeat.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_link_artifact_to_project_links_without_duplication() {
    let (_tmp, engine) = setup_test_engine();

    let project = handle_create_project(&engine, "Retrieval Systems".into(), None).unwrap();
    let artifact_id = make_note(&engine, "Vector Index Notes", "# Vector Index Notes\n\nHNSW body.");
    let before = engine.get_artifact(&artifact_id).unwrap();
    let artifacts_before = count_all_artifacts(&engine);

    handle_link_artifact_to_project(&engine, artifact_id.clone(), project.id.clone())
        .expect("link should succeed");

    // Exactly one active join row, no new artifact rows, content untouched.
    assert_eq!(count_active_links(&engine, &project.id, &artifact_id), 1);
    assert_eq!(
        count_all_artifacts(&engine),
        artifacts_before,
        "linking must not duplicate the artifact row"
    );

    let after = engine.get_artifact(&artifact_id).unwrap();
    assert_eq!(after.content_md, before.content_md);
    assert_eq!(after.content_hash, before.content_hash);
    assert_eq!(after.version, before.version);

    // Idempotent: linking again does not create a second row.
    handle_link_artifact_to_project(&engine, artifact_id.clone(), project.id.clone())
        .expect("re-link should be a no-op");
    assert_eq!(count_active_links(&engine, &project.id, &artifact_id), 1);

    // Linking a non-existent artifact fails loudly instead of writing a dangling row.
    assert!(handle_link_artifact_to_project(&engine, "does-not-exist".into(), project.id).is_err());
}

// ---------------------------------------------------------------------------
// 2. cmd_create_project_with_artifact: creates the project AND associates the note.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_create_project_with_artifact_creates_project_and_associates_note() {
    let (_tmp, engine) = setup_test_engine();

    let artifact_id = make_note(&engine, "Sparse Attention", "# Sparse Attention\n\nBody.");

    let new_project_id = handle_create_project_with_artifact(
        &engine,
        "  Sparse Attention Deep Dive  ".into(),
        artifact_id.clone(),
    )
    .expect("create_project_with_artifact should succeed");

    // Project exists with a trimmed name and records its originating artifact.
    {
        let reader = engine.db().reader().unwrap();
        let (name, origin): (String, Option<String>) = reader
            .query_row(
                "SELECT name, origin_artifact_id FROM projects WHERE id = ?1;",
                rusqlite::params![new_project_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("project row must exist");
        assert_eq!(name, "Sparse Attention Deep Dive");
        assert_eq!(origin.as_deref(), Some(artifact_id.as_str()));
    }

    // Note is actively associated.
    assert_eq!(count_active_links(&engine, &new_project_id, &artifact_id), 1);

    // An empty name is rejected.
    assert!(
        handle_create_project_with_artifact(&engine, "   ".into(), artifact_id).is_err(),
        "blank project name must be rejected"
    );
}

// ---------------------------------------------------------------------------
// 3. cmd_duplicate_artifact: independent note, fresh UUID, optional title/project.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_duplicate_artifact_produces_independent_note_with_fresh_uuid() {
    let (_tmp, engine) = setup_test_engine();

    let project = handle_create_project(&engine, "Alpha".into(), None).unwrap();
    let original_id = make_note(&engine, "Alpha Note", "# Alpha Note\n\nOriginal body.");
    handle_link_artifact_to_project(&engine, original_id.clone(), project.id.clone()).unwrap();

    let before_count = count_all_artifacts(&engine);

    // Default title + inherit the source's project association.
    let copy_id =
        handle_duplicate_artifact(&engine, original_id.clone(), None, None).expect("duplicate ok");

    assert_ne!(copy_id, original_id, "duplicate must get a fresh UUID");
    assert_eq!(count_all_artifacts(&engine), before_count + 1);

    let copy = engine.get_artifact(&copy_id).unwrap();
    let original = engine.get_artifact(&original_id).unwrap();
    assert_eq!(copy.title.as_deref(), Some("Alpha Note (Copy)"));
    assert_eq!(copy.content_md, original.content_md);
    assert_eq!(copy.content_hash, original.content_hash);

    // Copy is attached to the same project.
    assert_eq!(count_active_links(&engine, &project.id, &copy_id), 1);

    // Editing the copy must not affect the original (independent rows + vault files).
    engine
        .update_artifact(
            &copy_id,
            "# Alpha Note (Copy)\n\nDiverged.",
            sthira_core::Actor::User,
            None,
        )
        .unwrap();
    let original_after = engine.get_artifact(&original_id).unwrap();
    assert_eq!(original_after.content_md, original.content_md);
    assert_ne!(
        engine.get_artifact(&copy_id).unwrap().content_hash,
        original.content_hash
    );

    // Explicit title + explicit target project.
    let other = handle_create_project(&engine, "Beta".into(), None).unwrap();
    let named_id = handle_duplicate_artifact(
        &engine,
        original_id.clone(),
        Some("Renamed Copy".into()),
        Some(other.id.clone()),
    )
    .unwrap();
    assert_eq!(
        engine.get_artifact(&named_id).unwrap().title.as_deref(),
        Some("Renamed Copy")
    );
    assert_eq!(count_active_links(&engine, &other.id, &named_id), 1);
}


// ---------------------------------------------------------------------------
// 4. cmd_convert_artifact_to_object: writes object_instances, never touches the body.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_convert_artifact_to_object_creates_instance_and_preserves_content() {
    let (_tmp, engine) = setup_test_engine();

    handle_create_object_type(
        &engine,
        ObjectType {
            id: "experiment".into(),
            display_name: "Experiment".into(),
            icon: "🧪".into(),
            color: "#8B5CF6".into(),
            schema: vec![
                PropertyDefinition {
                    key: "hypothesis".into(),
                    label: "Hypothesis".into(),
                    property_type: PropertyType::Text,
                    options: vec![],
                    required: false,
                    default_value: Some(serde_json::json!("")),
                },
                PropertyDefinition {
                    key: "status".into(),
                    label: "Status".into(),
                    property_type: PropertyType::Select,
                    options: vec!["proposed".into(), "running".into(), "concluded".into()],
                    required: true,
                    default_value: Some(serde_json::json!("proposed")),
                },
            ],
            is_system: false,
            created_at: 1_700_000_000_000,
            description: String::new(),
            default_template_md: String::new(),
        },
    )
    .unwrap();

    let artifact_id = make_note(
        &engine,
        "Latency Probe",
        "# Latency Probe\n\n## Method\n\nMeasure p99 over 10k requests.",
    );
    let before = engine.get_artifact(&artifact_id).unwrap();

    handle_convert_artifact_to_object(
        &engine,
        artifact_id.clone(),
        "experiment".into(),
        Some(serde_json::json!({ "hypothesis": "p99 < 20ms" })),
    )
    .expect("convert should succeed");

    // 1. object_instances row exists with schema defaults + supplied initial properties.
    {
        let reader = engine.db().reader().unwrap();
        let instance = sthira_core::db::objects_repo::get_object_instance(&reader, &artifact_id)
            .unwrap()
            .expect("object instance must exist");
        assert_eq!(instance.object_type_id, "experiment");
        assert_eq!(
            instance.properties.get("status").and_then(|v| v.as_str()),
            Some("proposed")
        );
        assert_eq!(
            instance.properties.get("hypothesis").and_then(|v| v.as_str()),
            Some("p99 < 20ms")
        );
    }

    // 2. The Markdown body is byte-identical — no re-extraction, no version bump.
    let after = engine.get_artifact(&artifact_id).unwrap();
    assert_eq!(after.content_md, before.content_md);
    assert_eq!(after.content_hash, before.content_hash);
    assert_eq!(after.version, before.version);

    // 3. Re-converting preserves user-set property values.
    handle_convert_artifact_to_object(&engine, artifact_id.clone(), "experiment".into(), None)
        .unwrap();
    {
        let reader = engine.db().reader().unwrap();
        let instance = sthira_core::db::objects_repo::get_object_instance(&reader, &artifact_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            instance.properties.get("hypothesis").and_then(|v| v.as_str()),
            Some("p99 < 20ms"),
            "re-converting must not wipe stored property values"
        );
    }

    // 4. Unknown object type is rejected.
    assert!(
        handle_convert_artifact_to_object(&engine, artifact_id, "nope".into(), None).is_err()
    );
}


// ---------------------------------------------------------------------------
// 5. cmd_move_artifact_to_project / cmd_detach_artifact_from_project
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_move_and_detach_artifact_between_projects() {
    let (_tmp, engine) = setup_test_engine();

    let p1 = handle_create_project(&engine, "Project One".into(), None).unwrap();
    let p2 = handle_create_project(&engine, "Project Two".into(), None).unwrap();
    let artifact_id = make_note(&engine, "Roaming Note", "# Roaming Note\n\nBody.");

    handle_link_artifact_to_project(&engine, artifact_id.clone(), p1.id.clone()).unwrap();
    assert_eq!(count_active_links(&engine, &p1.id, &artifact_id), 1);

    // Move p1 -> p2 in one transaction.
    handle_move_artifact_to_project(
        &engine,
        artifact_id.clone(),
        Some(p1.id.clone()),
        p2.id.clone(),
    )
    .expect("move should succeed");

    assert_eq!(
        count_active_links(&engine, &p1.id, &artifact_id),
        0,
        "source project link must be inactive after a move"
    );
    assert_eq!(count_active_links(&engine, &p2.id, &artifact_id), 1);

    // Soft detach preserves the row (historical context) while deactivating the link.
    {
        let reader = engine.db().reader().unwrap();
        let detached_at: Option<i64> = reader
            .query_row(
                "SELECT detached_at FROM project_artifacts WHERE project_id = ?1 AND artifact_id = ?2;",
                rusqlite::params![p1.id, artifact_id],
                |r| r.get(0),
            )
            .expect("row must still exist after a soft detach");
        assert!(
            detached_at.is_some(),
            "move must stamp detached_at rather than deleting the row"
        );
    }

    // Move from standalone (no source project) into a project.
    let standalone_id = make_note(&engine, "Loose Note", "# Loose Note\n\nBody.");
    handle_move_artifact_to_project(&engine, standalone_id.clone(), None, p2.id.clone()).unwrap();
    assert_eq!(count_active_links(&engine, &p2.id, &standalone_id), 1);

    // Detach returns the note to the standalone pool.
    handle_detach_artifact_from_project(&engine, standalone_id.clone(), p2.id.clone())
        .expect("detach should succeed");
    assert_eq!(count_active_links(&engine, &p2.id, &standalone_id), 0);

    // Detaching an unlinked note is an error, not a silent no-op.
    assert!(
        handle_detach_artifact_from_project(&engine, standalone_id, p2.id).is_err(),
        "detaching a note that is not linked must fail"
    );

    // Re-linking a previously detached note reactivates the same row (no duplicate).
    handle_link_artifact_to_project(&engine, artifact_id.clone(), p1.id.clone()).unwrap();
    assert_eq!(count_active_links(&engine, &p1.id, &artifact_id), 1);
}

