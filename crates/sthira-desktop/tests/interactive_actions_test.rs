use std::sync::Arc;
use serde_json::json;
use sthira_core::{
    models::objects::ObjectType,
    Engine,
};
use sthira_desktop::{
    handle_create_object_type, handle_create_page, handle_create_project,
    handle_create_typed_object, handle_get_workspace_tree, handle_list_object_types,
    handle_list_objects_by_type, handle_save_artifact, handle_rename_artifact,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

fn setup_test_engine() -> (tempfile::TempDir, Arc<Engine>) {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("interactive_test.db");
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
async fn test_save_artifact_direct_and_title_extraction() {
    let (_tmp, engine) = setup_test_engine();

    // 1. Create a brand new artifact via handle_save_artifact with empty artifact_id
    let new_art = handle_save_artifact(
        &engine,
        "".to_string(),
        "# First Heading Note\n\nBody content of the note.".to_string(),
        None,
        None,
    )
    .expect("handle_save_artifact should create new note if id is empty");

    assert!(!new_art.id.is_empty());
    assert_eq!(new_art.title.as_deref(), Some("First Heading Note"));

    // Verify content on disk / via engine
    let content = engine
        .get_artifact(&new_art.id)
        .expect("Should read saved content")
        .content_md;
    assert_eq!(content, "# First Heading Note\n\nBody content of the note.");

    // 2. Update existing artifact with an explicit title override
    let updated_art = handle_save_artifact(
        &engine,
        new_art.id.clone(),
        "# Renamed Note Title\n\nUpdated body lines.".to_string(),
        Some("Explicit Title Override".to_string()),
        None,
    )
    .expect("handle_save_artifact should update existing note");

    assert_eq!(updated_art.id, new_art.id);

    // Verify in SQLite directly that title column was updated
    let reader = engine.db().reader().unwrap();
    let title_in_db: String = reader
        .query_row(
            "SELECT title FROM artifacts WHERE id = ?1;",
            rusqlite::params![new_art.id],
            |r| r.get(0),
        )
        .expect("Querying title from artifacts");

    assert_eq!(title_in_db, "Explicit Title Override");

    // Verify content reflects updated note and synced heading
    let updated_content = engine
        .get_artifact(&new_art.id)
        .expect("Should read updated content")
        .content_md;
    assert_eq!(updated_content, "# Explicit Title Override\n\nUpdated body lines.");
}

#[tokio::test]
async fn test_create_project_and_add_page_roundtrip() {
    let (_tmp, engine) = setup_test_engine();

    // 1. Create project via handler
    let project = handle_create_project(
        &engine,
        "Autonomous System Design".to_string(),
        Some("Build self-verifying test harnesses".to_string()),
    )
    .expect("handle_create_project failed");

    assert_eq!(project.name, "Autonomous System Design");
    assert_eq!(project.purpose, "Build self-verifying test harnesses");
    assert!(!project.id.is_empty());

    // 2. Add page inside this project
    let page = handle_create_page(
        &engine,
        Some(project.id.clone()),
        Some("Harness Specifications".to_string()),
    )
    .expect("handle_create_page failed");

    assert_eq!(page.title.as_deref(), Some("Harness Specifications"));

    // 3. Verify workspace tree reflects the newly added page inside the project
    let tree = handle_get_workspace_tree(&engine).expect("handle_get_workspace_tree failed");
    let tree_proj = tree
        .projects
        .iter()
        .find(|p| p.id == project.id)
        .expect("Created project must be present in workspace tree");

    assert_eq!(tree_proj.pages.len(), 1);
    assert_eq!(tree_proj.pages[0].id, page.id);
    assert_eq!(tree_proj.pages[0].title, "Harness Specifications");

    // 4. Verify in SQLite that project_artifacts table was populated correctly
    let reader = engine.db().reader().unwrap();
    let attached_count: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM project_artifacts WHERE project_id = ?1 AND artifact_id = ?2 AND detached_at IS NULL;",
            rusqlite::params![project.id, page.id],
            |r| r.get(0),
        )
        .expect("Querying project_artifacts");
    assert_eq!(attached_count, 1);
}

#[tokio::test]
async fn test_create_object_type_persistence() {
    let (_tmp, engine) = setup_test_engine();

    // 1. Create custom object type definition
    let custom_type = ObjectType {
        id: "bug_report".to_string(),
        display_name: "Bug Report".to_string(),
        icon: "🐛".to_string(),
        color: "#DC2626".to_string(),
        schema: vec![
            sthira_core::models::objects::PropertyDefinition {
                key: "severity".to_string(),
                label: "Severity".to_string(),
                property_type: sthira_core::models::objects::PropertyType::Select,
                options: vec!["low".to_string(), "medium".to_string(), "critical".to_string()],
                required: false,
                default_value: None,
            },
            sthira_core::models::objects::PropertyDefinition {
                key: "reproducible".to_string(),
                label: "Reproducible".to_string(),
                property_type: sthira_core::models::objects::PropertyType::Boolean,
                options: vec![],
                required: false,
                default_value: None,
            },
        ],
        is_system: false,
        created_at: 1_700_000_000_000,
        description: String::new(),
        default_template_md: String::new(),
    };

    let created_id = handle_create_object_type(&engine, custom_type.clone())
        .expect("handle_create_object_type failed");
    assert_eq!(created_id, "bug_report");

    // 2. Query back via handle_list_object_types
    let types = handle_list_object_types(&engine).expect("handle_list_object_types failed");
    let fetched = types
        .iter()
        .find(|t| t.id == "bug_report")
        .expect("Created object type must exist in list");

    assert_eq!(fetched.display_name, "Bug Report");
    assert_eq!(fetched.icon, "🐛");
    assert_eq!(fetched.color, "#DC2626");
    assert!(!fetched.is_system);

    // 3. Query directly in SQLite to guarantee schema & row persistence
    let reader = engine.db().reader().unwrap();
    let (db_name, db_icon, is_sys): (String, String, i32) = reader
        .query_row(
            "SELECT display_name, icon, is_system FROM object_types WHERE id = ?1;",
            rusqlite::params!["bug_report"],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("Querying object_types from SQLite");

    assert_eq!(db_name, "Bug Report");
    assert_eq!(db_icon, "🐛");
    assert_eq!(is_sys, 0);

    // 4. Create an instance of this typed object
    let item = handle_create_typed_object(
        &engine,
        "bug_report".to_string(),
        "UI Button Disconnect".to_string(),
        json!({
            "severity": "critical",
            "reproducible": true
        }),
        None,
    )
    .expect("handle_create_typed_object failed");

    assert_eq!(item.object_type_id, "bug_report");
    assert_eq!(item.title, "UI Button Disconnect");

    // Verify instance querying
    let instances = handle_list_objects_by_type(&engine, "bug_report".to_string())
        .expect("handle_list_objects_by_type failed");
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].artifact_id, item.artifact_id);
    assert_eq!(instances[0].properties["severity"], "critical");
    assert_eq!(instances[0].properties["reproducible"], true);
}

