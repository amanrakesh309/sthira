import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './ipc';
import type { GoogleSession } from '../components/AccountModal';

export type WorkspaceRole = 'owner' | 'admin' | 'editor' | 'viewer';
export type InvitationStatus = 'pending' | 'accepted' | 'expired' | 'revoked';
export type TransferStatus = 'pending' | 'accepted' | 'cancelled';

export interface WorkspaceSummary {
  workspaceId: string;
  name: string;
  myMembershipId: string;
  myRole: WorkspaceRole;
  createdAtMs: number;
  updatedAtMs: number;
}

export interface CreateWorkspaceOutcome {
  created: boolean;
  workspace: WorkspaceSummary;
}

export interface WorkspaceMemberView {
  membershipId: string;
  uid: string;
  displayName: string;
  role: WorkspaceRole;
  joinedAtMs: number;
  isCurrentUser: boolean;
}

export interface WorkspaceInvitationView {
  invitationId: string;
  workspaceId: string;
  recipientEmail: string;
  offeredRole: WorkspaceRole;
  status: InvitationStatus;
  invitedByMembershipId: string;
  invitedByDisplayName: string;
  createdAtMs: number;
  expiresAtMs: number;
  acceptedAtMs?: number | null;
  revokedAtMs?: number | null;
}

export interface CreatedInvitation {
  invitation: WorkspaceInvitationView;
  inviteCode: string;
}

export interface InvitationPreview {
  invitationId: string;
  workspaceId: string;
  workspaceName: string;
  offeredRole: WorkspaceRole;
  expiresAtMs: number;
  alreadyMember: boolean;
  existingRole?: WorkspaceRole | null;
}

export interface AcceptInvitationOutcome {
  alreadyMember: boolean;
  workspace: WorkspaceSummary;
  membership: WorkspaceMemberView;
}

export interface OwnershipTransferView {
  transferId: string;
  workspaceId: string;
  workspaceName: string;
  fromMembershipId: string;
  fromUid: string;
  fromDisplayName: string;
  toMembershipId: string;
  toUid: string;
  toDisplayName: string;
  status: TransferStatus;
  createdAtMs: number;
}

export interface WorkspaceDetails {
  workspace: WorkspaceSummary;
  members: WorkspaceMemberView[];
  invitations: WorkspaceInvitationView[];
  pendingTransfer?: OwnershipTransferView | null;
}

export interface WorkspaceListResponse {
  workspaces: WorkspaceSummary[];
  incomingTransfers: OwnershipTransferView[];
}

export type WorkspaceResponse<T> =
  | { outcome: 'ok'; data: T }
  | { outcome: 'sessionUpdated'; session: GoogleSession }
  | { outcome: 'accessLost'; message: string; workspaces?: WorkspaceListResponse | null }
  | { outcome: 'error'; code: string; message: string };

export const WORKSPACE_ROLE_LABELS: Record<WorkspaceRole, string> = {
  owner: 'Owner',
  admin: 'Admin',
  editor: 'Editor',
  viewer: 'Viewer',
};

export function canRenameWorkspace(role: WorkspaceRole): boolean {
  return role === 'owner' || role === 'admin';
}

export function grantableInviteRoles(role: WorkspaceRole): WorkspaceRole[] {
  if (role === 'owner') return ['admin', 'editor', 'viewer'];
  if (role === 'admin') return ['editor', 'viewer'];
  return [];
}

export function editableRolesForTarget(
  actorRole: WorkspaceRole,
  targetRole: WorkspaceRole,
  isSelf: boolean,
): WorkspaceRole[] {
  if (isSelf || targetRole === 'owner') return [];
  if (actorRole === 'owner') return ['admin', 'editor', 'viewer'];
  if (actorRole === 'admin' && (targetRole === 'editor' || targetRole === 'viewer')) {
    return ['editor', 'viewer'];
  }
  return [];
}

export function canRemoveTarget(
  actorRole: WorkspaceRole,
  targetRole: WorkspaceRole,
  isSelf: boolean,
): boolean {
  if (isSelf || targetRole === 'owner') return false;
  if (actorRole === 'owner') return true;
  return actorRole === 'admin' && (targetRole === 'editor' || targetRole === 'viewer');
}

