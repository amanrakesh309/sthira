use tauri::State;
use serde::{Deserialize, Serialize};
use rusqlite::params;
use sthira_core::{
    Actor, Artifact, ArtifactType, Authorship, CanvasItemKind, EventType, Materiality,
    Engine, ResumeBrief, ResumePaneHint,
    ObjectInstance, ObjectItem, ObjectTemplate, ObjectType,
    history::ProjectHistory,
    router::{Approval, RouterPlan},
    OperationalMode, ProgressiveStreamChunk,
};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyOutcome {
    pub success: bool,
    pub created_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCardView {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub last_substantive_at: i64,
    pub item_count: usize,
    pub category: String,
    pub icon_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentItemView {
    pub id: String,
    pub title: String,
    pub item_type: String,
    pub subtitle: String,
    pub project_id: Option<String>,
    pub artifact_id: Option<String>,
    pub canvas_id: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeStateView {
    pub projects: Vec<ProjectCardView>,
    pub recent_items: Vec<RecentItemView>,
    pub loose_captures: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTreePage {
    pub id: String,
    pub title: String,
    pub artifact_type: String,
    pub is_draft: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTreeCanvas {
    pub id: String,
    pub name: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTreeProject {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub pages: Vec<WorkspaceTreePage>,
    pub canvases: Vec<WorkspaceTreeCanvas>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTree {
    pub projects: Vec<WorkspaceTreeProject>,
    pub unassociated_pages: Vec<WorkspaceTreePage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarItem {
    pub id: String,
    pub title: String,
    pub item_type: String, // "project" | "note" | "object"
    pub is_pinned: bool,
    pub last_accessed_at: i64,
    pub updated_at: i64,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub object_type_id: Option<String>,
    pub object_type_name: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarItemsResponse {
    pub projects: Vec<SidebarItem>,
    pub notes: Vec<SidebarItem>,
    pub objects: Vec<SidebarItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCounts {
    pub all: usize,
    pub projects: usize,
    pub notes: usize,
    pub objects: usize,
    pub standalone: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceIndexItem {
    pub id: String,
    pub title: String,
    pub entity_type: String, // "project" | "note" | "object"
    pub description: Option<String>,
    pub is_pinned: bool,
    pub last_accessed_at: i64,
    pub updated_at: i64,
    pub created_at: i64,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub standalone: bool,
    pub item_count: Option<usize>,
    pub object_type_id: Option<String>,
    pub object_type_name: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceIndexData {
    pub counts: WorkspaceCounts,
    pub items: Vec<WorkspaceIndexItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpistemicNodeView {
    pub id: String,
    pub artifact_id: String,
    pub node_type: String,
    pub origin: String,
    pub lifecycle: String,
    pub statement: String,
    pub quote: Option<String>,
    pub quote_start: Option<i64>,
    pub quote_end: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalView {
    pub id: String,
    pub kind: String,
    pub reason: String,
    pub source_artifact_id: String,
    pub target_artifact_id: String,
    pub target_title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactContext {
    pub artifact: Artifact,
    pub epistemic_nodes: Vec<EpistemicNodeView>,
    pub pending_proposals: Vec<ProposalView>,
    pub project_id: Option<String>,
    pub is_unfinished_draft: bool,
    #[serde(default)]
    pub derived_from_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasItemPosition {
    pub item_id: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", content = "payload")]
pub enum CanvasOpsRequest {
    GetCanvas {
        canvas_id: Option<String>,
        project_id: Option<String>,
    },
    MoveItem {
        canvas_id: String,
        item_id: String,
        x: f64,
        y: f64,
    },
    AddItem {
        canvas_id: String,
        item_kind: String,
        artifact_id: Option<String>,
        x: f64,
        y: f64,
        w: Option<f64>,
        h: Option<f64>,
    },
    BatchUpdatePositions {
        canvas_id: String,
        positions: Vec<CanvasItemPosition>,
    },
    CreateLink {
        source_artifact_id: String,
        target_artifact_id: String,
        relationship_type: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasItemView {
    pub item_id: String,
    pub item_kind: String,
    pub artifact_id: Option<String>,
    pub title: Option<String>,
    pub excerpt: Option<String>,
    pub x: f64,
    pub y: f64,
    pub w: Option<f64>,
    pub h: Option<f64>,
    pub z: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactLinkView {
    pub source_artifact_id: String,
    pub target_artifact_id: String,
    pub relationship_type: String,
    pub origin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasDataView {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub items: Vec<CanvasItemView>,
    pub links: Vec<ArtifactLinkView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasOpsResponse {
    pub canvas: Option<CanvasDataView>,
    pub change_set_id: Option<String>,
    pub success: bool,
}

// ---------------------------------------------------------------------------
// Pure Handlers (Directly testable without webview)
// ---------------------------------------------------------------------------

pub async fn handle_route_input(engine: &Engine, raw: String) -> Result<RouterPlan, String> {
    engine.route_input(&raw).await.map_err(|e| e.to_string())
}

pub fn handle_apply_plan(
    engine: &Engine,
    plan: RouterPlan,
    approved: bool,
) -> Result<ApplyOutcome, String> {
    let approval = if approved {
        Approval::Confirmed
    } else {
        Approval::Unconfirmed
    };

    match engine.apply_plan(&plan, approval) {
        Ok(id) => Ok(ApplyOutcome {
            success: true,
            created_id: id,
            message: "Plan applied successfully".to_string(),
        }),
        Err(e) => Ok(ApplyOutcome {
            success: false,
            created_id: None,
            message: e.to_string(),
        }),
    }
}

pub fn handle_get_home_state(engine: &Engine) -> Result<HomeStateView, String> {
    let home = engine.get_home_state().map_err(|e| e.to_string())?;
    let reader = engine.db().reader().map_err(|e| e.to_string())?;

    let mut project_views = Vec::new();
    for thread in home.threads {
        let count_i64: i64 = reader
            .query_row(
                "SELECT COUNT(*) FROM project_artifacts WHERE project_id = ?1 AND detached_at IS NULL;",
                params![thread.project_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let count = count_i64 as usize;

        let is_research = thread.name.to_lowercase().contains("research")
            || thread.purpose.to_lowercase().contains("model")
            || thread.purpose.to_lowercase().contains("ai");

        project_views.push(ProjectCardView {
            id: thread.project_id,
            name: thread.name,
            purpose: thread.purpose,
            last_substantive_at: thread.last_substantive_at,
            item_count: count,
            category: if is_research { "Research".to_string() } else { "Project".to_string() },
            icon_type: if is_research { "brain".to_string() } else { "code".to_string() },
        });
    }

    // Recent items: fetch top 2 recent items across artifacts and canvases
    let mut recent_items = Vec::new();

    // 1. Most recently updated artifact
    let recent_art: Option<(String, String, Option<String>, i64)> = reader
        .query_row(
            "SELECT a.id, a.title, pa.project_id, a.updated_at
             FROM artifacts a
             LEFT JOIN project_artifacts pa ON a.id = pa.artifact_id AND pa.detached_at IS NULL
             WHERE a.deleted_at IS NULL
             ORDER BY a.updated_at DESC LIMIT 1;",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .ok();

    if let Some((aid, title, pid, ts)) = recent_art {
        recent_items.push(RecentItemView {
            id: aid.clone(),
            title,
            item_type: "Document".to_string(),
            subtitle: "Document · Edited recently".to_string(),
            project_id: pid,
            artifact_id: Some(aid),
            canvas_id: None,
            timestamp: ts,
        });
    }

    // 2. Most recent canvas
    let recent_canvas: Option<(String, String, Option<String>, i64)> = reader
        .query_row(
            "SELECT id, name, project_id, created_at FROM canvases ORDER BY created_at DESC LIMIT 1;",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .ok();

    if let Some((cid, name, pid, ts)) = recent_canvas {
        recent_items.push(RecentItemView {
            id: cid.clone(),
            title: name,
            item_type: "Canvas".to_string(),
            subtitle: "Canvas · Last opened earlier".to_string(),
            project_id: pid,
            artifact_id: None,
            canvas_id: Some(cid),
            timestamp: ts,
        });
    }

    Ok(HomeStateView {
        projects: project_views,
        recent_items,
        loose_captures: home.loose_captures,
    })
}

pub fn handle_get_workspace_tree(engine: &Engine) -> Result<WorkspaceTree, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;

    // 1. Projects with nested pages & canvases
    let mut proj_stmt = reader
        .prepare(
            "SELECT p.id, p.name, COALESCE(pp.purpose, ''), p.last_substantive_at
             FROM projects p
             LEFT JOIN project_purposes pp ON p.id = pp.project_id AND pp.superseded_at IS NULL
             ORDER BY p.last_substantive_at DESC, p.name ASC;",
        )
        .map_err(|e| e.to_string())?;

    let mut page_stmt = reader
        .prepare(
            "SELECT a.id, COALESCE(a.title, 'Untitled Note'), a.type, a.updated_at, a.is_unfinished_draft
             FROM artifacts a
             JOIN project_artifacts pa ON a.id = pa.artifact_id
             WHERE pa.project_id = ?1
               AND pa.detached_at IS NULL
               AND a.deleted_at IS NULL
             ORDER BY a.updated_at DESC, a.title ASC;",
        )
        .map_err(|e| e.to_string())?;

    let mut canvas_stmt = reader
        .prepare(
            "SELECT id, name, created_at
             FROM canvases
             WHERE project_id = ?1
             ORDER BY created_at DESC;",
        )
        .map_err(|e| e.to_string())?;

    let proj_rows = proj_stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let purpose: String = row.get(2)?;
            let updated_at: i64 = row.get(3)?;
            Ok((id, name, purpose, updated_at))
        })
        .map_err(|e| e.to_string())?;

    let mut projects = Vec::new();
    for p_res in proj_rows {
        let (p_id, p_name, p_purpose, p_updated) = p_res.map_err(|e| e.to_string())?;

        let pages: Vec<WorkspaceTreePage> = page_stmt
            .query_map(params![p_id], |r| {
                Ok(WorkspaceTreePage {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    artifact_type: r.get(2)?,
                    updated_at: r.get(3)?,
                    is_draft: r.get::<_, i32>(4)? != 0,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        let canvases: Vec<WorkspaceTreeCanvas> = canvas_stmt
            .query_map(params![p_id], |r| {
                Ok(WorkspaceTreeCanvas {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    created_at: r.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        projects.push(WorkspaceTreeProject {
            id: p_id,
            name: p_name,
            purpose: p_purpose,
            pages,
            canvases,
            updated_at: p_updated,
        });
    }

    // 2. Unassociated Pages (loose notes/captures where artifact is not attached to any project)
    let mut unassoc_stmt = reader
        .prepare(
            "SELECT a.id, COALESCE(a.title, 'Untitled Note'), a.type, a.updated_at, a.is_unfinished_draft
             FROM artifacts a
             WHERE a.deleted_at IS NULL
               AND a.id NOT IN (
                   SELECT artifact_id FROM project_artifacts WHERE detached_at IS NULL
               )
             ORDER BY a.updated_at DESC, a.title ASC;",
        )
        .map_err(|e| e.to_string())?;

    let unassociated_pages: Vec<WorkspaceTreePage> = unassoc_stmt
        .query_map([], |r| {
            Ok(WorkspaceTreePage {
                id: r.get(0)?,
                title: r.get(1)?,
                artifact_type: r.get(2)?,
                updated_at: r.get(3)?,
                is_draft: r.get::<_, i32>(4)? != 0,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(WorkspaceTree {
        projects,
        unassociated_pages,
    })
}

pub fn handle_create_project(
    engine: &Engine,
    name: String,
    purpose: Option<String>,
) -> Result<WorkspaceTreeProject, String> {
    let p = engine
        .create_project(&name, None)
        .map_err(|e| e.to_string())?;
    let purp = purpose.unwrap_or_else(|| format!("Project {}", name));
    let _ = engine.set_project_purpose(&p.id, &purp);

    Ok(WorkspaceTreeProject {
        id: p.id,
        name: p.name,
        purpose: purp,
        pages: vec![],
        canvases: vec![],
        updated_at: p.last_substantive_at,
    })
}

pub fn handle_create_page(
    engine: &Engine,
    project_id: Option<String>,
    title: Option<String>,
) -> Result<Artifact, String> {
    let t = title.unwrap_or_else(|| "Untitled Note".to_string());
    let initial_content = format!("# {}\n\n", t);
    let art = engine
        .create_artifact(ArtifactType::Note, Authorship::User, Some(&t), &initial_content, None)
        .map_err(|e| e.to_string())?;

    let valid_project_id = project_id.and_then(|pid| {
        let trimmed = pid.trim().to_string();
        if trimmed.is_empty() || trimmed == "default_project" {
            None
        } else {
            let exists: bool = if let Ok(reader) = engine.db().reader() {
                reader
                    .query_row(
                        "SELECT 1 FROM projects WHERE id = ?1;",
                        rusqlite::params![trimmed],
                        |_| Ok(true),
                    )
                    .unwrap_or(false)
            } else {
                false
            };
            if exists {
                Some(trimmed)
            } else {
                None
            }
        }
    });

    if let Some(ref pid) = valid_project_id {
        let _ = engine.attach_artifact(pid, &art.id, "user_explicit", None);
    }
    Ok(art)
}

pub async fn handle_get_resume_brief(
    engine: &Engine,
    project_id: String,
    level: u8,
) -> Result<ResumeBrief, String> {
    engine
        .get_resume_brief(&project_id, level as i64)
        .await
        .map_err(|e| e.to_string())
}

pub fn handle_get_resume_pane_hint(
    engine: &Engine,
    project_id: String,
) -> Result<ResumePaneHint, String> {
    engine
        .get_resume_pane_hint(&project_id)
        .map_err(|e| e.to_string())
}

pub fn handle_get_project_history(
    engine: &Engine,
    project_id: String,
) -> Result<ProjectHistory, String> {
    engine
        .get_project_history(&project_id)
        .map_err(|e| e.to_string())
}

pub fn handle_get_artifact_context(
    engine: &Engine,
    artifact_id: String,
) -> Result<ArtifactContext, String> {
    let art = engine.get_artifact(&artifact_id).map_err(|e| e.to_string())?;
    let reader = engine.db().reader().map_err(|e| e.to_string())?;

    // Project ID
    let project_id: Option<String> = reader
        .query_row(
            "SELECT project_id FROM project_artifacts WHERE artifact_id = ?1 AND detached_at IS NULL LIMIT 1;",
            params![artifact_id],
            |r| r.get(0),
        )
        .ok();

    // Epistemic nodes
    let mut node_stmt = reader
        .prepare(
            "SELECT id, artifact_id, node_type, origin, lifecycle, statement, quote, quote_start, quote_end
             FROM epistemic_nodes
             WHERE artifact_id = ?1 AND lifecycle <> 'stale'
             ORDER BY first_seen_at ASC;",
        )
        .map_err(|e| e.to_string())?;

    let nodes = node_stmt
        .query_map(params![artifact_id], |r| {
            Ok(EpistemicNodeView {
                id: r.get(0)?,
                artifact_id: r.get(1)?,
                node_type: r.get(2)?,
                origin: r.get(3)?,
                lifecycle: r.get(4)?,
                statement: r.get(5)?,
                quote: r.get(6)?,
                quote_start: r.get(7)?,
                quote_end: r.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Pending proposals (up to 2)
    let pattern = format!("%{}%", artifact_id);
    let mut prop_stmt = reader
        .prepare(
            "SELECT id, kind, evidence, payload
             FROM ai_proposals
             WHERE state = 'pending' AND payload LIKE ?1
             ORDER BY created_at DESC LIMIT 2;",
        )
        .map_err(|e| e.to_string())?;

    let proposals = prop_stmt
        .query_map(params![pattern], |r| {
            let id: String = r.get(0)?;
            let kind: String = r.get(1)?;
            let evidence_str: String = r.get(2)?;
            let payload_str: String = r.get(3)?;
            let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();
            let src = payload.get("source_artifact_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let dst = payload.get("target_artifact_id").and_then(|v| v.as_str()).unwrap_or("").to_string();

            Ok(ProposalView {
                id,
                kind,
                reason: evidence_str,
                source_artifact_id: src,
                target_artifact_id: dst.clone(),
                target_title: format!("Artifact {}", dst),
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let derived_from_title: Option<String> = if let Some(ref parent_id) = art.derived_from_artifact_id {
        reader
            .query_row(
                "SELECT title FROM artifacts WHERE id = ?1;",
                params![parent_id],
                |r| r.get(0),
            )
            .ok()
            .flatten()
            .or_else(|| Some("Untitled Note".to_string()))
    } else {
        None
    };

    Ok(ArtifactContext {
        is_unfinished_draft: art.is_unfinished_draft,
        artifact: art,
        epistemic_nodes: nodes,
        pending_proposals: proposals,
        project_id,
        derived_from_title,
    })
}

pub fn handle_save_artifact(
    engine: &Engine,
    artifact_id: String,
    content_md: String,
    title: Option<String>,
    project_id: Option<String>,
) -> Result<Artifact, String> {
    let explicit_title = title.and_then(|t| {
        let trimmed = t.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    let auto_title = content_md
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| {
            let t = l.trim();
            if t.starts_with("# ") {
                t.trim_start_matches("# ").trim().to_string()
            } else {
                let mut chars = t.chars();
                let s: String = chars.by_ref().take(40).collect();
                s.trim().to_string()
            }
        })
        .filter(|s| !s.is_empty());

    let final_title = explicit_title
        .clone()
        .or(auto_title)
        .unwrap_or_else(|| "Untitled Note".to_string());

    let mut final_content = content_md;
    if let Some(ref exp_title) = explicit_title {
        if let Some(first_line) = final_content.lines().next() {
            if first_line.trim().starts_with("# ") {
                let rest = if let Some(pos) = final_content.find('\n') {
                    &final_content[pos..]
                } else {
                    ""
                };
                final_content = format!("# {}{}", exp_title, rest);
            }
        }
    }

    // Validate that project_id actually exists in projects table to prevent foreign key violations
    let valid_project_id = project_id.and_then(|pid| {
        let trimmed = pid.trim().to_string();
        if trimmed.is_empty() || trimmed == "default_project" {
            None
        } else {
            let exists: bool = if let Ok(reader) = engine.db().reader() {
                reader
                    .query_row(
                        "SELECT 1 FROM projects WHERE id = ?1;",
                        rusqlite::params![trimmed],
                        |_| Ok(true),
                    )
                    .unwrap_or(false)
            } else {
                false
            };
            if exists {
                Some(trimmed)
            } else {
                None
            }
        }
    });

    if artifact_id.is_empty() {
        let art = engine
            .create_artifact(ArtifactType::Note, Authorship::User, Some(&final_title), &final_content, None)
            .map_err(|e| e.to_string())?;
        if let Some(ref pid) = valid_project_id {
            let _ = engine.attach_artifact(pid, &art.id, "user_explicit", None);
        }
        Ok(art)
    } else {
        {
            let writer = engine.db().writer();
            let _ = writer.execute(
                "UPDATE artifacts SET title = ?1 WHERE id = ?2;",
                rusqlite::params![final_title, artifact_id],
            );
        }
        let updated = engine
            .update_artifact(&artifact_id, &final_content, Actor::User, valid_project_id.as_deref())
            .map_err(|e| e.to_string())?;

        if let Some(ref fp) = updated.file_path {
            let full_path = engine.vault_dir().join(fp);
            let rendered = sthira_core::vault::render_file_with_frontmatter(&updated);
            let _ = std::fs::write(&full_path, rendered);
        }

        Ok(updated)
    }
}

pub fn handle_decide_proposal(
    engine: &Engine,
    proposal_id: String,
    decision: String,
) -> Result<String, String> {
    if decision.to_lowercase() == "accept" {
        engine
            .apply_discovery_link(&proposal_id)
            .map_err(|e| e.to_string())
    } else {
        let now_ms = engine.clock().now_ms();
        {
            let writer = engine.db().writer();
            writer
                .execute(
                    "UPDATE ai_proposals SET state = 'dismissed', decided_at = ?1 WHERE id = ?2;",
                    params![now_ms, proposal_id],
                )
                .map_err(|e| e.to_string())?;
        } // writer guard dropped here — prevents reentrant Mutex deadlock in log_event

        let _ = engine.log_event(
            None,
            None,
            Actor::User,
            EventType::ProposalDismissed,
            Materiality::None,
            serde_json::json!({ "proposal_id": proposal_id }),
        );

        Ok(proposal_id)
    }
}

pub fn handle_revert_change_set(
    engine: &Engine,
    change_set_id: String,
) -> Result<String, String> {
    engine
        .revert_change_set(&change_set_id)
        .map_err(|e| e.to_string())
}

pub fn handle_promote_to_project(
    engine: &Engine,
    artifact_ids: Vec<String>,
    name: String,
) -> Result<String, String> {
    let refs: Vec<&str> = artifact_ids.iter().map(|s| s.as_str()).collect();
    let proj = engine
        .promote_to_project(&refs, &name)
        .map_err(|e| e.to_string())?;
    Ok(proj.id)
}

pub fn handle_canvas_ops(
    engine: &Engine,
    req: CanvasOpsRequest,
) -> Result<CanvasOpsResponse, String> {
    match req {
        CanvasOpsRequest::GetCanvas { canvas_id, project_id } => {
            let reader = engine.db().reader().map_err(|e| e.to_string())?;
            let cid = if let Some(id) = canvas_id {
                id
            } else if let Some(ref pid) = project_id {
                reader
                    .query_row(
                        "SELECT id FROM canvases WHERE project_id = ?1 ORDER BY created_at DESC LIMIT 1;",
                        params![pid],
                        |r| r.get(0),
                    )
                    .unwrap_or_else(|_| {
                        drop(reader);
                        engine.create_canvas(project_id.as_deref(), "Workspace Canvas").unwrap()
                    })
            } else {
                reader
                    .query_row(
                        "SELECT id FROM canvases ORDER BY created_at DESC LIMIT 1;",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap_or_else(|_| {
                        drop(reader);
                        engine.create_canvas(None, "General Canvas").unwrap()
                    })
            };

            let reader = engine.db().reader().map_err(|e| e.to_string())?;
            let (name, pid): (String, Option<String>) = reader
                .query_row(
                    "SELECT name, project_id FROM canvases WHERE id = ?1;",
                    params![cid],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(|e| e.to_string())?;

            let mut item_stmt = reader
                .prepare(
                    "SELECT ci.item_id, ci.item_kind, ci.artifact_id, a.title, SUBSTR(a.content_md, 1, 100), ci.x, ci.y, ci.w, ci.h, ci.z
                     FROM canvas_items ci
                     LEFT JOIN artifacts a ON ci.artifact_id = a.id
                     WHERE ci.canvas_id = ?1;",
                )
                .map_err(|e| e.to_string())?;

            let items = item_stmt
                .query_map(params![cid], |r| {
                    Ok(CanvasItemView {
                        item_id: r.get(0)?,
                        item_kind: r.get(1)?,
                        artifact_id: r.get(2)?,
                        title: r.get(3)?,
                        excerpt: r.get(4)?,
                        x: r.get(5)?,
                        y: r.get(6)?,
                        w: r.get(7)?,
                        h: r.get(8)?,
                        z: r.get(9)?,
                    })
                })
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect();

            // Semantic links
            let mut link_stmt = reader
                .prepare(
                    "SELECT source_artifact_id, target_artifact_id, relationship_type, origin
                     FROM artifact_links
                     WHERE source_artifact_id IN (SELECT artifact_id FROM canvas_items WHERE canvas_id = ?1 AND artifact_id IS NOT NULL)
                        OR target_artifact_id IN (SELECT artifact_id FROM canvas_items WHERE canvas_id = ?1 AND artifact_id IS NOT NULL);",
                )
                .map_err(|e| e.to_string())?;

            let links = link_stmt
                .query_map(params![cid], |r| {
                    Ok(ArtifactLinkView {
                        source_artifact_id: r.get(0)?,
                        target_artifact_id: r.get(1)?,
                        relationship_type: r.get(2)?,
                        origin: r.get(3)?,
                    })
                })
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect();

            Ok(CanvasOpsResponse {
                canvas: Some(CanvasDataView {
                    id: cid,
                    project_id: pid,
                    name,
                    items,
                    links,
                }),
                change_set_id: None,
                success: true,
            })
        }
        CanvasOpsRequest::MoveItem { canvas_id, item_id, x, y } => {
            engine
                .move_canvas_item(&canvas_id, &item_id, x, y)
                .map_err(|e| e.to_string())?;
            Ok(CanvasOpsResponse {
                canvas: None,
                change_set_id: None,
                success: true,
            })
        }
        CanvasOpsRequest::AddItem { canvas_id, item_kind, artifact_id, x, y, w, h } => {
            let item_id = uuid::Uuid::new_v4().to_string();
            let kind = match item_kind.as_str() {
                "artifact" => CanvasItemKind::Artifact,
                "frame" => CanvasItemKind::Frame,
                _ => CanvasItemKind::Text,
            };
            engine
                .add_canvas_item(&canvas_id, &item_id, kind, artifact_id.as_deref(), x, y, w, h, 0)
                .map_err(|e| e.to_string())?;
            Ok(CanvasOpsResponse {
                canvas: None,
                change_set_id: None,
                success: true,
            })
        }
        CanvasOpsRequest::BatchUpdatePositions { canvas_id, positions } => {
            let now_ms = engine.clock().now_ms();

            // Capture original positions for the inverse payload (before any changes)
            let mut inverse_ops: Vec<serde_json::Value> = Vec::new();
            {
                let reader = engine
                    .db()
                    .reader()
                    .map_err(|e| e.to_string())?;
                for pos in &positions {
                    let orig: Option<(f64, f64)> = reader
                        .query_row(
                            "SELECT x, y FROM canvas_items WHERE canvas_id = ?1 AND item_id = ?2;",
                            params![&canvas_id, &pos.item_id],
                            |r| Ok((r.get(0)?, r.get(1)?)),
                        )
                        .ok();
                    if let Some((orig_x, orig_y)) = orig {
                        inverse_ops.push(serde_json::json!({
                            "type": "restore_canvas_item",
                            "canvas_id": &canvas_id,
                            "item_id": &pos.item_id,
                            "x": orig_x,
                            "y": orig_y,
                        }));
                    }
                }
                // reader returned to pool here
            }

            // Create ChangeSet record so revert_change_set can find it later
            let cs_id = uuid::Uuid::new_v4().to_string();
            let inverse_payload = serde_json::json!({ "operations": inverse_ops });
            engine
                .apply_change_set(
                    &cs_id,
                    Actor::User,
                    None,
                    "Batch canvas item position update",
                    &inverse_payload,
                )
                .map_err(|e| e.to_string())?;

            // Apply the actual position updates and record the layout revision
            {
                let writer = engine.db().writer();
                for pos in &positions {
                    let _ = writer.execute(
                        "UPDATE canvas_items SET x = ?1, y = ?2 WHERE canvas_id = ?3 AND item_id = ?4;",
                        params![pos.x, pos.y, &canvas_id, &pos.item_id],
                    );
                }

                let rev_id = uuid::Uuid::new_v4().to_string();
                let layout_json = serde_json::to_string(&positions).unwrap_or_else(|_| "[]".to_string());
                let _ = writer.execute(
                    "INSERT INTO canvas_layout_revisions (id, canvas_id, layout, actor, change_set_id, created_at)
                     VALUES (?1, ?2, ?3, 'user', ?4, ?5);",
                    params![rev_id, &canvas_id, layout_json, &cs_id, now_ms],
                );
                // writer dropped here
            }

            Ok(CanvasOpsResponse {
                canvas: None,
                change_set_id: Some(cs_id),
                success: true,
            })
        }
        CanvasOpsRequest::CreateLink { source_artifact_id, target_artifact_id, relationship_type } => {
            let writer = engine.db().writer();
            let now_ms = engine.clock().now_ms();
            writer
                .execute(
                    "INSERT INTO artifact_links (source_artifact_id, target_artifact_id, relationship_type, origin, created_at)
                     VALUES (?1, ?2, ?3, 'user_recorded', ?4);",
                    params![source_artifact_id, target_artifact_id, relationship_type, now_ms],
                )
                .map_err(|e| e.to_string())?;

            Ok(CanvasOpsResponse {
                canvas: None,
                change_set_id: None,
                success: true,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri IPC Command Wrappers
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn cmd_route_input(
    raw: String,
    state: State<'_, AppState>,
) -> Result<RouterPlan, String> {
    handle_route_input(&state.engine, raw).await
}

#[tauri::command]
pub fn cmd_apply_plan(
    plan: RouterPlan,
    approved: bool,
    state: State<'_, AppState>,
) -> Result<ApplyOutcome, String> {
    handle_apply_plan(&state.engine, plan, approved)
}

#[tauri::command]
pub fn cmd_get_home_state(state: State<'_, AppState>) -> Result<HomeStateView, String> {
    handle_get_home_state(&state.engine)
}

#[tauri::command]
pub fn cmd_get_workspace_tree(state: State<'_, AppState>) -> Result<WorkspaceTree, String> {
    handle_get_workspace_tree(&state.engine)
}

#[tauri::command]
pub fn cmd_create_project(
    name: String,
    purpose: Option<String>,
    state: State<'_, AppState>,
) -> Result<WorkspaceTreeProject, String> {
    handle_create_project(&state.engine, name, purpose)
}

#[tauri::command]
pub fn cmd_create_page(
    project_id: Option<String>,
    title: Option<String>,
    state: State<'_, AppState>,
) -> Result<Artifact, String> {
    handle_create_page(&state.engine, project_id, title)
}

#[tauri::command]
pub async fn cmd_get_resume_brief(
    project_id: String,
    level: u8,
    state: State<'_, AppState>,
) -> Result<ResumeBrief, String> {
    handle_get_resume_brief(&state.engine, project_id, level).await
}

#[tauri::command]
pub fn cmd_get_resume_pane_hint(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<ResumePaneHint, String> {
    handle_get_resume_pane_hint(&state.engine, project_id)
}

#[tauri::command]
pub fn cmd_get_project_history(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<ProjectHistory, String> {
    handle_get_project_history(&state.engine, project_id)
}

#[tauri::command]
pub fn cmd_get_artifact_context(
    artifact_id: String,
    state: State<'_, AppState>,
) -> Result<ArtifactContext, String> {
    handle_get_artifact_context(&state.engine, artifact_id)
}

#[tauri::command]
pub fn cmd_save_artifact(
    artifact_id: String,
    content_md: String,
    title: Option<String>,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Artifact, String> {
    handle_save_artifact(&state.engine, artifact_id, content_md, title, project_id)
}

#[tauri::command]
pub fn cmd_decide_proposal(
    proposal_id: String,
    decision: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_decide_proposal(&state.engine, proposal_id, decision)
}

#[tauri::command]
pub fn cmd_revert_change_set(
    change_set_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_revert_change_set(&state.engine, change_set_id)
}

#[tauri::command]
pub fn cmd_promote_to_project(
    artifact_ids: Vec<String>,
    name: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_promote_to_project(&state.engine, artifact_ids, name)
}

#[tauri::command]
pub fn cmd_canvas_ops(
    req: CanvasOpsRequest,
    state: State<'_, AppState>,
) -> Result<CanvasOpsResponse, String> {
    handle_canvas_ops(&state.engine, req)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidedProjectResult {
    pub project_id: String,
    pub artifact_id: String,
}

pub fn handle_create_guided_project(
    engine: &Engine,
    name: String,
    purpose: String,
    initial_title: String,
    initial_content: String,
) -> Result<GuidedProjectResult, String> {
    let proj = engine
        .create_project(&name, None)
        .map_err(|e| e.to_string())?;

    let _ = engine.set_project_purpose(&proj.id, &purpose);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some(&initial_title),
            &initial_content,
            None,
        )
        .map_err(|e| e.to_string())?;

    let _ = engine.attach_artifact(&proj.id, &art.id, "user_explicit", None);

    Ok(GuidedProjectResult {
        project_id: proj.id,
        artifact_id: art.id,
    })
}

#[tauri::command]
pub fn cmd_create_guided_project(
    name: String,
    purpose: String,
    initial_title: String,
    initial_content: String,
    state: State<'_, AppState>,
) -> Result<GuidedProjectResult, String> {
    handle_create_guided_project(
        &state.engine,
        name,
        purpose,
        initial_title,
        initial_content,
    )
}

pub fn handle_clear_vault_data(engine: &Engine) -> Result<(), String> {
    let writer = engine.db().writer();
    let _ = writer.execute_batch(
        "DELETE FROM canvas_layout_revisions;
         DELETE FROM canvas_items;
         DELETE FROM canvases;
         DELETE FROM artifact_links;
         DELETE FROM ai_proposals;
         DELETE FROM epistemic_nodes;
         DELETE FROM derived_briefs;
         DELETE FROM session_events;
         DELETE FROM project_artifacts;
         DELETE FROM project_purposes;
         DELETE FROM artifacts;
         DELETE FROM projects;"
    );
    Ok(())
}

#[tauri::command]
pub fn cmd_clear_vault_data(state: State<'_, AppState>) -> Result<(), String> {
    handle_clear_vault_data(&state.engine)
}

pub fn handle_seed_demo_workspace(engine: &std::sync::Arc<Engine>) -> Result<(), String> {
    crate::seed::seed_demo_workspace_if_empty(engine).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_seed_demo_workspace(state: State<'_, AppState>) -> Result<(), String> {
    handle_seed_demo_workspace(&state.engine)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub excerpt: String,
    pub project_id: Option<String>,
}

pub fn handle_search_all(engine: &Engine, query: String) -> Result<Vec<SearchResultItem>, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    let pattern = format!("%{}%", query.trim());
    let mut results = Vec::new();

    if query.trim().is_empty() {
        return Ok(results);
    }

    // Search projects
    let mut proj_stmt = reader.prepare(
        "SELECT p.id, p.name, pp.purpose FROM projects p
         LEFT JOIN project_purposes pp ON p.id = pp.project_id AND pp.superseded_at IS NULL
         WHERE p.status = 'active' AND (p.name LIKE ?1 OR pp.purpose LIKE ?1) LIMIT 10;",
    ).map_err(|e| e.to_string())?;

    let projs = proj_stmt.query_map(params![pattern], |r| {
        Ok(SearchResultItem {
            id: r.get(0)?,
            title: r.get(1)?,
            kind: "project".to_string(),
            excerpt: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            project_id: None,
        })
    }).map_err(|e| e.to_string())?;

    for p in projs.filter_map(|r| r.ok()) {
        results.push(p);
    }

    // Search artifacts
    let mut art_stmt = reader.prepare(
        "SELECT a.id, a.title, SUBSTR(a.content_md, 1, 120), pa.project_id
         FROM artifacts a
         LEFT JOIN project_artifacts pa ON a.id = pa.artifact_id AND pa.detached_at IS NULL
         WHERE a.deleted_at IS NULL AND (a.title LIKE ?1 OR a.content_md LIKE ?1) LIMIT 15;",
    ).map_err(|e| e.to_string())?;

    let arts = art_stmt.query_map(params![pattern], |r| {
        Ok(SearchResultItem {
            id: r.get(0)?,
            title: r.get::<_, Option<String>>(1)?.unwrap_or_else(|| "Untitled Note".to_string()),
            kind: "document".to_string(),
            excerpt: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            project_id: r.get(3)?,
        })
    }).map_err(|e| e.to_string())?;

    for a in arts.filter_map(|r| r.ok()) {
        results.push(a);
    }

    Ok(results)
}

#[tauri::command]
pub fn cmd_search_all(query: String, state: State<'_, AppState>) -> Result<Vec<SearchResultItem>, String> {
    handle_search_all(&state.engine, query)
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchSectionSchema {
    pub heading: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResearchSynthesisSchema {
    pub title: String,
    pub purpose: String,
    pub document_title: String,
    pub sections: Vec<ResearchSectionSchema>,
    pub hypotheses: Vec<String>,
    pub open_questions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AnswerResponseSchema {
    pub answer: String,
}

pub async fn handle_synthesize_and_create_research(
    engine: &Engine,
    prompt: String,
    model_id: Option<String>,
) -> Result<GuidedProjectResult, String> {
    let schema = schemars::schema_for!(ResearchSynthesisSchema);
    let schema_val = serde_json::to_value(&schema).unwrap_or_default();
    let system_prompt = "You are an expert research scientist and systems architect. Synthesize a grounded, comprehensive research plan and technical document for the user's inquiry. Provide technical rigor, architectural trade-offs, and clear hypotheses.";
    let user_prompt = format!(
        "Research topic and inquiry: {}\nSynthesize into the structured JSON schema.",
        prompt
    );

    // Resolve model provider based on selected model or system default
    let (provider, _model_name) = {
        let reader = engine.db().reader().ok();
        match reader.as_ref().and_then(|r| sthira_core::llm::dispatcher::resolve_model_provider(r, model_id.as_deref(), engine.model()).ok()) {
            Some(resolved) => (resolved.provider, resolved.display_name),
            None => (std::sync::Arc::clone(engine.model()), "Default Model".to_string()),
        }
    };

    let synthesis_res: Option<ResearchSynthesisSchema> = match provider
        .generate_json(&schema_val, system_prompt, &user_prompt)
        .await
    {
        Ok(json_str) => serde_json::from_str(&json_str).ok(),
        Err(e) => {
            eprintln!("[handle_synthesize_and_create_research] LLM generation fallback: {}", e);
            None
        }
    };

    let (title, purpose, doc_title, content_md) = if let Some(syn) = synthesis_res {
        let question_title = prompt.trim().to_string();
        let mut md = format!("# {}\n\n## Purpose & Research Scope\n{}\n\n", question_title, syn.purpose);
        for sec in syn.sections {
            md.push_str(&format!("## {}\n{}\n\n", sec.heading, sec.content));
        }
        if !syn.hypotheses.is_empty() {
            md.push_str("## Epistemic Hypotheses\n");
            for h in syn.hypotheses {
                md.push_str(&format!("- [ ] {}\n", h));
            }
            md.push('\n');
        }
        if !syn.open_questions.is_empty() {
            md.push_str("## Open Questions & Next Steps\n");
            for q in syn.open_questions {
                md.push_str(&format!("- {}\n", q));
            }
            md.push('\n');
        }
        (syn.title, syn.purpose, question_title, md)
    } else {
        let clean_title = if prompt.len() > 60 {
            let truncated = prompt.chars().take(60).collect::<String>();
            format!("{}...", truncated)
        } else {
            prompt.trim().to_string()
        };
        let purpose = format!("Research and investigate: {}", prompt.trim());
        let doc_title = prompt.trim().to_string();
        let md = format!(
            "# {}\n\n## Research Inquiry\n{}\n\n## Investigation & Problem Scope\n- **Objective**: Conduct systematic investigation and structured synthesis for \"{}\".\n- **Key Dimensions**: Analyze underlying mechanisms, context, domain requirements, and relevant precedents.\n- **Evaluation Criteria**: Establish clear qualitative or quantitative criteria to validate findings.\n\n## Epistemic Hypotheses\n- [ ] Structured analysis of this inquiry yields actionable decision criteria and clear tradeoffs.\n- [ ] Edge cases and primary constraints can be systematically characterized through focused exploration.\n\n## Open Questions & Next Steps\n- What are the core assumptions and boundaries governing this topic?\n- What empirical evidence or additional context is required to reach a definitive conclusion?\n",
            doc_title, prompt.trim(), clean_title
        );
        (clean_title, purpose, doc_title, md)
    };

    let proj = engine
        .create_project(&title, None)
        .map_err(|e| e.to_string())?;

    let _ = engine.set_project_purpose(&proj.id, &purpose);

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some(&doc_title),
            &content_md,
            None,
        )
        .map_err(|e| e.to_string())?;

    let _ = engine.attach_artifact(&proj.id, &art.id, "user_explicit", None);
    let _ = engine.extract_epistemic_nodes(&art.id).await;

    Ok(GuidedProjectResult {
        project_id: proj.id,
        artifact_id: art.id,
    })
}

pub async fn handle_answer_question(
    engine: &Engine,
    question: String,
) -> Result<String, String> {
    let schema = schemars::schema_for!(AnswerResponseSchema);
    let schema_val = serde_json::to_value(&schema).unwrap_or_default();
    let system_prompt = "You are a concise, factual AI assistant. Answer the user's question directly and concisely in 2 to 3 sentences.";

    match engine
        .model()
        .generate_json(&schema_val, system_prompt, &question)
        .await
    {
        Ok(json_str) => {
            if let Ok(resp) = serde_json::from_str::<AnswerResponseSchema>(&json_str) {
                Ok(resp.answer)
            } else {
                Ok(json_str)
            }
        }
        Err(_) => {
            Ok(format!("Unable to reach AI model to answer \"{}\". Please ensure an inference provider (e.g. Ollama or cloud API) is configured.", question.trim()))
        }
    }
}

#[tauri::command]
pub async fn cmd_synthesize_and_create_research(
    prompt: String,
    model_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<GuidedProjectResult, String> {
    handle_synthesize_and_create_research(&state.engine, prompt, model_id).await
}

#[tauri::command]
pub async fn cmd_answer_question(
    question: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_answer_question(&state.engine, question).await
}

// ---------------------------------------------------------------------------
// Phase 9: Provider-Centric Registry & Intent Steering Commands
// ---------------------------------------------------------------------------

use sthira_core::{
    db::providers_repo,
    engines::router_engine::RouteOutcome,
    llm::autodiscover::{discover_local_ollama_models, discover_provider_models, sync_discovered_models},
    llm::models::WebSearchCapability,
    models::router::{PlanProposedAction, ProviderEntry, ProviderModelEntry},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelView {
    pub id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub provider_type: String,
    pub model_identifier: String,
    pub display_name: String,
    pub context_window: u32,
    pub supports_structured_output: bool,
    pub is_enabled: bool,
    pub is_default: bool,
    pub web_search_capability: WebSearchCapability,
}

pub fn handle_list_available_models(engine: &Engine) -> Result<Vec<ModelView>, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    let providers = providers_repo::list_providers_with_models(&reader).map_err(|e| e.to_string())?;
    let mut views = Vec::new();
    for p in providers {
        if p.is_enabled {
            for m in p.models {
                if m.is_enabled {
                    views.push(ModelView {
                        id: m.id,
                        provider_id: p.id.clone(),
                        provider_name: p.display_name.clone(),
                        provider_type: p.provider_type.clone(),
                        model_identifier: m.model_identifier,
                        display_name: m.display_name,
                        context_window: m.context_window,
                        supports_structured_output: m.supports_structured_output,
                        is_enabled: m.is_enabled,
                        is_default: m.is_default,
                        web_search_capability: m.web_search_capability,
                    });
                }
            }
        }
    }
    Ok(views)
}

#[tauri::command]
pub fn cmd_list_available_models(state: State<'_, AppState>) -> Result<Vec<ModelView>, String> {
    handle_list_available_models(&state.engine)
}

pub fn handle_update_model_search_capability(
    engine: &Engine,
    model_id: String,
    capability: WebSearchCapability,
) -> Result<(), String> {
    let writer = engine.db().writer();
    providers_repo::update_model_search_capability(&writer, &model_id, capability)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_update_model_search_capability(
    model_id: String,
    capability: WebSearchCapability,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_update_model_search_capability(&state.engine, model_id, capability)
}

pub fn handle_get_provider_catalog(engine: &Engine) -> Result<Vec<ProviderEntry>, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    providers_repo::list_providers_with_models(&reader).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_get_provider_catalog(state: State<'_, AppState>) -> Result<Vec<ProviderEntry>, String> {
    handle_get_provider_catalog(&state.engine)
}

pub fn handle_save_provider(
    engine: &Engine,
    entry: ProviderEntry,
    api_key: Option<String>,
) -> Result<(), String> {
    let writer = engine.db().writer();
    let now_ms = engine.clock().now_ms();
    providers_repo::upsert_provider(&writer, &entry, api_key.as_deref(), now_ms)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_save_provider(
    entry: ProviderEntry,
    api_key: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_save_provider(&state.engine, entry, api_key)
}

pub async fn handle_discover_ollama_models(
    engine: &Engine,
    endpoint: Option<String>,
) -> Result<Vec<ProviderModelEntry>, String> {
    let host = endpoint.unwrap_or_else(|| {
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string())
    });

    let models = discover_local_ollama_models(&host)
        .await
        .map_err(|e| e.to_string())?;

    {
        let writer = engine.db().writer();
        let now_ms = engine.clock().now_ms();
        sync_discovered_models(&writer, &models, now_ms).map_err(|e| e.to_string())?;
    }

    Ok(models)
}

#[tauri::command]
pub async fn cmd_discover_ollama_models(
    endpoint: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ProviderModelEntry>, String> {
    handle_discover_ollama_models(&state.engine, endpoint).await
}

pub async fn handle_fetch_provider_models(
    engine: &Engine,
    provider_id: String,
    endpoint: Option<String>,
    api_key: Option<String>,
) -> Result<Vec<ProviderModelEntry>, String> {
    let (prov_type, base_url, key_to_use) = {
        let reader = engine.db().reader().map_err(|e| e.to_string())?;
        let catalog = providers_repo::list_providers_with_models(&reader).map_err(|e| e.to_string())?;
        let prov = catalog.into_iter().find(|p| p.id == provider_id);
        let stored_key = providers_repo::get_provider_api_key(&reader, &provider_id).map_err(|e| e.to_string())?;

        let p_type = prov
            .as_ref()
            .map(|p| p.provider_type.clone())
            .unwrap_or_else(|| "openai_compatible".to_string());
        let url = endpoint.unwrap_or_else(|| {
            prov.as_ref()
                .map(|p| p.base_url.clone())
                .unwrap_or_else(|| "http://localhost:11434".to_string())
        });
        let key = api_key.or(stored_key);
        (p_type, url, key)
    };

    let models = discover_provider_models(&provider_id, &prov_type, &base_url, key_to_use.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    {
        let writer = engine.db().writer();
        let now_ms = engine.clock().now_ms();
        sync_discovered_models(&writer, &models, now_ms).map_err(|e| e.to_string())?;
    }

    Ok(models)
}

#[tauri::command]
pub async fn cmd_fetch_provider_models(
    provider_id: String,
    endpoint: Option<String>,
    api_key: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ProviderModelEntry>, String> {
    handle_fetch_provider_models(&state.engine, provider_id, endpoint, api_key).await
}

pub async fn handle_discover_provider_models(
    _engine: &Engine,
    provider_id: String,
    provider_type: String,
    endpoint: String,
    api_key: Option<String>,
) -> Result<Vec<ProviderModelEntry>, String> {
    discover_provider_models(&provider_id, &provider_type, &endpoint, api_key.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cmd_discover_provider_models(
    provider_id: String,
    provider_type: String,
    endpoint: String,
    api_key: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ProviderModelEntry>, String> {
    handle_discover_provider_models(&state.engine, provider_id, provider_type, endpoint, api_key).await
}

pub fn handle_upsert_model(engine: &Engine, model: ProviderModelEntry) -> Result<(), String> {
    let writer = engine.db().writer();
    let now_ms = engine.clock().now_ms();
    providers_repo::upsert_model(&writer, &model, now_ms).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_upsert_model(model: ProviderModelEntry, state: State<'_, AppState>) -> Result<(), String> {
    handle_upsert_model(&state.engine, model)
}

pub fn handle_delete_model(engine: &Engine, model_id: String) -> Result<(), String> {
    let writer = engine.db().writer();
    providers_repo::delete_model(&writer, &model_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_delete_model(model_id: String, state: State<'_, AppState>) -> Result<(), String> {
    handle_delete_model(&state.engine, model_id)
}

pub fn handle_delete_provider(engine: &Engine, provider_id: String) -> Result<(), String> {
    let writer = engine.db().writer();
    providers_repo::delete_provider(&writer, &provider_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_delete_provider(provider_id: String, state: State<'_, AppState>) -> Result<(), String> {
    handle_delete_provider(&state.engine, provider_id)
}

pub fn handle_set_default_model(engine: &Engine, model_id: String) -> Result<(), String> {
    let writer = engine.db().writer();
    providers_repo::set_default_model(&writer, &model_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_set_default_model(model_id: String, state: State<'_, AppState>) -> Result<(), String> {
    handle_set_default_model(&state.engine, model_id)
}

pub fn handle_toggle_model(
    engine: &Engine,
    model_id: String,
    enabled: bool,
) -> Result<(), String> {
    let writer = engine.db().writer();
    providers_repo::toggle_model(&writer, &model_id, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_toggle_model(
    model_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_toggle_model(&state.engine, model_id, enabled)
}

pub async fn handle_route_input_steered(
    engine: &Engine,
    raw: String,
    mode: Option<OperationalMode>,
    model_id: Option<String>,
) -> Result<RouteOutcome, String> {
    engine
        .route_input_steered(&raw, mode, model_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cmd_route_input_steered(
    raw: String,
    mode: Option<OperationalMode>,
    model_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<RouteOutcome, String> {
    handle_route_input_steered(&state.engine, raw, mode, model_id).await
}

pub fn handle_execute_plan_action(
    engine: &Engine,
    action: PlanProposedAction,
) -> Result<String, String> {
    engine.apply_plan_action(&action).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_execute_plan_action(
    action: PlanProposedAction,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_execute_plan_action(&state.engine, action)
}

// ---------------------------------------------------------------------------
// Phase 9.2: Extensible Typed Object System Commands
// ---------------------------------------------------------------------------

pub fn handle_list_object_types(engine: &Engine) -> Result<Vec<ObjectType>, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    sthira_core::db::objects_repo::list_object_types(&reader).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_list_object_types(state: State<'_, AppState>) -> Result<Vec<ObjectType>, String> {
    handle_list_object_types(&state.engine)
}

pub fn handle_create_object_type(engine: &Engine, type_def: ObjectType) -> Result<String, String> {
    let writer = engine.db().writer();
    let id = type_def.id.clone();
    sthira_core::db::objects_repo::upsert_object_type(&writer, &type_def).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn cmd_create_object_type(
    type_def: ObjectType,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_create_object_type(&state.engine, type_def)
}

pub fn handle_list_objects_by_type(
    engine: &Engine,
    type_id: String,
) -> Result<Vec<ObjectItem>, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    sthira_core::db::objects_repo::list_objects_by_type(&reader, &type_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_list_objects_by_type(
    type_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ObjectItem>, String> {
    handle_list_objects_by_type(&state.engine, type_id)
}

pub fn handle_save_object_instance(
    engine: &Engine,
    instance: ObjectInstance,
) -> Result<(), String> {
    let writer = engine.db().writer();
    sthira_core::db::objects_repo::upsert_object_instance(&writer, &instance)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_save_object_instance(
    instance: ObjectInstance,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_save_object_instance(&state.engine, instance)
}

pub fn handle_get_object_type_counts(
    engine: &Engine,
) -> Result<std::collections::HashMap<String, usize>, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    sthira_core::db::objects_repo::count_objects_by_type(&reader).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_get_object_type_counts(
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, usize>, String> {
    handle_get_object_type_counts(&state.engine)
}

pub fn handle_create_typed_object(
    engine: &Engine,
    object_type_id: String,
    title: String,
    properties: serde_json::Value,
    project_id: Option<String>,
) -> Result<ObjectItem, String> {
    let initial_content = format!("# {}\n\n", title);
    let art = engine
        .create_artifact(ArtifactType::Note, Authorship::User, Some(&title), &initial_content, None)
        .map_err(|e| e.to_string())?;

    if let Some(ref pid) = project_id {
        let _ = engine.attach_artifact(pid, &art.id, "user_explicit", None);
    }

    let now_ms = engine.clock().now_ms();
    let instance = ObjectInstance {
        artifact_id: art.id.clone(),
        object_type_id: object_type_id.clone(),
        properties: properties.clone(),
        updated_at: now_ms,
    };

    {
        let writer = engine.db().writer();
        sthira_core::db::objects_repo::upsert_object_instance(&writer, &instance)
            .map_err(|e| e.to_string())?;
    }

    Ok(ObjectItem {
        artifact_id: art.id,
        title,
        content_md: initial_content,
        object_type_id,
        properties,
        project_id,
        updated_at: now_ms,
    })
}

#[tauri::command]
pub fn cmd_create_typed_object(
    object_type_id: String,
    title: String,
    properties: serde_json::Value,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<ObjectItem, String> {
    handle_create_typed_object(&state.engine, object_type_id, title, properties, project_id)
}

// ---------------------------------------------------------------------------
// Phase 16: Object Type & Template Management Commands
// ---------------------------------------------------------------------------

/// Updates an existing object type's display metadata, property schema, and default
/// Markdown template body in place. Existing instance property values are preserved
/// because they are stored as a keyed JSON document in `object_instances`.
///
/// (`ObjectTypeDefinition` in the product spec maps 1:1 onto [`ObjectType`].)
pub fn handle_update_object_type(engine: &Engine, type_def: ObjectType) -> Result<(), String> {
    let writer = engine.db().writer();
    sthira_core::db::objects_repo::update_object_type_schema(
        &writer,
        &type_def.id,
        &type_def.display_name,
        &type_def.icon,
        &type_def.color,
        &type_def.schema,
        &type_def.default_template_md,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_update_object_type(
    type_def: ObjectType,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_update_object_type(&state.engine, type_def)
}

/// Deletes a user-created object type. Built-in system types are protected by the
/// core repository invariant and will return an error.
pub fn handle_delete_object_type(engine: &Engine, type_id: String) -> Result<(), String> {
    let writer = engine.db().writer();
    sthira_core::db::objects_repo::delete_object_type(&writer, &type_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_delete_object_type(
    type_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_delete_object_type(&state.engine, type_id)
}

pub fn handle_get_type_templates(
    engine: &Engine,
    type_id: String,
) -> Result<Vec<ObjectTemplate>, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    sthira_core::db::objects_repo::list_templates_for_type(&reader, &type_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_get_type_templates(
    type_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ObjectTemplate>, String> {
    handle_get_type_templates(&state.engine, type_id)
}

/// Persists a named template for an object type and returns its id (generated when
/// the caller passes an empty id, enabling "create" semantics with the same command).
pub fn handle_save_object_template(
    engine: &Engine,
    template: ObjectTemplate,
) -> Result<String, String> {
    let mut template = template;
    if template.id.trim().is_empty() {
        template.id = uuid::Uuid::new_v4().to_string();
    }
    if template.created_at == 0 {
        template.created_at = engine.clock().now_ms();
    }

    let id = template.id.clone();
    let writer = engine.db().writer();
    sthira_core::db::objects_repo::save_object_template(&writer, &template)
        .map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn cmd_save_object_template(
    template: ObjectTemplate,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_save_object_template(&state.engine, template)
}

pub fn handle_get_object_instance(
    engine: &Engine,
    artifact_id: String,
) -> Result<Option<ObjectInstance>, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    sthira_core::db::objects_repo::get_object_instance(&reader, &artifact_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_get_object_instance(
    artifact_id: String,
    state: State<'_, AppState>,
) -> Result<Option<ObjectInstance>, String> {
    handle_get_object_instance(&state.engine, artifact_id)
}

/// Creates the backing artifact for a typed object, hydrating it with the resolved
/// template's Markdown body and merged default property set. Returns the new artifact id.
///
/// Resolution order for the body/properties:
/// 1. explicit `template_id` (when it names a stored template),
/// 2. the `"blank"` sentinel, which yields a bare `# Title` skeleton,
/// 3. the object type's `default_template_md`,
/// 4. a bare `# Title` skeleton.
pub fn handle_instantiate_object_from_template(
    engine: &Engine,
    type_id: String,
    template_id: Option<String>,
    title: String,
    project_id: Option<String>,
) -> Result<String, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;

    let obj_type = sthira_core::db::objects_repo::get_object_type(&reader, &type_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Object type '{}' not found", type_id))?;

    let requested = template_id.as_deref().map(str::trim).unwrap_or("");
    let explicit_template = if requested.is_empty() || requested == "blank" {
        None
    } else {
        sthira_core::db::objects_repo::get_object_template(&reader, requested)
            .map_err(|e| e.to_string())?
    };
    drop(reader);

    let (body_md, preset) = if requested == "blank" {
        // Explicit "Blank" choice: a bare title-only page, ignoring the type default.
        (String::new(), serde_json::json!({}))
    } else {
        match explicit_template {
            Some(t) => (t.body_template_md, t.properties_preset),
            None => (obj_type.default_template_md.clone(), serde_json::json!({})),
        }
    };

    // Start from schema default values, then overlay the template property preset.
    let mut properties = serde_json::Map::new();
    for prop in &obj_type.schema {
        if let Some(def) = &prop.default_value {
            properties.insert(prop.key.clone(), def.clone());
        }
    }
    if let Some(preset_map) = preset.as_object() {
        for (k, v) in preset_map {
            properties.insert(k.clone(), v.clone());
        }
    }
    let properties = serde_json::Value::Object(properties);

    let trimmed_body = body_md.trim();
    let initial_content = if trimmed_body.is_empty() {
        format!("# {}\n\n", title)
    } else {
        format!("# {}\n\n{}\n", title, trimmed_body)
    };

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some(&title),
            &initial_content,
            None,
        )
        .map_err(|e| e.to_string())?;

    if let Some(ref pid) = project_id {
        let _ = engine.attach_artifact(pid, &art.id, "user_explicit", None);
    }

    let now_ms = engine.clock().now_ms();
    let instance = ObjectInstance {
        artifact_id: art.id.clone(),
        object_type_id: type_id,
        properties,
        updated_at: now_ms,
    };
    {
        let writer = engine.db().writer();
        sthira_core::db::objects_repo::upsert_object_instance(&writer, &instance)
            .map_err(|e| e.to_string())?;
    }

    Ok(art.id)
}

#[tauri::command]
pub fn cmd_instantiate_object_from_template(
    type_id: String,
    template_id: Option<String>,
    title: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_instantiate_object_from_template(&state.engine, type_id, template_id, title, project_id)
}

// ---------------------------------------------------------------------------
// Step 2 & 3: Safe Multi-Project Deletion Engine & Knowledge Graph Commands
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSummary {
    pub id: String,
    pub title: String,
    pub artifact_type: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDeletionImpact {
    pub project_id: String,
    pub project_name: String,
    pub exclusive_artifacts: Vec<ArtifactSummary>,
    pub shared_artifacts: Vec<ArtifactSummary>,
}

pub fn handle_inspect_project_deletion(
    engine: &Engine,
    project_id: String,
) -> Result<ProjectDeletionImpact, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;

    let project_name: String = reader
        .query_row(
            "SELECT name FROM projects WHERE id = ?1;",
            params![project_id],
            |r| r.get(0),
        )
        .map_err(|_| format!("Project {project_id} not found"))?;

    let mut stmt = reader
        .prepare(
            "SELECT a.id, COALESCE(a.title, 'Untitled Note'), a.type, a.updated_at,
                    (SELECT COUNT(*) FROM project_artifacts pa2 
                     WHERE pa2.artifact_id = a.id AND pa2.project_id <> ?1 AND pa2.detached_at IS NULL) AS other_count
             FROM artifacts a
             JOIN project_artifacts pa ON a.id = pa.artifact_id
             WHERE pa.project_id = ?1 AND pa.detached_at IS NULL AND a.deleted_at IS NULL
             ORDER BY a.updated_at DESC;",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![project_id], |r| {
            let id: String = r.get(0)?;
            let title: String = r.get(1)?;
            let artifact_type: String = r.get(2)?;
            let updated_at: i64 = r.get(3)?;
            let other_count: i64 = r.get(4)?;
            Ok((
                ArtifactSummary {
                    id,
                    title,
                    artifact_type,
                    updated_at,
                },
                other_count,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut exclusive_artifacts = Vec::new();
    let mut shared_artifacts = Vec::new();

    for r in rows {
        let (summary, other_count) = r.map_err(|e| e.to_string())?;
        if other_count > 0 {
            shared_artifacts.push(summary);
        } else {
            exclusive_artifacts.push(summary);
        }
    }

    Ok(ProjectDeletionImpact {
        project_id,
        project_name,
        exclusive_artifacts,
        shared_artifacts,
    })
}

#[tauri::command]
pub fn cmd_inspect_project_deletion(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<ProjectDeletionImpact, String> {
    handle_inspect_project_deletion(&state.engine, project_id)
}

pub fn handle_delete_project(
    engine: &Engine,
    project_id: String,
    cascade_exclusive: bool,
) -> Result<(), String> {
    let impact = handle_inspect_project_deletion(engine, project_id.clone())?;

    let writer = engine.db().writer();
    let _ = writer.execute_batch("PRAGMA foreign_keys = OFF;");

    if cascade_exclusive {
        for art in impact.exclusive_artifacts {
            let _ = writer.execute(
                "DELETE FROM project_artifacts WHERE artifact_id = ?1;",
                params![art.id],
            );
            let _ = writer.execute(
                "DELETE FROM artifact_links WHERE source_artifact_id = ?1 OR target_artifact_id = ?1;",
                params![art.id],
            );
            let _ = writer.execute(
                "DELETE FROM artifact_revisions WHERE artifact_id = ?1;",
                params![art.id],
            );
            let _ = writer.execute(
                "DELETE FROM object_instances WHERE artifact_id = ?1;",
                params![art.id],
            );
            let _ = writer.execute(
                "DELETE FROM epistemic_nodes WHERE artifact_id = ?1;",
                params![art.id],
            );
            let _ = writer.execute(
                "DELETE FROM canvas_items WHERE artifact_id = ?1;",
                params![art.id],
            );

            let file_path: Option<String> = writer
                .query_row(
                    "SELECT file_path FROM artifacts WHERE id = ?1;",
                    params![art.id],
                    |r| r.get(0),
                )
                .ok();

            let _ = writer.execute(
                "DELETE FROM artifacts WHERE id = ?1;",
                params![art.id],
            );

            if let Some(fp) = file_path {
                let full = engine.vault_dir().join(fp);
                let _ = std::fs::remove_file(full);
            }
        }
    }

    let _ = writer.execute(
        "DELETE FROM project_artifacts WHERE project_id = ?1;",
        params![project_id],
    );
    let _ = writer.execute(
        "DELETE FROM derived_briefs WHERE project_id = ?1;",
        params![project_id],
    );
    let _ = writer.execute(
        "DELETE FROM canvas_items WHERE canvas_id IN (SELECT id FROM canvases WHERE project_id = ?1);",
        params![project_id],
    );
    let _ = writer.execute(
        "DELETE FROM canvases WHERE project_id = ?1;",
        params![project_id],
    );
    let _ = writer.execute(
        "DELETE FROM project_purposes WHERE project_id = ?1;",
        params![project_id],
    );
    let _ = writer.execute(
        "UPDATE epistemic_nodes SET context_project_id = NULL WHERE context_project_id = ?1;",
        params![project_id],
    );
    let _ = writer.execute(
        "UPDATE events SET project_id = NULL WHERE project_id = ?1;",
        params![project_id],
    );
    let _ = writer.execute(
        "DELETE FROM projects WHERE id = ?1;",
        params![project_id],
    );

    let _ = writer.execute_batch("PRAGMA foreign_keys = ON;");

    Ok(())
}

#[tauri::command]
pub fn cmd_delete_project(
    project_id: String,
    cascade_exclusive: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_delete_project(&state.engine, project_id, cascade_exclusive)
}

pub fn handle_delete_artifact(
    engine: &Engine,
    artifact_id: String,
    from_project_id: Option<String>,
) -> Result<(), String> {
    let writer = engine.db().writer();
    let _ = writer.execute_batch("PRAGMA foreign_keys = OFF;");

    if let Some(ref pid) = from_project_id {
        let other_count: i64 = writer
            .query_row(
                "SELECT COUNT(*) FROM project_artifacts 
                 WHERE artifact_id = ?1 AND project_id <> ?2 AND detached_at IS NULL;",
                params![artifact_id, pid],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if other_count > 0 {
            let _ = writer.execute(
                "DELETE FROM project_artifacts WHERE project_id = ?1 AND artifact_id = ?2;",
                params![pid, artifact_id],
            );
            let _ = writer.execute_batch("PRAGMA foreign_keys = ON;");
            return Ok(());
        }
    }

    let _ = writer.execute(
        "DELETE FROM project_artifacts WHERE artifact_id = ?1;",
        params![artifact_id],
    );
    let _ = writer.execute(
        "DELETE FROM artifact_links WHERE source_artifact_id = ?1 OR target_artifact_id = ?1;",
        params![artifact_id],
    );
    let _ = writer.execute(
        "DELETE FROM artifact_revisions WHERE artifact_id = ?1;",
        params![artifact_id],
    );
    let _ = writer.execute(
        "DELETE FROM object_instances WHERE artifact_id = ?1;",
        params![artifact_id],
    );
    let _ = writer.execute(
        "DELETE FROM epistemic_nodes WHERE artifact_id = ?1;",
        params![artifact_id],
    );
    let _ = writer.execute(
        "DELETE FROM canvas_items WHERE artifact_id = ?1;",
        params![artifact_id],
    );

    let file_path: Option<String> = writer
        .query_row(
            "SELECT file_path FROM artifacts WHERE id = ?1;",
            params![artifact_id],
            |r| r.get(0),
        )
        .ok();

    let _ = writer.execute(
        "DELETE FROM artifacts WHERE id = ?1;",
        params![artifact_id],
    );

    if let Some(fp) = file_path {
        let full = engine.vault_dir().join(fp);
        let _ = std::fs::remove_file(full);
    }

    let _ = writer.execute_batch("PRAGMA foreign_keys = ON;");

    Ok(())
}

#[tauri::command]
pub fn cmd_delete_artifact(
    artifact_id: String,
    from_project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_delete_artifact(&state.engine, artifact_id, from_project_id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorship: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_status: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub relationship: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

pub fn handle_get_knowledge_graph(engine: &Engine) -> Result<KnowledgeGraphData, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    let mut proj_stmt = reader
        .prepare(
            "SELECT p.id, p.name,
                    (SELECT COUNT(*) FROM project_artifacts pa WHERE pa.project_id = p.id AND pa.detached_at IS NULL)
             FROM projects p
             ORDER BY p.name ASC;",
        )
        .map_err(|e| e.to_string())?;

    let proj_rows = proj_stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            let name: String = r.get(1)?;
            let count_i64: i64 = r.get(2)?;
            Ok(GraphNode {
                id,
                label: name,
                kind: "project".to_string(),
                item_count: Some(count_i64 as usize),
                authorship: None,
                draft_status: None,
                object_type: None,
                color: Some("#282C25".to_string()),
                icon: Some("📂".to_string()),
                project_id: None,
            })
        })
        .map_err(|e| e.to_string())?;

    for p in proj_rows {
        nodes.push(p.map_err(|e| e.to_string())?);
    }

    let mut art_stmt = reader
        .prepare(
            "SELECT a.id, COALESCE(a.title, 'Untitled Note'), a.authorship, a.is_unfinished_draft,
                    oi.object_type_id, ot.display_name, ot.color, ot.icon,
                    (SELECT pa.project_id FROM project_artifacts pa WHERE pa.artifact_id = a.id AND pa.detached_at IS NULL LIMIT 1)
             FROM artifacts a
             LEFT JOIN object_instances oi ON a.id = oi.artifact_id
             LEFT JOIN object_types ot ON oi.object_type_id = ot.id
             WHERE a.deleted_at IS NULL
             ORDER BY a.created_at ASC;",
        )
        .map_err(|e| e.to_string())?;

    let art_rows = art_stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            let title: String = r.get(1)?;
            let authorship: String = r.get(2)?;
            let is_draft: i64 = r.get(3)?;
            let obj_type_id: Option<String> = r.get(4)?;
            let _obj_type_name: Option<String> = r.get(5)?;
            let obj_color: Option<String> = r.get(6)?;
            let obj_icon: Option<String> = r.get(7)?;
            let primary_project_id: Option<String> = r.get(8)?;

            if let Some(type_id) = obj_type_id {
                Ok(GraphNode {
                    id,
                    label: title,
                    kind: "object".to_string(),
                    item_count: None,
                    authorship: Some(authorship),
                    draft_status: Some(is_draft == 1),
                    object_type: Some(type_id),
                    color: obj_color.or_else(|| Some("#6366F1".to_string())),
                    icon: obj_icon.or_else(|| Some("📦".to_string())),
                    project_id: primary_project_id,
                })
            } else {
                Ok(GraphNode {
                    id,
                    label: title,
                    kind: "artifact".to_string(),
                    item_count: None,
                    authorship: Some(authorship),
                    draft_status: Some(is_draft == 1),
                    object_type: None,
                    color: Some("#F5F3EC".to_string()),
                    icon: Some("📄".to_string()),
                    project_id: primary_project_id,
                })
            }
        })
        .map_err(|e| e.to_string())?;

    for a in art_rows {
        nodes.push(a.map_err(|e| e.to_string())?);
    }

    let mut mem_stmt = reader
        .prepare(
            "SELECT project_id, artifact_id 
             FROM project_artifacts 
             WHERE detached_at IS NULL;",
        )
        .map_err(|e| e.to_string())?;

    let mem_rows = mem_stmt
        .query_map([], |r| {
            let project_id: String = r.get(0)?;
            let artifact_id: String = r.get(1)?;
            Ok(GraphEdge {
                source: artifact_id,
                target: project_id,
                relationship: "belongs_to".to_string(),
            })
        })
        .map_err(|e| e.to_string())?;

    for m in mem_rows {
        edges.push(m.map_err(|e| e.to_string())?);
    }

    let mut link_stmt = reader
        .prepare(
            "SELECT source_artifact_id, target_artifact_id, relationship_type 
             FROM artifact_links;",
        )
        .map_err(|e| e.to_string())?;

    let link_rows = link_stmt
        .query_map([], |r| {
            let src: String = r.get(0)?;
            let dst: String = r.get(1)?;
            let rel: String = r.get(2)?;
            Ok(GraphEdge {
                source: src,
                target: dst,
                relationship: rel,
            })
        })
        .map_err(|e| e.to_string())?;

    for l in link_rows {
        edges.push(l.map_err(|e| e.to_string())?);
    }

    Ok(KnowledgeGraphData { nodes, edges })
}

#[tauri::command]
pub fn cmd_get_knowledge_graph(
    state: State<'_, AppState>,
) -> Result<KnowledgeGraphData, String> {
    handle_get_knowledge_graph(&state.engine)
}

/// Deep-copies an artifact row, assigns a fresh UUID + timestamp, sets the title
/// (defaulting to `"{original_title} (Copy)"`), and optionally attaches the copy to
/// `target_project_id`. Returns the **new artifact id**.
///
/// When `target_project_id` is `None` the copy inherits the source's first active
/// project association so duplicating from inside a project keeps it in that project.
pub fn handle_duplicate_artifact(
    engine: &Engine,
    artifact_id: String,
    new_title: Option<String>,
    target_project_id: Option<String>,
) -> Result<String, String> {
    let source = engine.get_artifact(&artifact_id).map_err(|e| e.to_string())?;
    let source_title = source.title.as_deref().unwrap_or("Untitled Note");
    let title = new_title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| format!("{} (Copy)", source_title));

    // `create_artifact` mints a fresh UUID, timestamps and vault file, and records the
    // initial revision + `artifact_created` session event.
    let new_art = engine
        .create_artifact(
            source.artifact_type,
            source.authorship,
            Some(&title),
            &source.content_md,
            source.source_url.as_deref(),
        )
        .map_err(|e| e.to_string())?;

    let project_to_attach = match target_project_id {
        Some(pid) if !pid.trim().is_empty() => Some(pid),
        _ => {
            let reader = engine.db().reader().map_err(|e| e.to_string())?;
            reader
                .query_row(
                    "SELECT project_id FROM project_artifacts WHERE artifact_id = ?1 AND detached_at IS NULL LIMIT 1;",
                    params![artifact_id],
                    |r| r.get(0),
                )
                .ok()
        }
    };

    if let Some(pid) = project_to_attach {
        let _ = engine.attach_artifact(&pid, &new_art.id, "user_explicit", None);
    }

    Ok(new_art.id)
}

#[tauri::command]
pub fn cmd_duplicate_artifact(
    artifact_id: String,
    new_title: Option<String>,
    target_project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_duplicate_artifact(&state.engine, artifact_id, new_title, target_project_id)
}

pub fn handle_add_artifact_to_project(
    engine: &Engine,
    artifact_id: String,
    project_id: String,
) -> Result<(), String> {
    engine
        .attach_artifact(&project_id, &artifact_id, "user_explicit", None)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn cmd_add_artifact_to_project(
    artifact_id: String,
    project_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_add_artifact_to_project(&state.engine, artifact_id, project_id)
}

// ---------------------------------------------------------------------------
// Phase 17: Page Action Menu Commands (Link / Move / Create-with / Convert / Detach)
// ---------------------------------------------------------------------------

/// Links an artifact to a project with **zero content duplication** — the artifact row
/// and its Markdown body are untouched; only a `project_artifacts` join row is added.
///
/// Idempotent: re-linking an already-linked artifact is a no-op (the existing
/// `attach_artifact` `ON CONFLICT ... detached_at = NULL` clause also un-detaches a
/// previously detached link).
pub fn handle_link_artifact_to_project(
    engine: &Engine,
    artifact_id: String,
    project_id: String,
) -> Result<(), String> {
    // Fail loudly if the artifact does not exist rather than writing a dangling link.
    engine.get_artifact(&artifact_id).map_err(|e| e.to_string())?;

    let already_linked: bool = {
        let reader = engine.db().reader().map_err(|e| e.to_string())?;
        reader
            .query_row(
                "SELECT 1 FROM project_artifacts
                 WHERE project_id = ?1 AND artifact_id = ?2 AND detached_at IS NULL;",
                params![project_id, artifact_id],
                |_| Ok(true),
            )
            .unwrap_or(false)
    };

    if already_linked {
        return Ok(());
    }

    engine
        .attach_artifact(&project_id, &artifact_id, "user_explicit", None)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn cmd_link_artifact_to_project(
    artifact_id: String,
    project_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_link_artifact_to_project(&state.engine, artifact_id, project_id)
}

/// Reassigns an artifact from `from_project_id` (optional) to `to_project_id` inside a
/// single SQLite transaction, emitting both audit events. See
/// `Engine::move_artifact_to_project` for why the transaction lives in the engine.
pub fn handle_move_artifact_to_project(
    engine: &Engine,
    artifact_id: String,
    from_project_id: Option<String>,
    to_project_id: String,
) -> Result<(), String> {
    engine.get_artifact(&artifact_id).map_err(|e| e.to_string())?;

    let from = from_project_id.filter(|p| !p.trim().is_empty());
    engine
        .move_artifact_to_project(&artifact_id, from.as_deref(), &to_project_id)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn cmd_move_artifact_to_project(
    artifact_id: String,
    from_project_id: Option<String>,
    to_project_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_move_artifact_to_project(&state.engine, artifact_id, from_project_id, to_project_id)
}

/// Creates a brand-new project and immediately associates `artifact_id` with it.
/// Returns the new project id so the caller can redirect context.
pub fn handle_create_project_with_artifact(
    engine: &Engine,
    project_name: String,
    artifact_id: String,
) -> Result<String, String> {
    let name = project_name.trim();
    if name.is_empty() {
        return Err("Project name cannot be empty".to_string());
    }
    engine.get_artifact(&artifact_id).map_err(|e| e.to_string())?;

    // `origin_artifact_id` records the note this project was born from (provenance).
    let project = engine
        .create_project(name, Some(&artifact_id))
        .map_err(|e| e.to_string())?;
    let _ = engine.set_project_purpose(&project.id, &format!("Project {name}"));
    engine
        .attach_artifact(&project.id, &artifact_id, "user_explicit", None)
        .map_err(|e| e.to_string())?;

    Ok(project.id)
}

#[tauri::command]
pub fn cmd_create_project_with_artifact(
    project_name: String,
    artifact_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_create_project_with_artifact(&state.engine, project_name, artifact_id)
}

/// Registers an existing artifact as an instance of `object_type_id`.
///
/// **The artifact's Markdown body is never touched** — `artifacts.content_md`,
/// `content_hash`, `version` and its vault file all stay byte-identical, so no epistemic
/// re-extraction or authorship laundering can occur. Only an `object_instances` row is
/// written.
///
/// Property resolution (lowest → highest precedence):
/// `object_types.schema_json` default values → `initial_properties` → any property
/// values already stored on the instance (so re-converting never destroys data).
pub fn handle_convert_artifact_to_object(
    engine: &Engine,
    artifact_id: String,
    object_type_id: String,
    initial_properties: Option<serde_json::Value>,
) -> Result<(), String> {
    engine.get_artifact(&artifact_id).map_err(|e| e.to_string())?;

    let (obj_type, existing) = {
        let reader = engine.db().reader().map_err(|e| e.to_string())?;
        let obj_type = sthira_core::db::objects_repo::get_object_type(&reader, &object_type_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Object type '{}' not found", object_type_id))?;
        let existing = sthira_core::db::objects_repo::get_object_instance(&reader, &artifact_id)
            .map_err(|e| e.to_string())?;
        (obj_type, existing)
    };

    let mut properties = serde_json::Map::new();
    for prop in &obj_type.schema {
        if let Some(def) = &prop.default_value {
            properties.insert(prop.key.clone(), def.clone());
        }
    }
    if let Some(init) = initial_properties.as_ref().and_then(|v| v.as_object()) {
        for (k, v) in init {
            properties.insert(k.clone(), v.clone());
        }
    }
    if let Some(prev) = existing.as_ref().and_then(|i| i.properties.as_object()) {
        for (k, v) in prev {
            properties.insert(k.clone(), v.clone());
        }
    }

    let now_ms = engine.clock().now_ms();
    let instance = ObjectInstance {
        artifact_id: artifact_id.clone(),
        object_type_id,
        properties: serde_json::Value::Object(properties),
        updated_at: now_ms,
    };

    let writer = engine.db().writer();
    sthira_core::db::objects_repo::upsert_object_instance(&writer, &instance)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn cmd_convert_artifact_to_object(
    artifact_id: String,
    object_type_id: String,
    initial_properties: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_convert_artifact_to_object(
        &state.engine,
        artifact_id,
        object_type_id,
        initial_properties,
    )
}

/// Detaches an artifact from a project, returning it to the Standalone Notes pool.
///
/// **Soft detach by design:** `project_artifacts.detached_at` is stamped rather than the
/// row being deleted, because the schema documents detach as "preserves historical
/// context" and the resume-brief / project-history / reversibility engines all read the
/// `artifact_detached` session event. A hard `DELETE` would silently rewrite history and
/// break `tests/frozen/phase9_modes_registry.rs`. The link stays re-attachable via
/// `attach_artifact`'s `ON CONFLICT ... detached_at = NULL`.
pub fn handle_detach_artifact_from_project(
    engine: &Engine,
    artifact_id: String,
    project_id: String,
) -> Result<(), String> {
    let is_linked: bool = {
        let reader = engine.db().reader().map_err(|e| e.to_string())?;
        reader
            .query_row(
                "SELECT 1 FROM project_artifacts
                 WHERE project_id = ?1 AND artifact_id = ?2 AND detached_at IS NULL;",
                params![project_id, artifact_id],
                |_| Ok(true),
            )
            .unwrap_or(false)
    };

    if !is_linked {
        return Err(format!(
            "Note is not actively linked to project '{}'",
            project_id
        ));
    }

    engine
        .detach_artifact(&project_id, &artifact_id)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn cmd_detach_artifact_from_project(
    artifact_id: String,
    project_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_detach_artifact_from_project(&state.engine, artifact_id, project_id)
}

pub fn handle_duplicate_project(
    engine: &Engine,
    project_id: String,
) -> Result<WorkspaceTreeProject, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    let (name, purpose): (String, Option<String>) = reader
        .query_row(
            "SELECT p.name, pp.purpose FROM projects p
             LEFT JOIN project_purposes pp ON p.id = pp.project_id AND pp.superseded_at IS NULL
             WHERE p.id = ?1;",
            params![project_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let new_name = format!("{} (Copy)", name);
    let new_proj = engine
        .create_project(&new_name, purpose.as_deref())
        .map_err(|e| e.to_string())?;

    let mut page_stmt = reader
        .prepare(
            "SELECT artifact_id FROM project_artifacts WHERE project_id = ?1 AND detached_at IS NULL;",
        )
        .map_err(|e| e.to_string())?;

    let page_ids: Vec<String> = page_stmt
        .query_map(params![project_id], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    for aid in page_ids {
        let _ = engine.attach_artifact(&new_proj.id, &aid, "user_explicit", None);
    }

    Ok(WorkspaceTreeProject {
        id: new_proj.id,
        name: new_name,
        purpose: purpose.unwrap_or_default(),
        pages: vec![],
        canvases: vec![],
        updated_at: new_proj.created_at,
    })
}

#[tauri::command]
pub fn cmd_duplicate_project(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<WorkspaceTreeProject, String> {
    handle_duplicate_project(&state.engine, project_id)
}

pub fn handle_rename_project(
    engine: &Engine,
    project_id: String,
    new_name: String,
) -> Result<(), String> {
    let writer = engine.db().writer();
    writer
        .execute(
            "UPDATE projects SET name = ?1, updated_at = unixepoch() WHERE id = ?2;",
            params![new_name.trim(), project_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn cmd_rename_project(
    project_id: String,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_rename_project(&state.engine, project_id, new_name)
}

pub fn handle_rename_artifact(
    engine: &Engine,
    artifact_id: String,
    new_title: String,
) -> Result<(), String> {
    let trimmed = new_title.trim();
    if trimmed.is_empty() {
        return Err("Title cannot be empty".to_string());
    }
    let now_ms = engine.clock().now_ms();

    // Check existing content and file_path
    let existing: Option<(String, Option<String>)> = if let Ok(reader) = engine.db().reader() {
        reader
            .query_row(
                "SELECT content_md, file_path FROM artifacts WHERE id = ?1;",
                params![artifact_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok()
    } else {
        None
    };

    let mut new_content_opt = None;
    if let Some((content_md, _)) = existing {
        if let Some(first_line) = content_md.lines().next() {
            if first_line.trim().starts_with("# ") {
                let rest = if let Some(pos) = content_md.find('\n') {
                    &content_md[pos..]
                } else {
                    ""
                };
                new_content_opt = Some(format!("# {}{}", trimmed, rest));
            }
        }
    }

    {
        let writer = engine.db().writer();
        if let Some(ref new_c) = new_content_opt {
            let normalized = sthira_core::vault::normalize_content(new_c);
            let new_hash = sthira_core::vault::compute_content_hash(&normalized);
            writer
                .execute(
                    "UPDATE artifacts SET title = ?1, content_md = ?2, content_hash = ?3, updated_at = ?4 WHERE id = ?5;",
                    params![trimmed, normalized, new_hash, now_ms, artifact_id],
                )
                .map_err(|e| e.to_string())?;
        } else {
            writer
                .execute(
                    "UPDATE artifacts SET title = ?1, updated_at = ?2 WHERE id = ?3;",
                    params![trimmed, now_ms, artifact_id],
                )
                .map_err(|e| e.to_string())?;
        }
    }

    if let Ok(art) = engine.get_artifact(&artifact_id) {
        if let Some(ref fp) = art.file_path {
            let full_path = engine.vault_dir().join(fp);
            let rendered = sthira_core::vault::render_file_with_frontmatter(&art);
            let _ = std::fs::write(&full_path, rendered);
        }
    }

    Ok(())
}

#[tauri::command]
pub fn cmd_rename_artifact(
    artifact_id: String,
    new_title: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_rename_artifact(&state.engine, artifact_id, new_title)
}

// ============================================================================
// Phase 10: Pluggable Web Search Providers & Evidence Extraction
// ============================================================================

pub fn handle_get_search_providers(
    engine: &Engine,
) -> Result<Vec<sthira_core::search::SearchProviderEntry>, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    sthira_core::db::search_repo::list_search_providers(&reader).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_get_search_providers(
    state: State<'_, AppState>,
) -> Result<Vec<sthira_core::search::SearchProviderEntry>, String> {
    handle_get_search_providers(&state.engine)
}

pub fn handle_save_search_provider(
    engine: &Engine,
    entry: sthira_core::search::SearchProviderEntry,
) -> Result<(), String> {
    let writer = engine.db().writer();
    let now_ms = engine.clock().now_ms();
    sthira_core::db::search_repo::save_search_provider(&writer, &entry, now_ms).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_save_search_provider(
    entry: sthira_core::search::SearchProviderEntry,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_save_search_provider(&state.engine, entry)
}

pub async fn handle_test_search_provider(
    engine: &Engine,
    provider_id: String,
    query: String,
) -> Result<Vec<sthira_core::search::SearchResultItem>, String> {
    let provider_entry = {
        let reader = engine.db().reader().map_err(|e| e.to_string())?;
        let providers = sthira_core::db::search_repo::list_search_providers(&reader)
            .map_err(|e| e.to_string())?;
        providers.into_iter().find(|p| p.id == provider_id)
    };

    let entry = match provider_entry {
        Some(e) => e,
        None => match provider_id.as_str() {
            "searxng" => sthira_core::search::SearchProviderEntry {
                id: "searxng".to_string(),
                display_name: "SearXNG (Local)".to_string(),
                provider_type: "searxng".to_string(),
                endpoint_url: "http://localhost:8080".to_string(),
                api_key: None,
                is_enabled: true,
                is_default: false,
            },
            "brave" => sthira_core::search::SearchProviderEntry {
                id: "brave".to_string(),
                display_name: "Brave Search".to_string(),
                provider_type: "brave".to_string(),
                endpoint_url: "https://api.search.brave.com/res/v1/web/search".to_string(),
                api_key: None,
                is_enabled: true,
                is_default: false,
            },
            "tavily" => sthira_core::search::SearchProviderEntry {
                id: "tavily".to_string(),
                display_name: "Tavily Search".to_string(),
                provider_type: "tavily".to_string(),
                endpoint_url: "https://api.tavily.com/search".to_string(),
                api_key: None,
                is_enabled: true,
                is_default: false,
            },
            _ => sthira_core::search::SearchProviderEntry {
                id: "duckduckgo".to_string(),
                display_name: "DuckDuckGo".to_string(),
                provider_type: "duckduckgo_scraper".to_string(),
                endpoint_url: "https://html.duckduckgo.com/html/".to_string(),
                api_key: None,
                is_enabled: true,
                is_default: true,
            },
        },
    };

    let provider = sthira_core::search::build_search_provider(&entry);
    provider.search(&query, 5).await
}

#[tauri::command]
pub async fn cmd_test_search_provider(
    provider_id: String,
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<sthira_core::search::SearchResultItem>, String> {
    handle_test_search_provider(&state.engine, provider_id, query).await
}

pub fn handle_import_web_source_to_vault(
    engine: &Engine,
    url: String,
    title: String,
    content_md: String,
    project_id: Option<String>,
) -> Result<String, String> {
    let art = engine
        .create_artifact(
            sthira_core::ArtifactType::Source,
            sthira_core::Authorship::External,
            Some(&title),
            &content_md,
            Some(&url),
        )
        .map_err(|e| e.to_string())?;

    if let Some(ref pid) = project_id {
        if pid != "default_project" && !pid.is_empty() {
            let _ = engine.attach_artifact(pid, &art.id, "user_explicit", None);
        }
    }

    if let Some(ref fp) = art.file_path {
        let full_path = engine.vault_dir().join(fp);
        let rendered = sthira_core::vault::render_file_with_frontmatter(&art);
        let _ = std::fs::write(&full_path, rendered);
    }

    Ok(art.id)
}

#[tauri::command]
pub fn cmd_import_web_source_to_vault(
    url: String,
    title: String,
    content_md: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_import_web_source_to_vault(&state.engine, url, title, content_md, project_id)
}

pub async fn handle_execute_web_research(
    engine: &Engine,
    query: String,
    _model_id: Option<String>,
) -> Result<sthira_core::engines::research_engine::WebResearchResult, String> {
    let active_provider = {
        let reader = engine.db().reader().map_err(|e| e.to_string())?;
        sthira_core::db::search_repo::get_default_search_provider(&reader)
            .ok()
            .flatten()
    };

    let entry = active_provider.unwrap_or_else(|| sthira_core::search::SearchProviderEntry {
        id: "duckduckgo".to_string(),
        display_name: "DuckDuckGo".to_string(),
        provider_type: "duckduckgo_scraper".to_string(),
        endpoint_url: "https://html.duckduckgo.com/html/".to_string(),
        api_key: None,
        is_enabled: true,
        is_default: true,
    });

    let provider = sthira_core::search::build_search_provider(&entry);
    sthira_core::engines::research_engine::execute_multi_query_research(
        engine,
        &query,
        provider.as_ref(),
        3,
        None,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cmd_execute_web_research(
    query: String,
    model_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<sthira_core::engines::research_engine::WebResearchResult, String> {
    handle_execute_web_research(&state.engine, query, model_id).await
}

// ---------------------------------------------------------------------------
// Phase 19: Center-Workspace Research Pipeline
// ---------------------------------------------------------------------------

/// Creates the **draft research artifact** that will host a research run.
///
/// Returns the new `artifact_id` immediately so the UI can switch the middle
/// workspace to `FocusView` and watch the pipeline fill the page in — research results
/// are never trapped inside the narrow right rail.
///
/// Authorship is `External` (Phase 10 convention: web-derived research is tagged
/// `Authorship::External`, which is what allows grounded quotes to reach
/// `Provenance::SourceBacked`). `is_unfinished_draft` is forced to `1` because no
/// evidence has been gathered yet.
pub async fn handle_create_research_page(
    engine: &Engine,
    query: String,
    project_id: Option<String>,
) -> Result<String, String> {
    let q = query.trim();
    if q.is_empty() {
        return Err("Research query cannot be empty".to_string());
    }

    let title = sthira_core::engines::research_engine::generate_smart_title(
        Some(engine.model().as_ref()),
        q,
    )
    .await;

    let initial = format!(
        "# {title}\n\n> **Status:** draft — researching…\n\n## Research Inquiry\n{q}\n\n\
         _Evidence gathering in progress. Sources and grounded findings will appear here._\n"
    );

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::External,
            Some(&title),
            &initial,
            None,
        )
        .map_err(|e| e.to_string())?;

    {
        let writer = engine.db().writer();
        let _ = writer.execute(
            "UPDATE artifacts SET is_unfinished_draft = 1 WHERE id = ?1;",
            params![art.id],
        );
    }

    if let Some(pid) = project_id.filter(|p| !p.trim().is_empty()) {
        let _ = engine.attach_artifact(&pid, &art.id, "user_explicit", None);
    }

    Ok(art.id)
}

#[tauri::command]
pub async fn cmd_create_research_page(
    query: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_create_research_page(&state.engine, query, project_id).await
}

/// Runs the autonomous research pipeline and writes the grounded document **into the
/// existing artifact**, streaming real progress stages to the caller over a Tauri
/// `Channel` so `FocusView` can render a live banner.
///
/// No URL is ever required — the pipeline generates its own search queries, dispatches
/// them to the configured `SearchProvider`, scrapes the top documents, grounds quotes
/// with `VerifiedQuote::verify`, and synthesizes with an explicit temporal instruction.
pub async fn handle_run_research_into_page(
    engine: &Engine,
    artifact_id: String,
    query: String,
    web_search_mode: Option<String>,
    on_progress: tauri::ipc::Channel<sthira_core::engines::research_engine::ResearchStage>,
) -> Result<sthira_core::engines::research_engine::WebResearchResult, String> {
    use sthira_core::engines::research_engine::{execute_research, WebSearchMode};

    // Verify the target page exists before doing any expensive work.
    engine.get_artifact(&artifact_id).map_err(|e| e.to_string())?;

    let mode = web_search_mode
        .as_deref()
        .map(WebSearchMode::from_str_lossy)
        .unwrap_or(WebSearchMode::Auto);

    let entry = {
        let reader = engine.db().reader().map_err(|e| e.to_string())?;
        sthira_core::db::search_repo::get_default_search_provider(&reader)
            .ok()
            .flatten()
            .unwrap_or_else(|| sthira_core::search::SearchProviderEntry {
                id: "duckduckgo".to_string(),
                display_name: "DuckDuckGo".to_string(),
                provider_type: "duckduckgo_scraper".to_string(),
                endpoint_url: "https://html.duckduckgo.com/html/".to_string(),
                api_key: None,
                is_enabled: true,
                is_default: true,
            })
    };
    let provider_label = entry.display_name.clone();
    let provider = sthira_core::search::build_search_provider(&entry);

    // Forward every real pipeline stage to the UI as it happens.
    let progress = move |stage: sthira_core::engines::research_engine::ResearchStage| {
        let _ = on_progress.send(stage);
    };

    let result = execute_research(
        engine,
        query.trim(),
        mode,
        Some(provider.as_ref()),
        &provider_label,
        4,
        Some(&progress),
    )
    .await
    .map_err(|e| e.to_string())?;

    // Fill the page in with the grounded document and clear the draft flag.
    engine
        .update_artifact(&artifact_id, &result.synthesized_markdown, Actor::User, None)
        .map_err(|e| e.to_string())?;

    {
        let writer = engine.db().writer();
        let _ = writer.execute(
            "UPDATE artifacts SET title = ?1, is_unfinished_draft = 0 WHERE id = ?2;",
            params![result.title, artifact_id],
        );
    }

    engine.emit_event(sthira_core::CoreEvent::ArtifactChanged {
        id: artifact_id,
    });

    Ok(result)
}

#[tauri::command]
pub async fn cmd_run_research_into_page(
    artifact_id: String,
    query: String,
    web_search_mode: Option<String>,
    on_progress: tauri::ipc::Channel<sthira_core::engines::research_engine::ResearchStage>,
    state: State<'_, AppState>,
) -> Result<sthira_core::engines::research_engine::WebResearchResult, String> {
    handle_run_research_into_page(
        &state.engine,
        artifact_id,
        query,
        web_search_mode,
        on_progress,
    )
    .await
}

pub async fn handle_stream_adaptive_run(
    engine: &Engine,
    raw_input: String,
    mode: OperationalMode,
    model_id: Option<String>,
    on_chunk: tauri::ipc::Channel<ProgressiveStreamChunk>,
) -> Result<(), String> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ProgressiveStreamChunk>(64);
    let engine_clone = engine.clone();

    let producer_task = tokio::spawn(async move {
        sthira_core::engines::streaming_engine::execute_adaptive_stream(
            &engine_clone,
            &raw_input,
            mode,
            model_id,
            tx,
        )
        .await
    });

    while let Some(chunk) = rx.recv().await {
        if let Err(e) = on_chunk.send(chunk) {
            eprintln!("Failed to send chunk over Tauri IPC channel: {e}");
            break;
        }
    }

    producer_task
        .await
        .map_err(|e| format!("Streaming task join error: {e}"))?
}

#[tauri::command]
pub async fn cmd_stream_adaptive_run(
    raw_input: String,
    mode: OperationalMode,
    model_id: Option<String>,
    on_chunk: tauri::ipc::Channel<ProgressiveStreamChunk>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_stream_adaptive_run(&state.engine, raw_input, mode, model_id, on_chunk).await
}

// ============================================================================
// Workspace Organization: Sidebar & Workspace Index
// ============================================================================

pub fn handle_get_sidebar_items(engine: &Engine) -> Result<SidebarItemsResponse, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;

    // 1. Projects: Top 7 (max 5 pinned + most recent)
    let mut pinned_proj_stmt = reader
        .prepare(
            "SELECT p.id, p.name, p.is_pinned, p.last_accessed_at, p.last_substantive_at
             FROM projects p
             WHERE p.status <> 'archived' AND p.is_pinned = 1
             ORDER BY p.last_accessed_at DESC, p.last_substantive_at DESC
             LIMIT 5;",
        )
        .map_err(|e| e.to_string())?;

    let mut projects: Vec<SidebarItem> = pinned_proj_stmt
        .query_map([], |r| {
            Ok(SidebarItem {
                id: r.get(0)?,
                title: r.get(1)?,
                item_type: "project".to_string(),
                is_pinned: r.get::<_, i64>(2)? != 0,
                last_accessed_at: r.get(3)?,
                updated_at: r.get(4)?,
                project_id: None,
                project_name: None,
                object_type_id: None,
                object_type_name: None,
                icon: Some("folder".to_string()),
                color: None,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let unpinned_proj_limit = 7usize.saturating_sub(projects.len());
    if unpinned_proj_limit > 0 {
        let mut unpinned_proj_stmt = reader
            .prepare(
                "SELECT p.id, p.name, p.is_pinned, p.last_accessed_at, p.last_substantive_at
                 FROM projects p
                 WHERE p.status <> 'archived' AND p.is_pinned = 0
                 ORDER BY p.last_accessed_at DESC, p.last_substantive_at DESC
                 LIMIT ?1;",
            )
            .map_err(|e| e.to_string())?;

        let unpinned: Vec<SidebarItem> = unpinned_proj_stmt
            .query_map(params![unpinned_proj_limit as i64], |r| {
                Ok(SidebarItem {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    item_type: "project".to_string(),
                    is_pinned: r.get::<_, i64>(2)? != 0,
                    last_accessed_at: r.get(3)?,
                    updated_at: r.get(4)?,
                    project_id: None,
                    project_name: None,
                    object_type_id: None,
                    object_type_name: None,
                    icon: Some("folder".to_string()),
                    color: None,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        projects.extend(unpinned);
    }

    // 2. Notes: Top 7 (max 5 pinned + most recent)
    let mut pinned_note_stmt = reader
        .prepare(
            "SELECT a.id, COALESCE(a.title, 'Untitled Note'), a.is_pinned, a.last_accessed_at, a.updated_at,
                    pa.project_id, p.name
             FROM artifacts a
             LEFT JOIN project_artifacts pa ON a.id = pa.artifact_id AND pa.detached_at IS NULL
             LEFT JOIN projects p ON pa.project_id = p.id
             WHERE a.deleted_at IS NULL
               AND a.type = 'note'
               AND a.id NOT IN (SELECT artifact_id FROM object_instances)
               AND a.is_pinned = 1
             ORDER BY a.last_accessed_at DESC, a.updated_at DESC
             LIMIT 5;",
        )
        .map_err(|e| e.to_string())?;

    let mut notes: Vec<SidebarItem> = pinned_note_stmt
        .query_map([], |r| {
            Ok(SidebarItem {
                id: r.get(0)?,
                title: r.get(1)?,
                item_type: "note".to_string(),
                is_pinned: r.get::<_, i64>(2)? != 0,
                last_accessed_at: r.get(3)?,
                updated_at: r.get(4)?,
                project_id: r.get(5)?,
                project_name: r.get(6)?,
                object_type_id: None,
                object_type_name: None,
                icon: Some("file-text".to_string()),
                color: None,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let unpinned_note_limit = 7usize.saturating_sub(notes.len());
    if unpinned_note_limit > 0 {
        let mut unpinned_note_stmt = reader
            .prepare(
                "SELECT a.id, COALESCE(a.title, 'Untitled Note'), a.is_pinned, a.last_accessed_at, a.updated_at,
                        pa.project_id, p.name
                 FROM artifacts a
                 LEFT JOIN project_artifacts pa ON a.id = pa.artifact_id AND pa.detached_at IS NULL
                 LEFT JOIN projects p ON pa.project_id = p.id
                 WHERE a.deleted_at IS NULL
                   AND a.type = 'note'
                   AND a.id NOT IN (SELECT artifact_id FROM object_instances)
                   AND a.is_pinned = 0
                 ORDER BY a.last_accessed_at DESC, a.updated_at DESC
                 LIMIT ?1;",
            )
            .map_err(|e| e.to_string())?;

        let unpinned: Vec<SidebarItem> = unpinned_note_stmt
            .query_map(params![unpinned_note_limit as i64], |r| {
                Ok(SidebarItem {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    item_type: "note".to_string(),
                    is_pinned: r.get::<_, i64>(2)? != 0,
                    last_accessed_at: r.get(3)?,
                    updated_at: r.get(4)?,
                    project_id: r.get(5)?,
                    project_name: r.get(6)?,
                    object_type_id: None,
                    object_type_name: None,
                    icon: Some("file-text".to_string()),
                    color: None,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        notes.extend(unpinned);
    }

    // 3. Custom Objects: Top 7 (max 5 pinned + most recent)
    let mut pinned_obj_stmt = reader
        .prepare(
            "SELECT a.id, COALESCE(a.title, 'Untitled Object'), a.is_pinned, a.last_accessed_at, a.updated_at,
                    pa.project_id, p.name, ot.id, ot.display_name, ot.icon, ot.color
             FROM artifacts a
             JOIN object_instances oi ON a.id = oi.artifact_id
             JOIN object_types ot ON oi.object_type_id = ot.id
             LEFT JOIN project_artifacts pa ON a.id = pa.artifact_id AND pa.detached_at IS NULL
             LEFT JOIN projects p ON pa.project_id = p.id
             WHERE a.deleted_at IS NULL
               AND a.is_pinned = 1
             ORDER BY a.last_accessed_at DESC, a.updated_at DESC
             LIMIT 5;",
        )
        .map_err(|e| e.to_string())?;

    let mut objects: Vec<SidebarItem> = pinned_obj_stmt
        .query_map([], |r| {
            Ok(SidebarItem {
                id: r.get(0)?,
                title: r.get(1)?,
                item_type: "object".to_string(),
                is_pinned: r.get::<_, i64>(2)? != 0,
                last_accessed_at: r.get(3)?,
                updated_at: r.get(4)?,
                project_id: r.get(5)?,
                project_name: r.get(6)?,
                object_type_id: Some(r.get(7)?),
                object_type_name: Some(r.get(8)?),
                icon: Some(r.get(9)?),
                color: Some(r.get(10)?),
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let unpinned_obj_limit = 7usize.saturating_sub(objects.len());
    if unpinned_obj_limit > 0 {
        let mut unpinned_obj_stmt = reader
            .prepare(
                "SELECT a.id, COALESCE(a.title, 'Untitled Object'), a.is_pinned, a.last_accessed_at, a.updated_at,
                        pa.project_id, p.name, ot.id, ot.display_name, ot.icon, ot.color
                 FROM artifacts a
                 JOIN object_instances oi ON a.id = oi.artifact_id
                 JOIN object_types ot ON oi.object_type_id = ot.id
                 LEFT JOIN project_artifacts pa ON a.id = pa.artifact_id AND pa.detached_at IS NULL
                 LEFT JOIN projects p ON pa.project_id = p.id
                 WHERE a.deleted_at IS NULL
                   AND a.is_pinned = 0
                 ORDER BY a.last_accessed_at DESC, a.updated_at DESC
                 LIMIT ?1;",
            )
            .map_err(|e| e.to_string())?;

        let unpinned: Vec<SidebarItem> = unpinned_obj_stmt
            .query_map(params![unpinned_obj_limit as i64], |r| {
                Ok(SidebarItem {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    item_type: "object".to_string(),
                    is_pinned: r.get::<_, i64>(2)? != 0,
                    last_accessed_at: r.get(3)?,
                    updated_at: r.get(4)?,
                    project_id: r.get(5)?,
                    project_name: r.get(6)?,
                    object_type_id: Some(r.get(7)?),
                    object_type_name: Some(r.get(8)?),
                    icon: Some(r.get(9)?),
                    color: Some(r.get(10)?),
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        objects.extend(unpinned);
    }

    Ok(SidebarItemsResponse {
        projects,
        notes,
        objects,
    })
}

#[tauri::command]
pub fn cmd_get_sidebar_items(
    state: State<'_, AppState>,
) -> Result<SidebarItemsResponse, String> {
    handle_get_sidebar_items(&state.engine)
}

pub fn handle_toggle_pin(
    engine: &Engine,
    item_id: String,
    item_type: String,
    is_pinned: bool,
) -> Result<(), String> {
    let writer = engine.db().writer();
    let pin_val = if is_pinned { 1 } else { 0 };

    if item_type.to_lowercase() == "project" {
        let count = writer
            .execute(
                "UPDATE projects SET is_pinned = ?1 WHERE id = ?2;",
                params![pin_val, item_id],
            )
            .map_err(|e| e.to_string())?;
        if count == 0 {
            writer
                .execute(
                    "UPDATE artifacts SET is_pinned = ?1 WHERE id = ?2;",
                    params![pin_val, item_id],
                )
                .map_err(|e| e.to_string())?;
        }
    } else {
        writer
            .execute(
                "UPDATE artifacts SET is_pinned = ?1 WHERE id = ?2;",
                params![pin_val, item_id],
            )
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn cmd_toggle_pin(
    item_id: String,
    item_type: String,
    is_pinned: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_toggle_pin(&state.engine, item_id, item_type, is_pinned)
}

pub fn handle_record_access(
    engine: &Engine,
    item_id: String,
    item_type: String,
) -> Result<(), String> {
    let writer = engine.db().writer();
    let now_ms = engine.clock().now_ms();
    if item_type.to_lowercase() == "project" {
        writer
            .execute(
                "UPDATE projects SET last_accessed_at = ?1 WHERE id = ?2;",
                params![now_ms, item_id],
            )
            .map_err(|e| e.to_string())?;
    } else {
        writer
            .execute(
                "UPDATE artifacts SET last_accessed_at = ?1 WHERE id = ?2;",
                params![now_ms, item_id],
            )
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn cmd_record_access(
    item_id: String,
    item_type: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_record_access(&state.engine, item_id, item_type)
}

pub fn handle_get_workspace_index(
    engine: &Engine,
    tab: Option<String>,
    search: Option<String>,
) -> Result<WorkspaceIndexData, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;
    let search_term = search
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    // 1. Fetch counts
    let projects_count: usize = reader
        .query_row(
            "SELECT COUNT(*) FROM projects WHERE status <> 'archived';",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|c| c as usize)
        .unwrap_or(0);

    let notes_count: usize = reader
        .query_row(
            "SELECT COUNT(*) FROM artifacts a
             WHERE a.deleted_at IS NULL
               AND a.type = 'note'
               AND a.id NOT IN (SELECT artifact_id FROM object_instances);",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|c| c as usize)
        .unwrap_or(0);

    let objects_count: usize = reader
        .query_row(
            "SELECT COUNT(*) FROM artifacts a
             JOIN object_instances oi ON a.id = oi.artifact_id
             WHERE a.deleted_at IS NULL;",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|c| c as usize)
        .unwrap_or(0);

    let standalone_count: usize = reader
        .query_row(
            "SELECT COUNT(*) FROM artifacts a
             WHERE a.deleted_at IS NULL
               AND a.type = 'note'
               AND a.id NOT IN (SELECT artifact_id FROM object_instances)
               AND a.id NOT IN (SELECT artifact_id FROM project_artifacts WHERE detached_at IS NULL);",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|c| c as usize)
        .unwrap_or(0);

    let all_count = projects_count + notes_count + objects_count;

    let counts = WorkspaceCounts {
        all: all_count,
        projects: projects_count,
        notes: notes_count,
        objects: objects_count,
        standalone: standalone_count,
    };

    let active_tab = tab.as_deref().unwrap_or("all").to_lowercase();
    let pat = search_term
        .map(|s| format!("%{}%", s))
        .unwrap_or_else(|| "%".to_string());

    let mut items = Vec::new();

    // Query helper for Projects
    let fetch_projects = |items: &mut Vec<WorkspaceIndexItem>| -> Result<(), rusqlite::Error> {
        let mut stmt = reader.prepare(
            "SELECT p.id, p.name, COALESCE(pp.purpose, ''), p.is_pinned, p.last_accessed_at, p.last_substantive_at, p.created_at, COUNT(pa.artifact_id)
             FROM projects p
             LEFT JOIN project_purposes pp ON p.id = pp.project_id AND pp.superseded_at IS NULL
             LEFT JOIN project_artifacts pa ON p.id = pa.project_id AND pa.detached_at IS NULL
             WHERE p.status <> 'archived' AND (p.name LIKE ?1 OR COALESCE(pp.purpose, '') LIKE ?1)
             GROUP BY p.id
             ORDER BY p.is_pinned DESC, p.last_substantive_at DESC;",
        )?;

        let rows = stmt.query_map(params![pat], |r| {
            let item_count: i64 = r.get(7)?;
            Ok(WorkspaceIndexItem {
                id: r.get(0)?,
                title: r.get(1)?,
                entity_type: "project".to_string(),
                description: Some(r.get(2)?),
                is_pinned: r.get::<_, i64>(3)? != 0,
                last_accessed_at: r.get(4)?,
                updated_at: r.get(5)?,
                created_at: r.get(6)?,
                project_id: None,
                project_name: None,
                standalone: false,
                item_count: Some(item_count as usize),
                object_type_id: None,
                object_type_name: None,
                icon: Some("folder".to_string()),
                color: None,
            })
        })?;

        for r in rows {
            if let Ok(it) = r {
                items.push(it);
            }
        }
        Ok(())
    };

    // Query helper for Notes
    let fetch_notes = |items: &mut Vec<WorkspaceIndexItem>, only_standalone: bool| -> Result<(), rusqlite::Error> {
        let sql = if only_standalone {
            "SELECT a.id, COALESCE(a.title, 'Untitled Note'), SUBSTR(a.content_md, 1, 200),
                    a.is_pinned, a.last_accessed_at, a.updated_at, a.created_at,
                    NULL, NULL
             FROM artifacts a
             WHERE a.deleted_at IS NULL
               AND a.type = 'note'
               AND a.id NOT IN (SELECT artifact_id FROM object_instances)
               AND a.id NOT IN (SELECT artifact_id FROM project_artifacts WHERE detached_at IS NULL)
               AND (a.title LIKE ?1 OR a.content_md LIKE ?1)
             ORDER BY a.is_pinned DESC, a.updated_at DESC;"
        } else {
            "SELECT a.id, COALESCE(a.title, 'Untitled Note'), SUBSTR(a.content_md, 1, 200),
                    a.is_pinned, a.last_accessed_at, a.updated_at, a.created_at,
                    pa.project_id, p.name
             FROM artifacts a
             LEFT JOIN project_artifacts pa ON a.id = pa.artifact_id AND pa.detached_at IS NULL
             LEFT JOIN projects p ON pa.project_id = p.id
             WHERE a.deleted_at IS NULL
               AND a.type = 'note'
               AND a.id NOT IN (SELECT artifact_id FROM object_instances)
               AND (a.title LIKE ?1 OR a.content_md LIKE ?1 OR COALESCE(p.name, '') LIKE ?1)
             ORDER BY a.is_pinned DESC, a.updated_at DESC;"
        };

        let mut stmt = reader.prepare(sql)?;
        let rows = stmt.query_map(params![pat], |r| {
            let pid: Option<String> = r.get(7)?;
            Ok(WorkspaceIndexItem {
                id: r.get(0)?,
                title: r.get(1)?,
                entity_type: "note".to_string(),
                description: Some(r.get(2)?),
                is_pinned: r.get::<_, i64>(3)? != 0,
                last_accessed_at: r.get(4)?,
                updated_at: r.get(5)?,
                created_at: r.get(6)?,
                project_id: pid.clone(),
                project_name: r.get(8)?,
                standalone: pid.is_none(),
                item_count: None,
                object_type_id: None,
                object_type_name: None,
                icon: Some("file-text".to_string()),
                color: None,
            })
        })?;

        for r in rows {
            if let Ok(it) = r {
                items.push(it);
            }
        }
        Ok(())
    };

    // Query helper for Objects
    let fetch_objects = |items: &mut Vec<WorkspaceIndexItem>| -> Result<(), rusqlite::Error> {
        let mut stmt = reader.prepare(
            "SELECT a.id, COALESCE(a.title, 'Untitled Object'), SUBSTR(a.content_md, 1, 200),
                    a.is_pinned, a.last_accessed_at, a.updated_at, a.created_at,
                    pa.project_id, p.name, ot.id, ot.display_name, ot.icon, ot.color
             FROM artifacts a
             JOIN object_instances oi ON a.id = oi.artifact_id
             JOIN object_types ot ON oi.object_type_id = ot.id
             LEFT JOIN project_artifacts pa ON a.id = pa.artifact_id AND pa.detached_at IS NULL
             LEFT JOIN projects p ON pa.project_id = p.id
             WHERE a.deleted_at IS NULL
               AND (a.title LIKE ?1 OR a.content_md LIKE ?1 OR ot.display_name LIKE ?1 OR COALESCE(p.name, '') LIKE ?1)
             ORDER BY a.is_pinned DESC, a.updated_at DESC;",
        )?;

        let rows = stmt.query_map(params![pat], |r| {
            let pid: Option<String> = r.get(7)?;
            Ok(WorkspaceIndexItem {
                id: r.get(0)?,
                title: r.get(1)?,
                entity_type: "object".to_string(),
                description: Some(r.get(2)?),
                is_pinned: r.get::<_, i64>(3)? != 0,
                last_accessed_at: r.get(4)?,
                updated_at: r.get(5)?,
                created_at: r.get(6)?,
                project_id: pid.clone(),
                project_name: r.get(8)?,
                standalone: pid.is_none(),
                item_count: None,
                object_type_id: Some(r.get(9)?),
                object_type_name: Some(r.get(10)?),
                icon: Some(r.get(11)?),
                color: Some(r.get(12)?),
            })
        })?;

        for r in rows {
            if let Ok(it) = r {
                items.push(it);
            }
        }
        Ok(())
    };

    match active_tab.as_str() {
        "projects" => {
            fetch_projects(&mut items).map_err(|e| e.to_string())?;
        }
        "notes" => {
            fetch_notes(&mut items, false).map_err(|e| e.to_string())?;
        }
        "objects" => {
            fetch_objects(&mut items).map_err(|e| e.to_string())?;
        }
        "standalone" => {
            fetch_notes(&mut items, true).map_err(|e| e.to_string())?;
        }
        _ => {
            fetch_projects(&mut items).map_err(|e| e.to_string())?;
            fetch_notes(&mut items, false).map_err(|e| e.to_string())?;
            fetch_objects(&mut items).map_err(|e| e.to_string())?;
            items.sort_by(|a, b| {
                b.is_pinned.cmp(&a.is_pinned)
                    .then_with(|| b.updated_at.cmp(&a.updated_at))
            });
        }
    }

    Ok(WorkspaceIndexData { counts, items })
}

#[tauri::command]
pub fn cmd_get_workspace_index(
    tab: Option<String>,
    search: Option<String>,
    state: State<'_, AppState>,
) -> Result<WorkspaceIndexData, String> {
    handle_get_workspace_index(&state.engine, tab, search)
}



// ============================================================================
// Phase 18: Continuous & Scheduled Research / Planning Topics
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTopicRunView {
    pub id: String,
    pub artifact_id: String,
    pub artifact_title: String,
    pub sources_scraped_count: i64,
    pub executed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTopicView {
    pub id: String,
    pub title: String,
    pub prompt_query: String,
    pub topic_type: String,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub cadence: String,
    pub model_id: Option<String>,
    pub search_provider_id: Option<String>,
    pub is_active: bool,
    pub last_run_at: Option<i64>,
    pub created_at: i64,
    /// Phase 18.1: `"HH:MM"` (UTC) the topic fires at, or `None` for a rolling interval.
    pub schedule_time: Option<String>,
    /// Phase 18.1: ISO weekday anchor (0=Monday .. 6=Sunday) for weekly topics.
    pub schedule_weekday: Option<u8>,
    /// Phase 18.1: next expected wall-clock firing, when one is known.
    pub next_run_at: Option<i64>,
    /// Phase 18.1: human-readable schedule summary for the card.
    pub schedule_label: String,
    /// Past run artifacts, newest first — powers the "Previous Digests" list.
    pub runs: Vec<ScheduledTopicRunView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateScheduledTopicInput {
    pub title: String,
    pub prompt_query: String,
    pub topic_type: String,
    pub project_id: Option<String>,
    pub cadence: Option<String>,
    pub model_id: Option<String>,
    pub search_provider_id: Option<String>,
    /// Phase 18.1: `"HH:MM"` (UTC) time of day to fire at. `None` = rolling interval.
    pub schedule_time: Option<String>,
    /// Phase 18.1: ISO weekday anchor (0=Monday .. 6=Sunday) for weekly topics.
    pub schedule_weekday: Option<u8>,
}

fn weekday_label(w: u8) -> &'static str {
    match w % 7 {
        0 => "Mon",
        1 => "Tue",
        2 => "Wed",
        3 => "Thu",
        4 => "Fri",
        5 => "Sat",
        _ => "Sun",
    }
}

/// Human-readable one-line summary of a topic's schedule, e.g. `Daily at 06:00 (UTC)`.
fn schedule_label(cadence: sthira_core::TopicCadence, minutes: Option<i64>, weekday: Option<u8>) -> String {
    let time = minutes.map(sthira_core::format_hhmm);
    match (cadence, time) {
        (sthira_core::TopicCadence::Manual, _) => "Manual only".to_string(),
        (sthira_core::TopicCadence::Daily, None) => "Daily (rolling 24h)".to_string(),
        (sthira_core::TopicCadence::Daily, Some(t)) => format!("Daily at {t} (UTC)"),
        (sthira_core::TopicCadence::Weekly, None) => "Weekly (rolling 7d)".to_string(),
        (sthira_core::TopicCadence::Weekly, Some(t)) => match weekday {
            Some(w) => format!("Weekly · {} at {t} (UTC)", weekday_label(w)),
            None => format!("Weekly at {t} (UTC)"),
        },
    }
}

fn to_topic_view(
    engine: &Engine,
    topic: sthira_core::ScheduledTopic,
) -> Result<ScheduledTopicView, String> {
    let reader = engine.db().reader().map_err(|e| e.to_string())?;

    let project_name: Option<String> = topic.project_id.as_deref().and_then(|pid| {
        reader
            .query_row(
                "SELECT name FROM projects WHERE id = ?1;",
                params![pid],
                |r| r.get::<_, String>(0),
            )
            .ok()
    });

    let runs = sthira_core::db::scheduled_topics_repo::list_runs_for_topic(&reader, &topic.id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|run| {
            let artifact_title = reader
                .query_row(
                    "SELECT COALESCE(title, 'Untitled') FROM artifacts WHERE id = ?1;",
                    params![run.artifact_id],
                    |r| r.get::<_, String>(0),
                )
                .unwrap_or_else(|_| "Deleted digest".to_string());
            ScheduledTopicRunView {
                id: run.id,
                artifact_id: run.artifact_id,
                artifact_title,
                sources_scraped_count: run.sources_scraped_count,
                executed_at: run.executed_at,
            }
        })
        .collect();

    // Phase 18.1: derive schedule presentation values before moving fields out of `topic`.
    let schedule_time = topic.schedule_time_minutes.map(sthira_core::format_hhmm);
    let next_run_at = topic.next_run_at(engine.clock().now_ms());
    let label = schedule_label(
        topic.cadence,
        topic.schedule_time_minutes,
        topic.schedule_weekday,
    );

    Ok(ScheduledTopicView {
        id: topic.id,
        title: topic.title,
        prompt_query: topic.prompt_query,
        topic_type: topic.topic_type.as_str().to_string(),
        project_id: topic.project_id,
        project_name,
        cadence: topic.cadence.as_str().to_string(),
        model_id: topic.model_id,
        search_provider_id: topic.search_provider_id,
        is_active: topic.is_active,
        last_run_at: topic.last_run_at,
        created_at: topic.created_at,
        schedule_time,
        schedule_weekday: topic.schedule_weekday,
        next_run_at,
        schedule_label: label,
        runs,
    })
}

pub fn handle_list_scheduled_topics(
    engine: &Engine,
    topic_type: Option<String>,
) -> Result<Vec<ScheduledTopicView>, String> {
    let filter = topic_type
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(sthira_core::TopicType::from_str_lossy);

    let topics = {
        let reader = engine.db().reader().map_err(|e| e.to_string())?;
        sthira_core::db::scheduled_topics_repo::list_topics_by_type(&reader, filter)
            .map_err(|e| e.to_string())?
    };

    topics
        .into_iter()
        .map(|t| to_topic_view(engine, t))
        .collect()
}

#[tauri::command]
pub fn cmd_list_scheduled_topics(
    topic_type: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ScheduledTopicView>, String> {
    handle_list_scheduled_topics(&state.engine, topic_type)
}


pub fn handle_create_scheduled_topic(
    engine: &Engine,
    input: CreateScheduledTopicInput,
) -> Result<String, String> {
    let title = input.title.trim();
    let prompt = input.prompt_query.trim();
    if title.is_empty() {
        return Err("Topic title cannot be empty".to_string());
    }
    if prompt.is_empty() {
        return Err("Topic objective prompt cannot be empty".to_string());
    }

    // Phase 18.1: validate the optional wall-clock time ("HH:MM", UTC).
    let schedule_time_minutes = match input
        .schedule_time
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        Some(raw) => Some(sthira_core::parse_hhmm(raw).ok_or_else(|| {
            format!("Invalid schedule time '{raw}' — expected HH:MM in 24-hour form")
        })?),
        None => None,
    };

    let schedule_weekday = match input.schedule_weekday {
        Some(w) if w > 6 => return Err("Schedule weekday must be 0 (Monday) .. 6 (Sunday)".to_string()),
        other => other,
    };

    let id = uuid::Uuid::new_v4().to_string();
    let topic = sthira_core::ScheduledTopic {
        id: id.clone(),
        title: title.to_string(),
        prompt_query: prompt.to_string(),
        topic_type: sthira_core::TopicType::from_str_lossy(&input.topic_type),
        project_id: input.project_id.filter(|p| !p.trim().is_empty()),
        cadence: input
            .cadence
            .as_deref()
            .map(sthira_core::TopicCadence::from_str_lossy)
            .unwrap_or(sthira_core::TopicCadence::Daily),
        model_id: input.model_id.filter(|m| !m.trim().is_empty()),
        search_provider_id: input.search_provider_id.filter(|s| !s.trim().is_empty()),
        is_active: true,
        last_run_at: None,
        created_at: engine.clock().now_ms(),
        schedule_time_minutes,
        schedule_weekday,
    };

    let writer = engine.db().writer();
    sthira_core::db::scheduled_topics_repo::create_topic(&writer, &topic)
        .map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn cmd_create_scheduled_topic(
    topic: CreateScheduledTopicInput,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_create_scheduled_topic(&state.engine, topic)
}

/// Runs a topic immediately in the background and returns the created artifact id.
pub async fn handle_trigger_scheduled_topic_run(
    engine: &Engine,
    topic_id: String,
) -> Result<String, String> {
    sthira_core::engines::scheduled_runner::execute_topic_run(engine, &topic_id).await
}

#[tauri::command]
pub async fn cmd_trigger_scheduled_topic_run(
    topic_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_trigger_scheduled_topic_run(&state.engine, topic_id).await
}

pub fn handle_toggle_scheduled_topic_active(
    engine: &Engine,
    topic_id: String,
    is_active: bool,
) -> Result<(), String> {
    let writer = engine.db().writer();
    sthira_core::db::scheduled_topics_repo::update_topic_status(&writer, &topic_id, is_active)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_toggle_scheduled_topic_active(
    topic_id: String,
    is_active: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_toggle_scheduled_topic_active(&state.engine, topic_id, is_active)
}

pub fn handle_delete_scheduled_topic(engine: &Engine, topic_id: String) -> Result<(), String> {
    let writer = engine.db().writer();
    sthira_core::db::scheduled_topics_repo::delete_topic(&writer, &topic_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_delete_scheduled_topic(
    topic_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    handle_delete_scheduled_topic(&state.engine, topic_id)
}

pub async fn handle_refine_artifact(
    engine: &Engine,
    artifact_id: String,
    critique: String,
    model_id: Option<String>,
    web_search: bool,
) -> Result<String, String> {
    engine
        .refine_artifact(&artifact_id, &critique, model_id.as_deref(), web_search)
        .await
}

#[tauri::command]
pub async fn cmd_refine_artifact(
    artifact_id: String,
    critique: String,
    model_id: Option<String>,
    web_search: bool,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_refine_artifact(&state.engine, artifact_id, critique, model_id, web_search).await
}

pub async fn handle_derive_artifact(
    engine: &Engine,
    source_artifact_id: String,
    derivation_type: String,
    template_id: Option<String>,
    model_id: Option<String>,
) -> Result<String, String> {
    engine
        .derive_artifact(
            &source_artifact_id,
            &derivation_type,
            template_id.as_deref(),
            model_id.as_deref(),
        )
        .await
}

#[tauri::command]
pub async fn cmd_derive_artifact(
    source_artifact_id: String,
    derivation_type: String,
    template_id: Option<String>,
    model_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    handle_derive_artifact(
        &state.engine,
        source_artifact_id,
        derivation_type,
        template_id,
        model_id,
    )
    .await
}


