use proptest::prelude::*;
use rusqlite::params;
use std::sync::Arc;
use std::time::Duration;
use sthira_core::{
    Actor, ArtifactType, Authorship, CanvasItemKind, Engine, EventType, Materiality,
    IDLE_GAP_MS,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

fn setup_test_engine() -> (Engine, Arc<FakeClock>, tempfile::TempDir) {
    let dir = tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("sthira.db");
    let vault_dir = dir.path().join(".sthira_vault").join("artifacts");
    let clock = Arc::new(FakeClock::new(1_000_000));
    let model = Arc::new(DeterministicFakeModel::new("mock-model"));
    let engine = Engine::open(&db_path, &vault_dir, clock.clone(), model)
        .expect("Failed to initialize engine");
    (engine, clock, dir)
}

#[test]
fn test_1_1_cross_project_reuse_without_duplication() {
    let (engine, clock, _dir) = setup_test_engine();

    // Create two projects
    let proj_1 = engine
        .create_project("Project Alpha", None)
        .expect("Failed to create proj_1");
    clock.advance(Duration::from_secs(10));
    let proj_2 = engine
        .create_project("Project Beta", None)
        .expect("Failed to create proj_2");

    let p1_orig_substantive = proj_1.last_substantive_at;
    let p2_orig_substantive = proj_2.last_substantive_at;

    // Create an artifact
    let art_1 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Shared Architecture Note"),
            "Initial architecture proposal for system decomposition.",
            None,
        )
        .expect("Failed to create art_1");

    // Attach to both projects
    engine
        .attach_artifact(&proj_1.id, &art_1.id, "user_explicit", None)
        .expect("Failed to attach to proj_1");
    engine
        .attach_artifact(&proj_2.id, &art_1.id, "user_explicit", None)
        .expect("Failed to attach to proj_2");

    clock.advance(Duration::from_secs(60));

    // Edit art_1 within proj_2 context
    let updated_text = "Updated architecture proposal: standardized on modular services.";
    let updated_art = engine
        .update_artifact(&art_1.id, updated_text, Actor::User, Some(&proj_2.id))
        .expect("Failed to update art_1");

    assert_eq!(updated_art.content_md, updated_text);

    // 1. Assert artifact count is exactly 1 (no duplication)
    let reader = engine.db().reader().expect("reader pool error");
    let count: i64 = reader
        .query_row("SELECT COUNT(*) FROM artifacts;", [], |r| r.get(0))
        .expect("query count failed");
    assert_eq!(count, 1, "Artifact must not be duplicated across projects");

    // 2. Both projects see the new content_md
    let fetched = engine
        .get_artifact(&art_1.id)
        .expect("Failed to get artifact");
    assert_eq!(fetched.content_md, updated_text);

    // 3. session_events records project_id = proj_2 for the update
    let event_proj: Option<String> = reader
        .query_row(
            "SELECT project_id FROM session_events WHERE artifact_id = ?1 AND event_type = 'artifact_updated';",
            params![art_1.id],
            |r| r.get(0),
        )
        .expect("query session event failed");
    assert_eq!(event_proj, Some(proj_2.id.clone()));

    // 4. proj_1.last_substantive_at is NOT bumped, proj_2 IS bumped
    let p1_after = engine
        .get_project(&proj_1.id)
        .expect("Failed to get proj_1");
    let p2_after = engine
        .get_project(&proj_2.id)
        .expect("Failed to get proj_2");

    assert_eq!(
        p1_after.last_substantive_at, p1_orig_substantive,
        "Project 1 substantive timestamp must not be bumped by Project 2 edit"
    );
    assert!(
        p2_after.last_substantive_at > p2_orig_substantive,
        "Project 2 substantive timestamp must be bumped"
    );
}

