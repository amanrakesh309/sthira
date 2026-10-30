use crate::{
    models::objects::{
        ObjectInstance, ObjectItem, ObjectTemplate, ObjectType, PropertyDefinition, PropertyType,
    },
    CoreError,
};
use rusqlite::{params, Connection};
use std::collections::HashMap;

const OBJECT_TYPE_COLUMNS: &str =
    "id, display_name, icon, color, schema_json, is_system, created_at, description, default_template_md";

fn row_to_object_type(r: &rusqlite::Row<'_>) -> Result<ObjectType, rusqlite::Error> {
    let schema_json: String = r.get(4)?;
    let schema: Vec<PropertyDefinition> = serde_json::from_str(&schema_json).unwrap_or_default();

    Ok(ObjectType {
        id: r.get(0)?,
        display_name: r.get(1)?,
        icon: r.get(2)?,
        color: r.get(3)?,
        schema,
        is_system: r.get::<_, i32>(5)? != 0,
        created_at: r.get(6)?,
        description: r.get(7)?,
        default_template_md: r.get(8)?,
    })
}

pub fn list_object_types(conn: &Connection) -> Result<Vec<ObjectType>, CoreError> {
    let sql = format!(
        "SELECT {OBJECT_TYPE_COLUMNS}
         FROM object_types
         ORDER BY is_system DESC, display_name ASC;"
    );
    let mut stmt = conn.prepare(&sql)?;

    let rows = stmt.query_map([], row_to_object_type)?;

    let mut list = Vec::new();
    for row in rows {
        list.push(row?);
    }
    Ok(list)
}

pub fn get_object_type(conn: &Connection, id: &str) -> Result<Option<ObjectType>, CoreError> {
    let sql = format!("SELECT {OBJECT_TYPE_COLUMNS} FROM object_types WHERE id = ?1;");
    let mut stmt = conn.prepare(&sql)?;

    let mut rows = stmt.query(params![id])?;
    if let Some(r) = rows.next()? {
        Ok(Some(row_to_object_type(r)?))
    } else {
        Ok(None)
    }
}

pub fn upsert_object_type(conn: &Connection, obj_type: &ObjectType) -> Result<(), CoreError> {
    let schema_json = serde_json::to_string(&obj_type.schema)?;

    conn.execute(
        "INSERT INTO object_types
            (id, display_name, icon, color, schema_json, is_system, created_at, description, default_template_md)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
            display_name = excluded.display_name,
            icon = excluded.icon,
            color = excluded.color,
            schema_json = excluded.schema_json,
            description = excluded.description,
            default_template_md = excluded.default_template_md;",
        params![
            obj_type.id,
            obj_type.display_name,
            obj_type.icon,
            obj_type.color,
            schema_json,
            if obj_type.is_system { 1 } else { 0 },
            obj_type.created_at,
            obj_type.description,
            obj_type.default_template_md,
        ],
    )?;

    Ok(())
}

/// Phase 16: Targeted schema edit that preserves type identity, `is_system`, and
/// `created_at` while allowing display metadata, the property schema, and the default
/// Markdown template body to evolve.
///
/// Instance property values are deliberately untouched: `object_instances.properties_json`
/// is a JSON document keyed by property key, so existing values survive schema edits as
/// long as property keys are preserved (the primary invariant verified by
/// `test_editing_schema_preserves_existing_instance_values`).
pub fn update_object_type_schema(
    conn: &Connection,
    type_id: &str,
    display_name: &str,
    icon: &str,
    color: &str,
    properties: &[PropertyDefinition],
    default_template_md: &str,
) -> Result<(), CoreError> {
    let schema_json = serde_json::to_string(properties)?;

    let changed = conn.execute(
        "UPDATE object_types
         SET display_name = ?2,
             icon = ?3,
             color = ?4,
             schema_json = ?5,
             default_template_md = ?6
         WHERE id = ?1;",
        params![
            type_id,
            display_name,
            icon,
            color,
            schema_json,
            default_template_md,
        ],
    )?;

    if changed == 0 {
        return Err(CoreError::NotFound(format!(
            "Object type '{}' not found",
            type_id
        )));
    }

    Ok(())
}

