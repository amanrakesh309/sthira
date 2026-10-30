use std::sync::Arc;
use rusqlite::params;
use serde_json::json;
use sthira_core::{
    models::objects::ObjectType,
    Engine,
};
use sthira_desktop::{
    handle_create_object_type, handle_create_page, handle_create_project,
    handle_create_typed_object, handle_delete_artifact, handle_delete_project,
    handle_get_knowledge_graph, handle_inspect_project_deletion,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

fn setup_test_engine() -> (tempfile::TempDir, Arc<Engine>) {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test_deletion_graph.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    (tmp, engine)
}

#[tokio::test]
async fn test_inspect_and_cascade_project_deletion() {
    let (_tmp, engine) = setup_test_engine();

    // 1. Create Project 1 and Project 2
    let p1 = handle_create_project(&engine, "Project Alpha".into(), Some("Primary project".into())).unwrap();
    let p2 = handle_create_project(&engine, "Project Beta".into(), Some("Secondary project".into())).unwrap();

    // 2. Create Artifact A (Shared) and attach to both Project 1 and Project 2
    let art_a = handle_create_page(&engine, Some(p1.id.clone()), Some("Shared Protocol Spec".into())).unwrap();
    engine.attach_artifact(&p2.id, &art_a.id, "user_explicit", None).unwrap();

    // 3. Create Artifact B (Exclusive) and attach to Project 1 only
    let art_b = handle_create_page(&engine, Some(p1.id.clone()), Some("Alpha Internal Roadmap".into())).unwrap();

    // 4. Inspect deletion impact on Project 1
    let impact = handle_inspect_project_deletion(&engine, p1.id.clone())
        .expect("handle_inspect_project_deletion should succeed");

    assert_eq!(impact.project_id, p1.id);
    assert_eq!(impact.project_name, "Project Alpha");
    assert_eq!(impact.exclusive_artifacts.len(), 1, "Should have 1 exclusive artifact");
    assert_eq!(impact.exclusive_artifacts[0].id, art_b.id);
    assert_eq!(impact.shared_artifacts.len(), 1, "Should have 1 shared artifact");
    assert_eq!(impact.shared_artifacts[0].id, art_a.id);

    // 5. Delete Project 1 with cascade_exclusive = true
    handle_delete_project(&engine, p1.id.clone(), true)
        .expect("handle_delete_project with cascade should succeed");

    // 6. Verify Project 1 is deleted from SQLite
    let reader = engine.db().reader().unwrap();
    let p1_count: i64 = reader
        .query_row("SELECT COUNT(*) FROM projects WHERE id = ?1;", params![p1.id], |r| r.get(0))
        .unwrap();
    assert_eq!(p1_count, 0, "Project 1 must be deleted");

    // 7. Verify exclusive Artifact B is completely deleted from artifacts table
    let art_b_count: i64 = reader
        .query_row("SELECT COUNT(*) FROM artifacts WHERE id = ?1;", params![art_b.id], |r| r.get(0))
        .unwrap();
    assert_eq!(art_b_count, 0, "Exclusive Artifact B must be deleted from database");

    // 8. Verify shared Artifact A is still present in database and attached to Project 2
    let art_a_count: i64 = reader
        .query_row("SELECT COUNT(*) FROM artifacts WHERE id = ?1;", params![art_a.id], |r| r.get(0))
        .unwrap();
    assert_eq!(art_a_count, 1, "Shared Artifact A must remain in database");

    let p2_attachments: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM project_artifacts WHERE project_id = ?1 AND artifact_id = ?2 AND detached_at IS NULL;",
            params![p2.id, art_a.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(p2_attachments, 1, "Shared Artifact A must remain attached to Project 2");
}

#[tokio::test]
async fn test_safe_default_project_deletion_preserves_exclusive_artifacts_as_loose() {
    let (_tmp, engine) = setup_test_engine();

    let p = handle_create_project(&engine, "Ephemeral Research".into(), None).unwrap();
    let art = handle_create_page(&engine, Some(p.id.clone()), Some("Valuable Research Findings".into())).unwrap();

    // Delete with cascade_exclusive = false (safe default)
    handle_delete_project(&engine, p.id.clone(), false).unwrap();

    // Verify project is deleted
    let reader = engine.db().reader().unwrap();
    let p_count: i64 = reader
        .query_row("SELECT COUNT(*) FROM projects WHERE id = ?1;", params![p.id], |r| r.get(0))
        .unwrap();
    assert_eq!(p_count, 0);

    // Verify artifact STILL exists and has zero project attachments (unassociated loose note)
    let art_count: i64 = reader
        .query_row("SELECT COUNT(*) FROM artifacts WHERE id = ?1;", params![art.id], |r| r.get(0))
        .unwrap();
    assert_eq!(art_count, 1);

    let attached_count: i64 = reader
        .query_row("SELECT COUNT(*) FROM project_artifacts WHERE artifact_id = ?1;", params![art.id], |r| r.get(0))
        .unwrap();
    assert_eq!(attached_count, 0, "Artifact should have 0 project attachments, becoming loose knowledge");
}

#[tokio::test]
async fn test_delete_artifact_detach_vs_permanent() {
    let (_tmp, engine) = setup_test_engine();

    let p1 = handle_create_project(&engine, "P1".into(), None).unwrap();
    let p2 = handle_create_project(&engine, "P2".into(), None).unwrap();

    let shared_art = handle_create_page(&engine, Some(p1.id.clone()), Some("Shared Document".into())).unwrap();
    engine.attach_artifact(&p2.id, &shared_art.id, "user_explicit", None).unwrap();

    // Detach from P1
    handle_delete_artifact(&engine, shared_art.id.clone(), Some(p1.id.clone())).unwrap();

    let reader = engine.db().reader().unwrap();
    let p1_links: i64 = reader
        .query_row("SELECT COUNT(*) FROM project_artifacts WHERE project_id = ?1 AND artifact_id = ?2;", params![p1.id, shared_art.id], |r| r.get(0))
        .unwrap();
    assert_eq!(p1_links, 0, "Detached from P1");

    let p2_links: i64 = reader
        .query_row("SELECT COUNT(*) FROM project_artifacts WHERE project_id = ?1 AND artifact_id = ?2;", params![p2.id, shared_art.id], |r| r.get(0))
        .unwrap();
    assert_eq!(p2_links, 1, "Still attached to P2");

    // Delete permanently (from_project_id = None or from P2 where it's the last)
    handle_delete_artifact(&engine, shared_art.id.clone(), Some(p2.id.clone())).unwrap();

    let final_art_count: i64 = reader
        .query_row("SELECT COUNT(*) FROM artifacts WHERE id = ?1;", params![shared_art.id], |r| r.get(0))
        .unwrap();
    assert_eq!(final_art_count, 0, "Artifact permanently deleted");
}

#[tokio::test]
async fn test_knowledge_graph_export_nodes_and_edges() {
    let (_tmp, engine) = setup_test_engine();

    // 1. Create project
    let p = handle_create_project(&engine, "Knowledge Graph Testing".into(), None).unwrap();

    // 2. Create pages
    let page1 = handle_create_page(&engine, Some(p.id.clone()), Some("Node 1".into())).unwrap();
    let page2 = handle_create_page(&engine, Some(p.id.clone()), Some("Node 2".into())).unwrap();

    // 3. Create semantic link between page1 and page2
    {
        let writer = engine.db().writer();
        writer.execute(
            "INSERT INTO artifact_links (source_artifact_id, target_artifact_id, relationship_type, origin, created_at)
             VALUES (?1, ?2, 'supports', 'user_recorded', ?3);",
            params![page1.id, page2.id, 1_700_000_000_000i64],
        ).unwrap();
    }

    // 4. Create custom typed object
    let custom_type = ObjectType {
        id: "idea".into(),
        display_name: "Idea".into(),
        icon: "💡".into(),
        color: "#F59E0B".into(),
        schema: vec![],
        is_system: true,
        created_at: 1_700_000_000_000,
        description: String::new(),
        default_template_md: String::new(),
    };
    let _ = handle_create_object_type(&engine, custom_type);
    let obj = handle_create_typed_object(&engine, "idea".into(), "Autonomous AI Testing".into(), json!({}), Some(p.id.clone())).unwrap();

    // 5. Query knowledge graph
    let graph = handle_get_knowledge_graph(&engine).expect("handle_get_knowledge_graph should succeed");

    // Node checks
    let proj_node = graph.nodes.iter().find(|n| n.id == p.id).expect("Project node must exist");
    assert_eq!(proj_node.kind, "project");
    assert_eq!(proj_node.label, "Knowledge Graph Testing");

    let art_node = graph.nodes.iter().find(|n| n.id == page1.id).expect("Artifact node must exist");
    assert_eq!(art_node.kind, "artifact");
    assert_eq!(art_node.label, "Node 1");

    let obj_node = graph.nodes.iter().find(|n| n.id == obj.artifact_id).expect("Object node must exist");
    assert_eq!(obj_node.kind, "object");
    assert_eq!(obj_node.label, "Autonomous AI Testing");
    assert_eq!(obj_node.object_type.as_deref(), Some("idea"));

    // Edge checks
    let belongs_edges: Vec<_> = graph.edges.iter().filter(|e| e.relationship == "belongs_to" && e.target == p.id).collect();
    assert_eq!(belongs_edges.len(), 3, "page1, page2, and obj belong to project");

    let semantic_edge = graph.edges.iter().find(|e| e.relationship == "supports").expect("Supports edge must exist");
    assert_eq!(semantic_edge.source, page1.id);
    assert_eq!(semantic_edge.target, page2.id);
}
