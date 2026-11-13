use std::sync::Arc;
use sthira_core::{vault, Actor, ArtifactType, Authorship, Engine, SaveArtifactRequest};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

fn open_engine(dir: &std::path::Path) -> Engine {
    Engine::open(
        dir.join("sthira.db"),
        dir.join("vault"),
        Arc::new(FakeClock::new(1_700_000_000_000)),
        Arc::new(DeterministicFakeModel::new("wave-a-test-model")),
    )
    .unwrap()
}

#[test]
fn a1_single_transaction_save_rolls_back_on_body_or_revision_failure() {
    let tmp = tempdir().unwrap();
    let engine = open_engine(tmp.path());
    let created = engine
        .create_and_save_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Original title"),
            "# Original title\n\nOriginal body.",
            None,
            None,
        )
        .unwrap()
        .artifact;

    // 1. Trigger aborting content_md update must NOT commit title or bump version
    engine
        .db()
        .writer()
        .execute_batch(
            "CREATE TRIGGER wave_a_abort_body BEFORE UPDATE OF content_md ON artifacts
             BEGIN SELECT RAISE(ABORT, 'injected body save failure'); END;",
        )
        .unwrap();

    let res = engine.save_artifact(
        &created.id,
        SaveArtifactRequest {
            content_md: "# Changed title\n\nChanged body.",
            title: Some("Changed title"),
            actor: Actor::User,
            context_project_id: None,
            authorship: None,
            is_unfinished_draft: None,
        },
    );
    assert!(res.is_err());
    let persisted = engine.get_artifact(&created.id).unwrap();
    assert_eq!(persisted.title.as_deref(), Some("Original title"));
    assert_eq!(persisted.content_md, created.content_md);
    assert_eq!(persisted.version, created.version);

    engine
        .db()
        .writer()
        .execute_batch("DROP TRIGGER wave_a_abort_body;")
        .unwrap();

    // 2. Trigger aborting revision insert must roll back body/title/version atomically
    engine
        .db()
        .writer()
        .execute_batch(
            "CREATE TRIGGER wave_a_abort_rev BEFORE INSERT ON artifact_revisions
             BEGIN SELECT RAISE(ABORT, 'injected revision save failure'); END;",
        )
        .unwrap();

    let res2 = engine.save_artifact(
        &created.id,
        SaveArtifactRequest {
            content_md: "# Changed title\n\nChanged body 2.",
            title: Some("Changed title"),
            actor: Actor::User,
            context_project_id: None,
            authorship: None,
            is_unfinished_draft: None,
        },
    );
    assert!(res2.is_err());
    let persisted2 = engine.get_artifact(&created.id).unwrap();
    assert_eq!(persisted2.title.as_deref(), Some("Original title"));
    assert_eq!(persisted2.content_md, created.content_md);
    assert_eq!(persisted2.version, created.version);

    engine
        .db()
        .writer()
        .execute_batch("DROP TRIGGER wave_a_abort_rev;")
        .unwrap();

    // 3. Trigger aborting session_events insert after artifacts + revisions update rolls back EVERYTHING
    let rev_count_before: i64 = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM artifact_revisions WHERE artifact_id = ?1;",
            rusqlite::params![created.id],
            |r| r.get(0),
        )
        .unwrap();

    engine
        .db()
        .writer()
        .execute_batch(
            "CREATE TRIGGER wave_a_abort_event BEFORE INSERT ON session_events
             BEGIN SELECT RAISE(ABORT, 'injected event save failure'); END;",
        )
        .unwrap();

    let res3 = engine.save_artifact(
        &created.id,
        SaveArtifactRequest {
            content_md: "# Changed title\n\nChanged body 3.",
            title: Some("Changed title"),
            actor: Actor::User,
            context_project_id: None,
            authorship: None,
            is_unfinished_draft: None,
        },
    );
    assert!(res3.is_err());
    let persisted3 = engine.get_artifact(&created.id).unwrap();
    assert_eq!(persisted3.title.as_deref(), Some("Original title"));
    assert_eq!(persisted3.content_md, created.content_md);
    assert_eq!(persisted3.version, created.version);
    let rev_count_after: i64 = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM artifact_revisions WHERE artifact_id = ?1;",
            rusqlite::params![created.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        rev_count_after, rev_count_before,
        "A failed session_events insert must roll back revision insertion too"
    );
}