function previewUnavailable<T>(): WorkspaceResponse<T> {
  return { outcome: 'sessionUpdated', session: { status: 'unavailable' } };
}

export async function getActiveWorkspaceId(): Promise<string | null> {
  if (!isTauri) return null;
  return invoke<string | null>('cmd_get_active_workspace');
}

export async function setActiveWorkspaceId(workspaceId: string | null): Promise<void> {
  if (!isTauri) return;
  await invoke('cmd_set_active_workspace', { workspaceId });
}

export async function listWorkspaces(): Promise<WorkspaceResponse<WorkspaceListResponse>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<WorkspaceListResponse>>('cmd_list_workspaces');
}

export async function createWorkspace(
  name: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<CreateWorkspaceOutcome>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<CreateWorkspaceOutcome>>('cmd_create_workspace', {
    name,
    idempotencyKey,
  });
}

export async function getWorkspaceDetails(
  workspaceId: string,
): Promise<WorkspaceResponse<WorkspaceDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<WorkspaceDetails>>('cmd_get_workspace', { workspaceId });
}

export async function renameWorkspace(
  workspaceId: string,
  name: string,
): Promise<WorkspaceResponse<WorkspaceSummary>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<WorkspaceSummary>>('cmd_rename_workspace', {
    workspaceId,
    name,
  });
}

export async function createInvitation(
  workspaceId: string,
  email: string,
  role: WorkspaceRole,
): Promise<WorkspaceResponse<CreatedInvitation>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<CreatedInvitation>>('cmd_create_invitation', {
    workspaceId,
    email,
    role,
  });
}

export async function revokeInvitation(
  workspaceId: string,
  invitationId: string,
): Promise<WorkspaceResponse<WorkspaceInvitationView>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<WorkspaceInvitationView>>('cmd_revoke_invitation', {
    workspaceId,
    invitationId,
  });
}

export async function replaceInvitation(
  workspaceId: string,
  invitationId: string,
): Promise<WorkspaceResponse<CreatedInvitation>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<CreatedInvitation>>('cmd_replace_invitation', {
    workspaceId,
    invitationId,
  });
}

export async function previewInvitation(
  inviteCode: string,
): Promise<WorkspaceResponse<InvitationPreview>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<InvitationPreview>>('cmd_preview_invitation', {
    inviteCode,
  });
}

export async function acceptInvitation(
  inviteCode: string,
): Promise<WorkspaceResponse<AcceptInvitationOutcome>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<AcceptInvitationOutcome>>('cmd_accept_invitation', {
    inviteCode,
  });
}

export async function changeMemberRole(
  workspaceId: string,
  membershipId: string,
  role: WorkspaceRole,
): Promise<WorkspaceResponse<WorkspaceDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<WorkspaceDetails>>('cmd_change_member_role', {
    workspaceId,
    membershipId,
    role,
  });
}

export async function removeMember(
  workspaceId: string,
  membershipId: string,
): Promise<WorkspaceResponse<WorkspaceDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<WorkspaceDetails>>('cmd_remove_member', {
    workspaceId,
    membershipId,
  });
}

export async function leaveWorkspace(
  workspaceId: string,
): Promise<WorkspaceResponse<{ left: boolean; workspaceId: string }>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<{ left: boolean; workspaceId: string }>>('cmd_leave_workspace', {
    workspaceId,
  });
}

export async function initiateOwnershipTransfer(
  workspaceId: string,
  toMembershipId: string,
): Promise<WorkspaceResponse<WorkspaceDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<WorkspaceDetails>>('cmd_initiate_ownership_transfer', {
    workspaceId,
    toMembershipId,
  });
}

export async function acceptOwnershipTransfer(
  workspaceId: string,
  transferId: string,
): Promise<WorkspaceResponse<WorkspaceDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<WorkspaceDetails>>('cmd_accept_ownership_transfer', {
    workspaceId,
    transferId,
  });
}

export async function cancelOwnershipTransfer(
  workspaceId: string,
  transferId: string,
): Promise<WorkspaceResponse<WorkspaceDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<WorkspaceDetails>>('cmd_cancel_ownership_transfer', {
    workspaceId,
    transferId,
  });
}

// ---------------------------------------------------------------------------
// CW2 Shared Pages, Comments, Change Review, Activity, and Isolated Drafts
// ---------------------------------------------------------------------------

