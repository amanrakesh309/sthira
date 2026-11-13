use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use sthira_core::{vault, Actor, ArtifactType, Authorship, Engine, SaveArtifactRequest};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

fn open_engine(dir: &std::path::Path) -> Engine {
    Engine::open(
        dir.join("sthira.db"),
        dir.join("vault"),
        Arc::new(FakeClock::new(1_700_000_000_000)),
        Arc::new(DeterministicFakeModel::new("undo-recovery-test")),
    )
    .unwrap()
}

fn create_and_refine(engine: &Engine, title: &str, orig: &str, refined: &str) -> (String, String) {
    let art = engine
        .create_and_save_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some(title),
            orig,
            None,
            None,
        )
        .unwrap()
        .artifact;
    let inverse = serde_json::json!({
        "operations": [{
            "type": "restore_artifact",
            "artifact_id": art.id,
            "content_md": art.content_md,
            "content_hash": art.content_hash,
            "refined_content_md": refined,
            "refined_content_hash": vault::compute_content_hash(refined),
        }]
    });
    let (_, cs_id) = engine
        .apply_refinement_with_cancellation(
            &art.id,
            refined,
            &format!("Refined: {title}"),
            &inverse,
            None,
        )
        .unwrap();
    (art.id, cs_id)
}

fn mirror_body(engine: &Engine, artifact_id: &str) -> String {
    let art = engine.get_artifact(artifact_id).unwrap();
    let path = engine.vault_dir().join(art.file_path.as_ref().unwrap());
    let raw = std::fs::read_to_string(path).unwrap();
    let (_, body) = vault::parse_frontmatter(&raw);
    body
}

unsafe extern "C" fn exit_at_sqlite_commit(_: *mut std::ffi::c_void) -> std::ffi::c_int {
    std::process::exit(77);
}

#[test]
fn wb_r1_crash_at_sqlite_commit_boundary_does_not_publish_speculative_mirror() {
    if let Some(path) = std::env::var_os("STHIRA_WB_R1_UNDO_CRASH_DIR") {
        let path = std::path::PathBuf::from(path);
        let ids: Vec<String> =
            serde_json::from_str(&std::fs::read_to_string(path.join("ids.json")).unwrap()).unwrap();
        let engine = open_engine(&path);
        {
            let writer = engine.db().writer();
            unsafe {
                rusqlite::ffi::sqlite3_commit_hook(
                    writer.handle(),
                    Some(exit_at_sqlite_commit),
                    std::ptr::null_mut(),
                );
            }
        }
        let _ = engine.undo_refinement(&ids[0], &ids[1]);
        panic!("SQLite commit hook was not reached");
    }

    let dir = tempdir().unwrap();
    let engine = open_engine(dir.path());
    let (id, cs_id) = create_and_refine(&engine, "Review", "Original body.", "Refined body.");
    std::fs::write(
        dir.path().join("ids.json"),
        serde_json::to_string(&vec![&id, &cs_id]).unwrap(),
    )
    .unwrap();
    drop(engine);

    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "wb_r1_crash_at_sqlite_commit_boundary_does_not_publish_speculative_mirror",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("STHIRA_WB_R1_UNDO_CRASH_DIR", dir.path())
        .output()
        .unwrap();
    assert_eq!(child.status.code(), Some(77));

    let reopened = open_engine(dir.path());
    let art = reopened.get_artifact(&id).unwrap();
    let m_body = mirror_body(&reopened, &id);
    let reverted: Option<i64> = reopened
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT reverted_at FROM change_sets WHERE id = ?1;",
            [&cs_id],
            |r| r.get(0),
        )
        .unwrap();

    assert_eq!(art.content_md, "Refined body.");
    assert_eq!(reverted, None);
    assert_eq!(
        art.last_synced_hash.as_deref(),
        Some(art.content_hash.as_str())
    );
    assert_eq!(
        m_body, "Refined body.",
        "Undo must commit SQLite before publishing to the Markdown mirror"
    );
}