#[test]
fn a1_save_preserves_markdown_indentation_and_distinguishes_whitespace_changes() {
    let tmp = tempdir().unwrap();
    let engine = open_engine(tmp.path());

    let indented_md = "# Indented Doc\n\n  - nested list item\n    - grandchild item\n\n    fn main() {\n        println!(\"hi\");\n    }\n";
    let created = engine
        .create_and_save_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Indented Doc"),
            indented_md,
            None,
            None,
        )
        .unwrap();
    assert!(created.mirror_synced);

    // Vault normalization preserves internal indentation and only strips trailing LF
    assert!(created.artifact.content_md.contains("  - nested list item"));
    assert!(created
        .artifact
        .content_md
        .contains("    - grandchild item"));
    assert!(created
        .artifact
        .content_md
        .contains("        println!(\"hi\");"));

    // CRLF vs LF of identical content is an exact normalized no-op (no version bump)
    let crlf_copy = created.artifact.content_md.replace('\n', "\r\n");
    let noop = engine
        .save_artifact(
            &created.artifact.id,
            SaveArtifactRequest {
                content_md: &crlf_copy,
                title: Some("Indented Doc"),
                actor: Actor::User,
                context_project_id: None,
                authorship: None,
                is_unfinished_draft: None,
            },
        )
        .unwrap();
    assert!(noop.unchanged);
    assert!(!noop.repaired_mirror_only);
    assert!(noop.mirror_synced);
    assert_eq!(noop.artifact.version, created.artifact.version);

    // Changing leading indentation IS a real content change and bumps version
    let reindented = created
        .artifact
        .content_md
        .replace("  - nested list item", "- nested list item");
    let changed = engine
        .save_artifact(
            &created.artifact.id,
            SaveArtifactRequest {
                content_md: &reindented,
                title: Some("Indented Doc"),
                actor: Actor::User,
                context_project_id: None,
                authorship: None,
                is_unfinished_draft: None,
            },
        )
        .unwrap();
    assert!(!changed.unchanged);
    assert!(changed.mirror_synced);
    assert_eq!(changed.artifact.version, created.artifact.version + 1);
    assert!(changed.artifact.content_md.contains("\n- nested list item"));

    let disk = std::fs::read_to_string(
        engine
            .vault_dir()
            .join(changed.artifact.file_path.as_ref().unwrap()),
    )
    .unwrap();
    let (_, disk_body) = vault::parse_frontmatter(&disk);
    assert_eq!(disk_body, changed.artifact.content_md);
}

