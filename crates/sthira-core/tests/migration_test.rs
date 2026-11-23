use serde_json::json;
use std::sync::Arc;
use sthira_core::{migration::*, shared_memory::*, ArtifactType, Authorship, Engine};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
fn open(t: &tempfile::TempDir) -> Engine {
    Engine::open(
        t.path().join("sthira.db"),
        t.path().join("vault"),
        Arc::new(FakeClock::new(1000)),
        Arc::new(DeterministicFakeModel::new("unused").with_panic_on_call()),
    )
    .unwrap()
}
fn draft(key: &str, source: &str, files: Vec<(&str, &str)>) -> ImportDraft {
    ImportDraft {
        request_id: key.into(),
        source: source.into(),
        collection: "stable-export".into(),
        project_id: None,
        files: files
            .into_iter()
            .map(|(p, b)| ImportFile {
                path: p.into(),
                content: Some(b.into()),
                problem: None,
            })
            .collect(),
    }
}
fn apply(e: &Engine, r: &ImportRun) -> ImportRun {
    e.apply_migration(
        &r.id,
        r.revision,
        r.plan
            .items
            .iter()
            .filter(|i| i.status == "new")
            .map(|i| i.path.clone())
            .collect(),
    )
    .unwrap()
}
fn count(e: &Engine, table: &str) -> i64 {
    e.db()
        .reader()
        .unwrap()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
#[test]
fn all_source_presets_preview_and_import_unicode_without_model_or_original_file_mutation() {
    for source in ["notion", "obsidian", "google_docs", "markdown"] {
        let tmp = tempfile::tempdir().unwrap();
        let e = open(&tmp);
        let source_file = tmp.path().join("owner-export.md");
        let original = "# अनिश्चितता\r\n\r\nThe date is not confirmed.\r\n";
        std::fs::write(&source_file, original).unwrap();
        let r = e
            .preview_migration(draft("preview", source, vec![("Note.md", original)]))
            .unwrap();
        assert_eq!(count(&e, "artifacts"), 0);
        let r = apply(&e, &r);
        let a = e.get_artifact(&r.plan.items[0].artifact_id).unwrap();
        assert_eq!(a.authorship, Authorship::External);
        assert_eq!(a.title.as_deref(), Some("अनिश्चितता"));
        assert!(a.content_md.contains("not confirmed"));
        assert!(a.source_url.unwrap().contains(source));
        assert_eq!(std::fs::read_to_string(&source_file).unwrap(), original);
        assert_eq!(count(&e, "artifact_revisions"), 1);
        assert!(r.receipt.unwrap()["mirror_warnings"]
            .as_array()
            .unwrap()
            .is_empty());
    }
}
#[test]
fn explicit_links_resolve_by_identity_but_code_embeds_ambiguity_and_missing_targets_stay_literal() {
    let t = tempfile::tempdir().unwrap();
    let e = open(&t);
    let r=e.preview_migration(draft("links","obsidian",vec![("a.md","# A\n[Next](folder/B%20note.md) [[folder/B note|Named]]\n`[[missing]]`\n```md\n[[missing]]\n```\n![[picture.png]] [[unknown]]"),("folder/B note.md","# B\nUncertain.")])).unwrap();
    let b = &r.plan.items[1].artifact_id;
    assert!(r.plan.items[0]
        .preview_content
        .as_ref()
        .unwrap()
        .contains(b));
    assert!(!r.plan.items[0].warnings.is_empty());
    let r = apply(&e, &r);
    let a = e.get_artifact(&r.plan.items[0].artifact_id).unwrap();
    assert!(a.content_md.contains(&format!("sthira://artifact/{b}")));
    assert!(a.content_md.contains("`[[missing]]`"));
    assert!(a.content_md.contains("![[picture.png]]"));
    assert_eq!(count(&e, "workspace_links"), 1);
    assert_eq!(count(&e, "epistemic_nodes"), 0);
}
#[test]
fn code_spans_with_multiple_backticks_and_nested_fences_never_create_references() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(&tmp);
    let code = "# Code examples\n``[[Target]]``\n`[[Target]]`\n``multiline\n[[Target]]\n``\n````md\n```\n[[Target]]\n````\n    [[Target]]\n\t[[Target]]\n";
    let preview = engine
        .preview_migration(draft(
            "code",
            "obsidian",
            vec![("Examples.md", code), ("Target.md", "# Target")],
        ))
        .unwrap();
    assert_eq!(preview.plan.items[0].preview_content.as_deref(), Some(code));
    let _ = apply(&engine, &preview);
    assert_eq!(count(&engine, "workspace_links"), 0);
    let actual = engine
        .preview_migration(draft(
            "outside",
            "obsidian",
            vec![("Links.md", "``[[Target]]`` then [[Target]]")],
        ))
        .unwrap();
    let content = actual.plan.items[0].preview_content.as_ref().unwrap();
    assert!(content.starts_with("``[[Target]]`` then [Target](sthira://artifact/"));
    let _ = apply(&engine, &actual);
    assert_eq!(count(&engine, "workspace_links"), 1);
}
#[test]
fn incremental_import_links_to_live_prior_sources_in_the_same_collection_only() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(&tmp);
    let first = apply(
        &engine,
        &engine
            .preview_migration(draft(
                "first",
                "obsidian",
                vec![("folder/Target.md", "# Target")],
            ))
            .unwrap(),
    );
    let target = &first.plan.items[0].artifact_id;
    let second = engine
        .preview_migration(draft("next", "obsidian", vec![("New.md", "[[Target]]")]))
        .unwrap();
    assert!(second.plan.items[0]
        .preview_content
        .as_ref()
        .unwrap()
        .contains(target));
    let second = apply(&engine, &second);
    assert!(engine
        .get_artifact(&second.plan.items[0].artifact_id)
        .unwrap()
        .content_md
        .contains(target));
    assert_eq!(count(&engine, "workspace_links"), 1);
    // The new incoming reference prevents rollback from discarding the earlier target.
    assert!(engine
        .rollback_migration(&first.id, first.revision)
        .is_err());
    let other = engine
        .preview_migration(draft(
            "other-source",
            "notion",
            vec![("Other.md", "[[Target]]")],
        ))
        .unwrap();
    assert_eq!(
        other.plan.items[0].preview_content.as_deref(),
        Some("[[Target]]")
    );
    let mut other_collection = draft(
        "other-collection",
        "obsidian",
        vec![("Other.md", "[[Target]]")],
    );
    other_collection.collection = "different-export".into();
    let other = engine.preview_migration(other_collection).unwrap();
    assert_eq!(
        other.plan.items[0].preview_content.as_deref(),
        Some("[[Target]]")
    );
    engine
        .db()
        .writer()
        .execute("UPDATE artifacts SET deleted_at=2000 WHERE id=?1", [target])
        .unwrap();
    let missing = engine
        .preview_migration(draft(
            "deleted",
            "obsidian",
            vec![("Missing.md", "[[Target]]")],
        ))
        .unwrap();
    assert_eq!(
        missing.plan.items[0].preview_content.as_deref(),
        Some("[[Target]]")
    );
}
#[test]
fn duplicate_retry_changed_sources_and_owner_edits_are_not_silently_overwritten() {
    let t = tempfile::tempdir().unwrap();
    let e = open(&t);
    let input = draft("once", "notion", vec![("a.md", "# First\nUncertain.")]);
    let preview = e.preview_migration(input.clone()).unwrap();
    assert_eq!(e.preview_migration(input).unwrap().id, preview.id);
    let first = apply(&e, &preview);
    assert_eq!(apply(&e, &preview).id, first.id);
    assert_eq!(count(&e, "artifacts"), 1);
    let duplicate = e
        .preview_migration(draft(
            "again",
            "notion",
            vec![("a.md", "# First\nUncertain.")],
        ))
        .unwrap();
    assert_eq!(duplicate.plan.items[0].status, "duplicate");
    let conflict = e
        .preview_migration(draft(
            "changed",
            "notion",
            vec![("a.md", "# Changed\nDifferent source.")],
        ))
        .unwrap();
    assert_eq!(conflict.plan.items[0].status, "conflict");
    assert!(e
        .apply_migration(&conflict.id, 1, vec!["a.md".into()])
        .is_err());
    assert!(e
        .get_artifact(&first.plan.items[0].artifact_id)
        .unwrap()
        .content_md
        .contains("Uncertain."));
}
#[test]
fn unsupported_formats_and_invalid_paths_are_reported_or_rejected_before_writes() {
    let t = tempfile::tempdir().unwrap();
    let e = open(&t);
    let r = e
        .preview_migration(draft(
            "formats",
            "google_docs",
            vec![
                ("a.docx", "binary"),
                ("b.html", "<script>evil()</script>"),
                ("db.csv", "A,B"),
                (".obsidian/config.md", "settings"),
                ("empty.txt", ""),
            ],
        ))
        .unwrap();
    assert!(r.plan.items.iter().all(|i| i.status == "unsupported"));
    assert_eq!(count(&e, "artifacts"), 0);
    for p in [
        "../escape.md",
        "/absolute.md",
        "a/../../escape.md",
        "a\\b.md",
        "a/../b.md",
    ] {
        assert!(e
            .preview_migration(draft(p, "markdown", vec![(p, "Text")]))
            .is_err());
    }
    let r = e
        .preview_migration(draft("unicode", "markdown", vec![("百分.md", "[%é](%é)")]))
        .unwrap();
    let _ = apply(&e, &r);
}
#[test]
fn failure_receipt_rolls_back_all_pages_references_membership_and_provenance() {
    let t = tempfile::tempdir().unwrap();
    let e = open(&t);
    let p = e.create_project("Destination", None).unwrap();
    let mut d = draft(
        "atomic",
        "markdown",
        vec![("a.md", "[[b]]"), ("b.md", "Body")],
    );
    d.project_id = Some(p.id);
    let r = e.preview_migration(d).unwrap();
    e.db().writer().execute_batch("CREATE TRIGGER fail_import_receipt BEFORE UPDATE OF status ON migration_imports BEGIN SELECT RAISE(ABORT,'fixture failure'); END").unwrap();
    assert!(e
        .apply_migration(&r.id, 1, vec!["a.md".into(), "b.md".into()])
        .is_err());
    for table in [
        "artifacts",
        "artifact_revisions",
        "migration_sources",
        "workspace_links",
        "project_artifacts",
    ] {
        assert_eq!(count(&e, table), 0, "{table}");
    }
    assert_eq!(e.migration_run(&r.id).unwrap().status, "preview");
}
#[test]
fn guarded_rollback_preserves_newer_bodies_memberships_and_memory_reuse() {
    for use_kind in ["edit", "membership", "memory"] {
        let t = tempfile::tempdir().unwrap();
        let e = open(&t);
        let r = apply(
            &e,
            &e.preview_migration(draft("recover", "markdown", vec![("a.md", "Original")]))
                .unwrap(),
        );
        let a = &r.plan.items[0].artifact_id;
        match use_kind {
            "edit" => {
                e.db()
                    .writer()
                    .execute(
                        "UPDATE artifacts SET version=2,content_md='New owner work' WHERE id=?1",
                        [a],
                    )
                    .unwrap();
            }
            "membership" => {
                let p = e.create_project("New use", None).unwrap();
                e.attach_artifact(&p.id, a, "user_explicit", None).unwrap();
                e.detach_artifact(&p.id, a).unwrap();
            }
            _ => {
                e.execute_memory_action(
                    &MemoryAccess::owner(),
                    MemoryAction::Create {
                        draft: MemoryDraft {
                            scope: MemoryScope::Workspace,
                            title: "Remember source".into(),
                            body: "An open question.".into(),
                            source_ids: vec![a.clone()],
                            pinned: false,
                        },
                    },
                )
                .unwrap();
            }
        }
        assert!(
            e.rollback_migration(&r.id, r.revision).is_err(),
            "{use_kind}"
        );
        assert!(e.get_artifact(a).unwrap().deleted_at.is_none());
    }
}
#[test]
fn rollback_removes_only_imported_work_and_allows_a_fresh_reimport() {
    let t = tempfile::tempdir().unwrap();
    let e = open(&t);
    let existing = e
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Owner"),
            "Keep me",
            None,
        )
        .unwrap();
    let r = apply(
        &e,
        &e.preview_migration(draft(
            "recover",
            "markdown",
            vec![("a.md", "[[b]]"), ("b.md", "Text")],
        ))
        .unwrap(),
    );
    let rolled = e.rollback_migration(&r.id, r.revision).unwrap();
    assert_eq!(rolled.status, "rolled_back");
    assert_eq!(e.get_artifact(&existing.id).unwrap().content_md, "Keep me");
    assert!(r
        .plan
        .items
        .iter()
        .all(|i| e.get_artifact(&i.artifact_id).is_err()));
    assert_eq!(
        e.db()
            .reader()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM artifacts WHERE deleted_at IS NOT NULL",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
    assert_eq!(count(&e, "migration_sources"), 0);
    let fresh = e
        .preview_migration(draft(
            "fresh",
            "markdown",
            vec![("a.md", "[[b]]"), ("b.md", "Text")],
        ))
        .unwrap();
    assert!(fresh.plan.items.iter().all(|i| i.status == "new"));
}
#[test]
fn preview_cancellation_destination_conflicts_and_mirror_collisions_are_honest() {
    let t = tempfile::tempdir().unwrap();
    let e = open(&t);
    let r = e
        .preview_migration(draft("cancel", "markdown", vec![("a.md", "Text")]))
        .unwrap();
    e.cancel_migration(&r.id, r.revision).unwrap();
    assert!(e
        .apply_migration(&r.id, r.revision, vec!["a.md".into()])
        .is_err());
    let p = e.create_project("Destination", None).unwrap();
    let mut d = draft("project", "markdown", vec![("a.md", "Text")]);
    d.project_id = Some(p.id.clone());
    let r = e.preview_migration(d).unwrap();
    e.set_project_purpose(&p.id, "Changed direction").unwrap();
    assert!(e.apply_migration(&r.id, 1, vec!["a.md".into()]).is_err());
    let r = e
        .preview_migration(draft("collision", "markdown", vec![("a.md", "Text")]))
        .unwrap();
    let mirror = e
        .vault_dir()
        .join(format!("{}.md", r.plan.items[0].artifact_id));
    std::fs::write(&mirror, "Unrelated owner data").unwrap();
    let applied = apply(&e, &r);
    assert_eq!(
        std::fs::read_to_string(mirror).unwrap(),
        "Unrelated owner data"
    );
    assert_eq!(
        applied.receipt.unwrap()["mirror_warnings"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn mirror_failure_remains_visible_even_if_postcommit_receipt_update_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(&tmp);
    let preview = engine
        .preview_migration(draft(
            "mirror-receipt",
            "markdown",
            vec![("a.md", "First"), ("b.md", "Second")],
        ))
        .unwrap();
    let collision = engine
        .vault_dir()
        .join(format!("{}.md", preview.plan.items[0].artifact_id));
    std::fs::write(&collision, "Owner mirror edits").unwrap();
    engine.db().writer().execute_batch("CREATE TRIGGER fail_mirror_receipt BEFORE UPDATE OF receipt_json ON migration_imports WHEN OLD.status='applied' BEGIN SELECT RAISE(ABORT,'fixture postcommit failure'); END").unwrap();
    let applied = apply(&engine, &preview);
    assert_eq!(count(&engine, "artifacts"), 2);
    let warnings = applied.receipt.as_ref().unwrap()["mirror_warnings"]
        .as_array()
        .unwrap();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0]["error"].as_str().unwrap().contains("preserved"));
    assert_eq!(
        std::fs::read_to_string(collision).unwrap(),
        "Owner mirror edits"
    );
    // Even after reopen/retry, the durable receipt never falsely asserts mirror success.
    drop(engine);
    let engine = open(&tmp);
    let retried = apply(&engine, &preview);
    let warnings = retried.receipt.as_ref().unwrap()["mirror_warnings"]
        .as_array()
        .unwrap();
    assert_eq!(warnings.len(), 2);
    assert!(warnings.iter().all(|w| w["error"]
        .as_str()
        .unwrap()
        .contains("not yet been confirmed")));
    assert_eq!(count(&engine, "artifacts"), 2);
}
#[test]
fn preview_and_receipts_survive_reopen_and_v26_upgrade_is_additive() {
    let t = tempfile::tempdir().unwrap();
    let e = open(&t);
    let owner = e
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Owner"),
            "Keep existing work",
            None,
        )
        .unwrap();
    // Reproduce the actual prior schema: neither AE5 table exists yet.
    e.db()
        .writer()
        .execute_batch(
            "DROP TABLE migration_sources; DROP TABLE migration_imports; PRAGMA user_version=26;",
        )
        .unwrap();
    drop(e);
    let e = open(&t);
    assert_eq!(
        e.get_artifact(&owner.id).unwrap().content_md,
        "Keep existing work"
    );
    let r = e
        .preview_migration(draft(
            "persist",
            "markdown",
            vec![("a.md", "Keep uncertainty")],
        ))
        .unwrap();
    drop(e);
    let e = open(&t);
    let applied = apply(&e, &e.migration_run(&r.id).unwrap());
    drop(e);
    let e = open(&t);
    assert_eq!(e.migration_run(&r.id).unwrap().status, "applied");
    assert_eq!(
        e.get_artifact(&applied.plan.items[0].artifact_id)
            .unwrap()
            .version,
        1
    );
    assert_eq!(
        e.db()
            .reader()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        27
    );
    assert!(serde_json::from_value::<ImportDraft>(json!({"request_id":"x","source":"markdown","collection":"x","project_id":null,"files":[],"owner":true})).is_err());
}
