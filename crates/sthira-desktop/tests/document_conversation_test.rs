use std::sync::Arc;
use sthira_core::{Actor, ArtifactType, Authorship, Engine};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

#[tokio::test]
async fn discussion_is_durable_separate_and_edits_are_reviewed_and_conflict_checked() {
    let tmp = tempdir().unwrap();
    let db = tmp.path().join("sthira.db");
    let vault = tmp.path().join("vault");
    let model = Arc::new(DeterministicFakeModel::new("test-conversation"));
    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let engine = Engine::open(&db, &vault, clock.clone(), model.clone()).unwrap();
    engine
        .db()
        .writer()
        .execute_batch("DELETE FROM provider_models; DELETE FROM providers;")
        .unwrap();
    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Plan"),
            "# Plan\n\nOriginal",
            None,
        )
        .unwrap();
    let other = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Other"),
            "Other content",
            None,
        )
        .unwrap();
    assert!(engine
        .document_conversation(&other.id, None, "")
        .unwrap()
        .is_none());
    engine
        .document_conversation(&art.id, Some("plan"), "Make a plan")
        .unwrap();
    let prompt = serde_json::json!({"mode":"plan","original_goal":"Make a plan","document":art.content_md,"conversation":[],"message":"Shorten it","propose_edit":true});
    model.register_response(
        prompt.to_string(),
        r##"{"reply":"I suggest a shorter plan.","proposed_content_md":"# Plan\n\nShorter"}"##,
    );
    let chat = engine
        .discuss_document(&art.id, "Shorten it", None, true)
        .await
        .unwrap();
    assert_eq!(
        chat.turns[0].reply.as_deref(),
        Some("I suggest a shorter plan.")
    );
    assert_eq!(
        engine.get_artifact(&art.id).unwrap().content_md,
        art.content_md
    );
    let follow_up = serde_json::json!({"mode":"plan","original_goal":"Make a plan","document":art.content_md,"conversation":[{"user":"Shorten it","assistant":"I suggest a shorter plan."}],"message":"Why?","propose_edit":false});
    model.register_response(follow_up.to_string(), r##"{"reply":"It reduces the initial scope.","proposed_content_md":"Unrequested rewrite"}"##);
    let continued = engine
        .discuss_document(&art.id, "Why?", None, false)
        .await
        .unwrap();
    assert_eq!(
        continued.turns[1].reply.as_deref(),
        Some("It reduces the initial scope.")
    );
    assert!(continued.turns[1].proposed_content_md.is_none());
    assert!(engine
        .decide_document_edit(&other.id, &chat.turns[0].id, true)
        .is_err());
    let applied = engine
        .decide_document_edit(&art.id, &chat.turns[0].id, true)
        .unwrap();
    assert_eq!(applied.content_md, "# Plan\n\nShorter");
    assert!(engine
        .decide_document_edit(&art.id, &chat.turns[0].id, true)
        .is_err());
    let saved = engine
        .document_conversation(&art.id, None, "")
        .unwrap()
        .unwrap();
    let cs = saved.turns[0].change_set_id.as_deref().unwrap();
    engine.undo_document_edit(&art.id, cs).unwrap();
    if let Some(path) = &art.file_path {
        let mirrored = std::fs::read_to_string(vault.join(path)).unwrap();
        assert!(mirrored.contains("Original"));
        assert!(!mirrored.contains("Shorter"));
    }
    assert!(engine.undo_document_edit(&art.id, cs).is_err());
    assert_eq!(
        engine.get_artifact(&art.id).unwrap().content_md,
        art.content_md
    );
    let actor: String = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT actor FROM artifact_revisions WHERE change_set_id=?1",
            [cs],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(actor, "ai");
    drop(engine);
    let engine = Engine::open(&db, &vault, clock, model).unwrap();
    engine
        .db()
        .writer()
        .execute_batch("DELETE FROM provider_models; DELETE FROM providers;")
        .unwrap();
    let saved = engine
        .document_conversation(&art.id, None, "")
        .unwrap()
        .unwrap();
    assert_eq!(saved.initial_prompt, "Make a plan");
    assert_eq!(saved.turns.len(), 2);
    // A model/protocol failure keeps the attempted message and leaves the document alone.
    let failed = engine
        .discuss_document(&art.id, "Another question", None, false)
        .await
        .unwrap();
    assert!(failed.turns[2].error.is_some());
    assert_eq!(
        engine.get_artifact(&art.id).unwrap().content_md,
        art.content_md
    );
    // Build a pending suggestion and verify a newer document wins.
    engine
        .db()
        .writer()
        .execute(
            "UPDATE document_conversation_turns SET proposed_content_md='Replacement' WHERE id=?1",
            [&failed.turns[2].id],
        )
        .unwrap();
    engine
        .update_artifact(&art.id, "New user edit", Actor::User, None)
        .unwrap();
    assert!(engine
        .decide_document_edit(&art.id, &failed.turns[2].id, true)
        .unwrap_err()
        .contains("document changed"));
    engine
        .decide_document_edit(&art.id, &failed.turns[2].id, false)
        .unwrap();
    assert_eq!(
        engine.get_artifact(&art.id).unwrap().content_md,
        "New user edit"
    );
    assert_eq!(
        engine
            .document_conversation(&art.id, None, "")
            .unwrap()
            .unwrap()
            .turns[2]
            .proposal_state,
        "dismissed"
    );
}