#[test]
fn a1_mirror_failure_is_reported_honestly_and_repairs_without_version_bump() {
    let tmp = tempdir().unwrap();
    let engine = open_engine(tmp.path());

    let art = engine
        .create_and_save_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Mirror Check"),
            "# Mirror Check\n\nInitial version.",
            None,
            None,
        )
        .unwrap()
        .artifact;
    let initial_hash = art.content_hash.clone();
    let mirror_path = engine.vault_dir().join(art.file_path.as_ref().unwrap());
    std::fs::remove_file(&mirror_path).unwrap();
    std::fs::create_dir(&mirror_path).unwrap();

    let failed = engine
        .save_artifact(
            &art.id,
            SaveArtifactRequest {
                content_md: "# Mirror Check\n\nUpdated body while mirror blocked.",
                title: Some("Mirror Check"),
                actor: Actor::User,
                context_project_id: None,
                authorship: None,
                is_unfinished_draft: None,
            },
        )
        .unwrap();

    assert!(!failed.unchanged);
    assert!(!failed.mirror_synced);
    assert!(failed.mirror_sync_error.is_some());
    assert_eq!(failed.artifact.version, art.version + 1);
    assert_eq!(
        failed.artifact.last_synced_hash.as_deref(),
        Some(initial_hash.as_str()),
        "last_synced_hash must stay stale until mirror publish actually succeeds"
    );

    // Unblock mirror path and save identical content -> repairs mirror only without bumping version
    std::fs::remove_dir(&mirror_path).unwrap();
    let repaired = engine
        .save_artifact(
            &art.id,
            SaveArtifactRequest {
                content_md: "# Mirror Check\n\nUpdated body while mirror blocked.",
                title: Some("Mirror Check"),
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
    assert!(repaired.mirror_sync_error.is_none());
    assert_eq!(repaired.artifact.version, failed.artifact.version);
    assert_eq!(
        repaired.artifact.last_synced_hash.as_deref(),
        Some(repaired.artifact.content_hash.as_str())
    );
    let disk = std::fs::read_to_string(&mirror_path).unwrap();
    assert!(disk.contains("Updated body while mirror blocked."));

    // Read-only (chmod 0444) mirror failure also warns honestly and repairs without version bump once writable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut ro = std::fs::metadata(&mirror_path).unwrap().permissions();
        ro.set_mode(0o444);
        std::fs::set_permissions(&mirror_path, ro).unwrap();

        let ro_outcome = engine
            .save_artifact(
                &art.id,
                SaveArtifactRequest {
                    content_md: "# Mirror Check\n\nUpdated body while mirror read-only.",
                    title: Some("Mirror Check"),
                    actor: Actor::User,
                    context_project_id: None,
                    authorship: None,
                    is_unfinished_draft: None,
                },
            )
            .unwrap();
        assert!(!ro_outcome.mirror_synced);
        assert!(ro_outcome.mirror_sync_error.is_some());
        assert_eq!(ro_outcome.artifact.version, repaired.artifact.version + 1);
        assert_eq!(
            ro_outcome.artifact.last_synced_hash.as_deref(),
            Some(repaired.artifact.content_hash.as_str())
        );

        let mut rw = std::fs::metadata(&mirror_path).unwrap().permissions();
        rw.set_mode(0o644);
        std::fs::set_permissions(&mirror_path, rw).unwrap();

        let ro_retry = engine
            .save_artifact(
                &art.id,
                SaveArtifactRequest {
                    content_md: "# Mirror Check\n\nUpdated body while mirror read-only.",
                    title: Some("Mirror Check"),
                    actor: Actor::User,
                    context_project_id: None,
                    authorship: None,
                    is_unfinished_draft: None,
                },
            )
            .unwrap();
        assert!(ro_retry.unchanged);
        assert!(ro_retry.repaired_mirror_only);
        assert!(ro_retry.mirror_synced);
        assert_eq!(ro_retry.artifact.version, ro_outcome.artifact.version);
        let disk_after = std::fs::read_to_string(&mirror_path).unwrap();
        assert!(disk_after.contains("Updated body while mirror read-only."));
    }
}

#[test]
fn a2_v19_migration_and_fts_triggers_exclude_tombstones() {
    let tmp = tempdir().unwrap();
    let engine = open_engine(tmp.path());

    // Simulate pre-v19 database state
    let note = engine
        .create_and_save_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Searchable Active Note"),
            "# Searchable Active Note\n\nUniqueKeywordActive",
            None,
            None,
        )
        .unwrap()
        .artifact;
    let dead_note = engine
        .create_and_save_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Old Tombstone Note"),
            "# Old Tombstone Note\n\nUniqueKeywordTombstone",
            None,
            None,
        )
        .unwrap()
        .artifact;

    {
        let writer = engine.db().writer();
        writer
            .execute_batch(
                "DROP TRIGGER IF EXISTS artifacts_ai;
                 DROP TRIGGER IF EXISTS artifacts_au;
                 DROP TRIGGER IF EXISTS artifacts_ad;",
            )
            .unwrap();
        writer
            .execute(
                "UPDATE artifacts SET deleted_at = 12345 WHERE id = ?1;",
                rusqlite::params![dead_note.id],
            )
            .unwrap();
        writer.execute_batch("PRAGMA user_version = 18;").unwrap();
    }
    drop(engine);

    // Reopen -> runs v19 migration, purges tombstones from FTS, preserves vault_meta
    let reopened = open_engine(tmp.path());
    let reader = reopened.db().reader().unwrap();
    let version: i64 = reader
        .query_row("PRAGMA user_version;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 19);

    let active_fts_hits: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM artifacts_fts WHERE artifacts_fts MATCH 'UniqueKeywordActive';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(active_fts_hits, 1);

    let dead_fts_hits: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM artifacts_fts WHERE artifacts_fts MATCH 'UniqueKeywordTombstone';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        dead_fts_hits, 0,
        "Tombstoned artifacts must be absent from artifacts_fts after v19"
    );

    // Updating an active note to tombstoned via SQL removes it from artifacts_fts via trigger
    drop(reader);
    {
        let writer = reopened.db().writer();
        writer
            .execute(
                "UPDATE artifacts SET deleted_at = 99999 WHERE id = ?1;",
                rusqlite::params![note.id],
            )
            .unwrap();
    }
    let reader = reopened.db().reader().unwrap();
    let active_after_tombstone: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM artifacts_fts WHERE artifacts_fts MATCH 'UniqueKeywordActive';",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(active_after_tombstone, 0);

    // vault_instance_id exists and stays stable across reopens until rotated
    let id1 = reopened.vault_instance_id().unwrap();
    assert!(!id1.is_empty());
    drop(reader);
    drop(reopened);

    let reopened2 = open_engine(tmp.path());
    let id2 = reopened2.vault_instance_id().unwrap();
    assert_eq!(id1, id2);
}

