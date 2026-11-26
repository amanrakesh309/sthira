pub mod crdt;
pub mod membership;
pub mod realtime;
pub mod server;
pub mod shared_content;
pub mod store;
pub mod verify;

use std::time::{SystemTime, UNIX_EPOCH};

pub use membership::{
    AcceptInvitationOutcome, BatchInvitationItemResult, BatchInvitationOutcome,
    BatchInviteItemOutcome, CreateWorkspaceOutcome, CreatedInvitation,
    InMemoryWorkspaceMembershipStore, InvitationPreview, InvitationStatus, MembershipError,
    OwnershipTransferView, SqliteWorkspaceMembershipStore, TransferStatus, WorkspaceAuditRecord,
    WorkspaceDetails, WorkspaceInvitationView, WorkspaceListResponse, WorkspaceMemberView,
    WorkspaceMembershipStore, WorkspaceRole, WorkspaceSummary, DEFAULT_INVITE_TTL_MS,
};
pub use server::{serve, SessionServiceState};
pub use shared_content::{
    AttachProjectPageInput, CreateProjectThreadInput, CreateProposalInput, CreateSharedPageInput,
    CreateSharedPageOutcome, CreateSharedProjectInput, CreateSharedProjectOutcome,
    CreateThreadInput, DetachProjectPageInput, LiveDraftContributorView, LiveDraftDiffResponse,
    LiveDraftStatus, LiveDraftUpdateInput, LiveDraftUpdateOutcome, OpenLiveDraftInput,
    OpenLiveDraftOutcome, PageEntryMode, PageOriginKind, PagedList, ProjectPageMembershipOutcome,
    ProposalDecisionView, ProposalReviewAction, ProposalStatus, RealtimeCatchupEvent,
    ReplyProjectThreadInput, ReplyThreadInput, ReviewProposalInput, ShareableProvenance,
    SharedActivityItem, SharedCommentMessageView, SharedCommentThreadView, SharedContentError,
    SharedContentStore, SharedLiveDraftView, SharedPageDetails, SharedPageRevisionView,
    SharedPageSummary, SharedProjectDetails, SharedProjectMessageView, SharedProjectPageItem,
    SharedProjectSummary, SharedProjectThreadView, SharedProposalDetails, SharedProposalSummary,
    SqliteSharedContentStore, SubmitLiveDraftInput, ThreadStatus, UpdateSharedProjectInput,
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