#[test]
fn wb_r1_pre_commit_db_failures_leave_db_and_mirror_untouched() {
    let dir = tempdir().unwrap();
    let engine = open_engine(dir.path());
    let (id, cs_id) = create_and_refine(&engine, "PreCommit", "Original body.", "Refined body.");
    let before = engine.get_artifact(&id).unwrap();

    for (trigger_name, trigger_sql) in [
        (
            "wb_abort_undo_rev",
            "CREATE TRIGGER wb_abort_undo_rev BEFORE INSERT ON artifact_revisions
             WHEN NEW.actor = 'system'
             BEGIN SELECT RAISE(ABORT, 'injected revision abort'); END;",
        ),
        (
            "wb_abort_undo_evt",
            "CREATE TRIGGER wb_abort_undo_evt BEFORE INSERT ON session_events
             WHEN NEW.event_type = 'change_set_reverted'
             BEGIN SELECT RAISE(ABORT, 'injected event abort'); END;",
        ),
        (
            "wb_abort_undo_cs",
            "CREATE TRIGGER wb_abort_undo_cs BEFORE UPDATE OF reverted_at ON change_sets
             BEGIN SELECT RAISE(ABORT, 'injected cs abort'); END;",
        ),
    ] {
        engine.db().writer().execute_batch(trigger_sql).unwrap();
        let err = engine
            .undo_refinement(&id, &cs_id)
            .expect_err("Pre-commit failure must reject cleanly");
        assert!(
            err.contains("injected"),
            "Unexpected pre-commit error: {err}"
        );

        let after = engine.get_artifact(&id).unwrap();
        assert_eq!(after.version, before.version);
        assert_eq!(after.content_md, "Refined body.");
        assert_eq!(after.last_synced_hash, before.last_synced_hash);
        assert_eq!(mirror_body(&engine, &id), "Refined body.");

        let reverted: Option<i64> = engine
            .db()
            .reader()
            .unwrap()
            .query_row(
                "SELECT reverted_at FROM change_sets WHERE id = ?1;",
                [&cs_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(reverted, None);

        engine
            .db()
            .writer()
            .execute_batch(&format!("DROP TRIGGER {trigger_name};"))
            .unwrap();
    }

    let ok = engine.undo_refinement(&id, &cs_id).unwrap();
    assert!(ok.mirror_synced);
    assert_eq!(ok.artifact.content_md, "Original body.");
    assert_eq!(mirror_body(&engine, &id), "Original body.");
}

#[test]
fn wb_r1_post_commit_mirror_and_sync_failures_report_honest_outcome_and_repair_without_repeat_undo()
{
    // 1. Read-only mirror (0444) fails mirror write post-commit -> DB committed Undo, outcome warns,
    //    duplicate Undo is refused, and save_artifact repairs mirror cleanly with 0 extra versions/revs.
    {
        let dir = tempdir().unwrap();
        let engine = open_engine(dir.path());
        let (id, cs_id) =
            create_and_refine(&engine, "ReadOnlyMirror", "Original body.", "Refined body.");
        let refined = engine.get_artifact(&id).unwrap();
        let mirror_path = engine.vault_dir().join(refined.file_path.as_ref().unwrap());
        std::fs::set_permissions(&mirror_path, std::fs::Permissions::from_mode(0o444)).unwrap();

        let outcome = engine.undo_refinement(&id, &cs_id).expect(
            "Committed DB Undo must return Ok(SaveArtifactOutcome) even when mirror IO fails",
        );
        assert!(!outcome.mirror_synced);
        assert!(outcome.mirror_sync_error.is_some());
        assert_eq!(outcome.artifact.content_md, "Original body.");
        assert_eq!(outcome.artifact.version, refined.version + 1);
        assert_eq!(
            outcome.artifact.last_synced_hash.as_deref(),
            Some(refined.content_hash.as_str()),
            "Out-of-sync marker must stay pointing to prior synced hash"
        );
        assert_eq!(mirror_body(&engine, &id), "Refined body.");

        // Duplicate Undo after committed Undo with failed mirror MUST be rejected
        let dup = engine
            .undo_refinement(&id, &cs_id)
            .expect_err("Duplicate Undo must be refused");
        assert!(dup.contains("already been undone"));

        let revs_before: i64 = engine
            .db()
            .reader()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM artifact_revisions WHERE artifact_id = ?1;",
                [&id],
                |r| r.get(0),
            )
            .unwrap();
        let evts_before: i64 = engine
            .db()
            .reader()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM session_events WHERE artifact_id = ?1;",
                [&id],
                |r| r.get(0),
            )
            .unwrap();

        // Simulate reopen + reconcile_vault recovery (DB changed, disk still matches last_synced_hash)
        std::fs::set_permissions(&mirror_path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let rec = engine.reconcile_vault().unwrap();
        assert_eq!(rec.conflicts_detected, 0);
        assert_eq!(rec.files_exported, 1);
        assert_eq!(mirror_body(&engine, &id), "Original body.");

        let after = engine.get_artifact(&id).unwrap();
        assert_eq!(after.version, outcome.artifact.version);
        assert_eq!(
            after.last_synced_hash.as_deref(),
            Some(after.content_hash.as_str())
        );

        let revs_after: i64 = engine
            .db()
            .reader()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM artifact_revisions WHERE artifact_id = ?1;",
                [&id],
                |r| r.get(0),
            )
            .unwrap();
        let evts_after: i64 = engine
            .db()
            .reader()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM session_events WHERE artifact_id = ?1;",
                [&id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(revs_after, revs_before);
        assert_eq!(evts_after, evts_before);
    }

    // 2. Post-commit sync metadata update failure -> mirror has Original body, outcome warns honestly,
    //    and save_artifact mirror repair heals last_synced_hash cleanly with 0 extra versions/revs/events.
    {
        let dir = tempdir().unwrap();
        let engine = open_engine(dir.path());
        let (id, cs_id) = create_and_refine(
            &engine,
            "SyncBookkeeping",
            "Original body.",
            "Refined body.",
        );
        let refined = engine.get_artifact(&id).unwrap();

        engine
            .db()
            .writer()
            .execute_batch(
                "CREATE TRIGGER wb_abort_undo_sync BEFORE UPDATE OF last_synced_hash ON artifacts
                 WHEN NEW.last_synced_hash IS NOT NULL AND NEW.last_synced_hash = NEW.content_hash
                 BEGIN SELECT RAISE(ABORT, 'injected undo sync failure'); END;",
            )
            .unwrap();

        let outcome = engine.undo_refinement(&id, &cs_id).expect(
            "Committed DB Undo + mirror write must return Ok even when sync metadata fails",
        );
        assert!(!outcome.mirror_synced);
        assert!(outcome
            .mirror_sync_error
            .as_deref()
            .unwrap_or_default()
            .contains("injected undo sync failure"));
        assert_eq!(outcome.artifact.content_md, "Original body.");
        assert_eq!(mirror_body(&engine, &id), "Original body.");
        assert_eq!(
            outcome.artifact.last_synced_hash.as_deref(),
            Some(refined.content_hash.as_str())
        );

        let revs_before: i64 = engine
            .db()
            .reader()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM artifact_revisions WHERE artifact_id = ?1;",
                [&id],
                |r| r.get(0),
            )
            .unwrap();
        let evts_before: i64 = engine
            .db()
            .reader()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM session_events WHERE artifact_id = ?1;",
                [&id],
                |r| r.get(0),
            )
            .unwrap();

        engine
            .db()
            .writer()
            .execute_batch("DROP TRIGGER wb_abort_undo_sync;")
            .unwrap();

        let repaired = engine
            .save_artifact(
                &id,
                SaveArtifactRequest {
                    content_md: "Original body.",
                    title: Some("SyncBookkeeping"),
                    actor: Actor::User,
                    context_project_id: None,
                    authorship: None,
                    is_unfinished_draft: None,
                },
            )
            .unwrap();
        assert!(repaired.unchanged);
        assert!(repaired.repaired_mirror_only);
        assert!(repaired.mirror_synced);
        assert_eq!(repaired.artifact.version, outcome.artifact.version);
        assert_eq!(
            repaired.artifact.last_synced_hash.as_deref(),
            Some(repaired.artifact.content_hash.as_str())
        );

        let revs_after: i64 = engine
            .db()
            .reader()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM artifact_revisions WHERE artifact_id = ?1;",
                [&id],
                |r| r.get(0),
            )
            .unwrap();
        let evts_after: i64 = engine
            .db()
            .reader()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM session_events WHERE artifact_id = ?1;",
                [&id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(revs_after, revs_before);
        assert_eq!(evts_after, evts_before);
    }
}

