//! CW2 Shared-Content Storage Contract and Transactional SQLite QA Adapter.
//!
//! Authorization & transaction boundary:
//! 1. HTTP handlers validate the caller's opaque bearer credential against the separate CW1
//!    `AuthSessionStore` (`verify_session`) to resolve a verified `(uid, display_name)`.
//! 2. `SqliteSharedContentStore` operates directly on the shared workspace database (when CW2
//!    shared content is explicitly enabled alongside CW1 membership) using an `IMMEDIATE`
//!    SQLite transaction that re-reads active `workspace_memberships` for `(workspace_id, caller.uid)`,
//!    checks role/author/page/revision invariants, writes content/revisions/decisions/activity/
//!    idempotency receipts atomically, and executes `SessionRecheck` against `AuthSessionStore`
//!    immediately before `tx.commit()`.
//! 3. Lock order is strictly `WorkspaceDB -> AuthSessionStore`; `AuthSessionStore` never acquires
//!    the workspace DB lock, eliminating lock-order deadlocks across the boundary.

use crate::membership::{
    caller_display_name, require_active_membership, sanitize_idempotency_key, MembershipError,
    SessionRecheck, SqliteWorkspaceMembershipStore, WorkspaceRole,
};
use crate::store::{sha256_hex, StoreError, VerifiedSession};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use uuid::Uuid;

pub const MAX_PAGE_TITLE_LEN: usize = 160;
pub const MAX_PAGE_CONTENT_BYTES: usize = 48 * 1024;
pub const MAX_PROJECT_NAME_LEN: usize = 120;
pub const MAX_PROJECT_PURPOSE_LEN: usize = 500;
pub const MAX_SEARCH_QUERY_LEN: usize = 120;
pub const MAX_COMMENT_BODY_BYTES: usize = 8 * 1024;
pub const MAX_PROPOSAL_SUMMARY_LEN: usize = 280;
pub const MAX_DECISION_NOTE_LEN: usize = 500;
pub const MAX_PROVENANCE_LABEL_LEN: usize = 160;
pub const DEFAULT_PAGE_LIMIT: usize = 30;
pub const MAX_PAGE_LIMIT: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PageEntryMode {
    New,
    CopyPersonal,
}