export type PageEntryMode = 'new' | 'copyPersonal';
export type PageOriginKind = 'userAuthored' | 'aiAssisted' | 'externalSource';
export type ThreadStatus = 'open' | 'resolved';
export type ProposalStatus = 'pending' | 'changesRequested' | 'rejected' | 'applied' | 'withdrawn';
export type ProposalReviewAction = 'apply' | 'requestChanges' | 'reject';

export interface ShareableProvenance {
  originKind: PageOriginKind;
  originalAuthorLabel?: string | null;
  aiModelLabel?: string | null;
  sourceNote?: string | null;
}

export interface CreateSharedPageInput {
  title: string;
  contentMd: string;
  entryMode: PageEntryMode;
  provenance: ShareableProvenance;
  omittedSummary?: string[];
  idempotencyKey: string;
}

export interface SharedPageSummary {
  pageId: string;
  workspaceId: string;
  title: string;
  currentRevisionNumber: number;
  entryMode: PageEntryMode;
  provenance: ShareableProvenance;
  omittedSummary: string[];
  publisherUid: string;
  publisherDisplayName: string;
  lastEditorUid: string;
  lastEditorDisplayName: string;
  lastReviewerUid?: string | null;
  lastReviewerDisplayName?: string | null;
  archivedAtMs?: number | null;
  archivedByDisplayName?: string | null;
  unresolvedThreadCount: number;
  pendingProposalCount: number;
  createdAtMs: number;
  updatedAtMs: number;
}

export interface SharedPageRevisionView {
  revisionId: string;
  workspaceId: string;
  pageId: string;
  revisionNumber: number;
  title: string;
  contentMd: string;
  contentSha256: string;
  changeSummary: string;
  authorUid: string;
  authorDisplayName: string;
  reviewerUid?: string | null;
  reviewerDisplayName?: string | null;
  proposalId?: string | null;
  contributorNames?: string[];
  createdAtMs: number;
}

export interface SharedCommentMessageView {
  messageId: string;
  threadId: string;
  authorUid: string;
  authorDisplayName: string;
  body: string;
  createdAtMs: number;
}

export interface SharedCommentThreadView {
  threadId: string;
  workspaceId: string;
  pageId: string;
  baseRevisionNumber: number;
  authorUid: string;
  authorDisplayName: string;
  status: ThreadStatus;
  resolvedAtMs?: number | null;
  resolvedByDisplayName?: string | null;
  createdAtMs: number;
  updatedAtMs: number;
  messages: SharedCommentMessageView[];
}

export interface ProposalDecisionView {
  decisionId: string;
  submissionNumber: number;
  action: string;
  actorUid: string;
  actorDisplayName: string;
  note?: string | null;
  baseRevisionNumber: number;
  submittedTitle: string;
  submittedContentMd: string;
  resultingRevisionNumber?: number | null;
  createdAtMs: number;
}

export interface SharedProposalSummary {
  proposalId: string;
  workspaceId: string;
  pageId: string;
  pageTitle: string;
  baseRevisionNumber: number;
  summary: string;
  authorUid: string;
  authorDisplayName: string;
  status: ProposalStatus;
  submissionCount: number;
  isStale: boolean;
  createdAtMs: number;
  updatedAtMs: number;
  reviewedAtMs?: number | null;
  reviewerDisplayName?: string | null;
  appliedRevisionNumber?: number | null;
  liveDraftId?: string | null;
  draftVersion?: number | null;
  contributorUids?: string[];
  contributorNames?: string[];
}

export interface SharedProposalDetails {
  proposal: SharedProposalSummary;
  baseTitle: string;
  baseContentMd: string;
  proposedTitle: string;
  proposedContentMd: string;
  currentPageRevisionNumber: number;
  currentPageTitle: string;
  currentPageContentMd: string;
  pageArchived: boolean;
  canReview: boolean;
  canAuthorEdit: boolean;
  hasEligibleReviewer?: boolean;
  ineligibleReason?: string | null;
  decisions: ProposalDecisionView[];
}