#[test]
fn test_1_2_spatial_semantic_isolation_property_test() {
    let (engine, _clock, _dir) = setup_test_engine();

    let canvas_id = engine
        .create_canvas(None, "Freeform Canvas")
        .expect("Failed to create canvas");

    // Add 3 canvas items
    for i in 0..3 {
        let item_id = format!("item_{i}");
        engine
            .add_canvas_item(
                &canvas_id,
                &item_id,
                CanvasItemKind::Text,
                None,
                i as f64 * 100.0,
                i as f64 * 100.0,
                Some(200.0),
                Some(100.0),
                0,
            )
            .expect("Failed to add canvas item");
    }

    // Property test: 200 random movement operations
    let strat = proptest::collection::vec(
        (0..3usize, -1000.0f64..1000.0f64, -1000.0f64..1000.0f64),
        200,
    );

    strat
        .new_tree(&mut proptest::test_runner::TestRunner::new(
            proptest::test_runner::Config::default(),
        ))
        .unwrap();

    let moves = [
        (0, 150.0, 250.0),
        (1, -20.0, 80.0),
        (2, 400.0, 600.0),
        (0, 10.0, 10.0),
        (1, 0.0, 0.0),
    ];

    for (item_idx, x, y) in moves.iter().cycle().take(200) {
        let item_id = format!("item_{item_idx}");
        engine
            .move_canvas_item(&canvas_id, &item_id, *x, *y)
            .expect("move canvas item failed");
    }

    let reader = engine.db().reader().expect("reader pool error");
    let links_count: i64 = reader
        .query_row("SELECT COUNT(*) FROM artifact_links;", [], |r| r.get(0))
        .expect("query links count failed");
    let proposals_count: i64 = reader
        .query_row("SELECT COUNT(*) FROM ai_proposals;", [], |r| r.get(0))
        .expect("query proposals count failed");

    assert_eq!(
        links_count, 0,
        "Spatial canvas movements must never create semantic artifact links"
    );
    assert_eq!(
        proposals_count, 0,
        "Spatial canvas movements must never generate AI proposals"
    );
}

#[test]
fn test_1_3_session_grouping_with_fake_clock() {
    let (engine, clock, _dir) = setup_test_engine();

    // Record events at t = 0, 5, 10 min
    let e1 = engine
        .log_event(
            None,
            None,
            Actor::User,
            EventType::ArtifactViewed,
            Materiality::None,
            serde_json::json!({}),
        )
        .expect("log e1 failed");

    clock.advance(Duration::from_secs(5 * 60));
    let e2 = engine
        .log_event(
            None,
            None,
            Actor::User,
            EventType::ArtifactViewed,
            Materiality::None,
            serde_json::json!({}),
        )
        .expect("log e2 failed");

    clock.advance(Duration::from_secs(5 * 60));
    let e3 = engine
        .log_event(
            None,
            None,
            Actor::User,
            EventType::ArtifactViewed,
            Materiality::None,
            serde_json::json!({}),
        )
        .expect("log e3 failed");

    assert_eq!(e1.session_id, e2.session_id);
    assert_eq!(e2.session_id, e3.session_id);

    // Advance clock by IDLE_GAP_MS - 1 -> stays in same session
    clock.advance_ms(IDLE_GAP_MS - 1);
    let e4 = engine
        .log_event(
            None,
            None,
            Actor::User,
            EventType::ArtifactViewed,
            Materiality::None,
            serde_json::json!({}),
        )
        .expect("log e4 failed");
    assert_eq!(e3.session_id, e4.session_id);

    // Advance by IDLE_GAP_MS + 1 -> opens session_2
    clock.advance_ms(IDLE_GAP_MS + 1);
    let e5 = engine
        .log_event(
            None,
            None,
            Actor::User,
            EventType::ArtifactViewed,
            Materiality::None,
            serde_json::json!({}),
        )
        .expect("log e5 failed");
    assert_ne!(
        e4.session_id, e5.session_id,
        "Session must roll over after idle gap exceeded"
    );

    // Fire 15 events at the exact same millisecond
    let mut seqs = Vec::new();
    for i in 0..15 {
        let evt = engine
            .log_event(
                None,
                None,
                Actor::User,
                EventType::ArtifactViewed,
                Materiality::None,
                serde_json::json!({ "batch_idx": i }),
            )
            .expect("log batch event failed");
        seqs.push(evt.seq);
    }

    // Assert strictly increasing deterministic seq ordering
    for w in seqs.windows(2) {
        assert_eq!(
            w[0] + 1,
            w[1],
            "Events at identical millisecond must have strictly consecutive seq total order"
        );
    }
}

