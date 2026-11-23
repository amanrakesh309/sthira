use std::sync::Arc;
use sthira_core::{shared_memory::*, ArtifactType, Authorship, Engine};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
fn open(root: &std::path::Path) -> Engine {
    Engine::open(
        root.join("sthira.db"),
        root.join("vault"),
        Arc::new(FakeClock::new(1000)),
        Arc::new(DeterministicFakeModel::new("unused")),
    )
    .unwrap()
}
fn create(
    engine: &Engine,
    access: &MemoryAccess,
    scope: MemoryScope,
    title: &str,
    source_ids: Vec<String>,
) -> MemoryEntry {
    match engine
        .execute_memory_action(
            access,
            MemoryAction::Create {
                draft: MemoryDraft {
                    scope,
                    title: title.into(),
                    body: "Keep the original uncertainty; this is a contribution.".into(),
                    source_ids,
                    pinned: false,
                },
            },
        )
        .unwrap()
    {
        MemoryResult::Entry(entry) => entry,
        _ => panic!("entry expected"),
    }
}
fn list(engine: &Engine, access: &MemoryAccess, query: MemoryQuery) -> Vec<MemoryEntry> {
    match engine
        .execute_memory_action(access, MemoryAction::List { query })
        .unwrap()
    {
        MemoryResult::Entries(entries) => entries,
        _ => panic!(),
    }
}
fn agent(project_ids: Vec<String>) -> MemoryAccess {
    MemoryAccess {
        actor: MemoryActor::Agent {
            agent_id: "reader-agent".into(),
            model: "chosen-model".into(),
            provider: "configured-provider".into(),
        },
        owner: false,
        workspace: false,
        project_ids,
        session_ids: vec![],
        can_write: true,
    }
}
#[test]
fn migration_reopen_and_attribution_are_durable_without_touching_notes() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Research notebook", None).unwrap();
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Original"),
            "An unchanged source",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &note.id, "user_explicit", None)
        .unwrap();
    let path = engine.vault_dir().join(note.file_path.as_ref().unwrap());
    let original = std::fs::read(&path).unwrap();
    let mut events = engine.subscribe();
    let mut access = agent(vec![project.id.clone()]);
    let memory = create(
        &engine,
        &access,
        MemoryScope::Project(project.id.clone()),
        "A useful handoff",
        vec![note.id.clone()],
    );
    assert!(matches!(
        events.try_recv().unwrap(),
        sthira_core::CoreEvent::MemoryChanged { .. }
    ));
    assert!(matches!(memory.author, MemoryActor::Agent { .. }));
    access.can_write = false;
    assert!(engine
        .execute_memory_action(
            &access,
            MemoryAction::Archive {
                id: memory.id.clone(),
                expected_version: 1
            }
        )
        .is_err());
    drop(engine);
    let engine = open(tmp.path());
    let entries = list(&engine, &MemoryAccess::owner(), MemoryQuery::default());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, memory.id);
    assert_eq!(entries[0].sources[0].status, "current");
    assert_eq!(std::fs::read(path).unwrap(), original);
    let writer = engine.db().writer();
    assert_eq!(
        writer
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        27
    );
    assert_eq!(
        writer
            .query_row("SELECT COUNT(*) FROM epistemic_nodes", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn scope_grants_apply_to_reads_search_history_and_agent_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let owner = MemoryAccess::owner();
    let a = engine.create_project("First project", None).unwrap();
    let b = engine.create_project("Second project", None).unwrap();
    create(
        &engine,
        &owner,
        MemoryScope::Workspace,
        "Workspace preference",
        vec![],
    );
    let private = create(
        &engine,
        &owner,
        MemoryScope::Project(b.id.clone()),
        "Private memory",
        vec![],
    );
    let protected = create(
        &engine,
        &owner,
        MemoryScope::Project(a.id.clone()),
        "Human wording",
        vec![],
    );
    create(
        &engine,
        &owner,
        MemoryScope::Session("session-one".into()),
        "Session handoff",
        vec![],
    );
    let access = agent(vec![a.id.clone()]);
    let visible = list(&engine, &access, MemoryQuery::default());
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id, protected.id);
    assert!(engine
        .execute_memory_action(
            &access,
            MemoryAction::Read {
                id: private.id.clone()
            }
        )
        .is_err());
    assert!(engine
        .execute_memory_action(&access, MemoryAction::History { id: private.id })
        .is_err());
    assert!(engine
        .execute_memory_action(
            &access,
            MemoryAction::Update {
                id: protected.id,
                expected_version: 1,
                title: "Override".into(),
                body: "Changed".into(),
                pinned: false
            }
        )
        .is_err());
    assert!(engine
        .execute_memory_action(
            &access,
            MemoryAction::Create {
                draft: MemoryDraft {
                    scope: MemoryScope::Project(b.id),
                    title: "No".into(),
                    body: "No".into(),
                    source_ids: vec![],
                    pinned: false
                }
            }
        )
        .is_err());
    let own = create(
        &engine,
        &access,
        MemoryScope::Project(a.id),
        "Agent-owned memory",
        vec![],
    );
    let changed = engine
        .execute_memory_action(
            &owner,
            MemoryAction::Update {
                id: own.id.clone(),
                expected_version: 1,
                title: "Reviewed wording".into(),
                body: "A user edited this contribution.".into(),
                pinned: true,
            },
        )
        .unwrap();
    let MemoryResult::Entry(changed) = changed else {
        panic!()
    };
    assert_eq!(changed.author, access.actor);
    let MemoryResult::History(history) = engine
        .execute_memory_action(&owner, MemoryAction::History { id: own.id })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].editor, MemoryActor::User);
    assert_eq!(history[1].editor, access.actor);
}
#[test]
fn stale_writes_archive_restore_and_revision_failures_are_atomic() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let owner = MemoryAccess::owner();
    let memory = create(
        &engine,
        &owner,
        MemoryScope::Workspace,
        "Working preference",
        vec![],
    );
    engine
        .execute_memory_action(
            &owner,
            MemoryAction::Archive {
                id: memory.id.clone(),
                expected_version: 1,
            },
        )
        .unwrap();
    assert!(list(&engine, &owner, MemoryQuery::default()).is_empty());
    assert_eq!(
        list(
            &engine,
            &owner,
            MemoryQuery {
                include_archived: true,
                ..Default::default()
            }
        )
        .len(),
        1
    );
    assert!(engine
        .execute_memory_action(
            &owner,
            MemoryAction::Restore {
                id: memory.id.clone(),
                expected_version: 1
            }
        )
        .is_err());
    engine
        .execute_memory_action(
            &owner,
            MemoryAction::Restore {
                id: memory.id.clone(),
                expected_version: 2,
            },
        )
        .unwrap();
    let mut events = engine.subscribe();
    engine.db().writer().execute_batch("CREATE TRIGGER fail_memory_revision BEFORE INSERT ON shared_memory_revisions BEGIN SELECT RAISE(ABORT,'injected disk/database failure'); END;").unwrap();
    assert!(engine
        .execute_memory_action(
            &owner,
            MemoryAction::Update {
                id: memory.id,
                expected_version: 3,
                title: "Lost title".into(),
                body: "Never committed".into(),
                pinned: true
            }
        )
        .is_err());
    let saved = list(&engine, &owner, MemoryQuery::default());
    assert_eq!(saved[0].version, 3);
    assert_eq!(saved[0].title, "Working preference");
    assert!(!saved[0].pinned);
    assert!(events.try_recv().is_err());
}
#[test]
fn context_excludes_changed_deleted_archived_and_detached_sources_and_respects_budget() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let owner = MemoryAccess::owner();
    let project = engine.create_project("A topic", None).unwrap();
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Original source"),
            "Original saved prose",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &note.id, "user_explicit", None)
        .unwrap();
    create(
        &engine,
        &owner,
        MemoryScope::Project(project.id.clone()),
        "Source handoff",
        vec![note.id.clone()],
    );
    create(
        &engine,
        &owner,
        MemoryScope::Workspace,
        "General preference",
        vec![],
    );
    let context = |max_chars| match engine
        .execute_memory_action(
            &owner,
            MemoryAction::Context {
                query: MemoryQuery {
                    include_archived: true,
                    ..Default::default()
                },
                max_entries: 20,
                max_chars,
            },
        )
        .unwrap()
    {
        MemoryResult::Context(c) => c,
        _ => panic!(),
    };
    let initial = context(24000);
    assert_eq!(initial.entries.len(), 2);
    assert!(initial.content_is_untrusted);
    assert!(initial.used_chars <= 24000);
    assert!(context(0).entries.is_empty());
    assert_eq!(context(0).skipped_budget, 2);
    engine
        .db()
        .writer()
        .execute(
            "UPDATE artifacts SET content_hash='different' WHERE id=?1",
            [&note.id],
        )
        .unwrap();
    assert_eq!(context(24000).skipped_stale, 1);
    assert_eq!(context(24000).entries.len(), 1);
    engine
        .db()
        .writer()
        .execute("UPDATE artifacts SET deleted_at=1 WHERE id=?1", [&note.id])
        .unwrap();
    assert_eq!(
        list(
            &engine,
            &owner,
            MemoryQuery {
                scope: Some(MemoryScope::Project(project.id.clone())),
                ..Default::default()
            }
        )[0]
        .sources[0]
            .status,
        "unavailable"
    );
    engine
        .db()
        .writer()
        .execute(
            "UPDATE artifacts SET deleted_at=NULL,content_hash=?2 WHERE id=?1",
            rusqlite::params![note.id, note.content_hash],
        )
        .unwrap();
    engine
        .db()
        .writer()
        .execute(
            "UPDATE project_artifacts SET detached_at=1001 WHERE project_id=?1",
            [&project.id],
        )
        .unwrap();
    assert_eq!(context(24000).skipped_stale, 1);
    engine
        .db()
        .writer()
        .execute(
            "UPDATE projects SET deleted_at=1002 WHERE id=?1",
            [&project.id],
        )
        .unwrap();
    let retained = list(&engine, &owner, MemoryQuery::default());
    assert_eq!(retained.len(), 2);
    let orphan = retained
        .iter()
        .find(|entry| !entry.scope_available)
        .unwrap();
    assert!(matches!(orphan.scope, MemoryScope::Project(_)));
    assert!(engine
        .execute_memory_action(
            &owner,
            MemoryAction::History {
                id: orphan.id.clone()
            }
        )
        .is_ok());
    assert!(engine
        .execute_memory_action(
            &agent(vec![project.id]),
            MemoryAction::Read {
                id: orphan.id.clone()
            }
        )
        .is_err());
    assert_eq!(context(24000).entries.len(), 1);
    assert_eq!(context(24000).skipped_stale, 1);
}
#[test]
fn invalid_inputs_and_provenance_payloads_are_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let owner = MemoryAccess::owner();
    for (scope, title, body, sources) in [
        (
            MemoryScope::Project("missing".into()),
            "Name",
            "Body",
            vec![],
        ),
        (MemoryScope::Session(" ".into()), "Name", "Body", vec![]),
        (MemoryScope::Workspace, " ", "Body", vec![]),
        (MemoryScope::Workspace, "Name", " ", vec![]),
        (
            MemoryScope::Workspace,
            "Name",
            "Body",
            vec!["missing".into()],
        ),
    ] {
        assert!(engine
            .execute_memory_action(
                &owner,
                MemoryAction::Create {
                    draft: MemoryDraft {
                        scope,
                        title: title.into(),
                        body: body.into(),
                        source_ids: sources,
                        pinned: false
                    }
                }
            )
            .is_err());
    }
    assert!(list(&engine, &owner, MemoryQuery::default()).is_empty());
    let forged = r#"{"type":"create","draft":{"scope":{"kind":"workspace"},"title":"A title","body":"A body","author":{"kind":"user"}}}"#;
    assert!(serde_json::from_str::<MemoryAction>(forged).is_err());
    let m = create(
        &engine,
        &owner,
        MemoryScope::Workspace,
        "Original identity",
        vec![],
    );
    assert!(engine
        .db()
        .writer()
        .execute(
            "UPDATE shared_memories SET scope_kind='session',scope_id='elsewhere' WHERE id=?1",
            [&m.id]
        )
        .is_err());
    assert!(engine
        .db()
        .writer()
        .execute(
            "UPDATE shared_memories SET author_json='{}' WHERE id=?1",
            [&m.id]
        )
        .is_err());
}