export interface SharedPageDetails {
  page: SharedPageSummary;
  currentRevision: SharedPageRevisionView;
  revisions: SharedPageRevisionView[];
  revisionsNextCursor?: string | null;
  threads: SharedCommentThreadView[];
  threadsNextCursor?: string | null;
  proposals: SharedProposalSummary[];
  proposalsNextCursor?: string | null;
}

export interface CreateSharedPageOutcome {
  created: boolean;
  page: SharedPageDetails;
}

export interface SharedActivityItem {
  activityId: string;
  workspaceId: string;
  actorUid: string;
  actorDisplayName: string;
  action: string;
  targetType: string;
  projectId?: string | null;
  projectName?: string | null;
  pageId?: string | null;
  pageTitle?: string | null;
  proposalId?: string | null;
  threadId?: string | null;
  detailText: string;
  createdAtMs: number;
}

export interface SharedProjectSummary {
  projectId: string;
  workspaceId: string;
  name: string;
  purpose: string;
  version: number;
  activePageCount: number;
  createdByUid: string;
  createdByDisplayName: string;
  updatedByUid: string;
  updatedByDisplayName: string;
  archivedAtMs?: number | null;
  archivedByDisplayName?: string | null;
  createdAtMs: number;
  updatedAtMs: number;
}

export interface SharedProjectPageItem {
  membershipId: string;
  generation: number;
  attachedByUid: string;
  attachedByDisplayName: string;
  attachedAtMs: number;
  page: SharedPageSummary;
}

export interface SharedProjectDetails {
  project: SharedProjectSummary;
  pages: SharedProjectPageItem[];
  pagesNextCursor?: string | null;
}

export interface CreateSharedProjectInput {
  name: string;
  purpose: string;
  idempotencyKey: string;
}

export interface CreateSharedProjectOutcome {
  created: boolean;
  project: SharedProjectDetails;
}

export interface ProjectPageMembershipOutcome {
  attached: boolean;
  membershipId: string;
  generation: number;
  project: SharedProjectDetails;
}

export interface PagedList<T> {
  items: T[];
  nextCursor?: string | null;
}

export type BatchInviteItemOutcome =
  | 'created'
  | 'alreadyMember'
  | 'alreadyPending'
  | 'invalidEmail';

export interface BatchInvitationItemResult {
  email: string;
  outcome: BatchInviteItemOutcome;
  message: string;
  invitation?: WorkspaceInvitationView | null;
  inviteCode?: string | null;
  replayed: boolean;
}

export interface BatchInvitationOutcome {
  offeredRole: WorkspaceRole;
  replayed: boolean;
  results: BatchInvitationItemResult[];
}

export type LiveDraftStatus = 'editing' | 'pendingReview' | 'applied';

export interface LiveDraftContributorView {
  uid: string;
  displayName: string;
  contributionCount: number;
  lastEditedAtMs: number;
}

export interface SharedLiveDraftView {
  draftId: string;
  workspaceId: string;
  pageId: string;
  baseRevisionNumber: number;
  draftVersion: number;
  checkpointSeq: number;
  checkpointId: string;
  status: LiveDraftStatus;
  snapshotStateB64: string;
  snapshotVectorB64: string;
  baseTitle: string;
  baseContentMd: string;
  currentTitle: string;
  currentContentMd: string;
  contributors: LiveDraftContributorView[];
  pendingProposalId?: string | null;
  createdByUid: string;
  createdByDisplayName: string;
  createdAtMs: number;
  updatedAtMs: number;
}

export interface OpenLiveDraftOutcome {
  created: boolean;
  draft: SharedLiveDraftView;
}

export interface LiveDraftDiffResponse {
  draft: SharedLiveDraftView;
  missingUpdateB64: string;
  serverVectorB64: string;
}

export interface LiveDraftUpdateOutcome {
  accepted: boolean;
  duplicate: boolean;
  draftVersion: number;
  checkpointSeq: number;
  checkpointId: string;
  serverSeq: number;
  acceptedUpdateB64: string;
  contributors: LiveDraftContributorView[];
}

export type LiveDraftRebaseMode = 'carryDraft' | 'restartFromCurrent';

export interface SharedProjectMessageView {
  messageId: string;
  threadId: string;
  authorUid: string;
  authorDisplayName: string;
  body: string;
  createdAtMs: number;
}

