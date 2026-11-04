use std::sync::Arc;
use sthira_core::{ArtifactType, Authorship, CanvasItemKind, Engine};
use sthira_desktop::*;
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::{tempdir, TempDir};

fn setup() -> (TempDir, Engine) {
    let tmp = tempdir().unwrap();
    let engine = Engine::open(tmp.path().join("vault.db"), tmp.path(), Arc::new(FakeClock::new(1_700_000_000_000)), Arc::new(DeterministicFakeModel::new("test-model"))).unwrap();
    (tmp, engine)
}
fn artifact(id: &str) -> EntityRef { EntityRef { kind: EntityKind::Artifact, id: id.into() } }
fn project(id: &str) -> EntityRef { EntityRef { kind: EntityKind::Project, id: id.into() } }
fn page(engine: &Engine, title: &str) -> String {
    engine.create_artifact(ArtifactType::Note, Authorship::User, Some(title), &format!("# {title}\n\nSaved project content."), None).unwrap().id
}

#[test]
fn overview_contains_all_member_types_and_resolves_shared_items_in_the_requested_project() {
    let (_tmp, engine) = setup();
    let p = engine.create_project("Platform", None).unwrap().id;
    let other = engine.create_project("Other", None).unwrap().id;
    engine.set_project_purpose(&p, "Build a connected knowledge workspace").unwrap();
    let shared = page(&engine, "Shared design");
    engine.attach_artifact(&other, &shared, "user_explicit", None).unwrap();
    engine.attach_artifact(&p, &shared, "user_explicit", None).unwrap();
    let object = handle_create_typed_object(&engine, "idea".into(), "Explore links".into(), serde_json::json!({"status":"spark"}), Some(p.clone())).unwrap();
    let source = engine.create_artifact(ArtifactType::Source, Authorship::External, Some("Specification"), "Quoted specification", Some("https://example.com/spec")).unwrap();
    let snippet = engine.create_artifact(ArtifactType::Snippet, Authorship::User, Some("A snippet"), "A saved fragment", None).unwrap();
    for id in [&source.id, &snippet.id] { engine.attach_artifact(&p, id, "user_explicit", None).unwrap(); }
    let canvas = engine.create_canvas(Some(&p), "Concept map").unwrap();
    let overview = handle_get_project_overview(&engine, p.clone()).unwrap();
    assert_eq!(overview.project.purpose, "Build a connected knowledge workspace");
    assert_eq!(overview.items.len(), 5);
    assert!(overview.items.iter().all(|item| item.roles == ["member"]));
    assert_eq!(overview.items.iter().find(|item| item.id == shared).unwrap().project_id.as_deref(), Some(p.as_str()));
    let item = overview.items.iter().find(|item| item.id == object.artifact_id).unwrap();
    assert_eq!(item.item_type, "object"); assert_eq!(item.object_type_name.as_deref(), Some("Idea"));
    assert!(overview.items.iter().any(|item| item.id == source.id && item.item_type == "source"));
    assert!(overview.items.iter().any(|item| item.id == snippet.id && item.item_type == "snippet"));
    assert!(overview.items.iter().any(|item| item.id == canvas && item.kind == "canvas"));
}

#[test]
fn explicit_references_and_backlinks_are_deduplicated_without_changing_membership() {
    let (_tmp, engine) = setup();
    let p = engine.create_project("Platform", None).unwrap().id;
    let q = engine.create_project("Research", None).unwrap().id;
    let member = page(&engine, "A member"); let external = page(&engine, "External reference");
    engine.attach_artifact(&p, &member, "user_explicit", None).unwrap();
    engine.attach_artifact(&q, &external, "user_explicit", None).unwrap();
    for target in [artifact(&member), artifact(&external), project(&q)] {
        handle_create_workspace_connection(&engine, project(&p), target.clone()).unwrap();
        handle_create_workspace_connection(&engine, target, project(&p)).unwrap();
    }
    let before: i64 = engine.db().reader().unwrap().query_row("SELECT COUNT(*) FROM project_artifacts", [], |row| row.get(0)).unwrap();
    let overview = handle_get_project_overview(&engine, p.clone()).unwrap();
    assert_eq!(overview.items.len(), 3);
    assert_eq!(overview.items.iter().find(|item| item.id == member).unwrap().roles, ["member", "reference", "backlink"]);
    let referenced = overview.items.iter().find(|item| item.id == external).unwrap();
    assert_eq!(referenced.roles, ["reference", "backlink"]);
    assert_eq!(referenced.project_id.as_deref(), Some(q.as_str()));
    let related = overview.items.iter().find(|item| item.kind == "project").unwrap();
    assert_eq!(related.roles, ["reference", "backlink"]); assert_eq!(related.item_count, Some(1));
    assert_eq!(engine.db().reader().unwrap().query_row("SELECT COUNT(*) FROM project_artifacts", [], |row| row.get::<_, i64>(0)).unwrap(), before);
    assert_eq!(engine.get_artifact(&member).unwrap().content_md, "# A member\n\nSaved project content.");
}