#[tokio::test]
async fn test_save_artifact_with_sentinel_or_invalid_project_id_no_foreign_key_failure() {
    let (_tmp, engine) = setup_test_engine();

    // 1. Create a page with "default_project" as project_id
    let page = handle_create_page(
        &engine,
        Some("default_project".to_string()),
        Some("Captured Thought Test".to_string()),
    )
    .expect("handle_create_page with default_project must succeed without foreign key error");

    // 2. Save the artifact with "default_project" as project_id
    let saved = handle_save_artifact(
        &engine,
        page.id.clone(),
        "# Captured Thought Test\n\nContent captured without a project.".to_string(),
        Some("Captured Thought Test".to_string()),
        Some("default_project".to_string()),
    )
    .expect("handle_save_artifact with default_project must succeed without foreign key error");

    assert_eq!(saved.id, page.id);

    // 3. Save a note with an arbitrary non-existent project_id
    let loose = handle_save_artifact(
        &engine,
        "".to_string(),
        "# Loose Note\n\nLoose capture content.".to_string(),
        None,
        Some("non_existent_fake_proj_id".to_string()),
    )
    .expect("handle_save_artifact with non-existent project id must succeed without foreign key error");

    assert!(!loose.id.is_empty());

    // 4. Update the loose note with non_existent project_id
    let updated_loose = handle_save_artifact(
        &engine,
        loose.id.clone(),
        "# Loose Note\n\nUpdated loose content.".to_string(),
        None,
        Some("non_existent_fake_proj_id".to_string()),
    )
    .expect("handle_save_artifact update with non-existent project id must succeed");

    assert_eq!(updated_loose.id, loose.id);
}

#[tokio::test]
async fn test_rename_artifact_synchronizes_title_heading_and_vault() {
    let (_tmp, engine) = setup_test_engine();

    let page = handle_create_page(&engine, None, Some("Original Note Title".to_string()))
        .expect("create page");

    assert_eq!(page.title.as_deref(), Some("Original Note Title"));

    // Rename artifact via handle_rename_artifact
    handle_rename_artifact(&engine, page.id.clone(), "Brand New Title".to_string())
        .expect("rename artifact");

    let updated = engine.get_artifact(&page.id).expect("get artifact");
    assert_eq!(updated.title.as_deref(), Some("Brand New Title"));
    assert!(
        updated.content_md.starts_with("# Brand New Title"),
        "Content heading should be updated: {}",
        updated.content_md
    );
}