export interface SharedProjectThreadView {
  threadId: string;
  workspaceId: string;
  projectId: string;
  authorUid: string;
  authorDisplayName: string;
  status: ThreadStatus;
  resolvedAtMs?: number | null;
  resolvedByDisplayName?: string | null;
  createdAtMs: number;
  updatedAtMs: number;
  messages: SharedProjectMessageView[];
}

export interface RealtimeCatchupEvent {
  eventSeq: number;
  workspaceId: string;
  eventType: string;
  projectId?: string | null;
  pageId?: string | null;
  threadId?: string | null;
  proposalId?: string | null;
  draftId?: string | null;
  payloadJson: unknown;
  createdAtMs: number;
}

export interface SharedDraftRecord {
  accountUid: string;
  workspaceId: string;
  draftKey: string;
  pageId?: string | null;
  proposalId?: string | null;
  baseRevisionNumber?: number | null;
  title: string;
  contentMd: string;
  summary: string;
  updatedAtMs: number;
}

export function canAuthorSharedContent(role: WorkspaceRole): boolean {
  return role === 'owner' || role === 'admin' || role === 'editor';
}

export function canReviewOrArchiveSharedContent(role: WorkspaceRole): boolean {
  return role === 'owner' || role === 'admin';
}

export function canManageSharedProject(role: WorkspaceRole): boolean {
  return role === 'owner' || role === 'admin';
}

export function makeRequestIdempotencyKey(prefix = 'cw2'): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return `${prefix}_${crypto.randomUUID().replace(/-/g, '')}`;
  }
  return `${prefix}_${Date.now()}_${Math.random().toString(36).slice(2, 12)}`;
}

export async function listSharedPages(
  workspaceId: string,
  includeArchived = false,
  cursor?: string | null,
  limit?: number,
): Promise<WorkspaceResponse<PagedList<SharedPageSummary>>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<PagedList<SharedPageSummary>>>('cmd_list_shared_pages', {
    workspaceId,
    includeArchived,
    cursor: cursor ?? null,
    limit: limit ?? null,
  });
}

export async function createSharedPage(
  workspaceId: string,
  input: CreateSharedPageInput,
): Promise<WorkspaceResponse<CreateSharedPageOutcome>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<CreateSharedPageOutcome>>('cmd_create_shared_page', {
    workspaceId,
    input,
  });
}

export async function getSharedPage(
  workspaceId: string,
  pageId: string,
): Promise<WorkspaceResponse<SharedPageDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedPageDetails>>('cmd_get_shared_page', {
    workspaceId,
    pageId,
  });
}

export async function listSharedRevisions(
  workspaceId: string,
  pageId: string,
  cursor?: string | null,
  limit?: number,
): Promise<WorkspaceResponse<PagedList<SharedPageRevisionView>>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<PagedList<SharedPageRevisionView>>>('cmd_list_shared_revisions', {
    workspaceId,
    pageId,
    cursor: cursor ?? null,
    limit: limit ?? null,
  });
}

export async function archiveSharedPage(
  workspaceId: string,
  pageId: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedPageDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedPageDetails>>('cmd_archive_shared_page', {
    workspaceId,
    pageId,
    idempotencyKey,
  });
}

export async function restoreSharedPage(
  workspaceId: string,
  pageId: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedPageDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedPageDetails>>('cmd_restore_shared_page', {
    workspaceId,
    pageId,
    idempotencyKey,
  });
}

export async function listPageThreads(
  workspaceId: string,
  pageId: string,
  cursor?: string | null,
  limit?: number,
): Promise<WorkspaceResponse<PagedList<SharedCommentThreadView>>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<PagedList<SharedCommentThreadView>>>('cmd_list_page_threads', {
    workspaceId,
    pageId,
    cursor: cursor ?? null,
    limit: limit ?? null,
  });
}

export async function createPageThread(
  workspaceId: string,
  pageId: string,
  baseRevisionNumber: number,
  body: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedCommentThreadView>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedCommentThreadView>>('cmd_create_page_thread', {
    workspaceId,
    pageId,
    baseRevisionNumber,
    body,
    idempotencyKey,
  });
}

export async function replyPageThread(
  workspaceId: string,
  pageId: string,
  threadId: string,
  body: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedCommentThreadView>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedCommentThreadView>>('cmd_reply_page_thread', {
    workspaceId,
    pageId,
    threadId,
    body,
    idempotencyKey,
  });
}

