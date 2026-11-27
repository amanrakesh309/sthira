//! Scoped shared agent access for team workspaces.
//!
//! Authorization & transaction boundary (mirrors `shared_content`):
//! 1. Member endpoints validate the caller's opaque device-session credential through the CW1
//!    `AuthSessionStore`. Delegated MCP endpoints validate a separate, revocable connection token
//!    whose SHA-256 hash is stored server-side; the plaintext credential is returned exactly once
//!    at creation and never persisted.
//! 2. Every request re-derives the intersection of current membership + role, the issuing device
//!    session, workspace agent policy, connection grant, project availability, explicit page
//!    allowlist and the current `shared_project_pages` generation, inside one `IMMEDIATE`
//!    transaction. Reads are filtered by that intersection before search, snippets, pagination or
//!    revision retrieval.
//! 3. Writes re-check the device binding (and, for member endpoints, `SessionRecheck`) immediately
//!    before `COMMIT`, matching the CW2 write boundary.
//! 4. Agent proposals reuse `shared_change_proposals` for edits and a companion table for new
//!    pages. Human review, self-review rejection and submission/revision staleness stay in the
//!    service; agents never approve, publish or impersonate a reviewer.

use crate::membership::{
    caller_display_name, require_active_membership, sanitize_idempotency_key, SessionRecheck,
};
use crate::shared_content::{
    insert_activity_with_project, load_page_row, load_project_summary, load_revision_row,
    lookup_idempotent_response, payload_digest, sanitize_optional_note, sanitize_page_content,
    sanitize_page_title, sanitize_proposal_summary, save_idempotent_response, SharedContentError,
    SharedPageRevisionView, SharedPageSummary, SqliteSharedContentStore, MAX_PAGE_CONTENT_BYTES,
};
use crate::store::{sha256_hex, StoreError, VerifiedSession};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const AGENT_TOKEN_PREFIX: &str = "stha_";
pub const MAX_AGENT_LABEL_LEN: usize = 96;
pub const MAX_AGENT_CLIENT_LEN: usize = 64;
pub const MAX_AGENT_MODEL_LEN: usize = 120;
pub const MAX_AGENT_PROVIDER_LEN: usize = 120;
pub const MAX_AGENT_PAGES_PER_CONNECTION: usize = 100;
pub const MAX_AGENT_LIST_LIMIT: usize = 50;
pub const DEFAULT_AGENT_LIST_LIMIT: usize = 20;
pub const AGENT_SNIPPET_CHARS: usize = 180;
pub const AGENT_AUDIT_ROWS_PER_WORKSPACE: i64 = 500;

/// Re-verifies the device session bound to a delegated connection without accepting a credential.
pub type AgentSessionCheck<'a> =
    &'a (dyn Fn(&str, &str, &str) -> Result<VerifiedSession, StoreError> + Sync);

/// Current session checks for the reviewer and the member who delegated the agent.
pub struct ReviewSessionChecks<'a> {
    pub reviewer: SessionRecheck<'a>,
    pub agent: AgentSessionCheck<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentPermission {
    Read,
    Suggest,
}