#[test]
fn detached_deleted_and_unrelated_items_do_not_leak_into_the_directory() {
    let (_tmp, engine) = setup(); let p = engine.create_project("Project", None).unwrap().id;
    let detached = page(&engine, "Detached"); let deleted = page(&engine, "Deleted"); let unrelated = page(&engine, "Unrelated");
    for id in [&detached, &deleted] { engine.attach_artifact(&p, id, "user_explicit", None).unwrap(); }
    handle_detach_artifact_from_project(&engine, detached.clone(), p.clone()).unwrap();
    engine.db().writer().execute("UPDATE artifacts SET deleted_at = 1 WHERE id = ?1", [&deleted]).unwrap();
    handle_create_workspace_connection(&engine, project(&p), artifact(&unrelated)).unwrap();
    let links = handle_get_workspace_connections(&engine, project(&p)).unwrap();
    let overview = handle_get_project_overview(&engine, p.clone()).unwrap();
    assert_eq!(overview.items.len(), 1); assert_eq!(overview.items[0].id, unrelated);
    assert_eq!(overview.items[0].roles, ["reference"]);
    handle_delete_workspace_connection(&engine, links[0].id.clone()).unwrap();
    assert!(handle_get_project_overview(&engine, p).unwrap().items.is_empty());
}

#[test]
fn canvas_placement_does_not_implicitly_add_artifacts_and_canvas_edit_time_is_honest() {
    let (_tmp, engine) = setup(); let p = engine.create_project("Project", None).unwrap().id;
    let loose = page(&engine, "A loose page");
    let canvas = engine.create_canvas(Some(&p), "Board").unwrap();
    engine.add_canvas_item(&canvas, "placed", CanvasItemKind::Artifact, Some(&loose), 1.0, 2.0, None, None, 0).unwrap();
    engine.db().writer().execute("INSERT INTO canvas_layout_revisions (id, canvas_id, layout, actor, created_at) VALUES ('edit', ?1, '{}', 'user', 1700000000100)", [&canvas]).unwrap();
    let overview = handle_get_project_overview(&engine, p).unwrap();
    assert_eq!(overview.items.len(), 1); assert_eq!(overview.items[0].id, canvas);
    assert_eq!(overview.items[0].item_count, Some(1)); assert_eq!(overview.items[0].updated_at, 1_700_000_000_100);
    assert!(overview.items.iter().all(|item| item.id != loose));
}

#[test]
fn empty_and_missing_projects_are_distinct_and_summary_uses_current_saved_metadata() {
    let (_tmp, engine) = setup(); let p = engine.create_project("Project", None).unwrap().id;
    engine.set_project_purpose(&p, "Original purpose").unwrap(); engine.set_project_purpose(&p, "Current brief").unwrap();
    engine.db().writer().execute("UPDATE projects SET status = 'paused' WHERE id = ?1", [&p]).unwrap();
    let overview = handle_get_project_overview(&engine, p).unwrap();
    assert!(overview.items.is_empty()); assert_eq!(overview.project.purpose, "Current brief"); assert_eq!(overview.project.status, "paused");
    assert!(handle_get_project_overview(&engine, "missing".into()).is_err());
    assert_eq!(engine.db().reader().unwrap().query_row("SELECT COUNT(*) FROM artifacts", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
}