export async function resolvePageThread(
  workspaceId: string,
  pageId: string,
  threadId: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedCommentThreadView>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedCommentThreadView>>('cmd_resolve_page_thread', {
    workspaceId,
    pageId,
    threadId,
    idempotencyKey,
  });
}

export async function reopenPageThread(
  workspaceId: string,
  pageId: string,
  threadId: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedCommentThreadView>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedCommentThreadView>>('cmd_reopen_page_thread', {
    workspaceId,
    pageId,
    threadId,
    idempotencyKey,
  });
}

export async function getProposalDetails(
  workspaceId: string,
  proposalId: string,
): Promise<WorkspaceResponse<SharedProposalDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedProposalDetails>>('cmd_get_proposal', {
    workspaceId,
    proposalId,
  });
}

export async function createProposal(
  workspaceId: string,
  pageId: string,
  baseRevisionNumber: number,
  proposedTitle: string,
  proposedContentMd: string,
  summary: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedProposalDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedProposalDetails>>('cmd_create_proposal', {
    workspaceId,
    pageId,
    baseRevisionNumber,
    proposedTitle,
    proposedContentMd,
    summary,
    idempotencyKey,
  });
}

export async function resubmitProposal(
  workspaceId: string,
  proposalId: string,
  baseRevisionNumber: number,
  proposedTitle: string,
  proposedContentMd: string,
  summary: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedProposalDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedProposalDetails>>('cmd_resubmit_proposal', {
    workspaceId,
    proposalId,
    baseRevisionNumber,
    proposedTitle,
    proposedContentMd,
    summary,
    idempotencyKey,
  });
}

export async function withdrawProposal(
  workspaceId: string,
  proposalId: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedProposalDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedProposalDetails>>('cmd_withdraw_proposal', {
    workspaceId,
    proposalId,
    idempotencyKey,
  });
}

export async function reviewProposal(
  workspaceId: string,
  proposalId: string,
  action: ProposalReviewAction,
  expectedCurrentRevision: number,
  expectedSubmissionNumber: number,
  note: string | null,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedProposalDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedProposalDetails>>('cmd_review_proposal', {
    workspaceId,
    proposalId,
    action,
    expectedCurrentRevision,
    expectedSubmissionNumber,
    note,
    idempotencyKey,
  });
}

export async function listWorkspaceActivity(
  workspaceId: string,
  cursor?: string | null,
  limit?: number,
): Promise<WorkspaceResponse<PagedList<SharedActivityItem>>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<PagedList<SharedActivityItem>>>('cmd_list_workspace_activity', {
    workspaceId,
    cursor: cursor ?? null,
    limit: limit ?? null,
  });
}

export async function listSharedProjects(
  workspaceId: string,
  includeArchived = false,
  q?: string | null,
  cursor?: string | null,
  limit?: number,
): Promise<WorkspaceResponse<PagedList<SharedProjectSummary>>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<PagedList<SharedProjectSummary>>>('cmd_list_shared_projects', {
    workspaceId,
    includeArchived,
    q: q ?? null,
    cursor: cursor ?? null,
    limit: limit ?? null,
  });
}

export async function createSharedProject(
  workspaceId: string,
  input: CreateSharedProjectInput,
): Promise<WorkspaceResponse<CreateSharedProjectOutcome>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<CreateSharedProjectOutcome>>('cmd_create_shared_project', {
    workspaceId,
    input,
  });
}

export async function getSharedProject(
  workspaceId: string,
  projectId: string,
  q?: string | null,
  cursor?: string | null,
  limit?: number,
): Promise<WorkspaceResponse<SharedProjectDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedProjectDetails>>('cmd_get_shared_project', {
    workspaceId,
    projectId,
    q: q ?? null,
    cursor: cursor ?? null,
    limit: limit ?? null,
  });
}

export async function updateSharedProject(
  workspaceId: string,
  projectId: string,
  name: string,
  purpose: string,
  expectedVersion: number,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedProjectDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedProjectDetails>>('cmd_update_shared_project', {
    workspaceId,
    projectId,
    name,
    purpose,
    expectedVersion,
    idempotencyKey,
  });
}