impl AgentPermission {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentPermission::Read => "read",
            AgentPermission::Suggest => "suggest",
        }
    }

    pub fn from_db(raw: &str) -> Result<Self, SharedContentError> {
        match raw {
            "read" => Ok(AgentPermission::Read),
            "suggest" => Ok(AgentPermission::Suggest),
            _ => Err(SharedContentError::InvalidInput(
                "Unknown agent permission.".into(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAccessPolicyView {
    pub workspace_id: String,
    pub enabled: bool,
    pub updated_by_display_name: Option<String>,
    pub updated_at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConnectionPageView {
    pub page_id: String,
    pub title: String,
    pub current_revision_number: i64,
    pub available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConnectionView {
    pub connection_id: String,
    pub workspace_id: String,
    pub project_id: String,
    pub project_name: String,
    pub member_uid: String,
    pub member_display_name: String,
    pub device_id: String,
    pub device_label: String,
    pub label: String,
    pub client_name: String,
    pub permission: AgentPermission,
    pub generation: i64,
    pub pages: Vec<AgentConnectionPageView>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub last_used_at_ms: Option<i64>,
    pub revoked_at_ms: Option<i64>,
    pub revoked_by_display_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentConnectionInput {
    pub project_id: String,
    pub client_name: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub permission: Option<AgentPermission>,
    pub page_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedAgentConnection {
    pub connection: AgentConnectionView,
    /// Returned exactly once. Never logged, exported to MCP config, or placed in process arguments.
    pub credential: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentConnectionInput {
    pub expected_generation: i64,
    pub permission: AgentPermission,
    pub page_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConnectionLimits {
    pub max_pages_per_list: usize,
    pub max_content_bytes: usize,
    pub can_suggest: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConnectionStatusView {
    pub connection_id: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub project_id: String,
    pub project_name: String,
    pub member_display_name: String,
    pub device_label: String,
    pub permission: AgentPermission,
    pub generation: i64,
    pub created_at_ms: i64,
    pub pages: Vec<AgentConnectionPageView>,
    pub limits: AgentConnectionLimits,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSharedPageItem {
    pub page_id: String,
    pub title: String,
    pub revision_number: i64,
    pub updated_at_ms: i64,
    pub snippet: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSharedPageList {
    pub items: Vec<AgentSharedPageItem>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSharedPageRead {
    pub page_id: String,
    pub workspace_id: String,
    pub project_id: String,
    pub title: String,
    pub content_md: String,
    pub revision_id: String,
    pub revision_number: i64,
    pub content_sha256: String,
    pub page_updated_at_ms: i64,
    pub complete: bool,
    pub provenance: crate::shared_content::ShareableProvenance,
    pub contributor_names: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProposalSource {
    pub page_id: String,
    pub page_title: String,
    pub revision_number: i64,
    pub content_sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentProposalKind {
    Edit,
    NewPage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProposalView {
    pub proposal_id: String,
    pub kind: AgentProposalKind,
    pub workspace_id: String,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub target_page_id: Option<String>,
    pub target_title: String,
    pub base_revision_number: Option<i64>,
    pub proposed_title: String,
    pub proposed_content_md: String,
    pub summary: String,
    pub status: String,
    pub submission_count: i64,
    pub author_uid: String,
    pub author_display_name: String,
    pub connection_id: Option<String>,
    pub connection_label: Option<String>,
    pub agent_label: String,
    pub model_label: String,
    pub provider_label: String,
    pub model_trust: String,
    pub sources: Vec<AgentProposalSource>,
    pub is_stale: bool,
    pub stale_reason: Option<String>,
    pub review_eligible: bool,
    pub revocation_state: String,
    pub applied_page_id: Option<String>,
    pub applied_revision_number: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub reviewed_at_ms: Option<i64>,
    pub reviewer_display_name: Option<String>,
    #[serde(default)]
    pub reviewer_note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEditProposalInput {
    pub request_id: String,
    pub page_id: String,
    pub base_revision_number: i64,
    pub base_content_sha256: String,
    pub proposed_title: String,
    pub proposed_content_md: String,
    pub summary: String,
    pub agent_label: String,
    pub model_label: String,
    pub provider_label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentNewPageProposalInput {
    pub request_id: String,
    #[serde(default)]
    pub project_id: Option<String>,
    pub proposed_title: String,
    pub proposed_content_md: String,
    pub summary: String,
    pub agent_label: String,
    pub model_label: String,
    pub provider_label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRequestReceipt {
    pub request_id: String,
    pub operation: String,
    pub proposal_id: String,
    pub status: String,
    pub is_stale: bool,
    pub stale_reason: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewAgentProposalInput {
    pub action: String,
    pub expected_submission_number: i64,
    #[serde(default)]
    pub note: Option<String>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug)]
struct AgentConnectionRow {
    connection_id: String,
    workspace_id: String,
    project_id: String,
    member_uid: String,
    member_display_name: String,
    device_id: String,
    issuer_session_id: String,
    device_label: String,
    label: String,
    client_name: String,
    permission: AgentPermission,
    generation: i64,
    created_at_ms: i64,
    revoked_at_ms: Option<i64>,
}

#[derive(Clone, Debug)]
struct GrantPageRow {
    page_id: String,
    membership_id: String,
    membership_generation: i64,
    grant_generation: i64,
}

#[derive(Clone, Debug)]
struct ReadBasisRow {
    access_version: i64,
    access_digest: String,
    page_id: String,
    revision_number: i64,
    content_sha256: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSourceBasis {
    #[serde(default = "initial_access_version")]
    project_access_version: i64,
    #[serde(default = "initial_access_version")]
    access_version: i64,
    #[serde(default = "legacy_access_digest")]
    access_digest: String,
    page_id: String,
    revision_number: i64,
    content_sha256: String,
    membership_id: String,
    membership_generation: i64,
}

fn initial_access_version() -> i64 {
    1
}
fn legacy_access_digest() -> String {
    sha256_hex("workspace")
}

/// Persistence contract for scoped shared agent access.
pub trait SharedAgentAccessStore: Send + Sync {
    fn get_agent_access_policy(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
    ) -> Result<AgentAccessPolicyView, SharedContentError>;

    fn set_agent_access_policy(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        enabled: bool,
        now_ms: i64,
    ) -> Result<AgentAccessPolicyView, SharedContentError>;

    fn create_agent_connection(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        input: &CreateAgentConnectionInput,
        now_ms: i64,
    ) -> Result<CreatedAgentConnection, SharedContentError>;

    fn list_agent_connections(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        include_workspace: bool,
    ) -> Result<Vec<AgentConnectionView>, SharedContentError>;

    fn get_agent_connection(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        connection_id: &str,
    ) -> Result<AgentConnectionView, SharedContentError>;

    fn update_agent_connection(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        connection_id: &str,
        input: &UpdateAgentConnectionInput,
        now_ms: i64,
    ) -> Result<AgentConnectionView, SharedContentError>;

    fn revoke_agent_connection(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        connection_id: &str,
        now_ms: i64,
    ) -> Result<AgentConnectionView, SharedContentError>;

    fn list_agent_proposals(
        &self,
        caller: &VerifiedSession,
        check_agent_session: AgentSessionCheck<'_>,
        workspace_id: &str,
        include_closed: bool,
    ) -> Result<Vec<AgentProposalView>, SharedContentError>;

    fn get_agent_proposal(
        &self,
        caller: &VerifiedSession,
        check_agent_session: AgentSessionCheck<'_>,
        workspace_id: &str,
        proposal_id: &str,
    ) -> Result<AgentProposalView, SharedContentError>;

    fn review_agent_page_proposal(
        &self,
        caller: &VerifiedSession,
        checks: ReviewSessionChecks<'_>,
        workspace_id: &str,
        proposal_id: &str,
        input: &ReviewAgentProposalInput,
        now_ms: i64,
    ) -> Result<AgentProposalView, SharedContentError>;

    fn agent_connection_status(
        &self,
        credential: &str,
        check_session: AgentSessionCheck<'_>,
        now_ms: i64,
    ) -> Result<AgentConnectionStatusView, SharedContentError>;

    fn list_agent_pages(
        &self,
        credential: &str,
        check_session: AgentSessionCheck<'_>,
        search: Option<&str>,
        cursor: Option<&str>,
        limit: Option<usize>,
        now_ms: i64,
    ) -> Result<AgentSharedPageList, SharedContentError>;

    fn read_agent_page(
        &self,
        credential: &str,
        check_session: AgentSessionCheck<'_>,
        page_id: &str,
        now_ms: i64,
    ) -> Result<AgentSharedPageRead, SharedContentError>;

    fn create_agent_edit_proposal(
        &self,
        credential: &str,
        check_session: AgentSessionCheck<'_>,
        input: &AgentEditProposalInput,
        now_ms: i64,
    ) -> Result<AgentProposalView, SharedContentError>;

    fn create_agent_new_page_proposal(
        &self,
        credential: &str,
        check_session: AgentSessionCheck<'_>,
        input: &AgentNewPageProposalInput,
        now_ms: i64,
    ) -> Result<AgentProposalView, SharedContentError>;

    fn agent_request_status(
        &self,
        credential: &str,
        check_session: AgentSessionCheck<'_>,
        request_id: &str,
        now_ms: i64,
    ) -> Result<AgentRequestReceipt, SharedContentError>;

    fn agent_proposal_status(
        &self,
        credential: &str,
        check_session: AgentSessionCheck<'_>,
        proposal_id: &str,
        now_ms: i64,
    ) -> Result<AgentProposalView, SharedContentError>;
}

fn sanitize_label(raw: &str, field: &str) -> Result<String, SharedContentError> {
    let clean: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if clean.is_empty() {
        return Err(SharedContentError::InvalidInput(format!(
            "{field} is required."
        )));
    }
    if clean.chars().count() > MAX_AGENT_LABEL_LEN {
        return Err(SharedContentError::InvalidInput(format!(
            "{field} must be at most {MAX_AGENT_LABEL_LEN} characters."
        )));
    }
    Ok(clean)
}

fn sanitize_short(raw: &str, field: &str, max: usize) -> Result<String, SharedContentError> {
    let clean: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if clean.is_empty() {
        return Err(SharedContentError::InvalidInput(format!(
            "{field} is required."
        )));
    }
    if clean.chars().count() > max {
        return Err(SharedContentError::InvalidInput(format!(
            "{field} must be at most {max} characters."
        )));
    }
    Ok(clean)
}

fn sanitize_page_ids(raw: &[String]) -> Result<Vec<String>, SharedContentError> {
    let mut seen = std::collections::BTreeSet::new();
    for id in raw {
        let id = id.trim();
        if id.is_empty() {
            return Err(SharedContentError::InvalidInput(
                "Page selection contains an empty page ID.".into(),
            ));
        }
        if !seen.insert(id.to_owned()) {
            return Err(SharedContentError::InvalidInput(
                "Page selection contains a duplicate page ID.".into(),
            ));
        }
    }
    if seen.is_empty() {
        return Err(SharedContentError::InvalidInput(
            "Select at least one shared project page for this connection.".into(),
        ));
    }
    if seen.len() > MAX_AGENT_PAGES_PER_CONNECTION {
        return Err(SharedContentError::InvalidInput(format!(
            "A connection can include at most {MAX_AGENT_PAGES_PER_CONNECTION} pages."
        )));
    }
    Ok(seen.into_iter().collect())
}

fn random_agent_token() -> Result<String, SharedContentError> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes)
        .map_err(|_| SharedContentError::InvalidInput("Random generator unavailable.".into()))?;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    Ok(format!(
        "{AGENT_TOKEN_PREFIX}{}",
        URL_SAFE_NO_PAD.encode(bytes)
    ))
}

fn sanitize_request_id(raw: &str) -> Result<String, SharedContentError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(SharedContentError::InvalidInput(
            "A request ID is required for idempotent retries.".into(),
        ));
    }
    if trimmed.len() > 96
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
    {
        return Err(SharedContentError::InvalidInput(
            "Request IDs may use letters, digits, '-', '_', '.' and ':'. Maximum 96 characters."
                .into(),
        ));
    }
    Ok(trimmed.to_owned())
}

#[allow(clippy::too_many_arguments)]
fn audit(
    tx: &Transaction<'_>,
    workspace_id: &str,
    connection_id: &str,
    member_uid: &str,
    action: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    outcome: &str,
    proposal_id: Option<&str>,
    now_ms: i64,
) -> Result<(), SharedContentError> {
    tx.execute(
        "INSERT INTO shared_agent_audit (
            workspace_id, connection_id, member_uid, action, resource_type,
            resource_id, outcome, proposal_id, created_at_ms
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            workspace_id,
            connection_id,
            member_uid,
            action,
            resource_type,
            resource_id,
            outcome,
            proposal_id,
            now_ms,
        ],
    )?;
    tx.execute(
        "DELETE FROM shared_agent_audit
         WHERE workspace_id = ?1 AND seq NOT IN (
             SELECT seq FROM shared_agent_audit
             WHERE workspace_id = ?1 ORDER BY seq DESC LIMIT ?2
         )",
        params![workspace_id, AGENT_AUDIT_ROWS_PER_WORKSPACE],
    )?;
    Ok(())
}

fn check_binding(
    row: &AgentConnectionRow,
    check_session: AgentSessionCheck<'_>,
) -> Result<(), SharedContentError> {
    let verified = check_session(&row.issuer_session_id, &row.member_uid, &row.device_id)?;
    if verified.uid != row.member_uid || verified.device_id != row.device_id {
        return Err(SharedContentError::AgentConnectionUnavailable);
    }
    Ok(())
}

fn load_policy_enabled(
    tx: &Transaction<'_>,
    workspace_id: &str,
) -> Result<bool, SharedContentError> {
    let row: Option<i64> = tx
        .query_row(
            "SELECT enabled FROM shared_agent_workspace_policies WHERE workspace_id = ?1",
            params![workspace_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(row == Some(1))
}

fn require_policy_enabled(
    tx: &Transaction<'_>,
    workspace_id: &str,
) -> Result<(), SharedContentError> {
    if load_policy_enabled(tx, workspace_id)? {
        Ok(())
    } else {
        Err(SharedContentError::AgentAccessDisabled)
    }
}

/// Authorizes a delegated connection token and re-derives every current-validity condition.
fn authorize_agent_token(
    tx: &Transaction<'_>,
    credential: &str,
    now_ms: i64,
) -> Result<AgentConnectionRow, SharedContentError> {
    let credential = credential.trim();
    if !credential.starts_with(AGENT_TOKEN_PREFIX) || credential.len() > 128 {
        return Err(SharedContentError::AgentConnectionUnavailable);
    }
    let token_hash = sha256_hex(credential);
    let row = tx
        .query_row(
            "SELECT c.connection_id, c.workspace_id, c.project_id, c.member_uid,
                    c.member_display_name, c.device_id, c.issuer_session_id, c.device_label,
                    c.label, c.client_name, c.permission, c.generation, c.created_at_ms,
                    c.revoked_at_ms, t.revoked_at_ms
             FROM shared_agent_connection_tokens t
             JOIN shared_agent_connections c ON c.connection_id = t.connection_id
             WHERE t.token_hash = ?1",
            params![token_hash],
            |r| {
                let permission: String = r.get(10)?;
                Ok((
                    AgentConnectionRow {
                        connection_id: r.get(0)?,
                        workspace_id: r.get(1)?,
                        project_id: r.get(2)?,
                        member_uid: r.get(3)?,
                        member_display_name: r.get(4)?,
                        device_id: r.get(5)?,
                        issuer_session_id: r.get(6)?,
                        device_label: r.get(7)?,
                        label: r.get(8)?,
                        client_name: r.get(9)?,
                        permission: AgentPermission::Read,
                        generation: r.get(11)?,
                        created_at_ms: r.get(12)?,
                        revoked_at_ms: r.get(13)?,
                    },
                    permission,
                    r.get::<_, Option<i64>>(14)?,
                ))
            },
        )
        .optional()?;
    let Some((mut connection, permission, token_revoked_at)) = row else {
        return Err(SharedContentError::AgentConnectionUnavailable);
    };
    connection.permission = AgentPermission::from_db(&permission)?;
    if token_revoked_at.is_some() || connection.revoked_at_ms.is_some() {
        return Err(SharedContentError::AgentConnectionRevoked);
    }
    require_policy_enabled(tx, &connection.workspace_id)?;
    let member = require_active_membership(tx, &connection.workspace_id, &connection.member_uid)?;
    let ctx = crate::resource_access::require_caller_access(
        tx,
        &connection.workspace_id,
        &connection.member_uid,
    )?;
    if !crate::resource_access::project_readable(
        tx,
        &connection.workspace_id,
        &connection.project_id,
        &ctx,
    )? {
        return Err(SharedContentError::AgentStaleBasis(
            "The initiating member no longer has access to this project.".into(),
        ));
    }
    if connection.permission == AgentPermission::Suggest && !member.role.can_author_shared_content()
    {
        // A role downgrade reduces the effective grant without rewriting the stored grant. Role
        // restoration restores capability, but stale proposal bases remain stale.
        connection.permission = AgentPermission::Read;
    }
    let project = load_project_summary(tx, &connection.workspace_id, &connection.project_id)?;
    if project.archived_at_ms.is_some() {
        return Err(SharedContentError::AgentProjectUnavailable);
    }
    // A delegated grant is always intersected with the initiating member's current human access.
    let member_access = crate::resource_access::require_caller_access(
        tx,
        &connection.workspace_id,
        &connection.member_uid,
    )?;
    if !crate::resource_access::project_readable(
        tx,
        &connection.workspace_id,
        &connection.project_id,
        &member_access,
    )? {
        return Err(SharedContentError::AgentProjectUnavailable);
    }
    tx.execute(
        "UPDATE shared_agent_connections SET last_used_at_ms = ?2 WHERE connection_id = ?1",
        params![connection.connection_id, now_ms],
    )?;
    Ok(connection)
}

fn load_active_membership_for_page(
    tx: &Transaction<'_>,
    workspace_id: &str,
    project_id: &str,
    page_id: &str,
) -> Result<Option<GrantPageRow>, SharedContentError> {
    Ok(tx
        .query_row(
            "SELECT membership_id, generation FROM shared_project_pages
             WHERE workspace_id = ?1 AND project_id = ?2 AND page_id = ?3
               AND detached_at_ms IS NULL",
            params![workspace_id, project_id, page_id],
            |r| {
                Ok(GrantPageRow {
                    page_id: page_id.to_owned(),
                    membership_id: r.get(0)?,
                    membership_generation: r.get(1)?,
                    grant_generation: 0,
                })
            },
        )
        .optional()?)
}

fn load_grant_page(
    tx: &Transaction<'_>,
    connection: &AgentConnectionRow,
    page_id: &str,
) -> Result<(GrantPageRow, SharedPageSummary), SharedContentError> {
    let grant = tx
        .query_row(
            "SELECT p.membership_id, p.membership_generation, p.grant_generation
             FROM shared_agent_connection_pages p
             JOIN shared_project_pages pp
               ON pp.workspace_id = p.workspace_id AND pp.project_id = p.project_id
              AND pp.page_id = p.page_id AND pp.membership_id = p.membership_id
              AND pp.generation = p.membership_generation AND pp.detached_at_ms IS NULL
             WHERE p.connection_id = ?1 AND p.workspace_id = ?2 AND p.project_id = ?3
               AND p.page_id = ?4",
            params![
                connection.connection_id,
                connection.workspace_id,
                connection.project_id,
                page_id
            ],
            |r| {
                Ok(GrantPageRow {
                    page_id: page_id.to_owned(),
                    membership_id: r.get(0)?,
                    membership_generation: r.get(1)?,
                    grant_generation: r.get(2)?,
                })
            },
        )
        .optional()?;
    let Some(grant) = grant else {
        return Err(SharedContentError::AgentPageUnavailable);
    };
    if grant.grant_generation != connection.generation {
        return Err(SharedContentError::AgentPageUnavailable);
    }
    let page = load_page_row(tx, &connection.workspace_id, page_id)?;
    if page.archived_at_ms.is_some() {
        return Err(SharedContentError::AgentPageUnavailable);
    }
    let member_access = crate::resource_access::require_caller_access(
        tx,
        &connection.workspace_id,
        &connection.member_uid,
    )?;
    if !crate::resource_access::page_readable(
        tx,
        &connection.workspace_id,
        page_id,
        &member_access,
    )? {
        return Err(SharedContentError::AgentPageUnavailable);
    }
    Ok((grant, page))
}

fn load_read_basis(
    tx: &Transaction<'_>,
    connection: &AgentConnectionRow,
) -> Result<Vec<ReadBasisRow>, SharedContentError> {
    let mut stmt = tx.prepare(
        "SELECT page_id, revision_number, content_sha256, access_version, access_digest FROM shared_agent_page_reads
         WHERE connection_id = ?1 ORDER BY page_id ASC",
    )?;
    let rows = stmt
        .query_map(params![connection.connection_id], |r| {
            Ok(ReadBasisRow {
                page_id: r.get(0)?,
                revision_number: r.get(1)?,
                content_sha256: r.get(2)?,
                access_version: r.get(3)?,
                access_digest: r.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Verifies that every recorded read is still current and still inside the active grant. This is
/// the immutable proposal basis; a detach, reattach, archive or edit changes it.
fn snapshot_source_basis(
    tx: &Transaction<'_>,
    connection: &AgentConnectionRow,
) -> Result<Vec<StoredSourceBasis>, SharedContentError> {
    let reads = load_read_basis(tx, connection)?;
    if reads.is_empty() {
        return Err(SharedContentError::AgentStaleBasis(
            "Read the complete target page in this connection before proposing a change.".into(),
        ));
    }
    let mut basis = Vec::with_capacity(reads.len());
    for read in reads {
        let (grant, page) = load_grant_page(tx, connection, &read.page_id).map_err(|_| {
            SharedContentError::AgentStaleBasis(format!(
                "“{}” is no longer available to this connection. Read the current page set again.",
                read.page_id
            ))
        })?;
        if page.current_revision_number != read.revision_number {
            return Err(SharedContentError::AgentStaleBasis(format!(
                "“{}” advanced from revision {} to revision {} after this connection read it. Read it again before proposing.",
                page.title, read.revision_number, page.current_revision_number
            )));
        }
        let revision = load_revision_row(
            tx,
            &connection.workspace_id,
            &read.page_id,
            read.revision_number,
        )?;
        if revision.content_sha256 != read.content_sha256 {
            return Err(SharedContentError::AgentStaleBasis(format!(
                "“{}” no longer matches the revision this connection read.",
                page.title
            )));
        }
        let (access_version, access_digest) =
            crate::resource_access::page_access_basis(tx, &connection.workspace_id, &read.page_id)?;
        if (read.access_version, &read.access_digest) != (access_version, &access_digest) {
            return Err(SharedContentError::AgentStaleBasis(
                "A source audience changed after it was read. Read it again before proposing."
                    .into(),
            ));
        }
        basis.push(StoredSourceBasis {
            project_access_version: crate::resource_access::project_access_row(
                tx,
                &connection.workspace_id,
                &connection.project_id,
            )?
            .access_version,
            access_version,
            access_digest,
            page_id: read.page_id,
            revision_number: read.revision_number,
            content_sha256: read.content_sha256,
            membership_id: grant.membership_id,
            membership_generation: grant.membership_generation,
        });
    }
    Ok(basis)
}

fn parse_source_basis(raw: &str) -> Vec<StoredSourceBasis> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn parse_page_cursor(
    cursor: Option<&str>,
) -> Result<(Option<i64>, Option<String>), SharedContentError> {
    let Some(raw) = cursor.map(str::trim).filter(|c| !c.is_empty()) else {
        return Ok((None, None));
    };
    let Some((ts, id)) = raw.split_once(':') else {
        return Err(SharedContentError::InvalidCursor);
    };
    let ts = ts
        .parse::<i64>()
        .map_err(|_| SharedContentError::InvalidCursor)?;
    if id.is_empty() || id.len() > 96 {
        return Err(SharedContentError::InvalidCursor);
    }
    Ok((Some(ts), Some(id.to_owned())))
}

fn page_snippet(content: &str, search: Option<&str>) -> String {
    let normalized = content
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let chars: Vec<char> = normalized.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let lower = normalized.to_lowercase();
    let start = search
        .and_then(|needle| lower.find(&needle.to_lowercase()))
        .map(|byte_index| lower[..byte_index].chars().count())
        .unwrap_or(0)
        .min(chars.len().saturating_sub(1));
    let end = (start + AGENT_SNIPPET_CHARS).min(chars.len());
    let mut snippet: String = chars[start..end].iter().collect();
    if end < chars.len() {
        snippet.push('…');
    }
    if start > 0 {
        snippet.insert(0, '…');
    }
    snippet
}

fn validate_stored_basis(
    tx: &Transaction<'_>,
    connection: &AgentConnectionRow,
    basis: &[StoredSourceBasis],
) -> Result<(), SharedContentError> {
    if basis.is_empty() {
        return Err(SharedContentError::AgentStaleBasis(
            "This proposal has no recorded source basis.".into(),
        ));
    }
    for source in basis {
        if crate::resource_access::project_access_row(
            tx,
            &connection.workspace_id,
            &connection.project_id,
        )?
        .access_version
            != source.project_access_version
        {
            return Err(SharedContentError::AgentStaleBasis("The project audience changed after this proposal was prepared. Read and propose again.".into()));
        }
        let current_access = crate::resource_access::page_access_basis(
            tx,
            &connection.workspace_id,
            &source.page_id,
        )?;
        if current_access != (source.access_version, source.access_digest.clone()) {
            return Err(SharedContentError::AgentStaleBasis("A source audience changed after this proposal was prepared. Read and propose again.".into()));
        }
        load_grant_page(tx, connection, &source.page_id).map_err(|_| {
            SharedContentError::AgentStaleBasis(
                "A source is outside the current agent grant.".into(),
            )
        })?;
        let current = tx
            .query_row(
                "SELECT generation FROM shared_project_pages
                 WHERE workspace_id = ?1 AND project_id = ?2 AND page_id = ?3
                   AND membership_id = ?4 AND detached_at_ms IS NULL",
                params![
                    connection.workspace_id,
                    connection.project_id,
                    source.page_id,
                    source.membership_id
                ],
                |r| r.get::<_, i64>(0),
            )
            .optional()?;
        match current {
            None => {
                return Err(SharedContentError::AgentStaleBasis(format!(
                    "A source page left project ‘{}’ after this proposal was prepared. Dismiss it or revise against a fresh basis.",
                    connection.project_id
                )))
            }
            Some(generation) if generation != source.membership_generation => {
                return Err(SharedContentError::AgentStaleBasis(
                    "A source page was reattached with a new project membership generation. Dismiss this proposal or prepare it again."
                        .to_string(),
                ))
            }
            Some(_) => {}
        }
        let page = load_page_row(tx, &connection.workspace_id, &source.page_id)?;
        if page.archived_at_ms.is_some() {
            return Err(SharedContentError::AgentStaleBasis(format!(
                "“{}” is archived. This proposal can no longer be applied.",
                page.title
            )));
        }
        if page.current_revision_number != source.revision_number {
            return Err(SharedContentError::AgentStaleBasis(format!(
                "“{}” advanced to revision {} after this proposal was prepared. Dismiss it or revise against the current revision.",
                page.title, page.current_revision_number
            )));
        }
        let revision = load_revision_row(
            tx,
            &connection.workspace_id,
            &source.page_id,
            source.revision_number,
        )?;
        if revision.content_sha256 != source.content_sha256 {
            return Err(SharedContentError::AgentStaleBasis(format!(
                "“{}” no longer matches the revision this proposal was based on.",
                page.title
            )));
        }
    }
    Ok(())
}

fn authorize_proposal_response(
    tx: &Transaction<'_>,
    connection: &AgentConnectionRow,
    view: &AgentProposalView,
) -> Result<(), SharedContentError> {
    if view.connection_id.as_deref() != Some(connection.connection_id.as_str()) {
        return Err(SharedContentError::AgentProposalNotFound);
    }
    for source in &view.sources {
        load_grant_page(tx, connection, &source.page_id)?;
    }
    if let Some(page) = &view.target_page_id {
        load_grant_page(tx, connection, page)?;
    }
    if !proposal_readable(tx, &connection.workspace_id, &connection.member_uid, view)? {
        return Err(SharedContentError::AgentProposalNotFound);
    }
    Ok(())
}

fn load_connection_row(
    tx: &Transaction<'_>,
    workspace_id: &str,
    connection_id: &str,
) -> Result<AgentConnectionRow, SharedContentError> {
    let row = tx
        .query_row(
            "SELECT connection_id, workspace_id, project_id, member_uid, member_display_name,
                    device_id, issuer_session_id, device_label, label, client_name, permission,
                    generation, created_at_ms, revoked_at_ms
             FROM shared_agent_connections
             WHERE workspace_id = ?1 AND connection_id = ?2",
            params![workspace_id, connection_id],
            |r| {
                Ok((
                    AgentConnectionRow {
                        connection_id: r.get(0)?,
                        workspace_id: r.get(1)?,
                        project_id: r.get(2)?,
                        member_uid: r.get(3)?,
                        member_display_name: r.get(4)?,
                        device_id: r.get(5)?,
                        issuer_session_id: r.get(6)?,
                        device_label: r.get(7)?,
                        label: r.get(8)?,
                        client_name: r.get(9)?,
                        permission: AgentPermission::Read,
                        generation: r.get(11)?,
                        created_at_ms: r.get(12)?,
                        revoked_at_ms: r.get(13)?,
                    },
                    r.get::<_, String>(10)?,
                ))
            },
        )
        .optional()?;
    let Some((mut row, permission)) = row else {
        return Err(SharedContentError::AgentConnectionUnavailable);
    };
    row.permission = AgentPermission::from_db(&permission)?;
    Ok(row)
}

/// Called inside the existing shared-proposal Apply transaction. No-op for human proposals.
/// A revoked grant, removed initiating member, disabled policy, changed grant generation or stale
/// source basis blocks application while leaving the proposal inspectable and dismissible.
pub(crate) fn assert_agent_proposal_applicable(
    tx: &Transaction<'_>,
    workspace_id: &str,
    proposal_id: &str,
    check_session: AgentSessionCheck<'_>,
) -> Result<(), SharedContentError> {
    let row: Option<(Option<String>, Option<i64>, Option<String>)> = tx
        .query_row(
            "SELECT agent_connection_id, agent_grant_generation, source_basis_json
             FROM shared_change_proposals
             WHERE workspace_id = ?1 AND proposal_id = ?2",
            params![workspace_id, proposal_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((Some(connection_id), grant_generation, basis_json)) = row else {
        return Ok(());
    };
    let (Some(grant_generation), Some(basis_json)) = (grant_generation, basis_json) else {
        return Err(SharedContentError::AgentStaleBasis(
            "The agent proposal's authorization basis is incomplete.".into(),
        ));
    };
    require_policy_enabled(tx, workspace_id)?;
    let connection = load_connection_row(tx, workspace_id, &connection_id)?;
    check_proposal_connection(tx, &connection, check_session)?;
    if connection.revoked_at_ms.is_some() {
        return Err(SharedContentError::AgentStaleBasis(
            "This proposal came from an agent connection that was revoked. Dismiss it or ask the member to reconnect and prepare a fresh proposal.".into(),
        ));
    }
    if connection.generation != grant_generation {
        return Err(SharedContentError::AgentStaleBasis(
            "This proposal was prepared under an older connection grant. Dismiss it or revise it against the updated page selection.".into(),
        ));
    }
    if require_active_membership(tx, workspace_id, &connection.member_uid).is_err() {
        return Err(SharedContentError::AgentStaleBasis(
            "The member who initiated this proposal is no longer in this workspace. Dismiss it or revise it against a fresh basis.".into(),
        ));
    }
    let project = load_project_summary(tx, workspace_id, &connection.project_id)?;
    if project.archived_at_ms.is_some() {
        return Err(SharedContentError::AgentStaleBasis(
            "The target project is archived. This proposal can no longer be applied.".into(),
        ));
    }
    validate_stored_basis(tx, &connection, &parse_source_basis(&basis_json))
}

fn check_proposal_connection(
    tx: &Transaction<'_>,
    connection: &AgentConnectionRow,
    check_session: AgentSessionCheck<'_>,
) -> Result<(), SharedContentError> {
    check_binding(connection, check_session).map_err(|_| {
        SharedContentError::AgentStaleBasis(
            "The initiating device session is no longer valid. Prepare a fresh proposal.".into(),
        )
    })?;
    let member = require_active_membership(tx, &connection.workspace_id, &connection.member_uid)?;
    let ctx = crate::resource_access::require_caller_access(
        tx,
        &connection.workspace_id,
        &connection.member_uid,
    )?;
    if !crate::resource_access::project_readable(
        tx,
        &connection.workspace_id,
        &connection.project_id,
        &ctx,
    )? {
        return Err(SharedContentError::AgentStaleBasis(
            "The initiating member no longer has access to this project.".into(),
        ));
    }
    if connection.permission != AgentPermission::Suggest || !member.role.can_author_shared_content()
    {
        return Err(SharedContentError::AgentStaleBasis(
            "The initiating member or connection no longer has suggestion permission.".into(),
        ));
    }
    Ok(())
}

pub(crate) fn recheck_agent_proposal_session(
    tx: &Transaction<'_>,
    workspace_id: &str,
    proposal_id: &str,
    check_session: AgentSessionCheck<'_>,
) -> Result<(), SharedContentError> {
    let connection_id: Option<String> = tx.query_row(
        "SELECT agent_connection_id FROM shared_change_proposals WHERE workspace_id = ?1 AND proposal_id = ?2",
        params![workspace_id, proposal_id],
        |row| row.get(0),
    )?;
    if let Some(id) = connection_id {
        check_proposal_connection(
            tx,
            &load_connection_row(tx, workspace_id, &id)?,
            check_session,
        )?;
    }
    Ok(())
}

fn sanitize_permission(
    requested: Option<AgentPermission>,
    role: crate::membership::WorkspaceRole,
) -> Result<AgentPermission, SharedContentError> {
    let permission = requested.unwrap_or(AgentPermission::Read);
    if permission == AgentPermission::Suggest && !role.can_author_shared_content() {
        return Err(SharedContentError::Forbidden);
    }
    Ok(permission)
}

fn load_connection_pages(
    tx: &Transaction<'_>,
    connection_id: &str,
) -> Result<Vec<AgentConnectionPageView>, SharedContentError> {
    let mut stmt = tx.prepare(
        "SELECT p.page_id, s.title, s.current_revision_number, pp.generation, s.archived_at_ms
         FROM shared_agent_connection_pages p
         JOIN shared_pages s
           ON s.workspace_id = p.workspace_id AND s.page_id = p.page_id
         LEFT JOIN shared_project_pages pp
           ON pp.workspace_id = p.workspace_id AND pp.project_id = p.project_id
          AND pp.page_id = p.page_id AND pp.membership_id = p.membership_id
          AND pp.generation = p.membership_generation AND pp.detached_at_ms IS NULL
         WHERE p.connection_id = ?1
         ORDER BY s.title ASC, p.page_id ASC",
    )?;
    let items = stmt
        .query_map(params![connection_id], |r| {
            let archived: Option<i64> = r.get(4)?;
            let active_generation: Option<i64> = r.get(3)?;
            Ok(AgentConnectionPageView {
                page_id: r.get(0)?,
                title: r.get(1)?,
                current_revision_number: r.get(2)?,
                available: archived.is_none() && active_generation.is_some(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

fn connection_view(
    tx: &Transaction<'_>,
    row: &AgentConnectionRow,
    viewer_uid: &str,
) -> Result<AgentConnectionView, SharedContentError> {
    let project = load_project_summary(tx, &row.workspace_id, &row.project_id)?;
    let ctx = crate::resource_access::require_caller_access(tx, &row.workspace_id, viewer_uid)?;
    crate::resource_access::require_readable_project(tx, &row.workspace_id, &row.project_id, &ctx)?;
    let mut pages = Vec::new();
    for page in load_connection_pages(tx, &row.connection_id)? {
        if crate::resource_access::page_readable(tx, &row.workspace_id, &page.page_id, &ctx)? {
            pages.push(page);
        }
    }
    let (updated_at_ms, last_used_at_ms, revoked_by): (i64, Option<i64>, Option<String>) = tx
        .query_row(
            "SELECT updated_at_ms, last_used_at_ms, revoked_by_display_name
             FROM shared_agent_connections WHERE connection_id = ?1",
            params![row.connection_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
    Ok(AgentConnectionView {
        connection_id: row.connection_id.clone(),
        workspace_id: row.workspace_id.clone(),
        project_id: row.project_id.clone(),
        project_name: project.name,
        member_uid: row.member_uid.clone(),
        member_display_name: row.member_display_name.clone(),
        device_id: row.device_id.clone(),
        device_label: row.device_label.clone(),
        label: row.label.clone(),
        client_name: row.client_name.clone(),
        permission: row.permission,
        generation: row.generation,
        pages,
        created_at_ms: row.created_at_ms,
        updated_at_ms,
        last_used_at_ms,
        revoked_at_ms: row.revoked_at_ms,
        revoked_by_display_name: revoked_by,
    })
}

fn validate_pages_for_project(
    tx: &Transaction<'_>,
    workspace_id: &str,
    project_id: &str,
    page_ids: &[String],
) -> Result<Vec<GrantPageRow>, SharedContentError> {
    let project = load_project_summary(tx, workspace_id, project_id)?;
    if project.archived_at_ms.is_some() {
        return Err(SharedContentError::AgentProjectUnavailable);
    }
    let mut grants = Vec::with_capacity(page_ids.len());
    for page_id in page_ids {
        let page = load_page_row(tx, workspace_id, page_id)?;
        if page.archived_at_ms.is_some() {
            return Err(SharedContentError::AgentPageUnavailable);
        }
        let grant = load_active_membership_for_page(tx, workspace_id, project_id, page_id)?
            .ok_or(SharedContentError::AgentPageUnavailable)?;
        grants.push(grant);
    }
    Ok(grants)
}

fn require_connection_manager(
    caller: &VerifiedSession,
    row: &AgentConnectionRow,
    role: crate::membership::WorkspaceRole,
    allow_workspace_admin: bool,
) -> Result<(), SharedContentError> {
    if row.member_uid == caller.uid {
        return Ok(());
    }
    if allow_workspace_admin && role.can_moderate_or_review() {
        return Ok(());
    }
    Err(SharedContentError::AgentConnectionUnavailable)
}

impl SharedAgentAccessStore for SqliteSharedContentStore {
    fn get_agent_access_policy(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
    ) -> Result<AgentAccessPolicyView, SharedContentError> {
        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let _member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let row: Option<(i64, String, i64)> = tx
            .query_row(
                "SELECT enabled, updated_by_display_name, updated_at_ms
                 FROM shared_agent_workspace_policies WHERE workspace_id = ?1",
                params![workspace_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        tx.commit()?;
        Ok(match row {
            Some((enabled, display, at)) => AgentAccessPolicyView {
                workspace_id: workspace_id.to_owned(),
                enabled: enabled == 1,
                updated_by_display_name: Some(display),
                updated_at_ms: Some(at),
            },
            None => AgentAccessPolicyView {
                workspace_id: workspace_id.to_owned(),
                enabled: false,
                updated_by_display_name: None,
                updated_at_ms: None,
            },
        })
    }

    fn set_agent_access_policy(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        enabled: bool,
        now_ms: i64,
    ) -> Result<AgentAccessPolicyView, SharedContentError> {
        let display = caller_display_name(caller);
        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_moderate_or_review() {
            return Err(SharedContentError::Forbidden);
        }
        tx.execute(
            "INSERT INTO shared_agent_workspace_policies (
                workspace_id, enabled, updated_by_uid, updated_by_display_name, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(workspace_id) DO UPDATE SET
                enabled = excluded.enabled,
                updated_by_uid = excluded.updated_by_uid,
                updated_by_display_name = excluded.updated_by_display_name,
                updated_at_ms = excluded.updated_at_ms",
            params![
                workspace_id,
                i64::from(enabled),
                caller.uid,
                display,
                now_ms
            ],
        )?;
        insert_activity_with_project(
            &tx,
            workspace_id,
            &caller.uid,
            &display,
            if enabled {
                "agent_access_enabled"
            } else {
                "agent_access_disabled"
            },
            "workspace_policy",
            None,
            None,
            None,
            None,
            None,
            None,
            if enabled {
                "Enabled shared agent access for this workspace"
            } else {
                "Disabled shared agent access for this workspace"
            },
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(AgentAccessPolicyView {
            workspace_id: workspace_id.to_owned(),
            enabled,
            updated_by_display_name: Some(display),
            updated_at_ms: Some(now_ms),
        })
    }

    fn create_agent_connection(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        input: &CreateAgentConnectionInput,
        now_ms: i64,
    ) -> Result<CreatedAgentConnection, SharedContentError> {
        let project_id = input.project_id.trim();
        if project_id.is_empty() {
            return Err(SharedContentError::InvalidInput(
                "Choose a shared project for this connection.".into(),
            ));
        }
        let client_name = sanitize_short(&input.client_name, "Agent app", MAX_AGENT_CLIENT_LEN)?;
        let label_source = if input.label.trim().is_empty() {
            format!("{client_name} on the selected project")
        } else {
            input.label.clone()
        };
        let label = sanitize_label(&label_source, "Connection label")?;
        let page_ids = sanitize_page_ids(&input.page_ids)?;
        let display = caller_display_name(caller);
        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        require_policy_enabled(&tx, workspace_id)?;
        let permission = sanitize_permission(input.permission, member.role)?;
        let ctx = crate::resource_access::require_caller_access(&tx, workspace_id, &caller.uid)?;
        crate::resource_access::require_readable_project(&tx, workspace_id, project_id, &ctx)?;
        for id in &page_ids {
            crate::resource_access::require_readable_page(&tx, workspace_id, id, &ctx)?;
        }
        let grants = validate_pages_for_project(&tx, workspace_id, project_id, &page_ids)?;

        let connection_id = format!("agc_{}", Uuid::new_v4().simple());
        let credential = random_agent_token()?;
        let token_hash = sha256_hex(&credential);
        tx.execute(
            "INSERT INTO shared_agent_connections (
                connection_id, workspace_id, project_id, member_uid, member_display_name,
                member_email, device_id, issuer_session_id, device_label, label, client_name,
                permission, generation, created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 1, ?13, ?13)",
            params![
                connection_id,
                workspace_id,
                project_id,
                caller.uid,
                display,
                caller.email,
                caller.device_id,
                caller.session_id,
                caller.device_label,
                label,
                client_name,
                permission.as_str(),
                now_ms,
            ],
        )?;
        tx.execute(
            "INSERT INTO shared_agent_connection_tokens (token_hash, connection_id, created_at_ms)
             VALUES (?1, ?2, ?3)",
            params![token_hash, connection_id, now_ms],
        )?;
        for grant in &grants {
            tx.execute(
                "INSERT INTO shared_agent_connection_pages (
                    connection_id, workspace_id, project_id, page_id, membership_id,
                    membership_generation, grant_generation, attached_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)",
                params![
                    connection_id,
                    workspace_id,
                    project_id,
                    grant.page_id,
                    grant.membership_id,
                    grant.membership_generation,
                    now_ms,
                ],
            )?;
        }
        audit(
            &tx,
            workspace_id,
            &connection_id,
            &caller.uid,
            "agent_connection_created",
            "agent_connection",
            Some(&connection_id),
            "success",
            None,
            now_ms,
        )?;
        insert_activity_with_project(
            &tx,
            workspace_id,
            &caller.uid,
            &display,
            "agent_connection_created",
            "agent_connection",
            Some(project_id),
            None,
            None,
            None,
            None,
            None,
            &format!(
                "Connected an agent to a shared project ({})",
                permission.as_str()
            ),
            now_ms,
        )?;
        let row = load_connection_row(&tx, workspace_id, &connection_id)?;
        let view = connection_view(&tx, &row, &caller.uid)?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(CreatedAgentConnection {
            connection: view,
            credential,
        })
    }

    fn list_agent_connections(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        include_workspace: bool,
    ) -> Result<Vec<AgentConnectionView>, SharedContentError> {
        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if include_workspace && !member.role.can_moderate_or_review() {
            return Err(SharedContentError::Forbidden);
        }
        let mut stmt = tx.prepare(
            "SELECT connection_id FROM shared_agent_connections
             WHERE workspace_id = ?1 AND (?2 = 1 OR member_uid = ?3)
             ORDER BY created_at_ms DESC, connection_id DESC",
        )?;
        let ids = stmt
            .query_map(
                params![workspace_id, i64::from(include_workspace), caller.uid],
                |r| r.get::<_, String>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        let mut views = Vec::with_capacity(ids.len());
        for id in ids {
            let row = load_connection_row(&tx, workspace_id, &id)?;
            let ctx =
                crate::resource_access::require_caller_access(&tx, workspace_id, &caller.uid)?;
            if crate::resource_access::project_readable(&tx, workspace_id, &row.project_id, &ctx)? {
                views.push(connection_view(&tx, &row, &caller.uid)?);
            }
        }
        tx.commit()?;
        Ok(views)
    }

    fn get_agent_connection(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        connection_id: &str,
    ) -> Result<AgentConnectionView, SharedContentError> {
        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let row = load_connection_row(&tx, workspace_id, connection_id)?;
        require_connection_manager(caller, &row, member.role, true)?;
        let view = connection_view(&tx, &row, &caller.uid)?;
        tx.commit()?;
        Ok(view)
    }

    fn update_agent_connection(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        connection_id: &str,
        input: &UpdateAgentConnectionInput,
        now_ms: i64,
    ) -> Result<AgentConnectionView, SharedContentError> {
        let page_ids = sanitize_page_ids(&input.page_ids)?;
        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        require_policy_enabled(&tx, workspace_id)?;
        let row = load_connection_row(&tx, workspace_id, connection_id)?;
        if row.revoked_at_ms.is_some() {
            return Err(SharedContentError::AgentConnectionRevoked);
        }
        require_connection_manager(caller, &row, member.role, false)?;
        if row.generation != input.expected_generation {
            return Err(SharedContentError::AgentStaleBasis(
                "This connection changed since it was loaded. Reopen it before saving a new scope."
                    .into(),
            ));
        }
        let permission = sanitize_permission(Some(input.permission), member.role)?;
        let ctx = crate::resource_access::require_caller_access(&tx, workspace_id, &caller.uid)?;
        crate::resource_access::require_readable_project(&tx, workspace_id, &row.project_id, &ctx)?;
        for id in &page_ids {
            crate::resource_access::require_readable_page(&tx, workspace_id, id, &ctx)?;
        }
        let grants = validate_pages_for_project(&tx, workspace_id, &row.project_id, &page_ids)?;
        let next_generation = row.generation + 1;
        tx.execute(
            "UPDATE shared_agent_connections
             SET permission = ?1, generation = ?2, updated_at_ms = ?3
             WHERE workspace_id = ?4 AND connection_id = ?5",
            params![
                permission.as_str(),
                next_generation,
                now_ms,
                workspace_id,
                connection_id
            ],
        )?;
        tx.execute(
            "DELETE FROM shared_agent_connection_pages WHERE connection_id = ?1",
            params![connection_id],
        )?;
        for grant in &grants {
            tx.execute(
                "INSERT INTO shared_agent_connection_pages (
                    connection_id, workspace_id, project_id, page_id, membership_id,
                    membership_generation, grant_generation, attached_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    connection_id,
                    workspace_id,
                    row.project_id,
                    grant.page_id,
                    grant.membership_id,
                    grant.membership_generation,
                    next_generation,
                    now_ms,
                ],
            )?;
        }
        // A new grant requires fresh reads, including retained pages whose membership changed.
        tx.execute(
            "DELETE FROM shared_agent_page_reads WHERE connection_id = ?1",
            params![connection_id],
        )?;
        audit(
            &tx,
            workspace_id,
            connection_id,
            &caller.uid,
            "agent_connection_updated",
            "agent_connection",
            Some(connection_id),
            "success",
            None,
            now_ms,
        )?;
        let updated = load_connection_row(&tx, workspace_id, connection_id)?;
        let view = connection_view(&tx, &updated, &caller.uid)?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(view)
    }

    fn revoke_agent_connection(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        connection_id: &str,
        now_ms: i64,
    ) -> Result<AgentConnectionView, SharedContentError> {
        let display = caller_display_name(caller);
        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let row = load_connection_row(&tx, workspace_id, connection_id)?;
        require_connection_manager(caller, &row, member.role, true)?;
        if row.revoked_at_ms.is_none() {
            tx.execute(
                "UPDATE shared_agent_connections
                 SET revoked_at_ms = ?1, revoked_by_uid = ?2, revoked_by_display_name = ?3,
                     updated_at_ms = ?1
                 WHERE workspace_id = ?4 AND connection_id = ?5 AND revoked_at_ms IS NULL",
                params![now_ms, caller.uid, display, workspace_id, connection_id],
            )?;
            tx.execute(
                "UPDATE shared_agent_connection_tokens SET revoked_at_ms = ?1
                 WHERE connection_id = ?2 AND revoked_at_ms IS NULL",
                params![now_ms, connection_id],
            )?;
            audit(
                &tx,
                workspace_id,
                connection_id,
                &caller.uid,
                "agent_connection_revoked",
                "agent_connection",
                Some(connection_id),
                "success",
                None,
                now_ms,
            )?;
            insert_activity_with_project(
                &tx,
                workspace_id,
                &caller.uid,
                &display,
                "agent_connection_revoked",
                "agent_connection",
                Some(&row.project_id),
                None,
                None,
                None,
                None,
                None,
                "Revoked shared agent access",
                now_ms,
            )?;
        }
        let revoked = load_connection_row(&tx, workspace_id, connection_id)?;
        let view = connection_view(&tx, &revoked, &caller.uid)?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(view)
    }

    fn agent_connection_status(
        &self,
        credential: &str,
        check_session: AgentSessionCheck<'_>,
        now_ms: i64,
    ) -> Result<AgentConnectionStatusView, SharedContentError> {
        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row = authorize_agent_token(&tx, credential, now_ms)?;
        check_binding(&row, check_session)?;
        let project = load_project_summary(&tx, &row.workspace_id, &row.project_id)?;
        let workspace_name: String = tx.query_row(
            "SELECT name FROM workspaces WHERE workspace_id = ?1",
            params![row.workspace_id],
            |r| r.get(0),
        )?;
        let mut pages = Vec::new();
        for page in load_connection_pages(&tx, &row.connection_id)? {
            if page.available && load_grant_page(&tx, &row, &page.page_id).is_ok() {
                pages.push(page);
            }
        }
        audit(
            &tx,
            &row.workspace_id,
            &row.connection_id,
            &row.member_uid,
            "agent_status",
            "agent_connection",
            Some(&row.connection_id),
            "success",
            None,
            now_ms,
        )?;
        check_binding(&row, check_session)?;
        tx.commit()?;
        Ok(AgentConnectionStatusView {
            connection_id: row.connection_id.clone(),
            workspace_id: row.workspace_id.clone(),
            workspace_name,
            project_id: row.project_id.clone(),
            project_name: project.name,
            member_display_name: row.member_display_name.clone(),
            device_label: row.device_label.clone(),
            permission: row.permission,
            generation: row.generation,
            created_at_ms: row.created_at_ms,
            pages,
            limits: AgentConnectionLimits {
                max_pages_per_list: MAX_AGENT_LIST_LIMIT,
                max_content_bytes: MAX_PAGE_CONTENT_BYTES,
                can_suggest: row.permission == AgentPermission::Suggest,
            },
        })
    }

    fn list_agent_pages(
        &self,
        credential: &str,
        check_session: AgentSessionCheck<'_>,
        search: Option<&str>,
        cursor: Option<&str>,
        limit: Option<usize>,
        now_ms: i64,
    ) -> Result<AgentSharedPageList, SharedContentError> {
        let limit = limit
            .unwrap_or(DEFAULT_AGENT_LIST_LIMIT)
            .clamp(1, MAX_AGENT_LIST_LIMIT);
        let (cursor_ts, cursor_id) = parse_page_cursor(cursor)?;
        let search = search.map(str::trim).filter(|q| !q.is_empty());
        if search.is_some_and(|q| q.chars().count() > 120) {
            return Err(SharedContentError::InvalidInput(
                "Search text must be at most 120 characters.".into(),
            ));
        }
        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row = authorize_agent_token(&tx, credential, now_ms)?;
        check_binding(&row, check_session)?;
        let ctx =
            crate::resource_access::require_caller_access(&tx, &row.workspace_id, &row.member_uid)?;
        let visible = crate::resource_access::page_visible_predicate("s", 6, 7);
        let mut stmt = tx.prepare(&format!(
            "SELECT s.page_id, s.title, s.current_revision_number, s.updated_at_ms, s.content_md
             FROM shared_agent_connection_pages p
             JOIN shared_project_pages pp
               ON pp.workspace_id = p.workspace_id AND pp.project_id = p.project_id
              AND pp.page_id = p.page_id AND pp.membership_id = p.membership_id
              AND pp.generation = p.membership_generation AND pp.detached_at_ms IS NULL
             JOIN shared_pages s
               ON s.workspace_id = p.workspace_id AND s.page_id = p.page_id
             WHERE p.connection_id = ?1 AND s.archived_at_ms IS NULL
               AND (?2 IS NULL OR instr(lower(s.title), lower(?2)) > 0
                    OR instr(lower(s.content_md), lower(?2)) > 0)
               AND (?3 IS NULL OR s.updated_at_ms < ?3
                    OR (s.updated_at_ms = ?3 AND s.page_id < ?4))
               AND {visible}
             ORDER BY s.updated_at_ms DESC, s.page_id DESC
             LIMIT ?5"
        ))?;
        let rows = stmt
            .query_map(
                params![
                    row.connection_id,
                    search,
                    cursor_ts,
                    cursor_id,
                    (limit + 1) as i64,
                    ctx.membership_id,
                    crate::resource_access::privileged_flag(&ctx)
                ],
                |r| {
                    let content: String = r.get(4)?;
                    Ok(AgentSharedPageItem {
                        page_id: r.get(0)?,
                        title: r.get(1)?,
                        revision_number: r.get(2)?,
                        updated_at_ms: r.get(3)?,
                        snippet: page_snippet(&content, search),
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        let (items, next_cursor) = if rows.len() > limit {
            let next = rows
                .get(limit - 1)
                .map(|item| format!("{}:{}", item.updated_at_ms, item.page_id));
            (rows[..limit].to_vec(), next)
        } else {
            (rows, None)
        };
        audit(
            &tx,
            &row.workspace_id,
            &row.connection_id,
            &row.member_uid,
            "agent_pages_listed",
            "shared_page",
            None,
            "success",
            None,
            now_ms,
        )?;
        check_binding(&row, check_session)?;
        tx.commit()?;
        Ok(AgentSharedPageList { items, next_cursor })
    }

    fn read_agent_page(
        &self,
        credential: &str,
        check_session: AgentSessionCheck<'_>,
        page_id: &str,
        now_ms: i64,
    ) -> Result<AgentSharedPageRead, SharedContentError> {
        let page_id = page_id.trim();
        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row = authorize_agent_token(&tx, credential, now_ms)?;
        check_binding(&row, check_session)?;
        let (_grant, page) = load_grant_page(&tx, &row, page_id)?;
        let revision: SharedPageRevisionView = load_revision_row(
            &tx,
            &row.workspace_id,
            page_id,
            page.current_revision_number,
        )?;
        let (access_version, access_digest) =
            crate::resource_access::page_access_basis(&tx, &row.workspace_id, page_id)?;
        tx.execute(
            "INSERT INTO shared_agent_page_reads (
                connection_id, page_id, revision_number, content_sha256, read_at_ms, access_version, access_digest
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(connection_id, page_id) DO UPDATE SET
                revision_number = excluded.revision_number,
                content_sha256 = excluded.content_sha256,
                read_at_ms = excluded.read_at_ms,
                access_version = excluded.access_version,
                access_digest = excluded.access_digest",
            params![
                row.connection_id,
                page_id,
                revision.revision_number,
                revision.content_sha256,
                now_ms,
                access_version,
                access_digest
            ],
        )?;
        audit(
            &tx,
            &row.workspace_id,
            &row.connection_id,
            &row.member_uid,
            "agent_page_read",
            "shared_page",
            Some(page_id),
            "success",
            None,
            now_ms,
        )?;
        check_binding(&row, check_session)?;
        tx.commit()?;
        Ok(AgentSharedPageRead {
            page_id: page.page_id,
            workspace_id: page.workspace_id,
            project_id: row.project_id,
            title: page.title,
            content_md: revision.content_md,
            revision_id: revision.revision_id,
            revision_number: revision.revision_number,
            content_sha256: revision.content_sha256,
            page_updated_at_ms: page.updated_at_ms,
            complete: true,
            provenance: page.provenance,
            contributor_names: revision.contributor_names,
        })
    }

    fn create_agent_edit_proposal(
        &self,
        credential: &str,
        check_session: AgentSessionCheck<'_>,
        input: &AgentEditProposalInput,
        now_ms: i64,
    ) -> Result<AgentProposalView, SharedContentError> {
        let request_id = sanitize_request_id(&input.request_id)?;
        let page_id = input.page_id.trim();
        let proposed_title = sanitize_page_title(&input.proposed_title)?;
        let proposed_content_md = sanitize_page_content(&input.proposed_content_md)?;
        let summary = sanitize_proposal_summary(&input.summary)?;
        let agent_label = sanitize_label(&input.agent_label, "Agent label")?;
        let model_label = sanitize_short(&input.model_label, "Model label", MAX_AGENT_MODEL_LEN)?;
        let provider_label = sanitize_short(
            &input.provider_label,
            "Provider label",
            MAX_AGENT_PROVIDER_LEN,
        )?;
        let payload_hash = payload_digest(&serde_json::json!({
            "page_id": page_id,
            "base_revision_number": input.base_revision_number,
            "base_content_sha256": input.base_content_sha256.trim().to_ascii_lowercase(),
            "proposed_title": proposed_title,
            "proposed_content_md": proposed_content_md,
            "summary": summary,
            "agent_label": agent_label,
            "model_label": model_label,
            "provider_label": provider_label,
        }));

        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row = authorize_agent_token(&tx, credential, now_ms)?;
        check_binding(&row, check_session)?;
        if row.permission != AgentPermission::Suggest {
            return Err(SharedContentError::AgentSuggestionNotAllowed);
        }
        if let Some(existing) = lookup_agent_request(
            &tx,
            &row.connection_id,
            "edit_proposal",
            &request_id,
            &payload_hash,
        )? {
            let view = edit_proposal_view(&tx, &row.workspace_id, &existing)?;
            authorize_proposal_response(&tx, &row, &view)?;
            check_binding(&row, check_session)?;
            tx.commit()?;
            return Ok(view);
        }
        let (grant, page) = load_grant_page(&tx, &row, page_id)?;
        let revision = load_revision_row(
            &tx,
            &row.workspace_id,
            page_id,
            page.current_revision_number,
        )?;
        if input.base_revision_number != page.current_revision_number {
            return Err(SharedContentError::AgentStaleBasis(format!(
                "“{}” advanced to revision {} after it was read at revision {}. Read it again before proposing.",
                page.title, page.current_revision_number, input.base_revision_number
            )));
        }
        if !revision
            .content_sha256
            .eq_ignore_ascii_case(input.base_content_sha256.trim())
        {
            return Err(SharedContentError::AgentStaleBasis(
                "The supplied base revision identity does not match this page. Read the complete page again with the shared MCP tools.".into(),
            ));
        }
        let basis = snapshot_source_basis(&tx, &row)?;
        // An edit must not copy a narrower source into a broader existing page.
        for member in crate::resource_access::list_active_members(&tx, &row.workspace_id)? {
            let ctx =
                crate::resource_access::require_caller_access(&tx, &row.workspace_id, &member.uid)?;
            if crate::resource_access::page_readable(&tx, &row.workspace_id, page_id, &ctx)? {
                for source in &basis {
                    if !crate::resource_access::page_readable(
                        &tx,
                        &row.workspace_id,
                        &source.page_id,
                        &ctx,
                    )? {
                        return Err(SharedContentError::AgentStaleBasis("The target page has a wider audience than a source. Review its sharing before proposing this edit.".into()));
                    }
                }
            }
        }
        if !basis.iter().any(|source| source.page_id == page_id) {
            return Err(SharedContentError::AgentStaleBasis(
                "Read the complete target page before preparing an edit proposal.".into(),
            ));
        }
        let basis_json = serde_json::to_string(&basis).map_err(|_| {
            SharedContentError::InvalidInput("Could not record the proposal basis.".into())
        })?;

        let proposal_id = format!("prp_{}", Uuid::new_v4().simple());
        let decision_id = format!("dec_{}", Uuid::new_v4().simple());
        let (access_version, access_digest) =
            crate::resource_access::page_access_basis(&tx, &row.workspace_id, page_id)?;
        tx.execute(
            "INSERT INTO shared_change_proposals (
                proposal_id, workspace_id, page_id, base_revision_number, base_title,
                base_content_md, proposed_title, proposed_content_md, summary, author_uid,
                author_display_name, status, submission_count, created_at_ms, updated_at_ms,
                agent_connection_id, agent_label, agent_model_label, agent_provider_label,
                agent_model_trust, source_basis_json, agent_grant_generation,
                page_access_version, page_access_digest
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'pending', 1, ?12, ?12,
                       ?13, ?14, ?15, ?16, 'declared_unverified', ?17, ?18, ?19, ?20)",
            params![
                proposal_id,
                row.workspace_id,
                page_id,
                page.current_revision_number,
                revision.title,
                revision.content_md,
                proposed_title,
                proposed_content_md,
                summary,
                row.member_uid,
                row.member_display_name,
                now_ms,
                row.connection_id,
                agent_label,
                model_label,
                provider_label,
                basis_json,
                row.generation,
                access_version,
                access_digest,
            ],
        )?;
        tx.execute(
            "INSERT INTO shared_proposal_decisions (
                decision_id, workspace_id, proposal_id, submission_number, action,
                actor_uid, actor_display_name, note, base_revision_number,
                submitted_title, submitted_content_md, created_at_ms
             ) VALUES (?1, ?2, ?3, 1, 'submitted', ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                decision_id,
                row.workspace_id,
                proposal_id,
                row.member_uid,
                row.member_display_name,
                summary,
                page.current_revision_number,
                proposed_title,
                proposed_content_md,
                now_ms,
            ],
        )?;
        save_agent_request(
            &tx,
            &row.connection_id,
            "edit_proposal",
            &request_id,
            &payload_hash,
            &proposal_id,
            now_ms,
        )?;
        audit(
            &tx,
            &row.workspace_id,
            &row.connection_id,
            &row.member_uid,
            "agent_proposal_submitted",
            "shared_page",
            Some(page_id),
            "success",
            Some(&proposal_id),
            now_ms,
        )?;
        insert_activity_with_project(
            &tx,
            &row.workspace_id,
            &row.member_uid,
            &row.member_display_name,
            "proposal_submitted",
            "proposal",
            Some(&row.project_id),
            None,
            Some(page_id),
            Some(&page.title),
            Some(&proposal_id),
            None,
            &format!(
                "Agent proposal from {} awaits review on “{}” (basis {} rev {})",
                row.label, page.title, grant.membership_id, page.current_revision_number
            ),
            now_ms,
        )?;
        let view = edit_proposal_view(&tx, &row.workspace_id, &proposal_id)?;
        check_binding(&row, check_session)?;
        tx.commit()?;
        Ok(view)
    }

    fn create_agent_new_page_proposal(
        &self,
        credential: &str,
        check_session: AgentSessionCheck<'_>,
        input: &AgentNewPageProposalInput,
        now_ms: i64,
    ) -> Result<AgentProposalView, SharedContentError> {
        let request_id = sanitize_request_id(&input.request_id)?;
        let proposed_title = sanitize_page_title(&input.proposed_title)?;
        let proposed_content_md = sanitize_page_content(&input.proposed_content_md)?;
        let summary = sanitize_proposal_summary(&input.summary)?;
        let agent_label = sanitize_label(&input.agent_label, "Agent label")?;
        let model_label = sanitize_short(&input.model_label, "Model label", MAX_AGENT_MODEL_LEN)?;
        let provider_label = sanitize_short(
            &input.provider_label,
            "Provider label",
            MAX_AGENT_PROVIDER_LEN,
        )?;
        let payload_hash = payload_digest(&serde_json::json!({
            "project_id": input.project_id,
            "proposed_title": proposed_title,
            "proposed_content_md": proposed_content_md,
            "summary": summary,
            "agent_label": agent_label,
            "model_label": model_label,
            "provider_label": provider_label,
        }));

        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row = authorize_agent_token(&tx, credential, now_ms)?;
        check_binding(&row, check_session)?;
        if row.permission != AgentPermission::Suggest {
            return Err(SharedContentError::AgentSuggestionNotAllowed);
        }
        if input
            .project_id
            .as_deref()
            .is_some_and(|id| id.trim() != row.project_id)
        {
            return Err(SharedContentError::AgentProjectUnavailable);
        }
        if let Some(existing) = lookup_agent_request(
            &tx,
            &row.connection_id,
            "new_page_proposal",
            &request_id,
            &payload_hash,
        )? {
            let view = new_page_proposal_view(&tx, &row.workspace_id, &existing)?;
            authorize_proposal_response(&tx, &row, &view)?;
            check_binding(&row, check_session)?;
            tx.commit()?;
            return Ok(view);
        }
        let project = load_project_summary(&tx, &row.workspace_id, &row.project_id)?;
        if project.archived_at_ms.is_some() {
            return Err(SharedContentError::AgentProjectUnavailable);
        }
        let basis = snapshot_source_basis(&tx, &row)?;
        let basis_json = serde_json::to_string(&basis).map_err(|_| {
            SharedContentError::InvalidInput("Could not record the proposal basis.".into())
        })?;
        let proposal_id = format!("prp_{}", Uuid::new_v4().simple());
        tx.execute(
            "INSERT INTO shared_agent_page_proposals (
                proposal_id, workspace_id, project_id, proposed_title, proposed_content_md,
                summary, author_uid, author_display_name, connection_id, agent_label,
                model_label, provider_label, model_trust, source_basis_json, grant_generation,
                status, submission_count, created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                       'declared_unverified', ?13, ?14, 'pending', 1, ?15, ?15)",
            params![
                proposal_id,
                row.workspace_id,
                row.project_id,
                proposed_title,
                proposed_content_md,
                summary,
                row.member_uid,
                row.member_display_name,
                row.connection_id,
                agent_label,
                model_label,
                provider_label,
                basis_json,
                row.generation,
                now_ms,
            ],
        )?;
        save_agent_request(
            &tx,
            &row.connection_id,
            "new_page_proposal",
            &request_id,
            &payload_hash,
            &proposal_id,
            now_ms,
        )?;
        audit(
            &tx,
            &row.workspace_id,
            &row.connection_id,
            &row.member_uid,
            "agent_new_page_proposed",
            "shared_page",
            None,
            "success",
            Some(&proposal_id),
            now_ms,
        )?;
        insert_activity_with_project(
            &tx,
            &row.workspace_id,
            &row.member_uid,
            &row.member_display_name,
            "proposal_submitted",
            "proposal",
            Some(&row.project_id),
            Some(&project.name),
            None,
            Some(&proposed_title),
            Some(&proposal_id),
            None,
            &format!(
                "Agent proposal from {} proposes a new page in “{}”",
                row.label, project.name
            ),
            now_ms,
        )?;
        let view = new_page_proposal_view(&tx, &row.workspace_id, &proposal_id)?;
        check_binding(&row, check_session)?;
        tx.commit()?;
        Ok(view)
    }

    fn agent_request_status(
        &self,
        credential: &str,
        check_session: AgentSessionCheck<'_>,
        request_id: &str,
        now_ms: i64,
    ) -> Result<AgentRequestReceipt, SharedContentError> {
        let request_id = sanitize_request_id(request_id)?;
        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let row = authorize_agent_token(&tx, credential, now_ms)?;
        check_binding(&row, check_session)?;
        let receipt = tx
            .query_row(
                "SELECT operation, proposal_id, created_at_ms FROM shared_agent_proposal_requests
                 WHERE connection_id = ?1 AND request_id = ?2
                 ORDER BY created_at_ms DESC LIMIT 1",
                params![row.connection_id, request_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(SharedContentError::AgentProposalNotFound)?;
        let proposal = load_proposal_view_any(&tx, &row.workspace_id, &receipt.1)?;
        authorize_proposal_response(&tx, &row, &proposal)?;
        check_binding(&row, check_session)?;
        tx.commit()?;
        Ok(AgentRequestReceipt {
            request_id,
            operation: receipt.0,
            proposal_id: receipt.1,
            status: proposal.status,
            is_stale: proposal.is_stale,
            stale_reason: proposal.stale_reason,
            created_at_ms: receipt.2,
        })
    }

    fn agent_proposal_status(
        &self,
        credential: &str,
        check_session: AgentSessionCheck<'_>,
        proposal_id: &str,
        now_ms: i64,
    ) -> Result<AgentProposalView, SharedContentError> {
        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let row = authorize_agent_token(&tx, credential, now_ms)?;
        check_binding(&row, check_session)?;
        let view = load_proposal_view_any(&tx, &row.workspace_id, proposal_id)?;
        authorize_proposal_response(&tx, &row, &view)?;
        check_binding(&row, check_session)?;
        tx.commit()?;
        Ok(view)
    }

    fn list_agent_proposals(
        &self,
        caller: &VerifiedSession,
        check_agent_session: AgentSessionCheck<'_>,
        workspace_id: &str,
        include_closed: bool,
    ) -> Result<Vec<AgentProposalView>, SharedContentError> {
        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let mut ids: Vec<String> = Vec::new();
        {
            let mut stmt = tx.prepare(
                "SELECT proposal_id FROM shared_change_proposals
                 WHERE workspace_id = ?1 AND agent_connection_id IS NOT NULL
                   AND (?2 = 1 OR status = 'pending')
                 ORDER BY updated_at_ms DESC, proposal_id DESC",
            )?;
            ids.extend(
                stmt.query_map(params![workspace_id, i64::from(include_closed)], |r| {
                    r.get::<_, String>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?,
            );
        }
        {
            let mut stmt = tx.prepare(
                "SELECT proposal_id FROM shared_agent_page_proposals
                 WHERE workspace_id = ?1 AND (?2 = 1 OR status = 'pending')
                 ORDER BY updated_at_ms DESC, proposal_id DESC",
            )?;
            ids.extend(
                stmt.query_map(params![workspace_id, i64::from(include_closed)], |r| {
                    r.get::<_, String>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?,
            );
        }
        let mut views = Vec::with_capacity(ids.len());
        for id in ids {
            let mut view = load_proposal_view_any(&tx, workspace_id, &id)?;
            if !proposal_readable(&tx, workspace_id, &caller.uid, &view)? {
                continue;
            }
            check_review_connection(&tx, &mut view, check_agent_session)?;
            set_review_eligibility(&mut view, caller, member.role);
            views.push(view);
        }
        views.sort_by(|a, b| {
            b.updated_at_ms
                .cmp(&a.updated_at_ms)
                .then_with(|| b.proposal_id.cmp(&a.proposal_id))
        });
        tx.commit()?;
        Ok(views)
    }

    fn get_agent_proposal(
        &self,
        caller: &VerifiedSession,
        check_agent_session: AgentSessionCheck<'_>,
        workspace_id: &str,
        proposal_id: &str,
    ) -> Result<AgentProposalView, SharedContentError> {
        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let mut view = load_proposal_view_any(&tx, workspace_id, proposal_id)?;
        if !proposal_readable(&tx, workspace_id, &caller.uid, &view)? {
            return Err(SharedContentError::AgentProposalNotFound);
        }
        check_review_connection(&tx, &mut view, check_agent_session)?;
        set_review_eligibility(&mut view, caller, member.role);
        tx.commit()?;
        Ok(view)
    }

    fn review_agent_page_proposal(
        &self,
        caller: &VerifiedSession,
        checks: ReviewSessionChecks<'_>,
        workspace_id: &str,
        proposal_id: &str,
        input: &ReviewAgentProposalInput,
        now_ms: i64,
    ) -> Result<AgentProposalView, SharedContentError> {
        let recheck_session = checks.reviewer;
        let check_agent_session = checks.agent;
        if input.expected_submission_number <= 0 {
            return Err(SharedContentError::InvalidInput(
                "Expected submission number must be at least 1.".into(),
            ));
        }
        let action = match input.action.trim() {
            "apply" => "apply",
            "request_changes" => "request_changes",
            "reject" => "reject",
            other => {
                return Err(SharedContentError::InvalidInput(format!(
                    "Unknown review action “{other}”."
                )))
            }
        };
        let note = sanitize_optional_note(input.note.as_deref())?;
        let idempotency_key = sanitize_idempotency_key(&input.idempotency_key)?;
        let display = caller_display_name(caller);
        let op = format!("review_agent_proposal:{proposal_id}");
        let payload_hash = payload_digest(&serde_json::json!({
            "proposal_id": proposal_id,
            "action": action,
            "expected_submission_number": input.expected_submission_number,
            "note": note,
        }));

        let mut conn = self.lock_shared_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_moderate_or_review() {
            return Err(SharedContentError::Forbidden);
        }
        let current = load_proposal_view_any(&tx, workspace_id, proposal_id)?;
        if current.kind != AgentProposalKind::NewPage {
            return Err(SharedContentError::InvalidInput(
                "Edit proposals are reviewed through the shared proposal endpoint.".into(),
            ));
        }
        if current.author_uid == caller.uid {
            return Err(SharedContentError::SelfReviewForbidden);
        }
        if let Some(cached) = lookup_idempotent_response::<AgentProposalView>(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
        )? {
            let _ = recheck_session()?;
            tx.commit()?;
            return Ok(cached);
        }
        if current.submission_count != input.expected_submission_number {
            return Err(SharedContentError::StaleSubmission(format!(
                "Submission #{} was replaced by submission #{}. Refresh the review before deciding.",
                input.expected_submission_number, current.submission_count
            )));
        }
        if current.status != "pending" {
            return Err(SharedContentError::ProposalNotPending);
        }
        let connection_id = current
            .connection_id
            .clone()
            .ok_or(SharedContentError::AgentProposalNotFound)?;
        let project_id = current
            .project_id
            .clone()
            .ok_or(SharedContentError::AgentProposalNotFound)?;
        let connection = load_connection_row(&tx, workspace_id, &connection_id)?;

        let mut applied_page_id: Option<String> = None;
        if action == "apply" {
            check_proposal_connection(&tx, &connection, check_agent_session)?;
            require_policy_enabled(&tx, workspace_id)?;
            if connection.revoked_at_ms.is_some() {
                return Err(SharedContentError::AgentStaleBasis(
                    "This proposal's agent connection was revoked. Dismiss it or prepare a fresh proposal.".into(),
                ));
            }
            if current.revocation_state != "active" {
                return Err(SharedContentError::AgentStaleBasis(
                    "Workspace access changed. Dismiss this proposal or prepare a fresh one."
                        .into(),
                ));
            }
            let grant_generation: i64 = tx.query_row(
                "SELECT grant_generation FROM shared_agent_page_proposals
                 WHERE workspace_id = ?1 AND proposal_id = ?2",
                params![workspace_id, proposal_id],
                |r| r.get(0),
            )?;
            if connection.generation != grant_generation {
                return Err(SharedContentError::AgentStaleBasis(
                    "This proposal was prepared under an older connection grant. Dismiss it or revise it.".into(),
                ));
            }
            let project = load_project_summary(&tx, workspace_id, &project_id)?;
            if project.archived_at_ms.is_some() {
                return Err(SharedContentError::AgentStaleBasis(
                    "The target project is archived. This proposal can no longer be applied."
                        .into(),
                ));
            }
            let basis_json: String = tx.query_row(
                "SELECT source_basis_json FROM shared_agent_page_proposals
                 WHERE workspace_id = ?1 AND proposal_id = ?2",
                params![workspace_id, proposal_id],
                |r| r.get(0),
            )?;
            match validate_stored_basis(&tx, &connection, &parse_source_basis(&basis_json)) {
                Ok(()) => {}
                Err(SharedContentError::AgentStaleBasis(reason)) => {
                    return Err(SharedContentError::AgentStaleBasis(reason))
                }
                Err(err) => return Err(err),
            }
            let page_id = format!("shp_{}", Uuid::new_v4().simple());
            let revision_id = format!("rev_{}", Uuid::new_v4().simple());
            let membership_id = format!("prjmem_{}", Uuid::new_v4().simple());
            let content_sha = sha256_hex(&current.proposed_content_md);
            let source_note = format!(
                "Initiated by {} via {} ({})",
                current.author_display_name, current.agent_label, current.provider_label
            );
            tx.execute(
                "INSERT INTO shared_pages (
                    page_id, workspace_id, title, content_md, current_revision_number,
                    entry_mode, origin_kind, original_author_label, ai_model_label, source_note,
                    omitted_summary_json, publisher_uid, publisher_display_name,
                    last_editor_uid, last_editor_display_name, last_reviewer_uid,
                    last_reviewer_display_name, created_at_ms, updated_at_ms,
                    access_mode
                 ) VALUES (?1, ?2, ?3, ?4, 1, 'new', 'ai_assisted', ?5, ?6, ?7, '[]',
                           ?8, ?9, ?8, ?9, ?10, ?11, ?12, ?12, 'selected')",
                params![
                    page_id,
                    workspace_id,
                    current.proposed_title,
                    current.proposed_content_md,
                    current.agent_label,
                    current.model_label,
                    source_note,
                    current.author_uid,
                    current.author_display_name,
                    caller.uid,
                    display,
                    now_ms,
                ],
            )?;
            let sources = parse_source_basis(&basis_json);
            for member in crate::resource_access::list_active_members(&tx, workspace_id)? {
                let ctx =
                    crate::resource_access::require_caller_access(&tx, workspace_id, &member.uid)?;
                let mut allowed =
                    crate::resource_access::project_readable(&tx, workspace_id, &project_id, &ctx)?;
                for source in &sources {
                    allowed &= crate::resource_access::page_readable(
                        &tx,
                        workspace_id,
                        &source.page_id,
                        &ctx,
                    )?;
                }
                if allowed {
                    tx.execute("INSERT INTO shared_resource_audience (audience_id,workspace_id,resource_kind,resource_id,membership_id,grant_generation,granted_by_uid,granted_at_ms) VALUES (?1,?2,'page',?3,?4,?5,?6,?7)",
                        params![format!("aud_{}",Uuid::new_v4().simple()),workspace_id,page_id,member.membership_id,member.uid,caller.uid,now_ms])?;
                }
            }
            tx.execute(
                "INSERT INTO shared_page_revisions (
                    revision_id, workspace_id, page_id, revision_number, title, content_md,
                    content_sha256, change_summary, author_uid, author_display_name,
                    reviewer_uid, reviewer_display_name, created_at_ms
                 ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    revision_id,
                    workspace_id,
                    page_id,
                    current.proposed_title,
                    current.proposed_content_md,
                    content_sha,
                    current.summary,
                    current.author_uid,
                    current.author_display_name,
                    caller.uid,
                    display,
                    now_ms,
                ],
            )?;
            let max_gen: i64 = tx.query_row(
                "SELECT COALESCE(MAX(generation), 0) FROM shared_project_pages
                 WHERE workspace_id = ?1 AND project_id = ?2 AND page_id = ?3",
                params![workspace_id, project_id, page_id],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT INTO shared_project_pages (
                    membership_id, workspace_id, project_id, page_id, generation,
                    attached_by_uid, attached_by_display_name, attached_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    membership_id,
                    workspace_id,
                    project_id,
                    page_id,
                    max_gen + 1,
                    caller.uid,
                    display,
                    now_ms,
                ],
            )?;
            tx.execute(
                "UPDATE shared_projects SET updated_at_ms = ?1
                 WHERE workspace_id = ?2 AND project_id = ?3",
                params![now_ms, workspace_id, project_id],
            )?;
            applied_page_id = Some(page_id);
        }

        let (next_status, activity_action, detail) = match action {
            "apply" => (
                "applied",
                "proposal_applied",
                format!(
                    "Applied agent proposal “{}” to “{}”",
                    current.proposed_title,
                    current.project_name.clone().unwrap_or_default()
                ),
            ),
            "request_changes" => (
                "changes_requested",
                "proposal_changes_requested",
                format!(
                    "Requested changes on agent proposal “{}”",
                    current.proposed_title
                ),
            ),
            _ => (
                "rejected",
                "proposal_rejected",
                format!("Dismissed agent proposal “{}”", current.proposed_title),
            ),
        };
        let updated = tx.execute(
            "UPDATE shared_agent_page_proposals
             SET status = ?1, updated_at_ms = ?2, reviewed_at_ms = ?2, reviewer_uid = ?3,
                 reviewer_display_name = ?4, reviewer_note = ?5,
                 applied_page_id = COALESCE(?6, applied_page_id),
                 applied_revision_number = CASE WHEN ?6 IS NULL THEN applied_revision_number ELSE 1 END
             WHERE workspace_id = ?7 AND proposal_id = ?8
               AND status = 'pending' AND submission_count = ?9",
            params![
                next_status,
                now_ms,
                caller.uid,
                display,
                note,
                applied_page_id,
                workspace_id,
                proposal_id,
                input.expected_submission_number,
            ],
        )?;
        if updated != 1 {
            return Err(SharedContentError::ProposalNotPending);
        }
        audit(
            &tx,
            workspace_id,
            &connection_id,
            &caller.uid,
            activity_action,
            "agent_proposal",
            Some(proposal_id),
            "success",
            Some(proposal_id),
            now_ms,
        )?;
        insert_activity_with_project(
            &tx,
            workspace_id,
            &caller.uid,
            &display,
            activity_action,
            "proposal",
            Some(&project_id),
            None,
            applied_page_id.as_deref(),
            Some(&current.proposed_title),
            Some(proposal_id),
            None,
            &detail,
            now_ms,
        )?;
        let mut view = load_proposal_view_any(&tx, workspace_id, proposal_id)?;
        set_review_eligibility(&mut view, caller, member.role);
        save_idempotent_response(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
            &view,
            now_ms,
        )?;
        let _ = recheck_session()?;
        if action == "apply" {
            check_proposal_connection(&tx, &connection, check_agent_session)?;
        }
        tx.commit()?;
        Ok(view)
    }
}

fn load_proposal_view_any(
    tx: &Transaction<'_>,
    workspace_id: &str,
    proposal_id: &str,
) -> Result<AgentProposalView, SharedContentError> {
    match new_page_proposal_view(tx, workspace_id, proposal_id) {
        Ok(view) => Ok(view),
        Err(SharedContentError::AgentProposalNotFound) => {
            edit_proposal_view(tx, workspace_id, proposal_id)
        }
        Err(err) => Err(err),
    }
}

fn proposal_readable(
    tx: &Transaction<'_>,
    workspace_id: &str,
    uid: &str,
    view: &AgentProposalView,
) -> Result<bool, SharedContentError> {
    let ctx = crate::resource_access::require_caller_access(tx, workspace_id, uid)?;
    if let Some(id) = &view.project_id {
        if !crate::resource_access::project_readable(tx, workspace_id, id, &ctx)? {
            return Ok(false);
        }
    }
    for id in view
        .target_page_id
        .iter()
        .chain(view.applied_page_id.iter())
        .chain(view.sources.iter().map(|source| &source.page_id))
    {
        if !crate::resource_access::page_readable(tx, workspace_id, id, &ctx)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn set_review_eligibility(
    view: &mut AgentProposalView,
    caller: &VerifiedSession,
    role: crate::membership::WorkspaceRole,
) {
    view.review_eligible = role.can_moderate_or_review()
        && caller.uid != view.author_uid
        && view.status == "pending"
        && !view.is_stale
        && view.revocation_state == "active";
}

fn check_review_connection(
    tx: &Transaction<'_>,
    view: &mut AgentProposalView,
    check_session: AgentSessionCheck<'_>,
) -> Result<(), SharedContentError> {
    if let Some(id) = &view.connection_id {
        let connection = load_connection_row(tx, &view.workspace_id, id)?;
        if let Err(err) = check_proposal_connection(tx, &connection, check_session) {
            view.revocation_state = "authorization_changed".into();
            view.is_stale = true;
            view.stale_reason = Some(err.to_string());
        }
    }
    Ok(())
}

fn revocation_state(
    tx: &Transaction<'_>,
    workspace_id: &str,
    connection_id: &str,
) -> Result<String, SharedContentError> {
    let connection = load_connection_row(tx, workspace_id, connection_id)?;
    if connection.revoked_at_ms.is_some() {
        return Ok("revoked".into());
    }
    if !load_policy_enabled(tx, workspace_id)? {
        return Ok("policy_disabled".into());
    }
    if require_active_membership(tx, workspace_id, &connection.member_uid).is_err() {
        return Ok("member_removed".into());
    }
    let project = load_project_summary(tx, workspace_id, &connection.project_id)?;
    if project.archived_at_ms.is_some() {
        return Ok("project_archived".into());
    }
    Ok("active".into())
}

fn evaluate_stored_basis(
    tx: &Transaction<'_>,
    workspace_id: &str,
    connection_id: &str,
    basis_json: &str,
) -> Result<Option<String>, SharedContentError> {
    let connection = load_connection_row(tx, workspace_id, connection_id)?;
    match validate_stored_basis(tx, &connection, &parse_source_basis(basis_json)) {
        Ok(()) => Ok(None),
        Err(SharedContentError::AgentStaleBasis(reason)) => Ok(Some(reason)),
        Err(SharedContentError::Session(err)) => Err(SharedContentError::Session(err)),
        Err(SharedContentError::Storage(err)) => Err(SharedContentError::Storage(err)),
        Err(_) => Ok(Some(
            "The recorded proposal basis is no longer available.".into(),
        )),
    }
}

#[allow(clippy::type_complexity)]
struct RawEditProposal {
    proposal_id: String,
    connection_id: String,
    base_revision_number: i64,
    proposed_title: String,
    proposed_content_md: String,
    summary: String,
    status: String,
    author_uid: String,
    author_display_name: String,
    agent_label: Option<String>,
    model_label: Option<String>,
    provider_label: Option<String>,
    model_trust: Option<String>,
    basis_json: Option<String>,
    grant_generation: Option<i64>,
    page_id: String,
    applied_revision_number: Option<i64>,
    created_at_ms: i64,
    updated_at_ms: i64,
    reviewed_at_ms: Option<i64>,
    reviewer_display_name: Option<String>,
    submission_count: i64,
}

fn edit_proposal_view(
    tx: &Transaction<'_>,
    workspace_id: &str,
    proposal_id: &str,
) -> Result<AgentProposalView, SharedContentError> {
    let raw = tx
        .query_row(
            "SELECT proposal_id, agent_connection_id, base_revision_number, proposed_title,
                    proposed_content_md, summary, status, author_uid, author_display_name,
                    agent_label, agent_model_label, agent_provider_label, agent_model_trust,
                    source_basis_json, agent_grant_generation, page_id,
                    applied_revision_number, created_at_ms, updated_at_ms, reviewed_at_ms,
                    reviewer_display_name, submission_count
             FROM shared_change_proposals
             WHERE workspace_id = ?1 AND proposal_id = ?2 AND agent_connection_id IS NOT NULL",
            params![workspace_id, proposal_id],
            |r| {
                Ok(RawEditProposal {
                    proposal_id: r.get(0)?,
                    connection_id: r.get(1)?,
                    base_revision_number: r.get(2)?,
                    proposed_title: r.get(3)?,
                    proposed_content_md: r.get(4)?,
                    summary: r.get(5)?,
                    status: r.get(6)?,
                    author_uid: r.get(7)?,
                    author_display_name: r.get(8)?,
                    agent_label: r.get(9)?,
                    model_label: r.get(10)?,
                    provider_label: r.get(11)?,
                    model_trust: r.get(12)?,
                    basis_json: r.get(13)?,
                    grant_generation: r.get(14)?,
                    page_id: r.get(15)?,
                    applied_revision_number: r.get(16)?,
                    created_at_ms: r.get(17)?,
                    updated_at_ms: r.get(18)?,
                    reviewed_at_ms: r.get(19)?,
                    reviewer_display_name: r.get(20)?,
                    submission_count: r.get(21)?,
                })
            },
        )
        .optional()?
        .ok_or(SharedContentError::AgentProposalNotFound)?;
    let connection = load_connection_row(tx, workspace_id, &raw.connection_id)?;
    let project = load_project_summary(tx, workspace_id, &connection.project_id)?;
    let page = load_page_row(tx, workspace_id, &raw.page_id)?;
    let state = revocation_state(tx, workspace_id, &raw.connection_id)?;
    let mut stale_reason = evaluate_stored_basis(
        tx,
        workspace_id,
        &raw.connection_id,
        raw.basis_json.as_deref().unwrap_or("[]"),
    )?;
    if stale_reason.is_none() && raw.grant_generation != Some(connection.generation) {
        stale_reason = Some(
            "This proposal was prepared under an older connection grant. Dismiss it or revise it against the updated page selection."
                .into(),
        );
    }
    if stale_reason.is_none() && page.current_revision_number != raw.base_revision_number {
        stale_reason = Some(format!(
            "“{}” advanced to revision {} after this proposal was based on revision {}.",
            page.title, page.current_revision_number, raw.base_revision_number
        ));
    }
    let sources = parse_source_basis(raw.basis_json.as_deref().unwrap_or("[]"))
        .into_iter()
        .map(|source| {
            let title =
                load_revision_row(tx, workspace_id, &source.page_id, source.revision_number)
                    .map(|p| p.title)
                    .unwrap_or_else(|_| source.page_id.clone());
            AgentProposalSource {
                page_id: source.page_id,
                page_title: title,
                revision_number: source.revision_number,
                content_sha256: source.content_sha256,
            }
        })
        .collect();
    Ok(AgentProposalView {
        proposal_id: raw.proposal_id,
        kind: AgentProposalKind::Edit,
        workspace_id: workspace_id.to_owned(),
        project_id: Some(connection.project_id.clone()),
        project_name: Some(project.name),
        target_page_id: Some(page.page_id.clone()),
        target_title: page.title,
        base_revision_number: Some(raw.base_revision_number),
        proposed_title: raw.proposed_title,
        proposed_content_md: raw.proposed_content_md,
        summary: raw.summary,
        status: raw.status,
        submission_count: raw.submission_count,
        author_uid: raw.author_uid,
        author_display_name: raw.author_display_name,
        connection_id: Some(connection.connection_id),
        connection_label: Some(connection.label),
        agent_label: raw
            .agent_label
            .unwrap_or_else(|| connection.client_name.clone()),
        model_label: raw.model_label.unwrap_or_else(|| "Not declared".into()),
        provider_label: raw.provider_label.unwrap_or_else(|| "Not declared".into()),
        model_trust: raw
            .model_trust
            .unwrap_or_else(|| "declared_unverified".into()),
        sources,
        is_stale: stale_reason.is_some(),
        stale_reason,
        review_eligible: false,
        revocation_state: state,
        applied_page_id: None,
        applied_revision_number: raw.applied_revision_number,
        created_at_ms: raw.created_at_ms,
        updated_at_ms: raw.updated_at_ms,
        reviewed_at_ms: raw.reviewed_at_ms,
        reviewer_display_name: raw.reviewer_display_name,
        reviewer_note: if raw.reviewed_at_ms.is_some() {
            tx.query_row("SELECT note FROM shared_proposal_decisions WHERE workspace_id=?1 AND proposal_id=?2 AND action IN ('applied','changes_requested','rejected') ORDER BY decision_seq DESC LIMIT 1", params![workspace_id, proposal_id], |r|r.get::<_,Option<String>>(0)).optional()?.flatten()
        } else {
            None
        },
    })
}

fn new_page_proposal_view(
    tx: &Transaction<'_>,
    workspace_id: &str,
    proposal_id: &str,
) -> Result<AgentProposalView, SharedContentError> {
    let row = tx
        .query_row(
            "SELECT proposal_id, project_id, proposed_title, proposed_content_md, summary,
                    status, author_uid, author_display_name, connection_id, agent_label,
                    model_label, provider_label, model_trust, source_basis_json,
                    grant_generation, created_at_ms, updated_at_ms, reviewed_at_ms,
                    reviewer_display_name, applied_page_id, applied_revision_number,
                    submission_count
             FROM shared_agent_page_proposals
             WHERE workspace_id = ?1 AND proposal_id = ?2",
            params![workspace_id, proposal_id],
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
                    r.get::<_, String>(8)?,
                    r.get::<_, String>(9)?,
                    r.get::<_, String>(10)?,
                    r.get::<_, String>(11)?,
                    r.get::<_, String>(12)?,
                    r.get::<_, String>(13)?,
                    r.get::<_, i64>(14)?,
                    r.get::<_, i64>(15)?,
                    r.get::<_, i64>(16)?,
                    r.get::<_, Option<i64>>(17)?,
                    r.get::<_, Option<String>>(18)?,
                    r.get::<_, Option<String>>(19)?,
                    r.get::<_, Option<i64>>(20)?,
                    r.get::<_, i64>(21)?,
                ))
            },
        )
        .optional()?
        .ok_or(SharedContentError::AgentProposalNotFound)?;
    let (
        proposal_id,
        project_id,
        proposed_title,
        proposed_content_md,
        summary,
        status,
        author_uid,
        author_display_name,
        connection_id,
        agent_label,
        model_label,
        provider_label,
        model_trust,
        basis_json,
        grant_generation,
        created_at_ms,
        updated_at_ms,
        reviewed_at_ms,
        reviewer_display_name,
        applied_page_id,
        applied_revision_number,
        submission_count,
    ) = row;
    let connection = load_connection_row(tx, workspace_id, &connection_id)?;
    let project = load_project_summary(tx, workspace_id, &project_id)?;
    let state = revocation_state(tx, workspace_id, &connection_id)?;
    let mut stale_reason = evaluate_stored_basis(tx, workspace_id, &connection_id, &basis_json)?;
    if stale_reason.is_none() && grant_generation != connection.generation {
        stale_reason = Some(
            "This proposal was prepared under an older connection grant. Dismiss it or revise it against the updated page selection."
                .into(),
        );
    }
    let sources = parse_source_basis(&basis_json)
        .into_iter()
        .map(|source| {
            let title =
                load_revision_row(tx, workspace_id, &source.page_id, source.revision_number)
                    .map(|p| p.title)
                    .unwrap_or_else(|_| source.page_id.clone());
            AgentProposalSource {
                page_id: source.page_id,
                page_title: title,
                revision_number: source.revision_number,
                content_sha256: source.content_sha256,
            }
        })
        .collect();
    let reviewer_note = tx.query_row("SELECT reviewer_note FROM shared_agent_page_proposals WHERE workspace_id=?1 AND proposal_id=?2", params![workspace_id, proposal_id], |r|r.get(0))?;
    Ok(AgentProposalView {
        proposal_id,
        kind: AgentProposalKind::NewPage,
        workspace_id: workspace_id.to_owned(),
        project_id: Some(project_id),
        project_name: Some(project.name),
        target_page_id: None,
        target_title: proposed_title.clone(),
        base_revision_number: None,
        proposed_title,
        proposed_content_md,
        summary,
        status,
        submission_count,
        author_uid,
        author_display_name,
        connection_id: Some(connection.connection_id),
        connection_label: Some(connection.label),
        agent_label,
        model_label,
        provider_label,
        model_trust,
        sources,
        is_stale: stale_reason.is_some(),
        stale_reason,
        review_eligible: false,
        revocation_state: state,
        applied_page_id,
        applied_revision_number,
        created_at_ms,
        updated_at_ms,
        reviewed_at_ms,
        reviewer_display_name,
        reviewer_note,
    })
}

fn lookup_agent_request(
    tx: &Transaction<'_>,
    connection_id: &str,
    operation: &str,
    request_id: &str,
    payload_hash: &str,
) -> Result<Option<String>, SharedContentError> {
    let row: Option<(String, String, String)> = tx
        .query_row(
            "SELECT payload_sha256, proposal_id, operation FROM shared_agent_proposal_requests
             WHERE connection_id = ?1 AND request_id = ?2",
            params![connection_id, request_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((stored_hash, proposal_id, stored_operation)) = row else {
        return Ok(None);
    };
    if stored_hash != payload_hash || stored_operation != operation {
        return Err(SharedContentError::IdempotencyConflict);
    }
    Ok(Some(proposal_id))
}

#[allow(clippy::too_many_arguments)]
fn save_agent_request(
    tx: &Transaction<'_>,
    connection_id: &str,
    operation: &str,
    request_id: &str,
    payload_hash: &str,
    proposal_id: &str,
    now_ms: i64,
) -> Result<(), SharedContentError> {
    tx.execute(
        "INSERT INTO shared_agent_proposal_requests (
            connection_id, operation, request_id, payload_sha256, proposal_id, created_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            connection_id,
            operation,
            request_id,
            payload_hash,
            proposal_id,
            now_ms
        ],
    )?;
    Ok(())
}

/// Adds the v5 agent-access tables. Additive only; safe to run against the signed-off CW3 schema.
pub(crate) fn migrate_v5(conn: &Connection) -> Result<(), SharedContentError> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS shared_agent_workspace_policies (
            workspace_id TEXT PRIMARY KEY REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
            enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
            updated_by_uid TEXT NOT NULL,
            updated_by_display_name TEXT NOT NULL,
            updated_at_ms INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS shared_agent_connections (
            connection_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
            project_id TEXT NOT NULL,
            member_uid TEXT NOT NULL,
            member_display_name TEXT NOT NULL,
            member_email TEXT NOT NULL,
            device_id TEXT NOT NULL,
            issuer_session_id TEXT NOT NULL,
            device_label TEXT NOT NULL,
            label TEXT NOT NULL,
            client_name TEXT NOT NULL,
            permission TEXT NOT NULL CHECK (permission IN ('read', 'suggest')),
            generation INTEGER NOT NULL CHECK (generation >= 1),
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            last_used_at_ms INTEGER,
            revoked_at_ms INTEGER,
            revoked_by_uid TEXT,
            revoked_by_display_name TEXT,
            FOREIGN KEY (workspace_id, project_id)
                REFERENCES shared_projects(workspace_id, project_id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_shared_agent_connections_member
            ON shared_agent_connections (workspace_id, member_uid, revoked_at_ms, created_at_ms DESC);

        CREATE TABLE IF NOT EXISTS shared_agent_connection_pages (
            connection_id TEXT NOT NULL
                REFERENCES shared_agent_connections(connection_id) ON DELETE CASCADE,
            workspace_id TEXT NOT NULL,
            project_id TEXT NOT NULL,
            page_id TEXT NOT NULL,
            membership_id TEXT NOT NULL,
            membership_generation INTEGER NOT NULL CHECK (membership_generation >= 1),
            grant_generation INTEGER NOT NULL CHECK (grant_generation >= 1),
            attached_at_ms INTEGER NOT NULL,
            PRIMARY KEY (connection_id, page_id)
        );

        CREATE TABLE IF NOT EXISTS shared_agent_connection_tokens (
            token_hash TEXT PRIMARY KEY,
            connection_id TEXT NOT NULL UNIQUE
                REFERENCES shared_agent_connections(connection_id) ON DELETE CASCADE,
            created_at_ms INTEGER NOT NULL,
            revoked_at_ms INTEGER
        );

        CREATE TABLE IF NOT EXISTS shared_agent_page_reads (
            connection_id TEXT NOT NULL
                REFERENCES shared_agent_connections(connection_id) ON DELETE CASCADE,
            page_id TEXT NOT NULL,
            revision_number INTEGER NOT NULL,
            content_sha256 TEXT NOT NULL,
            read_at_ms INTEGER NOT NULL,
            PRIMARY KEY (connection_id, page_id)
        );
        ",
    )?;
    migrate_v5_proposals(conn)
}

fn migrate_v5_proposals(conn: &Connection) -> Result<(), SharedContentError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS shared_agent_page_proposals (
            proposal_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
            project_id TEXT NOT NULL,
            proposed_title TEXT NOT NULL,
            proposed_content_md TEXT NOT NULL,
            summary TEXT NOT NULL,
            author_uid TEXT NOT NULL,
            author_display_name TEXT NOT NULL,
            connection_id TEXT NOT NULL,
            agent_label TEXT NOT NULL,
            model_label TEXT NOT NULL,
            provider_label TEXT NOT NULL,
            model_trust TEXT NOT NULL,
            source_basis_json TEXT NOT NULL,
            grant_generation INTEGER NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('pending', 'changes_requested', 'rejected', 'applied', 'withdrawn')),
            submission_count INTEGER NOT NULL DEFAULT 1 CHECK (submission_count >= 1),
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            reviewed_at_ms INTEGER,
            reviewer_uid TEXT,
            reviewer_display_name TEXT,
            reviewer_note TEXT,
            applied_page_id TEXT,
            applied_revision_number INTEGER,
            FOREIGN KEY (workspace_id, project_id)
                REFERENCES shared_projects(workspace_id, project_id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_shared_agent_page_proposals_ws
            ON shared_agent_page_proposals (workspace_id, status, updated_at_ms DESC, proposal_id DESC);

        CREATE TABLE IF NOT EXISTS shared_agent_proposal_requests (
            connection_id TEXT NOT NULL,
            operation TEXT NOT NULL,
            request_id TEXT NOT NULL,
            payload_sha256 TEXT NOT NULL,
            proposal_id TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL,
            PRIMARY KEY (connection_id, operation, request_id)
        );

        CREATE TABLE IF NOT EXISTS shared_agent_audit (
            seq INTEGER PRIMARY KEY AUTOINCREMENT,
            workspace_id TEXT NOT NULL,
            connection_id TEXT NOT NULL,
            member_uid TEXT NOT NULL,
            action TEXT NOT NULL,
            resource_type TEXT NOT NULL,
            resource_id TEXT,
            outcome TEXT NOT NULL,
            proposal_id TEXT,
            created_at_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_shared_agent_audit_ws
            ON shared_agent_audit (workspace_id, seq DESC);
        ",
    )?;
    ensure_agent_columns(conn)?;
    conn.execute_batch(
        "
        CREATE TRIGGER IF NOT EXISTS shared_agent_grant_changed
        AFTER UPDATE OF generation ON shared_agent_connections
        WHEN NEW.generation != OLD.generation
        BEGIN
            DELETE FROM shared_agent_page_reads WHERE connection_id = NEW.connection_id;
            UPDATE shared_agent_connection_pages SET grant_generation = NEW.generation
            WHERE connection_id = NEW.connection_id;
        END;

        CREATE TRIGGER IF NOT EXISTS shared_agent_member_removed
        AFTER UPDATE OF removed_at_ms ON workspace_memberships
        WHEN OLD.removed_at_ms IS NULL AND NEW.removed_at_ms IS NOT NULL
        BEGIN
            UPDATE shared_agent_connections
            SET revoked_at_ms = NEW.removed_at_ms, generation = generation + 1
            WHERE workspace_id = NEW.workspace_id AND member_uid = NEW.uid
              AND revoked_at_ms IS NULL;
        END;

        CREATE TRIGGER IF NOT EXISTS shared_agent_member_downgraded
        AFTER UPDATE OF role ON workspace_memberships
        WHEN OLD.role != NEW.role AND NEW.role = 'viewer'
        BEGIN
            UPDATE shared_agent_connections SET permission = 'read', generation = generation + 1
            WHERE workspace_id = NEW.workspace_id AND member_uid = NEW.uid
              AND revoked_at_ms IS NULL;
        END;

        CREATE TRIGGER IF NOT EXISTS shared_agent_policy_disabled
        AFTER UPDATE OF enabled ON shared_agent_workspace_policies
        WHEN OLD.enabled = 1 AND NEW.enabled = 0
        BEGIN
            UPDATE shared_agent_connections SET generation = generation + 1
            WHERE workspace_id = NEW.workspace_id AND revoked_at_ms IS NULL;
        END;

        CREATE TRIGGER IF NOT EXISTS shared_agent_project_archived
        AFTER UPDATE OF archived_at_ms ON shared_projects
        WHEN OLD.archived_at_ms IS NULL AND NEW.archived_at_ms IS NOT NULL
        BEGIN
            UPDATE shared_agent_connections SET generation = generation + 1
            WHERE workspace_id = NEW.workspace_id AND project_id = NEW.project_id
              AND revoked_at_ms IS NULL;
        END;

        CREATE TRIGGER IF NOT EXISTS shared_agent_page_archived
        AFTER UPDATE OF archived_at_ms ON shared_pages
        WHEN OLD.archived_at_ms IS NULL AND NEW.archived_at_ms IS NOT NULL
        BEGIN
            DELETE FROM shared_agent_page_reads
            WHERE page_id = NEW.page_id AND connection_id IN (
                SELECT connection_id FROM shared_agent_connection_pages
                WHERE workspace_id = NEW.workspace_id AND page_id = NEW.page_id
            );
            DELETE FROM shared_agent_connection_pages
            WHERE workspace_id = NEW.workspace_id AND page_id = NEW.page_id;
        END;
        ",
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, name, applied_at_ms)
         VALUES (5, 'cw4_scoped_shared_agent_access_v5', ?1)",
        params![crate::now_epoch_ms()],
    )?;
    Ok(())
}

fn ensure_agent_columns(conn: &Connection) -> Result<(), SharedContentError> {
    let ensure_col = |table: &str, col: &str, ddl: &str| -> Result<(), SharedContentError> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        if !cols.iter().any(|c| c == col) {
            conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {ddl}"), [])?;
        }
        Ok(())
    };
    for (col, ddl) in [
        ("agent_connection_id", "agent_connection_id TEXT"),
        ("agent_label", "agent_label TEXT"),
        ("agent_model_label", "agent_model_label TEXT"),
        ("agent_provider_label", "agent_provider_label TEXT"),
        ("agent_model_trust", "agent_model_trust TEXT"),
        ("source_basis_json", "source_basis_json TEXT"),
        ("agent_grant_generation", "agent_grant_generation INTEGER"),
    ] {
        ensure_col("shared_change_proposals", col, ddl)?;
    }
    Ok(())
}
