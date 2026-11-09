use std::sync::Arc;
use sthira_core::{
    references::{markdown_reference, parse_workspace_reference},
    ArtifactType, Authorship, Engine,
};
use sthira_desktop::{
    handle_get_workspace_entity, handle_search_all, handle_search_connection_targets, EntityKind,
    EntityRef,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
fn open(path: &std::path::Path) -> Engine {
    Engine::open(
        path.join("vault.db"),
        path.join("vault"),
        Arc::new(FakeClock::new(1000)),
        Arc::new(DeterministicFakeModel::new("no-inference").with_panic_on_call()),
    )
    .unwrap()
}
#[test]
fn internal_reference_urls_resolve_exact_saved_identity_and_remain_searchable_after_rename_and_reopen(
) {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Privacy Focused Distraction Blocking App"),
            "Original saved material.",
            None,
        )
        .unwrap();
    let other = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Privacy Focused Distraction Blocking App"),
            "A different note with the same name.",
            None,
        )
        .unwrap();
    let uri = format!("sthira://artifact/{}", note.id);
    let body = format!("## References\n\n1. Privacy Focused Distraction Blocking App – `{uri}`");
    let report = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::AiGenerated,
            Some("Generated report"),
            &body,
            None,
        )
        .unwrap();
    let p = engine.create_project("First context", None).unwrap();
    let q = engine.create_project("Second context", None).unwrap();
    engine
        .attach_artifact(&p.id, &note.id, "user_explicit", None)
        .unwrap();
    engine
        .attach_artifact(&q.id, &note.id, "user_explicit", None)
        .unwrap();
    for query in [
        &uri,
        &format!("`{uri}`"),
        &format!("<{}>", uri),
        &format!("#/sthira/artifact/{}", note.id),
        &note.id,
    ] {
        let results = handle_search_all(&engine, query.clone()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, note.id);
        assert_ne!(results[0].id, other.id);
        assert_ne!(results[0].id, report.id);
        assert!(results[0].project_id.is_some());
        let targets = handle_search_connection_targets(&engine, query.clone()).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].id, note.id);
    }
    engine
        .db()
        .writer()
        .execute(
            "UPDATE artifacts SET title='Renamed source' WHERE id=?1",
            [&note.id],
        )
        .unwrap();
    drop(engine);
    let reopened = open(temp.path());
    assert_eq!(
        handle_search_all(&reopened, uri).unwrap()[0].title,
        "Renamed source"
    );
    assert_eq!(reopened.get_artifact(&report.id).unwrap().content_md, body);
    let detail = handle_get_workspace_entity(
        &reopened,
        EntityRef {
            kind: EntityKind::Artifact,
            id: note.id,
        },
    )
    .unwrap();
    assert_eq!(detail.entity.title, "Renamed source");
    assert_eq!(detail.content_md, "Original saved material.");
    assert_eq!(
        reopened
            .db()
            .reader()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM workspace_links", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn project_references_and_missing_or_deleted_targets_have_exact_results() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let project = engine.create_project("Research project", None).unwrap();
    for query in [
        format!("sthira://project/{}", project.id),
        format!("#/sthira/project/{}", project.id),
        project.id.clone(),
    ] {
        let found = handle_search_all(&engine, query).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, "project");
        assert_eq!(found[0].id, project.id);
    }
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Removed source"),
            "Original body.",
            None,
        )
        .unwrap();
    engine
        .db()
        .writer()
        .execute("UPDATE artifacts SET deleted_at=10 WHERE id=?1", [&note.id])
        .unwrap();
    for query in [
        format!("sthira://artifact/{}", note.id),
        note.id,
        format!("sthira://artifact/{}", uuid::Uuid::new_v4()),
    ] {
        assert!(handle_search_all(&engine, query.clone())
            .unwrap()
            .is_empty());
        assert!(handle_search_connection_targets(&engine, query)
            .unwrap()
            .is_empty());
    }
}
#[test]
fn markdown_sources_are_named_links_with_escaped_titles_and_preserved_identity() {
    let url = "sthira://artifact/9ea18c81-36ab-4111-ae5d-a6e0250011fd";
    assert_eq!(
        markdown_reference("Privacy Focused App", url),
        format!("[Privacy Focused App]({url})")
    );
    assert_eq!(
        markdown_reference("A [source] *draft*", url),
        format!("[A \\[source\\] \\*draft\\*]({url})")
    );
    let parsed = parse_workspace_reference("`#/sthira/project/hello%20world`").unwrap();
    assert_eq!(parsed.id, "hello world");
    assert_eq!(parsed.kind, "project");
    for invalid in [
        "sthira://artifact/a/b",
        "sthira://artifact/%00",
        "sthira://artifact/%2Fetc",
        "sthira://other/id",
        "javascript:alert(1)",
        "sthira://artifact/%invalid",
    ] {
        assert!(parse_workspace_reference(invalid).is_none());
    }
}
