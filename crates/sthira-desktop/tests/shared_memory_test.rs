use std::sync::Arc;
use sthira_core::{shared_memory::*, Engine};
use sthira_desktop::{commands::handle_clear_vault_data, shared_memory::handle_memory_action};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
#[test]
fn ipc_owner_boundary_persists_and_reset_removes_memory_and_revisions() {
    let tmp = tempfile::tempdir().unwrap();
    let open = || {
        Engine::open(
            tmp.path().join("sthira.db"),
            tmp.path().join("vault"),
            Arc::new(FakeClock::new(1000)),
            Arc::new(DeterministicFakeModel::new("unused")),
        )
        .unwrap()
    };
    let engine = open();
    let MemoryResult::Entry(entry) = handle_memory_action(
        &engine,
        MemoryAction::Create {
            draft: MemoryDraft {
                scope: MemoryScope::Workspace,
                title: "A saved preference".into(),
                body: "Retain uncertain wording.".into(),
                source_ids: vec![],
                pinned: true,
            },
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(entry.author, MemoryActor::User);
    drop(engine);
    let engine = open();
    let MemoryResult::Entry(saved) = handle_memory_action(
        &engine,
        MemoryAction::Read {
            id: entry.id.clone(),
        },
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(saved.body, entry.body);
    handle_clear_vault_data(&engine).unwrap();
    assert!(handle_memory_action(&engine, MemoryAction::Read { id: entry.id }).is_err());
    assert_eq!(
        engine
            .db()
            .reader()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM shared_memory_revisions", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