impl PageEntryMode {
    pub fn as_str(self) -> &'static str {
        match self {
            PageEntryMode::New => "new",
            PageEntryMode::CopyPersonal => "copy_personal",
        }
    }

    pub fn from_db(raw: &str) -> Result<Self, SharedContentError> {
        match raw {
            "new" => Ok(PageEntryMode::New),
            "copy_personal" => Ok(PageEntryMode::CopyPersonal),
            _ => Err(SharedContentError::InvalidInput(
                "Unknown entry mode.".into(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PageOriginKind {
    UserAuthored,
    AiAssisted,
    ExternalSource,
}

impl PageOriginKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PageOriginKind::UserAuthored => "user_authored",
            PageOriginKind::AiAssisted => "ai_assisted",
            PageOriginKind::ExternalSource => "external_source",
        }
    }

    pub fn from_db(raw: &str) -> Result<Self, SharedContentError> {
        match raw {
            "user_authored" => Ok(PageOriginKind::UserAuthored),
            "ai_assisted" => Ok(PageOriginKind::AiAssisted),
            "external_source" => Ok(PageOriginKind::ExternalSource),
            _ => Err(SharedContentError::InvalidInput(
                "Unknown origin kind.".into(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareableProvenance {
    pub origin_kind: PageOriginKind,
    #[serde(default)]
    pub original_author_label: Option<String>,
    #[serde(default)]
    pub ai_model_label: Option<String>,
    #[serde(default)]
    pub source_note: Option<String>,
}

impl Default for ShareableProvenance {
    fn default() -> Self {
        Self {
            origin_kind: PageOriginKind::UserAuthored,
            original_author_label: None,
            ai_model_label: None,
            source_note: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSharedPageInput {
    pub title: String,
    pub content_md: String,
    pub entry_mode: PageEntryMode,
    #[serde(default)]
    pub provenance: ShareableProvenance,
    #[serde(default)]
    pub omitted_summary: Vec<String>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedPageSummary {
    pub page_id: String,
    pub workspace_id: String,
    pub title: String,
    pub current_revision_number: i64,
    pub entry_mode: PageEntryMode,
    pub provenance: ShareableProvenance,
    pub omitted_summary: Vec<String>,
    pub publisher_uid: String,
    pub publisher_display_name: String,
    pub last_editor_uid: String,
    pub last_editor_display_name: String,
    pub last_reviewer_uid: Option<String>,
    pub last_reviewer_display_name: Option<String>,
    pub archived_at_ms: Option<i64>,
    pub archived_by_display_name: Option<String>,
    pub unresolved_thread_count: i64,
    pub pending_proposal_count: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedPageRevisionView {
    pub revision_id: String,
    pub workspace_id: String,
    pub page_id: String,
    pub revision_number: i64,
    pub title: String,
    pub content_md: String,
    pub content_sha256: String,
    pub change_summary: String,
    pub author_uid: String,
    pub author_display_name: String,
    pub reviewer_uid: Option<String>,
    pub reviewer_display_name: Option<String>,
    pub proposal_id: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThreadStatus {
    Open,
    Resolved,
}

impl ThreadStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ThreadStatus::Open => "open",
            ThreadStatus::Resolved => "resolved",
        }
    }

    pub fn from_db(raw: &str) -> Result<Self, SharedContentError> {
        match raw {
            "open" => Ok(ThreadStatus::Open),
            "resolved" => Ok(ThreadStatus::Resolved),
            _ => Err(SharedContentError::InvalidInput(
                "Unknown thread status.".into(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedCommentMessageView {
    pub message_id: String,
    pub thread_id: String,
    pub author_uid: String,
    pub author_display_name: String,
    pub body: String,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedCommentThreadView {
    pub thread_id: String,
    pub workspace_id: String,
    pub page_id: String,
    pub base_revision_number: i64,
    pub author_uid: String,
    pub author_display_name: String,
    pub status: ThreadStatus,
    pub resolved_at_ms: Option<i64>,
    pub resolved_by_display_name: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub messages: Vec<SharedCommentMessageView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProposalStatus {
    Pending,
    ChangesRequested,
    Rejected,
    Applied,
    Withdrawn,
}

impl ProposalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ProposalStatus::Pending => "pending",
            ProposalStatus::ChangesRequested => "changes_requested",
            ProposalStatus::Rejected => "rejected",
            ProposalStatus::Applied => "applied",
            ProposalStatus::Withdrawn => "withdrawn",
        }
    }

    pub fn from_db(raw: &str) -> Result<Self, SharedContentError> {
        match raw {
            "pending" => Ok(ProposalStatus::Pending),
            "changes_requested" => Ok(ProposalStatus::ChangesRequested),
            "rejected" => Ok(ProposalStatus::Rejected),
            "applied" => Ok(ProposalStatus::Applied),
            "withdrawn" => Ok(ProposalStatus::Withdrawn),
            _ => Err(SharedContentError::InvalidInput(
                "Unknown proposal status.".into(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProposalReviewAction {
    Apply,
    RequestChanges,
    Reject,
}

impl ProposalReviewAction {
    pub fn as_str(self) -> &'static str {
        match self {
            ProposalReviewAction::Apply => "apply",
            ProposalReviewAction::RequestChanges => "request_changes",
            ProposalReviewAction::Reject => "reject",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalDecisionView {
    pub decision_id: String,
    pub submission_number: i64,
    pub action: String,
    pub actor_uid: String,
    pub actor_display_name: String,
    pub note: Option<String>,
    pub base_revision_number: i64,
    pub submitted_title: String,
    pub submitted_content_md: String,
    pub resulting_revision_number: Option<i64>,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedProposalSummary {
    pub proposal_id: String,
    pub workspace_id: String,
    pub page_id: String,
    pub page_title: String,
    pub base_revision_number: i64,
    pub summary: String,
    pub author_uid: String,
    pub author_display_name: String,
    pub status: ProposalStatus,
    pub submission_count: i64,
    pub is_stale: bool,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub reviewed_at_ms: Option<i64>,
    pub reviewer_display_name: Option<String>,
    pub applied_revision_number: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedProposalDetails {
    pub proposal: SharedProposalSummary,
    pub base_title: String,
    pub base_content_md: String,
    pub proposed_title: String,
    pub proposed_content_md: String,
    pub current_page_revision_number: i64,
    pub current_page_title: String,
    pub current_page_content_md: String,
    pub page_archived: bool,
    pub can_review: bool,
    pub can_author_edit: bool,
    pub decisions: Vec<ProposalDecisionView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedPageDetails {
    pub page: SharedPageSummary,
    pub current_revision: SharedPageRevisionView,
    pub revisions: Vec<SharedPageRevisionView>,
    pub revisions_next_cursor: Option<String>,
    pub threads: Vec<SharedCommentThreadView>,
    pub threads_next_cursor: Option<String>,
    pub proposals: Vec<SharedProposalSummary>,
    pub proposals_next_cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSharedPageOutcome {
    pub created: bool,
    pub page: SharedPageDetails,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProposalInput {
    pub base_revision_number: i64,
    pub proposed_title: String,
    pub proposed_content_md: String,
    pub summary: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateThreadInput {
    pub base_revision_number: i64,
    pub body: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplyThreadInput {
    pub body: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewProposalInput {
    pub action: ProposalReviewAction,
    pub expected_current_revision: i64,
    pub expected_submission_number: i64,
    #[serde(default)]
    pub note: Option<String>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedActivityItem {
    pub activity_id: String,
    pub workspace_id: String,
    pub actor_uid: String,
    pub actor_display_name: String,
    pub action: String,
    pub target_type: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub project_name: Option<String>,
    pub page_id: Option<String>,
    pub page_title: Option<String>,
    pub proposal_id: Option<String>,
    pub thread_id: Option<String>,
    pub detail_text: String,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedProjectSummary {
    pub project_id: String,
    pub workspace_id: String,
    pub name: String,
    pub purpose: String,
    pub version: i64,
    pub active_page_count: i64,
    pub created_by_uid: String,
    pub created_by_display_name: String,
    pub updated_by_uid: String,
    pub updated_by_display_name: String,
    pub archived_at_ms: Option<i64>,
    pub archived_by_display_name: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedProjectPageItem {
    pub membership_id: String,
    pub generation: i64,
    pub attached_by_uid: String,
    pub attached_by_display_name: String,
    pub attached_at_ms: i64,
    pub page: SharedPageSummary,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedProjectDetails {
    pub project: SharedProjectSummary,
    pub pages: Vec<SharedProjectPageItem>,
    pub pages_next_cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSharedProjectInput {
    pub name: String,
    #[serde(default)]
    pub purpose: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSharedProjectOutcome {
    pub created: bool,
    pub project: SharedProjectDetails,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSharedProjectInput {
    pub name: String,
    #[serde(default)]
    pub purpose: String,
    pub expected_version: i64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachProjectPageInput {
    pub page_id: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPageMembershipOutcome {
    pub attached: bool,
    pub membership_id: String,
    pub generation: i64,
    pub project: SharedProjectDetails,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetachProjectPageInput {
    pub membership_id: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PagedList<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Error)]
pub enum SharedContentError {
    #[error("{0}")]
    InvalidInput(String),
    #[error("{0}")]
    ContentTooLarge(String),
    #[error("Authentication required: {0}")]
    Session(#[from] StoreError),
    #[error("You do not have permission to perform this workspace action.")]
    Forbidden,
    #[error("Another Owner or Admin must review this proposal.")]
    SelfReviewForbidden,
    #[error("Shared page was not found in this workspace.")]
    PageNotFound,
    #[error("Page revision was not found.")]
    RevisionNotFound,
    #[error("Discussion thread was not found in this workspace.")]
    ThreadNotFound,
    #[error("Proposal was not found in this workspace.")]
    ProposalNotFound,
    #[error("Shared project was not found in this workspace.")]
    ProjectNotFound,
    #[error("This page is archived. Restore it before submitting or reviewing changes.")]
    PageArchived,
    #[error("This page is already archived.")]
    AlreadyArchived,
    #[error("This page is not archived.")]
    NotArchived,
    #[error("This project is archived. Restore it before changing details or pages.")]
    ProjectArchived,
    #[error("This project is already archived.")]
    ProjectAlreadyArchived,
    #[error("This project is not archived.")]
    ProjectNotArchived,
    #[error("This page is already in this project.")]
    AlreadyProjectMember,
    #[error("{0}")]
    StaleProjectVersion(String),
    #[error("{0}")]
    StaleMembership(String),
    #[error("This proposal is no longer open for that action.")]
    ProposalNotPending,
    #[error("{0}")]
    StaleSubmission(String),
    #[error("{0}")]
    StaleProposal(String),
    #[error("This request identifier was already used with different input.")]
    IdempotencyConflict,
    #[error("Invalid pagination cursor.")]
    InvalidCursor,
    #[error("Shared content storage unavailable.")]
    Storage(#[from] rusqlite::Error),
}

impl From<MembershipError> for SharedContentError {
    fn from(err: MembershipError) -> Self {
        match err {
            MembershipError::InvalidInput(msg) => SharedContentError::InvalidInput(msg),
            MembershipError::Session(e) => SharedContentError::Session(e),
            MembershipError::Forbidden | MembershipError::WorkspaceNotFound => {
                SharedContentError::Forbidden
            }
            MembershipError::Storage(e) => SharedContentError::Storage(e),
            _ => SharedContentError::Forbidden,
        }
    }
}

/// Persistence contract for CW2 shared pages, discussions, proposals, and workspace activity.
pub trait SharedContentStore: Send + Sync {
    fn list_pages(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        include_archived: bool,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<PagedList<SharedPageSummary>, SharedContentError>;

    fn create_page(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        input: &CreateSharedPageInput,
        now_ms: i64,
    ) -> Result<CreateSharedPageOutcome, SharedContentError>;

    fn get_page_details(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        page_id: &str,
    ) -> Result<SharedPageDetails, SharedContentError>;

    fn list_revisions(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        page_id: &str,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<PagedList<SharedPageRevisionView>, SharedContentError>;

    fn archive_page(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedPageDetails, SharedContentError>;

    fn restore_page(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedPageDetails, SharedContentError>;

    fn list_threads(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        page_id: &str,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<PagedList<SharedCommentThreadView>, SharedContentError>;

    fn create_thread(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        input: &CreateThreadInput,
        now_ms: i64,
    ) -> Result<SharedCommentThreadView, SharedContentError>;

    #[allow(clippy::too_many_arguments)]
    fn reply_thread(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        thread_id: &str,
        input: &ReplyThreadInput,
        now_ms: i64,
    ) -> Result<SharedCommentThreadView, SharedContentError>;

    #[allow(clippy::too_many_arguments)]
    fn resolve_thread(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        thread_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedCommentThreadView, SharedContentError>;

    #[allow(clippy::too_many_arguments)]
    fn reopen_thread(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        thread_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedCommentThreadView, SharedContentError>;

    fn list_proposals(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        page_id: &str,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<PagedList<SharedProposalSummary>, SharedContentError>;

    fn get_proposal_details(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        proposal_id: &str,
    ) -> Result<SharedProposalDetails, SharedContentError>;

    fn create_proposal(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        input: &CreateProposalInput,
        now_ms: i64,
    ) -> Result<SharedProposalDetails, SharedContentError>;

    fn resubmit_proposal(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        proposal_id: &str,
        input: &CreateProposalInput,
        now_ms: i64,
    ) -> Result<SharedProposalDetails, SharedContentError>;

    fn withdraw_proposal(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        proposal_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedProposalDetails, SharedContentError>;

    fn review_proposal(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        proposal_id: &str,
        input: &ReviewProposalInput,
        now_ms: i64,
    ) -> Result<SharedProposalDetails, SharedContentError>;

    fn list_activity(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<PagedList<SharedActivityItem>, SharedContentError>;

    fn list_projects(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        include_archived: bool,
        search_query: Option<&str>,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<PagedList<SharedProjectSummary>, SharedContentError>;

    fn create_project(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        input: &CreateSharedProjectInput,
        now_ms: i64,
    ) -> Result<CreateSharedProjectOutcome, SharedContentError>;

    fn get_project_details(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        project_id: &str,
        search_query: Option<&str>,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<SharedProjectDetails, SharedContentError>;

    fn update_project(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        project_id: &str,
        input: &UpdateSharedProjectInput,
        now_ms: i64,
    ) -> Result<SharedProjectDetails, SharedContentError>;

    fn archive_project(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        project_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedProjectDetails, SharedContentError>;

    fn restore_project(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        project_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedProjectDetails, SharedContentError>;

    fn list_project_picker_pages(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        project_id: &str,
        search_query: Option<&str>,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<PagedList<SharedPageSummary>, SharedContentError>;

    fn attach_page_to_project(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        project_id: &str,
        input: &AttachProjectPageInput,
        now_ms: i64,
    ) -> Result<ProjectPageMembershipOutcome, SharedContentError>;

    #[allow(clippy::too_many_arguments)]
    fn detach_page_from_project(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        project_id: &str,
        page_id: &str,
        input: &DetachProjectPageInput,
        now_ms: i64,
    ) -> Result<SharedProjectDetails, SharedContentError>;
}

#[derive(Clone)]
pub struct SqliteSharedContentStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteSharedContentStore {
    /// Attaches CW2 shared-content tables (migrations v2 and v3) inside the existing CW1
    /// membership SQLite database so authorization and content mutations share a single ACID
    /// transaction.
    pub fn enable_on_membership_store(
        membership: &SqliteWorkspaceMembershipStore,
    ) -> Result<Self, SharedContentError> {
        let conn = Arc::clone(membership.conn());
        {
            let guard = conn.lock().expect("workspace db lock poisoned");
            migrate_v2(&guard)?;
            migrate_v3(&guard)?;
        }
        Ok(Self { conn })
    }

    pub fn schema_version(&self) -> Result<i64, SharedContentError> {
        let guard = self.lock_conn();
        let ver: Option<i64> = guard
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })
            .optional()?
            .flatten();
        Ok(ver.unwrap_or(0))
    }

    fn lock_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("workspace db lock poisoned")
    }
}

fn migrate_v2(conn: &Connection) -> Result<(), SharedContentError> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS shared_pages (
            page_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            content_md TEXT NOT NULL,
            current_revision_number INTEGER NOT NULL CHECK (current_revision_number >= 1),
            entry_mode TEXT NOT NULL CHECK (entry_mode IN ('new', 'copy_personal')),
            origin_kind TEXT NOT NULL CHECK (origin_kind IN ('user_authored', 'ai_assisted', 'external_source')),
            original_author_label TEXT,
            ai_model_label TEXT,
            source_note TEXT,
            omitted_summary_json TEXT NOT NULL,
            publisher_uid TEXT NOT NULL,
            publisher_display_name TEXT NOT NULL,
            last_editor_uid TEXT NOT NULL,
            last_editor_display_name TEXT NOT NULL,
            last_reviewer_uid TEXT,
            last_reviewer_display_name TEXT,
            archived_at_ms INTEGER,
            archived_by_uid TEXT,
            archived_by_display_name TEXT,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_shared_pages_ws
            ON shared_pages (workspace_id, archived_at_ms, updated_at_ms DESC, page_id DESC);

        CREATE TABLE IF NOT EXISTS shared_page_revisions (
            revision_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
            page_id TEXT NOT NULL REFERENCES shared_pages(page_id) ON DELETE CASCADE,
            revision_number INTEGER NOT NULL CHECK (revision_number >= 1),
            title TEXT NOT NULL,
            content_md TEXT NOT NULL,
            content_sha256 TEXT NOT NULL,
            change_summary TEXT NOT NULL,
            author_uid TEXT NOT NULL,
            author_display_name TEXT NOT NULL,
            reviewer_uid TEXT,
            reviewer_display_name TEXT,
            proposal_id TEXT,
            created_at_ms INTEGER NOT NULL,
            UNIQUE (page_id, revision_number)
        );
        CREATE INDEX IF NOT EXISTS idx_shared_revisions_page
            ON shared_page_revisions (workspace_id, page_id, revision_number DESC);

        CREATE TABLE IF NOT EXISTS shared_comment_threads (
            thread_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
            page_id TEXT NOT NULL REFERENCES shared_pages(page_id) ON DELETE CASCADE,
            base_revision_number INTEGER NOT NULL,
            author_uid TEXT NOT NULL,
            author_display_name TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('open', 'resolved')),
            resolved_at_ms INTEGER,
            resolved_by_uid TEXT,
            resolved_by_display_name TEXT,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_shared_threads_page
            ON shared_comment_threads (workspace_id, page_id, updated_at_ms DESC, thread_id DESC);

        CREATE TABLE IF NOT EXISTS shared_comment_messages (
            message_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
            page_id TEXT NOT NULL REFERENCES shared_pages(page_id) ON DELETE CASCADE,
            thread_id TEXT NOT NULL REFERENCES shared_comment_threads(thread_id) ON DELETE CASCADE,
            author_uid TEXT NOT NULL,
            author_display_name TEXT NOT NULL,
            body TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_shared_messages_thread
            ON shared_comment_messages (workspace_id, thread_id, created_at_ms ASC, message_id ASC);

        CREATE TABLE IF NOT EXISTS shared_change_proposals (
            proposal_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
            page_id TEXT NOT NULL REFERENCES shared_pages(page_id) ON DELETE CASCADE,
            base_revision_number INTEGER NOT NULL,
            base_title TEXT NOT NULL,
            base_content_md TEXT NOT NULL,
            proposed_title TEXT NOT NULL,
            proposed_content_md TEXT NOT NULL,
            summary TEXT NOT NULL,
            author_uid TEXT NOT NULL,
            author_display_name TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('pending', 'changes_requested', 'rejected', 'applied', 'withdrawn')),
            submission_count INTEGER NOT NULL CHECK (submission_count >= 1),
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            reviewed_at_ms INTEGER,
            reviewer_uid TEXT,
            reviewer_display_name TEXT,
            reviewer_note TEXT,
            applied_revision_number INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_shared_proposals_page
            ON shared_change_proposals (workspace_id, page_id, updated_at_ms DESC, proposal_id DESC);

        CREATE TABLE IF NOT EXISTS shared_proposal_decisions (
            decision_seq INTEGER PRIMARY KEY AUTOINCREMENT,
            decision_id TEXT NOT NULL UNIQUE,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
            proposal_id TEXT NOT NULL REFERENCES shared_change_proposals(proposal_id) ON DELETE CASCADE,
            submission_number INTEGER NOT NULL,
            action TEXT NOT NULL,
            actor_uid TEXT NOT NULL,
            actor_display_name TEXT NOT NULL,
            note TEXT,
            base_revision_number INTEGER NOT NULL,
            submitted_title TEXT NOT NULL,
            submitted_content_md TEXT NOT NULL,
            resulting_revision_number INTEGER,
            created_at_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_shared_decisions_proposal
            ON shared_proposal_decisions (workspace_id, proposal_id, decision_seq ASC);

        CREATE TABLE IF NOT EXISTS shared_workspace_activity (
            seq INTEGER PRIMARY KEY AUTOINCREMENT,
            activity_id TEXT NOT NULL UNIQUE,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
            actor_uid TEXT NOT NULL,
            actor_display_name TEXT NOT NULL,
            action TEXT NOT NULL,
            target_type TEXT NOT NULL,
            page_id TEXT,
            page_title TEXT,
            proposal_id TEXT,
            thread_id TEXT,
            detail_text TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_shared_activity_ws
            ON shared_workspace_activity (workspace_id, seq DESC);

        CREATE TABLE IF NOT EXISTS shared_write_idempotency (
            actor_uid TEXT NOT NULL,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
            operation TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            payload_sha256 TEXT NOT NULL,
            response_json TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL,
            PRIMARY KEY (actor_uid, workspace_id, operation, idempotency_key)
        );
        ",
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, name, applied_at_ms)
         VALUES (2, 'cw2_shared_content_v2', ?1)",
        params![crate::now_epoch_ms()],
    )?;
    Ok(())
}

fn migrate_v3(conn: &Connection) -> Result<(), SharedContentError> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        CREATE UNIQUE INDEX IF NOT EXISTS idx_shared_pages_ws_page
            ON shared_pages (workspace_id, page_id);

        CREATE TABLE IF NOT EXISTS shared_projects (
            project_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            purpose TEXT NOT NULL,
            version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
            created_by_uid TEXT NOT NULL,
            created_by_display_name TEXT NOT NULL,
            updated_by_uid TEXT NOT NULL,
            updated_by_display_name TEXT NOT NULL,
            archived_at_ms INTEGER,
            archived_by_uid TEXT,
            archived_by_display_name TEXT,
            created_at_ms INTEGER NOT NULL,
            updated_at_ms INTEGER NOT NULL,
            UNIQUE (workspace_id, project_id)
        );
        CREATE INDEX IF NOT EXISTS idx_shared_projects_ws
            ON shared_projects (workspace_id, archived_at_ms, updated_at_ms DESC, project_id DESC);

        CREATE TABLE IF NOT EXISTS shared_project_pages (
            membership_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL,
            project_id TEXT NOT NULL,
            page_id TEXT NOT NULL,
            generation INTEGER NOT NULL CHECK (generation >= 1),
            attached_by_uid TEXT NOT NULL,
            attached_by_display_name TEXT NOT NULL,
            attached_at_ms INTEGER NOT NULL,
            detached_at_ms INTEGER,
            detached_by_uid TEXT,
            detached_by_display_name TEXT,
            FOREIGN KEY (workspace_id, project_id)
                REFERENCES shared_projects(workspace_id, project_id) ON DELETE CASCADE,
            FOREIGN KEY (workspace_id, page_id)
                REFERENCES shared_pages(workspace_id, page_id) ON DELETE CASCADE
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_shared_project_pages_active
            ON shared_project_pages (workspace_id, project_id, page_id)
            WHERE detached_at_ms IS NULL;
        CREATE INDEX IF NOT EXISTS idx_shared_project_pages_project
            ON shared_project_pages (workspace_id, project_id, detached_at_ms, attached_at_ms DESC, membership_id DESC);
        ",
    )?;

    let mut stmt = conn.prepare("PRAGMA table_info(shared_workspace_activity)")?;
    let cols: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    if !cols.iter().any(|c| c == "project_id") {
        conn.execute(
            "ALTER TABLE shared_workspace_activity ADD COLUMN project_id TEXT",
            [],
        )?;
    }
    if !cols.iter().any(|c| c == "project_name") {
        conn.execute(
            "ALTER TABLE shared_workspace_activity ADD COLUMN project_name TEXT",
            [],
        )?;
    }

    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, name, applied_at_ms)
         VALUES (3, 'cw2_shared_projects_v3', ?1)",
        params![crate::now_epoch_ms()],
    )?;
    Ok(())
}

impl SharedContentStore for SqliteSharedContentStore {
    fn list_pages(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        include_archived: bool,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<PagedList<SharedPageSummary>, SharedContentError> {
        let limit = clamp_limit(limit);
        let parsed_cursor = parse_time_id_cursor(cursor)?;
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let _member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let items_plus_one = query_page_summaries(
            &tx,
            workspace_id,
            include_archived,
            parsed_cursor.as_ref().map(|(ts, id)| (*ts, id.as_str())),
            limit + 1,
        )?;
        let (items, next_cursor) = paginate(items_plus_one, limit, |p| {
            format!("{}:{}", p.updated_at_ms, p.page_id)
        });
        tx.commit()?;
        Ok(PagedList { items, next_cursor })
    }

    fn create_page(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        input: &CreateSharedPageInput,
        now_ms: i64,
    ) -> Result<CreateSharedPageOutcome, SharedContentError> {
        let title = sanitize_page_title(&input.title)?;
        let content_md = sanitize_page_content(&input.content_md)?;
        let provenance = sanitize_provenance(&input.provenance)?;
        let omitted_summary = sanitize_omitted_summary(&input.omitted_summary)?;
        let idempotency_key = sanitize_idempotency_key(&input.idempotency_key)?;
        let payload_hash = payload_digest(&serde_json::json!({
            "title": title,
            "content_md": content_md,
            "entry_mode": input.entry_mode.as_str(),
            "provenance": provenance,
            "omitted_summary": omitted_summary,
        }));
        let actor_display = caller_display_name(caller);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_author_shared_content() {
            return Err(SharedContentError::Forbidden);
        }

        if let Some(cached) = lookup_idempotent_response::<SharedPageDetails>(
            &tx,
            &caller.uid,
            workspace_id,
            "create_page",
            &idempotency_key,
            &payload_hash,
        )? {
            tx.commit()?;
            return Ok(CreateSharedPageOutcome {
                created: false,
                page: cached,
            });
        }

        let page_id = format!("shp_{}", Uuid::new_v4().simple());
        let rev_id = format!("rev_{}", Uuid::new_v4().simple());
        let content_sha = sha256_hex(&content_md);
        let omitted_json = serde_json::to_string(&omitted_summary).unwrap_or_else(|_| "[]".into());
        let change_summary = match input.entry_mode {
            PageEntryMode::New => "Initial shared page".to_owned(),
            PageEntryMode::CopyPersonal => "Published reviewed personal copy".to_owned(),
        };

        tx.execute(
            "INSERT INTO shared_pages (
                page_id, workspace_id, title, content_md, current_revision_number,
                entry_mode, origin_kind, original_author_label, ai_model_label, source_note,
                omitted_summary_json, publisher_uid, publisher_display_name,
                last_editor_uid, last_editor_display_name, created_at_ms, updated_at_ms
            ) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?11, ?12, ?13, ?13)",
            params![
                page_id,
                workspace_id,
                title,
                content_md,
                input.entry_mode.as_str(),
                provenance.origin_kind.as_str(),
                provenance.original_author_label,
                provenance.ai_model_label,
                provenance.source_note,
                omitted_json,
                caller.uid,
                actor_display,
                now_ms,
            ],
        )?;

        tx.execute(
            "INSERT INTO shared_page_revisions (
                revision_id, workspace_id, page_id, revision_number, title, content_md,
                content_sha256, change_summary, author_uid, author_display_name, created_at_ms
            ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                rev_id,
                workspace_id,
                page_id,
                title,
                content_md,
                content_sha,
                change_summary,
                caller.uid,
                actor_display,
                now_ms,
            ],
        )?;

        let action_name = match input.entry_mode {
            PageEntryMode::New => "page_created",
            PageEntryMode::CopyPersonal => "page_published_copy",
        };
        let detail = match input.entry_mode {
            PageEntryMode::New => format!("Created shared page “{title}”"),
            PageEntryMode::CopyPersonal => format!("Published a reviewed copy as “{title}”"),
        };
        insert_activity(
            &tx,
            workspace_id,
            &caller.uid,
            &actor_display,
            action_name,
            "page",
            Some(&page_id),
            Some(&title),
            None,
            None,
            &detail,
            now_ms,
        )?;

        let details = load_page_details(&tx, workspace_id, &page_id)?;
        save_idempotent_response(
            &tx,
            &caller.uid,
            workspace_id,
            "create_page",
            &idempotency_key,
            &payload_hash,
            &details,
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(CreateSharedPageOutcome {
            created: true,
            page: details,
        })
    }

    fn get_page_details(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        page_id: &str,
    ) -> Result<SharedPageDetails, SharedContentError> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let _member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let details = load_page_details(&tx, workspace_id, page_id)?;
        tx.commit()?;
        Ok(details)
    }

    fn list_revisions(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        page_id: &str,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<PagedList<SharedPageRevisionView>, SharedContentError> {
        let limit = clamp_limit(limit);
        let cursor_rev = parse_i64_cursor(cursor)?;
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let _member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let _page = load_page_row(&tx, workspace_id, page_id)?;
        let rows = query_revisions(&tx, workspace_id, page_id, cursor_rev, limit + 1)?;
        let (items, next_cursor) = paginate(rows, limit, |r| r.revision_number.to_string());
        tx.commit()?;
        Ok(PagedList { items, next_cursor })
    }

    fn archive_page(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedPageDetails, SharedContentError> {
        self.set_page_archived(
            caller,
            recheck_session,
            workspace_id,
            page_id,
            true,
            idempotency_key,
            now_ms,
        )
    }

    fn restore_page(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedPageDetails, SharedContentError> {
        self.set_page_archived(
            caller,
            recheck_session,
            workspace_id,
            page_id,
            false,
            idempotency_key,
            now_ms,
        )
    }

    fn list_threads(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        page_id: &str,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<PagedList<SharedCommentThreadView>, SharedContentError> {
        let limit = clamp_limit(limit);
        let parsed = parse_time_id_cursor(cursor)?;
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let _member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let _page = load_page_row(&tx, workspace_id, page_id)?;
        let rows = query_threads(
            &tx,
            workspace_id,
            page_id,
            parsed.as_ref().map(|(ts, id)| (*ts, id.as_str())),
            limit + 1,
        )?;
        let (items, next_cursor) = paginate(rows, limit, |t| {
            format!("{}:{}", t.updated_at_ms, t.thread_id)
        });
        tx.commit()?;
        Ok(PagedList { items, next_cursor })
    }

    fn create_thread(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        input: &CreateThreadInput,
        now_ms: i64,
    ) -> Result<SharedCommentThreadView, SharedContentError> {
        let base_revision_number = input.base_revision_number;
        let body = sanitize_comment_body(&input.body)?;
        let idempotency_key = sanitize_idempotency_key(&input.idempotency_key)?;
        let op = format!("create_thread:{page_id}");
        let payload_hash = payload_digest(&serde_json::json!({
            "page_id": page_id,
            "base_revision_number": base_revision_number,
            "body": body,
        }));
        let actor_display = caller_display_name(caller);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_author_shared_content() {
            return Err(SharedContentError::Forbidden);
        }
        if let Some(cached) = lookup_idempotent_response::<SharedCommentThreadView>(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
        )? {
            tx.commit()?;
            return Ok(cached);
        }

        let page = load_page_row(&tx, workspace_id, page_id)?;
        if page.archived_at_ms.is_some() {
            return Err(SharedContentError::PageArchived);
        }
        ensure_revision_exists(&tx, workspace_id, page_id, base_revision_number)?;

        let thread_id = format!("thr_{}", Uuid::new_v4().simple());
        let msg_id = format!("msg_{}", Uuid::new_v4().simple());
        tx.execute(
            "INSERT INTO shared_comment_threads (
                thread_id, workspace_id, page_id, base_revision_number,
                author_uid, author_display_name, status, created_at_ms, updated_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', ?7, ?7)",
            params![
                thread_id,
                workspace_id,
                page_id,
                base_revision_number,
                caller.uid,
                actor_display,
                now_ms,
            ],
        )?;
        tx.execute(
            "INSERT INTO shared_comment_messages (
                message_id, workspace_id, page_id, thread_id, author_uid, author_display_name, body, created_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                msg_id,
                workspace_id,
                page_id,
                thread_id,
                caller.uid,
                actor_display,
                body,
                now_ms,
            ],
        )?;
        insert_activity(
            &tx,
            workspace_id,
            &caller.uid,
            &actor_display,
            "comment_started",
            "thread",
            Some(page_id),
            Some(&page.title),
            None,
            Some(&thread_id),
            &format!(
                "Started a comment thread on “{}” (rev {})",
                page.title, base_revision_number
            ),
            now_ms,
        )?;
        let thread = load_thread_by_id(&tx, workspace_id, page_id, &thread_id)?;
        save_idempotent_response(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
            &thread,
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(thread)
    }

    fn reply_thread(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        thread_id: &str,
        input: &ReplyThreadInput,
        now_ms: i64,
    ) -> Result<SharedCommentThreadView, SharedContentError> {
        let body = sanitize_comment_body(&input.body)?;
        let idempotency_key = sanitize_idempotency_key(&input.idempotency_key)?;
        let op = format!("reply_thread:{page_id}:{thread_id}");
        let payload_hash = payload_digest(&serde_json::json!({
            "page_id": page_id,
            "thread_id": thread_id,
            "body": body,
        }));
        let actor_display = caller_display_name(caller);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_author_shared_content() {
            return Err(SharedContentError::Forbidden);
        }
        if let Some(cached) = lookup_idempotent_response::<SharedCommentThreadView>(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
        )? {
            tx.commit()?;
            return Ok(cached);
        }

        let page = load_page_row(&tx, workspace_id, page_id)?;
        if page.archived_at_ms.is_some() {
            return Err(SharedContentError::PageArchived);
        }
        let _existing = load_thread_by_id(&tx, workspace_id, page_id, thread_id)?;
        let msg_id = format!("msg_{}", Uuid::new_v4().simple());
        tx.execute(
            "INSERT INTO shared_comment_messages (
                message_id, workspace_id, page_id, thread_id, author_uid, author_display_name, body, created_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                msg_id,
                workspace_id,
                page_id,
                thread_id,
                caller.uid,
                actor_display,
                body,
                now_ms,
            ],
        )?;
        tx.execute(
            "UPDATE shared_comment_threads SET updated_at_ms = ?1 WHERE workspace_id = ?2 AND thread_id = ?3",
            params![now_ms, workspace_id, thread_id],
        )?;
        insert_activity(
            &tx,
            workspace_id,
            &caller.uid,
            &actor_display,
            "comment_replied",
            "thread",
            Some(page_id),
            Some(&page.title),
            None,
            Some(thread_id),
            &format!("Replied in comment thread on “{}”", page.title),
            now_ms,
        )?;
        let thread = load_thread_by_id(&tx, workspace_id, page_id, thread_id)?;
        save_idempotent_response(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
            &thread,
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(thread)
    }

    fn resolve_thread(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        thread_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedCommentThreadView, SharedContentError> {
        self.set_thread_resolved(
            caller,
            recheck_session,
            workspace_id,
            page_id,
            thread_id,
            true,
            idempotency_key,
            now_ms,
        )
    }

    fn reopen_thread(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        thread_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedCommentThreadView, SharedContentError> {
        self.set_thread_resolved(
            caller,
            recheck_session,
            workspace_id,
            page_id,
            thread_id,
            false,
            idempotency_key,
            now_ms,
        )
    }

    fn list_proposals(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        page_id: &str,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<PagedList<SharedProposalSummary>, SharedContentError> {
        let limit = clamp_limit(limit);
        let parsed = parse_time_id_cursor(cursor)?;
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let _member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let _page = load_page_row(&tx, workspace_id, page_id)?;
        let rows = query_proposal_summaries(
            &tx,
            workspace_id,
            page_id,
            parsed.as_ref().map(|(ts, id)| (*ts, id.as_str())),
            limit + 1,
        )?;
        let (items, next_cursor) = paginate(rows, limit, |p| {
            format!("{}:{}", p.updated_at_ms, p.proposal_id)
        });
        tx.commit()?;
        Ok(PagedList { items, next_cursor })
    }

    fn get_proposal_details(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        proposal_id: &str,
    ) -> Result<SharedProposalDetails, SharedContentError> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let details =
            load_proposal_details(&tx, workspace_id, proposal_id, &member.uid, member.role)?;
        tx.commit()?;
        Ok(details)
    }

    fn create_proposal(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        input: &CreateProposalInput,
        now_ms: i64,
    ) -> Result<SharedProposalDetails, SharedContentError> {
        let proposed_title = sanitize_page_title(&input.proposed_title)?;
        let proposed_content_md = sanitize_page_content(&input.proposed_content_md)?;
        let summary = sanitize_proposal_summary(&input.summary)?;
        let idempotency_key = sanitize_idempotency_key(&input.idempotency_key)?;
        let op = format!("create_proposal:{page_id}");
        let payload_hash = payload_digest(&serde_json::json!({
            "page_id": page_id,
            "base_revision_number": input.base_revision_number,
            "proposed_title": proposed_title,
            "proposed_content_md": proposed_content_md,
            "summary": summary,
        }));
        let actor_display = caller_display_name(caller);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_author_shared_content() {
            return Err(SharedContentError::Forbidden);
        }
        if let Some(cached) = lookup_idempotent_response::<SharedProposalDetails>(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
        )? {
            tx.commit()?;
            return Ok(cached);
        }

        let page = load_page_row(&tx, workspace_id, page_id)?;
        if page.archived_at_ms.is_some() {
            return Err(SharedContentError::PageArchived);
        }
        let base_rev = load_revision_row(&tx, workspace_id, page_id, input.base_revision_number)?;
        let proposal_id = format!("prp_{}", Uuid::new_v4().simple());
        let decision_id = format!("dec_{}", Uuid::new_v4().simple());

        tx.execute(
            "INSERT INTO shared_change_proposals (
                proposal_id, workspace_id, page_id, base_revision_number,
                base_title, base_content_md, proposed_title, proposed_content_md,
                summary, author_uid, author_display_name, status, submission_count,
                created_at_ms, updated_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'pending', 1, ?12, ?12)",
            params![
                proposal_id,
                workspace_id,
                page_id,
                base_rev.revision_number,
                base_rev.title,
                base_rev.content_md,
                proposed_title,
                proposed_content_md,
                summary,
                caller.uid,
                actor_display,
                now_ms,
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
                workspace_id,
                proposal_id,
                caller.uid,
                actor_display,
                summary,
                base_rev.revision_number,
                proposed_title,
                proposed_content_md,
                now_ms,
            ],
        )?;
        insert_activity(
            &tx,
            workspace_id,
            &caller.uid,
            &actor_display,
            "proposal_submitted",
            "proposal",
            Some(page_id),
            Some(&page.title),
            Some(&proposal_id),
            None,
            &format!("Proposed changes to “{}”: {}", page.title, summary),
            now_ms,
        )?;
        let details =
            load_proposal_details(&tx, workspace_id, &proposal_id, &caller.uid, member.role)?;
        save_idempotent_response(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
            &details,
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(details)
    }

    fn resubmit_proposal(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        proposal_id: &str,
        input: &CreateProposalInput,
        now_ms: i64,
    ) -> Result<SharedProposalDetails, SharedContentError> {
        let proposed_title = sanitize_page_title(&input.proposed_title)?;
        let proposed_content_md = sanitize_page_content(&input.proposed_content_md)?;
        let summary = sanitize_proposal_summary(&input.summary)?;
        let idempotency_key = sanitize_idempotency_key(&input.idempotency_key)?;
        let op = format!("resubmit_proposal:{proposal_id}");
        let payload_hash = payload_digest(&serde_json::json!({
            "proposal_id": proposal_id,
            "base_revision_number": input.base_revision_number,
            "proposed_title": proposed_title,
            "proposed_content_md": proposed_content_md,
            "summary": summary,
        }));
        let actor_display = caller_display_name(caller);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_author_shared_content() {
            return Err(SharedContentError::Forbidden);
        }
        let existing = load_proposal_row(&tx, workspace_id, proposal_id)?;
        if existing.author_uid != caller.uid {
            return Err(SharedContentError::Forbidden);
        }
        if let Some(cached) = lookup_idempotent_response::<SharedProposalDetails>(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
        )? {
            tx.commit()?;
            return Ok(cached);
        }
        if !matches!(
            existing.status,
            ProposalStatus::Pending | ProposalStatus::ChangesRequested
        ) {
            return Err(SharedContentError::ProposalNotPending);
        }
        let page = load_page_row(&tx, workspace_id, &existing.page_id)?;
        if page.archived_at_ms.is_some() {
            return Err(SharedContentError::PageArchived);
        }
        let base_rev = load_revision_row(
            &tx,
            workspace_id,
            &existing.page_id,
            input.base_revision_number,
        )?;
        let next_submission = existing.submission_count + 1;
        let decision_id = format!("dec_{}", Uuid::new_v4().simple());

        tx.execute(
            "UPDATE shared_change_proposals
             SET base_revision_number = ?1, base_title = ?2, base_content_md = ?3,
                 proposed_title = ?4, proposed_content_md = ?5, summary = ?6,
                 status = 'pending', submission_count = ?7, updated_at_ms = ?8,
                 reviewed_at_ms = NULL, reviewer_uid = NULL, reviewer_display_name = NULL, reviewer_note = NULL
             WHERE workspace_id = ?9 AND proposal_id = ?10",
            params![
                base_rev.revision_number,
                base_rev.title,
                base_rev.content_md,
                proposed_title,
                proposed_content_md,
                summary,
                next_submission,
                now_ms,
                workspace_id,
                proposal_id,
            ],
        )?;
        tx.execute(
            "INSERT INTO shared_proposal_decisions (
                decision_id, workspace_id, proposal_id, submission_number, action,
                actor_uid, actor_display_name, note, base_revision_number,
                submitted_title, submitted_content_md, created_at_ms
            ) VALUES (?1, ?2, ?3, ?4, 'resubmitted', ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                decision_id,
                workspace_id,
                proposal_id,
                next_submission,
                caller.uid,
                actor_display,
                summary,
                base_rev.revision_number,
                proposed_title,
                proposed_content_md,
                now_ms,
            ],
        )?;
        insert_activity(
            &tx,
            workspace_id,
            &caller.uid,
            &actor_display,
            "proposal_resubmitted",
            "proposal",
            Some(&page.page_id),
            Some(&page.title),
            Some(proposal_id),
            None,
            &format!(
                "Resubmitted proposal #{} for “{}” (rev {})",
                next_submission, page.title, base_rev.revision_number
            ),
            now_ms,
        )?;
        let details =
            load_proposal_details(&tx, workspace_id, proposal_id, &caller.uid, member.role)?;
        save_idempotent_response(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
            &details,
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(details)
    }

    fn withdraw_proposal(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        proposal_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedProposalDetails, SharedContentError> {
        let idempotency_key = sanitize_idempotency_key(idempotency_key)?;
        let op = format!("withdraw_proposal:{proposal_id}");
        let payload_hash = payload_digest(&serde_json::json!({ "proposal_id": proposal_id }));
        let actor_display = caller_display_name(caller);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_author_shared_content() {
            return Err(SharedContentError::Forbidden);
        }
        let existing = load_proposal_row(&tx, workspace_id, proposal_id)?;
        if existing.author_uid != caller.uid {
            return Err(SharedContentError::Forbidden);
        }
        if let Some(cached) = lookup_idempotent_response::<SharedProposalDetails>(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
        )? {
            tx.commit()?;
            return Ok(cached);
        }
        if !matches!(
            existing.status,
            ProposalStatus::Pending | ProposalStatus::ChangesRequested
        ) {
            return Err(SharedContentError::ProposalNotPending);
        }
        let page = load_page_row(&tx, workspace_id, &existing.page_id)?;
        let decision_id = format!("dec_{}", Uuid::new_v4().simple());

        tx.execute(
            "UPDATE shared_change_proposals
             SET status = 'withdrawn', updated_at_ms = ?1
             WHERE workspace_id = ?2 AND proposal_id = ?3",
            params![now_ms, workspace_id, proposal_id],
        )?;
        tx.execute(
            "INSERT INTO shared_proposal_decisions (
                decision_id, workspace_id, proposal_id, submission_number, action,
                actor_uid, actor_display_name, note, base_revision_number,
                submitted_title, submitted_content_md, created_at_ms
            ) VALUES (?1, ?2, ?3, ?4, 'withdrawn', ?5, ?6, NULL, ?7, ?8, ?9, ?10)",
            params![
                decision_id,
                workspace_id,
                proposal_id,
                existing.submission_count,
                caller.uid,
                actor_display,
                existing.base_revision_number,
                existing.proposed_title,
                existing.proposed_content_md,
                now_ms,
            ],
        )?;
        insert_activity(
            &tx,
            workspace_id,
            &caller.uid,
            &actor_display,
            "proposal_withdrawn",
            "proposal",
            Some(&page.page_id),
            Some(&page.title),
            Some(proposal_id),
            None,
            &format!("Withdrew proposal on “{}”", page.title),
            now_ms,
        )?;
        let details =
            load_proposal_details(&tx, workspace_id, proposal_id, &caller.uid, member.role)?;
        save_idempotent_response(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
            &details,
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(details)
    }

    fn review_proposal(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        proposal_id: &str,
        input: &ReviewProposalInput,
        now_ms: i64,
    ) -> Result<SharedProposalDetails, SharedContentError> {
        if input.expected_submission_number <= 0 {
            return Err(SharedContentError::InvalidInput(
                "Expected submission number must be at least 1.".into(),
            ));
        }
        let note = sanitize_optional_note(input.note.as_deref())?;
        let idempotency_key = sanitize_idempotency_key(&input.idempotency_key)?;
        let op = format!("review_proposal:{proposal_id}");
        let payload_hash = payload_digest(&serde_json::json!({
            "proposal_id": proposal_id,
            "action": input.action.as_str(),
            "expected_current_revision": input.expected_current_revision,
            "expected_submission_number": input.expected_submission_number,
            "note": note,
        }));
        let actor_display = caller_display_name(caller);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_moderate_or_review() {
            return Err(SharedContentError::Forbidden);
        }
        let existing = load_proposal_row(&tx, workspace_id, proposal_id)?;
        if existing.author_uid == caller.uid {
            return Err(SharedContentError::SelfReviewForbidden);
        }
        if let Some(cached) = lookup_idempotent_response::<SharedProposalDetails>(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
        )? {
            tx.commit()?;
            return Ok(cached);
        }

        let page = load_page_row(&tx, workspace_id, &existing.page_id)?;
        if page.archived_at_ms.is_some() {
            return Err(SharedContentError::PageArchived);
        }
        if existing.submission_count != input.expected_submission_number {
            return Err(SharedContentError::StaleSubmission(format!(
                "Submission #{} was replaced by submission #{}. Refresh the review to inspect the replacement before deciding.",
                input.expected_submission_number, existing.submission_count
            )));
        }
        if existing.status != ProposalStatus::Pending {
            return Err(SharedContentError::ProposalNotPending);
        }

        let decision_id = format!("dec_{}", Uuid::new_v4().simple());
        match input.action {
            ProposalReviewAction::Apply => {
                if existing.base_revision_number != page.current_revision_number
                    || input.expected_current_revision != page.current_revision_number
                {
                    return Err(SharedContentError::StaleProposal(format!(
                        "The page advanced to revision {} while this proposal was based on revision {}. Rebase before applying.",
                        page.current_revision_number, existing.base_revision_number
                    )));
                }
                let next_rev_num = page.current_revision_number + 1;
                let rev_id = format!("rev_{}", Uuid::new_v4().simple());
                let content_sha = sha256_hex(&existing.proposed_content_md);

                tx.execute(
                    "INSERT INTO shared_page_revisions (
                        revision_id, workspace_id, page_id, revision_number, title, content_md,
                        content_sha256, change_summary, author_uid, author_display_name,
                        reviewer_uid, reviewer_display_name, proposal_id, created_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                    params![
                        rev_id,
                        workspace_id,
                        page.page_id,
                        next_rev_num,
                        existing.proposed_title,
                        existing.proposed_content_md,
                        content_sha,
                        existing.summary,
                        existing.author_uid,
                        existing.author_display_name,
                        caller.uid,
                        actor_display,
                        proposal_id,
                        now_ms,
                    ],
                )?;

                tx.execute(
                    "UPDATE shared_pages
                     SET title = ?1, content_md = ?2, current_revision_number = ?3,
                         last_editor_uid = ?4, last_editor_display_name = ?5,
                         last_reviewer_uid = ?6, last_reviewer_display_name = ?7,
                         updated_at_ms = ?8
                     WHERE workspace_id = ?9 AND page_id = ?10",
                    params![
                        existing.proposed_title,
                        existing.proposed_content_md,
                        next_rev_num,
                        existing.author_uid,
                        existing.author_display_name,
                        caller.uid,
                        actor_display,
                        now_ms,
                        workspace_id,
                        page.page_id,
                    ],
                )?;

                tx.execute(
                    "UPDATE shared_change_proposals
                     SET status = 'applied', updated_at_ms = ?1, reviewed_at_ms = ?1,
                         reviewer_uid = ?2, reviewer_display_name = ?3, reviewer_note = ?4,
                         applied_revision_number = ?5
                     WHERE workspace_id = ?6 AND proposal_id = ?7",
                    params![
                        now_ms,
                        caller.uid,
                        actor_display,
                        note,
                        next_rev_num,
                        workspace_id,
                        proposal_id,
                    ],
                )?;

                tx.execute(
                    "INSERT INTO shared_proposal_decisions (
                        decision_id, workspace_id, proposal_id, submission_number, action,
                        actor_uid, actor_display_name, note, base_revision_number,
                        submitted_title, submitted_content_md, resulting_revision_number, created_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, 'applied', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        decision_id,
                        workspace_id,
                        proposal_id,
                        existing.submission_count,
                        caller.uid,
                        actor_display,
                        note,
                        existing.base_revision_number,
                        existing.proposed_title,
                        existing.proposed_content_md,
                        next_rev_num,
                        now_ms,
                    ],
                )?;

                insert_activity(
                    &tx,
                    workspace_id,
                    &caller.uid,
                    &actor_display,
                    "proposal_applied",
                    "proposal",
                    Some(&page.page_id),
                    Some(&existing.proposed_title),
                    Some(proposal_id),
                    None,
                    &format!(
                        "Applied proposal from {} to “{}” (rev {})",
                        existing.author_display_name, existing.proposed_title, next_rev_num
                    ),
                    now_ms,
                )?;
            }
            ProposalReviewAction::RequestChanges | ProposalReviewAction::Reject => {
                let (next_status, decision_action, activity_action, label) = match input.action {
                    ProposalReviewAction::RequestChanges => (
                        ProposalStatus::ChangesRequested,
                        "changes_requested",
                        "proposal_changes_requested",
                        "Requested changes on proposal",
                    ),
                    ProposalReviewAction::Reject => (
                        ProposalStatus::Rejected,
                        "rejected",
                        "proposal_rejected",
                        "Rejected proposal",
                    ),
                    ProposalReviewAction::Apply => unreachable!(),
                };
                tx.execute(
                    "UPDATE shared_change_proposals
                     SET status = ?1, updated_at_ms = ?2, reviewed_at_ms = ?2,
                         reviewer_uid = ?3, reviewer_display_name = ?4, reviewer_note = ?5
                     WHERE workspace_id = ?6 AND proposal_id = ?7",
                    params![
                        next_status.as_str(),
                        now_ms,
                        caller.uid,
                        actor_display,
                        note,
                        workspace_id,
                        proposal_id,
                    ],
                )?;
                tx.execute(
                    "INSERT INTO shared_proposal_decisions (
                        decision_id, workspace_id, proposal_id, submission_number, action,
                        actor_uid, actor_display_name, note, base_revision_number,
                        submitted_title, submitted_content_md, created_at_ms
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        decision_id,
                        workspace_id,
                        proposal_id,
                        existing.submission_count,
                        decision_action,
                        caller.uid,
                        actor_display,
                        note,
                        existing.base_revision_number,
                        existing.proposed_title,
                        existing.proposed_content_md,
                        now_ms,
                    ],
                )?;
                insert_activity(
                    &tx,
                    workspace_id,
                    &caller.uid,
                    &actor_display,
                    activity_action,
                    "proposal",
                    Some(&page.page_id),
                    Some(&page.title),
                    Some(proposal_id),
                    None,
                    &format!("{label} for “{}”", page.title),
                    now_ms,
                )?;
            }
        }

        let details =
            load_proposal_details(&tx, workspace_id, proposal_id, &caller.uid, member.role)?;
        save_idempotent_response(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
            &details,
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(details)
    }

    fn list_activity(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<PagedList<SharedActivityItem>, SharedContentError> {
        let limit = clamp_limit(limit);
        let cursor_seq = parse_i64_cursor(cursor)?;
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let _member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let mut stmt = if cursor_seq.is_some() {
            tx.prepare(
                "SELECT seq, activity_id, workspace_id, actor_uid, actor_display_name,
                        action, target_type, project_id, project_name,
                        page_id, page_title, proposal_id, thread_id,
                        detail_text, created_at_ms
                 FROM shared_workspace_activity
                 WHERE workspace_id = ?1 AND seq < ?2
                 ORDER BY seq DESC
                 LIMIT ?3",
            )?
        } else {
            tx.prepare(
                "SELECT seq, activity_id, workspace_id, actor_uid, actor_display_name,
                        action, target_type, project_id, project_name,
                        page_id, page_title, proposal_id, thread_id,
                        detail_text, created_at_ms
                 FROM shared_workspace_activity
                 WHERE workspace_id = ?1
                 ORDER BY seq DESC
                 LIMIT ?2",
            )?
        };
        let mut collected: Vec<(i64, SharedActivityItem)> = Vec::new();
        let map_row = |r: &rusqlite::Row<'_>| -> rusqlite::Result<(i64, SharedActivityItem)> {
            Ok((
                r.get::<_, i64>(0)?,
                SharedActivityItem {
                    activity_id: r.get(1)?,
                    workspace_id: r.get(2)?,
                    actor_uid: r.get(3)?,
                    actor_display_name: r.get(4)?,
                    action: r.get(5)?,
                    target_type: r.get(6)?,
                    project_id: r.get(7)?,
                    project_name: r.get(8)?,
                    page_id: r.get(9)?,
                    page_title: r.get(10)?,
                    proposal_id: r.get(11)?,
                    thread_id: r.get(12)?,
                    detail_text: r.get(13)?,
                    created_at_ms: r.get(14)?,
                },
            ))
        };
        if let Some(seq) = cursor_seq {
            let rows = stmt.query_map(params![workspace_id, seq, (limit + 1) as i64], map_row)?;
            for row in rows {
                collected.push(row?);
            }
        } else {
            let rows = stmt.query_map(params![workspace_id, (limit + 1) as i64], map_row)?;
            for row in rows {
                collected.push(row?);
            }
        }
        drop(stmt);
        let (pairs, next_cursor) = paginate(collected, limit, |(seq, _)| seq.to_string());
        let items = pairs.into_iter().map(|(_, item)| item).collect();
        tx.commit()?;
        Ok(PagedList { items, next_cursor })
    }

    fn list_projects(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        include_archived: bool,
        search_query: Option<&str>,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<PagedList<SharedProjectSummary>, SharedContentError> {
        let limit = clamp_limit(limit);
        let q = sanitize_search_query(search_query)?;
        let parsed_cursor = parse_time_id_cursor(cursor)?;
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let _member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let items_plus_one = query_project_summaries(
            &tx,
            workspace_id,
            include_archived,
            q.as_deref(),
            parsed_cursor.as_ref().map(|(ts, id)| (*ts, id.as_str())),
            limit + 1,
        )?;
        let (items, next_cursor) = paginate(items_plus_one, limit, |p| {
            format!("{}:{}", p.updated_at_ms, p.project_id)
        });
        tx.commit()?;
        Ok(PagedList { items, next_cursor })
    }

    fn create_project(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        input: &CreateSharedProjectInput,
        now_ms: i64,
    ) -> Result<CreateSharedProjectOutcome, SharedContentError> {
        let name = sanitize_project_name(&input.name)?;
        let purpose = sanitize_project_purpose(&input.purpose)?;
        let idempotency_key = sanitize_idempotency_key(&input.idempotency_key)?;
        let payload_hash = payload_digest(&serde_json::json!({
            "name": name,
            "purpose": purpose,
        }));
        let actor_display = caller_display_name(caller);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_author_shared_content() {
            return Err(SharedContentError::Forbidden);
        }

        if let Some(cached) = lookup_idempotent_response::<SharedProjectDetails>(
            &tx,
            &caller.uid,
            workspace_id,
            "create_project",
            &idempotency_key,
            &payload_hash,
        )? {
            let _ = recheck_session()?;
            tx.commit()?;
            return Ok(CreateSharedProjectOutcome {
                created: false,
                project: cached,
            });
        }

        let project_id = format!("shpprj_{}", Uuid::new_v4().simple());
        tx.execute(
            "INSERT INTO shared_projects (
                project_id, workspace_id, name, purpose, version,
                created_by_uid, created_by_display_name,
                updated_by_uid, updated_by_display_name,
                created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?5, ?6, ?7, ?7)",
            params![
                project_id,
                workspace_id,
                name,
                purpose,
                caller.uid,
                actor_display,
                now_ms
            ],
        )?;

        insert_activity_with_project(
            &tx,
            workspace_id,
            &caller.uid,
            &actor_display,
            "project_created",
            "project",
            Some(&project_id),
            Some(&name),
            None,
            None,
            None,
            None,
            &format!("Created project “{name}”"),
            now_ms,
        )?;

        let details = load_project_details(&tx, workspace_id, &project_id, None, None, None)?;
        save_idempotent_response(
            &tx,
            &caller.uid,
            workspace_id,
            "create_project",
            &idempotency_key,
            &payload_hash,
            &details,
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(CreateSharedProjectOutcome {
            created: true,
            project: details,
        })
    }

    fn get_project_details(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        project_id: &str,
        search_query: Option<&str>,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<SharedProjectDetails, SharedContentError> {
        let q = sanitize_search_query(search_query)?;
        let parsed_cursor = parse_time_id_cursor(cursor)?;
        let limit = clamp_limit(limit);
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let _member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let details = load_project_details(
            &tx,
            workspace_id,
            project_id,
            q.as_deref(),
            parsed_cursor.as_ref().map(|(ts, id)| (*ts, id.as_str())),
            Some(limit),
        )?;
        tx.commit()?;
        Ok(details)
    }

    fn update_project(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        project_id: &str,
        input: &UpdateSharedProjectInput,
        now_ms: i64,
    ) -> Result<SharedProjectDetails, SharedContentError> {
        if input.expected_version < 1 {
            return Err(SharedContentError::InvalidInput(
                "Expected project version must be at least 1.".into(),
            ));
        }
        let name = sanitize_project_name(&input.name)?;
        let purpose = sanitize_project_purpose(&input.purpose)?;
        let idempotency_key = sanitize_idempotency_key(&input.idempotency_key)?;
        let op = format!("update_project:{project_id}");
        let payload_hash = payload_digest(&serde_json::json!({
            "project_id": project_id,
            "name": name,
            "purpose": purpose,
            "expected_version": input.expected_version,
        }));
        let actor_display = caller_display_name(caller);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_manage_shared_project() {
            return Err(SharedContentError::Forbidden);
        }
        if let Some(cached) = lookup_idempotent_response::<SharedProjectDetails>(
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

        let existing = load_project_summary(&tx, workspace_id, project_id)?;
        if existing.archived_at_ms.is_some() {
            return Err(SharedContentError::ProjectArchived);
        }
        if existing.version != input.expected_version {
            return Err(SharedContentError::StaleProjectVersion(format!(
                "Project details changed to v{} while you were editing. Refresh before saving.",
                existing.version
            )));
        }

        tx.execute(
            "UPDATE shared_projects
             SET name = ?1, purpose = ?2, version = version + 1,
                 updated_by_uid = ?3, updated_by_display_name = ?4, updated_at_ms = ?5
             WHERE workspace_id = ?6 AND project_id = ?7 AND version = ?8 AND archived_at_ms IS NULL",
            params![
                name,
                purpose,
                caller.uid,
                actor_display,
                now_ms,
                workspace_id,
                project_id,
                input.expected_version,
            ],
        )?;

        insert_activity_with_project(
            &tx,
            workspace_id,
            &caller.uid,
            &actor_display,
            "project_updated",
            "project",
            Some(project_id),
            Some(&name),
            None,
            None,
            None,
            None,
            &format!("Updated project “{name}”"),
            now_ms,
        )?;

        let details = load_project_details(&tx, workspace_id, project_id, None, None, None)?;
        save_idempotent_response(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
            &details,
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(details)
    }

    fn archive_project(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        project_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedProjectDetails, SharedContentError> {
        self.set_project_archived(
            caller,
            recheck_session,
            workspace_id,
            project_id,
            true,
            idempotency_key,
            now_ms,
        )
    }

    fn restore_project(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        project_id: &str,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedProjectDetails, SharedContentError> {
        self.set_project_archived(
            caller,
            recheck_session,
            workspace_id,
            project_id,
            false,
            idempotency_key,
            now_ms,
        )
    }

    fn list_project_picker_pages(
        &self,
        caller: &VerifiedSession,
        workspace_id: &str,
        project_id: &str,
        search_query: Option<&str>,
        cursor: Option<&str>,
        limit: Option<usize>,
    ) -> Result<PagedList<SharedPageSummary>, SharedContentError> {
        let limit = clamp_limit(limit);
        let q = sanitize_search_query(search_query)?;
        let parsed_cursor = parse_time_id_cursor(cursor)?;
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let _member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        let _project = load_project_summary(&tx, workspace_id, project_id)?;
        let items_plus_one = query_picker_pages(
            &tx,
            workspace_id,
            project_id,
            q.as_deref(),
            parsed_cursor.as_ref().map(|(ts, id)| (*ts, id.as_str())),
            limit + 1,
        )?;
        let (items, next_cursor) = paginate(items_plus_one, limit, |p| {
            format!("{}:{}", p.updated_at_ms, p.page_id)
        });
        tx.commit()?;
        Ok(PagedList { items, next_cursor })
    }

    fn attach_page_to_project(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        project_id: &str,
        input: &AttachProjectPageInput,
        now_ms: i64,
    ) -> Result<ProjectPageMembershipOutcome, SharedContentError> {
        let page_id = input.page_id.trim();
        if page_id.is_empty() {
            return Err(SharedContentError::InvalidInput(
                "Shared page ID is required.".into(),
            ));
        }
        let idempotency_key = sanitize_idempotency_key(&input.idempotency_key)?;
        let op = format!("attach_project_page:{project_id}:{page_id}");
        let payload_hash = payload_digest(&serde_json::json!({
            "project_id": project_id,
            "page_id": page_id,
        }));
        let actor_display = caller_display_name(caller);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_author_shared_content() {
            return Err(SharedContentError::Forbidden);
        }
        if let Some(mut cached) = lookup_idempotent_response::<ProjectPageMembershipOutcome>(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
        )? {
            let _ = recheck_session()?;
            tx.commit()?;
            cached.attached = false;
            return Ok(cached);
        }

        let project = load_project_summary(&tx, workspace_id, project_id)?;
        if project.archived_at_ms.is_some() {
            return Err(SharedContentError::ProjectArchived);
        }
        let page = load_page_row(&tx, workspace_id, page_id)?;
        if page.archived_at_ms.is_some() {
            return Err(SharedContentError::PageArchived);
        }

        let already_active: Option<String> = tx
            .query_row(
                "SELECT membership_id FROM shared_project_pages
                 WHERE workspace_id = ?1 AND project_id = ?2 AND page_id = ?3 AND detached_at_ms IS NULL",
                params![workspace_id, project_id, page_id],
                |r| r.get(0),
            )
            .optional()?;
        if already_active.is_some() {
            return Err(SharedContentError::AlreadyProjectMember);
        }

        let max_gen: i64 = tx.query_row(
            "SELECT COALESCE(MAX(generation), 0)
             FROM shared_project_pages
             WHERE workspace_id = ?1 AND project_id = ?2 AND page_id = ?3",
            params![workspace_id, project_id, page_id],
            |r| r.get(0),
        )?;
        let generation = max_gen + 1;
        let membership_id = format!("prjmem_{}", Uuid::new_v4().simple());

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
                generation,
                caller.uid,
                actor_display,
                now_ms,
            ],
        )?;

        tx.execute(
            "UPDATE shared_projects SET updated_at_ms = ?1
             WHERE workspace_id = ?2 AND project_id = ?3",
            params![now_ms, workspace_id, project_id],
        )?;

        insert_activity_with_project(
            &tx,
            workspace_id,
            &caller.uid,
            &actor_display,
            "project_page_attached",
            "project_page",
            Some(project_id),
            Some(&project.name),
            Some(page_id),
            Some(&page.title),
            None,
            None,
            &format!("Added “{}” to project “{}”", page.title, project.name),
            now_ms,
        )?;

        let details = load_project_details(&tx, workspace_id, project_id, None, None, None)?;
        let outcome = ProjectPageMembershipOutcome {
            attached: true,
            membership_id,
            generation,
            project: details,
        };
        save_idempotent_response(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
            &outcome,
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(outcome)
    }

    fn detach_page_from_project(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        project_id: &str,
        page_id: &str,
        input: &DetachProjectPageInput,
        now_ms: i64,
    ) -> Result<SharedProjectDetails, SharedContentError> {
        let membership_id = input.membership_id.trim();
        if membership_id.is_empty() {
            return Err(SharedContentError::InvalidInput(
                "Project membership ID is required.".into(),
            ));
        }
        let idempotency_key = sanitize_idempotency_key(&input.idempotency_key)?;
        let op = format!("detach_project_page:{project_id}:{page_id}:{membership_id}");
        let payload_hash = payload_digest(&serde_json::json!({
            "project_id": project_id,
            "page_id": page_id,
            "membership_id": membership_id,
        }));
        let actor_display = caller_display_name(caller);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_author_shared_content() {
            return Err(SharedContentError::Forbidden);
        }
        if let Some(cached) = lookup_idempotent_response::<SharedProjectDetails>(
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

        let project = load_project_summary(&tx, workspace_id, project_id)?;
        if project.archived_at_ms.is_some() {
            return Err(SharedContentError::ProjectArchived);
        }
        let page = load_page_row(&tx, workspace_id, page_id)?;

        let affected = tx.execute(
            "UPDATE shared_project_pages
             SET detached_at_ms = ?1, detached_by_uid = ?2, detached_by_display_name = ?3
             WHERE workspace_id = ?4 AND project_id = ?5 AND page_id = ?6
               AND membership_id = ?7 AND detached_at_ms IS NULL",
            params![
                now_ms,
                caller.uid,
                actor_display,
                workspace_id,
                project_id,
                page_id,
                membership_id,
            ],
        )?;
        if affected == 0 {
            return Err(SharedContentError::StaleMembership(
                "That project-page membership has already changed. Refresh the project before retrying."
                    .into(),
            ));
        }

        tx.execute(
            "UPDATE shared_projects SET updated_at_ms = ?1
             WHERE workspace_id = ?2 AND project_id = ?3",
            params![now_ms, workspace_id, project_id],
        )?;

        insert_activity_with_project(
            &tx,
            workspace_id,
            &caller.uid,
            &actor_display,
            "project_page_detached",
            "project_page",
            Some(project_id),
            Some(&project.name),
            Some(page_id),
            Some(&page.title),
            None,
            None,
            &format!("Removed “{}” from project “{}”", page.title, project.name),
            now_ms,
        )?;

        let details = load_project_details(&tx, workspace_id, project_id, None, None, None)?;
        save_idempotent_response(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
            &details,
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(details)
    }
}

impl SqliteSharedContentStore {
    #[allow(clippy::too_many_arguments)]
    fn set_project_archived(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        project_id: &str,
        archive: bool,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedProjectDetails, SharedContentError> {
        let idempotency_key = sanitize_idempotency_key(idempotency_key)?;
        let verb = if archive {
            "archive_project"
        } else {
            "restore_project"
        };
        let op = format!("{verb}:{project_id}");
        let payload_hash =
            payload_digest(&serde_json::json!({ "project_id": project_id, "archive": archive }));
        let actor_display = caller_display_name(caller);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_manage_shared_project() {
            return Err(SharedContentError::Forbidden);
        }
        if let Some(cached) = lookup_idempotent_response::<SharedProjectDetails>(
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

        let project = load_project_summary(&tx, workspace_id, project_id)?;
        if archive && project.archived_at_ms.is_some() {
            return Err(SharedContentError::ProjectAlreadyArchived);
        }
        if !archive && project.archived_at_ms.is_none() {
            return Err(SharedContentError::ProjectNotArchived);
        }
        if archive {
            tx.execute(
                "UPDATE shared_projects
                 SET archived_at_ms = ?1, archived_by_uid = ?2, archived_by_display_name = ?3,
                     updated_by_uid = ?2, updated_by_display_name = ?3, updated_at_ms = ?1
                 WHERE workspace_id = ?4 AND project_id = ?5",
                params![now_ms, caller.uid, actor_display, workspace_id, project_id],
            )?;
            insert_activity_with_project(
                &tx,
                workspace_id,
                &caller.uid,
                &actor_display,
                "project_archived",
                "project",
                Some(project_id),
                Some(&project.name),
                None,
                None,
                None,
                None,
                &format!("Archived project “{}”", project.name),
                now_ms,
            )?;
        } else {
            tx.execute(
                "UPDATE shared_projects
                 SET archived_at_ms = NULL, archived_by_uid = NULL, archived_by_display_name = NULL,
                     updated_by_uid = ?1, updated_by_display_name = ?2, updated_at_ms = ?3
                 WHERE workspace_id = ?4 AND project_id = ?5",
                params![caller.uid, actor_display, now_ms, workspace_id, project_id],
            )?;
            insert_activity_with_project(
                &tx,
                workspace_id,
                &caller.uid,
                &actor_display,
                "project_restored",
                "project",
                Some(project_id),
                Some(&project.name),
                None,
                None,
                None,
                None,
                &format!("Restored project “{}”", project.name),
                now_ms,
            )?;
        }

        let details = load_project_details(&tx, workspace_id, project_id, None, None, None)?;
        save_idempotent_response(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
            &details,
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(details)
    }

    #[allow(clippy::too_many_arguments)]
    fn set_thread_resolved(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        thread_id: &str,
        resolved: bool,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedCommentThreadView, SharedContentError> {
        let idempotency_key = sanitize_idempotency_key(idempotency_key)?;
        let verb = if resolved {
            "resolve_thread"
        } else {
            "reopen_thread"
        };
        let op = format!("{verb}:{page_id}:{thread_id}");
        let payload_hash = payload_digest(&serde_json::json!({
            "page_id": page_id,
            "thread_id": thread_id,
            "resolved": resolved,
        }));
        let actor_display = caller_display_name(caller);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_author_shared_content() {
            return Err(SharedContentError::Forbidden);
        }
        let thread = load_thread_by_id(&tx, workspace_id, page_id, thread_id)?;
        if !member.role.can_moderate_or_review() && thread.author_uid != caller.uid {
            return Err(SharedContentError::Forbidden);
        }
        if let Some(cached) = lookup_idempotent_response::<SharedCommentThreadView>(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
        )? {
            tx.commit()?;
            return Ok(cached);
        }

        let page = load_page_row(&tx, workspace_id, page_id)?;
        if resolved {
            tx.execute(
                "UPDATE shared_comment_threads
                 SET status = 'resolved', resolved_at_ms = ?1, resolved_by_uid = ?2,
                     resolved_by_display_name = ?3, updated_at_ms = ?1
                 WHERE workspace_id = ?4 AND page_id = ?5 AND thread_id = ?6",
                params![
                    now_ms,
                    caller.uid,
                    actor_display,
                    workspace_id,
                    page_id,
                    thread_id
                ],
            )?;
        } else {
            tx.execute(
                "UPDATE shared_comment_threads
                 SET status = 'open', resolved_at_ms = NULL, resolved_by_uid = NULL,
                     resolved_by_display_name = NULL, updated_at_ms = ?1
                 WHERE workspace_id = ?2 AND page_id = ?3 AND thread_id = ?4",
                params![now_ms, workspace_id, page_id, thread_id],
            )?;
        }
        let moderated = thread.author_uid != caller.uid;
        let action_name = match (resolved, moderated) {
            (true, true) => "thread_moderated_resolved",
            (true, false) => "thread_resolved",
            (false, true) => "thread_moderated_reopened",
            (false, false) => "thread_reopened",
        };
        let detail = if resolved {
            format!("Resolved thread on “{}”", page.title)
        } else {
            format!("Reopened thread on “{}”", page.title)
        };
        insert_activity(
            &tx,
            workspace_id,
            &caller.uid,
            &actor_display,
            action_name,
            "thread",
            Some(page_id),
            Some(&page.title),
            None,
            Some(thread_id),
            &detail,
            now_ms,
        )?;
        let updated = load_thread_by_id(&tx, workspace_id, page_id, thread_id)?;
        save_idempotent_response(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
            &updated,
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(updated)
    }

    #[allow(clippy::too_many_arguments)]
    fn set_page_archived(
        &self,
        caller: &VerifiedSession,
        recheck_session: SessionRecheck<'_>,
        workspace_id: &str,
        page_id: &str,
        archive: bool,
        idempotency_key: &str,
        now_ms: i64,
    ) -> Result<SharedPageDetails, SharedContentError> {
        let idempotency_key = sanitize_idempotency_key(idempotency_key)?;
        let verb = if archive {
            "archive_page"
        } else {
            "restore_page"
        };
        let op = format!("{verb}:{page_id}");
        let payload_hash =
            payload_digest(&serde_json::json!({ "page_id": page_id, "archive": archive }));
        let actor_display = caller_display_name(caller);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let member = require_active_membership(&tx, workspace_id, &caller.uid)?;
        if !member.role.can_archive_shared_page() {
            return Err(SharedContentError::Forbidden);
        }
        if let Some(cached) = lookup_idempotent_response::<SharedPageDetails>(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
        )? {
            tx.commit()?;
            return Ok(cached);
        }

        let page = load_page_row(&tx, workspace_id, page_id)?;
        if archive && page.archived_at_ms.is_some() {
            return Err(SharedContentError::AlreadyArchived);
        }
        if !archive && page.archived_at_ms.is_none() {
            return Err(SharedContentError::NotArchived);
        }
        if archive {
            tx.execute(
                "UPDATE shared_pages
                 SET archived_at_ms = ?1, archived_by_uid = ?2, archived_by_display_name = ?3, updated_at_ms = ?1
                 WHERE workspace_id = ?4 AND page_id = ?5",
                params![now_ms, caller.uid, actor_display, workspace_id, page_id],
            )?;
            insert_activity(
                &tx,
                workspace_id,
                &caller.uid,
                &actor_display,
                "page_archived",
                "page",
                Some(page_id),
                Some(&page.title),
                None,
                None,
                &format!("Archived page “{}”", page.title),
                now_ms,
            )?;
        } else {
            tx.execute(
                "UPDATE shared_pages
                 SET archived_at_ms = NULL, archived_by_uid = NULL, archived_by_display_name = NULL, updated_at_ms = ?1
                 WHERE workspace_id = ?2 AND page_id = ?3",
                params![now_ms, workspace_id, page_id],
            )?;
            insert_activity(
                &tx,
                workspace_id,
                &caller.uid,
                &actor_display,
                "page_restored",
                "page",
                Some(page_id),
                Some(&page.title),
                None,
                None,
                &format!("Restored page “{}”", page.title),
                now_ms,
            )?;
        }
        let details = load_page_details(&tx, workspace_id, page_id)?;
        save_idempotent_response(
            &tx,
            &caller.uid,
            workspace_id,
            &op,
            &idempotency_key,
            &payload_hash,
            &details,
            now_ms,
        )?;
        let _ = recheck_session()?;
        tx.commit()?;
        Ok(details)
    }
}

struct ProposalRow {
    proposal_id: String,
    workspace_id: String,
    page_id: String,
    base_revision_number: i64,
    base_title: String,
    base_content_md: String,
    proposed_title: String,
    proposed_content_md: String,
    summary: String,
    author_uid: String,
    author_display_name: String,
    status: ProposalStatus,
    submission_count: i64,
    created_at_ms: i64,
    updated_at_ms: i64,
    reviewed_at_ms: Option<i64>,
    reviewer_display_name: Option<String>,
    applied_revision_number: Option<i64>,
}

fn clamp_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_PAGE_LIMIT).clamp(1, MAX_PAGE_LIMIT)
}

fn parse_i64_cursor(cursor: Option<&str>) -> Result<Option<i64>, SharedContentError> {
    let Some(raw) = cursor.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    raw.parse::<i64>()
        .map(Some)
        .map_err(|_| SharedContentError::InvalidCursor)
}

fn parse_time_id_cursor(cursor: Option<&str>) -> Result<Option<(i64, String)>, SharedContentError> {
    let Some(raw) = cursor.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let Some((ts_str, id_str)) = raw.split_once(':') else {
        return Err(SharedContentError::InvalidCursor);
    };
    let ts = ts_str
        .parse::<i64>()
        .map_err(|_| SharedContentError::InvalidCursor)?;
    if id_str.is_empty() || id_str.len() > 80 {
        return Err(SharedContentError::InvalidCursor);
    }
    Ok(Some((ts, id_str.to_owned())))
}

fn paginate<T>(
    mut items_plus_one: Vec<T>,
    limit: usize,
    cursor_fn: impl Fn(&T) -> String,
) -> (Vec<T>, Option<String>) {
    if items_plus_one.len() > limit {
        items_plus_one.truncate(limit);
        let next = items_plus_one.last().map(cursor_fn);
        (items_plus_one, next)
    } else {
        (items_plus_one, None)
    }
}

fn payload_digest(value: &serde_json::Value) -> String {
    sha256_hex(&value.to_string())
}

fn sanitize_page_title(raw: &str) -> Result<String, SharedContentError> {
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        return Err(SharedContentError::InvalidInput(
            "Page title is required.".into(),
        ));
    }
    if cleaned.chars().count() > MAX_PAGE_TITLE_LEN {
        return Err(SharedContentError::InvalidInput(format!(
            "Page title must be at most {MAX_PAGE_TITLE_LEN} characters."
        )));
    }
    Ok(cleaned)
}

fn sanitize_page_content(raw: &str) -> Result<String, SharedContentError> {
    let normalized = raw.replace("\r\n", "\n");
    if normalized.trim().is_empty() {
        return Err(SharedContentError::InvalidInput(
            "Page content cannot be empty.".into(),
        ));
    }
    if normalized.len() > MAX_PAGE_CONTENT_BYTES {
        return Err(SharedContentError::ContentTooLarge(format!(
            "Shared page content exceeds the {} KiB limit ({} bytes submitted). Shorten the page and retry.",
            MAX_PAGE_CONTENT_BYTES / 1024,
            normalized.len()
        )));
    }
    validate_shared_markdown(&normalized)?;
    Ok(normalized)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DestKind {
    Link,
    Image,
}

fn decode_basic_html_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '&' {
            if let Some(rel_semi) = chars[i + 1..].iter().take(12).position(|&c| c == ';') {
                let entity: String = chars[i + 1..i + 1 + rel_semi].iter().collect();
                let decoded = if let Some(hex) = entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                {
                    u32::from_str_radix(hex, 16)
                        .ok()
                        .and_then(char::from_u32)
                        .filter(|c| !c.is_control() || c.is_ascii_whitespace())
                } else if let Some(dec) = entity.strip_prefix('#') {
                    dec.parse::<u32>()
                        .ok()
                        .and_then(char::from_u32)
                        .filter(|c| !c.is_control() || c.is_ascii_whitespace())
                } else {
                    match entity.to_ascii_lowercase().as_str() {
                        "colon" => Some(':'),
                        "amp" => Some('&'),
                        "lt" => Some('<'),
                        "gt" => Some('>'),
                        "quot" => Some('"'),
                        "apos" => Some('\''),
                        "sol" => Some('/'),
                        "bsol" => Some('\\'),
                        "num" => Some('#'),
                        "tab" => Some('\t'),
                        "newline" => Some('\n'),
                        _ => None,
                    }
                };
                if let Some(ch) = decoded {
                    out.push(ch);
                    i += rel_semi + 2;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn validate_destination(destination: &str, kind: DestKind) -> Result<(), SharedContentError> {
    let trimmed = destination.trim();
    let compact: String = trimmed
        .chars()
        .filter(|c| !c.is_ascii_control() && !c.is_ascii_whitespace())
        .collect();
    let lower = compact.to_ascii_lowercase();

    if lower.starts_with("sthira:") || lower.starts_with("#/sthira") {
        return Err(SharedContentError::InvalidInput(
            "Personal vault links must be omitted before publishing to a shared page.".into(),
        ));
    }
    if kind == DestKind::Link
        && (trimmed.starts_with('#')
            || lower
                .strip_prefix("mailto:")
                .is_some_and(|address| !address.is_empty()))
    {
        return Ok(());
    }
    if url::Url::parse(trimmed)
        .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.has_host())
    {
        return Ok(());
    }
    Err(SharedContentError::InvalidInput(
        "Local file links, attachment paths, and unsupported URLs must be omitted before publishing to a shared page."
            .into(),
    ))
}

fn markdown_nodes(root: &markdown::mdast::Node) -> impl Iterator<Item = &markdown::mdast::Node> {
    let mut pending = vec![root];
    std::iter::from_fn(move || {
        let node = pending.pop()?;
        if let Some(children) = node.children() {
            pending.extend(children.iter().rev());
        }
        Some(node)
    })
}

fn validate_shared_markdown(md: &str) -> Result<(), SharedContentError> {
    use markdown::mdast::Node;
    use std::collections::HashMap;

    let tree = markdown::to_mdast(md, &markdown::ParseOptions::gfm()).map_err(|_| {
        SharedContentError::InvalidInput("Could not read shared page Markdown.".into())
    })?;

    // Check every definition, even if unused. Match CommonMark's first definition
    // when enforcing the narrower image policy on reference-style images.
    let mut definitions = HashMap::new();
    for node in markdown_nodes(&tree) {
        if let Node::Definition(def) = node {
            validate_destination(&def.url, DestKind::Link)?;
            definitions
                .entry(def.identifier.as_str())
                .or_insert(def.url.as_str());
        }
    }

    for node in markdown_nodes(&tree) {
        match node {
            Node::Link(link) => validate_destination(&link.url, DestKind::Link)?,
            Node::Image(image) => validate_destination(&image.url, DestKind::Image)?,
            Node::ImageReference(image) => {
                if let Some(destination) = definitions.get(image.identifier.as_str()) {
                    validate_destination(destination, DestKind::Image)?;
                }
            }
            Node::Html(_) => {
                return Err(SharedContentError::InvalidInput(
                    "Raw HTML is unsupported in shared pages. Use Markdown formatting.".into(),
                ));
            }
            Node::Text(text) => {
                let lower = text.value.to_ascii_lowercase();
                if lower.contains("sthira://") || lower.contains("#/sthira/") {
                    return Err(SharedContentError::InvalidInput(
                        "Private vault references must be omitted before publishing.".into(),
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn contains_disallowed_reference(raw: &str) -> bool {
    let decoded = decode_basic_html_entities(raw);
    let compact: String = decoded
        .chars()
        .filter(|c| !c.is_ascii_control() && !c.is_ascii_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    let lower = decoded.to_ascii_lowercase();
    compact.contains("sthira:")
        || compact.contains("#/sthira")
        || compact.contains("file:")
        || compact.contains("javascript:")
        || compact.contains("vbscript:")
        || compact.contains("data:")
        || compact.contains("blob:")
        || lower.contains("/home/")
        || lower.contains("/users/")
        || lower.contains("/tmp/")
        || lower.contains("~/")
        || lower.contains("c:\\")
        || lower.contains("\\\\")
}

fn sanitize_provenance(p: &ShareableProvenance) -> Result<ShareableProvenance, SharedContentError> {
    Ok(ShareableProvenance {
        origin_kind: p.origin_kind,
        original_author_label: sanitize_optional_label(
            p.original_author_label.as_deref(),
            "author attribution",
        )?,
        ai_model_label: sanitize_optional_label(p.ai_model_label.as_deref(), "model label")?,
        source_note: sanitize_optional_label(p.source_note.as_deref(), "source note")?,
    })
}

fn sanitize_optional_label(
    raw: Option<&str>,
    field_name: &str,
) -> Result<Option<String>, SharedContentError> {
    let Some(val) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if val.chars().count() > MAX_PROVENANCE_LABEL_LEN {
        return Err(SharedContentError::InvalidInput(format!(
            "Shareable {field_name} must be at most {MAX_PROVENANCE_LABEL_LEN} characters."
        )));
    }
    if contains_disallowed_reference(val) {
        return Err(SharedContentError::InvalidInput(format!(
            "Shareable {field_name} cannot include private file paths or vault URIs."
        )));
    }
    Ok(Some(val.to_owned()))
}

fn sanitize_omitted_summary(items: &[String]) -> Result<Vec<String>, SharedContentError> {
    let mut out = Vec::new();
    for item in items.iter().take(16) {
        let cleaned = item.trim();
        if !cleaned.is_empty() && cleaned.chars().count() <= 160 {
            if contains_disallowed_reference(cleaned) {
                return Err(SharedContentError::InvalidInput(
                    "Omission metadata cannot retain unsupported URLs, file paths, or vault URIs."
                        .into(),
                ));
            }
            out.push(cleaned.to_owned());
        }
    }
    Ok(out)
}

fn sanitize_comment_body(raw: &str) -> Result<String, SharedContentError> {
    let normalized = raw.replace("\r\n", "\n");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return Err(SharedContentError::InvalidInput(
            "Comment text is required.".into(),
        ));
    }
    if trimmed.len() > MAX_COMMENT_BODY_BYTES {
        return Err(SharedContentError::ContentTooLarge(format!(
            "Comment must be at most {} KiB.",
            MAX_COMMENT_BODY_BYTES / 1024
        )));
    }
    Ok(trimmed.to_owned())
}

fn sanitize_proposal_summary(raw: &str) -> Result<String, SharedContentError> {
    let cleaned = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        return Err(SharedContentError::InvalidInput(
            "Change summary is required.".into(),
        ));
    }
    if cleaned.chars().count() > MAX_PROPOSAL_SUMMARY_LEN {
        return Err(SharedContentError::InvalidInput(format!(
            "Change summary must be at most {MAX_PROPOSAL_SUMMARY_LEN} characters."
        )));
    }
    Ok(cleaned)
}

fn sanitize_optional_note(raw: Option<&str>) -> Result<Option<String>, SharedContentError> {
    let Some(val) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if val.chars().count() > MAX_DECISION_NOTE_LEN {
        return Err(SharedContentError::InvalidInput(format!(
            "Review note must be at most {MAX_DECISION_NOTE_LEN} characters."
        )));
    }
    Ok(Some(val.to_owned()))
}

fn lookup_idempotent_response<T: DeserializeOwned>(
    tx: &Transaction<'_>,
    actor_uid: &str,
    workspace_id: &str,
    operation: &str,
    idempotency_key: &str,
    payload_hash: &str,
) -> Result<Option<T>, SharedContentError> {
    let row = tx
        .query_row(
            "SELECT payload_sha256, response_json
             FROM shared_write_idempotency
             WHERE actor_uid = ?1 AND workspace_id = ?2 AND operation = ?3 AND idempotency_key = ?4",
            params![actor_uid, workspace_id, operation, idempotency_key],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?;
    let Some((stored_hash, response_json)) = row else {
        return Ok(None);
    };
    if stored_hash != payload_hash {
        return Err(SharedContentError::IdempotencyConflict);
    }
    let decoded: T = serde_json::from_str(&response_json)
        .map_err(|_| SharedContentError::InvalidInput("Corrupt idempotency receipt.".into()))?;
    Ok(Some(decoded))
}

#[allow(clippy::too_many_arguments)]
fn save_idempotent_response<T: Serialize>(
    tx: &Transaction<'_>,
    actor_uid: &str,
    workspace_id: &str,
    operation: &str,
    idempotency_key: &str,
    payload_hash: &str,
    response: &T,
    now_ms: i64,
) -> Result<(), SharedContentError> {
    let json = serde_json::to_string(response)
        .map_err(|_| SharedContentError::InvalidInput("Could not serialize response.".into()))?;
    tx.execute(
        "INSERT INTO shared_write_idempotency (
            actor_uid, workspace_id, operation, idempotency_key, payload_sha256, response_json, created_at_ms
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            actor_uid,
            workspace_id,
            operation,
            idempotency_key,
            payload_hash,
            json,
            now_ms,
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_activity(
    tx: &Transaction<'_>,
    workspace_id: &str,
    actor_uid: &str,
    actor_display_name: &str,
    action: &str,
    target_type: &str,
    page_id: Option<&str>,
    page_title: Option<&str>,
    proposal_id: Option<&str>,
    thread_id: Option<&str>,
    detail_text: &str,
    now_ms: i64,
) -> Result<(), SharedContentError> {
    insert_activity_with_project(
        tx,
        workspace_id,
        actor_uid,
        actor_display_name,
        action,
        target_type,
        None,
        None,
        page_id,
        page_title,
        proposal_id,
        thread_id,
        detail_text,
        now_ms,
    )
}

#[allow(clippy::too_many_arguments)]
fn insert_activity_with_project(
    tx: &Transaction<'_>,
    workspace_id: &str,
    actor_uid: &str,
    actor_display_name: &str,
    action: &str,
    target_type: &str,
    project_id: Option<&str>,
    project_name: Option<&str>,
    page_id: Option<&str>,
    page_title: Option<&str>,
    proposal_id: Option<&str>,
    thread_id: Option<&str>,
    detail_text: &str,
    now_ms: i64,
) -> Result<(), SharedContentError> {
    let activity_id = format!("act_{}", Uuid::new_v4().simple());
    tx.execute(
        "INSERT INTO shared_workspace_activity (
            activity_id, workspace_id, actor_uid, actor_display_name, action,
            target_type, project_id, project_name, page_id, page_title,
            proposal_id, thread_id, detail_text, created_at_ms
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            activity_id,
            workspace_id,
            actor_uid,
            actor_display_name,
            action,
            target_type,
            project_id,
            project_name,
            page_id,
            page_title,
            proposal_id,
            thread_id,
            detail_text,
            now_ms,
        ],
    )?;
    Ok(())
}

fn sanitize_project_name(raw: &str) -> Result<String, SharedContentError> {
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        return Err(SharedContentError::InvalidInput(
            "Project name is required.".into(),
        ));
    }
    if cleaned.chars().count() > MAX_PROJECT_NAME_LEN {
        return Err(SharedContentError::InvalidInput(format!(
            "Project name must be at most {MAX_PROJECT_NAME_LEN} characters."
        )));
    }
    if contains_disallowed_reference(&cleaned) {
        return Err(SharedContentError::InvalidInput(
            "Project name cannot include private file paths or vault URIs.".into(),
        ));
    }
    Ok(cleaned)
}

fn sanitize_project_purpose(raw: &str) -> Result<String, SharedContentError> {
    let normalized = raw.replace("\r\n", "\n");
    let cleaned = normalized.trim();
    if cleaned.chars().count() > MAX_PROJECT_PURPOSE_LEN {
        return Err(SharedContentError::InvalidInput(format!(
            "Project purpose must be at most {MAX_PROJECT_PURPOSE_LEN} characters."
        )));
    }
    if contains_disallowed_reference(cleaned) {
        return Err(SharedContentError::InvalidInput(
            "Project purpose cannot include private file paths or vault URIs.".into(),
        ));
    }
    Ok(cleaned.to_owned())
}

fn sanitize_search_query(raw: Option<&str>) -> Result<Option<String>, SharedContentError> {
    let Some(val) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if val.chars().count() > MAX_SEARCH_QUERY_LEN {
        return Err(SharedContentError::InvalidInput(format!(
            "Search query must be at most {MAX_SEARCH_QUERY_LEN} characters."
        )));
    }
    Ok(Some(val.to_lowercase()))
}

fn escape_like_pattern(q: &str) -> String {
    let mut out = String::with_capacity(q.len() + 4);
    for ch in q.chars() {
        match ch {
            '\\' | '%' | '_' => {
                out.push('\\');
                out.push(ch);
            }
            c => out.push(c),
        }
    }
    format!("%{out}%")
}

fn map_project_summary(r: &rusqlite::Row<'_>) -> rusqlite::Result<SharedProjectSummary> {
    Ok(SharedProjectSummary {
        project_id: r.get(0)?,
        workspace_id: r.get(1)?,
        name: r.get(2)?,
        purpose: r.get(3)?,
        version: r.get(4)?,
        created_by_uid: r.get(5)?,
        created_by_display_name: r.get(6)?,
        updated_by_uid: r.get(7)?,
        updated_by_display_name: r.get(8)?,
        archived_at_ms: r.get(9)?,
        archived_by_display_name: r.get(10)?,
        created_at_ms: r.get(11)?,
        updated_at_ms: r.get(12)?,
        active_page_count: r.get(13)?,
    })
}

fn load_project_summary(
    tx: &Transaction<'_>,
    workspace_id: &str,
    project_id: &str,
) -> Result<SharedProjectSummary, SharedContentError> {
    tx.query_row(
        "SELECT pr.project_id, pr.workspace_id, pr.name, pr.purpose, pr.version,
                pr.created_by_uid, pr.created_by_display_name,
                pr.updated_by_uid, pr.updated_by_display_name,
                pr.archived_at_ms, pr.archived_by_display_name,
                pr.created_at_ms, pr.updated_at_ms,
                (SELECT COUNT(*) FROM shared_project_pages m
                 JOIN shared_pages p ON p.workspace_id = m.workspace_id AND p.page_id = m.page_id
                 WHERE m.workspace_id = pr.workspace_id
                   AND m.project_id = pr.project_id
                   AND m.detached_at_ms IS NULL
                   AND p.archived_at_ms IS NULL)
         FROM shared_projects pr
         WHERE pr.workspace_id = ?1 AND pr.project_id = ?2",
        params![workspace_id, project_id],
        map_project_summary,
    )
    .optional()?
    .ok_or(SharedContentError::ProjectNotFound)
}

fn query_project_summaries(
    tx: &Transaction<'_>,
    workspace_id: &str,
    include_archived: bool,
    search_query: Option<&str>,
    cursor: Option<(i64, &str)>,
    limit: usize,
) -> Result<Vec<SharedProjectSummary>, SharedContentError> {
    let archive_clause = if include_archived {
        ""
    } else {
        "AND pr.archived_at_ms IS NULL"
    };
    let like_pat = search_query.map(escape_like_pattern);
    let sql = format!(
        "SELECT pr.project_id, pr.workspace_id, pr.name, pr.purpose, pr.version,
                pr.created_by_uid, pr.created_by_display_name,
                pr.updated_by_uid, pr.updated_by_display_name,
                pr.archived_at_ms, pr.archived_by_display_name,
                pr.created_at_ms, pr.updated_at_ms,
                (SELECT COUNT(*) FROM shared_project_pages m
                 JOIN shared_pages p ON p.workspace_id = m.workspace_id AND p.page_id = m.page_id
                 WHERE m.workspace_id = pr.workspace_id
                   AND m.project_id = pr.project_id
                   AND m.detached_at_ms IS NULL
                   AND p.archived_at_ms IS NULL)
         FROM shared_projects pr
         WHERE pr.workspace_id = ?1
           {archive_clause}
           AND (?2 IS NULL OR LOWER(pr.name) LIKE ?2 ESCAPE '\\' OR LOWER(pr.purpose) LIKE ?2 ESCAPE '\\')
           AND (?3 IS NULL OR (pr.updated_at_ms < ?3 OR (pr.updated_at_ms = ?3 AND pr.project_id < ?4)))
         ORDER BY pr.updated_at_ms DESC, pr.project_id DESC
         LIMIT {limit}"
    );
    let mut stmt = tx.prepare(&sql)?;
    let cursor_ts = cursor.map(|(ts, _)| ts);
    let cursor_id = cursor.map(|(_, id)| id);
    let rows = stmt.query_map(
        params![workspace_id, like_pat, cursor_ts, cursor_id],
        map_project_summary,
    )?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

fn query_project_page_items(
    tx: &Transaction<'_>,
    workspace_id: &str,
    project_id: &str,
    search_query: Option<&str>,
    cursor: Option<(i64, &str)>,
    limit: usize,
) -> Result<Vec<SharedProjectPageItem>, SharedContentError> {
    let like_pat = search_query.map(escape_like_pattern);
    let sql = format!(
        "SELECT p.page_id, p.workspace_id, p.title, p.current_revision_number, p.entry_mode,
                p.origin_kind, p.original_author_label, p.ai_model_label, p.source_note,
                p.omitted_summary_json, p.publisher_uid, p.publisher_display_name,
                p.last_editor_uid, p.last_editor_display_name,
                p.last_reviewer_uid, p.last_reviewer_display_name,
                p.archived_at_ms, p.archived_by_display_name, p.created_at_ms, p.updated_at_ms,
                (SELECT COUNT(*) FROM shared_comment_threads t
                 WHERE t.workspace_id = p.workspace_id AND t.page_id = p.page_id AND t.status = 'open'),
                (SELECT COUNT(*) FROM shared_change_proposals pr
                 WHERE pr.workspace_id = p.workspace_id AND pr.page_id = p.page_id AND pr.status = 'pending'),
                m.membership_id, m.generation, m.attached_by_uid, m.attached_by_display_name, m.attached_at_ms
         FROM shared_project_pages m
         JOIN shared_pages p ON p.workspace_id = m.workspace_id AND p.page_id = m.page_id
         WHERE m.workspace_id = ?1
           AND m.project_id = ?2
           AND m.detached_at_ms IS NULL
           AND p.archived_at_ms IS NULL
           AND (?3 IS NULL OR LOWER(p.title) LIKE ?3 ESCAPE '\\')
           AND (?4 IS NULL OR (p.updated_at_ms < ?4 OR (p.updated_at_ms = ?4 AND m.membership_id < ?5)))
         ORDER BY p.updated_at_ms DESC, m.membership_id DESC
         LIMIT {limit}"
    );
    let mut stmt = tx.prepare(&sql)?;
    let cursor_ts = cursor.map(|(ts, _)| ts);
    let cursor_id = cursor.map(|(_, id)| id);
    let rows = stmt.query_map(
        params![workspace_id, project_id, like_pat, cursor_ts, cursor_id],
        |r| {
            let page_res = map_page_summary(r)?;
            let membership_id: String = r.get(22)?;
            let generation: i64 = r.get(23)?;
            let attached_by_uid: String = r.get(24)?;
            let attached_by_display_name: String = r.get(25)?;
            let attached_at_ms: i64 = r.get(26)?;
            Ok(page_res.map(|page| SharedProjectPageItem {
                membership_id,
                generation,
                attached_by_uid,
                attached_by_display_name,
                attached_at_ms,
                page,
            }))
        },
    )?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r??);
    }
    Ok(out)
}

fn load_project_details(
    tx: &Transaction<'_>,
    workspace_id: &str,
    project_id: &str,
    search_query: Option<&str>,
    cursor: Option<(i64, &str)>,
    limit: Option<usize>,
) -> Result<SharedProjectDetails, SharedContentError> {
    let project = load_project_summary(tx, workspace_id, project_id)?;
    let page_limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    let rows = query_project_page_items(
        tx,
        workspace_id,
        project_id,
        search_query,
        cursor,
        page_limit + 1,
    )?;
    let (pages, pages_next_cursor) = paginate(rows, page_limit, |item| {
        format!("{}:{}", item.page.updated_at_ms, item.membership_id)
    });
    Ok(SharedProjectDetails {
        project,
        pages,
        pages_next_cursor,
    })
}

fn query_picker_pages(
    tx: &Transaction<'_>,
    workspace_id: &str,
    project_id: &str,
    search_query: Option<&str>,
    cursor: Option<(i64, &str)>,
    limit: usize,
) -> Result<Vec<SharedPageSummary>, SharedContentError> {
    let like_pat = search_query.map(escape_like_pattern);
    let sql = format!(
        "SELECT p.page_id, p.workspace_id, p.title, p.current_revision_number, p.entry_mode,
                p.origin_kind, p.original_author_label, p.ai_model_label, p.source_note,
                p.omitted_summary_json, p.publisher_uid, p.publisher_display_name,
                p.last_editor_uid, p.last_editor_display_name,
                p.last_reviewer_uid, p.last_reviewer_display_name,
                p.archived_at_ms, p.archived_by_display_name, p.created_at_ms, p.updated_at_ms,
                (SELECT COUNT(*) FROM shared_comment_threads t
                 WHERE t.workspace_id = p.workspace_id AND t.page_id = p.page_id AND t.status = 'open'),
                (SELECT COUNT(*) FROM shared_change_proposals pr
                 WHERE pr.workspace_id = p.workspace_id AND pr.page_id = p.page_id AND pr.status = 'pending')
         FROM shared_pages p
         WHERE p.workspace_id = ?1
           AND p.archived_at_ms IS NULL
           AND NOT EXISTS (
               SELECT 1 FROM shared_project_pages m
               WHERE m.workspace_id = p.workspace_id
                 AND m.project_id = ?2
                 AND m.page_id = p.page_id
                 AND m.detached_at_ms IS NULL
           )
           AND (?3 IS NULL OR LOWER(p.title) LIKE ?3 ESCAPE '\\')
           AND (?4 IS NULL OR (p.updated_at_ms < ?4 OR (p.updated_at_ms = ?4 AND p.page_id < ?5)))
         ORDER BY p.updated_at_ms DESC, p.page_id DESC
         LIMIT {limit}"
    );
    let mut stmt = tx.prepare(&sql)?;
    let cursor_ts = cursor.map(|(ts, _)| ts);
    let cursor_id = cursor.map(|(_, id)| id);
    let rows = stmt.query_map(
        params![workspace_id, project_id, like_pat, cursor_ts, cursor_id],
        map_page_summary,
    )?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r??);
    }
    Ok(out)
}

fn ensure_revision_exists(
    tx: &Transaction<'_>,
    workspace_id: &str,
    page_id: &str,
    revision_number: i64,
) -> Result<(), SharedContentError> {
    let found: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM shared_page_revisions
             WHERE workspace_id = ?1 AND page_id = ?2 AND revision_number = ?3",
            params![workspace_id, page_id, revision_number],
            |r| r.get(0),
        )
        .optional()?;
    if found.is_none() {
        return Err(SharedContentError::RevisionNotFound);
    }
    Ok(())
}

fn load_page_row(
    tx: &Transaction<'_>,
    workspace_id: &str,
    page_id: &str,
) -> Result<SharedPageSummary, SharedContentError> {
    let row = tx
        .query_row(
            "SELECT p.page_id, p.workspace_id, p.title, p.current_revision_number, p.entry_mode,
                    p.origin_kind, p.original_author_label, p.ai_model_label, p.source_note,
                    p.omitted_summary_json, p.publisher_uid, p.publisher_display_name,
                    p.last_editor_uid, p.last_editor_display_name,
                    p.last_reviewer_uid, p.last_reviewer_display_name,
                    p.archived_at_ms, p.archived_by_display_name, p.created_at_ms, p.updated_at_ms,
                    (SELECT COUNT(*) FROM shared_comment_threads t
                     WHERE t.workspace_id = p.workspace_id AND t.page_id = p.page_id AND t.status = 'open'),
                    (SELECT COUNT(*) FROM shared_change_proposals pr
                     WHERE pr.workspace_id = p.workspace_id AND pr.page_id = p.page_id AND pr.status = 'pending')
             FROM shared_pages p
             WHERE p.workspace_id = ?1 AND p.page_id = ?2",
            params![workspace_id, page_id],
            map_page_summary,
        )
        .optional()?;
    row.transpose()?.ok_or(SharedContentError::PageNotFound)
}

fn map_page_summary(
    r: &rusqlite::Row<'_>,
) -> rusqlite::Result<Result<SharedPageSummary, SharedContentError>> {
    let entry_raw: String = r.get(4)?;
    let origin_raw: String = r.get(5)?;
    let omitted_raw: String = r.get(9)?;
    let entry_mode = match PageEntryMode::from_db(&entry_raw) {
        Ok(v) => v,
        Err(e) => return Ok(Err(e)),
    };
    let origin_kind = match PageOriginKind::from_db(&origin_raw) {
        Ok(v) => v,
        Err(e) => return Ok(Err(e)),
    };
    let omitted_summary: Vec<String> = serde_json::from_str(&omitted_raw).unwrap_or_default();
    Ok(Ok(SharedPageSummary {
        page_id: r.get(0)?,
        workspace_id: r.get(1)?,
        title: r.get(2)?,
        current_revision_number: r.get(3)?,
        entry_mode,
        provenance: ShareableProvenance {
            origin_kind,
            original_author_label: r.get(6)?,
            ai_model_label: r.get(7)?,
            source_note: r.get(8)?,
        },
        omitted_summary,
        publisher_uid: r.get(10)?,
        publisher_display_name: r.get(11)?,
        last_editor_uid: r.get(12)?,
        last_editor_display_name: r.get(13)?,
        last_reviewer_uid: r.get(14)?,
        last_reviewer_display_name: r.get(15)?,
        archived_at_ms: r.get(16)?,
        archived_by_display_name: r.get(17)?,
        created_at_ms: r.get(18)?,
        updated_at_ms: r.get(19)?,
        unresolved_thread_count: r.get(20)?,
        pending_proposal_count: r.get(21)?,
    }))
}

fn query_page_summaries(
    tx: &Transaction<'_>,
    workspace_id: &str,
    include_archived: bool,
    cursor: Option<(i64, &str)>,
    limit: usize,
) -> Result<Vec<SharedPageSummary>, SharedContentError> {
    let archive_clause = if include_archived {
        ""
    } else {
        "AND p.archived_at_ms IS NULL"
    };
    let cursor_clause = if cursor.is_some() {
        "AND (p.updated_at_ms < ?2 OR (p.updated_at_ms = ?2 AND p.page_id < ?3))"
    } else {
        ""
    };
    let sql = format!(
        "SELECT p.page_id, p.workspace_id, p.title, p.current_revision_number, p.entry_mode,
                p.origin_kind, p.original_author_label, p.ai_model_label, p.source_note,
                p.omitted_summary_json, p.publisher_uid, p.publisher_display_name,
                p.last_editor_uid, p.last_editor_display_name,
                p.last_reviewer_uid, p.last_reviewer_display_name,
                p.archived_at_ms, p.archived_by_display_name, p.created_at_ms, p.updated_at_ms,
                (SELECT COUNT(*) FROM shared_comment_threads t
                 WHERE t.workspace_id = p.workspace_id AND t.page_id = p.page_id AND t.status = 'open'),
                (SELECT COUNT(*) FROM shared_change_proposals pr
                 WHERE pr.workspace_id = p.workspace_id AND pr.page_id = p.page_id AND pr.status = 'pending')
         FROM shared_pages p
         WHERE p.workspace_id = ?1 {archive_clause} {cursor_clause}
         ORDER BY p.updated_at_ms DESC, p.page_id DESC
         LIMIT {}",
        limit
    );
    let mut stmt = tx.prepare(&sql)?;
    let mut out = Vec::new();
    if let Some((ts, id)) = cursor {
        let rows = stmt.query_map(params![workspace_id, ts, id], map_page_summary)?;
        for row in rows {
            out.push(row??);
        }
    } else {
        let rows = stmt.query_map(params![workspace_id], map_page_summary)?;
        for row in rows {
            out.push(row??);
        }
    }
    Ok(out)
}

fn load_revision_row(
    tx: &Transaction<'_>,
    workspace_id: &str,
    page_id: &str,
    revision_number: i64,
) -> Result<SharedPageRevisionView, SharedContentError> {
    let row = tx
        .query_row(
            "SELECT revision_id, workspace_id, page_id, revision_number, title, content_md,
                    content_sha256, change_summary, author_uid, author_display_name,
                    reviewer_uid, reviewer_display_name, proposal_id, created_at_ms
             FROM shared_page_revisions
             WHERE workspace_id = ?1 AND page_id = ?2 AND revision_number = ?3",
            params![workspace_id, page_id, revision_number],
            map_revision_row,
        )
        .optional()?;
    row.ok_or(SharedContentError::RevisionNotFound)
}

fn map_revision_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SharedPageRevisionView> {
    Ok(SharedPageRevisionView {
        revision_id: r.get(0)?,
        workspace_id: r.get(1)?,
        page_id: r.get(2)?,
        revision_number: r.get(3)?,
        title: r.get(4)?,
        content_md: r.get(5)?,
        content_sha256: r.get(6)?,
        change_summary: r.get(7)?,
        author_uid: r.get(8)?,
        author_display_name: r.get(9)?,
        reviewer_uid: r.get(10)?,
        reviewer_display_name: r.get(11)?,
        proposal_id: r.get(12)?,
        created_at_ms: r.get(13)?,
    })
}

fn query_revisions(
    tx: &Transaction<'_>,
    workspace_id: &str,
    page_id: &str,
    cursor_rev: Option<i64>,
    limit: usize,
) -> Result<Vec<SharedPageRevisionView>, SharedContentError> {
    let mut stmt = if cursor_rev.is_some() {
        tx.prepare(
            "SELECT revision_id, workspace_id, page_id, revision_number, title, content_md,
                    content_sha256, change_summary, author_uid, author_display_name,
                    reviewer_uid, reviewer_display_name, proposal_id, created_at_ms
             FROM shared_page_revisions
             WHERE workspace_id = ?1 AND page_id = ?2 AND revision_number < ?3
             ORDER BY revision_number DESC
             LIMIT ?4",
        )?
    } else {
        tx.prepare(
            "SELECT revision_id, workspace_id, page_id, revision_number, title, content_md,
                    content_sha256, change_summary, author_uid, author_display_name,
                    reviewer_uid, reviewer_display_name, proposal_id, created_at_ms
             FROM shared_page_revisions
             WHERE workspace_id = ?1 AND page_id = ?2
             ORDER BY revision_number DESC
             LIMIT ?3",
        )?
    };
    let mut out = Vec::new();
    if let Some(rev) = cursor_rev {
        let rows = stmt.query_map(
            params![workspace_id, page_id, rev, limit as i64],
            map_revision_row,
        )?;
        for row in rows {
            out.push(row?);
        }
    } else {
        let rows = stmt.query_map(
            params![workspace_id, page_id, limit as i64],
            map_revision_row,
        )?;
        for row in rows {
            out.push(row?);
        }
    }
    Ok(out)
}

fn load_thread_by_id(
    tx: &Transaction<'_>,
    workspace_id: &str,
    page_id: &str,
    thread_id: &str,
) -> Result<SharedCommentThreadView, SharedContentError> {
    let row = tx
        .query_row(
            "SELECT thread_id, workspace_id, page_id, base_revision_number, author_uid,
                    author_display_name, status, resolved_at_ms, resolved_by_display_name,
                    created_at_ms, updated_at_ms
             FROM shared_comment_threads
             WHERE workspace_id = ?1 AND page_id = ?2 AND thread_id = ?3",
            params![workspace_id, page_id, thread_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, Option<i64>>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, i64>(9)?,
                    r.get::<_, i64>(10)?,
                ))
            },
        )
        .optional()?;
    let Some((
        tid,
        wid,
        pid,
        base_rev,
        author_uid,
        author_display_name,
        status_raw,
        resolved_at_ms,
        resolved_by_display_name,
        created_at_ms,
        updated_at_ms,
    )) = row
    else {
        return Err(SharedContentError::ThreadNotFound);
    };
    let status = ThreadStatus::from_db(&status_raw)?;
    let messages = query_thread_messages(tx, workspace_id, &tid)?;
    Ok(SharedCommentThreadView {
        thread_id: tid,
        workspace_id: wid,
        page_id: pid,
        base_revision_number: base_rev,
        author_uid,
        author_display_name,
        status,
        resolved_at_ms,
        resolved_by_display_name,
        created_at_ms,
        updated_at_ms,
        messages,
    })
}

fn query_thread_messages(
    tx: &Transaction<'_>,
    workspace_id: &str,
    thread_id: &str,
) -> Result<Vec<SharedCommentMessageView>, SharedContentError> {
    let mut stmt = tx.prepare(
        "SELECT message_id, thread_id, author_uid, author_display_name, body, created_at_ms
         FROM shared_comment_messages
         WHERE workspace_id = ?1 AND thread_id = ?2
         ORDER BY created_at_ms ASC, message_id ASC",
    )?;
    let rows = stmt.query_map(params![workspace_id, thread_id], |r| {
        Ok(SharedCommentMessageView {
            message_id: r.get(0)?,
            thread_id: r.get(1)?,
            author_uid: r.get(2)?,
            author_display_name: r.get(3)?,
            body: r.get(4)?,
            created_at_ms: r.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn query_threads(
    tx: &Transaction<'_>,
    workspace_id: &str,
    page_id: &str,
    cursor: Option<(i64, &str)>,
    limit: usize,
) -> Result<Vec<SharedCommentThreadView>, SharedContentError> {
    let sql = if cursor.is_some() {
        format!(
            "SELECT thread_id, workspace_id, page_id, base_revision_number, author_uid,
                    author_display_name, status, resolved_at_ms, resolved_by_display_name,
                    created_at_ms, updated_at_ms
             FROM shared_comment_threads
             WHERE workspace_id = ?1 AND page_id = ?2
               AND (updated_at_ms < ?3 OR (updated_at_ms = ?3 AND thread_id < ?4))
             ORDER BY updated_at_ms DESC, thread_id DESC
             LIMIT {}",
            limit
        )
    } else {
        format!(
            "SELECT thread_id, workspace_id, page_id, base_revision_number, author_uid,
                    author_display_name, status, resolved_at_ms, resolved_by_display_name,
                    created_at_ms, updated_at_ms
             FROM shared_comment_threads
             WHERE workspace_id = ?1 AND page_id = ?2
             ORDER BY updated_at_ms DESC, thread_id DESC
             LIMIT {}",
            limit
        )
    };
    let mut stmt = tx.prepare(&sql)?;
    let mut headers = Vec::new();
    let map_hdr = |r: &rusqlite::Row<'_>| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, String>(6)?,
            r.get::<_, Option<i64>>(7)?,
            r.get::<_, Option<String>>(8)?,
            r.get::<_, i64>(9)?,
            r.get::<_, i64>(10)?,
        ))
    };
    if let Some((ts, id)) = cursor {
        let rows = stmt.query_map(params![workspace_id, page_id, ts, id], map_hdr)?;
        for r in rows {
            headers.push(r?);
        }
    } else {
        let rows = stmt.query_map(params![workspace_id, page_id], map_hdr)?;
        for r in rows {
            headers.push(r?);
        }
    }
    drop(stmt);
    let mut out = Vec::with_capacity(headers.len());
    for (
        thread_id,
        wid,
        pid,
        base_revision_number,
        author_uid,
        author_display_name,
        status_raw,
        resolved_at_ms,
        resolved_by_display_name,
        created_at_ms,
        updated_at_ms,
    ) in headers
    {
        let status = ThreadStatus::from_db(&status_raw)?;
        let messages = query_thread_messages(tx, workspace_id, &thread_id)?;
        out.push(SharedCommentThreadView {
            thread_id,
            workspace_id: wid,
            page_id: pid,
            base_revision_number,
            author_uid,
            author_display_name,
            status,
            resolved_at_ms,
            resolved_by_display_name,
            created_at_ms,
            updated_at_ms,
            messages,
        });
    }
    Ok(out)
}

fn load_proposal_row(
    tx: &Transaction<'_>,
    workspace_id: &str,
    proposal_id: &str,
) -> Result<ProposalRow, SharedContentError> {
    let row = tx
        .query_row(
            "SELECT proposal_id, workspace_id, page_id, base_revision_number,
                    base_title, base_content_md, proposed_title, proposed_content_md,
                    summary, author_uid, author_display_name, status, submission_count,
                    created_at_ms, updated_at_ms, reviewed_at_ms, reviewer_display_name,
                    applied_revision_number
             FROM shared_change_proposals
             WHERE workspace_id = ?1 AND proposal_id = ?2",
            params![workspace_id, proposal_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, String>(8)?,
                    r.get::<_, String>(9)?,
                    r.get::<_, String>(10)?,
                    r.get::<_, String>(11)?,
                    r.get::<_, i64>(12)?,
                    r.get::<_, i64>(13)?,
                    r.get::<_, i64>(14)?,
                    r.get::<_, Option<i64>>(15)?,
                    r.get::<_, Option<String>>(16)?,
                    r.get::<_, Option<i64>>(17)?,
                ))
            },
        )
        .optional()?;
    let Some((
        proposal_id,
        workspace_id,
        page_id,
        base_revision_number,
        base_title,
        base_content_md,
        proposed_title,
        proposed_content_md,
        summary,
        author_uid,
        author_display_name,
        status_raw,
        submission_count,
        created_at_ms,
        updated_at_ms,
        reviewed_at_ms,
        reviewer_display_name,
        applied_revision_number,
    )) = row
    else {
        return Err(SharedContentError::ProposalNotFound);
    };
    Ok(ProposalRow {
        proposal_id,
        workspace_id,
        page_id,
        base_revision_number,
        base_title,
        base_content_md,
        proposed_title,
        proposed_content_md,
        summary,
        author_uid,
        author_display_name,
        status: ProposalStatus::from_db(&status_raw)?,
        submission_count,
        created_at_ms,
        updated_at_ms,
        reviewed_at_ms,
        reviewer_display_name,
        applied_revision_number,
    })
}

fn query_proposal_summaries(
    tx: &Transaction<'_>,
    workspace_id: &str,
    page_id: &str,
    cursor: Option<(i64, &str)>,
    limit: usize,
) -> Result<Vec<SharedProposalSummary>, SharedContentError> {
    let cursor_clause = if cursor.is_some() {
        "AND (pr.updated_at_ms < ?3 OR (pr.updated_at_ms = ?3 AND pr.proposal_id < ?4))"
    } else {
        ""
    };
    let sql = format!(
        "SELECT pr.proposal_id, pr.workspace_id, pr.page_id, p.title, pr.base_revision_number,
                pr.summary, pr.author_uid, pr.author_display_name, pr.status, pr.submission_count,
                p.current_revision_number, pr.created_at_ms, pr.updated_at_ms,
                pr.reviewed_at_ms, pr.reviewer_display_name, pr.applied_revision_number
         FROM shared_change_proposals pr
         JOIN shared_pages p ON p.workspace_id = pr.workspace_id AND p.page_id = pr.page_id
         WHERE pr.workspace_id = ?1 AND pr.page_id = ?2 {cursor_clause}
         ORDER BY pr.updated_at_ms DESC, pr.proposal_id DESC
         LIMIT {}",
        limit
    );
    let mut stmt = tx.prepare(&sql)?;
    let map_prop = |r: &rusqlite::Row<'_>| -> rusqlite::Result<Result<SharedProposalSummary, SharedContentError>> {
        let status_raw: String = r.get(8)?;
        let status = match ProposalStatus::from_db(&status_raw) {
            Ok(v) => v,
            Err(e) => return Ok(Err(e)),
        };
        let base_rev: i64 = r.get(4)?;
        let cur_rev: i64 = r.get(10)?;
        let is_stale = matches!(status, ProposalStatus::Pending | ProposalStatus::ChangesRequested)
            && base_rev != cur_rev;
        Ok(Ok(SharedProposalSummary {
            proposal_id: r.get(0)?,
            workspace_id: r.get(1)?,
            page_id: r.get(2)?,
            page_title: r.get(3)?,
            base_revision_number: base_rev,
            summary: r.get(5)?,
            author_uid: r.get(6)?,
            author_display_name: r.get(7)?,
            status,
            submission_count: r.get(9)?,
            is_stale,
            created_at_ms: r.get(11)?,
            updated_at_ms: r.get(12)?,
            reviewed_at_ms: r.get(13)?,
            reviewer_display_name: r.get(14)?,
            applied_revision_number: r.get(15)?,
        }))
    };
    let mut out = Vec::new();
    if let Some((ts, id)) = cursor {
        let rows = stmt.query_map(params![workspace_id, page_id, ts, id], map_prop)?;
        for r in rows {
            out.push(r??);
        }
    } else {
        let rows = stmt.query_map(params![workspace_id, page_id], map_prop)?;
        for r in rows {
            out.push(r??);
        }
    }
    Ok(out)
}

fn load_proposal_details(
    tx: &Transaction<'_>,
    workspace_id: &str,
    proposal_id: &str,
    caller_uid: &str,
    caller_role: WorkspaceRole,
) -> Result<SharedProposalDetails, SharedContentError> {
    let row = load_proposal_row(tx, workspace_id, proposal_id)?;
    let page = load_page_row(tx, workspace_id, &row.page_id)?;
    let cur_rev = load_revision_row(tx, workspace_id, &row.page_id, page.current_revision_number)?;
    let is_stale = matches!(
        row.status,
        ProposalStatus::Pending | ProposalStatus::ChangesRequested
    ) && row.base_revision_number != page.current_revision_number;

    let mut stmt = tx.prepare(
        "SELECT decision_id, submission_number, action, actor_uid, actor_display_name, note,
                base_revision_number, submitted_title, submitted_content_md, resulting_revision_number, created_at_ms
         FROM shared_proposal_decisions
         WHERE workspace_id = ?1 AND proposal_id = ?2
         ORDER BY decision_seq ASC",
    )?;
    let rows = stmt.query_map(params![workspace_id, proposal_id], |r| {
        Ok(ProposalDecisionView {
            decision_id: r.get(0)?,
            submission_number: r.get(1)?,
            action: r.get(2)?,
            actor_uid: r.get(3)?,
            actor_display_name: r.get(4)?,
            note: r.get(5)?,
            base_revision_number: r.get(6)?,
            submitted_title: r.get(7)?,
            submitted_content_md: r.get(8)?,
            resulting_revision_number: r.get(9)?,
            created_at_ms: r.get(10)?,
        })
    })?;
    let mut decisions = Vec::new();
    for r in rows {
        decisions.push(r?);
    }

    let page_archived = page.archived_at_ms.is_some();
    let can_review = !page_archived
        && row.status == ProposalStatus::Pending
        && caller_role.can_moderate_or_review()
        && row.author_uid != caller_uid;
    let can_author_edit = !page_archived
        && matches!(
            row.status,
            ProposalStatus::Pending | ProposalStatus::ChangesRequested
        )
        && caller_role.can_author_shared_content()
        && row.author_uid == caller_uid;

    Ok(SharedProposalDetails {
        proposal: SharedProposalSummary {
            proposal_id: row.proposal_id,
            workspace_id: row.workspace_id,
            page_id: row.page_id,
            page_title: page.title,
            base_revision_number: row.base_revision_number,
            summary: row.summary,
            author_uid: row.author_uid,
            author_display_name: row.author_display_name,
            status: row.status,
            submission_count: row.submission_count,
            is_stale,
            created_at_ms: row.created_at_ms,
            updated_at_ms: row.updated_at_ms,
            reviewed_at_ms: row.reviewed_at_ms,
            reviewer_display_name: row.reviewer_display_name,
            applied_revision_number: row.applied_revision_number,
        },
        base_title: row.base_title,
        base_content_md: row.base_content_md,
        proposed_title: row.proposed_title,
        proposed_content_md: row.proposed_content_md,
        current_page_revision_number: cur_rev.revision_number,
        current_page_title: cur_rev.title,
        current_page_content_md: cur_rev.content_md,
        page_archived,
        can_review,
        can_author_edit,
        decisions,
    })
}

fn load_page_details(
    tx: &Transaction<'_>,
    workspace_id: &str,
    page_id: &str,
) -> Result<SharedPageDetails, SharedContentError> {
    let page = load_page_row(tx, workspace_id, page_id)?;
    let current_revision =
        load_revision_row(tx, workspace_id, page_id, page.current_revision_number)?;
    let (revisions, revisions_next_cursor) = paginate(
        query_revisions(tx, workspace_id, page_id, None, DEFAULT_PAGE_LIMIT + 1)?,
        DEFAULT_PAGE_LIMIT,
        |r| r.revision_number.to_string(),
    );
    let (threads, threads_next_cursor) = paginate(
        query_threads(tx, workspace_id, page_id, None, DEFAULT_PAGE_LIMIT + 1)?,
        DEFAULT_PAGE_LIMIT,
        |t| format!("{}:{}", t.updated_at_ms, t.thread_id),
    );
    let (proposals, proposals_next_cursor) = paginate(
        query_proposal_summaries(tx, workspace_id, page_id, None, DEFAULT_PAGE_LIMIT + 1)?,
        DEFAULT_PAGE_LIMIT,
        |p| format!("{}:{}", p.updated_at_ms, p.proposal_id),
    );
    Ok(SharedPageDetails {
        page,
        current_revision,
        revisions,
        revisions_next_cursor,
        threads,
        threads_next_cursor,
        proposals,
        proposals_next_cursor,
    })
}