#[test]
fn wb_r2_undo_rejects_all_superseded_revisions_and_preserves_noop_or_mirror_repairs() {
    // A. B -> C -> B returns to identical refined body bytes: MUST reject
    {
        let dir = tempdir().unwrap();
        let engine = open_engine(dir.path());
        let (id, cs_id) = create_and_refine(&engine, "Review", "Original body.", "Refined body.");
        for text in ["Newer user revision.", "Refined body."] {
            engine
                .save_artifact(
                    &id,
                    SaveArtifactRequest {
                        content_md: text,
                        title: Some("Review"),
                        actor: Actor::User,
                        context_project_id: None,
                        authorship: None,
                        is_unfinished_draft: Some(false),
                    },
                )
                .unwrap();
        }
        let before = engine.get_artifact(&id).unwrap();
        assert_eq!(before.version, 4);
        let err = engine
            .undo_refinement(&id, &cs_id)
            .expect_err("Undo must reject superseded revision even if body bytes match again");
        assert!(err.contains("document changed after this refinement"));
        let after = engine.get_artifact(&id).unwrap();
        assert_eq!(after.version, before.version);
        assert_eq!(after.content_md, before.content_md);
    }

    // B. A -> B (refined v2) -> A (saved v3): MUST reject
    {
        let dir = tempdir().unwrap();
        let engine = open_engine(dir.path());
        let (id, cs_id) =
            create_and_refine(&engine, "RoundTrip", "Original body.", "Refined body.");
        engine
            .save_artifact(
                &id,
                SaveArtifactRequest {
                    content_md: "Original body.",
                    title: Some("RoundTrip"),
                    actor: Actor::User,
                    context_project_id: None,
                    authorship: None,
                    is_unfinished_draft: Some(false),
                },
            )
            .unwrap();
        let err = engine
            .undo_refinement(&id, &cs_id)
            .expect_err("Undo must reject A->B->A superseded revision");
        assert!(err.contains("document changed after this refinement"));
    }

    // C. Title-only superseding edit (body untouched) via save_artifact AND apply_suggested_title: MUST reject
    {
        let dir = tempdir().unwrap();
        let engine = open_engine(dir.path());
        let (id, cs_id) =
            create_and_refine(&engine, "Initial Title", "Original body.", "Refined body.");
        engine
            .save_artifact(
                &id,
                SaveArtifactRequest {
                    content_md: "Refined body.",
                    title: Some("Renamed Title Only"),
                    actor: Actor::User,
                    context_project_id: None,
                    authorship: None,
                    is_unfinished_draft: Some(false),
                },
            )
            .unwrap();
        let err = engine
            .undo_refinement(&id, &cs_id)
            .expect_err("Undo must reject title-only saved revision");
        assert!(err.contains("document changed after this refinement"));
        assert_eq!(
            engine.get_artifact(&id).unwrap().title.as_deref(),
            Some("Renamed Title Only")
        );

        // Also test apply_suggested_title
        let (id2, cs_id2) =
            create_and_refine(&engine, "Title Basis", "Original body.", "Refined body.");
        let art2 = engine.get_artifact(&id2).unwrap();
        engine
            .apply_suggested_title(
                &id2,
                "Suggested New Title",
                &art2.content_hash,
                art2.title.as_deref(),
            )
            .unwrap();
        let err2 = engine
            .undo_refinement(&id2, &cs_id2)
            .expect_err("Undo must reject after apply_suggested_title");
        assert!(err2.contains("document changed after this refinement"));
    }

    // D. Metadata-only edit (e.g. authorship change with identical body & title): MUST reject
    {
        let dir = tempdir().unwrap();
        let engine = open_engine(dir.path());
        let (id, cs_id) = create_and_refine(&engine, "Meta", "Original body.", "Refined body.");
        engine
            .save_artifact(
                &id,
                SaveArtifactRequest {
                    content_md: "Refined body.",
                    title: Some("Meta"),
                    actor: Actor::Ai,
                    context_project_id: None,
                    authorship: Some(Authorship::AiGenerated),
                    is_unfinished_draft: Some(false),
                },
            )
            .unwrap();
        let err = engine
            .undo_refinement(&id, &cs_id)
            .expect_err("Undo must reject metadata-only saved revision");
        assert!(err.contains("document changed after this refinement"));
    }

    // E. No-op save and mirror-only repair retaining exact same revision STILL allow Undo!
    {
        let dir = tempdir().unwrap();
        let engine = open_engine(dir.path());
        let (id, cs_id) =
            create_and_refine(&engine, "MirrorRepair", "Original body.", "Refined body.");
        let refined = engine.get_artifact(&id).unwrap();
        let mirror_path = engine.vault_dir().join(refined.file_path.as_ref().unwrap());

        // Simulate missing mirror, then run unchanged save_artifact to repair mirror without bumping version
        std::fs::remove_file(&mirror_path).unwrap();
        let repair_outcome = engine
            .save_artifact(
                &id,
                SaveArtifactRequest {
                    content_md: "Refined body.",
                    title: Some("MirrorRepair"),
                    actor: Actor::User,
                    context_project_id: None,
                    authorship: None,
                    is_unfinished_draft: None,
                },
            )
            .unwrap();
        assert!(repair_outcome.unchanged);
        assert!(repair_outcome.repaired_mirror_only);
        assert_eq!(repair_outcome.artifact.version, refined.version);

        let undo_outcome = engine
            .undo_refinement(&id, &cs_id)
            .expect("Harmless mirror repair keeping refined revision must preserve Undo");
        assert!(undo_outcome.mirror_synced);
        assert_eq!(undo_outcome.artifact.content_md, "Original body.");
        assert_eq!(mirror_body(&engine, &id), "Original body.");
    }
}

