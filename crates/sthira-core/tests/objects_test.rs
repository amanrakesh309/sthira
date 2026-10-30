use std::sync::Arc;
use sthira_core::{
    db::objects_repo::{
        delete_object_type, get_object_instance, get_object_type, list_object_types,
        list_objects_by_type, upsert_object_instance, upsert_object_type,
    },
    models::objects::{ObjectInstance, ObjectType, PropertyDefinition, PropertyType},
    ArtifactType, Authorship, Clock, Engine,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

#[tokio::test]
async fn test_system_object_types_seeded_and_protected() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test_objects.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    let reader = engine.db().reader().unwrap();
    let types = list_object_types(&reader).expect("Failed to list object types");

    // Must have at least 4 default system types: idea, todo, decision, source
    assert!(types.len() >= 4);

    let idea = types.iter().find(|t| t.id == "idea").expect("idea missing");
    assert_eq!(idea.icon, "💡");
    assert_eq!(idea.color, "#F59E0B");
    assert!(idea.is_system);
    assert!(idea.schema.iter().any(|p| p.key == "status" && p.property_type == PropertyType::Select));

    let todo = types.iter().find(|t| t.id == "todo").expect("todo missing");
    assert_eq!(todo.icon, "☑️");
    assert_eq!(todo.color, "#10B981");
    assert!(todo.is_system);
    assert!(todo.schema.iter().any(|p| p.key == "completed" && p.property_type == PropertyType::Boolean));

    let decision = types.iter().find(|t| t.id == "decision").expect("decision missing");
    assert_eq!(decision.icon, "⚖️");
    assert_eq!(decision.color, "#6366F1");
    assert!(decision.is_system);
    assert!(decision.schema.iter().any(|p| p.key == "rationale"));

    let source = types.iter().find(|t| t.id == "source").expect("source missing");
    assert_eq!(source.icon, "🔗");
    assert_eq!(source.color, "#3B82F6");
    assert!(source.is_system);
    assert!(source.schema.iter().any(|p| p.key == "url" && p.property_type == PropertyType::Url));

    // Invariant: System types cannot be deleted
    drop(reader);
    let writer = engine.db().writer();
    let del_res = delete_object_type(&writer, "idea");
    assert!(del_res.is_err(), "System object types must be protected against deletion");
}

#[tokio::test]
async fn test_custom_object_type_creation_and_instance_lifecycle() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test_custom_objects.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    let now_ms = clock.now_ms();

    // 1. Create a Custom Object Type: "Book"
    let book_type = ObjectType {
        id: "book".to_string(),
        display_name: "Book".to_string(),
        icon: "📚".to_string(),
        color: "#8B5CF6".to_string(),
        schema: vec![
            PropertyDefinition {
                key: "author".to_string(),
                label: "Author".to_string(),
                property_type: PropertyType::Text,
                options: vec![],
                required: true,
                default_value: None,
            },
            PropertyDefinition {
                key: "rating".to_string(),
                label: "Rating (1-5)".to_string(),
                property_type: PropertyType::Number,
                options: vec![],
                required: false,
                default_value: Some(serde_json::json!(5)),
            },
            PropertyDefinition {
                key: "genre".to_string(),
                label: "Genre".to_string(),
                property_type: PropertyType::Select,
                options: vec!["Sci-Fi".to_string(), "Philosophy".to_string(), "Systems".to_string()],
                required: true,
                default_value: Some(serde_json::json!("Systems")),
            },
        ],
        is_system: false,
        created_at: now_ms,
        description: String::new(),
        default_template_md: String::new(),
    };

    {
        let writer = engine.db().writer();
        upsert_object_type(&writer, &book_type).expect("Failed to create custom object type");
    }

    // Verify type was persisted
    let reader = engine.db().reader().unwrap();
    let loaded_type = get_object_type(&reader, "book").unwrap().expect("Book type missing");
    assert_eq!(loaded_type.display_name, "Book");
    assert_eq!(loaded_type.schema.len(), 3);
    assert!(!loaded_type.is_system);

    // 2. Create an Artifact representing a book instance
    drop(reader);
    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Designing Data-Intensive Applications"),
            "# Designing Data-Intensive Applications\n\nNotes on reliability and consensus.",
            None,
        )
        .unwrap();

    // 3. Create an ObjectInstance with typed properties
    let book_props = serde_json::json!({
        "author": "Martin Kleppmann",
        "rating": 5,
        "genre": "Systems"
    });

    let instance = ObjectInstance {
        artifact_id: art.id.clone(),
        object_type_id: "book".to_string(),
        properties: book_props,
        updated_at: now_ms,
    };

    {
        let writer = engine.db().writer();
        upsert_object_instance(&writer, &instance).expect("Failed to upsert object instance");
    }

    // 4. List objects by type "book"
    let reader = engine.db().reader().unwrap();
    let books = list_objects_by_type(&reader, "book").expect("Failed to list objects by type");
    assert_eq!(books.len(), 1);
    assert_eq!(books[0].artifact_id, art.id);
    assert_eq!(books[0].title, "Designing Data-Intensive Applications");
    assert_eq!(books[0].properties["author"], "Martin Kleppmann");
    assert_eq!(books[0].properties["genre"], "Systems");

    // 5. Update properties
    drop(reader);
    let updated_props = serde_json::json!({
        "author": "Martin Kleppmann",
        "rating": 5,
        "genre": "Systems",
        "notes": "Essential reading for distributed state."
    });
    let updated_instance = ObjectInstance {
        artifact_id: art.id.clone(),
        object_type_id: "book".to_string(),
        properties: updated_props,
        updated_at: now_ms + 1000,
    };

    {
        let writer = engine.db().writer();
        upsert_object_instance(&writer, &updated_instance).expect("Failed to update object instance");
    }

    let reader = engine.db().reader().unwrap();
    let fetched = get_object_instance(&reader, &art.id).unwrap().expect("Instance missing");
    assert_eq!(fetched.properties["notes"], "Essential reading for distributed state.");
}

#[tokio::test]
async fn test_properties_json_enforces_valid_json_constraint() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("test_json_constraint.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Note for json test"),
            "Content",
            None,
        )
        .unwrap();

    let writer = engine.db().writer();

    // 1. Direct raw SQL insert with invalid JSON in properties_json must fail SQLite CHECK(json_valid(properties_json))
    let invalid_json_res = writer.execute(
        "INSERT INTO object_instances (artifact_id, object_type_id, properties_json, updated_at)
         VALUES (?1, 'idea', 'INVALID NOT JSON', 1000);",
        rusqlite::params![art.id],
    );
    assert!(invalid_json_res.is_err(), "Raw insert with invalid JSON must fail SQLite CHECK constraint");

    // 2. Direct raw SQL insert with invalid JSON in schema_json on object_types must fail
    let invalid_schema_res = writer.execute(
        "INSERT INTO object_types (id, display_name, icon, color, schema_json, is_system, created_at)
         VALUES ('bad_type', 'Bad', '❌', '#000', '{bad: json}', 0, 1000);",
        [],
    );
    assert!(invalid_schema_res.is_err(), "Raw insert with invalid schema_json must fail SQLite CHECK constraint");

    // 3. Valid JSON succeeds
    let valid_json_res = writer.execute(
        "INSERT INTO object_instances (artifact_id, object_type_id, properties_json, updated_at)
         VALUES (?1, 'idea', '{\"status\":\"spark\"}', 1000);",
        rusqlite::params![art.id],
    );
    assert!(valid_json_res.is_ok(), "Valid JSON insert must succeed");
}