pub fn delete_object_type(conn: &Connection, id: &str) -> Result<(), CoreError> {
    let is_system: Option<i32> = conn
        .query_row(
            "SELECT is_system FROM object_types WHERE id = ?1;",
            params![id],
            |r| r.get(0),
        )
        .ok();

    if let Some(1) = is_system {
        return Err(CoreError::InvariantViolation(format!(
            "Cannot delete built-in system object type '{}'",
            id
        )));
    }

    let changed = conn.execute("DELETE FROM object_types WHERE id = ?1;", params![id])?;
    if changed == 0 {
        return Err(CoreError::NotFound(format!("Object type '{}' not found", id)));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Phase 16: Object Templates (named, reusable presets per object type)
// ---------------------------------------------------------------------------

const OBJECT_TEMPLATE_COLUMNS: &str =
    "id, object_type_id, name, icon, is_default, properties_preset_json, body_template_md, created_at";

fn row_to_object_template(r: &rusqlite::Row<'_>) -> Result<ObjectTemplate, rusqlite::Error> {
    let preset_str: String = r.get(5)?;
    let properties_preset: serde_json::Value =
        serde_json::from_str(&preset_str).unwrap_or_else(|_| serde_json::json!({}));

    Ok(ObjectTemplate {
        id: r.get(0)?,
        object_type_id: r.get(1)?,
        name: r.get(2)?,
        icon: r.get(3)?,
        is_default: r.get::<_, i32>(4)? != 0,
        properties_preset,
        body_template_md: r.get(6)?,
        created_at: r.get(7)?,
    })
}

pub fn list_templates_for_type(
    conn: &Connection,
    type_id: &str,
) -> Result<Vec<ObjectTemplate>, CoreError> {
    let sql = format!(
        "SELECT {OBJECT_TEMPLATE_COLUMNS}
         FROM object_templates
         WHERE object_type_id = ?1
         ORDER BY is_default DESC, created_at ASC;"
    );
    let mut stmt = conn.prepare(&sql)?;

    let rows = stmt.query_map(params![type_id], row_to_object_template)?;

    let mut list = Vec::new();
    for row in rows {
        list.push(row?);
    }
    Ok(list)
}

pub fn get_object_template(
    conn: &Connection,
    template_id: &str,
) -> Result<Option<ObjectTemplate>, CoreError> {
    let sql = format!("SELECT {OBJECT_TEMPLATE_COLUMNS} FROM object_templates WHERE id = ?1;");
    let mut stmt = conn.prepare(&sql)?;

    let mut rows = stmt.query(params![template_id])?;
    if let Some(r) = rows.next()? {
        Ok(Some(row_to_object_template(r)?))
    } else {
        Ok(None)
    }
}

pub fn save_object_template(
    conn: &Connection,
    template: &ObjectTemplate,
) -> Result<(), CoreError> {
    let preset_json = serde_json::to_string(&template.properties_preset)?;

    // Invariant: only one template per object type may be flagged as the default.
    if template.is_default {
        conn.execute(
            "UPDATE object_templates SET is_default = 0 WHERE object_type_id = ?1;",
            params![template.object_type_id],
        )?;
    }

    conn.execute(
        "INSERT INTO object_templates
            (id, object_type_id, name, icon, is_default, properties_preset_json, body_template_md, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET
            object_type_id = excluded.object_type_id,
            name = excluded.name,
            icon = excluded.icon,
            is_default = excluded.is_default,
            properties_preset_json = excluded.properties_preset_json,
            body_template_md = excluded.body_template_md;",
        params![
            template.id,
            template.object_type_id,
            template.name,
            template.icon,
            if template.is_default { 1 } else { 0 },
            preset_json,
            template.body_template_md,
            template.created_at,
        ],
    )?;

    Ok(())
}

pub fn delete_object_template(conn: &Connection, template_id: &str) -> Result<(), CoreError> {
    let changed = conn.execute(
        "DELETE FROM object_templates WHERE id = ?1;",
        params![template_id],
    )?;
    if changed == 0 {
        return Err(CoreError::NotFound(format!(
            "Object template '{}' not found",
            template_id
        )));
    }
    Ok(())
}

pub fn list_objects_by_type(
    conn: &Connection,
    type_id: &str,
) -> Result<Vec<ObjectItem>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT 
            oi.artifact_id,
            COALESCE(a.title, 'Untitled Note') AS title,
            a.content_md,
            oi.object_type_id,
            oi.properties_json,
            pa.project_id,
            oi.updated_at
         FROM object_instances oi
         JOIN artifacts a ON oi.artifact_id = a.id
         LEFT JOIN project_artifacts pa ON a.id = pa.artifact_id AND pa.detached_at IS NULL
         WHERE oi.object_type_id = ?1
           AND a.deleted_at IS NULL
         ORDER BY oi.updated_at DESC;",
    )?;

    let rows = stmt.query_map(params![type_id], |r| {
        let artifact_id: String = r.get(0)?;
        let title: String = r.get(1)?;
        let content_md: String = r.get(2)?;
        let object_type_id: String = r.get(3)?;
        let props_str: String = r.get(4)?;
        let project_id: Option<String> = r.get(5)?;
        let updated_at: i64 = r.get(6)?;

        let properties: serde_json::Value =
            serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));

        Ok(ObjectItem {
            artifact_id,
            title,
            content_md,
            object_type_id,
            properties,
            project_id,
            updated_at,
        })
    })?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