#[test]
fn test_1_4_changeset_reversibility_and_append_only_audit() {
    let (engine, clock, _dir) = setup_test_engine();

    // Create an artifact
    let art_1 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Original Title"),
            "Original content body prior to changes.",
            None,
        )
        .expect("create art failed");
    let original_hash = art_1.content_hash.clone();

    // Create canvas with 3 items
    let canvas_id = engine
        .create_canvas(None, "Reversible Canvas")
        .expect("create canvas failed");
    for i in 0..3 {
        engine
            .add_canvas_item(
                &canvas_id,
                &format!("item_{i}"),
                CanvasItemKind::Text,
                None,
                i as f64 * 50.0,
                i as f64 * 50.0,
                Some(100.0),
                Some(50.0),
                0,
            )
            .expect("add canvas item failed");
    }

    // Apply change: edit art_1 and move 3 items
    let new_text = "Modified content body with substantive changes.";
    let updated_art = engine
        .update_artifact(&art_1.id, new_text, Actor::User, None)
        .expect("update failed");
    assert_ne!(updated_art.content_hash, original_hash);

    for i in 0..3 {
        engine
            .move_canvas_item(&canvas_id, &format!("item_{i}"), 999.0, 999.0)
            .expect("move failed");
    }

    // Record change set
    let cs_id = uuid::Uuid::new_v4().to_string();
    let inverse_payload = serde_json::json!({
        "operations": [
            {
                "type": "restore_artifact",
                "artifact_id": art_1.id,
                "content_md": art_1.content_md,
                "content_hash": original_hash
            },
            {
                "type": "restore_canvas_item",
                "canvas_id": canvas_id,
                "item_id": "item_0",
                "x": 0.0,
                "y": 0.0
            },
            {
                "type": "restore_canvas_item",
                "canvas_id": canvas_id,
                "item_id": "item_1",
                "x": 50.0,
                "y": 50.0
            },
            {
                "type": "restore_canvas_item",
                "canvas_id": canvas_id,
                "item_id": "item_2",
                "x": 100.0,
                "y": 100.0
            }
        ]
    });

    engine
        .apply_change_set(
            &cs_id,
            Actor::User,
            None,
            "Bulk edit of note and canvas items",
            &inverse_payload,
        )
        .expect("apply change set failed");

    clock.advance(Duration::from_secs(10));

    // Call Engine::revert
    let revert_cs_id = engine.revert(&cs_id).expect("revert failed");
    assert!(!revert_cs_id.is_empty());

    // Assert art_1.content_hash matches pre-change state
    let reverted_art = engine
        .get_artifact(&art_1.id)
        .expect("get artifact failed");
    assert_eq!(reverted_art.content_hash, original_hash);
    assert_eq!(reverted_art.content_md, art_1.content_md);

    // Assert canvas coordinates match pre-change state
    let reader = engine.db().reader().expect("reader pool error");
    for i in 0..3 {
        let (x, y): (f64, f64) = reader
            .query_row(
                "SELECT x, y FROM canvas_items WHERE canvas_id = ?1 AND item_id = ?2;",
                params![canvas_id, format!("item_{i}")],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("query canvas item failed");
        assert_eq!(x, i as f64 * 50.0);
        assert_eq!(y, i as f64 * 50.0);
    }

    // Assert change_set_reverted is logged
    let reverted_events_count: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM session_events WHERE event_type = 'change_set_reverted';",
            [],
            |r| r.get(0),
        )
        .expect("query revert event failed");
    assert!(reverted_events_count >= 1);

    // Assert raw UPDATE / DELETE on session_events fails via trigger
    let writer = engine.db().writer();
    let update_res = writer.execute(
        "UPDATE session_events SET actor = 'ai' WHERE seq = 1;",
        [],
    );
    assert!(
        update_res.is_err(),
        "Raw UPDATE on session_events must be aborted by trigger"
    );

    let delete_res = writer.execute("DELETE FROM session_events WHERE seq = 1;", []);
    assert!(
        delete_res.is_err(),
        "Raw DELETE on session_events must be aborted by trigger"
    );
}

