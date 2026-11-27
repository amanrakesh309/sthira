//! Human resource permissions for shared projects and pages (local Linux pilot).
//!
//! Access narrows visibility inside a workspace; it never elevates a workspace role. Every
//! operation still requires a verified device session and an active membership, and the workspace
//! role capability for the requested action. This module owns the single authoritative
//! authorization/query vocabulary used by the shared-content, realtime, agent and backup paths.
//!
//! Model:
//! - projects: `workspace` (all active members) or `restricted` (selected active members).
//! - pages: `workspace`, `project` (follows one designated project's current audience) or
//!   `selected` (selected active members).
//! - Owners and Admins implicitly read every resource in their workspace.
//! - Audience grants bind the membership row (`membership_id`), which is unique per membership
//!   incarnation. Removing and rejoining creates a new row, so old grants never revive.
//! - `access_version` is monotonic. Any policy or audience change increments it (and the versions
//!   of project-following pages), so a change followed by a reversal still invalidates previews,
//!   submissions and review bases built on the earlier authority.

use crate::{
    membership::{sanitize_idempotency_key, SessionRecheck, WorkspaceRole},
    shared_content::SharedContentError,
    store::{sha256_hex, VerifiedSession},
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

pub const ACCESS_SCHEMA_VERSION: i64 = 7;
pub const SHARING_PREVIEW_TTL_MS: i64 = 5 * 60 * 1000;

/// Administrative access is broad by design. It is disclosed in every sharing surface.
pub const ADMIN_ACCESS_DISCLOSURE: &str =
    "Owners and Admins can always open restricted projects and pages, their history, and full workspace backups. Restricting a resource does not hide it from workspace administrators.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectAccessMode {
    Workspace,
    Restricted,
}