#[test]
fn upgrading_a_v19_vault_is_additive_and_repeatable() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Existing writing"),
            "Keep every saved word.",
            None,
        )
        .unwrap();
    let project = engine.create_project("Existing project", None).unwrap();
    engine
        .attach_artifact(&project.id, &note.id, "user_explicit", None)
        .unwrap();
    let bytes = std::fs::read(engine.vault_dir().join(note.file_path.as_ref().unwrap())).unwrap();
    engine.db().writer().execute_batch("DROP TRIGGER shared_memory_identity_immutable; DROP TABLE shared_memory_revisions; DROP TABLE shared_memories; PRAGMA user_version=19;").unwrap();
    drop(engine);
    for _ in 0..2 {
        let engine = open(tmp.path());
        let reader = engine.db().reader().unwrap();
        assert_eq!(
            reader
                .query_row(
                    "SELECT content_md FROM artifacts WHERE id=?1",
                    [&note.id],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            note.content_md
        );
        assert_eq!(
            reader
                .query_row(
                    "SELECT COUNT(*) FROM project_artifacts WHERE artifact_id=?1 AND project_id=?2",
                    rusqlite::params![note.id, project.id],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(
            std::fs::read(engine.vault_dir().join(note.file_path.as_ref().unwrap())).unwrap(),
            bytes
        );
        assert!(list(&engine, &MemoryAccess::owner(), MemoryQuery::default()).is_empty());
    }
}
