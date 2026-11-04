use std::sync::Arc;
use sthira_core::{ArtifactType, Authorship, Engine};
use sthira_desktop::*;
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::{tempdir, TempDir};

fn open(tmp: &TempDir) -> Engine {
    Engine::open(tmp.path().join("vault.db"), tmp.path(), Arc::new(FakeClock::new(1_700_000_000_000)), Arc::new(DeterministicFakeModel::new("test-model"))).unwrap()
}
fn artifact(id: &str) -> EntityRef { EntityRef { kind: EntityKind::Artifact, id: id.into() } }
fn project(id: &str) -> EntityRef { EntityRef { kind: EntityKind::Project, id: id.into() } }
fn page(engine: &Engine, title: &str) -> String {
    engine.create_artifact(ArtifactType::Note, Authorship::User, Some(title), "# Original body", None).unwrap().id
}

#[test]
fn references_cross_entity_kinds_have_backlinks_and_do_not_change_membership() {
    let tmp = tempdir().unwrap(); let engine = open(&tmp);
    let a = page(&engine, "Design notes");
    let p = engine.create_project("Sthira", None).unwrap().id;
    let q = engine.create_project("Research", None).unwrap().id;
    let object = handle_create_typed_object(&engine, "idea".into(), "One source of truth".into(), serde_json::json!({}), None).unwrap();
    let b = object.artifact_id;
    let preview = handle_get_workspace_entity(&engine, artifact(&b)).unwrap();
    assert_eq!(preview.object_type.unwrap().display_name, "Idea");
    assert!(preview.object_instance.is_some());
    for (source, target) in [(artifact(&a), artifact(&b)), (artifact(&a), project(&p)), (project(&p), artifact(&b)), (project(&p), project(&q))] {
        handle_create_workspace_connection(&engine, source, target).unwrap();
    }
    let backlinks = handle_get_workspace_connections(&engine, artifact(&b)).unwrap();
    assert_eq!(backlinks.len(), 2);
    assert!(backlinks.iter().all(|link| link.target.id == b && link.target.type_label == "Idea"));
    let graph = handle_get_knowledge_graph(&engine).unwrap();
    assert_eq!(graph.edges.iter().filter(|edge| edge.relationship == "references").count(), 4);
    assert_eq!(engine.get_artifact(&a).unwrap().content_md, "# Original body");
    let reader = engine.db().reader().unwrap();
    assert_eq!(reader.query_row("SELECT COUNT(*) FROM project_artifacts", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    assert_eq!(handle_search_connection_targets(&engine, "idea".into()).unwrap()[0].id, b);
    assert_eq!(handle_search_connection_targets(&engine, "%".into()).unwrap().len(), 0);
}

#[test]
fn references_survive_reopen_resolve_current_titles_and_unlink_without_deleting() {
    let tmp = tempdir().unwrap(); let engine = open(&tmp);
    let a = page(&engine, "A"); let b = page(&engine, "B");
    let id = handle_create_workspace_connection(&engine, artifact(&a), artifact(&b)).unwrap();
    assert_eq!(id, handle_create_workspace_connection(&engine, artifact(&a), artifact(&b)).unwrap());
    drop(engine);
    let engine = open(&tmp);
    engine.db().writer().execute("UPDATE artifacts SET title = 'Renamed' WHERE id = ?1", [&b]).unwrap();
    let links = handle_get_workspace_connections(&engine, artifact(&a)).unwrap();
    assert_eq!(links.len(), 1); assert_eq!(links[0].target.title, "Renamed");
    handle_delete_workspace_connection(&engine, id).unwrap();
    assert!(handle_get_workspace_connections(&engine, artifact(&b)).unwrap().is_empty());
    assert!(engine.get_artifact(&a).is_ok()); assert!(engine.get_artifact(&b).is_ok());
}

#[test]
fn invalid_deleted_and_self_targets_are_rejected_atomically() {
    let tmp = tempdir().unwrap(); let engine = open(&tmp); let a = page(&engine, "A"); let b = page(&engine, "B");
    assert!(handle_create_workspace_connection(&engine, artifact(&a), artifact(&a)).is_err());
    assert!(handle_create_workspace_connection(&engine, artifact(&a), project("missing")).is_err());
    engine.db().writer().execute("UPDATE artifacts SET deleted_at = 1 WHERE id = ?1", [&b]).unwrap();
    assert!(handle_create_workspace_connection(&engine, artifact(&a), artifact(&b)).is_err());
    assert!(handle_get_workspace_entity(&engine, artifact(&b)).is_err());
    assert!(handle_get_workspace_connections(&engine, artifact(&a)).unwrap().is_empty());
}

#[test]
fn detach_preserves_references_and_permanent_deletion_cleans_them() {
    let tmp = tempdir().unwrap(); let engine = open(&tmp);
    let a = page(&engine, "Shared"); let b = page(&engine, "Source");
    let p = engine.create_project("P", None).unwrap().id; let q = engine.create_project("Q", None).unwrap().id;
    engine.attach_artifact(&p, &a, "user_explicit", None).unwrap(); engine.attach_artifact(&q, &a, "user_explicit", None).unwrap();
    handle_create_workspace_connection(&engine, artifact(&a), artifact(&b)).unwrap();
    handle_create_workspace_connection(&engine, project(&p), artifact(&b)).unwrap();
    handle_delete_artifact(&engine, a.clone(), Some(p.clone())).unwrap();
    assert_eq!(handle_get_workspace_connections(&engine, artifact(&a)).unwrap().len(), 1);
    handle_delete_project(&engine, p, false).unwrap();
    assert_eq!(handle_get_workspace_connections(&engine, artifact(&b)).unwrap().len(), 1);
    handle_delete_artifact(&engine, a, None).unwrap();
    assert!(handle_get_workspace_connections(&engine, artifact(&b)).unwrap().is_empty());
    let reader = engine.db().reader().unwrap();
    assert_eq!(reader.query_row("SELECT COUNT(*) FROM workspace_links", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
}

#[test]
fn project_previews_have_live_members_and_cascade_deletion_cleans_references() {
    let tmp = tempdir().unwrap(); let engine = open(&tmp);
    let p = engine.create_project("P", None).unwrap().id; engine.set_project_purpose(&p, "Preserve context").unwrap();
    let a = page(&engine, "Exclusive"); let b = page(&engine, "Other");
    engine.attach_artifact(&p, &a, "user_explicit", None).unwrap();
    handle_create_workspace_connection(&engine, artifact(&b), artifact(&a)).unwrap();
    let preview = handle_get_workspace_entity(&engine, project(&p)).unwrap();
    assert_eq!(preview.description, "Preserve context"); assert_eq!(preview.members.len(), 1);
    handle_delete_project(&engine, p, true).unwrap();
    assert!(handle_get_workspace_connections(&engine, artifact(&b)).unwrap().is_empty());
    assert_eq!(engine.db().reader().unwrap().query_row("SELECT COUNT(*) FROM workspace_links", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
}

#[test]
fn migration_from_v11_is_additive_and_idempotent() {
    let tmp = tempdir().unwrap(); let engine = open(&tmp); let a = page(&engine, "Keep me");
    engine.db().writer().execute_batch("DROP TABLE workspace_links; PRAGMA user_version = 11;").unwrap();
    drop(engine);
    let engine = open(&tmp);
    assert_eq!(engine.get_artifact(&a).unwrap().title.as_deref(), Some("Keep me"));
    assert_eq!(engine.db().reader().unwrap().query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).unwrap(), 12);
    drop(engine); let engine = open(&tmp);
    assert!(handle_get_workspace_connections(&engine, artifact(&a)).unwrap().is_empty());
}

#[test]
fn derived_items_automatically_expose_source_and_backlinks_without_manual_rows() {
    let tmp = tempdir().unwrap(); let engine = open(&tmp);
    let source = page(&engine, "Research");
    let child = engine.create_derived_artifact(
        ArtifactType::Note, Authorship::AiGenerated, Some("Execution plan"), "# Plan", None,
        Some(&source), Some("plan_from_research"),
    ).unwrap();
    let outgoing = handle_get_workspace_connections(&engine, artifact(&child.id)).unwrap();
    assert_eq!(outgoing.len(), 1);
    assert_eq!(outgoing[0].relationship, "derived_from");
    assert_eq!(outgoing[0].source.id, child.id);
    assert_eq!(outgoing[0].target.id, source);
    let incoming = handle_get_workspace_connections(&engine, artifact(&source)).unwrap();
    assert_eq!(incoming[0].id, outgoing[0].id);
    assert!(handle_delete_workspace_connection(&engine, outgoing[0].id.clone()).is_err());
    assert_eq!(engine.db().reader().unwrap().query_row("SELECT COUNT(*) FROM workspace_links", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    assert!(handle_get_knowledge_graph(&engine).unwrap().edges.iter().any(|edge| edge.source == child.id && edge.target == source && edge.relationship == "derived_from"));
}