impl ProjectAccessMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ProjectAccessMode::Workspace => "workspace",
            ProjectAccessMode::Restricted => "restricted",
        }
    }

    pub fn from_db(raw: &str) -> Result<Self, SharedContentError> {
        match raw {
            "workspace" => Ok(ProjectAccessMode::Workspace),
            "restricted" => Ok(ProjectAccessMode::Restricted),
            _ => Err(SharedContentError::InvalidInput(
                "Unknown project access mode.".into(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PageAccessMode {
    Workspace,
    Project,
    Selected,
}

impl PageAccessMode {
    pub fn as_str(self) -> &'static str {
        match self {
            PageAccessMode::Workspace => "workspace",
            PageAccessMode::Project => "project",
            PageAccessMode::Selected => "selected",
        }
    }

    pub fn from_db(raw: &str) -> Result<Self, SharedContentError> {
        match raw {
            "workspace" => Ok(PageAccessMode::Workspace),
            "project" => Ok(PageAccessMode::Project),
            "selected" => Ok(PageAccessMode::Selected),
            _ => Err(SharedContentError::InvalidInput(
                "Unknown page access mode.".into(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessAudienceMember {
    pub membership_id: String,
    pub uid: String,
    pub display_name: String,
    pub role: WorkspaceRole,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowingPageEntry {
    pub page_id: String,
    pub title: String,
    pub access_mode: PageAccessMode,
    pub access_project_id: Option<String>,
    pub access_version: i64,
    pub archived: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceAccessPolicyView {
    pub resource_kind: String,
    pub resource_id: String,
    pub access_mode: String,
    pub access_project_id: Option<String>,
    pub access_project_name: Option<String>,
    pub access_version: i64,
    pub audience: Vec<AccessAudienceMember>,
    pub can_manage: bool,
    pub administrative_access: bool,
    pub administrative_disclosure: String,
    pub following_pages: Vec<FollowingPageEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessChangePreviewInput {
    pub proposed_mode: String,
    #[serde(default)]
    pub selected_membership_ids: Vec<String>,
    #[serde(default)]
    pub follow_project_id: Option<String>,
    /// Optional explicit conversion of legacy pages to "follow this project" during restriction.
    #[serde(default)]
    pub convert_page_ids: Vec<String>,
    /// Legacy input accepted for compatibility. Confirmation is enforced at commit, after preview.
    #[serde(default)]
    pub confirm_expansion: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessAudienceChange {
    pub added: Vec<AccessAudienceMember>,
    pub removed: Vec<AccessAudienceMember>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessChangePreview {
    pub preview_id: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub current_mode: String,
    pub proposed_mode: String,
    pub current_audience: Vec<AccessAudienceMember>,
    pub proposed_audience: Vec<AccessAudienceMember>,
    pub audience_change: AccessAudienceChange,
    pub follow_project_id: Option<String>,
    pub follow_project_name: Option<String>,
    pub affected_pages: Vec<FollowingPageEntry>,
    pub conversion_page_ids: Vec<String>,
    pub administrative_disclosure: String,
    pub shares_history: String,
    pub expansion_requires_confirmation: bool,
    pub expires_at_ms: i64,
    pub base_access_version: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessChangeCommitInput {
    #[serde(default)]
    pub confirm_expansion: bool,
    pub preview_id: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessChangeOutcome {
    pub committed: bool,
    pub policy: ResourceAccessPolicyView,
    pub changed_page_ids: Vec<String>,
}

/// Immutable authorization context for one caller in one workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallerAccess {
    pub membership_id: String,
    pub uid: String,
    pub display_name: String,
    pub role: WorkspaceRole,
    /// Monotonic membership row version (updated on role/display changes).
    pub membership_version: i64,
}

impl CallerAccess {
    pub fn is_privileged(&self) -> bool {
        self.role.can_moderate_or_review()
    }

    pub fn can_manage_access(&self) -> bool {
        self.is_privileged()
    }

    pub fn can_author(&self) -> bool {
        self.role.can_author_shared_content()
    }
}

pub(crate) fn caller_access(
    conn: &Connection,
    workspace_id: &str,
    uid: &str,
) -> Result<Option<CallerAccess>, SharedContentError> {
    let row = conn
        .query_row(
            "SELECT m.membership_id, m.uid, m.display_name, m.role, m.updated_at_ms,
                    (SELECT COUNT(*) FROM workspace_audit_logs l
                      WHERE l.workspace_id = m.workspace_id
                        AND l.target_membership_id = m.membership_id
                        AND l.action = 'member_role_changed')
             FROM workspace_memberships m
             WHERE m.workspace_id = ?1 AND m.uid = ?2 AND m.removed_at_ms IS NULL",
            params![workspace_id, uid],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()?;
    let Some((membership_id, uid, display_name, role_raw, _updated_at, role_changes)) = row else {
        return Ok(None);
    };
    Ok(Some(CallerAccess {
        membership_id,
        uid,
        display_name,
        role: WorkspaceRole::from_db(&role_raw)?,
        membership_version: role_changes,
    }))
}

pub(crate) fn require_caller_access(
    conn: &Connection,
    workspace_id: &str,
    uid: &str,
) -> Result<CallerAccess, SharedContentError> {
    caller_access(conn, workspace_id, uid)?.ok_or(SharedContentError::Forbidden)
}

/// SQL predicate over a `shared_pages p` alias. Appends two positional parameters:
/// the caller membership ID and the privileged flag.
pub(crate) fn page_visible_predicate(
    alias: &str,
    membership_param: usize,
    privileged_param: usize,
) -> String {
    format!(
        "(
            ?{privileged_param} = 1
            OR {alias}.access_mode = 'workspace'
            OR ({alias}.access_mode = 'selected' AND EXISTS (
                SELECT 1 FROM shared_resource_audience a
                 WHERE a.workspace_id = {alias}.workspace_id
                   AND a.resource_kind = 'page'
                   AND a.resource_id = {alias}.page_id
                   AND a.revoked_at_ms IS NULL
                   AND a.membership_id = ?{membership_param}
            ))
            OR ({alias}.access_mode = 'project' AND EXISTS (
                SELECT 1 FROM shared_projects f
                 WHERE f.workspace_id = {alias}.workspace_id
                   AND f.project_id = {alias}.access_project_id
                   AND (
                       f.access_mode = 'workspace'
                       OR EXISTS (
                           SELECT 1 FROM shared_resource_audience fa
                            WHERE fa.workspace_id = f.workspace_id
                              AND fa.resource_kind = 'project'
                              AND fa.resource_id = f.project_id
                              AND fa.revoked_at_ms IS NULL
                              AND fa.membership_id = ?{membership_param}
                       )
                   )
            ))
        )"
    )
}

/// SQL predicate over a `shared_projects pr` alias. Appends two positional parameters.
pub(crate) fn project_visible_predicate(
    alias: &str,
    membership_param: usize,
    privileged_param: usize,
) -> String {
    format!(
        "(
            ?{privileged_param} = 1
            OR {alias}.access_mode = 'workspace'
            OR ({alias}.access_mode = 'restricted' AND EXISTS (
                SELECT 1 FROM shared_resource_audience a
                 WHERE a.workspace_id = {alias}.workspace_id
                   AND a.resource_kind = 'project'
                   AND a.resource_id = {alias}.project_id
                   AND a.revoked_at_ms IS NULL
                   AND a.membership_id = ?{membership_param}
            ))
        )"
    )
}

pub(crate) fn privileged_flag(ctx: &CallerAccess) -> i64 {
    if ctx.is_privileged() {
        1
    } else {
        0
    }
}

pub(crate) fn page_readable(
    conn: &Connection,
    workspace_id: &str,
    page_id: &str,
    ctx: &CallerAccess,
) -> Result<bool, SharedContentError> {
    let sql = format!(
        "SELECT 1 FROM shared_pages p
          WHERE p.workspace_id = ?1 AND p.page_id = ?2 AND {}",
        page_visible_predicate("p", 3, 4)
    );
    Ok(conn
        .query_row(
            &sql,
            params![
                workspace_id,
                page_id,
                ctx.membership_id,
                privileged_flag(ctx)
            ],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .is_some())
}

pub(crate) fn project_readable(
    conn: &Connection,
    workspace_id: &str,
    project_id: &str,
    ctx: &CallerAccess,
) -> Result<bool, SharedContentError> {
    let sql = format!(
        "SELECT 1 FROM shared_projects pr
          WHERE pr.workspace_id = ?1 AND pr.project_id = ?2 AND {}",
        project_visible_predicate("pr", 3, 4)
    );
    Ok(conn
        .query_row(
            &sql,
            params![
                workspace_id,
                project_id,
                ctx.membership_id,
                privileged_flag(ctx)
            ],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .is_some())
}

/// Missing and inaccessible resources share one response shape.
pub(crate) fn require_readable_page(
    conn: &Connection,
    workspace_id: &str,
    page_id: &str,
    ctx: &CallerAccess,
) -> Result<(), SharedContentError> {
    if page_readable(conn, workspace_id, page_id, ctx)? {
        Ok(())
    } else {
        Err(SharedContentError::PageNotFound)
    }
}

pub(crate) fn require_readable_project(
    conn: &Connection,
    workspace_id: &str,
    project_id: &str,
    ctx: &CallerAccess,
) -> Result<(), SharedContentError> {
    if project_readable(conn, workspace_id, project_id, ctx)? {
        Ok(())
    } else {
        Err(SharedContentError::ProjectNotFound)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PageAccessRow {
    pub access_mode: PageAccessMode,
    pub access_project_id: Option<String>,
    pub access_version: i64,
}

pub(crate) fn page_access_row(
    conn: &Connection,
    workspace_id: &str,
    page_id: &str,
) -> Result<PageAccessRow, SharedContentError> {
    let row = conn
        .query_row(
            "SELECT access_mode, access_project_id, access_version
             FROM shared_pages WHERE workspace_id = ?1 AND page_id = ?2",
            params![workspace_id, page_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((mode, project, version)) = row else {
        return Err(SharedContentError::PageNotFound);
    };
    Ok(PageAccessRow {
        access_mode: PageAccessMode::from_db(&mode)?,
        access_project_id: project,
        access_version: version,
    })
}

#[derive(Clone, Debug)]
pub(crate) struct ProjectAccessRow {
    pub access_mode: ProjectAccessMode,
    pub access_version: i64,
}

pub(crate) fn project_access_row(
    conn: &Connection,
    workspace_id: &str,
    project_id: &str,
) -> Result<ProjectAccessRow, SharedContentError> {
    let row = conn
        .query_row(
            "SELECT access_mode, access_version
             FROM shared_projects WHERE workspace_id = ?1 AND project_id = ?2",
            params![workspace_id, project_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        )
        .optional()?;
    let Some((mode, version)) = row else {
        return Err(SharedContentError::ProjectNotFound);
    };
    Ok(ProjectAccessRow {
        access_mode: ProjectAccessMode::from_db(&mode)?,
        access_version: version,
    })
}

/// Stable digest of a page's effective access basis. Stored with submissions and review bases so
/// returning to the same audience later cannot revive old review authority.
pub(crate) fn page_access_basis(
    conn: &Connection,
    workspace_id: &str,
    page_id: &str,
) -> Result<(i64, String), SharedContentError> {
    let policy = page_access_row(conn, workspace_id, page_id)?;
    let mut parts: Vec<String> = vec![policy.access_mode.as_str().to_owned()];
    match policy.access_mode {
        PageAccessMode::Workspace => {}
        PageAccessMode::Selected => {
            let mut ids = active_audience_ids(conn, workspace_id, "page", page_id)?;
            ids.sort();
            parts.push(ids.join(","));
        }
        PageAccessMode::Project => {
            let anchor = policy.access_project_id.clone().unwrap_or_default();
            parts.push(anchor.clone());
            match project_access_row(conn, workspace_id, &anchor) {
                Ok(project) => {
                    parts.push(project.access_mode.as_str().to_owned());
                    if project.access_mode == ProjectAccessMode::Restricted {
                        let mut ids = active_audience_ids(conn, workspace_id, "project", &anchor)?;
                        ids.sort();
                        parts.push(ids.join(","));
                    }
                }
                Err(SharedContentError::ProjectNotFound) => parts.push("missing".into()),
                Err(err) => return Err(err),
            }
        }
    }
    Ok((policy.access_version, sha256_hex(&parts.join("|"))))
}

pub(crate) fn active_audience_ids(
    conn: &Connection,
    workspace_id: &str,
    kind: &str,
    resource_id: &str,
) -> Result<Vec<String>, SharedContentError> {
    let mut stmt = conn.prepare(
        "SELECT a.membership_id
           FROM shared_resource_audience a
           JOIN workspace_memberships m
             ON m.membership_id = a.membership_id
            AND m.workspace_id = a.workspace_id
            AND m.removed_at_ms IS NULL
          WHERE a.workspace_id = ?1 AND a.resource_kind = ?2
            AND a.resource_id = ?3 AND a.revoked_at_ms IS NULL
          ORDER BY a.membership_id",
    )?;
    let rows = stmt.query_map(params![workspace_id, kind, resource_id], |r| {
        r.get::<_, String>(0)
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub(crate) fn audience_members(
    conn: &Connection,
    workspace_id: &str,
    kind: &str,
    resource_id: &str,
) -> Result<Vec<AccessAudienceMember>, SharedContentError> {
    let mut stmt = conn.prepare(
        "SELECT m.membership_id, m.uid, m.display_name, m.role
           FROM shared_resource_audience a
           JOIN workspace_memberships m
             ON m.membership_id = a.membership_id
            AND m.workspace_id = a.workspace_id
            AND m.removed_at_ms IS NULL
          WHERE a.workspace_id = ?1 AND a.resource_kind = ?2
            AND a.resource_id = ?3 AND a.revoked_at_ms IS NULL
          ORDER BY LOWER(m.display_name), m.membership_id",
    )?;
    let rows = stmt.query_map(params![workspace_id, kind, resource_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    let mut out = Vec::new();
    for r in rows {
        let (membership_id, uid, display_name, role) = r?;
        out.push(AccessAudienceMember {
            membership_id,
            uid,
            display_name,
            role: WorkspaceRole::from_db(&role)?,
        });
    }
    Ok(out)
}

pub(crate) fn list_active_members(
    conn: &Connection,
    workspace_id: &str,
) -> Result<Vec<AccessAudienceMember>, SharedContentError> {
    let mut stmt = conn.prepare(
        "SELECT membership_id, uid, display_name, role
           FROM workspace_memberships
          WHERE workspace_id = ?1 AND removed_at_ms IS NULL
          ORDER BY LOWER(display_name), membership_id",
    )?;
    let rows = stmt.query_map(params![workspace_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    let mut out = Vec::new();
    for r in rows {
        let (membership_id, uid, display_name, role) = r?;
        out.push(AccessAudienceMember {
            membership_id,
            uid,
            display_name,
            role: WorkspaceRole::from_db(&role)?,
        });
    }
    Ok(out)
}

/// Additive migration. Legacy resources stay workspace-visible and legacy submissions bind the
/// workspace basis they were written under.
pub(crate) fn migrate(conn: &Connection) -> Result<(), SharedContentError> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS shared_resource_audience (
            audience_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
            resource_kind TEXT NOT NULL CHECK (resource_kind IN ('project', 'page')),
            resource_id TEXT NOT NULL,
            membership_id TEXT NOT NULL REFERENCES workspace_memberships(membership_id),
            grant_generation TEXT NOT NULL,
            granted_by_uid TEXT NOT NULL,
            granted_at_ms INTEGER NOT NULL,
            revoked_at_ms INTEGER,
            revoked_by_uid TEXT
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_shared_audience_active
            ON shared_resource_audience (workspace_id, resource_kind, resource_id, membership_id)
            WHERE revoked_at_ms IS NULL;
        CREATE INDEX IF NOT EXISTS idx_shared_audience_resource
            ON shared_resource_audience (workspace_id, resource_kind, resource_id, revoked_at_ms);

        CREATE TABLE IF NOT EXISTS shared_sharing_previews (
            preview_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
            actor_uid TEXT NOT NULL,
            actor_membership_id TEXT NOT NULL,
            actor_role TEXT NOT NULL DEFAULT 'viewer',
            actor_membership_version INTEGER NOT NULL DEFAULT 0,
            session_id TEXT NOT NULL,
            resource_kind TEXT NOT NULL CHECK (resource_kind IN ('project', 'page')),
            resource_id TEXT NOT NULL,
            base_access_version INTEGER NOT NULL,
            base_audience_digest TEXT NOT NULL,
            proposed_mode TEXT NOT NULL,
            proposed_project_id TEXT,
            proposed_membership_ids_json TEXT NOT NULL,
            conversion_page_ids_json TEXT NOT NULL DEFAULT '[]',
            affected_pages_json TEXT NOT NULL,
            dependency_digest TEXT NOT NULL,
            request_fingerprint TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL,
            expires_at_ms INTEGER NOT NULL,
            committed_at_ms INTEGER
        );

        CREATE TABLE IF NOT EXISTS shared_resource_access_audit (
            audit_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
            resource_kind TEXT NOT NULL,
            resource_id TEXT NOT NULL,
            actor_uid TEXT NOT NULL,
            actor_display_name TEXT NOT NULL,
            action TEXT NOT NULL,
            from_mode TEXT NOT NULL,
            to_mode TEXT NOT NULL,
            detail_json TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_shared_access_audit_ws
            ON shared_resource_access_audit (workspace_id, created_at_ms DESC);
        ",
    )?;
    add_access_columns(conn)?;
    if !table_columns(conn, "shared_sharing_previews")?
        .iter()
        .any(|c| c == "actor_role")
    {
        conn.execute(
            "ALTER TABLE shared_sharing_previews ADD COLUMN actor_role TEXT NOT NULL DEFAULT 'viewer'",
            [],
        )?;
    }
    if !table_columns(conn, "shared_sharing_previews")?
        .iter()
        .any(|c| c == "actor_membership_version")
    {
        conn.execute(
            "ALTER TABLE shared_sharing_previews ADD COLUMN actor_membership_version INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    ensure_constraints(conn)?;
    let legacy_workspace_digest = sha256_hex("workspace");
    conn.execute(
        "UPDATE shared_change_proposals SET page_access_digest = ?1
          WHERE page_access_digest = ''",
        params![legacy_workspace_digest],
    )?;
    conn.execute(
        "UPDATE shared_live_drafts SET access_digest = ?1 WHERE access_digest = ''",
        params![legacy_workspace_digest],
    )?;
    conn.execute(
        "UPDATE shared_agent_page_reads SET access_digest = ?1 WHERE access_digest = ''",
        params![legacy_workspace_digest],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, name, applied_at_ms)
         VALUES (?1, 'shared_resource_permissions_v7', ?2)",
        params![ACCESS_SCHEMA_VERSION, crate::now_epoch_ms()],
    )?;
    Ok(())
}

fn add_access_columns(conn: &Connection) -> Result<(), SharedContentError> {
    let columns: [(&str, &str, &str); 13] = [
        (
            "shared_projects",
            "access_mode",
            "TEXT NOT NULL DEFAULT 'workspace'",
        ),
        (
            "shared_projects",
            "access_version",
            "INTEGER NOT NULL DEFAULT 1",
        ),
        (
            "shared_pages",
            "access_mode",
            "TEXT NOT NULL DEFAULT 'workspace'",
        ),
        ("shared_pages", "access_project_id", "TEXT"),
        (
            "shared_pages",
            "access_version",
            "INTEGER NOT NULL DEFAULT 1",
        ),
        (
            "shared_change_proposals",
            "page_access_version",
            "INTEGER NOT NULL DEFAULT 1",
        ),
        (
            "shared_change_proposals",
            "page_access_digest",
            "TEXT NOT NULL DEFAULT ''",
        ),
        (
            "shared_live_drafts",
            "access_version",
            "INTEGER NOT NULL DEFAULT 1",
        ),
        (
            "shared_live_drafts",
            "access_digest",
            "TEXT NOT NULL DEFAULT ''",
        ),
        (
            "shared_agent_page_proposals",
            "page_access_version",
            "INTEGER NOT NULL DEFAULT 1",
        ),
        (
            "shared_agent_page_proposals",
            "page_access_digest",
            "TEXT NOT NULL DEFAULT ''",
        ),
        (
            "shared_agent_page_reads",
            "access_version",
            "INTEGER NOT NULL DEFAULT 1",
        ),
        (
            "shared_agent_page_reads",
            "access_digest",
            "TEXT NOT NULL DEFAULT ''",
        ),
    ];
    for (table, column, ddl) in columns {
        if !table_columns(conn, table)?.iter().any(|c| c == column) {
            conn.execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {ddl}"),
                [],
            )?;
        }
    }
    Ok(())
}

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>, SharedContentError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

fn ensure_constraints(conn: &Connection) -> Result<(), SharedContentError> {
    conn.execute_batch(
        "
        CREATE TRIGGER IF NOT EXISTS trg_shared_pages_access_insert
        BEFORE INSERT ON shared_pages
        WHEN NEW.access_mode NOT IN ('workspace', 'project', 'selected')
          OR (NEW.access_mode = 'project' AND NOT EXISTS (
                SELECT 1 FROM shared_projects
                 WHERE workspace_id = NEW.workspace_id AND project_id = NEW.access_project_id))
        BEGIN
            SELECT RAISE(ABORT, 'invalid_page_access_policy');
        END;

        CREATE TRIGGER IF NOT EXISTS trg_shared_pages_access_update
        BEFORE UPDATE OF access_mode, access_project_id ON shared_pages
        WHEN NEW.access_mode NOT IN ('workspace', 'project', 'selected')
          OR (NEW.access_mode = 'project' AND NOT EXISTS (
                SELECT 1 FROM shared_projects
                 WHERE workspace_id = NEW.workspace_id AND project_id = NEW.access_project_id))
        BEGIN
            SELECT RAISE(ABORT, 'invalid_page_access_policy');
        END;

        CREATE TRIGGER IF NOT EXISTS trg_shared_projects_access
        BEFORE UPDATE OF access_mode ON shared_projects
        WHEN NEW.access_mode NOT IN ('workspace', 'restricted')
        BEGIN
            SELECT RAISE(ABORT, 'invalid_project_access_policy');
        END;

        CREATE TRIGGER IF NOT EXISTS trg_shared_audience_membership
        BEFORE INSERT ON shared_resource_audience
        WHEN NOT EXISTS (
            SELECT 1 FROM workspace_memberships
             WHERE membership_id = NEW.membership_id
               AND workspace_id = NEW.workspace_id
               AND uid = NEW.grant_generation
               AND removed_at_ms IS NULL)
        BEGIN
            SELECT RAISE(ABORT, 'invalid_audience_membership');
        END;
        ",
    )?;
    Ok(())
}

fn audience_digest(
    conn: &Connection,
    workspace_id: &str,
    kind: &str,
    resource_id: &str,
) -> Result<String, SharedContentError> {
    let (mode, anchor) = match kind {
        "project" => (
            project_access_row(conn, workspace_id, resource_id)?
                .access_mode
                .as_str()
                .to_owned(),
            None,
        ),
        _ => {
            let p = page_access_row(conn, workspace_id, resource_id)?;
            (p.access_mode.as_str().to_owned(), p.access_project_id)
        }
    };
    let ids = effective_member_ids(
        conn,
        workspace_id,
        kind,
        &mode,
        anchor.as_deref(),
        &active_audience_ids(conn, workspace_id, kind, resource_id)?,
    )?;
    Ok(sha256_hex(&ids.into_iter().collect::<Vec<_>>().join(",")))
}

#[allow(clippy::too_many_arguments)]
fn audit(
    conn: &Connection,
    workspace_id: &str,
    kind: &str,
    resource_id: &str,
    actor: &CallerAccess,
    action: &str,
    from_mode: &str,
    to_mode: &str,
    detail: serde_json::Value,
    now_ms: i64,
) -> Result<(), SharedContentError> {
    conn.execute(
        "INSERT INTO shared_resource_access_audit
         (audit_id, workspace_id, resource_kind, resource_id, actor_uid, actor_display_name,
          action, from_mode, to_mode, detail_json, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            format!("aud_{}", Uuid::new_v4().simple()),
            workspace_id,
            kind,
            resource_id,
            actor.uid,
            actor.display_name,
            action,
            from_mode,
            to_mode,
            serde_json::to_string(&detail).unwrap_or_else(|_| "{}".into()),
            now_ms,
        ],
    )?;
    Ok(())
}

pub(crate) fn following_pages(
    conn: &Connection,
    workspace_id: &str,
    project_id: &str,
) -> Result<Vec<FollowingPageEntry>, SharedContentError> {
    let mut stmt = conn.prepare(
        "SELECT page_id, title, access_mode, access_project_id, access_version, archived_at_ms
           FROM shared_pages
          WHERE workspace_id = ?1 AND access_mode = 'project' AND access_project_id = ?2
          ORDER BY LOWER(title), page_id",
    )?;
    let rows = stmt.query_map(params![workspace_id, project_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, Option<i64>>(5)?,
        ))
    })?;
    let mut out = Vec::new();
    for r in rows {
        let (page_id, title, mode, anchor, version, archived) = r?;
        out.push(FollowingPageEntry {
            page_id,
            title,
            access_mode: PageAccessMode::from_db(&mode)?,
            access_project_id: anchor,
            access_version: version,
            archived: archived.is_some(),
        });
    }
    Ok(out)
}

pub(crate) fn project_pages(
    conn: &Connection,
    workspace_id: &str,
    project_id: &str,
) -> Result<Vec<FollowingPageEntry>, SharedContentError> {
    let mut stmt = conn.prepare(
        "SELECT p.page_id, p.title, p.access_mode, p.access_project_id, p.access_version,
                p.archived_at_ms
           FROM shared_project_pages m
           JOIN shared_pages p
             ON p.workspace_id = m.workspace_id AND p.page_id = m.page_id
          WHERE m.workspace_id = ?1 AND m.project_id = ?2 AND m.detached_at_ms IS NULL
          ORDER BY LOWER(p.title), p.page_id",
    )?;
    let rows = stmt.query_map(params![workspace_id, project_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, Option<i64>>(5)?,
        ))
    })?;
    let mut out = Vec::new();
    for r in rows {
        let (page_id, title, mode, anchor, version, archived) = r?;
        out.push(FollowingPageEntry {
            page_id,
            title,
            access_mode: PageAccessMode::from_db(&mode)?,
            access_project_id: anchor,
            access_version: version,
            archived: archived.is_some(),
        });
    }
    Ok(out)
}

fn project_access_pages(
    conn: &Connection,
    workspace_id: &str,
    project_id: &str,
    ctx: &CallerAccess,
) -> Result<Vec<FollowingPageEntry>, SharedContentError> {
    let mut pages = following_pages(conn, workspace_id, project_id)?;
    if ctx.can_manage_access() {
        for page in project_pages(conn, workspace_id, project_id)? {
            if !pages.iter().any(|p| p.page_id == page.page_id) {
                pages.push(page);
            }
        }
    }
    Ok(pages)
}

fn effective_member_ids(
    conn: &Connection,
    workspace_id: &str,
    kind: &str,
    mode: &str,
    anchor: Option<&str>,
    explicit: &[String],
) -> Result<BTreeSet<String>, SharedContentError> {
    let all_active = || -> Result<BTreeSet<String>, SharedContentError> {
        Ok(list_active_members(conn, workspace_id)?
            .into_iter()
            .map(|m| m.membership_id)
            .collect())
    };
    match (kind, mode) {
        ("project", "workspace") | ("page", "workspace") => all_active(),
        ("project", "restricted") | ("page", "selected") => Ok(explicit.iter().cloned().collect()),
        ("page", "project") => {
            let Some(anchor) = anchor else {
                return Ok(BTreeSet::new());
            };
            let project = match project_access_row(conn, workspace_id, anchor) {
                Ok(p) => p,
                Err(SharedContentError::ProjectNotFound) => return Ok(BTreeSet::new()),
                Err(err) => return Err(err),
            };
            if project.access_mode == ProjectAccessMode::Workspace {
                all_active()
            } else {
                Ok(active_audience_ids(conn, workspace_id, "project", anchor)?
                    .into_iter()
                    .collect())
            }
        }
        _ => Ok(BTreeSet::new()),
    }
}

#[allow(clippy::too_many_arguments)]
fn change_expands_audience(
    conn: &Connection,
    workspace_id: &str,
    kind: &str,
    resource_id: &str,
    mode: &str,
    explicit: &[String],
    anchor: Option<&str>,
    conversions: &[String],
) -> Result<bool, SharedContentError> {
    let (current_mode, current_anchor) = if kind == "project" {
        (
            project_access_row(conn, workspace_id, resource_id)?
                .access_mode
                .as_str()
                .to_owned(),
            None,
        )
    } else {
        let p = page_access_row(conn, workspace_id, resource_id)?;
        (p.access_mode.as_str().to_owned(), p.access_project_id)
    };
    let current = effective_member_ids(
        conn,
        workspace_id,
        kind,
        &current_mode,
        current_anchor.as_deref(),
        &active_audience_ids(conn, workspace_id, kind, resource_id)?,
    )?;
    let proposed = effective_member_ids(conn, workspace_id, kind, mode, anchor, explicit)?;
    if !proposed.is_subset(&current) {
        return Ok(true);
    }
    for id in conversions {
        let page = page_access_row(conn, workspace_id, id)?;
        let current = effective_member_ids(
            conn,
            workspace_id,
            "page",
            page.access_mode.as_str(),
            page.access_project_id.as_deref(),
            &active_audience_ids(conn, workspace_id, "page", id)?,
        )?;
        if !proposed.is_subset(&current) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn mode_label(kind: &str, mode: &str) -> String {
    match (kind, mode) {
        ("project", "workspace") => "Workspace access".into(),
        ("project", "restricted") => "Selected members".into(),
        ("page", "workspace") => "Workspace access".into(),
        ("page", "project") => "Access follows project".into(),
        ("page", "selected") => "Selected members".into(),
        _ => "Workspace access".into(),
    }
}

pub(crate) fn policy_view(
    conn: &Connection,
    workspace_id: &str,
    kind: &str,
    resource_id: &str,
    ctx: &CallerAccess,
) -> Result<ResourceAccessPolicyView, SharedContentError> {
    let (mode, anchor, version, follow_name, followers): (
        String,
        Option<String>,
        i64,
        Option<String>,
        Vec<FollowingPageEntry>,
    ) = match kind {
        "project" => {
            let policy = project_access_row(conn, workspace_id, resource_id)?;
            if !project_readable(conn, workspace_id, resource_id, ctx)? {
                return Err(SharedContentError::ProjectNotFound);
            }
            (
                policy.access_mode.as_str().into(),
                None,
                policy.access_version,
                None,
                project_access_pages(conn, workspace_id, resource_id, ctx)?,
            )
        }
        "page" => {
            let policy = page_access_row(conn, workspace_id, resource_id)?;
            if !page_readable(conn, workspace_id, resource_id, ctx)? {
                return Err(SharedContentError::PageNotFound);
            }
            let name = match policy.access_project_id.as_deref() {
                Some(id) => conn
                    .query_row(
                        "SELECT name FROM shared_projects WHERE workspace_id = ?1 AND project_id = ?2",
                        params![workspace_id, id],
                        |r| r.get::<_, String>(0),
                    )
                    .optional()?,
                None => None,
            };
            (
                policy.access_mode.as_str().into(),
                policy.access_project_id,
                policy.access_version,
                name,
                Vec::new(),
            )
        }
        _ => {
            return Err(SharedContentError::InvalidInput(
                "Unknown resource kind.".into(),
            ))
        }
    };
    let audience = audience_members(conn, workspace_id, kind, resource_id)?;
    Ok(ResourceAccessPolicyView {
        resource_kind: kind.into(),
        resource_id: resource_id.into(),
        access_mode: mode,
        access_project_id: anchor,
        access_project_name: follow_name,
        access_version: version,
        audience,
        can_manage: ctx.can_manage_access(),
        administrative_access: ctx.is_privileged(),
        administrative_disclosure: ADMIN_ACCESS_DISCLOSURE.into(),
        following_pages: followers,
    })
}

fn normalize_membership_selection(
    conn: &Connection,
    workspace_id: &str,
    requested: &[String],
) -> Result<Vec<AccessAudienceMember>, SharedContentError> {
    let active = list_active_members(conn, workspace_id)?;
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for id in requested {
        let id = id.trim();
        if id.is_empty() || !seen.insert(id.to_owned()) {
            continue;
        }
        let Some(member) = active.iter().find(|m| m.membership_id == id) else {
            return Err(SharedContentError::InvalidInput(
                "Every selected member must be an active member of this workspace.".into(),
            ));
        };
        out.push(member.clone());
    }
    Ok(out)
}

fn dependency_digest(
    conn: &Connection,
    workspace_id: &str,
    affected: &[FollowingPageEntry],
    anchor: Option<&str>,
    anchor_version: Option<i64>,
) -> Result<String, SharedContentError> {
    let mut parts: Vec<String> = affected
        .iter()
        .map(|p| {
            format!(
                "{}:{}:{}",
                p.page_id,
                p.access_version,
                p.access_mode.as_str()
            )
        })
        .collect();
    for page in affected {
        parts.push(audience_digest(conn, workspace_id, "page", &page.page_id)?);
    }
    // Bind prospective audiences too: joining/removal or a role change during preview
    // must not change the meaning of a later confirmation.
    for member in list_active_members(conn, workspace_id)? {
        parts.push(format!(
            "member:{}:{}",
            member.membership_id,
            member.role.as_str()
        ));
    }
    parts.sort();
    if let (Some(id), Some(version)) = (anchor, anchor_version) {
        parts.push(format!(
            "anchor:{id}:{version}:{}",
            audience_digest(conn, workspace_id, "project", id)?
        ));
    }
    Ok(sha256_hex(&parts.join("|")))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn preview_change(
    conn: &mut Connection,
    caller: &VerifiedSession,
    session_id: &str,
    workspace_id: &str,
    kind: &str,
    resource_id: &str,
    input: &AccessChangePreviewInput,
    now_ms: i64,
) -> Result<AccessChangePreview, SharedContentError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let ctx = require_caller_access(&tx, workspace_id, &caller.uid)?;
    if !ctx.can_manage_access() {
        return Err(SharedContentError::Forbidden);
    }
    let (current_mode, current_version, current_anchor, current_pages) = match kind {
        "project" => {
            let policy = project_access_row(&tx, workspace_id, resource_id)?;
            (
                policy.access_mode.as_str().to_owned(),
                policy.access_version,
                None,
                project_access_pages(&tx, workspace_id, resource_id, &ctx)?,
            )
        }
        "page" => {
            let policy = page_access_row(&tx, workspace_id, resource_id)?;
            (
                policy.access_mode.as_str().to_owned(),
                policy.access_version,
                policy.access_project_id.clone(),
                Vec::new(),
            )
        }
        _ => {
            return Err(SharedContentError::InvalidInput(
                "Unknown resource kind.".into(),
            ))
        }
    };
    let proposed_mode = input.proposed_mode.trim();
    let mut proposed_ids: Vec<String> = Vec::new();
    let mut follow_project: Option<String> = None;
    match (kind, proposed_mode) {
        ("project", "workspace") | ("page", "workspace") => {}
        ("project", "restricted") | ("page", "selected") => {
            let members =
                normalize_membership_selection(&tx, workspace_id, &input.selected_membership_ids)?;
            proposed_ids = members.into_iter().map(|m| m.membership_id).collect();
        }
        ("page", "project") => {
            let anchor = input
                .follow_project_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    SharedContentError::InvalidInput(
                        "Choose the project this page should follow.".into(),
                    )
                })?;
            project_access_row(&tx, workspace_id, anchor)?;
            follow_project = Some(anchor.to_owned());
        }
        _ => {
            return Err(SharedContentError::InvalidInput(
                "Unsupported access mode for this resource.".into(),
            ))
        }
    }

    let mut conversion_page_ids: Vec<String> = Vec::new();
    if kind == "project" {
        let attached = project_pages(&tx, workspace_id, resource_id)?;
        for page_id in &input.convert_page_ids {
            let page_id = page_id.trim();
            let Some(page) = attached.iter().find(|p| p.page_id == page_id) else {
                return Err(SharedContentError::InvalidInput(
                    "Only pages attached to this project can be converted to follow it.".into(),
                ));
            };
            let already_follows = page.access_mode == PageAccessMode::Project
                && page.access_project_id.as_deref() == Some(resource_id);
            if !already_follows && !conversion_page_ids.iter().any(|id| id == page_id) {
                conversion_page_ids.push(page_id.to_owned());
            }
        }
    }

    let current_effective = effective_member_ids(
        &tx,
        workspace_id,
        kind,
        &current_mode,
        current_anchor.as_deref(),
        &active_audience_ids(&tx, workspace_id, kind, resource_id)?,
    )?;
    let proposed_effective = effective_member_ids(
        &tx,
        workspace_id,
        kind,
        proposed_mode,
        follow_project.as_deref(),
        &proposed_ids,
    )?;
    let expansion = change_expands_audience(
        &tx,
        workspace_id,
        kind,
        resource_id,
        proposed_mode,
        &proposed_ids,
        follow_project.as_deref(),
        &conversion_page_ids,
    )?;
    let all_members = list_active_members(&tx, workspace_id)?;
    let proposed_audience: Vec<AccessAudienceMember> = all_members
        .iter()
        .filter(|m| proposed_effective.contains(&m.membership_id))
        .cloned()
        .collect();
    let current_audience: Vec<AccessAudienceMember> = all_members
        .iter()
        .filter(|m| current_effective.contains(&m.membership_id))
        .cloned()
        .collect();
    let added = proposed_audience
        .iter()
        .filter(|m| !current_effective.contains(&m.membership_id))
        .cloned()
        .collect();
    let removed = current_audience
        .iter()
        .filter(|m| !proposed_effective.contains(&m.membership_id))
        .cloned()
        .collect();

    let anchor_version = match follow_project.as_deref() {
        Some(id) => Some(project_access_row(&tx, workspace_id, id)?.access_version),
        None => None,
    };
    let follow_name = match follow_project.as_deref() {
        Some(id) => tx
            .query_row(
                "SELECT name FROM shared_projects WHERE workspace_id = ?1 AND project_id = ?2",
                params![workspace_id, id],
                |r| r.get::<_, String>(0),
            )
            .optional()?,
        None => None,
    };
    let affected_pages = if kind == "project" {
        current_pages
    } else {
        Vec::new()
    };
    let base_audience_digest = audience_digest(&tx, workspace_id, kind, resource_id)?;
    let dep_digest = dependency_digest(
        &tx,
        workspace_id,
        &affected_pages,
        follow_project.as_deref(),
        anchor_version,
    )?;
    let preview_id = format!("sha_{}", Uuid::new_v4().simple());
    let expires_at_ms = now_ms + SHARING_PREVIEW_TTL_MS;
    let fingerprint = sha256_hex(
        &serde_json::to_string(&serde_json::json!({
            "resource": format!("{kind}:{resource_id}"),
            "mode": proposed_mode,
            "ids": proposed_ids,
            "anchor": follow_project,
            "convert": conversion_page_ids,
        }))
        .map_err(|_| SharedContentError::InvalidInput("Invalid preview data.".into()))?,
    );

    tx.execute(
        "INSERT INTO shared_sharing_previews
         (preview_id, workspace_id, actor_uid, actor_membership_id, actor_role,
          actor_membership_version, session_id, resource_kind,
          resource_id, base_access_version, base_audience_digest, proposed_mode,
          proposed_project_id, proposed_membership_ids_json, conversion_page_ids_json,
          affected_pages_json, dependency_digest, request_fingerprint, created_at_ms, expires_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            preview_id,
            workspace_id,
            caller.uid,
            ctx.membership_id,
            ctx.role.as_str(),
            ctx.membership_version,
            session_id,
            kind,
            resource_id,
            current_version,
            base_audience_digest,
            proposed_mode,
            follow_project,
            serde_json::to_string(&proposed_ids).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(&conversion_page_ids).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(&affected_pages).unwrap_or_else(|_| "[]".into()),
            dep_digest,
            fingerprint,
            now_ms,
            expires_at_ms,
        ],
    )?;
    tx.commit()?;
    Ok(AccessChangePreview {
        preview_id,
        resource_kind: kind.into(),
        resource_id: resource_id.into(),
        current_mode: mode_label(kind, &current_mode),
        proposed_mode: mode_label(kind, proposed_mode),
        current_audience,
        proposed_audience,
        audience_change: AccessAudienceChange { added, removed },
        follow_project_id: follow_project,
        follow_project_name: follow_name,
        affected_pages,
        conversion_page_ids,
        administrative_disclosure: ADMIN_ACCESS_DISCLOSURE.into(),
        shares_history: "Sharing includes the page's readable revision history, discussions, proposals and other existing history, not only the current body.".into(),
        expansion_requires_confirmation: expansion,
        expires_at_ms,
        base_access_version: current_version,
    })
}

struct PreviewRow {
    actor_uid: String,
    actor_membership_id: String,
    actor_role: String,
    actor_membership_version: i64,
    session_id: String,
    proposed_mode: String,
    proposed_project_id: Option<String>,
    proposed_membership_ids: Vec<String>,
    conversion_page_ids: Vec<String>,
    affected_pages: Vec<FollowingPageEntry>,
    base_access_version: i64,
    base_audience_digest: String,
    dependency_digest: String,
    request_fingerprint: String,
    expires_at_ms: i64,
    committed_at_ms: Option<i64>,
}

fn load_preview(
    conn: &Connection,
    workspace_id: &str,
    kind: &str,
    resource_id: &str,
    preview_id: &str,
) -> Result<PreviewRow, SharedContentError> {
    let row = conn
        .query_row(
            "SELECT actor_uid, actor_membership_id, actor_role, actor_membership_version, session_id, proposed_mode,
                    proposed_project_id, proposed_membership_ids_json, conversion_page_ids_json,
                    affected_pages_json, base_access_version, base_audience_digest,
                    dependency_digest, request_fingerprint, expires_at_ms, committed_at_ms
               FROM shared_sharing_previews
              WHERE preview_id = ?1 AND workspace_id = ?2 AND resource_kind = ?3
                AND resource_id = ?4",
            params![preview_id, workspace_id, kind, resource_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, String>(8)?,
                    r.get::<_, String>(9)?,
                    r.get::<_, i64>(10)?,
                    r.get::<_, String>(11)?,
                    r.get::<_, String>(12)?,
                    r.get::<_, String>(13)?,
                    r.get::<_, i64>(14)?,
                    r.get::<_, Option<i64>>(15)?,
                ))
            },
        )
        .optional()?;
    let Some((
        actor_uid,
        actor_membership_id,
        actor_role,
        actor_membership_version,
        session_id,
        proposed_mode,
        proposed_project_id,
        ids_json,
        conversions_json,
        pages_json,
        base_access_version,
        base_audience_digest,
        dependency_digest,
        request_fingerprint,
        expires_at_ms,
        committed_at_ms,
    )) = row
    else {
        return Err(SharedContentError::BackupNotFound);
    };
    Ok(PreviewRow {
        actor_uid,
        actor_membership_id,
        actor_role,
        actor_membership_version,
        session_id,
        proposed_mode,
        proposed_project_id,
        proposed_membership_ids: serde_json::from_str(&ids_json).unwrap_or_default(),
        conversion_page_ids: serde_json::from_str(&conversions_json).unwrap_or_default(),
        affected_pages: serde_json::from_str(&pages_json).unwrap_or_default(),
        base_access_version,
        base_audience_digest,
        dependency_digest,
        request_fingerprint,
        expires_at_ms,
        committed_at_ms,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn commit_change(
    conn: &mut Connection,
    caller: &VerifiedSession,
    recheck_session: SessionRecheck<'_>,
    workspace_id: &str,
    kind: &str,
    resource_id: &str,
    input: &AccessChangeCommitInput,
    now_ms: i64,
) -> Result<AccessChangeOutcome, SharedContentError> {
    let idempotency_key = sanitize_idempotency_key(&input.idempotency_key)?;
    let operation = format!("commit_access:{kind}:{resource_id}");
    let payload_hash = sha256_hex(&format!(
        "{kind}:{resource_id}:{}:{}",
        input.preview_id, input.confirm_expansion
    ));
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let ctx = require_caller_access(&tx, workspace_id, &caller.uid)?;
    if !ctx.can_manage_access() {
        return Err(SharedContentError::Forbidden);
    }
    let preview = load_preview(&tx, workspace_id, kind, resource_id, &input.preview_id)?;
    if preview.actor_uid != caller.uid
        || preview.actor_membership_id != ctx.membership_id
        || preview.actor_role != ctx.role.as_str()
        || preview.actor_membership_version != ctx.membership_version
        || preview.session_id != caller.session_id
    {
        return Err(SharedContentError::StaleMembership(
            "This sharing preview belongs to another membership session.".into(),
        ));
    }
    let _ = recheck_session()?;

    if let Some((stored_hash, response_json)) = tx
        .query_row(
            "SELECT payload_sha256, response_json FROM shared_write_idempotency
              WHERE actor_uid = ?1 AND workspace_id = ?2 AND operation = ?3
                AND idempotency_key = ?4",
            params![caller.uid, workspace_id, operation, idempotency_key],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?
    {
        if stored_hash != payload_hash {
            return Err(SharedContentError::IdempotencyConflict);
        }
        let outcome: AccessChangeOutcome = serde_json::from_str(&response_json)
            .map_err(|_| SharedContentError::IdempotencyConflict)?;
        tx.commit()?;
        return Ok(outcome);
    }

    if now_ms > preview.expires_at_ms {
        return Err(SharedContentError::BackupExpired);
    }
    if preview.committed_at_ms.is_some() {
        return Err(SharedContentError::BackupConflict(
            "This sharing preview was already used.".into(),
        ));
    }
    let current_mode = match kind {
        "project" => project_access_row(&tx, workspace_id, resource_id)?
            .access_mode
            .as_str()
            .to_owned(),
        "page" => page_access_row(&tx, workspace_id, resource_id)?
            .access_mode
            .as_str()
            .to_owned(),
        _ => {
            return Err(SharedContentError::InvalidInput(
                "Unknown resource kind.".into(),
            ))
        }
    };
    let current_version = match kind {
        "project" => project_access_row(&tx, workspace_id, resource_id)?.access_version,
        _ => page_access_row(&tx, workspace_id, resource_id)?.access_version,
    };
    if preview.base_access_version != current_version
        || preview.base_audience_digest != audience_digest(&tx, workspace_id, kind, resource_id)?
    {
        return Err(SharedContentError::StaleMembership(
            "The access policy changed after this preview. Preview the change again.".into(),
        ));
    }
    for page in &preview.affected_pages {
        let current = page_access_row(&tx, workspace_id, &page.page_id)?;
        if current.access_version != page.access_version
            || current.access_mode != page.access_mode
            || current.access_project_id != page.access_project_id
        {
            return Err(SharedContentError::StaleMembership(
                "A page that depends on this project changed after the preview. Preview the change again."
                    .into(),
            ));
        }
    }
    let anchor_version = match preview.proposed_project_id.as_deref() {
        Some(id) => Some(project_access_row(&tx, workspace_id, id)?.access_version),
        None => None,
    };
    if preview.dependency_digest
        != dependency_digest(
            &tx,
            workspace_id,
            &preview.affected_pages,
            preview.proposed_project_id.as_deref(),
            anchor_version,
        )?
    {
        return Err(SharedContentError::StaleMembership(
            "A dependent resource or project audience changed after the preview. Preview the change again."
                .into(),
        ));
    }
    if change_expands_audience(
        &tx,
        workspace_id,
        kind,
        resource_id,
        &preview.proposed_mode,
        &preview.proposed_membership_ids,
        preview.proposed_project_id.as_deref(),
        &preview.conversion_page_ids,
    )? && !input.confirm_expansion
    {
        return Err(SharedContentError::InvalidInput(
            "Confirm that this change shares the resource or a converted page with more members."
                .into(),
        ));
    }
    let _ = recheck_session()?;

    let mut changed_page_ids: Vec<String> = Vec::new();
    let target_mode = preview.proposed_mode.as_str();
    if !matches!(target_mode, "restricted" | "selected") {
        tx.execute(
            "UPDATE shared_resource_audience
                SET revoked_at_ms = ?1, revoked_by_uid = ?2
              WHERE workspace_id = ?3 AND resource_kind = ?4 AND resource_id = ?5
                AND revoked_at_ms IS NULL",
            params![now_ms, caller.uid, workspace_id, kind, resource_id],
        )?;
    } else {
        let mut stmt = tx.prepare(
            "SELECT membership_id FROM shared_resource_audience
              WHERE workspace_id = ?1 AND resource_kind = ?2 AND resource_id = ?3
                AND revoked_at_ms IS NULL",
        )?;
        let rows = stmt.query_map(params![workspace_id, kind, resource_id], |r| {
            r.get::<_, String>(0)
        })?;
        let mut revoke = Vec::new();
        for r in rows {
            let id = r?;
            if !preview.proposed_membership_ids.iter().any(|k| k == &id) {
                revoke.push(id);
            }
        }
        drop(stmt);
        for id in revoke {
            tx.execute(
                "UPDATE shared_resource_audience
                    SET revoked_at_ms = ?1, revoked_by_uid = ?2
                  WHERE workspace_id = ?3 AND resource_kind = ?4 AND resource_id = ?5
                    AND membership_id = ?6 AND revoked_at_ms IS NULL",
                params![now_ms, caller.uid, workspace_id, kind, resource_id, id],
            )?;
        }
        let active_members = list_active_members(&tx, workspace_id)?;
        for id in &preview.proposed_membership_ids {
            let Some(member) = active_members.iter().find(|m| &m.membership_id == id) else {
                return Err(SharedContentError::InvalidInput(
                    "A selected member is no longer active in this workspace. Preview the change again."
                        .into(),
                ));
            };
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT 1 FROM shared_resource_audience
                      WHERE workspace_id = ?1 AND resource_kind = ?2 AND resource_id = ?3
                        AND membership_id = ?4 AND revoked_at_ms IS NULL",
                    params![workspace_id, kind, resource_id, id],
                    |r| r.get(0),
                )
                .optional()?;
            if existing.is_none() {
                tx.execute(
                    "INSERT INTO shared_resource_audience
                     (audience_id, workspace_id, resource_kind, resource_id, membership_id,
                      grant_generation, granted_by_uid, granted_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        format!("aud_{}", Uuid::new_v4().simple()),
                        workspace_id,
                        kind,
                        resource_id,
                        id,
                        member.uid,
                        caller.uid,
                        now_ms,
                    ],
                )?;
            }
        }
    }

    if kind == "project" {
        tx.execute(
            "UPDATE shared_projects SET access_mode = ?1, access_version = access_version + 1
              WHERE workspace_id = ?2 AND project_id = ?3",
            params![target_mode, workspace_id, resource_id],
        )?;
        let mut stmt = tx.prepare(
            "SELECT page_id FROM shared_pages
              WHERE workspace_id = ?1 AND access_mode = 'project' AND access_project_id = ?2",
        )?;
        let rows = stmt.query_map(params![workspace_id, resource_id], |r| {
            r.get::<_, String>(0)
        })?;
        let mut following = Vec::new();
        for r in rows {
            following.push(r?);
        }
        drop(stmt);
        tx.execute(
            "UPDATE shared_pages SET access_version = access_version + 1
              WHERE workspace_id = ?1 AND access_mode = 'project' AND access_project_id = ?2",
            params![workspace_id, resource_id],
        )?;
        changed_page_ids.extend(following);
        for page_id in &preview.conversion_page_ids {
            let already_follows: Option<i64> = tx
                .query_row(
                    "SELECT 1 FROM shared_pages
                      WHERE workspace_id = ?1 AND page_id = ?2
                        AND access_mode = 'project' AND access_project_id = ?3",
                    params![workspace_id, page_id, resource_id],
                    |r| r.get(0),
                )
                .optional()?;
            if already_follows.is_some() {
                continue;
            }
            let changed = tx.execute(
                "UPDATE shared_pages
                    SET access_mode = 'project', access_project_id = ?1,
                        access_version = access_version + 1
                  WHERE workspace_id = ?2 AND page_id = ?3",
                params![resource_id, workspace_id, page_id],
            )?;
            if changed != 1 {
                return Err(SharedContentError::PageNotFound);
            }
            if !changed_page_ids.iter().any(|id| id == page_id) {
                changed_page_ids.push(page_id.clone());
            }
        }
    } else {
        let (mode, anchor) = match target_mode {
            "workspace" => ("workspace", None),
            "selected" => ("selected", None),
            "project" => ("project", preview.proposed_project_id.clone()),
            _ => {
                return Err(SharedContentError::InvalidInput(
                    "Unsupported access mode for this resource.".into(),
                ))
            }
        };
        tx.execute(
            "UPDATE shared_pages SET access_mode = ?1, access_project_id = ?2,
                    access_version = access_version + 1
              WHERE workspace_id = ?3 AND page_id = ?4",
            params![mode, anchor, workspace_id, resource_id],
        )?;
        changed_page_ids.push(resource_id.to_owned());
    }

    tx.execute(
        "UPDATE shared_sharing_previews SET committed_at_ms = ?1 WHERE preview_id = ?2",
        params![now_ms, input.preview_id],
    )?;
    let audit_detail = serde_json::json!({
        "previewId": input.preview_id,
        "requestFingerprint": preview.request_fingerprint,
        "baseAccessVersion": preview.base_access_version,
        "proposedAudience": preview.proposed_membership_ids,
        "changedPages": changed_page_ids,
        "proposedProjectId": preview.proposed_project_id,
    });
    audit(
        &tx,
        workspace_id,
        kind,
        resource_id,
        &ctx,
        "access_policy_changed",
        &current_mode,
        target_mode,
        audit_detail,
        now_ms,
    )?;
    let policy = policy_view(&tx, workspace_id, kind, resource_id, &ctx)?;
    let outcome = AccessChangeOutcome {
        committed: true,
        policy,
        changed_page_ids,
    };
    crate::shared_content::insert_realtime_event(
        &tx,
        workspace_id,
        "resource_access_changed",
        None,
        None,
        None,
        None,
        None,
        &serde_json::json!({}),
        now_ms,
    )?;
    let response_json = serde_json::to_string(&outcome)
        .map_err(|_| SharedContentError::InvalidInput("Invalid access response.".into()))?;
    tx.execute(
        "INSERT INTO shared_write_idempotency
         (actor_uid, workspace_id, operation, idempotency_key, payload_sha256, response_json, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            caller.uid,
            workspace_id,
            operation,
            idempotency_key,
            payload_hash,
            response_json,
            now_ms
        ],
    )?;
    tx.commit()?;
    Ok(outcome)
}