pub fn get_object_instance(
    conn: &Connection,
    artifact_id: &str,
) -> Result<Option<ObjectInstance>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT artifact_id, object_type_id, properties_json, updated_at
         FROM object_instances
         WHERE artifact_id = ?1;",
    )?;

    let mut rows = stmt.query(params![artifact_id])?;
    if let Some(r) = rows.next()? {
        let artifact_id: String = r.get(0)?;
        let object_type_id: String = r.get(1)?;
        let props_str: String = r.get(2)?;
        let updated_at: i64 = r.get(3)?;

        let properties: serde_json::Value =
            serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));

        Ok(Some(ObjectInstance {
            artifact_id,
            object_type_id,
            properties,
            updated_at,
        }))
    } else {
        Ok(None)
    }
}

pub fn upsert_object_instance(
    conn: &Connection,
    instance: &ObjectInstance,
) -> Result<(), CoreError> {
    let props_json = serde_json::to_string(&instance.properties)?;

    conn.execute(
        "INSERT INTO object_instances (artifact_id, object_type_id, properties_json, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(artifact_id) DO UPDATE SET
            object_type_id = excluded.object_type_id,
            properties_json = excluded.properties_json,
            updated_at = excluded.updated_at;",
        params![
            instance.artifact_id,
            instance.object_type_id,
            props_json,
            instance.updated_at,
        ],
    )?;

    Ok(())
}

pub fn delete_object_instance(conn: &Connection, artifact_id: &str) -> Result<(), CoreError> {
    let changed = conn.execute(
        "DELETE FROM object_instances WHERE artifact_id = ?1;",
        params![artifact_id],
    )?;
    if changed == 0 {
        return Err(CoreError::NotFound(format!(
            "Object instance for artifact '{}' not found",
            artifact_id
        )));
    }
    Ok(())
}

pub fn count_objects_by_type(
    conn: &Connection,
) -> Result<HashMap<String, usize>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT oi.object_type_id, COUNT(*)
         FROM object_instances oi
         JOIN artifacts a ON oi.artifact_id = a.id
         WHERE a.deleted_at IS NULL
         GROUP BY oi.object_type_id;",
    )?;

    let rows = stmt.query_map([], |r| {
        let type_id: String = r.get(0)?;
        let count: i64 = r.get(1)?;
        Ok((type_id, count as usize))
    })?;

    let mut map = HashMap::new();
    for row in rows {
        let (k, v) = row?;
        map.insert(k, v);
    }
    Ok(map)
}