#[test]
fn wb_r2_concurrent_save_and_undo_preserve_db_and_disk_consistency() {
    for _ in 0..8 {
        let dir = tempdir().unwrap();
        let engine = Arc::new(open_engine(dir.path()));
        let (id, cs_id) = create_and_refine(&engine, "Race", "Original body.", "Refined body.");
        let barrier = Arc::new(std::sync::Barrier::new(2));

        let e_undo = Arc::clone(&engine);
        let b_undo = Arc::clone(&barrier);
        let id_undo = id.clone();
        let cs_undo = cs_id.clone();
        let h_undo = std::thread::spawn(move || {
            b_undo.wait();
            e_undo.undo_refinement(&id_undo, &cs_undo)
        });

        let e_save = Arc::clone(&engine);
        let b_save = Arc::clone(&barrier);
        let id_save = id.clone();
        let h_save = std::thread::spawn(move || {
            b_save.wait();
            e_save.save_artifact(
                &id_save,
                SaveArtifactRequest {
                    content_md: "Concurrent user save.",
                    title: Some("Race"),
                    actor: Actor::User,
                    context_project_id: None,
                    authorship: None,
                    is_unfinished_draft: Some(false),
                },
            )
        });

        let undo_res = h_undo.join().unwrap();
        let save_res = h_save.join().unwrap().unwrap();
        let final_art = engine.get_artifact(&id).unwrap();
        let final_mirror = mirror_body(&engine, &id);

        assert_eq!(final_art.content_md, "Concurrent user save.");
        assert_eq!(final_mirror, final_art.content_md);
        assert_eq!(
            final_art.last_synced_hash.as_deref(),
            Some(final_art.content_hash.as_str())
        );

        match undo_res {
            Ok(u) => {
                // Undo won the writer lock first -> Save ran after Undo (version 4)
                assert_eq!(u.artifact.version, 3);
                assert_eq!(save_res.artifact.version, 4);
                assert_eq!(final_art.version, 4);
            }
            Err(err) => {
                // Save won the writer lock first -> Undo rejected superseding revision (version 3)
                assert!(err.contains("document changed after this refinement"));
                assert_eq!(save_res.artifact.version, 3);
                assert_eq!(final_art.version, 3);
            }
        }
    }
}
