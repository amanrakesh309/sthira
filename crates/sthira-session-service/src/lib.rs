pub mod membership;
pub mod server;
pub mod shared_content;
pub mod store;
pub mod verify;

use std::time::{SystemTime, UNIX_EPOCH};

pub use membership::{
    AcceptInvitationOutcome, CreateWorkspaceOutcome, CreatedInvitation,
    InMemoryWorkspaceMembershipStore, InvitationPreview, InvitationStatus, MembershipError,
    OwnershipTransferView, SqliteWorkspaceMembershipStore, TransferStatus, WorkspaceAuditRecord,
    WorkspaceDetails, WorkspaceInvitationView, WorkspaceListResponse, WorkspaceMemberView,
    WorkspaceMembershipStore, WorkspaceRole, WorkspaceSummary, DEFAULT_INVITE_TTL_MS,
};
pub use server::{serve, SessionServiceState};
pub use shared_content::{
    CreateProposalInput, CreateSharedPageInput, CreateSharedPageOutcome, CreateThreadInput,
    PageEntryMode, PageOriginKind, PagedList, ProposalDecisionView, ProposalReviewAction,
    ProposalStatus, ReplyThreadInput, ReviewProposalInput, ShareableProvenance, SharedActivityItem,
    SharedCommentMessageView, SharedCommentThreadView, SharedContentError, SharedContentStore,
    SharedPageDetails, SharedPageRevisionView, SharedPageSummary, SharedProposalDetails,
    SharedProposalSummary, SqliteSharedContentStore, ThreadStatus,
};
pub use store::{
    AuthSessionStore, ChallengeResponse, DeviceSummary, EnrolledSession, RevokeOutcome,
    SqliteAuthSessionStore, StoreError, VerifiedSession,
};
pub use verify::{
    GoogleFirebaseVerifier, IdentityVerifier, NonceExpectation, VerifiedIdentity, VerifyError,
};

pub fn now_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
