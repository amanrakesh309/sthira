use std::sync::Arc;
use sthira_core::{
    ArtifactType, Authorship, CanvasItemKind, Engine, ObjectType, PropertyDefinition, PropertyType,
};
use sthira_desktop::*;
use sthira_testkit::{DeterministicFakeModel, FakeClock};

fn prepare_fixture_dir(path: &std::path::Path) -> std::io::Result<()> {
    if path.exists() && std::fs::read_dir(path)?.next().is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "QA fixture target must be new or empty; existing data will not be replaced",
        ));
    }
    std::fs::create_dir_all(path)
}

#[test]
fn retained_fixture_rejects_existing_data_without_removing_it() {
    let existing = tempfile::tempdir().unwrap();
    let marker = existing.path().join("keep.md");
    std::fs::write(&marker, "Existing workspace content").unwrap();
    assert_eq!(prepare_fixture_dir(existing.path()).unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "Existing workspace content");
}

#[tokio::test]
async fn test_qa_deterministic_fixture_seed_and_restart() {
    // Normal test runs leave no repository data. Retention requires a new/empty
    // explicit target and uses the same database/vault layout as native startup.
    let temporary_dir = tempfile::tempdir().unwrap();
    let retained = std::env::var_os("STHIRA_QA_VAULT_DIR");
    let retain_dir = retained.as_ref().map(std::path::PathBuf::from)
        .unwrap_or_else(|| temporary_dir.path().join("workspace"));
    prepare_fixture_dir(&retain_dir).expect("Refusing to overwrite a QA fixture target");
    let db_path = retain_dir.join("sthira.db");
    let vault_dir = retain_dir.clone();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("qa-eval-model"));

    let prj_a_id;
    let prj_b_id;
    let prj_empty_id;
    let prj_source_only_id;
    let page_arch_id;
    let page_roadmap_id;
    let page_research_id;
    let page_shared_id;
    let page_ext_ref_id;
    let src_wal_id;
    let canvas_id;

    // --- PHASE 1: Populate QA Workspace ---
    {
        let engine = Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap();

        // 1. Projects
        let p_a = engine.create_project("Knowledge platform", None).unwrap();
        prj_a_id = p_a.id.clone();
        engine.set_project_purpose(&prj_a_id, "Build a connected, local-first knowledge workspace for high-density research and thought.").unwrap();

        let p_b = engine.create_project("Research library", None).unwrap();
        prj_b_id = p_b.id.clone();
        engine.set_project_purpose(&prj_b_id, "Durable repository of external specifications, papers, and benchmark quotes.").unwrap();

        let p_empty = engine.create_project("Draft Exploration", None).unwrap();
        prj_empty_id = p_empty.id.clone();

        let p_src = engine.create_project("External Standards", None).unwrap();
        prj_source_only_id = p_src.id.clone();
        engine.set_project_purpose(&prj_source_only_id, "Collection of canonical specifications without internal user notes.").unwrap();

        // 2. Member Notes for Project A
        let p1 = engine.create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Architecture & design"),
            "# Architecture & design\n\nCore local-first architecture. We will use SQLite for local storage.",
            None,
        ).unwrap();
        page_arch_id = p1.id.clone();
        engine.attach_artifact(&prj_a_id, &page_arch_id, "user_explicit", None).unwrap();

        let p2 = engine.create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Implementation roadmap"),
            "# Implementation roadmap\n\n- [x] Phase 1 Storage\n- [ ] Phase 2 Visual Refactor",
            None,
        ).unwrap();
        page_roadmap_id = p2.id.clone();
        engine.attach_artifact(&prj_a_id, &page_roadmap_id, "user_explicit", None).unwrap();

        let p3 = engine.create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Reading & research"),
            "# Reading & research\n\nNotes from external database whitepapers.",
            None,
        ).unwrap();
        page_research_id = p3.id.clone();
        engine.attach_artifact(&prj_a_id, &page_research_id, "user_explicit", None).unwrap();

        // 3. Shared Page attached to both Project A and Project B
        let p_shared = engine.create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Shared guidelines"),
            "# Shared guidelines\n\nUniversal epistemic guidelines across projects.",
            None,
        ).unwrap();
        page_shared_id = p_shared.id.clone();
        engine.attach_artifact(&prj_a_id, &page_shared_id, "user_explicit", None).unwrap();
        engine.attach_artifact(&prj_b_id, &page_shared_id, "user_explicit", None).unwrap();

        // 4. External Page owned by Project B, referenced by Project A
        let p_ext = engine.create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("External spec reference"),
            "# External spec reference\n\nOwned by Research library, referenced from Knowledge platform.",
            None,
        ).unwrap();
        page_ext_ref_id = p_ext.id.clone();
        engine.attach_artifact(&prj_b_id, &page_ext_ref_id, "user_explicit", None).unwrap();

        // Explicit link: Project A references Project B's note
        handle_create_workspace_connection(&engine, EntityRef { kind: EntityKind::Project, id: prj_a_id.clone() }, EntityRef { kind: EntityKind::Artifact, id: page_ext_ref_id.clone() }).unwrap();

        // Explicit link: Project A references Project B directly
        handle_create_workspace_connection(&engine, EntityRef { kind: EntityKind::Project, id: prj_a_id.clone() }, EntityRef { kind: EntityKind::Project, id: prj_b_id.clone() }).unwrap();

        // 5. Source Artifacts
        let src = engine.create_artifact(
            ArtifactType::Source,
            Authorship::External,
            Some("SQLite WAL Mode Specification"),
            "# SQLite WAL Mode Specification\n\nWrite-Ahead Logging provides significantly higher concurrency.",
            Some("https://sqlite.org/wal.html"),
        ).unwrap();
        src_wal_id = src.id.clone();
        engine.attach_artifact(&prj_source_only_id, &src_wal_id, "user_explicit", None).unwrap();

        // 6. Custom Typed Object: Benchmark
        handle_create_object_type(
            &engine,
            ObjectType {
                id: "benchmark".into(),
                display_name: "Benchmark".into(),
                icon: "⚡".into(),
                color: "#BE8B41".into(),
                schema: vec![
                    PropertyDefinition {
                        key: "metric".into(),
                        label: "Metric".into(),
                        property_type: PropertyType::Text,
                        options: vec![],
                        required: true,
                        default_value: Some(serde_json::json!("p99_read_ms")),
                    },
                    PropertyDefinition {
                        key: "value".into(),
                        label: "Value".into(),
                        property_type: PropertyType::Number,
                        options: vec![],
                        required: true,
                        default_value: Some(serde_json::json!(4.2)),
                    },
                ],
                is_system: false,
                created_at: 1_700_000_000_000,
                description: "Latency and throughput benchmark measurements".into(),
                default_template_md: String::new(),
            },
        ).unwrap();

        let obj = handle_create_typed_object(
            &engine,
            "benchmark".into(),
            "Cold-start latency benchmark".into(),
            serde_json::json!({ "metric": "p99_read_ms", "value": 4.2 }),
            Some(prj_a_id.clone()),
        ).unwrap();

        // 7. Spatial Canvas
        let cv = engine.create_canvas(Some(&prj_a_id), "Workspace Architecture Map").unwrap();
        canvas_id = cv;
        engine.add_canvas_item(
            &canvas_id,
            "card-1",
            CanvasItemKind::Artifact,
            Some(&page_arch_id),
            80.0,
            120.0,
            Some(280.0),
            Some(140.0),
            1,
        ).unwrap();
        engine.add_canvas_item(
            &canvas_id,
            "card-2",
            CanvasItemKind::Artifact,
            Some(&obj.artifact_id),
            400.0,
            120.0,
            Some(280.0),
            Some(140.0),
            2,
        ).unwrap();
    }

    // --- PHASE 2: Reopen Workspace and Assert Parity Across Restart ---
    {
        let engine2 = Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap();

        // Assert Project A Overview counts and deduplication
        let overview_a = handle_get_project_overview(&engine2, prj_a_id.clone()).unwrap();
        assert_eq!(overview_a.project.name, "Knowledge platform");
        assert_eq!(overview_a.project.purpose, "Build a connected, local-first knowledge workspace for high-density research and thought.");

        // In Project A:
        // Members: Architecture, Roadmap, Research, Shared guidelines, Benchmark object, Canvas = 6
        // Connected references: External spec reference (reference from B), Project B = 2
        let member_count = overview_a.items.iter().filter(|i| i.roles.contains(&"member".to_string())).count();
        let reference_count = overview_a.items.iter().filter(|i| i.roles.contains(&"reference".to_string())).count();
        assert_eq!(member_count, 6, "Expected 6 member items in Project A");
        assert_eq!(reference_count, 2, "Expected 2 explicit reference items in Project A");

        // Assert Shared Page is member in Project A and member in Project B
        let overview_b = handle_get_project_overview(&engine2, prj_b_id.clone()).unwrap();
        assert!(overview_a.items.iter().any(|i| i.id == page_shared_id && i.roles == ["member"]));
        assert!(overview_b.items.iter().any(|i| i.id == page_shared_id && i.roles == ["member"]));

        // Assert Project Empty has zero items
        let overview_empty = handle_get_project_overview(&engine2, prj_empty_id.clone()).unwrap();
        assert_eq!(overview_empty.items.len(), 0);

        // Assert Source-only Project contains WAL source
        let overview_src = handle_get_project_overview(&engine2, prj_source_only_id.clone()).unwrap();
        assert_eq!(overview_src.items.len(), 1);
        assert_eq!(overview_src.items[0].id, src_wal_id);

        println!("QA Fixture Seed and Restart PASSED:");
        println!("  Fixture root: {} ({})", retain_dir.display(), if retained.is_some() { "retained" } else { "temporary" });
        println!("  Project A ID: {}", prj_a_id);
        println!("  Project B ID: {}", prj_b_id);
        println!("  Shared Page ID: {}", page_shared_id);
        println!("  Source WAL ID: {}", src_wal_id);
        println!("  Canvas ID: {}", canvas_id);
    }
}
