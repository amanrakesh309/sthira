use crate::{
    models::objects::{ObjectInstance, ObjectItem, ObjectType, PropertyDefinition, PropertyType},
    CoreError,
};
use rusqlite::{params, Connection};
use std::collections::HashMap;

pub fn list_object_types(conn: &Connection) -> Result<Vec<ObjectType>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, display_name, icon, color, schema_json, is_system, created_at
         FROM object_types
         ORDER BY is_system DESC, display_name ASC;",
    )?;

    let rows = stmt.query_map([], |r| {
        let id: String = r.get(0)?;
        let display_name: String = r.get(1)?;
        let icon: String = r.get(2)?;
        let color: String = r.get(3)?;
        let schema_json: String = r.get(4)?;
        let is_system: bool = r.get::<_, i32>(5)? != 0;
        let created_at: i64 = r.get(6)?;

        let schema: Vec<PropertyDefinition> =
            serde_json::from_str(&schema_json).unwrap_or_default();

        Ok(ObjectType {
            id,
            display_name,
            icon,
            color,
            schema,
            is_system,
            created_at,
        })
    })?;

    let mut list = Vec::new();
    for row in rows {
        list.push(row?);
    }
    Ok(list)
}

pub fn get_object_type(conn: &Connection, id: &str) -> Result<Option<ObjectType>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, display_name, icon, color, schema_json, is_system, created_at
         FROM object_types
         WHERE id = ?1;",
    )?;

    let mut rows = stmt.query(params![id])?;
    if let Some(r) = rows.next()? {
        let id: String = r.get(0)?;
        let display_name: String = r.get(1)?;
        let icon: String = r.get(2)?;
        let color: String = r.get(3)?;
        let schema_json: String = r.get(4)?;
        let is_system: bool = r.get::<_, i32>(5)? != 0;
        let created_at: i64 = r.get(6)?;

        let schema: Vec<PropertyDefinition> =
            serde_json::from_str(&schema_json).unwrap_or_default();

        Ok(Some(ObjectType {
            id,
            display_name,
            icon,
            color,
            schema,
            is_system,
            created_at,
        }))
    } else {
        Ok(None)
    }
}

pub fn upsert_object_type(conn: &Connection, obj_type: &ObjectType) -> Result<(), CoreError> {
    let schema_json = serde_json::to_string(&obj_type.schema)?;

    conn.execute(
        "INSERT INTO object_types (id, display_name, icon, color, schema_json, is_system, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
            display_name = excluded.display_name,
            icon = excluded.icon,
            color = excluded.color,
            schema_json = excluded.schema_json;",
        params![
            obj_type.id,
            obj_type.display_name,
            obj_type.icon,
            obj_type.color,
            schema_json,
            if obj_type.is_system { 1 } else { 0 },
            obj_type.created_at,
        ],
    )?;

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
    };
    upsert_object_type(conn, &source)?;

    Ok(())
}
