use std::sync::Arc;
use sthira_core::{
    ArtifactType, Authorship, Engine,
};
use sthira_desktop::{
    handle_get_sidebar_items, handle_toggle_pin, handle_get_workspace_index,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

#[tokio::test]
async fn test_pinning_prioritizes_item_at_top_of_sidebar() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test_pinning.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    // Create 3 projects
    let p1 = engine.create_project("Alpha Project", None).unwrap();
    let _p2 = engine.create_project("Beta Project", None).unwrap();
    let _p3 = engine.create_project("Gamma Project", None).unwrap();

    // Initially, Gamma was created last so it's top
    let initial_sidebar = handle_get_sidebar_items(&engine).unwrap();
    assert_eq!(initial_sidebar.projects.len(), 3);
    assert!(!initial_sidebar.projects[0].is_pinned);

    // Pin Alpha Project
    handle_toggle_pin(&engine, p1.id.clone(), "project".to_string(), true).unwrap();

    let updated_sidebar = handle_get_sidebar_items(&engine).unwrap();
    assert_eq!(updated_sidebar.projects.len(), 3);
    assert_eq!(updated_sidebar.projects[0].id, p1.id);
    assert!(updated_sidebar.projects[0].is_pinned);
    assert_eq!(updated_sidebar.projects[0].title, "Alpha Project");

    // Also test pinning a note
    let n1 = engine.create_artifact(ArtifactType::Note, Authorship::User, Some("Note 1"), "Content 1", None).unwrap();
    let n2 = engine.create_artifact(ArtifactType::Note, Authorship::User, Some("Note 2"), "Content 2", None).unwrap();

    handle_toggle_pin(&engine, n1.id.clone(), "note".to_string(), true).unwrap();
    let note_sidebar = handle_get_sidebar_items(&engine).unwrap();
    assert_eq!(note_sidebar.notes[0].id, n1.id);
    assert!(note_sidebar.notes[0].is_pinned);
    assert_eq!(note_sidebar.notes[1].id, n2.id);
    assert!(!note_sidebar.notes[1].is_pinned);
}

#[tokio::test]
async fn test_7_item_cap_enforcement_max_5_pinned() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test_cap.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    // Create 8 projects and pin all 8
    for i in 1..=8 {
        let p = engine.create_project(&format!("Pinned Project {}", i), None).unwrap();
        handle_toggle_pin(&engine, p.id, "project".to_string(), true).unwrap();
    }

    // Create 5 unpinned projects
    for i in 1..=5 {
        engine.create_project(&format!("Unpinned Project {}", i), None).unwrap();
    }

    // Get sidebar items
    let sidebar = handle_get_sidebar_items(&engine).unwrap();

    // Must cap at exactly 7 items
    assert_eq!(sidebar.projects.len(), 7, "Should enforce a strict 7-item cap");

    // Exactly 5 should be pinned, and 2 should be unpinned
    let pinned_count = sidebar.projects.iter().filter(|p| p.is_pinned).count();
    let unpinned_count = sidebar.projects.iter().filter(|p| !p.is_pinned).count();

    assert_eq!(pinned_count, 5, "Should cap pinned items at max 5");
    assert_eq!(unpinned_count, 2, "Remaining 2 slots should be unpinned recent items");
}

#[tokio::test]
async fn test_workspace_index_filters_standalone_notes_correctly() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test_ws_index.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    // Create a Project
    let proj = engine.create_project("Project Apollo", None).unwrap();

    // Create Note 1 and attach to Project Apollo
    let note_attached = engine.create_artifact(
        ArtifactType::Note,
        Authorship::User,
        Some("Apollo Brief"),
        "Mission details",
        None,
    ).unwrap();
    engine.attach_artifact(&proj.id, &note_attached.id, "user_explicit", None).unwrap();

    // Create Note 2 as standalone (loose note)
    let note_standalone = engine.create_artifact(
        ArtifactType::Note,
        Authorship::User,
        Some("Random Shower Thought"),
        "A note not attached to any project",
        None,
    ).unwrap();

    // 1. Check workspace index for tab: "standalone"
    let index_standalone = handle_get_workspace_index(&engine, Some("standalone".to_string()), None).unwrap();
    assert_eq!(index_standalone.counts.standalone, 1);
    assert_eq!(index_standalone.counts.notes, 2);
    assert_eq!(index_standalone.counts.projects, 1);

    // Items list should contain ONLY the standalone note
    assert_eq!(index_standalone.items.len(), 1);
    assert_eq!(index_standalone.items[0].id, note_standalone.id);
    assert!(index_standalone.items[0].standalone);
    assert!(index_standalone.items[0].project_id.is_none());

    // 2. Check tab: "notes"
    let index_notes = handle_get_workspace_index(&engine, Some("notes".to_string()), None).unwrap();
    assert_eq!(index_notes.items.len(), 2);
    let attached_item = index_notes.items.iter().find(|i| i.id == note_attached.id).unwrap();
    assert!(!attached_item.standalone);
    assert_eq!(attached_item.project_id.as_deref(), Some(proj.id.as_str()));

    let standalone_item = index_notes.items.iter().find(|i| i.id == note_standalone.id).unwrap();
    assert!(standalone_item.standalone);
    assert!(standalone_item.project_id.is_none());

    // 3. Check search filter
    let index_searched = handle_get_workspace_index(&engine, Some("all".to_string()), Some("Shower".to_string())).unwrap();
    assert_eq!(index_searched.items.len(), 1);
    assert_eq!(index_searched.items[0].id, note_standalone.id);
}