#[test]
fn legacy_document_conversation_and_v13_history_survive_upgrade() {
    let tmp = tempdir().unwrap();
    let db = tmp.path().join("sthira.db");
    let vault = tmp.path().join("vault");
    let clock = Arc::new(FakeClock::new(1000));
    let model = Arc::new(DeterministicFakeModel::new("test-upgrade"));
    let engine = Engine::open(&db, &vault, clock.clone(), model.clone()).unwrap();
    let artifact = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Old document"),
            "Saved long before chat",
            None,
        )
        .unwrap();
    engine
        .document_conversation(&artifact.id, Some("plan"), "Original goal")
        .unwrap();
    engine.db().writer().execute("INSERT INTO document_conversation_turns(id,artifact_id,user_text,reply,basis_content_md,created_at) VALUES('saved-turn',?1,'Earlier question','Earlier answer','Saved long before chat',1000)",[&artifact.id]).unwrap();
    engine
        .db()
        .writer()
        .execute_batch("PRAGMA user_version=13")
        .unwrap();
    drop(engine);
    let engine = Engine::open(&db, &vault, clock, model).unwrap();
    let chat = engine
        .document_conversation(&artifact.id, None, "")
        .unwrap()
        .unwrap();
    assert_eq!(chat.turns[0].reply.as_deref(), Some("Earlier answer"));
    assert_eq!(chat.initial_prompt, "Original goal");
    let old_note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Unclassified"),
            "Old saved text",
            None,
        )
        .unwrap();
    let chat = engine
        .document_conversation(&old_note.id, Some("document"), "")
        .unwrap()
        .unwrap();
    assert_eq!(chat.mode, "document");
    assert!(chat.turns.is_empty());
    assert_eq!(
        engine.get_artifact(&old_note.id).unwrap().content_md,
        "Old saved text"
    );
}

#[test]
fn early_v14_without_model_columns_opens_empty_chat_and_preserves_history() {
    let tmp = tempdir().unwrap();
    let db = tmp.path().join("sthira.db");
    let vault = tmp.path().join("vault");
    let clock = Arc::new(FakeClock::new(1000));
    let model = Arc::new(DeterministicFakeModel::new("test-migration"));
    let engine = Engine::open(&db, &vault, clock.clone(), model.clone()).unwrap();
    let old = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Existing file"),
            "Keep my saved document",
            None,
        )
        .unwrap();
    let discussed = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Existing discussion"),
            "Saved context",
            None,
        )
        .unwrap();
    engine
        .document_conversation(&discussed.id, Some("plan"), "Earlier goal")
        .unwrap();
    engine.db().writer().execute(
        "INSERT INTO document_conversation_turns(id,artifact_id,user_text,reply,basis_content_md,created_at) VALUES('older-turn',?1,'Earlier question','Earlier answer','Saved context',1000)", [&discussed.id]).unwrap();
    // Reproduce the actual early-v14 shape: the version is current, but metadata columns are absent.
    engine
        .db()
        .writer()
        .execute_batch(
            "ALTER TABLE document_conversations DROP COLUMN model_id;
        ALTER TABLE document_conversations DROP COLUMN model_name;
        PRAGMA user_version=14;",
        )
        .unwrap();
    let error = engine
        .document_conversation(&old.id, Some("document"), "")
        .unwrap_err();
    assert!(error.contains("no such column: model_id"), "{error}");
    drop(engine);

    let engine = Engine::open(&db, &vault, clock.clone(), model.clone()).unwrap();
    let empty = engine
        .document_conversation(&old.id, Some("document"), "")
        .unwrap()
        .unwrap();
    assert!(empty.turns.is_empty());
    assert_eq!(empty.mode, "document");
    assert!(empty.model_id.is_none());
    assert!(empty.model_name.is_none());
    assert_eq!(
        engine.get_artifact(&old.id).unwrap().content_md,
        "Keep my saved document"
    );
    let history = engine
        .document_conversation(&discussed.id, None, "")
        .unwrap()
        .unwrap();
    assert_eq!(history.initial_prompt, "Earlier goal");
    assert_eq!(history.turns.len(), 1);
    assert_eq!(history.turns[0].reply.as_deref(), Some("Earlier answer"));
    assert_eq!(
        engine
            .db()
            .reader()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        19
    );
    drop(engine);
    let engine = Engine::open(&db, &vault, clock, model).unwrap();
    assert!(engine
        .document_conversation(&old.id, None, "")
        .unwrap()
        .unwrap()
        .turns
        .is_empty());
    assert_eq!(
        engine
            .document_conversation(&discussed.id, None, "")
            .unwrap()
            .unwrap()
            .turns
            .len(),
        1
    );
}

