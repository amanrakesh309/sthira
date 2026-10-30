use std::sync::Arc;
use sthira_core::{
    db::objects_repo::{get_object_instance, get_object_type, list_templates_for_type},
    models::objects::{ObjectTemplate, ObjectType, PropertyDefinition, PropertyType},
    Engine,
};
use sthira_desktop::{
    handle_create_object_type, handle_delete_object_type, handle_get_type_templates,
    handle_instantiate_object_from_template, handle_save_object_template, handle_update_object_type,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

fn setup_test_engine() -> (tempfile::TempDir, Arc<Engine>) {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("object_templates_test.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(&db_path, &vault_dir, Arc::clone(&clock) as _, Arc::clone(&model) as _).unwrap(),
    );

    (tmp, engine)
}

fn meeting_type() -> ObjectType {
    ObjectType {
        id: "meeting".to_string(),
        display_name: "Meeting Note".to_string(),
        icon: "🗓️".to_string(),
        color: "#14B8A6".to_string(),
        schema: vec![
            PropertyDefinition {
                key: "status".to_string(),
                label: "Status".to_string(),
                property_type: PropertyType::Select,
                options: vec!["scheduled".to_string(), "held".to_string()],
                required: true,
                default_value: Some(serde_json::json!("scheduled")),
            },
            PropertyDefinition {
                key: "attendees".to_string(),
                label: "Attendees".to_string(),
                property_type: PropertyType::Text,
                options: vec![],
                required: false,
                default_value: Some(serde_json::json!("")),
            },
        ],
        is_system: false,
        created_at: 1_700_000_000_000,
        description: "A recurring meeting note.".to_string(),
        default_template_md: "## Agenda\n\n## Notes\n\n## Action Items\n- [ ] ".to_string(),
    }
}

// ---------------------------------------------------------------------------
// 1. Custom type with a default template -> instantiate -> artifact is hydrated
//    with `content_md` (template body) and `properties_json` (schema defaults).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_instantiate_object_from_template_hydrates_content_and_properties() {
    let (_tmp, engine) = setup_test_engine();

    let created_id =
        handle_create_object_type(&engine, meeting_type()).expect("handle_create_object_type failed");
    assert_eq!(created_id, "meeting");

    // The type's description + default template must round-trip through SQLite.
    {
        let reader = engine.db().reader().unwrap();
        let stored = get_object_type(&reader, "meeting").unwrap().unwrap();
        assert_eq!(stored.display_name, "Meeting Note");
        assert_eq!(stored.description, "A recurring meeting note.");
        assert!(stored.default_template_md.contains("## Agenda"));
        assert!(stored.default_template_md.contains("## Action Items"));
    }

    // Instantiate without an explicit template id -> the type default is used.
    let artifact_id = handle_instantiate_object_from_template(
        &engine,
        "meeting".to_string(),
        None,
        "Sprint Sync".to_string(),
        None,
    )
    .expect("handle_instantiate_object_from_template failed");
    assert!(!artifact_id.is_empty());

    // content_md is pre-populated with the title + the default Markdown skeleton.
    let artifact = engine.get_artifact(&artifact_id).expect("artifact must exist");
    assert_eq!(artifact.title.as_deref(), Some("Sprint Sync"));
    assert!(artifact.content_md.starts_with("# Sprint Sync"));
    assert!(artifact.content_md.contains("## Agenda"));
    assert!(artifact.content_md.contains("## Notes"));
    assert!(artifact.content_md.contains("## Action Items"));

    // properties_json is pre-populated with schema default values.
    {
        let reader = engine.db().reader().unwrap();
        let instance = get_object_instance(&reader, &artifact_id)
            .expect("get_object_instance failed")
            .expect("object instance must exist");
        assert_eq!(instance.object_type_id, "meeting");
        assert_eq!(
            instance.properties.get("status").and_then(|v| v.as_str()),
            Some("scheduled")
        );
        assert_eq!(
            instance.properties.get("attendees").and_then(|v| v.as_str()),
            Some("")
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Named templates: an explicit template id overrides the type default and
//    merges its `properties_preset` on top of schema defaults.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_named_template_overrides_default_body_and_property_preset() {
    let (_tmp, engine) = setup_test_engine();

    handle_create_object_type(&engine, meeting_type()).unwrap();

    let template_id = handle_save_object_template(
        &engine,
        ObjectTemplate {
            id: String::new(),
            object_type_id: "meeting".to_string(),
            name: "Retrospective".to_string(),
            icon: "🔁".to_string(),
            is_default: true,
            properties_preset: serde_json::json!({ "status": "held", "attendees": "Team" }),
            body_template_md: "## What went well\n\n## What to improve".to_string(),
            created_at: 0,
        },
    )
    .expect("handle_save_object_template failed");
    assert!(!template_id.is_empty());

    let listed = handle_get_type_templates(&engine, "meeting".to_string())
        .expect("handle_get_type_templates failed");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "Retrospective");
    assert!(listed[0].is_default);
    assert_eq!(listed[0].icon, "🔁");

    let artifact_id = handle_instantiate_object_from_template(
        &engine,
        "meeting".to_string(),
        Some(template_id.clone()),
        "Sprint 12 Retro".to_string(),
        None,
    )
    .expect("instantiate from named template failed");

    let artifact = engine.get_artifact(&artifact_id).unwrap();
    assert!(artifact.content_md.contains("## What went well"));
    assert!(artifact.content_md.contains("## What to improve"));
    assert!(!artifact.content_md.contains("## Agenda"));

    {
        let reader = engine.db().reader().unwrap();
        let instance = get_object_instance(&reader, &artifact_id).unwrap().unwrap();
        // Preset overrides the schema default for `status` and fills `attendees`.
        assert_eq!(
            instance.properties.get("status").and_then(|v| v.as_str()),
            Some("held")
        );
        assert_eq!(
            instance.properties.get("attendees").and_then(|v| v.as_str()),
            Some("Team")
        );
    }

    // A `blank` sentinel produces a bare title-only skeleton.
    let blank_id = handle_instantiate_object_from_template(
        &engine,
        "meeting".to_string(),
        Some("blank".to_string()),
        "Ad-hoc Chat".to_string(),
        None,
    )
    .unwrap();
    let blank_artifact = engine.get_artifact(&blank_id).unwrap();
    assert_eq!(blank_artifact.content_md.trim(), "# Ad-hoc Chat");
}

// ---------------------------------------------------------------------------
// 3. Editing an existing schema (adding/updating properties) preserves the
//    property values already stored on existing instances.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_editing_schema_preserves_existing_instance_values() {
    let (_tmp, engine) = setup_test_engine();

    handle_create_object_type(&engine, meeting_type()).unwrap();

    let artifact_id = handle_instantiate_object_from_template(
        &engine,
        "meeting".to_string(),
        None,
        "Weekly Standup".to_string(),
        None,
    )
    .unwrap();

    // Mutate the instance: status -> held, attendees -> "Ada, Grace".
    {
        let reader = engine.db().reader().unwrap();
        let mut instance = get_object_instance(&reader, &artifact_id).unwrap().unwrap();
        drop(reader);
        instance.properties = serde_json::json!({ "status": "held", "attendees": "Ada, Grace" });
        let writer = engine.db().writer();
        sthira_core::db::objects_repo::upsert_object_instance(&writer, &instance).unwrap();
    }

    // Edit the schema: rename the type, add a `deadline` property, and change the
    // default template body.
    let mut edited = meeting_type();
    edited.display_name = "Meeting Notes (v2)".to_string();
    edited.color = "#6366F1".to_string();
    edited.schema.push(PropertyDefinition {
        key: "deadline".to_string(),
        label: "Deadline".to_string(),
        property_type: PropertyType::Date,
        options: vec![],
        required: false,
        default_value: None,
    });
    edited.default_template_md = "## Revised Agenda".to_string();

    handle_update_object_type(&engine, edited).expect("handle_update_object_type failed");

    // 1. Type identity + metadata updated in place.
    {
        let reader = engine.db().reader().unwrap();
        let stored = get_object_type(&reader, "meeting").unwrap().unwrap();
        assert_eq!(stored.display_name, "Meeting Notes (v2)");
        assert_eq!(stored.color, "#6366F1");
        assert_eq!(stored.schema.len(), 3);
        assert!(stored.schema.iter().any(|p| p.key == "deadline"));
        assert_eq!(stored.default_template_md, "## Revised Agenda");
        assert!(!stored.is_system);
    }

    // 2. The pre-existing instance keeps its stored values untouched.
    {
        let reader = engine.db().reader().unwrap();
        let instance = get_object_instance(&reader, &artifact_id).unwrap().unwrap();
        assert_eq!(
            instance.properties.get("status").and_then(|v| v.as_str()),
            Some("held")
        );
        assert_eq!(
            instance.properties.get("attendees").and_then(|v| v.as_str()),
            Some("Ada, Grace")
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Built-in types ship default template bodies; system types are protected
//    from deletion while custom types can be removed.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_builtin_default_templates_and_type_deletion_protection() {
    let (_tmp, engine) = setup_test_engine();

    {
        let reader = engine.db().reader().unwrap();

        let idea = get_object_type(&reader, "idea").unwrap().unwrap();
        assert_eq!(
            idea.default_template_md,
            "## The Spark\n\n## Potential Impact\n\n## Next Experiments"
        );

        let todo = get_object_type(&reader, "todo").unwrap().unwrap();
        assert_eq!(
            todo.default_template_md,
            "## Acceptance Criteria\n- [ ] \n\n## Context"
        );

        let decision = get_object_type(&reader, "decision").unwrap().unwrap();
        assert_eq!(
            decision.default_template_md,
            "## Context & Problem\n\n## Considered Alternatives\n\n> **Decision:** \n\n## Consequences"
        );

        // Every built-in type ships exactly one seeded default template.
        for type_id in ["idea", "todo", "decision", "source"] {
            let templates = list_templates_for_type(&reader, type_id).unwrap();
            assert_eq!(templates.len(), 1, "expected one seeded template for {type_id}");
            assert!(templates[0].is_default);
        }
    }

    // System types are protected.
    let err = handle_delete_object_type(&engine, "idea".to_string());
    assert!(err.is_err(), "built-in system type must not be deletable");

    // Custom types can be deleted.
    handle_create_object_type(&engine, meeting_type()).unwrap();
    handle_delete_object_type(&engine, "meeting".to_string())
        .expect("custom type deletion should succeed");
    {
        let reader = engine.db().reader().unwrap();
        assert!(get_object_type(&reader, "meeting").unwrap().is_none());
    }
}

