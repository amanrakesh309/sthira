use crate::state::AppState;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sthira_core::{Engine, ObjectInstance, ObjectType};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Artifact,
    Project,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntityRef {
    pub kind: EntityKind,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceEntity {
    pub kind: EntityKind,
    pub id: String,
    pub title: String,
    pub type_label: String,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceEntityDetail {
    pub entity: WorkspaceEntity,
    pub content_md: String,
    pub description: String,
    pub members: Vec<WorkspaceEntity>,
    pub object_instance: Option<ObjectInstance>,
    pub object_type: Option<ObjectType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConnection {
    pub id: String,
    pub source: WorkspaceEntity,
    pub target: WorkspaceEntity,
    pub created_at: i64,
    pub relationship: String,
}

fn resolve(conn: &Connection, entity: &EntityRef) -> Result<WorkspaceEntity, String> {
    let (title, type_label, project_id) = match entity.kind {
        EntityKind::Artifact => conn.query_row(
            "SELECT COALESCE(NULLIF(a.title, ''), 'Untitled page'),
                    COALESCE(ot.display_name, CASE a.type WHEN 'note' THEN 'Page' WHEN 'source' THEN 'Source' ELSE 'Snippet' END),
                    CASE WHEN (
                        SELECT COUNT(*) FROM project_artifacts pa
                        JOIN projects p2 ON p2.id = pa.project_id
                        WHERE pa.artifact_id = a.id AND pa.detached_at IS NULL AND p2.deleted_at IS NULL
                    ) = 1
                    THEN (
                        SELECT pa.project_id FROM project_artifacts pa
                        JOIN projects p2 ON p2.id = pa.project_id
                        WHERE pa.artifact_id = a.id AND pa.detached_at IS NULL AND p2.deleted_at IS NULL
                        LIMIT 1
                    )
                    ELSE NULL END
             FROM artifacts a LEFT JOIN object_instances oi ON oi.artifact_id = a.id
             LEFT JOIN object_types ot ON ot.id = oi.object_type_id
             WHERE a.id = ?1 AND a.deleted_at IS NULL;",
            [&entity.id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)),
        ),
        EntityKind::Project => conn.query_row(
            "SELECT name, 'Project', NULL FROM projects WHERE id = ?1 AND deleted_at IS NULL;",
            [&entity.id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)),
        ),
    }.map_err(|_| "This item is no longer available.".to_string())?;
    Ok(WorkspaceEntity {
        kind: entity.kind.clone(),
        id: entity.id.clone(),
        title,
        type_label,
        project_id,
    })
}

pub fn handle_get_workspace_entity(
    engine: &Engine,
    entity: EntityRef,
) -> Result<WorkspaceEntityDetail, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    let resolved = resolve(&reader, &entity)?;
    let mut detail = WorkspaceEntityDetail {
        entity: resolved,
        content_md: String::new(),
        description: String::new(),
        members: vec![],
        object_instance: None,
        object_type: None,
    };
    match entity.kind {
        EntityKind::Artifact => {
            detail.content_md = reader
                .query_row(
                    "SELECT content_md FROM artifacts WHERE id = ?1",
                    [&entity.id],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            detail.object_instance =
                sthira_core::db::objects_repo::get_object_instance(&reader, &entity.id)
                    .map_err(|e| e.to_string())?;
            if let Some(ref instance) = detail.object_instance {
                detail.object_type = sthira_core::db::objects_repo::get_object_type(
                    &reader,
                    &instance.object_type_id,
                )
                .map_err(|e| e.to_string())?;
            }
        }
        EntityKind::Project => {
            detail.description = reader.query_row("SELECT purpose FROM project_purposes WHERE project_id = ?1 AND superseded_at IS NULL", [&entity.id], |r| r.get(0)).unwrap_or_default();
            let mut stmt = reader.prepare("SELECT a.id FROM project_artifacts pa JOIN artifacts a ON a.id = pa.artifact_id WHERE pa.project_id = ?1 AND pa.detached_at IS NULL AND a.deleted_at IS NULL ORDER BY a.updated_at DESC, a.id").map_err(|e| e.to_string())?;
            let ids = stmt
                .query_map([&entity.id], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            for id in ids {
                detail.members.push(resolve(
                    &reader,
                    &EntityRef {
                        kind: EntityKind::Artifact,
                        id: id.map_err(|e| e.to_string())?,
                    },
                )?);
            }
        }
    }
    Ok(detail)
}

pub fn reference_from_query(query: &str) -> Option<EntityRef> {
    let reference = sthira_core::references::parse_workspace_reference(query)?;
    Some(EntityRef {
        kind: if reference.kind == "project" {
            EntityKind::Project
        } else {
            EntityKind::Artifact
        },
        id: reference.id,
    })
}

pub fn lookup_reference_query(engine: &Engine, query: &str) -> Option<Vec<WorkspaceEntity>> {
    let reader = engine.db().reader().ok()?;
    if let Some(reference) = reference_from_query(query) {
        return Some(resolve(&reader, &reference).into_iter().collect());
    }
    let id = query.trim();
    if uuid::Uuid::parse_str(id).is_ok() {
        let matches: Vec<_> = [EntityKind::Artifact, EntityKind::Project]
            .into_iter()
            .filter_map(|kind| {
                resolve(
                    &reader,
                    &EntityRef {
                        kind,
                        id: id.into(),
                    },
                )
                .ok()
            })
            .collect();
        return Some(matches);
    }
    None
}

pub fn handle_search_connection_targets(
    engine: &Engine,
    query: String,
) -> Result<Vec<WorkspaceEntity>, String> {
    if let Some(matches) = lookup_reference_query(engine, &query) {
        return Ok(matches);
    }
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    let mut stmt = reader.prepare(
        "SELECT kind, id FROM (
            SELECT 'artifact' AS kind, a.id, COALESCE(a.title, 'Untitled page') AS title, a.updated_at AS recency
            FROM artifacts a LEFT JOIN object_instances oi ON oi.artifact_id = a.id LEFT JOIN object_types ot ON ot.id = oi.object_type_id
            WHERE a.deleted_at IS NULL AND instr(lower(COALESCE(a.title, '') || ' ' || COALESCE(ot.display_name, 'Page')), lower(?1)) > 0
            UNION ALL
            SELECT 'project', id, name, last_substantive_at FROM projects WHERE deleted_at IS NULL AND instr(lower(name || ' Project'), lower(?1)) > 0
        ) ORDER BY recency DESC, title COLLATE NOCASE, id LIMIT 60;"
    ).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([query.trim()], |r| {
            Ok(EntityRef {
                kind: if r.get::<_, String>(0)? == "project" {
                    EntityKind::Project
                } else {
                    EntityKind::Artifact
                },
                id: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.map(|row| resolve(&reader, &row.map_err(|e| e.to_string())?))
        .collect()
}

pub fn handle_get_workspace_connections(
    engine: &Engine,
    entity: EntityRef,
) -> Result<Vec<WorkspaceConnection>, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    resolve(&reader, &entity)?;
    let (artifact_id, project_id) = split_ref(&entity);
    let mut stmt = reader.prepare(
        "SELECT id, source_artifact_id, source_project_id, target_artifact_id, target_project_id, created_at, 'references' AS relationship
         FROM workspace_links WHERE source_artifact_id = ?1 OR target_artifact_id = ?1 OR source_project_id = ?2 OR target_project_id = ?2
         UNION ALL
         SELECT 'lineage:' || id, id, NULL, derived_from_artifact_id, NULL, created_at, 'derived_from'
         FROM artifacts WHERE deleted_at IS NULL AND derived_from_artifact_id IS NOT NULL AND (id = ?1 OR derived_from_artifact_id = ?1)
         ORDER BY created_at DESC, id;"
    ).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![artifact_id, project_id], |r| {
            let source_artifact: Option<String> = r.get(1)?;
            let target_artifact: Option<String> = r.get(3)?;
            Ok((
                r.get::<_, String>(0)?,
                EntityRef {
                    kind: if source_artifact.is_some() {
                        EntityKind::Artifact
                    } else {
                        EntityKind::Project
                    },
                    id: source_artifact
                        .unwrap_or(r.get::<_, Option<String>>(2)?.unwrap_or_default()),
                },
                EntityRef {
                    kind: if target_artifact.is_some() {
                        EntityKind::Artifact
                    } else {
                        EntityKind::Project
                    },
                    id: target_artifact
                        .unwrap_or(r.get::<_, Option<String>>(4)?.unwrap_or_default()),
                },
                r.get::<_, i64>(5)?,
                r.get::<_, String>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut result = vec![];
    for row in rows {
        let (id, source, target, created_at, relationship) = row.map_err(|e| e.to_string())?;
        // Soft-deleted endpoints must not produce dangling reference cards.
        if let (Ok(source), Ok(target)) = (resolve(&reader, &source), resolve(&reader, &target)) {
            result.push(WorkspaceConnection {
                id,
                source,
                target,
                created_at,
                relationship,
            });
        }
    }
    Ok(result)
}

fn split_ref(entity: &EntityRef) -> (Option<&str>, Option<&str>) {
    match entity.kind {
        EntityKind::Artifact => (Some(entity.id.as_str()), None),
        EntityKind::Project => (None, Some(entity.id.as_str())),
    }
}

pub fn handle_create_workspace_connection(
    engine: &Engine,
    source: EntityRef,
    target: EntityRef,
) -> Result<String, String> {
    if source == target {
        return Err("An item cannot link to itself.".to_string());
    }
    let mut writer = engine.db().writer();
    let tx = writer.transaction().map_err(|e| e.to_string())?;
    resolve(&tx, &source)?;
    resolve(&tx, &target)?;
    let (sa, sp) = split_ref(&source);
    let (ta, tp) = split_ref(&target);
    tx.execute(
        "INSERT OR IGNORE INTO workspace_links (id, source_artifact_id, source_project_id, target_artifact_id, target_project_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![uuid::Uuid::new_v4().to_string(), sa, sp, ta, tp, engine.clock().now_ms()],
    ).map_err(|e| e.to_string())?;
    let id = tx.query_row(
        "SELECT id FROM workspace_links WHERE source_artifact_id IS ?1 AND source_project_id IS ?2 AND target_artifact_id IS ?3 AND target_project_id IS ?4",
        params![sa, sp, ta, tp], |r| r.get::<_, String>(0),
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

pub fn handle_delete_workspace_connection(
    engine: &Engine,
    connection_id: String,
) -> Result<(), String> {
    if connection_id.starts_with("lineage:") {
        return Err("Derivation lineage is preserved with the document.".into());
    }
    engine
        .db()
        .writer()
        .execute("DELETE FROM workspace_links WHERE id = ?1", [connection_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn cmd_get_workspace_entity(
    state: State<'_, AppState>,
    entity: EntityRef,
) -> Result<WorkspaceEntityDetail, String> {
    handle_get_workspace_entity(&state.engine, entity)
}
#[tauri::command]
pub fn cmd_search_connection_targets(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<WorkspaceEntity>, String> {
    handle_search_connection_targets(&state.engine, query)
}
#[tauri::command]
pub fn cmd_get_workspace_connections(
    state: State<'_, AppState>,
    entity: EntityRef,
) -> Result<Vec<WorkspaceConnection>, String> {
    handle_get_workspace_connections(&state.engine, entity)
}
#[tauri::command]
pub fn cmd_create_workspace_connection(
    state: State<'_, AppState>,
    source: EntityRef,
    target: EntityRef,
) -> Result<String, String> {
    handle_create_workspace_connection(&state.engine, source, target)
}
#[tauri::command]
pub fn cmd_delete_workspace_connection(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    handle_delete_workspace_connection(&state.engine, connection_id)
}