export async function archiveSharedProject(
  workspaceId: string,
  projectId: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedProjectDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedProjectDetails>>('cmd_archive_shared_project', {
    workspaceId,
    projectId,
    idempotencyKey,
  });
}

export async function restoreSharedProject(
  workspaceId: string,
  projectId: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedProjectDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedProjectDetails>>('cmd_restore_shared_project', {
    workspaceId,
    projectId,
    idempotencyKey,
  });
}

export async function listProjectPickerPages(
  workspaceId: string,
  projectId: string,
  q?: string | null,
  cursor?: string | null,
  limit?: number,
): Promise<WorkspaceResponse<PagedList<SharedPageSummary>>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<PagedList<SharedPageSummary>>>('cmd_list_project_picker_pages', {
    workspaceId,
    projectId,
    q: q ?? null,
    cursor: cursor ?? null,
    limit: limit ?? null,
  });
}

export async function attachPageToProject(
  workspaceId: string,
  projectId: string,
  pageId: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<ProjectPageMembershipOutcome>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<ProjectPageMembershipOutcome>>('cmd_attach_page_to_project', {
    workspaceId,
    projectId,
    pageId,
    idempotencyKey,
  });
}

export async function detachPageFromProject(
  workspaceId: string,
  projectId: string,
  pageId: string,
  membershipId: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedProjectDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedProjectDetails>>('cmd_detach_page_from_project', {
    workspaceId,
    projectId,
    pageId,
    membershipId,
    idempotencyKey,
  });
}

export async function getSharedDraft(
  workspaceId: string,
  draftKey: string,
  accountUid?: string | null,
): Promise<SharedDraftRecord | null> {
  if (!isTauri) return null;
  return invoke<SharedDraftRecord | null>('cmd_get_shared_draft', {
    workspaceId,
    draftKey,
    accountUid: accountUid ?? null,
  });
}

export async function saveSharedDraft(params: {
  workspaceId: string;
  draftKey: string;
  pageId?: string | null;
  proposalId?: string | null;
  baseRevisionNumber?: number | null;
  title: string;
  contentMd: string;
  summary: string;
  accountUid?: string | null;
}): Promise<SharedDraftRecord | null> {
  if (!isTauri) return null;
  return invoke<SharedDraftRecord>('cmd_save_shared_draft', {
    workspaceId: params.workspaceId,
    draftKey: params.draftKey,
    pageId: params.pageId ?? null,
    proposalId: params.proposalId ?? null,
    baseRevisionNumber: params.baseRevisionNumber ?? null,
    title: params.title,
    contentMd: params.contentMd,
    summary: params.summary,
    accountUid: params.accountUid ?? null,
  });
}

export async function deleteSharedDraft(
  workspaceId: string,
  draftKey: string,
  accountUid?: string | null,
): Promise<void> {
  if (!isTauri) return;
  await invoke('cmd_delete_shared_draft', {
    workspaceId,
    draftKey,
    accountUid: accountUid ?? null,
  });
}

export async function createInvitationBatch(
  workspaceId: string,
  emails: string[],
  role: WorkspaceRole,
  idempotencyKey: string,
): Promise<WorkspaceResponse<BatchInvitationOutcome>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<BatchInvitationOutcome>>('cmd_create_invitation_batch', {
    workspaceId,
    emails,
    role,
    idempotencyKey,
  });
}

export async function openLiveDraft(
  workspaceId: string,
  pageId: string,
  initialUpdateB64?: string | null,
  expectedBaseRevisionNumber?: number | null,
): Promise<WorkspaceResponse<OpenLiveDraftOutcome>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<OpenLiveDraftOutcome>>('cmd_open_live_draft', {
    workspaceId,
    pageId,
    initialUpdateB64: initialUpdateB64 ?? null,
    expectedBaseRevisionNumber: expectedBaseRevisionNumber ?? null,
  });
}

export async function getLiveDraft(
  workspaceId: string,
  pageId: string,
): Promise<WorkspaceResponse<SharedLiveDraftView | null>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedLiveDraftView | null>>('cmd_get_live_draft', {
    workspaceId,
    pageId,
  });
}

