use crate::store::{ct_eq_str, sha256_hex, StoreError, VerifiedSession};
use crate::verify::resolve_display_name;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use thiserror::Error;
use uuid::Uuid;

pub const DEFAULT_INVITE_TTL_MS: i64 = 7 * 24 * 60 * 60 * 1000;
pub const INVITE_RATE_LIMIT_WINDOW_MS: i64 = 60 * 1000;
pub const INVITE_RATE_LIMIT_MAX_ATTEMPTS: usize = 20;
const MAX_WORKSPACE_NAME_LEN: usize = 80;
const MAX_EMAIL_LEN: usize = 254;
const MAX_IDEMPOTENCY_KEY_LEN: usize = 128;

pub type SessionRecheck<'a> = &'a (dyn Fn() -> Result<VerifiedSession, StoreError> + Sync);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceRole {
    Owner,
    Admin,
    Editor,
    Viewer,
}

impl WorkspaceRole {
    pub fn as_str(self) -> &'static str {
        match self {
            WorkspaceRole::Owner => "owner",
            WorkspaceRole::Admin => "admin",
            WorkspaceRole::Editor => "editor",
            WorkspaceRole::Viewer => "viewer",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, MembershipError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "owner" => Ok(WorkspaceRole::Owner),
            "admin" => Ok(WorkspaceRole::Admin),
            "editor" => Ok(WorkspaceRole::Editor),
            "viewer" => Ok(WorkspaceRole::Viewer),
            _ => Err(MembershipError::InvalidInput(
                "Unknown workspace role.".into(),
            )),
        }
    }

    pub fn can_rename(self) -> bool {
        matches!(self, WorkspaceRole::Owner | WorkspaceRole::Admin)
    }

    pub fn can_grant_invite_role(self, target: WorkspaceRole) -> bool {
        match self {
            WorkspaceRole::Owner => matches!(
                target,
                WorkspaceRole::Admin | WorkspaceRole::Editor | WorkspaceRole::Viewer
            ),
            WorkspaceRole::Admin => matches!(target, WorkspaceRole::Editor | WorkspaceRole::Viewer),
            WorkspaceRole::Editor | WorkspaceRole::Viewer => false,
        }
    }

    pub fn can_manage_invitation(self, offered: WorkspaceRole) -> bool {
        self.can_grant_invite_role(offered)
    }

    pub fn can_change_member_role(
        self,
        current_target_role: WorkspaceRole,
        new_target_role: WorkspaceRole,
    ) -> bool {
        if current_target_role == WorkspaceRole::Owner || new_target_role == WorkspaceRole::Owner {
            return false;
        }
        match self {
            WorkspaceRole::Owner => matches!(
                new_target_role,
                WorkspaceRole::Admin | WorkspaceRole::Editor | WorkspaceRole::Viewer
            ),
            WorkspaceRole::Admin => {
                matches!(
                    current_target_role,
                    WorkspaceRole::Editor | WorkspaceRole::Viewer
                ) && matches!(
                    new_target_role,
                    WorkspaceRole::Editor | WorkspaceRole::Viewer
                )
            }
            WorkspaceRole::Editor | WorkspaceRole::Viewer => false,
        }
    }

    pub fn can_remove_member(self, target_role: WorkspaceRole) -> bool {
        if target_role == WorkspaceRole::Owner {
            return false;
        }
        match self {
            WorkspaceRole::Owner => matches!(
                target_role,
                WorkspaceRole::Admin | WorkspaceRole::Editor | WorkspaceRole::Viewer
            ),
            WorkspaceRole::Admin => {
                matches!(target_role, WorkspaceRole::Editor | WorkspaceRole::Viewer)
            }
            WorkspaceRole::Editor | WorkspaceRole::Viewer => false,
        }
    }

    pub fn can_leave(self) -> bool {
        matches!(
            self,
            WorkspaceRole::Admin | WorkspaceRole::Editor | WorkspaceRole::Viewer
        )
    }

    pub fn can_author_shared_content(self) -> bool {
        matches!(
            self,
            WorkspaceRole::Owner | WorkspaceRole::Admin | WorkspaceRole::Editor
        )
    }

    pub fn can_moderate_or_review(self) -> bool {
        matches!(self, WorkspaceRole::Owner | WorkspaceRole::Admin)
    }

    pub fn can_archive_shared_page(self) -> bool {
        self.can_moderate_or_review()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InvitationStatus {
    Pending,
    Accepted,
    Expired,
    Revoked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferStatus {
    Pending,
    Accepted,
    Cancelled,
}

#[derive(Debug, Error)]
pub enum MembershipError {
    #[error("{0}")]
    InvalidInput(String),
    #[error("Authentication required: {0}")]
    Session(#[from] StoreError),
    #[error("Workspace was not found.")]
    WorkspaceNotFound,
    #[error("You do not have permission to perform this workspace action.")]
    Forbidden,
    #[error("Member was not found in this workspace.")]
    MemberNotFound,
    #[error("The workspace must keep an Owner. Transfer ownership before leaving.")]
    LastOwnerProtected,
    #[error("That invitation code was not found.")]
    InvitationNotFound,
    #[error(
        "This invitation belongs to a different Google account. Switch accounts and try again."
    )]
    WrongAccount,
    #[error("This invitation has already been used.")]
    InvitationAlreadyUsed,
    #[error("This invitation has expired. Ask a workspace manager for a new invitation.")]
    InvitationExpired,
    #[error("This invitation is no longer active.")]
    InvitationRevoked,
    #[error("Too many invitation attempts. Wait a moment and try again.")]
    RateLimited,
    #[error("An ownership transfer is already pending for this workspace.")]
    TransferAlreadyPending,
    #[error("That ownership transfer is no longer pending.")]
    TransferNotPending,
    #[error("Workspace membership store unavailable.")]
    Storage(#[from] rusqlite::Error),
    #[error("Random generator unavailable.")]
    Random,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummary {
    pub workspace_id: String,
    pub name: String,
    pub my_membership_id: String,
    pub my_role: WorkspaceRole,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceOutcome {
    pub created: bool,
    pub workspace: WorkspaceSummary,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMemberView {
    pub membership_id: String,
    pub uid: String,
    pub display_name: String,
    pub role: WorkspaceRole,
    pub joined_at_ms: i64,
    pub is_current_user: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInvitationView {
    pub invitation_id: String,
    pub workspace_id: String,
    pub recipient_email: String,
    pub offered_role: WorkspaceRole,
    pub status: InvitationStatus,
    pub invited_by_membership_id: String,
    pub invited_by_display_name: String,
    pub created_at_ms: i64,
    pub expires_at_ms: i64,
    pub accepted_at_ms: Option<i64>,
    pub revoked_at_ms: Option<i64>,
}

impl fmt::Debug for WorkspaceInvitationView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkspaceInvitationView")
            .field("invitation_id", &self.invitation_id)
            .field("workspace_id", &self.workspace_id)
            .field("recipient_email", &"[redacted]")
            .field("offered_role", &self.offered_role)
            .field("status", &self.status)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedInvitation {
    pub invitation: WorkspaceInvitationView,
    pub invite_code: String,
}

impl fmt::Debug for CreatedInvitation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CreatedInvitation")
            .field("invitation", &self.invitation)
            .field("invite_code", &"[redacted]")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitationPreview {
    pub invitation_id: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub offered_role: WorkspaceRole,
    pub expires_at_ms: i64,
    pub already_member: bool,
    pub existing_role: Option<WorkspaceRole>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptInvitationOutcome {
    pub already_member: bool,
    pub workspace: WorkspaceSummary,
    pub membership: WorkspaceMemberView,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipTransferView {
    pub transfer_id: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub from_membership_id: String,
    pub from_uid: String,
    pub from_display_name: String,
    pub to_membership_id: String,
    pub to_uid: String,
    pub to_display_name: String,
    pub status: TransferStatus,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDetails {
    pub workspace: WorkspaceSummary,
    pub members: Vec<WorkspaceMemberView>,
    pub invitations: Vec<WorkspaceInvitationView>,
    pub pending_transfer: Option<OwnershipTransferView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceListResponse {
    pub workspaces: Vec<WorkspaceSummary>,
    pub incoming_transfers: Vec<OwnershipTransferView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceAuditRecord {
    pub log_id: String,
    pub workspace_id: String,
    pub action: String,
    pub actor_uid: String,
    pub actor_membership_id: Option<String>,
    pub target_uid: Option<String>,
    pub target_membership_id: Option<String>,
    pub invitation_id: Option<String>,
    pub detail_json: String,
    pub created_at_ms: i64,
}

/// Persistence contract independent of the future production shared-workspace database.
pub trait WorkspaceMembershipStore: Send + Sync {
    fn list_workspaces(
        &self,
        caller: &VerifiedSession,
        now_ms: i64,
    ) -> Result<WorkspaceListResponse, MembershipError>;

    fn create_workspace(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        name: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<CreateWorkspaceOutcome, MembershipError>;

    fn get_workspace_details(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError>;

    fn rename_workspace(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        name: &str,
        now_ms: i64,
    ) -> Result<WorkspaceSummary, MembershipError>;

    #[allow(clippy::too_many_arguments)]
    fn create_invitation(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        recipient_email: &str,
        offered_role: WorkspaceRole,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<CreatedInvitation, MembershipError>;

    fn revoke_invitation(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        invitation_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceInvitationView, MembershipError>;

    fn replace_invitation(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        invitation_id: &str,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<CreatedInvitation, MembershipError>;

    fn preview_invitation(
        &self,
        caller: &VerifiedSession,
        invite_code: &str,
        now_ms: i64,
    ) -> Result<InvitationPreview, MembershipError>;

    fn accept_invitation(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        invite_code: &str,
        now_ms: i64,
    ) -> Result<AcceptInvitationOutcome, MembershipError>;

    fn change_member_role(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        target_membership_id: &str,
        new_role: WorkspaceRole,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError>;

    fn remove_member(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        target_membership_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError>;

    fn leave_workspace(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        now_ms: i64,
    ) -> Result<(), MembershipError>;

    fn initiate_ownership_transfer(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        to_membership_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError>;

    fn cancel_ownership_transfer(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        transfer_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError>;

    fn accept_ownership_transfer(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        transfer_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError>;

    fn list_audit_records(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
    ) -> Result<Vec<WorkspaceAuditRecord>, MembershipError>;
}

#[derive(Clone)]
pub struct SqliteWorkspaceMembershipStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteWorkspaceMembershipStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MembershipError> {
        if let Some(parent) = path.as_ref().parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self, MembershipError> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self, MembershipError> {
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS workspaces (
                workspace_id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_by_uid TEXT NOT NULL,
                create_idempotency_key TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                UNIQUE (created_by_uid, create_idempotency_key)
            );

            CREATE TABLE IF NOT EXISTS workspace_memberships (
                membership_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
                uid TEXT NOT NULL,
                display_name TEXT NOT NULL,
                role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'editor', 'viewer')),
                joined_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                removed_at_ms INTEGER,
                removed_by_uid TEXT
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_workspace_active_member
                ON workspace_memberships (workspace_id, uid) WHERE removed_at_ms IS NULL;
            CREATE UNIQUE INDEX IF NOT EXISTS idx_workspace_single_owner
                ON workspace_memberships (workspace_id) WHERE role = 'owner' AND removed_at_ms IS NULL;

            CREATE TABLE IF NOT EXISTS workspace_invitations (
                invitation_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
                code_digest TEXT NOT NULL UNIQUE,
                recipient_email TEXT NOT NULL,
                recipient_email_norm TEXT NOT NULL,
                offered_role TEXT NOT NULL CHECK (offered_role IN ('admin', 'editor', 'viewer')),
                invited_by_membership_id TEXT NOT NULL,
                invited_by_uid TEXT NOT NULL,
                invited_by_display_name TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL,
                expires_at_ms INTEGER NOT NULL,
                accepted_at_ms INTEGER,
                accepted_by_uid TEXT,
                accepted_membership_id TEXT,
                revoked_at_ms INTEGER,
                revoked_by_uid TEXT,
                revoked_reason TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_workspace_invitations_ws
                ON workspace_invitations (workspace_id, created_at_ms DESC);

            CREATE TABLE IF NOT EXISTS workspace_ownership_transfers (
                transfer_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
                from_membership_id TEXT NOT NULL,
                from_uid TEXT NOT NULL,
                from_display_name TEXT NOT NULL,
                to_membership_id TEXT NOT NULL,
                to_uid TEXT NOT NULL,
                to_display_name TEXT NOT NULL,
                status TEXT NOT NULL CHECK (status IN ('pending', 'accepted', 'cancelled')),
                created_at_ms INTEGER NOT NULL,
                decided_at_ms INTEGER
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_workspace_pending_transfer
                ON workspace_ownership_transfers (workspace_id) WHERE status = 'pending';

            CREATE TABLE IF NOT EXISTS workspace_rate_limits (
                scope TEXT NOT NULL,
                actor_uid TEXT NOT NULL,
                attempted_at_ms INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_workspace_rate_limits
                ON workspace_rate_limits (scope, actor_uid, attempted_at_ms);

            CREATE TABLE IF NOT EXISTS workspace_audit_logs (
                log_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                action TEXT NOT NULL,
                actor_uid TEXT NOT NULL,
                actor_membership_id TEXT,
                target_uid TEXT,
                target_membership_id TEXT,
                invitation_id TEXT,
                detail_json TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_workspace_audit_ws
                ON workspace_audit_logs (workspace_id, created_at_ms ASC);

            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at_ms INTEGER NOT NULL
            );
            ",
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO schema_migrations (version, name, applied_at_ms)
             VALUES (1, 'cw1_membership_v1', ?1)",
            params![crate::now_epoch_ms()],
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn schema_version(&self) -> Result<i64, MembershipError> {
        let conn = self.lock_conn();
        let ver: Option<i64> = conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })
            .optional()?
            .flatten();
        Ok(ver.unwrap_or(0))
    }

    pub(crate) fn conn(&self) -> &Arc<Mutex<Connection>> {
        &self.conn
    }

    pub(crate) fn lock_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn
            .lock()
            .expect("workspace membership store lock poisoned")
    }
}

impl WorkspaceMembershipStore for SqliteWorkspaceMembershipStore {
    fn list_workspaces(
        &self,
        caller: &VerifiedSession,
        _now_ms: i64,
    ) -> Result<WorkspaceListResponse, MembershipError> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let workspaces = list_caller_workspaces(&tx, &caller.uid)?;
        let incoming_transfers = list_incoming_transfers(&tx, &caller.uid)?;
        tx.commit()?;
        Ok(WorkspaceListResponse {
            workspaces,
            incoming_transfers,
        })
    }

    fn create_workspace(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        name: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<CreateWorkspaceOutcome, MembershipError> {
        let name = sanitize_workspace_name(name)?;
        let idempotency_key = sanitize_idempotency_key(idempotency_key)?;
        let actor_display = caller_display_name(caller);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some((existing_ws_id, existing_name, created_at_ms, updated_at_ms)) = tx
            .query_row(
                "SELECT workspace_id, name, created_at_ms, updated_at_ms
                 FROM workspaces WHERE created_by_uid = ?1 AND create_idempotency_key = ?2",
                params![caller.uid, idempotency_key],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?
        {
            let active = load_active_membership(&tx, &existing_ws_id, &caller.uid)?
                .ok_or(MembershipError::Forbidden)?;
            let _ = recheck_session()?;
            tx.commit()?;
            return Ok(CreateWorkspaceOutcome {
                created: false,
                workspace: WorkspaceSummary {
                    workspace_id: existing_ws_id,
                    name: existing_name,
                    my_membership_id: active.membership_id,
                    my_role: active.role,
                    created_at_ms,
                    updated_at_ms,
                },
            });
        }

        let workspace_id = Uuid::new_v4().to_string();
        let membership_id = Uuid::new_v4().to_string();

        tx.execute(
            "INSERT INTO workspaces (workspace_id, name, created_by_uid, create_idempotency_key, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![workspace_id, name, caller.uid, idempotency_key, now_ms],
        )?;
        tx.execute(
            "INSERT INTO workspace_memberships
                (membership_id, workspace_id, uid, display_name, role, joined_at_ms, updated_at_ms, removed_at_ms, removed_by_uid)
             VALUES (?1, ?2, ?3, ?4, 'owner', ?5, ?5, NULL, NULL)",
            params![membership_id, workspace_id, caller.uid, actor_display, now_ms],
        )?;
        record_audit(
            &tx,
            &workspace_id,
            "workspace_created",
            &caller.uid,
            Some(&membership_id),
            None,
            None,
            None,
            &serde_json::json!({ "role": "owner" }),
            now_ms,
        )?;

        let _ = recheck_session()?;
        tx.commit()?;

        Ok(CreateWorkspaceOutcome {
            created: true,
            workspace: WorkspaceSummary {
                workspace_id,
                name,
                my_membership_id: membership_id,
                my_role: WorkspaceRole::Owner,
                created_at_ms: now_ms,
                updated_at_ms: now_ms,
            },
        })
    }

    fn get_workspace_details(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let details = build_workspace_details(&tx, caller, workspace_id, now_ms)?;
        tx.commit()?;
        Ok(details)
    }

    fn rename_workspace(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        name: &str,
        now_ms: i64,
    ) -> Result<WorkspaceSummary, MembershipError> {
        let name = sanitize_workspace_name(name)?;
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let actor = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !actor.role.can_rename() {
            return Err(MembershipError::Forbidden);
        }
        tx.execute(
            "UPDATE workspaces SET name = ?2, updated_at_ms = ?3 WHERE workspace_id = ?1",
            params![workspace_id, name, now_ms],
        )?;
        record_audit(
            &tx,
            workspace_id,
            "workspace_renamed",
            &caller.uid,
            Some(&actor.membership_id),
            None,
            None,
            None,
            &serde_json::json!({ "role": actor.role.as_str() }),
            now_ms,
        )?;
        let summary = load_workspace_summary(&tx, workspace_id, &caller.uid)?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(summary)
    }

    fn create_invitation(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        recipient_email: &str,
        offered_role: WorkspaceRole,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<CreatedInvitation, MembershipError> {
        let (recipient_email, recipient_email_norm) = sanitize_email(recipient_email)?;
        if ttl_ms <= 0 {
            return Err(MembershipError::InvalidInput(
                "Invitation TTL must be positive.".into(),
            ));
        }
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let actor = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !actor.role.can_grant_invite_role(offered_role) {
            return Err(MembershipError::Forbidden);
        }

        let created = insert_invitation_in_tx(
            &tx,
            workspace_id,
            &actor,
            &recipient_email,
            &recipient_email_norm,
            offered_role,
            now_ms,
            ttl_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(created)
    }

    fn revoke_invitation(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        invitation_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceInvitationView, MembershipError> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let actor = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let inv = load_invitation_by_id(&tx, workspace_id, invitation_id)?;
        if !actor.role.can_manage_invitation(inv.offered_role) {
            return Err(MembershipError::Forbidden);
        }
        if inv.accepted_at_ms.is_some() {
            return Err(MembershipError::InvitationAlreadyUsed);
        }
        if inv.revoked_at_ms.is_none() && now_ms > inv.expires_at_ms {
            return Err(MembershipError::InvitationExpired);
        }
        if inv.revoked_at_ms.is_none() {
            tx.execute(
                "UPDATE workspace_invitations
                 SET revoked_at_ms = ?2, revoked_by_uid = ?3, revoked_reason = 'manager_revoked'
                 WHERE invitation_id = ?1 AND revoked_at_ms IS NULL",
                params![invitation_id, now_ms, caller.uid],
            )?;
            record_audit(
                &tx,
                workspace_id,
                "invitation_revoked",
                &caller.uid,
                Some(&actor.membership_id),
                None,
                None,
                Some(invitation_id),
                &serde_json::json!({ "offered_role": inv.offered_role.as_str(), "reason": "manager_revoked" }),
                now_ms,
            )?;
        }
        let updated = load_invitation_by_id(&tx, workspace_id, invitation_id)?.into_view(now_ms);
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(updated)
    }

    fn replace_invitation(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        invitation_id: &str,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<CreatedInvitation, MembershipError> {
        if ttl_ms <= 0 {
            return Err(MembershipError::InvalidInput(
                "Invitation TTL must be positive.".into(),
            ));
        }
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let actor = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let inv = load_invitation_by_id(&tx, workspace_id, invitation_id)?;
        if !actor.role.can_manage_invitation(inv.offered_role)
            || !actor.role.can_grant_invite_role(inv.offered_role)
        {
            return Err(MembershipError::Forbidden);
        }
        if inv.accepted_at_ms.is_some() {
            return Err(MembershipError::InvitationAlreadyUsed);
        }
        if inv.revoked_at_ms.is_none() {
            tx.execute(
                "UPDATE workspace_invitations
                 SET revoked_at_ms = ?2, revoked_by_uid = ?3, revoked_reason = 'replaced'
                 WHERE invitation_id = ?1 AND revoked_at_ms IS NULL",
                params![invitation_id, now_ms, caller.uid],
            )?;
            record_audit(
                &tx,
                workspace_id,
                "invitation_revoked",
                &caller.uid,
                Some(&actor.membership_id),
                None,
                None,
                Some(invitation_id),
                &serde_json::json!({ "offered_role": inv.offered_role.as_str(), "reason": "replaced" }),
                now_ms,
            )?;
        }
        let created = insert_invitation_in_tx(
            &tx,
            workspace_id,
            &actor,
            &inv.recipient_email,
            &inv.recipient_email_norm,
            inv.offered_role,
            now_ms,
            ttl_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(created)
    }

    fn preview_invitation(
        &self,
        caller: &VerifiedSession,
        invite_code: &str,
        now_ms: i64,
    ) -> Result<InvitationPreview, MembershipError> {
        let code = invite_code.trim();
        if code.is_empty() {
            return Err(MembershipError::InvalidInput(
                "Enter an invitation code.".into(),
            ));
        }
        let code_digest = sha256_hex(code);
        let caller_email_norm = canonical_email(&caller.email);

        let mut conn = self.lock_conn();
        // Record rate-limit attempts in an independent committed transaction so failed lookups still count.
        {
            let rl_tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let allowed = check_and_record_rate_limit(&rl_tx, "preview", &caller.uid, now_ms)?;
            rl_tx.commit()?;
            if !allowed {
                return Err(MembershipError::RateLimited);
            }
        }

        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some(inv) = load_invitation_by_digest(&tx, &code_digest)? else {
            return Err(MembershipError::InvitationNotFound);
        };
        if !ct_eq_str(&caller_email_norm, &inv.recipient_email_norm) {
            return Err(MembershipError::WrongAccount);
        }

        let existing_member = load_active_membership(&tx, &inv.workspace_id, &caller.uid)?;
        if let Some(accepted_by_uid) = &inv.accepted_by_uid {
            if accepted_by_uid == &caller.uid && existing_member.is_some() {
                let ws_name = load_workspace_name(&tx, &inv.workspace_id)?;
                tx.commit()?;
                return Ok(InvitationPreview {
                    invitation_id: inv.invitation_id,
                    workspace_id: inv.workspace_id,
                    workspace_name: ws_name,
                    offered_role: inv.offered_role,
                    expires_at_ms: inv.expires_at_ms,
                    already_member: true,
                    existing_role: existing_member.map(|m| m.role),
                });
            }
            return Err(MembershipError::InvitationAlreadyUsed);
        }

        if inv.revoked_at_ms.is_some() {
            return Err(MembershipError::InvitationRevoked);
        }
        if now_ms > inv.expires_at_ms {
            return Err(MembershipError::InvitationExpired);
        }

        if !issuer_retains_authority(&tx, &inv)? {
            tx.execute(
                "UPDATE workspace_invitations
                 SET revoked_at_ms = ?2, revoked_by_uid = ?3, revoked_reason = 'issuer_authority_lost'
                 WHERE invitation_id = ?1 AND revoked_at_ms IS NULL",
                params![inv.invitation_id, now_ms, caller.uid],
            )?;
            record_audit(
                &tx,
                &inv.workspace_id,
                "invitation_revoked",
                &caller.uid,
                None,
                Some(&inv.invited_by_uid),
                Some(&inv.invited_by_membership_id),
                Some(&inv.invitation_id),
                &serde_json::json!({ "offered_role": inv.offered_role.as_str(), "reason": "issuer_authority_lost" }),
                now_ms,
            )?;
            tx.commit()?;
            return Err(MembershipError::InvitationRevoked);
        }

        let ws_name = load_workspace_name(&tx, &inv.workspace_id)?;
        tx.commit()?;
        Ok(InvitationPreview {
            invitation_id: inv.invitation_id,
            workspace_id: inv.workspace_id,
            workspace_name: ws_name,
            offered_role: inv.offered_role,
            expires_at_ms: inv.expires_at_ms,
            already_member: existing_member.is_some(),
            existing_role: existing_member.map(|m| m.role),
        })
    }

    fn accept_invitation(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        invite_code: &str,
        now_ms: i64,
    ) -> Result<AcceptInvitationOutcome, MembershipError> {
        let code = invite_code.trim();
        if code.is_empty() {
            return Err(MembershipError::InvalidInput(
                "Enter an invitation code.".into(),
            ));
        }
        let code_digest = sha256_hex(code);
        let caller_email_norm = canonical_email(&caller.email);
        let display_name = caller_display_name(caller);

        let mut conn = self.lock_conn();
        {
            let rl_tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let allowed = check_and_record_rate_limit(&rl_tx, "accept", &caller.uid, now_ms)?;
            rl_tx.commit()?;
            if !allowed {
                return Err(MembershipError::RateLimited);
            }
        }

        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some(inv) = load_invitation_by_digest(&tx, &code_digest)? else {
            return Err(MembershipError::InvitationNotFound);
        };
        if !ct_eq_str(&caller_email_norm, &inv.recipient_email_norm) {
            return Err(MembershipError::WrongAccount);
        }

        let existing_member = load_active_membership(&tx, &inv.workspace_id, &caller.uid)?;
        if inv.accepted_at_ms.is_some() {
            if inv.accepted_by_uid.as_deref() == Some(caller.uid.as_str()) {
                if let Some(member) = existing_member {
                    let ws = load_workspace_summary(&tx, &inv.workspace_id, &caller.uid)?;
                    let view = member.into_view(&caller.uid);
                    let _ = recheck_session()?;
                    tx.commit()?;
                    return Ok(AcceptInvitationOutcome {
                        already_member: true,
                        workspace: ws,
                        membership: view,
                    });
                }
            }
            return Err(MembershipError::InvitationAlreadyUsed);
        }

        if inv.revoked_at_ms.is_some() {
            return Err(MembershipError::InvitationRevoked);
        }
        if now_ms > inv.expires_at_ms {
            return Err(MembershipError::InvitationExpired);
        }
        if !issuer_retains_authority(&tx, &inv)? {
            tx.execute(
                "UPDATE workspace_invitations
                 SET revoked_at_ms = ?2, revoked_by_uid = ?3, revoked_reason = 'issuer_authority_lost'
                 WHERE invitation_id = ?1 AND revoked_at_ms IS NULL",
                params![inv.invitation_id, now_ms, caller.uid],
            )?;
            record_audit(
                &tx,
                &inv.workspace_id,
                "invitation_revoked",
                &caller.uid,
                None,
                Some(&inv.invited_by_uid),
                Some(&inv.invited_by_membership_id),
                Some(&inv.invitation_id),
                &serde_json::json!({ "offered_role": inv.offered_role.as_str(), "reason": "issuer_authority_lost" }),
                now_ms,
            )?;
            tx.commit()?;
            return Err(MembershipError::InvitationRevoked);
        }

        let (already_member, membership_id, effective_role) = if let Some(member) = existing_member
        {
            (true, member.membership_id, member.role)
        } else {
            let new_membership_id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO workspace_memberships
                    (membership_id, workspace_id, uid, display_name, role, joined_at_ms, updated_at_ms, removed_at_ms, removed_by_uid)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, NULL, NULL)",
                params![
                    new_membership_id,
                    inv.workspace_id,
                    caller.uid,
                    display_name,
                    inv.offered_role.as_str(),
                    now_ms
                ],
            )?;
            (false, new_membership_id, inv.offered_role)
        };

        let updated = tx.execute(
            "UPDATE workspace_invitations
             SET accepted_at_ms = ?2, accepted_by_uid = ?3, accepted_membership_id = ?4
             WHERE invitation_id = ?1 AND accepted_at_ms IS NULL AND revoked_at_ms IS NULL",
            params![inv.invitation_id, now_ms, caller.uid, membership_id],
        )?;
        if updated != 1 {
            return Err(MembershipError::InvitationAlreadyUsed);
        }
        tx.execute(
            "UPDATE workspaces SET updated_at_ms = MAX(updated_at_ms, ?2) WHERE workspace_id = ?1",
            params![inv.workspace_id, now_ms],
        )?;
        record_audit(
            &tx,
            &inv.workspace_id,
            "invitation_accepted",
            &caller.uid,
            Some(&membership_id),
            Some(&caller.uid),
            Some(&membership_id),
            Some(&inv.invitation_id),
            &serde_json::json!({
                "offered_role": inv.offered_role.as_str(),
                "effective_role": effective_role.as_str(),
                "already_member": already_member,
            }),
            now_ms,
        )?;

        let ws = load_workspace_summary(&tx, &inv.workspace_id, &caller.uid)?;
        let member = require_active_membership(&tx, &inv.workspace_id, &caller.uid)?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(AcceptInvitationOutcome {
            already_member,
            workspace: ws,
            membership: member.into_view(&caller.uid),
        })
    }

    fn change_member_role(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        target_membership_id: &str,
        new_role: WorkspaceRole,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError> {
        if new_role == WorkspaceRole::Owner {
            return Err(MembershipError::Forbidden);
        }
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let actor = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let target = load_active_membership_by_id(&tx, workspace_id, target_membership_id)?;
        if target.uid == caller.uid {
            return Err(MembershipError::Forbidden);
        }
        if target.role == WorkspaceRole::Owner {
            return Err(MembershipError::LastOwnerProtected);
        }
        if !actor.role.can_change_member_role(target.role, new_role) {
            return Err(MembershipError::Forbidden);
        }
        if target.role != new_role {
            tx.execute(
                "UPDATE workspace_memberships SET role = ?2, updated_at_ms = ?3
                 WHERE membership_id = ?1 AND removed_at_ms IS NULL",
                params![target_membership_id, new_role.as_str(), now_ms],
            )?;
            revoke_ungrantable_pending_invites(
                &tx,
                workspace_id,
                target_membership_id,
                Some(new_role),
                &caller.uid,
                now_ms,
            )?;
            tx.execute(
                "UPDATE workspaces SET updated_at_ms = MAX(updated_at_ms, ?2) WHERE workspace_id = ?1",
                params![workspace_id, now_ms],
            )?;
            record_audit(
                &tx,
                workspace_id,
                "member_role_changed",
                &caller.uid,
                Some(&actor.membership_id),
                Some(&target.uid),
                Some(&target.membership_id),
                None,
                &serde_json::json!({
                    "from_role": target.role.as_str(),
                    "to_role": new_role.as_str(),
                }),
                now_ms,
            )?;
        }
        let details = build_workspace_details(&tx, caller, workspace_id, now_ms)?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(details)
    }

    fn remove_member(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        target_membership_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let actor = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let target = load_active_membership_by_id(&tx, workspace_id, target_membership_id)?;
        if target.role == WorkspaceRole::Owner {
            return Err(MembershipError::LastOwnerProtected);
        }
        if !actor.role.can_remove_member(target.role) {
            return Err(MembershipError::Forbidden);
        }
        if target.uid == caller.uid {
            return Err(MembershipError::InvalidInput(
                "Use Leave workspace to remove yourself.".into(),
            ));
        }

        let updated = tx.execute(
            "UPDATE workspace_memberships SET removed_at_ms = ?2, removed_by_uid = ?3, updated_at_ms = ?2
             WHERE membership_id = ?1 AND removed_at_ms IS NULL",
            params![target_membership_id, now_ms, caller.uid],
        )?;
        if updated != 1 {
            return Err(MembershipError::MemberNotFound);
        }
        revoke_ungrantable_pending_invites(
            &tx,
            workspace_id,
            target_membership_id,
            None,
            &caller.uid,
            now_ms,
        )?;
        tx.execute(
            "UPDATE workspace_ownership_transfers
             SET status = 'cancelled', decided_at_ms = ?2
             WHERE workspace_id = ?1 AND status = 'pending' AND to_membership_id = ?3",
            params![workspace_id, now_ms, target_membership_id],
        )?;
        tx.execute(
            "UPDATE workspaces SET updated_at_ms = MAX(updated_at_ms, ?2) WHERE workspace_id = ?1",
            params![workspace_id, now_ms],
        )?;
        record_audit(
            &tx,
            workspace_id,
            "member_removed",
            &caller.uid,
            Some(&actor.membership_id),
            Some(&target.uid),
            Some(&target.membership_id),
            None,
            &serde_json::json!({ "removed_role": target.role.as_str() }),
            now_ms,
        )?;
        let details = build_workspace_details(&tx, caller, workspace_id, now_ms)?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(details)
    }

    fn leave_workspace(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        now_ms: i64,
    ) -> Result<(), MembershipError> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let actor = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !actor.role.can_leave() {
            return Err(MembershipError::LastOwnerProtected);
        }
        tx.execute(
            "UPDATE workspace_memberships SET removed_at_ms = ?2, removed_by_uid = ?3, updated_at_ms = ?2
             WHERE membership_id = ?1 AND removed_at_ms IS NULL",
            params![actor.membership_id, now_ms, caller.uid],
        )?;
        revoke_ungrantable_pending_invites(
            &tx,
            workspace_id,
            &actor.membership_id,
            None,
            &caller.uid,
            now_ms,
        )?;
        tx.execute(
            "UPDATE workspace_ownership_transfers
             SET status = 'cancelled', decided_at_ms = ?2
             WHERE workspace_id = ?1 AND status = 'pending' AND to_membership_id = ?3",
            params![workspace_id, now_ms, actor.membership_id],
        )?;
        tx.execute(
            "UPDATE workspaces SET updated_at_ms = MAX(updated_at_ms, ?2) WHERE workspace_id = ?1",
            params![workspace_id, now_ms],
        )?;
        record_audit(
            &tx,
            workspace_id,
            "member_left",
            &caller.uid,
            Some(&actor.membership_id),
            Some(&caller.uid),
            Some(&actor.membership_id),
            None,
            &serde_json::json!({ "role": actor.role.as_str() }),
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(())
    }

    fn initiate_ownership_transfer(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        to_membership_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let actor = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if actor.role != WorkspaceRole::Owner {
            return Err(MembershipError::Forbidden);
        }
        let target = load_active_membership_by_id(&tx, workspace_id, to_membership_id)?;
        if target.uid == caller.uid {
            return Err(MembershipError::InvalidInput(
                "Choose another member to transfer ownership to.".into(),
            ));
        }
        let has_pending: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM workspace_ownership_transfers WHERE workspace_id = ?1 AND status = 'pending')",
                params![workspace_id],
                |r| r.get(0),
            )?;
        if has_pending {
            return Err(MembershipError::TransferAlreadyPending);
        }
        let transfer_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO workspace_ownership_transfers
                (transfer_id, workspace_id, from_membership_id, from_uid, from_display_name, to_membership_id, to_uid, to_display_name, status, created_at_ms, decided_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', ?9, NULL)",
            params![
                transfer_id,
                workspace_id,
                actor.membership_id,
                actor.uid,
                actor.display_name,
                target.membership_id,
                target.uid,
                target.display_name,
                now_ms,
            ],
        )?;
        record_audit(
            &tx,
            workspace_id,
            "ownership_transfer_initiated",
            &caller.uid,
            Some(&actor.membership_id),
            Some(&target.uid),
            Some(&target.membership_id),
            None,
            &serde_json::json!({ "transfer_id": transfer_id }),
            now_ms,
        )?;
        let details = build_workspace_details(&tx, caller, workspace_id, now_ms)?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(details)
    }

    fn cancel_ownership_transfer(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        transfer_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let actor = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if actor.role != WorkspaceRole::Owner {
            return Err(MembershipError::Forbidden);
        }
        let updated = tx.execute(
            "UPDATE workspace_ownership_transfers
             SET status = 'cancelled', decided_at_ms = ?3
             WHERE transfer_id = ?1 AND workspace_id = ?2 AND status = 'pending'",
            params![transfer_id, workspace_id, now_ms],
        )?;
        if updated != 1 {
            return Err(MembershipError::TransferNotPending);
        }
        record_audit(
            &tx,
            workspace_id,
            "ownership_transfer_cancelled",
            &caller.uid,
            Some(&actor.membership_id),
            None,
            None,
            None,
            &serde_json::json!({ "transfer_id": transfer_id }),
            now_ms,
        )?;
        let details = build_workspace_details(&tx, caller, workspace_id, now_ms)?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(details)
    }

    fn accept_ownership_transfer(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        transfer_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row: Option<(String, String, String, String, String)> = tx
            .query_row(
                "SELECT from_membership_id, from_uid, to_membership_id, to_uid, status
                 FROM workspace_ownership_transfers
                 WHERE transfer_id = ?1 AND workspace_id = ?2",
                params![transfer_id, workspace_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?;
        let Some((from_membership_id, from_uid, to_membership_id, to_uid, status)) = row else {
            return Err(MembershipError::TransferNotPending);
        };
        if status != "pending" {
            return Err(MembershipError::TransferNotPending);
        }
        if to_uid != caller.uid {
            return Err(MembershipError::Forbidden);
        }

        let current_recipient = load_active_membership_by_id(&tx, workspace_id, &to_membership_id)?;
        if current_recipient.uid != caller.uid {
            return Err(MembershipError::Forbidden);
        }
        let current_owner = load_active_membership_by_id(&tx, workspace_id, &from_membership_id)?;
        if current_owner.role != WorkspaceRole::Owner || current_owner.uid != from_uid {
            return Err(MembershipError::TransferNotPending);
        }

        // Demote outgoing Owner -> Admin first inside the atomic tx so unique active owner index stays valid.
        let demoted = tx.execute(
            "UPDATE workspace_memberships SET role = 'admin', updated_at_ms = ?2
             WHERE membership_id = ?1 AND role = 'owner' AND removed_at_ms IS NULL",
            params![from_membership_id, now_ms],
        )?;
        let promoted = tx.execute(
            "UPDATE workspace_memberships SET role = 'owner', updated_at_ms = ?2
             WHERE membership_id = ?1 AND removed_at_ms IS NULL",
            params![to_membership_id, now_ms],
        )?;
        let marked = tx.execute(
            "UPDATE workspace_ownership_transfers SET status = 'accepted', decided_at_ms = ?2
             WHERE transfer_id = ?1 AND status = 'pending'",
            params![transfer_id, now_ms],
        )?;
        if demoted != 1 || promoted != 1 || marked != 1 {
            return Err(MembershipError::TransferNotPending);
        }

        // Outgoing Owner becomes Admin, so any pending Admin invites they issued must be invalidated.
        revoke_ungrantable_pending_invites(
            &tx,
            workspace_id,
            &from_membership_id,
            Some(WorkspaceRole::Admin),
            &caller.uid,
            now_ms,
        )?;
        tx.execute(
            "UPDATE workspaces SET updated_at_ms = MAX(updated_at_ms, ?2) WHERE workspace_id = ?1",
            params![workspace_id, now_ms],
        )?;
        record_audit(
            &tx,
            workspace_id,
            "ownership_transferred",
            &caller.uid,
            Some(&to_membership_id),
            Some(&from_uid),
            Some(&from_membership_id),
            None,
            &serde_json::json!({
                "transfer_id": transfer_id,
                "outgoing_role": "admin",
                "incoming_role": "owner",
            }),
            now_ms,
        )?;
        let details = build_workspace_details(&tx, caller, workspace_id, now_ms)?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(details)
    }

    fn list_audit_records(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
    ) -> Result<Vec<WorkspaceAuditRecord>, MembershipError> {
        let conn = self.lock_conn();
        let member = load_active_membership_conn(&conn, workspace_id, &caller.uid)?
            .ok_or(MembershipError::Forbidden)?;
        if !matches!(member.role, WorkspaceRole::Owner | WorkspaceRole::Admin) {
            return Err(MembershipError::Forbidden);
        }
        let mut stmt = conn.prepare(
            "SELECT log_id, workspace_id, action, actor_uid, actor_membership_id,
                    target_uid, target_membership_id, invitation_id, detail_json, created_at_ms
             FROM workspace_audit_logs
             WHERE workspace_id = ?1
             ORDER BY created_at_ms ASC, rowid ASC",
        )?;
        let rows = stmt
            .query_map(params![workspace_id], |r| {
                Ok(WorkspaceAuditRecord {
                    log_id: r.get(0)?,
                    workspace_id: r.get(1)?,
                    action: r.get(2)?,
                    actor_uid: r.get(3)?,
                    actor_membership_id: r.get(4)?,
                    target_uid: r.get(5)?,
                    target_membership_id: r.get(6)?,
                    invitation_id: r.get(7)?,
                    detail_json: r.get(8)?,
                    created_at_ms: r.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[derive(Clone)]
pub(crate) struct ActiveMemberRow {
    pub(crate) membership_id: String,
    pub(crate) uid: String,
    pub(crate) display_name: String,
    pub(crate) role: WorkspaceRole,
    pub(crate) joined_at_ms: i64,
}

impl ActiveMemberRow {
    fn into_view(self, caller_uid: &str) -> WorkspaceMemberView {
        WorkspaceMemberView {
            is_current_user: self.uid == caller_uid,
            membership_id: self.membership_id,
            uid: self.uid,
            display_name: self.display_name,
            role: self.role,
            joined_at_ms: self.joined_at_ms,
        }
    }
}

struct InvitationRow {
    invitation_id: String,
    workspace_id: String,
    recipient_email: String,
    recipient_email_norm: String,
    offered_role: WorkspaceRole,
    invited_by_membership_id: String,
    invited_by_uid: String,
    invited_by_display_name: String,
    created_at_ms: i64,
    expires_at_ms: i64,
    accepted_at_ms: Option<i64>,
    accepted_by_uid: Option<String>,
    revoked_at_ms: Option<i64>,
}

impl InvitationRow {
    fn status_at(&self, now_ms: i64) -> InvitationStatus {
        if self.revoked_at_ms.is_some() {
            InvitationStatus::Revoked
        } else if self.accepted_at_ms.is_some() {
            InvitationStatus::Accepted
        } else if now_ms > self.expires_at_ms {
            InvitationStatus::Expired
        } else {
            InvitationStatus::Pending
        }
    }

    fn into_view(self, now_ms: i64) -> WorkspaceInvitationView {
        let status = self.status_at(now_ms);
        WorkspaceInvitationView {
            invitation_id: self.invitation_id,
            workspace_id: self.workspace_id,
            recipient_email: self.recipient_email,
            offered_role: self.offered_role,
            status,
            invited_by_membership_id: self.invited_by_membership_id,
            invited_by_display_name: self.invited_by_display_name,
            created_at_ms: self.created_at_ms,
            expires_at_ms: self.expires_at_ms,
            accepted_at_ms: self.accepted_at_ms,
            revoked_at_ms: self.revoked_at_ms,
        }
    }
}

pub(crate) fn caller_display_name(caller: &VerifiedSession) -> String {
    resolve_display_name(Some(&caller.display_name), None, &caller.email)
}

fn canonical_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn sanitize_workspace_name(raw: &str) -> Result<String, MembershipError> {
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        return Err(MembershipError::InvalidInput(
            "Workspace name is required.".into(),
        ));
    }
    if cleaned.chars().count() > MAX_WORKSPACE_NAME_LEN {
        return Err(MembershipError::InvalidInput(format!(
            "Workspace name must be at most {MAX_WORKSPACE_NAME_LEN} characters."
        )));
    }
    Ok(cleaned)
}

pub(crate) fn sanitize_idempotency_key(raw: &str) -> Result<String, MembershipError> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.len() > MAX_IDEMPOTENCY_KEY_LEN
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(MembershipError::InvalidInput(
            "Invalid create request identifier.".into(),
        ));
    }
    Ok(trimmed.to_owned())
}

fn sanitize_email(raw: &str) -> Result<(String, String), MembershipError> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.len() > MAX_EMAIL_LEN
        || trimmed.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(MembershipError::InvalidInput(
            "Enter a valid email address.".into(),
        ));
    }
    let Some((local, domain)) = trimmed.split_once('@') else {
        return Err(MembershipError::InvalidInput(
            "Enter a valid email address.".into(),
        ));
    };
    if local.is_empty()
        || domain.is_empty()
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return Err(MembershipError::InvalidInput(
            "Enter a valid email address.".into(),
        ));
    }
    let normalized = canonical_email(trimmed);
    Ok((trimmed.to_owned(), normalized))
}

fn random_invite_code() -> Result<String, MembershipError> {
    let mut bytes = [0u8; 24];
    getrandom::getrandom(&mut bytes).map_err(|_| MembershipError::Random)?;
    Ok(format!("sthi_{}", URL_SAFE_NO_PAD.encode(bytes)))
}

fn check_and_record_rate_limit(
    tx: &Transaction<'_>,
    scope: &str,
    actor_uid: &str,
    now_ms: i64,
) -> Result<bool, MembershipError> {
    let cutoff = now_ms.saturating_sub(INVITE_RATE_LIMIT_WINDOW_MS);
    tx.execute(
        "DELETE FROM workspace_rate_limits WHERE scope = ?1 AND actor_uid = ?2 AND attempted_at_ms < ?3",
        params![scope, actor_uid, cutoff],
    )?;
    let recent_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM workspace_rate_limits WHERE scope = ?1 AND actor_uid = ?2 AND attempted_at_ms >= ?3",
        params![scope, actor_uid, cutoff],
        |r| r.get(0),
    )?;
    if recent_count as usize >= INVITE_RATE_LIMIT_MAX_ATTEMPTS {
        return Ok(false);
    }
    tx.execute(
        "INSERT INTO workspace_rate_limits (scope, actor_uid, attempted_at_ms) VALUES (?1, ?2, ?3)",
        params![scope, actor_uid, now_ms],
    )?;
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn record_audit(
    tx: &Transaction<'_>,
    workspace_id: &str,
    action: &str,
    actor_uid: &str,
    actor_membership_id: Option<&str>,
    target_uid: Option<&str>,
    target_membership_id: Option<&str>,
    invitation_id: Option<&str>,
    detail: &serde_json::Value,
    now_ms: i64,
) -> Result<(), MembershipError> {
    let log_id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO workspace_audit_logs
            (log_id, workspace_id, action, actor_uid, actor_membership_id, target_uid, target_membership_id, invitation_id, detail_json, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            log_id,
            workspace_id,
            action,
            actor_uid,
            actor_membership_id,
            target_uid,
            target_membership_id,
            invitation_id,
            detail.to_string(),
            now_ms,
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_invitation_in_tx(
    tx: &Transaction<'_>,
    workspace_id: &str,
    actor: &ActiveMemberRow,
    recipient_email: &str,
    recipient_email_norm: &str,
    offered_role: WorkspaceRole,
    now_ms: i64,
    ttl_ms: i64,
) -> Result<CreatedInvitation, MembershipError> {
    let invitation_id = Uuid::new_v4().to_string();
    let invite_code = random_invite_code()?;
    let code_digest = sha256_hex(&invite_code);
    let expires_at_ms = now_ms.saturating_add(ttl_ms);
    tx.execute(
        "INSERT INTO workspace_invitations
            (invitation_id, workspace_id, code_digest, recipient_email, recipient_email_norm, offered_role,
             invited_by_membership_id, invited_by_uid, invited_by_display_name, created_at_ms, expires_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            invitation_id,
            workspace_id,
            code_digest,
            recipient_email,
            recipient_email_norm,
            offered_role.as_str(),
            actor.membership_id,
            actor.uid,
            actor.display_name,
            now_ms,
            expires_at_ms,
        ],
    )?;
    record_audit(
        tx,
        workspace_id,
        "invitation_created",
        &actor.uid,
        Some(&actor.membership_id),
        None,
        None,
        Some(&invitation_id),
        &serde_json::json!({ "offered_role": offered_role.as_str() }),
        now_ms,
    )?;
    Ok(CreatedInvitation {
        invitation: WorkspaceInvitationView {
            invitation_id,
            workspace_id: workspace_id.to_owned(),
            recipient_email: recipient_email.to_owned(),
            offered_role,
            status: InvitationStatus::Pending,
            invited_by_membership_id: actor.membership_id.clone(),
            invited_by_display_name: actor.display_name.clone(),
            created_at_ms: now_ms,
            expires_at_ms,
            accepted_at_ms: None,
            revoked_at_ms: None,
        },
        invite_code,
    })
}

fn revoke_ungrantable_pending_invites(
    tx: &Transaction<'_>,
    workspace_id: &str,
    issuer_membership_id: &str,
    issuer_new_role: Option<WorkspaceRole>,
    actor_uid: &str,
    now_ms: i64,
) -> Result<(), MembershipError> {
    let mut stmt = tx.prepare(
        "SELECT invitation_id, offered_role
         FROM workspace_invitations
         WHERE workspace_id = ?1 AND invited_by_membership_id = ?2
           AND accepted_at_ms IS NULL AND revoked_at_ms IS NULL",
    )?;
    let pending = stmt
        .query_map(params![workspace_id, issuer_membership_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    for (invitation_id, offered_role_str) in pending {
        let offered = WorkspaceRole::from_db(&offered_role_str)?;
        let keeps_authority = issuer_new_role.is_some_and(|r| r.can_grant_invite_role(offered));
        if !keeps_authority {
            tx.execute(
                "UPDATE workspace_invitations
                 SET revoked_at_ms = ?2, revoked_by_uid = ?3, revoked_reason = 'issuer_authority_lost'
                 WHERE invitation_id = ?1 AND revoked_at_ms IS NULL",
                params![invitation_id, now_ms, actor_uid],
            )?;
            record_audit(
                tx,
                workspace_id,
                "invitation_revoked",
                actor_uid,
                None,
                None,
                Some(issuer_membership_id),
                Some(&invitation_id),
                &serde_json::json!({ "offered_role": offered.as_str(), "reason": "issuer_authority_lost" }),
                now_ms,
            )?;
        }
    }
    Ok(())
}

fn issuer_retains_authority(
    tx: &Transaction<'_>,
    inv: &InvitationRow,
) -> Result<bool, MembershipError> {
    let issuer_role: Option<String> = tx
        .query_row(
            "SELECT role FROM workspace_memberships
             WHERE membership_id = ?1 AND workspace_id = ?2 AND removed_at_ms IS NULL",
            params![inv.invited_by_membership_id, inv.workspace_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(role_str) = issuer_role else {
        return Ok(false);
    };
    Ok(WorkspaceRole::from_db(&role_str)?.can_grant_invite_role(inv.offered_role))
}

fn load_active_membership(
    tx: &Transaction<'_>,
    workspace_id: &str,
    uid: &str,
) -> Result<Option<ActiveMemberRow>, MembershipError> {
    load_active_membership_conn(tx, workspace_id, uid)
}

fn load_active_membership_conn(
    conn: &Connection,
    workspace_id: &str,
    uid: &str,
) -> Result<Option<ActiveMemberRow>, MembershipError> {
    let row = conn
        .query_row(
            "SELECT membership_id, uid, display_name, role, joined_at_ms
             FROM workspace_memberships
             WHERE workspace_id = ?1 AND uid = ?2 AND removed_at_ms IS NULL",
            params![workspace_id, uid],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?;
    row.map(
        |(membership_id, uid, display_name, role_str, joined_at_ms)| {
            Ok(ActiveMemberRow {
                membership_id,
                uid,
                display_name,
                role: WorkspaceRole::from_db(&role_str)?,
                joined_at_ms,
            })
        },
    )
    .transpose()
}

pub(crate) fn require_active_membership(
    tx: &Transaction<'_>,
    workspace_id: &str,
    uid: &str,
) -> Result<ActiveMemberRow, MembershipError> {
    load_active_membership(tx, workspace_id, uid)?.ok_or(MembershipError::Forbidden)
}

fn load_active_membership_by_id(
    tx: &Transaction<'_>,
    workspace_id: &str,
    membership_id: &str,
) -> Result<ActiveMemberRow, MembershipError> {
    let row = tx
        .query_row(
            "SELECT membership_id, uid, display_name, role, joined_at_ms
             FROM workspace_memberships
             WHERE workspace_id = ?1 AND membership_id = ?2 AND removed_at_ms IS NULL",
            params![workspace_id, membership_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((membership_id, uid, display_name, role_str, joined_at_ms)) = row else {
        return Err(MembershipError::MemberNotFound);
    };
    Ok(ActiveMemberRow {
        membership_id,
        uid,
        display_name,
        role: WorkspaceRole::from_db(&role_str)?,
        joined_at_ms,
    })
}

fn load_workspace_name(
    tx: &Transaction<'_>,
    workspace_id: &str,
) -> Result<String, MembershipError> {
    tx.query_row(
        "SELECT name FROM workspaces WHERE workspace_id = ?1",
        params![workspace_id],
        |r| r.get(0),
    )
    .optional()?
    .ok_or(MembershipError::WorkspaceNotFound)
}

fn load_workspace_summary(
    tx: &Transaction<'_>,
    workspace_id: &str,
    caller_uid: &str,
) -> Result<WorkspaceSummary, MembershipError> {
    let row = tx
        .query_row(
            "SELECT w.workspace_id, w.name, m.membership_id, m.role, w.created_at_ms, w.updated_at_ms
             FROM workspaces w
             JOIN workspace_memberships m ON m.workspace_id = w.workspace_id
             WHERE w.workspace_id = ?1 AND m.uid = ?2 AND m.removed_at_ms IS NULL",
            params![workspace_id, caller_uid],
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
    let Some((workspace_id, name, my_membership_id, role_str, created_at_ms, updated_at_ms)) = row
    else {
        return Err(MembershipError::Forbidden);
    };
    Ok(WorkspaceSummary {
        workspace_id,
        name,
        my_membership_id,
        my_role: WorkspaceRole::from_db(&role_str)?,
        created_at_ms,
        updated_at_ms,
    })
}

fn list_caller_workspaces(
    tx: &Transaction<'_>,
    caller_uid: &str,
) -> Result<Vec<WorkspaceSummary>, MembershipError> {
    let mut stmt = tx.prepare(
        "SELECT w.workspace_id, w.name, m.membership_id, m.role, w.created_at_ms, w.updated_at_ms
         FROM workspaces w
         JOIN workspace_memberships m ON m.workspace_id = w.workspace_id
         WHERE m.uid = ?1 AND m.removed_at_ms IS NULL
         ORDER BY w.updated_at_ms DESC, LOWER(w.name) ASC",
    )?;
    let rows = stmt
        .query_map(params![caller_uid], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(
            |(workspace_id, name, my_membership_id, role_str, created_at_ms, updated_at_ms)| {
                Ok(WorkspaceSummary {
                    workspace_id,
                    name,
                    my_membership_id,
                    my_role: WorkspaceRole::from_db(&role_str)?,
                    created_at_ms,
                    updated_at_ms,
                })
            },
        )
        .collect()
}

fn list_incoming_transfers(
    tx: &Transaction<'_>,
    caller_uid: &str,
) -> Result<Vec<OwnershipTransferView>, MembershipError> {
    let mut stmt = tx.prepare(
        "SELECT t.transfer_id, t.workspace_id, w.name, t.from_membership_id, t.from_uid, t.from_display_name,
                t.to_membership_id, t.to_uid, t.to_display_name, t.created_at_ms
         FROM workspace_ownership_transfers t
         JOIN workspaces w ON w.workspace_id = t.workspace_id
         JOIN workspace_memberships m ON m.membership_id = t.to_membership_id
         WHERE t.to_uid = ?1 AND t.status = 'pending' AND m.removed_at_ms IS NULL
         ORDER BY t.created_at_ms DESC",
    )?;
    let rows = stmt
        .query_map(params![caller_uid], |r| {
            Ok(OwnershipTransferView {
                transfer_id: r.get(0)?,
                workspace_id: r.get(1)?,
                workspace_name: r.get(2)?,
                from_membership_id: r.get(3)?,
                from_uid: r.get(4)?,
                from_display_name: r.get(5)?,
                to_membership_id: r.get(6)?,
                to_uid: r.get(7)?,
                to_display_name: r.get(8)?,
                status: TransferStatus::Pending,
                created_at_ms: r.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_invitation_by_digest(
    tx: &Transaction<'_>,
    code_digest: &str,
) -> Result<Option<InvitationRow>, MembershipError> {
    let row = tx
        .query_row(
            "SELECT invitation_id, workspace_id, recipient_email, recipient_email_norm, offered_role,
                    invited_by_membership_id, invited_by_uid, invited_by_display_name,
                    created_at_ms, expires_at_ms, accepted_at_ms, accepted_by_uid, revoked_at_ms
             FROM workspace_invitations WHERE code_digest = ?1",
            params![code_digest],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, i64>(9)?,
                    r.get::<_, Option<i64>>(10)?,
                    r.get::<_, Option<String>>(11)?,
                    r.get::<_, Option<i64>>(12)?,
                ))
            },
        )
        .optional()?;
    row.map(|r| {
        Ok(InvitationRow {
            invitation_id: r.0,
            workspace_id: r.1,
            recipient_email: r.2,
            recipient_email_norm: r.3,
            offered_role: WorkspaceRole::from_db(&r.4)?,
            invited_by_membership_id: r.5,
            invited_by_uid: r.6,
            invited_by_display_name: r.7,
            created_at_ms: r.8,
            expires_at_ms: r.9,
            accepted_at_ms: r.10,
            accepted_by_uid: r.11,
            revoked_at_ms: r.12,
        })
    })
    .transpose()
}

fn load_invitation_by_id(
    tx: &Transaction<'_>,
    workspace_id: &str,
    invitation_id: &str,
) -> Result<InvitationRow, MembershipError> {
    let row = tx
        .query_row(
            "SELECT invitation_id, workspace_id, recipient_email, recipient_email_norm, offered_role,
                    invited_by_membership_id, invited_by_uid, invited_by_display_name,
                    created_at_ms, expires_at_ms, accepted_at_ms, accepted_by_uid, revoked_at_ms
             FROM workspace_invitations
             WHERE invitation_id = ?1 AND workspace_id = ?2",
            params![invitation_id, workspace_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, i64>(9)?,
                    r.get::<_, Option<i64>>(10)?,
                    r.get::<_, Option<String>>(11)?,
                    r.get::<_, Option<i64>>(12)?,
                ))
            },
        )
        .optional()?;
    let Some(r) = row else {
        return Err(MembershipError::InvitationNotFound);
    };
    Ok(InvitationRow {
        invitation_id: r.0,
        workspace_id: r.1,
        recipient_email: r.2,
        recipient_email_norm: r.3,
        offered_role: WorkspaceRole::from_db(&r.4)?,
        invited_by_membership_id: r.5,
        invited_by_uid: r.6,
        invited_by_display_name: r.7,
        created_at_ms: r.8,
        expires_at_ms: r.9,
        accepted_at_ms: r.10,
        accepted_by_uid: r.11,
        revoked_at_ms: r.12,
    })
}

fn build_workspace_details(
    tx: &Transaction<'_>,
    caller: &VerifiedSession,
    workspace_id: &str,
    now_ms: i64,
) -> Result<WorkspaceDetails, MembershipError> {
    let workspace = load_workspace_summary(tx, workspace_id, &caller.uid)?;
    let mut mem_stmt = tx.prepare(
        "SELECT membership_id, uid, display_name, role, joined_at_ms
         FROM workspace_memberships
         WHERE workspace_id = ?1 AND removed_at_ms IS NULL
         ORDER BY CASE role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 WHEN 'editor' THEN 2 ELSE 3 END ASC,
                  joined_at_ms ASC, rowid ASC",
    )?;
    let raw_members = mem_stmt
        .query_map(params![workspace_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(mem_stmt);

    let members = raw_members
        .into_iter()
        .map(
            |(membership_id, uid, display_name, role_str, joined_at_ms)| {
                Ok(ActiveMemberRow {
                    membership_id,
                    uid,
                    display_name,
                    role: WorkspaceRole::from_db(&role_str)?,
                    joined_at_ms,
                }
                .into_view(&caller.uid))
            },
        )
        .collect::<Result<Vec<_>, MembershipError>>()?;

    let invitations = if matches!(
        workspace.my_role,
        WorkspaceRole::Owner | WorkspaceRole::Admin
    ) {
        let mut inv_stmt = tx.prepare(
            "SELECT invitation_id, workspace_id, recipient_email, recipient_email_norm, offered_role,
                    invited_by_membership_id, invited_by_uid, invited_by_display_name,
                    created_at_ms, expires_at_ms, accepted_at_ms, accepted_by_uid, revoked_at_ms
             FROM workspace_invitations
             WHERE workspace_id = ?1
             ORDER BY created_at_ms DESC, rowid DESC",
        )?;
        let raw_invs = inv_stmt
            .query_map(params![workspace_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, i64>(8)?,
                    r.get::<_, i64>(9)?,
                    r.get::<_, Option<i64>>(10)?,
                    r.get::<_, Option<String>>(11)?,
                    r.get::<_, Option<i64>>(12)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(inv_stmt);
        raw_invs
            .into_iter()
            .filter_map(|r| {
                let Ok(offered_role) = WorkspaceRole::from_db(&r.4) else {
                    return None;
                };
                if !workspace.my_role.can_manage_invitation(offered_role) {
                    return None;
                }
                Some(
                    InvitationRow {
                        invitation_id: r.0,
                        workspace_id: r.1,
                        recipient_email: r.2,
                        recipient_email_norm: r.3,
                        offered_role,
                        invited_by_membership_id: r.5,
                        invited_by_uid: r.6,
                        invited_by_display_name: r.7,
                        created_at_ms: r.8,
                        expires_at_ms: r.9,
                        accepted_at_ms: r.10,
                        accepted_by_uid: r.11,
                        revoked_at_ms: r.12,
                    }
                    .into_view(now_ms),
                )
            })
            .collect()
    } else {
        Vec::new()
    };

    let pending_transfer = tx
        .query_row(
            "SELECT t.transfer_id, t.workspace_id, w.name, t.from_membership_id, t.from_uid, t.from_display_name,
                    t.to_membership_id, t.to_uid, t.to_display_name, t.created_at_ms
             FROM workspace_ownership_transfers t
             JOIN workspaces w ON w.workspace_id = t.workspace_id
             WHERE t.workspace_id = ?1 AND t.status = 'pending'",
            params![workspace_id],
            |r| {
                Ok(OwnershipTransferView {
                    transfer_id: r.get(0)?,
                    workspace_id: r.get(1)?,
                    workspace_name: r.get(2)?,
                    from_membership_id: r.get(3)?,
                    from_uid: r.get(4)?,
                    from_display_name: r.get(5)?,
                    to_membership_id: r.get(6)?,
                    to_uid: r.get(7)?,
                    to_display_name: r.get(8)?,
                    status: TransferStatus::Pending,
                    created_at_ms: r.get(9)?,
                })
            },
        )
        .optional()?
        .filter(|t| workspace.my_role == WorkspaceRole::Owner || t.to_uid == caller.uid);

    Ok(WorkspaceDetails {
        workspace,
        members,
        invitations,
        pending_transfer,
    })
}

/// Pure in-memory adapter backing tests that verify storage-provider independence of `WorkspaceMembershipStore`.
#[derive(Default, Clone)]
pub struct InMemoryWorkspaceMembershipStore {
    inner: Arc<Mutex<InMemoryState>>,
}

#[derive(Default)]
struct InMemoryState {
    workspaces: HashMap<String, MemWorkspace>,
    members: HashMap<String, MemMember>,
    invitations: HashMap<String, MemInvite>,
    transfers: HashMap<String, MemTransfer>,
    rate_limits: HashMap<(String, String), Vec<i64>>,
    audit_logs: Vec<WorkspaceAuditRecord>,
}

#[derive(Clone)]
struct MemWorkspace {
    workspace_id: String,
    name: String,
    created_by_uid: String,
    create_idempotency_key: String,
    created_at_ms: i64,
    updated_at_ms: i64,
}

#[derive(Clone)]
struct MemMember {
    membership_id: String,
    workspace_id: String,
    uid: String,
    display_name: String,
    role: WorkspaceRole,
    joined_at_ms: i64,
    removed_at_ms: Option<i64>,
}

#[derive(Clone)]
struct MemInvite {
    invitation_id: String,
    workspace_id: String,
    code_digest: String,
    recipient_email: String,
    recipient_email_norm: String,
    offered_role: WorkspaceRole,
    invited_by_membership_id: String,
    invited_by_uid: String,
    invited_by_display_name: String,
    created_at_ms: i64,
    expires_at_ms: i64,
    accepted_at_ms: Option<i64>,
    accepted_by_uid: Option<String>,
    revoked_at_ms: Option<i64>,
}

#[derive(Clone)]
struct MemTransfer {
    transfer_id: String,
    workspace_id: String,
    from_membership_id: String,
    from_uid: String,
    from_display_name: String,
    to_membership_id: String,
    to_uid: String,
    to_display_name: String,
    status: TransferStatus,
    created_at_ms: i64,
}

impl InMemoryWorkspaceMembershipStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn active_member<'a>(
        state: &'a InMemoryState,
        workspace_id: &str,
        uid: &str,
    ) -> Option<&'a MemMember> {
        state
            .members
            .values()
            .find(|m| m.workspace_id == workspace_id && m.uid == uid && m.removed_at_ms.is_none())
    }

    fn summary(
        state: &InMemoryState,
        workspace_id: &str,
        uid: &str,
    ) -> Result<WorkspaceSummary, MembershipError> {
        let ws = state
            .workspaces
            .get(workspace_id)
            .ok_or(MembershipError::Forbidden)?;
        let mem =
            Self::active_member(state, workspace_id, uid).ok_or(MembershipError::Forbidden)?;
        Ok(WorkspaceSummary {
            workspace_id: ws.workspace_id.clone(),
            name: ws.name.clone(),
            my_membership_id: mem.membership_id.clone(),
            my_role: mem.role,
            created_at_ms: ws.created_at_ms,
            updated_at_ms: ws.updated_at_ms,
        })
    }

    fn details(
        state: &InMemoryState,
        caller_uid: &str,
        workspace_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError> {
        let workspace = Self::summary(state, workspace_id, caller_uid)?;
        let mut members: Vec<WorkspaceMemberView> = state
            .members
            .values()
            .filter(|m| m.workspace_id == workspace_id && m.removed_at_ms.is_none())
            .map(|m| WorkspaceMemberView {
                membership_id: m.membership_id.clone(),
                uid: m.uid.clone(),
                display_name: m.display_name.clone(),
                role: m.role,
                joined_at_ms: m.joined_at_ms,
                is_current_user: m.uid == caller_uid,
            })
            .collect();
        members.sort_by(|a, b| {
            a.role
                .cmp(&b.role)
                .then(a.joined_at_ms.cmp(&b.joined_at_ms))
        });

        let mut invitations: Vec<WorkspaceInvitationView> = state
            .invitations
            .values()
            .filter(|inv| {
                inv.workspace_id == workspace_id
                    && workspace.my_role.can_manage_invitation(inv.offered_role)
            })
            .map(|inv| WorkspaceInvitationView {
                invitation_id: inv.invitation_id.clone(),
                workspace_id: inv.workspace_id.clone(),
                recipient_email: inv.recipient_email.clone(),
                offered_role: inv.offered_role,
                status: if inv.revoked_at_ms.is_some() {
                    InvitationStatus::Revoked
                } else if inv.accepted_at_ms.is_some() {
                    InvitationStatus::Accepted
                } else if now_ms > inv.expires_at_ms {
                    InvitationStatus::Expired
                } else {
                    InvitationStatus::Pending
                },
                invited_by_membership_id: inv.invited_by_membership_id.clone(),
                invited_by_display_name: inv.invited_by_display_name.clone(),
                created_at_ms: inv.created_at_ms,
                expires_at_ms: inv.expires_at_ms,
                accepted_at_ms: inv.accepted_at_ms,
                revoked_at_ms: inv.revoked_at_ms,
            })
            .collect();
        invitations.sort_by_key(|a| std::cmp::Reverse(a.created_at_ms));

        let pending_transfer = state
            .transfers
            .values()
            .find(|t| t.workspace_id == workspace_id && t.status == TransferStatus::Pending)
            .filter(|t| workspace.my_role == WorkspaceRole::Owner || t.to_uid == caller_uid)
            .and_then(|t| {
                let ws_name = state.workspaces.get(&t.workspace_id)?.name.clone();
                Some(OwnershipTransferView {
                    transfer_id: t.transfer_id.clone(),
                    workspace_id: t.workspace_id.clone(),
                    workspace_name: ws_name,
                    from_membership_id: t.from_membership_id.clone(),
                    from_uid: t.from_uid.clone(),
                    from_display_name: t.from_display_name.clone(),
                    to_membership_id: t.to_membership_id.clone(),
                    to_uid: t.to_uid.clone(),
                    to_display_name: t.to_display_name.clone(),
                    status: t.status,
                    created_at_ms: t.created_at_ms,
                })
            });

        Ok(WorkspaceDetails {
            workspace,
            members,
            invitations,
            pending_transfer,
        })
    }

    fn revoke_ungrantable(
        state: &mut InMemoryState,
        workspace_id: &str,
        issuer_membership_id: &str,
        new_role: Option<WorkspaceRole>,
        actor_uid: &str,
        now_ms: i64,
    ) {
        let ids: Vec<String> = state
            .invitations
            .values()
            .filter(|inv| {
                inv.workspace_id == workspace_id
                    && inv.invited_by_membership_id == issuer_membership_id
                    && inv.accepted_at_ms.is_none()
                    && inv.revoked_at_ms.is_none()
                    && !new_role.is_some_and(|r| r.can_grant_invite_role(inv.offered_role))
            })
            .map(|inv| inv.invitation_id.clone())
            .collect();
        for id in ids {
            if let Some(inv) = state.invitations.get_mut(&id) {
                inv.revoked_at_ms = Some(now_ms);
                state.audit_logs.push(WorkspaceAuditRecord {
                    log_id: Uuid::new_v4().to_string(),
                    workspace_id: workspace_id.to_owned(),
                    action: "invitation_revoked".into(),
                    actor_uid: actor_uid.to_owned(),
                    actor_membership_id: None,
                    target_uid: Some(inv.invited_by_uid.clone()),
                    target_membership_id: Some(issuer_membership_id.to_owned()),
                    invitation_id: Some(id),
                    detail_json: r#"{"reason":"issuer_authority_lost"}"#.into(),
                    created_at_ms: now_ms,
                });
            }
        }
    }
}

impl WorkspaceMembershipStore for InMemoryWorkspaceMembershipStore {
    fn list_workspaces(
        &self,
        caller: &VerifiedSession,
        _now_ms: i64,
    ) -> Result<WorkspaceListResponse, MembershipError> {
        let state = self.inner.lock().unwrap();
        let mut workspaces: Vec<WorkspaceSummary> = state
            .members
            .values()
            .filter(|m| m.uid == caller.uid && m.removed_at_ms.is_none())
            .filter_map(|m| Self::summary(&state, &m.workspace_id, &caller.uid).ok())
            .collect();
        workspaces.sort_by(|a, b| {
            b.updated_at_ms
                .cmp(&a.updated_at_ms)
                .then(a.name.cmp(&b.name))
        });
        let incoming_transfers = state
            .transfers
            .values()
            .filter(|t| {
                t.to_uid == caller.uid
                    && t.status == TransferStatus::Pending
                    && Self::active_member(&state, &t.workspace_id, &caller.uid).is_some()
            })
            .filter_map(|t| {
                let ws_name = state.workspaces.get(&t.workspace_id)?.name.clone();
                Some(OwnershipTransferView {
                    transfer_id: t.transfer_id.clone(),
                    workspace_id: t.workspace_id.clone(),
                    workspace_name: ws_name,
                    from_membership_id: t.from_membership_id.clone(),
                    from_uid: t.from_uid.clone(),
                    from_display_name: t.from_display_name.clone(),
                    to_membership_id: t.to_membership_id.clone(),
                    to_uid: t.to_uid.clone(),
                    to_display_name: t.to_display_name.clone(),
                    status: t.status,
                    created_at_ms: t.created_at_ms,
                })
            })
            .collect();
        Ok(WorkspaceListResponse {
            workspaces,
            incoming_transfers,
        })
    }

    fn create_workspace(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        name: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<CreateWorkspaceOutcome, MembershipError> {
        let name = sanitize_workspace_name(name)?;
        let idempotency_key = sanitize_idempotency_key(idempotency_key)?;
        let mut state = self.inner.lock().unwrap();
        if let Some(existing) = state
            .workspaces
            .values()
            .find(|w| w.created_by_uid == caller.uid && w.create_idempotency_key == idempotency_key)
            .cloned()
        {
            let ws = Self::summary(&state, &existing.workspace_id, &caller.uid)?;
            let _ = recheck_session()?;
            return Ok(CreateWorkspaceOutcome {
                created: false,
                workspace: ws,
            });
        }
        let _ = recheck_session()?;
        let workspace_id = Uuid::new_v4().to_string();
        let membership_id = Uuid::new_v4().to_string();
        state.workspaces.insert(
            workspace_id.clone(),
            MemWorkspace {
                workspace_id: workspace_id.clone(),
                name: name.clone(),
                created_by_uid: caller.uid.clone(),
                create_idempotency_key: idempotency_key,
                created_at_ms: now_ms,
                updated_at_ms: now_ms,
            },
        );
        state.members.insert(
            membership_id.clone(),
            MemMember {
                membership_id: membership_id.clone(),
                workspace_id: workspace_id.clone(),
                uid: caller.uid.clone(),
                display_name: caller_display_name(caller),
                role: WorkspaceRole::Owner,
                joined_at_ms: now_ms,
                removed_at_ms: None,
            },
        );
        state.audit_logs.push(WorkspaceAuditRecord {
            log_id: Uuid::new_v4().to_string(),
            workspace_id: workspace_id.clone(),
            action: "workspace_created".into(),
            actor_uid: caller.uid.clone(),
            actor_membership_id: Some(membership_id.clone()),
            target_uid: None,
            target_membership_id: None,
            invitation_id: None,
            detail_json: r#"{"role":"owner"}"#.into(),
            created_at_ms: now_ms,
        });
        Ok(CreateWorkspaceOutcome {
            created: true,
            workspace: WorkspaceSummary {
                workspace_id,
                name,
                my_membership_id: membership_id,
                my_role: WorkspaceRole::Owner,
                created_at_ms: now_ms,
                updated_at_ms: now_ms,
            },
        })
    }

    fn get_workspace_details(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError> {
        let state = self.inner.lock().unwrap();
        Self::details(&state, &caller.uid, workspace_id, now_ms)
    }

    fn rename_workspace(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        name: &str,
        now_ms: i64,
    ) -> Result<WorkspaceSummary, MembershipError> {
        let name = sanitize_workspace_name(name)?;
        let mut state = self.inner.lock().unwrap();
        let actor = Self::active_member(&state, workspace_id, &caller.uid)
            .cloned()
            .ok_or(MembershipError::Forbidden)?;
        if !actor.role.can_rename() {
            return Err(MembershipError::Forbidden);
        }
        let _ = recheck_session()?;
        let ws = state
            .workspaces
            .get_mut(workspace_id)
            .ok_or(MembershipError::WorkspaceNotFound)?;
        ws.name = name;
        ws.updated_at_ms = now_ms;
        state.audit_logs.push(WorkspaceAuditRecord {
            log_id: Uuid::new_v4().to_string(),
            workspace_id: workspace_id.to_owned(),
            action: "workspace_renamed".into(),
            actor_uid: caller.uid.clone(),
            actor_membership_id: Some(actor.membership_id),
            target_uid: None,
            target_membership_id: None,
            invitation_id: None,
            detail_json: r#"{}"#.into(),
            created_at_ms: now_ms,
        });
        Self::summary(&state, workspace_id, &caller.uid)
    }

    fn create_invitation(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        recipient_email: &str,
        offered_role: WorkspaceRole,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<CreatedInvitation, MembershipError> {
        let (recipient_email, recipient_email_norm) = sanitize_email(recipient_email)?;
        if ttl_ms <= 0 {
            return Err(MembershipError::InvalidInput(
                "Invitation TTL must be positive.".into(),
            ));
        }
        let mut state = self.inner.lock().unwrap();
        let actor = Self::active_member(&state, workspace_id, &caller.uid)
            .cloned()
            .ok_or(MembershipError::Forbidden)?;
        if !actor.role.can_grant_invite_role(offered_role) {
            return Err(MembershipError::Forbidden);
        }
        let _ = recheck_session()?;
        let invitation_id = Uuid::new_v4().to_string();
        let invite_code = random_invite_code()?;
        let code_digest = sha256_hex(&invite_code);
        let expires_at_ms = now_ms.saturating_add(ttl_ms);
        state.invitations.insert(
            invitation_id.clone(),
            MemInvite {
                invitation_id: invitation_id.clone(),
                workspace_id: workspace_id.to_owned(),
                code_digest,
                recipient_email: recipient_email.clone(),
                recipient_email_norm,
                offered_role,
                invited_by_membership_id: actor.membership_id.clone(),
                invited_by_uid: caller.uid.clone(),
                invited_by_display_name: actor.display_name.clone(),
                created_at_ms: now_ms,
                expires_at_ms,
                accepted_at_ms: None,
                accepted_by_uid: None,
                revoked_at_ms: None,
            },
        );
        state.audit_logs.push(WorkspaceAuditRecord {
            log_id: Uuid::new_v4().to_string(),
            workspace_id: workspace_id.to_owned(),
            action: "invitation_created".into(),
            actor_uid: caller.uid.clone(),
            actor_membership_id: Some(actor.membership_id.clone()),
            target_uid: None,
            target_membership_id: None,
            invitation_id: Some(invitation_id.clone()),
            detail_json: serde_json::json!({ "offered_role": offered_role.as_str() }).to_string(),
            created_at_ms: now_ms,
        });
        Ok(CreatedInvitation {
            invitation: WorkspaceInvitationView {
                invitation_id,
                workspace_id: workspace_id.to_owned(),
                recipient_email,
                offered_role,
                status: InvitationStatus::Pending,
                invited_by_membership_id: actor.membership_id,
                invited_by_display_name: actor.display_name,
                created_at_ms: now_ms,
                expires_at_ms,
                accepted_at_ms: None,
                revoked_at_ms: None,
            },
            invite_code,
        })
    }

    fn revoke_invitation(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        invitation_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceInvitationView, MembershipError> {
        let mut state = self.inner.lock().unwrap();
        let actor = Self::active_member(&state, workspace_id, &caller.uid)
            .cloned()
            .ok_or(MembershipError::Forbidden)?;
        let inv = state
            .invitations
            .get_mut(invitation_id)
            .filter(|i| i.workspace_id == workspace_id)
            .ok_or(MembershipError::InvitationNotFound)?;
        if !actor.role.can_manage_invitation(inv.offered_role) {
            return Err(MembershipError::Forbidden);
        }
        if inv.accepted_at_ms.is_some() {
            return Err(MembershipError::InvitationAlreadyUsed);
        }
        if inv.revoked_at_ms.is_none() && now_ms > inv.expires_at_ms {
            return Err(MembershipError::InvitationExpired);
        }
        let _ = recheck_session()?;
        if inv.revoked_at_ms.is_none() {
            inv.revoked_at_ms = Some(now_ms);
        }
        let view = WorkspaceInvitationView {
            invitation_id: inv.invitation_id.clone(),
            workspace_id: inv.workspace_id.clone(),
            recipient_email: inv.recipient_email.clone(),
            offered_role: inv.offered_role,
            status: InvitationStatus::Revoked,
            invited_by_membership_id: inv.invited_by_membership_id.clone(),
            invited_by_display_name: inv.invited_by_display_name.clone(),
            created_at_ms: inv.created_at_ms,
            expires_at_ms: inv.expires_at_ms,
            accepted_at_ms: inv.accepted_at_ms,
            revoked_at_ms: inv.revoked_at_ms,
        };
        Ok(view)
    }

    fn replace_invitation(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        invitation_id: &str,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<CreatedInvitation, MembershipError> {
        let existing = {
            let mut state = self.inner.lock().unwrap();
            let actor = Self::active_member(&state, workspace_id, &caller.uid)
                .cloned()
                .ok_or(MembershipError::Forbidden)?;
            let inv = state
                .invitations
                .get_mut(invitation_id)
                .filter(|i| i.workspace_id == workspace_id)
                .ok_or(MembershipError::InvitationNotFound)?;
            if !actor.role.can_manage_invitation(inv.offered_role) {
                return Err(MembershipError::Forbidden);
            }
            if inv.accepted_at_ms.is_some() {
                return Err(MembershipError::InvitationAlreadyUsed);
            }
            let _ = recheck_session()?;
            if inv.revoked_at_ms.is_none() {
                inv.revoked_at_ms = Some(now_ms);
            }
            (inv.recipient_email.clone(), inv.offered_role)
        };
        self.create_invitation(
            caller,
            recheck_session,
            workspace_id,
            &existing.0,
            existing.1,
            now_ms,
            ttl_ms,
        )
    }

    fn preview_invitation(
        &self,
        caller: &VerifiedSession,
        invite_code: &str,
        now_ms: i64,
    ) -> Result<InvitationPreview, MembershipError> {
        let code = invite_code.trim();
        if code.is_empty() {
            return Err(MembershipError::InvalidInput(
                "Enter an invitation code.".into(),
            ));
        }
        let digest = sha256_hex(code);
        let mut state = self.inner.lock().unwrap();
        let cutoff = now_ms.saturating_sub(INVITE_RATE_LIMIT_WINDOW_MS);
        let entry = state
            .rate_limits
            .entry(("preview".into(), caller.uid.clone()))
            .or_default();
        entry.retain(|t| *t >= cutoff);
        if entry.len() >= INVITE_RATE_LIMIT_MAX_ATTEMPTS {
            return Err(MembershipError::RateLimited);
        }
        entry.push(now_ms);

        let inv = state
            .invitations
            .values()
            .find(|i| ct_eq_str(&i.code_digest, &digest))
            .cloned()
            .ok_or(MembershipError::InvitationNotFound)?;
        if !ct_eq_str(&canonical_email(&caller.email), &inv.recipient_email_norm) {
            return Err(MembershipError::WrongAccount);
        }
        let existing = Self::active_member(&state, &inv.workspace_id, &caller.uid).cloned();
        if inv.accepted_at_ms.is_some() {
            if inv.accepted_by_uid.as_deref() == Some(caller.uid.as_str()) && existing.is_some() {
                let ws_name = state
                    .workspaces
                    .get(&inv.workspace_id)
                    .map(|w| w.name.clone())
                    .ok_or(MembershipError::WorkspaceNotFound)?;
                return Ok(InvitationPreview {
                    invitation_id: inv.invitation_id,
                    workspace_id: inv.workspace_id,
                    workspace_name: ws_name,
                    offered_role: inv.offered_role,
                    expires_at_ms: inv.expires_at_ms,
                    already_member: true,
                    existing_role: existing.map(|m| m.role),
                });
            }
            return Err(MembershipError::InvitationAlreadyUsed);
        }
        if inv.revoked_at_ms.is_some() {
            return Err(MembershipError::InvitationRevoked);
        }
        if now_ms > inv.expires_at_ms {
            return Err(MembershipError::InvitationExpired);
        }
        let issuer_ok = state
            .members
            .get(&inv.invited_by_membership_id)
            .is_some_and(|m| {
                m.removed_at_ms.is_none() && m.role.can_grant_invite_role(inv.offered_role)
            });
        if !issuer_ok {
            return Err(MembershipError::InvitationRevoked);
        }
        let ws_name = state
            .workspaces
            .get(&inv.workspace_id)
            .map(|w| w.name.clone())
            .ok_or(MembershipError::WorkspaceNotFound)?;
        Ok(InvitationPreview {
            invitation_id: inv.invitation_id,
            workspace_id: inv.workspace_id,
            workspace_name: ws_name,
            offered_role: inv.offered_role,
            expires_at_ms: inv.expires_at_ms,
            already_member: existing.is_some(),
            existing_role: existing.map(|m| m.role),
        })
    }

    fn accept_invitation(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        invite_code: &str,
        now_ms: i64,
    ) -> Result<AcceptInvitationOutcome, MembershipError> {
        let code = invite_code.trim();
        if code.is_empty() {
            return Err(MembershipError::InvalidInput(
                "Enter an invitation code.".into(),
            ));
        }
        let digest = sha256_hex(code);
        let mut state = self.inner.lock().unwrap();
        let cutoff = now_ms.saturating_sub(INVITE_RATE_LIMIT_WINDOW_MS);
        let entry = state
            .rate_limits
            .entry(("accept".into(), caller.uid.clone()))
            .or_default();
        entry.retain(|t| *t >= cutoff);
        if entry.len() >= INVITE_RATE_LIMIT_MAX_ATTEMPTS {
            return Err(MembershipError::RateLimited);
        }
        entry.push(now_ms);

        let inv = state
            .invitations
            .values()
            .find(|i| ct_eq_str(&i.code_digest, &digest))
            .cloned()
            .ok_or(MembershipError::InvitationNotFound)?;
        if !ct_eq_str(&canonical_email(&caller.email), &inv.recipient_email_norm) {
            return Err(MembershipError::WrongAccount);
        }
        let existing = Self::active_member(&state, &inv.workspace_id, &caller.uid).cloned();
        if inv.accepted_at_ms.is_some() {
            if inv.accepted_by_uid.as_deref() == Some(caller.uid.as_str()) {
                if let Some(member) = existing {
                    let _ = recheck_session()?;
                    let ws = Self::summary(&state, &inv.workspace_id, &caller.uid)?;
                    return Ok(AcceptInvitationOutcome {
                        already_member: true,
                        workspace: ws,
                        membership: WorkspaceMemberView {
                            membership_id: member.membership_id,
                            uid: member.uid,
                            display_name: member.display_name,
                            role: member.role,
                            joined_at_ms: member.joined_at_ms,
                            is_current_user: true,
                        },
                    });
                }
            }
            return Err(MembershipError::InvitationAlreadyUsed);
        }
        if inv.revoked_at_ms.is_some() {
            return Err(MembershipError::InvitationRevoked);
        }
        if now_ms > inv.expires_at_ms {
            return Err(MembershipError::InvitationExpired);
        }
        let issuer_ok = state
            .members
            .get(&inv.invited_by_membership_id)
            .is_some_and(|m| {
                m.removed_at_ms.is_none() && m.role.can_grant_invite_role(inv.offered_role)
            });
        if !issuer_ok {
            return Err(MembershipError::InvitationRevoked);
        }
        let _ = recheck_session()?;

        let (already_member, mem_id) = if let Some(member) = existing {
            (true, member.membership_id)
        } else {
            let new_id = Uuid::new_v4().to_string();
            state.members.insert(
                new_id.clone(),
                MemMember {
                    membership_id: new_id.clone(),
                    workspace_id: inv.workspace_id.clone(),
                    uid: caller.uid.clone(),
                    display_name: caller_display_name(caller),
                    role: inv.offered_role,
                    joined_at_ms: now_ms,
                    removed_at_ms: None,
                },
            );
            (false, new_id)
        };
        if let Some(stored_inv) = state.invitations.get_mut(&inv.invitation_id) {
            stored_inv.accepted_at_ms = Some(now_ms);
            stored_inv.accepted_by_uid = Some(caller.uid.clone());
        }
        let ws = Self::summary(&state, &inv.workspace_id, &caller.uid)?;
        let member = state.members.get(&mem_id).unwrap().clone();
        Ok(AcceptInvitationOutcome {
            already_member,
            workspace: ws,
            membership: WorkspaceMemberView {
                membership_id: member.membership_id,
                uid: member.uid,
                display_name: member.display_name,
                role: member.role,
                joined_at_ms: member.joined_at_ms,
                is_current_user: true,
            },
        })
    }

    fn change_member_role(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        target_membership_id: &str,
        new_role: WorkspaceRole,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError> {
        if new_role == WorkspaceRole::Owner {
            return Err(MembershipError::Forbidden);
        }
        let mut state = self.inner.lock().unwrap();
        let actor = Self::active_member(&state, workspace_id, &caller.uid)
            .cloned()
            .ok_or(MembershipError::Forbidden)?;
        let target = state
            .members
            .get(target_membership_id)
            .filter(|m| m.workspace_id == workspace_id && m.removed_at_ms.is_none())
            .cloned()
            .ok_or(MembershipError::MemberNotFound)?;
        if target.uid == caller.uid {
            return Err(MembershipError::Forbidden);
        }
        if target.role == WorkspaceRole::Owner {
            return Err(MembershipError::LastOwnerProtected);
        }
        if !actor.role.can_change_member_role(target.role, new_role) {
            return Err(MembershipError::Forbidden);
        }
        let _ = recheck_session()?;
        if let Some(m) = state.members.get_mut(target_membership_id) {
            m.role = new_role;
        }
        Self::revoke_ungrantable(
            &mut state,
            workspace_id,
            target_membership_id,
            Some(new_role),
            &caller.uid,
            now_ms,
        );
        Self::details(&state, &caller.uid, workspace_id, now_ms)
    }

    fn remove_member(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        target_membership_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError> {
        let mut state = self.inner.lock().unwrap();
        let actor = Self::active_member(&state, workspace_id, &caller.uid)
            .cloned()
            .ok_or(MembershipError::Forbidden)?;
        let target = state
            .members
            .get(target_membership_id)
            .filter(|m| m.workspace_id == workspace_id && m.removed_at_ms.is_none())
            .cloned()
            .ok_or(MembershipError::MemberNotFound)?;
        if target.role == WorkspaceRole::Owner {
            return Err(MembershipError::LastOwnerProtected);
        }
        if !actor.role.can_remove_member(target.role) {
            return Err(MembershipError::Forbidden);
        }
        if target.uid == caller.uid {
            return Err(MembershipError::InvalidInput(
                "Use Leave workspace to remove yourself.".into(),
            ));
        }
        let _ = recheck_session()?;
        if let Some(m) = state.members.get_mut(target_membership_id) {
            m.removed_at_ms = Some(now_ms);
        }
        Self::revoke_ungrantable(
            &mut state,
            workspace_id,
            target_membership_id,
            None,
            &caller.uid,
            now_ms,
        );
        for t in state.transfers.values_mut() {
            if t.workspace_id == workspace_id
                && t.status == TransferStatus::Pending
                && t.to_membership_id == target_membership_id
            {
                t.status = TransferStatus::Cancelled;
            }
        }
        Self::details(&state, &caller.uid, workspace_id, now_ms)
    }

    fn leave_workspace(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        now_ms: i64,
    ) -> Result<(), MembershipError> {
        let mut state = self.inner.lock().unwrap();
        let actor = Self::active_member(&state, workspace_id, &caller.uid)
            .cloned()
            .ok_or(MembershipError::Forbidden)?;
        if !actor.role.can_leave() {
            return Err(MembershipError::LastOwnerProtected);
        }
        let _ = recheck_session()?;
        if let Some(m) = state.members.get_mut(&actor.membership_id) {
            m.removed_at_ms = Some(now_ms);
        }
        Self::revoke_ungrantable(
            &mut state,
            workspace_id,
            &actor.membership_id,
            None,
            &caller.uid,
            now_ms,
        );
        for t in state.transfers.values_mut() {
            if t.workspace_id == workspace_id
                && t.status == TransferStatus::Pending
                && t.to_membership_id == actor.membership_id
            {
                t.status = TransferStatus::Cancelled;
            }
        }
        Ok(())
    }

    fn initiate_ownership_transfer(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        to_membership_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError> {
        let mut state = self.inner.lock().unwrap();
        let actor = Self::active_member(&state, workspace_id, &caller.uid)
            .cloned()
            .ok_or(MembershipError::Forbidden)?;
        if actor.role != WorkspaceRole::Owner {
            return Err(MembershipError::Forbidden);
        }
        let target = state
            .members
            .get(to_membership_id)
            .filter(|m| m.workspace_id == workspace_id && m.removed_at_ms.is_none())
            .cloned()
            .ok_or(MembershipError::MemberNotFound)?;
        if target.uid == caller.uid {
            return Err(MembershipError::InvalidInput(
                "Choose another member to transfer ownership to.".into(),
            ));
        }
        if state
            .transfers
            .values()
            .any(|t| t.workspace_id == workspace_id && t.status == TransferStatus::Pending)
        {
            return Err(MembershipError::TransferAlreadyPending);
        }
        let _ = recheck_session()?;
        let transfer_id = Uuid::new_v4().to_string();
        state.transfers.insert(
            transfer_id.clone(),
            MemTransfer {
                transfer_id,
                workspace_id: workspace_id.to_owned(),
                from_membership_id: actor.membership_id,
                from_uid: actor.uid,
                from_display_name: actor.display_name,
                to_membership_id: target.membership_id,
                to_uid: target.uid,
                to_display_name: target.display_name,
                status: TransferStatus::Pending,
                created_at_ms: now_ms,
            },
        );
        Self::details(&state, &caller.uid, workspace_id, now_ms)
    }

    fn cancel_ownership_transfer(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        transfer_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError> {
        let mut state = self.inner.lock().unwrap();
        let actor = Self::active_member(&state, workspace_id, &caller.uid)
            .cloned()
            .ok_or(MembershipError::Forbidden)?;
        if actor.role != WorkspaceRole::Owner {
            return Err(MembershipError::Forbidden);
        }
        let _ = recheck_session()?;
        let transfer = state
            .transfers
            .get_mut(transfer_id)
            .filter(|t| t.workspace_id == workspace_id && t.status == TransferStatus::Pending)
            .ok_or(MembershipError::TransferNotPending)?;
        transfer.status = TransferStatus::Cancelled;
        Self::details(&state, &caller.uid, workspace_id, now_ms)
    }

    fn accept_ownership_transfer(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        transfer_id: &str,
        now_ms: i64,
    ) -> Result<WorkspaceDetails, MembershipError> {
        let mut state = self.inner.lock().unwrap();
        let transfer = state
            .transfers
            .get(transfer_id)
            .filter(|t| t.workspace_id == workspace_id && t.status == TransferStatus::Pending)
            .cloned()
            .ok_or(MembershipError::TransferNotPending)?;
        if transfer.to_uid != caller.uid {
            return Err(MembershipError::Forbidden);
        }
        let current_owner = state
            .members
            .get(&transfer.from_membership_id)
            .filter(|m| m.removed_at_ms.is_none() && m.role == WorkspaceRole::Owner)
            .cloned()
            .ok_or(MembershipError::TransferNotPending)?;
        let current_target = state
            .members
            .get(&transfer.to_membership_id)
            .filter(|m| m.removed_at_ms.is_none() && m.uid == caller.uid)
            .cloned()
            .ok_or(MembershipError::Forbidden)?;
        let _ = recheck_session()?;
        state
            .members
            .get_mut(&current_owner.membership_id)
            .unwrap()
            .role = WorkspaceRole::Admin;
        state
            .members
            .get_mut(&current_target.membership_id)
            .unwrap()
            .role = WorkspaceRole::Owner;
        state.transfers.get_mut(transfer_id).unwrap().status = TransferStatus::Accepted;
        Self::revoke_ungrantable(
            &mut state,
            workspace_id,
            &current_owner.membership_id,
            Some(WorkspaceRole::Admin),
            &caller.uid,
            now_ms,
        );
        Self::details(&state, &caller.uid, workspace_id, now_ms)
    }

    fn list_audit_records(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
    ) -> Result<Vec<WorkspaceAuditRecord>, MembershipError> {
        let state = self.inner.lock().unwrap();
        let actor = Self::active_member(&state, workspace_id, &caller.uid)
            .ok_or(MembershipError::Forbidden)?;
        if !matches!(actor.role, WorkspaceRole::Owner | WorkspaceRole::Admin) {
            return Err(MembershipError::Forbidden);
        }
        Ok(state
            .audit_logs
            .iter()
            .filter(|r| r.workspace_id == workspace_id)
            .cloned()
            .collect())
    }
}