pub fn seed_default_object_types_if_empty(
    conn: &Connection,
    now_ms: i64,
) -> Result<(), CoreError> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM object_types;", [], |r| r.get(0))?;
    if count > 0 {
        // Phase 16: Templates are keyed by deterministic ids and upserted, so this is
        // safe (and desirable) to run on every startup — it back-fills templates for
        // workspaces created before the template system existed.
        seed_default_object_templates(conn, now_ms)?;
        return Ok(());
    }

    // 1. Idea (💡)
    let idea = ObjectType {
        id: "idea".to_string(),
        display_name: "Idea".to_string(),
        icon: "💡".to_string(),
        color: "#F59E0B".to_string(),
        schema: vec![
            PropertyDefinition {
                key: "status".to_string(),
                label: "Status".to_string(),
                property_type: PropertyType::Select,
                options: vec![
                    "spark".to_string(),
                    "exploring".to_string(),
                    "dormant".to_string(),
                    "validated".to_string(),
                ],
                required: true,
                default_value: Some(serde_json::json!("spark")),
            },
            PropertyDefinition {
                key: "tags".to_string(),
                label: "Tags".to_string(),
                property_type: PropertyType::Text,
                options: vec![],
                required: false,
                default_value: Some(serde_json::json!("")),
            },
        ],
        is_system: true,
        created_at: now_ms,
        description: "A nascent thought worth exploring and validating.".to_string(),
        default_template_md:
            "## The Spark\n\n## Potential Impact\n\n## Next Experiments".to_string(),
    };
    upsert_object_type(conn, &idea)?;

    // 2. Task / Todo (☑️)
    let todo = ObjectType {
        id: "todo".to_string(),
        display_name: "Task".to_string(),
        icon: "☑️".to_string(),
        color: "#10B981".to_string(),
        schema: vec![
            PropertyDefinition {
                key: "completed".to_string(),
                label: "Completed".to_string(),
                property_type: PropertyType::Boolean,
                options: vec![],
                required: true,
                default_value: Some(serde_json::json!(false)),
            },
            PropertyDefinition {
                key: "due_date".to_string(),
                label: "Due Date".to_string(),
                property_type: PropertyType::Date,
                options: vec![],
                required: false,
                default_value: None,
            },
            PropertyDefinition {
                key: "priority".to_string(),
                label: "Priority".to_string(),
                property_type: PropertyType::Select,
                options: vec!["low".to_string(), "medium".to_string(), "high".to_string()],
                required: true,
                default_value: Some(serde_json::json!("medium")),
            },
        ],
        is_system: true,
        created_at: now_ms,
        description: "A concrete action item with acceptance criteria.".to_string(),
        default_template_md: "## Acceptance Criteria\n- [ ] \n\n## Context".to_string(),
    };
    upsert_object_type(conn, &todo)?;

    // 3. Decision (⚖️)
    let decision = ObjectType {
        id: "decision".to_string(),
        display_name: "Decision".to_string(),
        icon: "⚖️".to_string(),
        color: "#6366F1".to_string(),
        schema: vec![
            PropertyDefinition {
                key: "rationale".to_string(),
                label: "Rationale".to_string(),
                property_type: PropertyType::Text,
                options: vec![],
                required: false,
                default_value: Some(serde_json::json!("")),
            },
            PropertyDefinition {
                key: "superseded_by".to_string(),
                label: "Superseded By".to_string(),
                property_type: PropertyType::Text,
                options: vec![],
                required: false,
                default_value: None,
            },
        ],
        is_system: true,
        created_at: now_ms,
        description: "A recorded decision with rationale and consequences.".to_string(),
        default_template_md:
            "## Context & Problem\n\n## Considered Alternatives\n\n> **Decision:** \n\n## Consequences"
                .to_string(),
    };
    upsert_object_type(conn, &decision)?;

    // 4. Source (🔗)
    let source = ObjectType {
        id: "source".to_string(),
        display_name: "Source".to_string(),
        icon: "🔗".to_string(),
        color: "#3B82F6".to_string(),
        schema: vec![
            PropertyDefinition {
                key: "url".to_string(),
                label: "URL".to_string(),
                property_type: PropertyType::Url,
                options: vec![],
                required: false,
                default_value: Some(serde_json::json!("")),
            },
            PropertyDefinition {
                key: "read_status".to_string(),
                label: "Read Status".to_string(),
                property_type: PropertyType::Select,
                options: vec![
                    "unread".to_string(),
                    "reading".to_string(),
                    "processed".to_string(),
                ],
                required: true,
                default_value: Some(serde_json::json!("unread")),
            },
        ],
        is_system: true,
        created_at: now_ms,
        description: "An external reference, paper, or web page under study.".to_string(),
        default_template_md: "## Key Claims\n\n## Verified Quotes\n\n## Takeaways".to_string(),
    };
    upsert_object_type(conn, &source)?;

    // Phase 16: Seed the default named templates for the built-in types so the
    // template selector popover is populated out of the box.
    seed_default_object_templates(conn, now_ms)?;

    Ok(())
}

/// Phase 16: Seeds one canonical named template per built-in system type.
///
/// Idempotent: templates are upserted by a deterministic id (`<type>-default`), so
/// re-running this never duplicates rows.
pub fn seed_default_object_templates(conn: &Connection, now_ms: i64) -> Result<(), CoreError> {
    let presets: [(&str, &str, &str, &str, serde_json::Value); 4] = [
        (
            "idea",
            "Idea Exploration",
            "💡",
            "## The Spark\n\n## Potential Impact\n\n## Next Experiments",
            serde_json::json!({ "status": "spark" }),
        ),
        (
            "todo",
            "Task Checklist",
            "☑️",
            "## Acceptance Criteria\n- [ ] \n\n## Context",
            serde_json::json!({ "completed": false, "priority": "medium" }),
        ),
        (
            "decision",
            "Decision Record",
            "⚖️",
            "## Context & Problem\n\n## Considered Alternatives\n\n> **Decision:** \n\n## Consequences",
            serde_json::json!({ "rationale": "" }),
        ),
        (
            "source",
            "Source Review",
            "🔗",
            "## Key Claims\n\n## Verified Quotes\n\n## Takeaways",
            serde_json::json!({ "read_status": "unread" }),
        ),
    ];

    for (type_id, name, icon, body, preset) in presets {
        let template = ObjectTemplate {
            id: format!("{type_id}-default"),
            object_type_id: type_id.to_string(),
            name: name.to_string(),
            icon: icon.to_string(),
            is_default: true,
            properties_preset: preset,
            body_template_md: body.to_string(),
            created_at: now_ms,
        };
        save_object_template(conn, &template)?;
    }

    Ok(())
}