export async function getLiveDraftDiff(
  workspaceId: string,
  pageId: string,
  clientStateVectorB64?: string | null,
): Promise<WorkspaceResponse<LiveDraftDiffResponse | null>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<LiveDraftDiffResponse | null>>('cmd_get_live_draft_diff', {
    workspaceId,
    pageId,
    clientStateVectorB64: clientStateVectorB64 ?? null,
  });
}

export async function applyLiveDraftUpdateHttp(
  workspaceId: string,
  pageId: string,
  draftId: string,
  draftVersion: number,
  clientConnId: string,
  clientOpSeq: number,
  updateB64: string,
): Promise<WorkspaceResponse<LiveDraftUpdateOutcome>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<LiveDraftUpdateOutcome>>('cmd_apply_live_draft_update', {
    workspaceId,
    pageId,
    draftId,
    draftVersion,
    clientConnId,
    clientOpSeq,
    updateB64,
  });
}

export async function submitLiveDraft(
  workspaceId: string,
  pageId: string,
  expectedDraftId: string,
  expectedDraftVersion: number,
  expectedCheckpointId: string,
  summary: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedProposalDetails>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedProposalDetails>>('cmd_submit_live_draft', {
    workspaceId,
    pageId,
    expectedDraftId,
    expectedDraftVersion,
    expectedCheckpointId,
    summary,
    idempotencyKey,
  });
}

export async function rebaseLiveDraft(
  workspaceId: string,
  pageId: string,
  params: {
    expectedDraftId: string;
    expectedDraftVersion: number;
    expectedCheckpointId: string;
    expectedBaseRevisionNumber: number;
    mode: LiveDraftRebaseMode;
    rebasedTitle?: string | null;
    rebasedContentMd?: string | null;
    idempotencyKey: string;
  },
): Promise<WorkspaceResponse<SharedLiveDraftView>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedLiveDraftView>>('cmd_rebase_live_draft', {
    workspaceId,
    pageId,
    expectedDraftId: params.expectedDraftId,
    expectedDraftVersion: params.expectedDraftVersion,
    expectedCheckpointId: params.expectedCheckpointId,
    expectedBaseRevisionNumber: params.expectedBaseRevisionNumber,
    mode: params.mode,
    rebasedTitle: params.rebasedTitle ?? null,
    rebasedContentMd: params.rebasedContentMd ?? null,
    idempotencyKey: params.idempotencyKey,
  });
}

export async function listProjectThreads(
  workspaceId: string,
  projectId: string,
  cursor?: string | null,
  limit?: number,
): Promise<WorkspaceResponse<PagedList<SharedProjectThreadView>>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<PagedList<SharedProjectThreadView>>>('cmd_list_project_threads', {
    workspaceId,
    projectId,
    cursor: cursor ?? null,
    limit: limit ?? null,
  });
}

export async function createProjectThread(
  workspaceId: string,
  projectId: string,
  body: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedProjectThreadView>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedProjectThreadView>>('cmd_create_project_thread', {
    workspaceId,
    projectId,
    body,
    idempotencyKey,
  });
}

export async function replyProjectThread(
  workspaceId: string,
  projectId: string,
  threadId: string,
  body: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedProjectThreadView>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedProjectThreadView>>('cmd_reply_project_thread', {
    workspaceId,
    projectId,
    threadId,
    body,
    idempotencyKey,
  });
}

export async function resolveProjectThread(
  workspaceId: string,
  projectId: string,
  threadId: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedProjectThreadView>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedProjectThreadView>>('cmd_resolve_project_thread', {
    workspaceId,
    projectId,
    threadId,
    idempotencyKey,
  });
}

export async function reopenProjectThread(
  workspaceId: string,
  projectId: string,
  threadId: string,
  idempotencyKey: string,
): Promise<WorkspaceResponse<SharedProjectThreadView>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<SharedProjectThreadView>>('cmd_reopen_project_thread', {
    workspaceId,
    projectId,
    threadId,
    idempotencyKey,
  });
}

export async function listRealtimeEvents(
  workspaceId: string,
  afterSeq: number,
  limit?: number,
): Promise<WorkspaceResponse<RealtimeCatchupEvent[]>> {
  if (!isTauri) return previewUnavailable();
  return invoke<WorkspaceResponse<RealtimeCatchupEvent[]>>('cmd_list_realtime_events', {
    workspaceId,
    afterSeq,
    limit: limit ?? null,
  });
}