#[test]
fn complete_v14_model_metadata_survives_the_upgrade() {
    let tmp = tempdir().unwrap();
    let db = tmp.path().join("sthira.db");
    let vault = tmp.path().join("vault");
    let clock = Arc::new(FakeClock::new(1000));
    let model = Arc::new(DeterministicFakeModel::new("test-migration"));
    let engine = Engine::open(&db, &vault, clock.clone(), model.clone()).unwrap();
    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("File"),
            "Saved text",
            None,
        )
        .unwrap();
    engine
        .document_conversation(&art.id, Some("research"), "Original goal")
        .unwrap();
    engine
        .remember_conversation_model(&art.id, Some("selected-model"), "Selected model")
        .unwrap();
    engine
        .db()
        .writer()
        .execute_batch("PRAGMA user_version=14;")
        .unwrap();
    drop(engine);
    let engine = Engine::open(&db, &vault, clock, model).unwrap();
    let conversation = engine
        .document_conversation(&art.id, None, "")
        .unwrap()
        .unwrap();
    assert_eq!(conversation.model_id.as_deref(), Some("selected-model"));
    assert_eq!(conversation.model_name.as_deref(), Some("Selected model"));
}

#[test]
fn v15_history_gets_nullable_reply_attribution_without_relabeling_older_messages() {
    let tmp = tempdir().unwrap();
    let db = tmp.path().join("sthira.db");
    let vault = tmp.path().join("vault");
    let clock = Arc::new(FakeClock::new(1000));
    let model = Arc::new(DeterministicFakeModel::new("test-migration"));
    let engine = Engine::open(&db, &vault, clock.clone(), model.clone()).unwrap();
    let artifact = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Old chat"),
            "Saved text",
            None,
        )
        .unwrap();
    engine
        .document_conversation(&artifact.id, Some("document"), "")
        .unwrap();
    engine
        .remember_conversation_model(&artifact.id, Some("chosen-model"), "Chosen model")
        .unwrap();
    engine.db().writer().execute("INSERT INTO document_conversation_turns(id,artifact_id,user_text,reply,basis_content_md,created_at) VALUES('old-turn',?1,'Question','Older answer','Saved text',1000)",[&artifact.id]).unwrap();
    engine.db().writer().execute_batch("ALTER TABLE document_conversation_turns DROP COLUMN model_id; ALTER TABLE document_conversation_turns DROP COLUMN model_name; ALTER TABLE document_conversation_turns DROP COLUMN provider_id; ALTER TABLE document_conversation_turns DROP COLUMN provider_name; PRAGMA user_version=15;").unwrap();
    drop(engine);
    let engine = Engine::open(&db, &vault, clock.clone(), model.clone()).unwrap();
    let chat = engine
        .document_conversation(&artifact.id, None, "")
        .unwrap()
        .unwrap();
    assert_eq!(chat.model_id.as_deref(), Some("chosen-model"));
    assert_eq!(chat.turns[0].reply.as_deref(), Some("Older answer"));
    assert!(chat.turns[0].model_id.is_none());
    assert!(chat.turns[0].model_name.is_none());
    drop(engine);
    let engine = Engine::open(&db, &vault, clock, model).unwrap();
    assert_eq!(
        engine
            .document_conversation(&artifact.id, None, "")
            .unwrap()
            .unwrap()
            .turns
            .len(),
        1
    );
}