#[test]
fn a2_reconcile_vault_skips_tombstoned_artifacts_even_if_stray_file_remains() {
    let tmp = tempdir().unwrap();
    let engine = open_engine(tmp.path());

    let art = engine
        .create_and_save_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Stray Note"),
            "# Stray Note\n\nShould stay hidden after deletion.",
            None,
            None,
        )
        .unwrap()
        .artifact;
    let mirror = engine.vault_dir().join(art.file_path.as_ref().unwrap());

    {
        let writer = engine.db().writer();
        writer
            .execute(
                "UPDATE artifacts SET deleted_at = 1700000000999 WHERE id = ?1;",
                rusqlite::params![art.id],
            )
            .unwrap();
    }

    // Even if a stray .md mirror file sits on disk for a tombstoned artifact, reconcile skips it
    assert!(mirror.exists());
    let rec = engine.reconcile_vault().unwrap();
    assert_eq!(rec.files_imported, 0);
    assert_eq!(rec.conflicts_detected, 0);
    assert!(engine.get_artifact(&art.id).is_err());
}

#[test]
fn a1_committed_save_and_mirror_repair_report_honest_outcome_on_sync_bookkeeping_error() {
    let tmp = tempdir().unwrap();
    let engine = open_engine(tmp.path());

    let original = engine
        .create_and_save_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Original"),
            "Original body.",
            None,
            None,
        )
        .unwrap()
        .artifact;
    let original_hash = original.content_hash.clone();
    let mirror_path = engine
        .vault_dir()
        .join(original.file_path.as_ref().unwrap());

    // Reject only post-commit `last_synced_hash` updates on artifacts
    engine
        .db()
        .writer()
        .execute_batch(
            "CREATE TRIGGER test_abort_sync_bookkeeping BEFORE UPDATE OF last_synced_hash ON artifacts
             WHEN NEW.last_synced_hash IS NOT NULL AND NEW.last_synced_hash = NEW.content_hash
             BEGIN SELECT RAISE(ABORT, 'injected sync bookkeeping failure'); END;",
        )
        .unwrap();

    // 1. Changed save commits DB + writes mirror, then fails sync bookkeeping -> MUST still return Ok(committed)
    let changed_outcome = engine
        .save_artifact(
            &original.id,
            SaveArtifactRequest {
                content_md: "New body already committed.",
                title: Some("New title"),
                actor: Actor::User,
                context_project_id: None,
                authorship: None,
                is_unfinished_draft: None,
            },
        )
        .expect("Committed save + mirror write must return Ok(SaveArtifactOutcome) even when sync bookkeeping errors");

    assert!(!changed_outcome.mirror_synced);
    assert!(changed_outcome
        .mirror_sync_error
        .as_deref()
        .unwrap_or_default()
        .contains("injected sync bookkeeping failure"));
    assert!(!changed_outcome.unchanged);
    assert!(!changed_outcome.repaired_mirror_only);
    assert_eq!(changed_outcome.artifact.version, original.version + 1);
    assert_eq!(changed_outcome.artifact.title.as_deref(), Some("New title"));
    assert!(changed_outcome
        .artifact
        .content_md
        .contains("New body already committed."));
    assert_eq!(
        changed_outcome.artifact.last_synced_hash.as_deref(),
        Some(original_hash.as_str()),
        "Out-of-sync marker must remain stale in returned outcome until bookkeeping succeeds"
    );

    let persisted = engine.get_artifact(&original.id).unwrap();
    assert_eq!(persisted.version, original.version + 1);
    assert_eq!(
        persisted.last_synced_hash.as_deref(),
        Some(original_hash.as_str())
    );
    let mirror = std::fs::read_to_string(&mirror_path).unwrap();
    assert!(mirror.contains("New body already committed."));

    let count_revs_and_events = || -> (i64, i64) {
        let reader = engine.db().reader().unwrap();
        let revs: i64 = reader
            .query_row(
                "SELECT COUNT(*) FROM artifact_revisions WHERE artifact_id = ?1;",
                rusqlite::params![original.id],
                |r| r.get(0),
            )
            .unwrap();
        let evts: i64 = reader
            .query_row(
                "SELECT COUNT(*) FROM session_events WHERE artifact_id = ?1;",
                rusqlite::params![original.id],
                |r| r.get(0),
            )
            .unwrap();
        (revs, evts)
    };
    let (revs_before_repair, events_before_repair) = count_revs_and_events();
    assert_eq!(revs_before_repair, 2);

    // 2. Unchanged mirror repair while sync bookkeeping still fails -> Ok(unchanged + retryable error, zero extra versions/revs/events)
    let blocked_repair = engine
        .save_artifact(
            &original.id,
            SaveArtifactRequest {
                content_md: "New body already committed.",
                title: Some("New title"),
                actor: Actor::User,
                context_project_id: None,
                authorship: None,
                is_unfinished_draft: None,
            },
        )
        .expect(
            "Unchanged mirror repair with failing bookkeeping must return honest retryable outcome",
        );
    assert!(blocked_repair.unchanged);
    assert!(!blocked_repair.repaired_mirror_only);
    assert!(!blocked_repair.mirror_synced);
    assert!(blocked_repair.mirror_sync_error.is_some());
    assert_eq!(blocked_repair.artifact.version, persisted.version);
    assert_ne!(
        blocked_repair.artifact.last_synced_hash.as_deref(),
        Some(persisted.content_hash.as_str())
    );
    assert_eq!(
        count_revs_and_events(),
        (revs_before_repair, events_before_repair)
    );

    // 3. Unblock sync bookkeeping and retry repair -> succeeds without extra versions, revisions, or events
    engine
        .db()
        .writer()
        .execute_batch("DROP TRIGGER test_abort_sync_bookkeeping;")
        .unwrap();
    let repaired = engine
        .save_artifact(
            &original.id,
            SaveArtifactRequest {
                content_md: "New body already committed.",
                title: Some("New title"),
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
    assert!(repaired.mirror_sync_error.is_none());
    assert_eq!(repaired.artifact.version, persisted.version);
    assert_eq!(
        repaired.artifact.last_synced_hash.as_deref(),
        Some(persisted.content_hash.as_str())
    );
    assert_eq!(
        count_revs_and_events(),
        (revs_before_repair, events_before_repair)
    );

    // 4. Create path also reports committed artifact + retryable sync warning if post-commit bookkeeping errors
    engine
        .db()
        .writer()
        .execute_batch(
            "CREATE TRIGGER test_abort_create_sync BEFORE UPDATE OF last_synced_hash ON artifacts
             WHEN NEW.last_synced_hash IS NOT NULL AND NEW.last_synced_hash = NEW.content_hash
             BEGIN SELECT RAISE(ABORT, 'injected create sync bookkeeping failure'); END;",
        )
        .unwrap();
    let created_warn = engine
        .create_and_save_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Created Sync Warn"),
            "# Created Sync Warn\n\nBody.",
            None,
            None,
        )
        .expect(
            "Committed creation + mirror write must return Ok even when sync bookkeeping errors",
        );
    assert_eq!(created_warn.artifact.version, 1);
    assert!(!created_warn.mirror_synced);
    assert!(created_warn.mirror_sync_error.is_some());
    assert!(created_warn.artifact.last_synced_hash.is_none());

    engine
        .db()
        .writer()
        .execute_batch("DROP TRIGGER test_abort_create_sync;")
        .unwrap();
    let created_repaired = engine
        .save_artifact(
            &created_warn.artifact.id,
            SaveArtifactRequest {
                content_md: "# Created Sync Warn\n\nBody.",
                title: Some("Created Sync Warn"),
                actor: Actor::User,
                context_project_id: None,
                authorship: None,
                is_unfinished_draft: None,
            },
        )
        .unwrap();
    assert!(created_repaired.unchanged);
    assert!(created_repaired.repaired_mirror_only);
    assert!(created_repaired.mirror_synced);
    assert_eq!(created_repaired.artifact.version, 1);
    assert_eq!(
        created_repaired.artifact.last_synced_hash.as_deref(),
        Some(created_repaired.artifact.content_hash.as_str())
    );
}