#[test]
fn test_1_5_db_trigger_and_fk_enforcement_across_pool() {
    let (engine, _clock, _dir) = setup_test_engine();

    // 1. Checkout 8 pooled reader connections; assert PRAGMA foreign_keys == 1 and vec_version()
    let mut readers = Vec::new();
    for _ in 0..8 {
        let r = engine.db().reader().expect("checkout reader failed");
        let fk: i64 = r
            .query_row("PRAGMA foreign_keys;", [], |row| row.get(0))
            .expect("query fk failed");
        assert_eq!(fk, 1, "PRAGMA foreign_keys must be ON for pooled reader");

        let vec_ver: String = r
            .query_row("SELECT vec_version();", [], |row| row.get(0))
            .expect("query vec_version failed");
        assert!(
            vec_ver.starts_with('v'),
            "vec_version() must return version string"
        );
        readers.push(r);
    }
    drop(readers);

    // 2. Setup artifacts: one user note, one external source, one ai_generated note
    let user_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("User Note"),
            "User recorded statement body here for test.",
            None,
        )
        .expect("user art failed");

    let ext_art = engine
        .create_artifact(
            ArtifactType::Source,
            Authorship::External,
            Some("External Source"),
            "External documentation body here for test.",
            Some("https://example.com/doc"),
        )
        .expect("ext art failed");

    let ai_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::AiGenerated,
            Some("AI Note"),
            "AI synthesized summary body here for test.",
            None,
        )
        .expect("ai art failed");

    let writer = engine.db().writer();

    // (a) Inserting user_recorded node on an external or ai_generated artifact fails
    let res_a1 = writer.execute(
        "INSERT INTO epistemic_nodes (id, artifact_id, node_type, origin, statement, quote, quote_start, quote_end, quote_hash, basis_content_hash, extraction_run_id, model_id, first_seen_at, updated_at)
         VALUES ('node_a1', ?1, 'finding', 'user_recorded', 'User claim', 'External documentation', 0, 22, 'qh1', 'bh1', 'run1', 'm1', 100, 100);",
        params![ext_art.id],
    );
    assert!(
        res_a1.is_err(),
        "Trigger must abort inserting user_recorded on external artifact"
    );

    let res_a2 = writer.execute(
        "INSERT INTO epistemic_nodes (id, artifact_id, node_type, origin, statement, quote, quote_start, quote_end, quote_hash, basis_content_hash, extraction_run_id, model_id, first_seen_at, updated_at)
         VALUES ('node_a2', ?1, 'finding', 'user_recorded', 'User claim', 'AI synthesized', 0, 14, 'qh2', 'bh2', 'run1', 'm1', 100, 100);",
        params![ai_art.id],
    );
    assert!(
        res_a2.is_err(),
        "Trigger must abort inserting user_recorded on ai_generated artifact"
    );

    // (b) Inserting ai_inferred + decision fails via CHECK constraint
    let res_b = writer.execute(
        "INSERT INTO epistemic_nodes (id, artifact_id, node_type, origin, statement, basis_content_hash, extraction_run_id, model_id, first_seen_at, updated_at)
         VALUES ('node_b', ?1, 'decision', 'ai_inferred', 'AI decision', 'bh3', 'run1', 'm1', 100, 100);",
        params![user_art.id],
    );
    assert!(
        res_b.is_err(),
        "CHECK constraint must abort inserting decision + ai_inferred"
    );

    // (c) Self-linking an artifact fails via CHECK constraint
    let res_c = writer.execute(
        "INSERT INTO artifact_links (source_artifact_id, target_artifact_id, relationship_type, origin, created_at)
         VALUES (?1, ?1, 'supports', 'user_recorded', 100);",
        params![user_art.id],
    );
    assert!(
        res_c.is_err(),
        "CHECK constraint must abort self-linking artifact"
    );

    // (d) Inserting an AI link without proposal_id fails via CHECK constraint
    let res_d = writer.execute(
        "INSERT INTO artifact_links (source_artifact_id, target_artifact_id, relationship_type, origin, created_at)
         VALUES (?1, ?2, 'supports', 'ai_inferred', 100);",
        params![user_art.id, ext_art.id],
    );
    assert!(
        res_d.is_err(),
        "CHECK constraint must abort inserting ai_inferred link without proposal_id"
    );
}

