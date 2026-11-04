use crate::state::AppState;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sthira_core::Engine;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub status: String,
    pub created_at: i64,
    pub last_substantive_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContentItem {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub item_type: String,
    pub description: String,
    pub project_id: Option<String>,
    pub object_type_id: Option<String>,
    pub object_type_name: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_draft: bool,
    pub roles: Vec<String>,
    pub item_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectOverviewData {
    pub project: ProjectSummary,
    pub items: Vec<ProjectContentItem>,
}

fn roles(member: bool, reference: bool, backlink: bool) -> Vec<String> {
    [(member, "member"), (reference, "reference"), (backlink, "backlink")]
        .into_iter().filter(|(present, _)| *present).map(|(_, name)| name.to_string()).collect()
}

/// A read-only project directory. Membership and explicit references stay distinct.
pub fn handle_get_project_overview(engine: &Engine, project_id: String) -> Result<ProjectOverviewData, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    let project = reader.query_row(
        "SELECT p.id, p.name, COALESCE(pp.purpose, ''), p.status, p.created_at, p.last_substantive_at
         FROM projects p LEFT JOIN project_purposes pp ON pp.project_id = p.id AND pp.superseded_at IS NULL
         WHERE p.id = ?1",
        [&project_id], |row| Ok(ProjectSummary {
            id: row.get(0)?, name: row.get(1)?, purpose: row.get(2)?, status: row.get(3)?,
            created_at: row.get(4)?, last_substantive_at: row.get(5)?,
        }),
    ).optional().map_err(|e| e.to_string())?.ok_or_else(|| "This project is no longer available.".to_string())?;

    let mut stmt = reader.prepare(
        "WITH candidates(id, role) AS (
            SELECT artifact_id, 'member' FROM project_artifacts WHERE project_id = ?1 AND detached_at IS NULL
            UNION ALL SELECT target_artifact_id, 'reference' FROM workspace_links WHERE source_project_id = ?1 AND target_artifact_id IS NOT NULL
            UNION ALL SELECT source_artifact_id, 'backlink' FROM workspace_links WHERE target_project_id = ?1 AND source_artifact_id IS NOT NULL
         )
         SELECT a.id, COALESCE(NULLIF(trim(a.title), ''), 'Untitled page'),
                CASE WHEN oi.artifact_id IS NOT NULL THEN 'object' ELSE a.type END,
                substr(a.content_md, 1, 400),
                CASE WHEN MAX(c.role = 'member') THEN ?1 ELSE
                    (SELECT pa.project_id FROM project_artifacts pa WHERE pa.artifact_id = a.id AND pa.detached_at IS NULL ORDER BY pa.project_id LIMIT 1) END,
                oi.object_type_id, ot.display_name, ot.icon, ot.color, a.created_at, a.updated_at, a.is_unfinished_draft,
                MAX(c.role = 'member'), MAX(c.role = 'reference'), MAX(c.role = 'backlink')
         FROM candidates c JOIN artifacts a ON a.id = c.id
         LEFT JOIN object_instances oi ON oi.artifact_id = a.id LEFT JOIN object_types ot ON ot.id = oi.object_type_id
         WHERE a.deleted_at IS NULL GROUP BY a.id ORDER BY a.updated_at DESC, a.id",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([&project_id], |row| Ok(ProjectContentItem {
        id: row.get(0)?, kind: "artifact".into(), title: row.get(1)?, item_type: row.get(2)?, description: row.get(3)?,
        project_id: row.get(4)?, object_type_id: row.get(5)?, object_type_name: row.get(6)?, icon: row.get(7)?, color: row.get(8)?,
        created_at: row.get(9)?, updated_at: row.get(10)?, is_draft: row.get(11)?,
        roles: roles(row.get(12)?, row.get(13)?, row.get(14)?), item_count: None,
    })).map_err(|e| e.to_string())?;
    let mut items = rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;

    let mut stmt = reader.prepare(
        "SELECT c.id, c.name, c.created_at,
                COALESCE((SELECT MAX(created_at) FROM canvas_layout_revisions r WHERE r.canvas_id = c.id), c.created_at),
                (SELECT COUNT(*) FROM canvas_items ci WHERE ci.canvas_id = c.id)
         FROM canvases c WHERE c.project_id = ?1 ORDER BY c.created_at DESC, c.id",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([&project_id], |row| Ok(ProjectContentItem {
        id: row.get(0)?, kind: "canvas".into(), title: row.get(1)?, item_type: "canvas".into(), description: String::new(),
        project_id: Some(project_id.clone()), object_type_id: None, object_type_name: None, icon: None, color: None,
        created_at: row.get(2)?, updated_at: row.get(3)?, is_draft: false, roles: vec!["member".into()], item_count: row.get(4)?,
    })).map_err(|e| e.to_string())?;
    items.extend(rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?);

    let mut stmt = reader.prepare(
        "WITH candidates(id, role) AS (
            SELECT target_project_id, 'reference' FROM workspace_links WHERE source_project_id = ?1 AND target_project_id IS NOT NULL
            UNION ALL SELECT source_project_id, 'backlink' FROM workspace_links WHERE target_project_id = ?1 AND source_project_id IS NOT NULL
         )
         SELECT p.id, p.name, COALESCE(pp.purpose, ''), p.created_at, p.last_substantive_at,
                MAX(c.role = 'reference'), MAX(c.role = 'backlink'),
                (SELECT COUNT(*) FROM project_artifacts pa JOIN artifacts a ON a.id = pa.artifact_id WHERE pa.project_id = p.id AND pa.detached_at IS NULL AND a.deleted_at IS NULL)
         FROM candidates c JOIN projects p ON p.id = c.id
         LEFT JOIN project_purposes pp ON pp.project_id = p.id AND pp.superseded_at IS NULL
         WHERE p.id <> ?1 GROUP BY p.id ORDER BY p.last_substantive_at DESC, p.id",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![project_id], |row| Ok(ProjectContentItem {
        id: row.get(0)?, kind: "project".into(), title: row.get(1)?, item_type: "project".into(), description: row.get(2)?,
        project_id: None, object_type_id: None, object_type_name: None, icon: None, color: None,
        created_at: row.get(3)?, updated_at: row.get(4)?, is_draft: false,
        roles: roles(false, row.get(5)?, row.get(6)?), item_count: row.get(7)?,
    })).map_err(|e| e.to_string())?;
    items.extend(rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?);
    Ok(ProjectOverviewData { project, items })
}

#[tauri::command]
pub fn cmd_get_project_overview(state: State<'_, AppState>, project_id: String) -> Result<ProjectOverviewData, String> {
    handle_get_project_overview(&state.engine, project_id)
}