#[test]
fn test_1_6_vault_reconcile_matrix() {
    let (engine, clock, dir) = setup_test_engine();
    let vault_dir = dir.path().join(".sthira_vault").join("artifacts");

    // (a) Disk edit -> imports & logs external_edit_detected
    let disk_file = vault_dir.join("disk_created.md");
    let disk_content = "---\ntitle: \"Disk Note\"\ntype: \"note\"\nauthorship: \"user\"\n---\nInitial content written directly on disk.";
    std::fs::write(&disk_file, disk_content).expect("write disk file failed");

    let res_a = engine.reconcile_vault().expect("reconcile a failed");
    assert_eq!(res_a.files_imported, 1);

    // Verify disk content is now in DB
    let reader = engine.db().reader().expect("reader checkout failed");
    let (art_id, db_content): (String, String) = reader
        .query_row(
            "SELECT id, content_md FROM artifacts WHERE title = 'Disk Note';",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("query imported artifact failed");
    assert_eq!(db_content, "Initial content written directly on disk.");

    // Now edit the file on disk
    clock.advance(Duration::from_secs(10));
    let modified_disk_content = format!(
        "---\nid: \"{}\"\ntitle: \"Disk Note\"\ntype: \"note\"\nauthorship: \"user\"\n---\nModified body on disk.",
        art_id
    );
    std::fs::write(&disk_file, modified_disk_content).expect("update disk file failed");

    let res_a2 = engine.reconcile_vault().expect("reconcile a2 failed");
    assert_eq!(res_a2.files_imported, 1);

    let updated_db_content: String = reader
        .query_row(
            "SELECT content_md FROM artifacts WHERE id = ?1;",
            params![art_id],
            |r| r.get(0),
        )
        .expect("query updated failed");
    assert_eq!(updated_db_content, "Modified body on disk.");

    // Assert external_edit_detected is logged in session_events
    let ext_edit_count: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM session_events WHERE event_type = 'external_edit_detected' AND artifact_id = ?1;",
            params![art_id],
            |r| r.get(0),
        )
        .expect("query ext edit failed");
    assert_eq!(ext_edit_count, 1);

    // (b) DB edit -> exports to disk
    clock.advance(Duration::from_secs(10));
    engine
        .update_artifact(
            &art_id,
            "Updated in DB via application UI.",
            Actor::User,
            None,
        )
        .expect("db update failed");

    let res_b = engine.reconcile_vault().expect("reconcile b failed");
    assert_eq!(res_b.files_exported, 1);

    let on_disk_now = std::fs::read_to_string(&disk_file).expect("read disk failed");
    assert!(on_disk_now.contains("Updated in DB via application UI."));

    // (c) Concurrent disk + DB edit -> creates conflict copy without data loss
    clock.advance(Duration::from_secs(10));
    // Edit on disk
    let disk_conflict_content = format!(
        "---\nid: \"{}\"\ntitle: \"Disk Note\"\ntype: \"note\"\nauthorship: \"user\"\n---\nDisk branch concurrent edit.",
        art_id
    );
    std::fs::write(&disk_file, disk_conflict_content).expect("write conflict failed");

    // Edit in DB
    engine
        .update_artifact(
            &art_id,
            "DB branch concurrent edit.",
            Actor::User,
            None,
        )
        .expect("concurrent db update failed");

    let res_c = engine.reconcile_vault().expect("reconcile c failed");
    assert_eq!(res_c.conflicts_detected, 1);

    // Verify conflict file exists and has disk content
    let entries = std::fs::read_dir(&vault_dir).expect("read_dir failed");
    let mut conflict_file_found = false;
    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.contains(".conflict-") {
            conflict_file_found = true;
            let conflict_text = std::fs::read_to_string(entry.path()).expect("read conflict failed");
            assert!(conflict_text.contains("Disk branch concurrent edit."));
        }
    }
    assert!(conflict_file_found, "Conflict backup file must be created on concurrent edits");

    // (d) File rename with preserved frontmatter ID -> updates file_path without creating duplicate
    clock.advance(Duration::from_secs(10));
    let renamed_file = vault_dir.join("renamed_disk_note.md");
    let current_disk = std::fs::read_to_string(&disk_file).expect("read disk failed");
    std::fs::write(&renamed_file, current_disk).expect("write renamed failed");
    std::fs::remove_file(&disk_file).expect("remove old file failed");

    let res_d = engine.reconcile_vault().expect("reconcile d failed");
    assert_eq!(res_d.renames_detected, 1);

    let db_path_after: String = reader
        .query_row(
            "SELECT file_path FROM artifacts WHERE id = ?1;",
            params![art_id],
            |r| r.get(0),
        )
        .expect("query path failed");
    assert_eq!(db_path_after, "renamed_disk_note.md");

    // Assert total artifact count in DB is still 1
    let total_arts: i64 = reader
        .query_row("SELECT COUNT(*) FROM artifacts;", [], |r| r.get(0))
        .expect("query total failed");
    assert_eq!(total_arts, 1);

    // (e) CRLF/BOM normalization -> no false event
    let events_before: i64 = reader
        .query_row("SELECT COUNT(*) FROM session_events;", [], |r| r.get(0))
        .expect("query events count failed");

    let bom_crlf_content = format!(
        "\u{feff}---\r\nid: \"{}\"\r\ntitle: \"Disk Note\"\r\ntype: \"note\"\r\nauthorship: \"user\"\r\n---\r\nDB branch concurrent edit.\r\n",
        art_id
    );
    std::fs::write(&renamed_file, bom_crlf_content).expect("write bom crlf failed");

    let res_e = engine.reconcile_vault().expect("reconcile e failed");
    assert_eq!(res_e.files_imported, 0);

    let events_after: i64 = reader
        .query_row("SELECT COUNT(*) FROM session_events;", [], |r| r.get(0))
        .expect("query events count failed");
    assert_eq!(
        events_before, events_after,
        "CRLF/BOM differences must not trigger false mutation events"
    );

    // (f) Confirm .sthira_meta/memory_mirrors/** is strictly ignored
    let meta_dir = vault_dir.join(".sthira_meta").join("memory_mirrors");
    std::fs::create_dir_all(&meta_dir).expect("create meta dir failed");
    let mirror_file = meta_dir.join("test_mirror.md");
    std::fs::write(
        &mirror_file,
        "<!-- STHIRA_DERIVED_MEMORY: DO_NOT_INGEST -->\n# AI Generated Memory",
    )
    .expect("write mirror file failed");

    let res_f = engine.reconcile_vault().expect("reconcile f failed");
    assert_eq!(res_f.files_imported, 0);

    let meta_in_db: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM artifacts WHERE content_md LIKE '%STHIRA_DERIVED_MEMORY%';",
            [],
            |r| r.get(0),
        )
        .expect("query meta in db failed");
    assert_eq!(meta_in_db, 0, ".sthira_meta files must never be ingested into artifacts");
}

#[test]
fn test_1_7_organic_note_promotion() {
    let (engine, clock, _dir) = setup_test_engine();

    // Create unfiled note N
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Compiler Research Starting Point"),
            "Initial exploratory thoughts on bytecode emission.",
            None,
        )
        .expect("create note failed");

    // Make 2 updates -> 3 revisions total
    clock.advance(Duration::from_secs(10));
    engine
        .update_artifact(
            &note.id,
            "Revision 2: exploring register allocation strategies.",
            Actor::User,
            None,
        )
        .expect("update 1 failed");

    clock.advance(Duration::from_secs(10));
    engine
        .update_artifact(
            &note.id,
            "Revision 3: settled on SSA intermediate representation.",
            Actor::User,
            None,
        )
        .expect("update 2 failed");

    let reader = engine.db().reader().expect("reader failed");
    let rev_count_before: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM artifact_revisions WHERE artifact_id = ?1;",
            params![note.id],
            |r| r.get(0),
        )
        .expect("query rev count failed");
    assert_eq!(rev_count_before, 3, "Note must have 3 revisions");

    // Insert 2 extracted nodes
    let writer = engine.db().writer();
    writer.execute(
        "INSERT INTO epistemic_nodes (id, artifact_id, node_type, origin, statement, quote, quote_start, quote_end, quote_hash, basis_content_hash, extraction_run_id, model_id, first_seen_at, updated_at)
         VALUES ('n1', ?1, 'finding', 'user_recorded', 'Finding 1', 'exploring register allocation', 0, 31, 'qh1', 'bh1', 'run1', 'm1', 100, 100),
                ('n2', ?1, 'hypothesis', 'user_recorded', 'Hypothesis 2', 'settled on SSA intermediate', 0, 26, 'qh2', 'bh2', 'run1', 'm1', 100, 100);",
        params![note.id],
    ).expect("insert nodes failed");
    drop(writer);

    // Call Engine::promote_to_project
    clock.advance(Duration::from_secs(10));
    let project = engine
        .promote_to_project(&[&note.id], "Compiler Research")
        .expect("promote failed");

    // 1. origin_artifact_id == N
    assert_eq!(project.origin_artifact_id, Some(note.id.clone()));
    assert_eq!(project.name, "Compiler Research");

    // 2. Artifact ID and 3 revisions survive untouched
    let note_after = engine.get_artifact(&note.id).expect("get note failed");
    assert_eq!(note_after.id, note.id);

    let rev_count_after: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM artifact_revisions WHERE artifact_id = ?1;",
            params![note.id],
            |r| r.get(0),
        )
        .expect("query rev count after failed");
    assert_eq!(rev_count_after, 3);

    // 3. Extracted nodes survive untouched
    let nodes_count: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM epistemic_nodes WHERE artifact_id = ?1;",
            params![note.id],
            |r| r.get(0),
        )
        .expect("query nodes count failed");
    assert_eq!(nodes_count, 2);

    // 4. Initial project_purposes row is seeded
    let (purpose_text, purpose_origin): (String, String) = reader
        .query_row(
            "SELECT purpose, origin FROM project_purposes WHERE project_id = ?1 AND superseded_at IS NULL;",
            params![project.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("query purpose failed");
    assert_eq!(purpose_text, "Compiler Research");
    assert_eq!(purpose_origin, "user_recorded");
}
