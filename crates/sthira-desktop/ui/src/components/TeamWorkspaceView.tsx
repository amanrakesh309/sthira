import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  Activity,
  ArrowRightLeft,
  Check,
  Copy,
  CopyPlus,
  FilePlus,
  FileText,
  FolderKanban,
  FolderPlus,
  HardDrive,
  LogOut,
  Mail,
  RefreshCw,
  Shield,
  Trash2,
  UserPlus,
  Users,
} from 'lucide-react';
import {
  acceptOwnershipTransfer,
  canAuthorSharedContent,
  cancelOwnershipTransfer,
  canRemoveTarget,
  canRenameWorkspace,
  changeMemberRole,
  createInvitation,
  editableRolesForTarget,
  getWorkspaceDetails,
  grantableInviteRoles,
  initiateOwnershipTransfer,
  leaveWorkspace,
  removeMember,
  renameWorkspace,
  replaceInvitation,
  revokeInvitation,
  WORKSPACE_ROLE_LABELS,
  type CreatedInvitation,
  type InvitationStatus,
  type WorkspaceDetails,
  type WorkspaceInvitationView,
  type WorkspaceListResponse,
  type WorkspaceMemberView,
  type WorkspaceRole,
  type WorkspaceSummary,
} from '../api/workspaces';
import type { GoogleSession } from './AccountModal';
import { HelpTopicButton } from './HelpTopicButton';
import { SharedActivityFeed, SharedPagesPanel } from './SharedPagesPanel';
import { SharedProjectsPanel, type ProjectOpenRequest } from './SharedProjectsPanel';

export type TeamWorkspaceTab = 'overview' | 'projects' | 'pages' | 'activity' | 'members';

interface TeamWorkspaceViewProps {
  workspace: WorkspaceSummary;
  initialTab?: TeamWorkspaceTab;
  currentUid?: string | null;
  onTabChange?: (tab: TeamWorkspaceTab) => void;
  registerBeforeLeave?: (guard: (() => Promise<boolean>) | null) => void;
  onSelectPersonal: (notice?: string) => void;
  onWorkspaceUpdated: (workspace: WorkspaceSummary) => void;
  onAccessLost: (message: string, list?: WorkspaceListResponse | null) => void;
  onSessionChange: (session: GoogleSession) => void;
  onRefreshList: () => Promise<void> | void;
}

type PendingConfirmation =
  | { kind: 'role'; member: WorkspaceMemberView; nextRole: WorkspaceRole }
  | { kind: 'remove'; member: WorkspaceMemberView }
  | { kind: 'leave' }
  | { kind: 'transfer-start'; member: WorkspaceMemberView }
  | { kind: 'transfer-accept'; transferId: string; fromName: string }
  | { kind: 'transfer-cancel'; transferId: string; toName: string }
  | { kind: 'invite-revoke'; invitation: WorkspaceInvitationView }
  | { kind: 'invite-replace'; invitation: WorkspaceInvitationView };

const INVITE_STATUS_LABELS: Record<InvitationStatus, string> = {
  pending: 'Pending',
  accepted: 'Accepted',
  expired: 'Expired',
  revoked: 'Revoked',
};

function formatShortDate(epochMs: number): string {
  if (!Number.isFinite(epochMs) || epochMs <= 0) return '—';
  try {
    return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' }).format(new Date(epochMs));
  } catch {
    return new Date(epochMs).toISOString().slice(0, 10);
  }
}

export const TeamWorkspaceView: React.FC<TeamWorkspaceViewProps> = ({
  workspace,
  initialTab = 'overview',
  currentUid = null,
  onTabChange,
  registerBeforeLeave,
  onSelectPersonal,
  onWorkspaceUpdated,
  onAccessLost,
  onSessionChange,
  onRefreshList,
}) => {
  const [tab, setTab] = useState<TeamWorkspaceTab>(initialTab);
  const [details, setDetails] = useState<WorkspaceDetails | null>(null);
  const [loading, setLoading] = useState(true);
  const [mutating, setMutating] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [cw2RefreshSignal, setCw2RefreshSignal] = useState(0);
  const [pendingOpenRequest, setPendingOpenRequest] = useState<{
    pageId?: string | null;
    proposalId?: string | null;
    mode?: 'newPage' | 'publishCopy';
  } | null>(null);
  const [pendingProjectRequest, setPendingProjectRequest] =
    useState<ProjectOpenRequest | null>(null);

  // Editable form state preserved across retryable failures
  const [renameDraft, setRenameDraft] = useState(workspace.name);
  const [inviteEmail, setInviteEmail] = useState('');
  const [inviteRole, setInviteRole] = useState<WorkspaceRole>('viewer');
  const [createdInvite, setCreatedInvite] = useState<CreatedInvitation | null>(null);
  const [copiedCode, setCopiedCode] = useState(false);
  const [roleDrafts, setRoleDrafts] = useState<Record<string, WorkspaceRole>>({});
  const [confirmation, setConfirmation] = useState<PendingConfirmation | null>(null);

  const callbacksRef = useRef({
    onWorkspaceUpdated,
    onAccessLost,
    onSessionChange,
  });
  callbacksRef.current = {
    onWorkspaceUpdated,
    onAccessLost,
    onSessionChange,
  };
  const loadSeqRef = useRef(0);

  const pagesBeforeLeaveRef = useRef<(() => Promise<boolean>) | null>(null);
  const registerPagesBeforeLeave = useCallback(
    (guard: (() => Promise<boolean>) | null) => {
      pagesBeforeLeaveRef.current = guard;
      registerBeforeLeave?.(guard);
    },
    [registerBeforeLeave],
  );

  useEffect(() => {
    setTab(initialTab);
  }, [initialTab]);

  const selectTab = async (next: TeamWorkspaceTab) => {
    if (next !== tab && pagesBeforeLeaveRef.current) {
      const ok = await pagesBeforeLeaveRef.current();
      if (!ok) return;
    }
    setTab(next);
    if (next === 'projects' || next === 'pages' || next === 'activity') {
      setCw2RefreshSignal((n) => n + 1);
    }
    onTabChange?.(next);
  };

  const applyDetails = useCallback((nextDetails: WorkspaceDetails) => {
    setDetails(nextDetails);
    setRenameDraft(nextDetails.workspace.name);
    callbacksRef.current.onWorkspaceUpdated(nextDetails.workspace);
    const nextDrafts: Record<string, WorkspaceRole> = {};
    for (const m of nextDetails.members) {
      nextDrafts[m.membershipId] = m.role;
    }
    setRoleDrafts(nextDrafts);
    const allowedInviteRoles = grantableInviteRoles(nextDetails.workspace.myRole);
    if (allowedInviteRoles.length > 0) {
      setInviteRole((prev) =>
        allowedInviteRoles.includes(prev)
          ? prev
          : allowedInviteRoles[allowedInviteRoles.length - 1],
      );
    }
  }, []);

  const loadDetails = useCallback(async () => {
    const seq = ++loadSeqRef.current;
    setLoading(true);
    setError('');
    try {
      const res = await getWorkspaceDetails(workspace.workspaceId);
      if (seq !== loadSeqRef.current) return;
      if (res.outcome === 'ok') {
        applyDetails(res.data);
        return;
      }
      if (res.outcome === 'accessLost') {
        callbacksRef.current.onAccessLost(res.message, res.workspaces);
        return;
      }
      if (res.outcome === 'sessionUpdated') {
        callbacksRef.current.onSessionChange(res.session);
        setError(
          res.session.status === 'unverified'
            ? res.session.message
            : 'Sign in with Google to open team workspaces.',
        );
        return;
      }
      if (res.outcome === 'error') {
        setError(res.message);
      }
    } catch (err) {
      if (seq !== loadSeqRef.current) return;
      setError(typeof err === 'string' ? err : 'Could not load workspace details.');
    } finally {
      if (seq === loadSeqRef.current) {
        setLoading(false);
      }
    }
  }, [applyDetails, workspace.workspaceId]);

  useEffect(() => {
    void loadDetails();
    return () => {
      loadSeqRef.current += 1;
    };
  }, [loadDetails]);

  useEffect(() => {
    if (details && details.workspace.myRole !== workspace.myRole) {
      void loadDetails();
    }
  }, [details, loadDetails, workspace.myRole]);

  const handleWorkspaceResponseError = async (
    res:
      | { outcome: 'sessionUpdated'; session: GoogleSession }
      | { outcome: 'accessLost'; message: string; workspaces?: WorkspaceListResponse | null }
      | { outcome: 'error'; code: string; message: string },
  ) => {
    if (res.outcome === 'accessLost') {
      onAccessLost(res.message, res.workspaces);
      return;
    }
    if (res.outcome === 'sessionUpdated') {
      onSessionChange(res.session);
      setError(
        res.session.status === 'unverified'
          ? res.session.message
          : 'Sign in again with Google to continue.',
      );
      return;
    }
    setError(res.message);
    if (res.code === 'forbidden' || res.code === 'transfer_not_pending') {
      await loadDetails();
    }
  };

  const currentSummary = details?.workspace ?? workspace;
  const myRole = currentSummary.myRole;
  const canRename = canRenameWorkspace(myRole);
  const allowedInviteRoles = grantableInviteRoles(myRole);
  const canManageInvites = allowedInviteRoles.length > 0;
  const pendingTransfer = details?.pendingTransfer ?? null;
  const myMember = details?.members.find((m) => m.isCurrentUser) ?? null;
  const isTransferRecipient =
    !!pendingTransfer &&
    !!myMember &&
    pendingTransfer.toMembershipId === myMember.membershipId;

  const handleRenameSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    const trimmed = renameDraft.trim();
    if (!trimmed || mutating) return;
    setMutating(true);
    setError('');
    setNotice('');
    try {
      const res = await renameWorkspace(currentSummary.workspaceId, trimmed);
      if (res.outcome === 'ok') {
        onWorkspaceUpdated(res.data);
        setNotice(`Renamed workspace to “${res.data.name}”.`);
        await loadDetails();
        await onRefreshList();
      } else {
        await handleWorkspaceResponseError(res);
      }
    } catch (err) {
      setError(typeof err === 'string' ? err : 'Could not rename workspace.');
    } finally {
      setMutating(false);
    }
  };

  const handleCreateInvite = async (event: React.FormEvent) => {
    event.preventDefault();
    const email = inviteEmail.trim();
    if (!email || mutating) return;
    setMutating(true);
    setError('');
    setNotice('');
    try {
      const res = await createInvitation(currentSummary.workspaceId, email, inviteRole);
      if (res.outcome === 'ok') {
        setCreatedInvite(res.data);
        setCopiedCode(false);
        setInviteEmail('');
        setNotice(
          `Invitation created for ${res.data.invitation.recipientEmail} (${WORKSPACE_ROLE_LABELS[res.data.invitation.offeredRole]}). Share the single-use code directly.`,
        );
        await loadDetails();
      } else {
        await handleWorkspaceResponseError(res);
      }
    } catch (err) {
      setError(typeof err === 'string' ? err : 'Could not create invitation.');
    } finally {
      setMutating(false);
    }
  };

  const executeConfirmation = async () => {
    if (!confirmation || mutating) return;
    setMutating(true);
    setError('');
    setNotice('');
    try {
      switch (confirmation.kind) {
        case 'role': {
          const res = await changeMemberRole(
            currentSummary.workspaceId,
            confirmation.member.membershipId,
            confirmation.nextRole,
          );
          if (res.outcome === 'ok') {
            applyDetails(res.data);
            setConfirmation(null);
            setNotice(
              `Updated ${confirmation.member.displayName} to ${WORKSPACE_ROLE_LABELS[confirmation.nextRole]}.`,
            );
            await onRefreshList();
          } else {
            setConfirmation(null);
            await handleWorkspaceResponseError(res);
          }
          break;
        }
        case 'remove': {
          const res = await removeMember(
            currentSummary.workspaceId,
            confirmation.member.membershipId,
          );
          if (res.outcome === 'ok') {
            applyDetails(res.data);
            setConfirmation(null);
            setNotice(`Removed ${confirmation.member.displayName} from ${currentSummary.name}.`);
            await onRefreshList();
          } else {
            setConfirmation(null);
            await handleWorkspaceResponseError(res);
          }
          break;
        }
        case 'leave': {
          const res = await leaveWorkspace(currentSummary.workspaceId);
          if (res.outcome === 'ok') {
            setConfirmation(null);
            await onRefreshList();
            onSelectPersonal(`Left “${currentSummary.name}”.`);
          } else {
            setConfirmation(null);
            await handleWorkspaceResponseError(res);
          }
          break;
        }
        case 'transfer-start': {
          const res = await initiateOwnershipTransfer(
            currentSummary.workspaceId,
            confirmation.member.membershipId,
          );
          if (res.outcome === 'ok') {
            applyDetails(res.data);
            setConfirmation(null);
            setNotice(
              `Ownership transfer proposed to ${confirmation.member.displayName}. You stay Owner until they accept.`,
            );
          } else {
            setConfirmation(null);
            await handleWorkspaceResponseError(res);
          }
          break;
        }
        case 'transfer-accept': {
          const res = await acceptOwnershipTransfer(
            currentSummary.workspaceId,
            confirmation.transferId,
          );
          if (res.outcome === 'ok') {
            applyDetails(res.data);
            setConfirmation(null);
            setNotice(
              `You are now the Owner of ${currentSummary.name}. ${confirmation.fromName} became Admin.`,
            );
            await onRefreshList();
          } else {
            setConfirmation(null);
            await handleWorkspaceResponseError(res);
          }
          break;
        }
        case 'transfer-cancel': {
          const res = await cancelOwnershipTransfer(
            currentSummary.workspaceId,
            confirmation.transferId,
          );
          if (res.outcome === 'ok') {
            applyDetails(res.data);
            setConfirmation(null);
            setNotice(`Cancelled ownership transfer to ${confirmation.toName}.`);
          } else {
            setConfirmation(null);
            await handleWorkspaceResponseError(res);
          }
          break;
        }
        case 'invite-revoke': {
          const res = await revokeInvitation(
            currentSummary.workspaceId,
            confirmation.invitation.invitationId,
          );
          if (res.outcome === 'ok') {
            setConfirmation(null);
            setNotice(`Revoked invitation for ${confirmation.invitation.recipientEmail}.`);
            await loadDetails();
          } else {
            setConfirmation(null);
            await handleWorkspaceResponseError(res);
          }
          break;
        }
        case 'invite-replace': {
          const res = await replaceInvitation(
            currentSummary.workspaceId,
            confirmation.invitation.invitationId,
          );
          if (res.outcome === 'ok') {
            setCreatedInvite(res.data);
            setCopiedCode(false);
            setConfirmation(null);
            setNotice(`Issued a fresh invitation code for ${res.data.invitation.recipientEmail}.`);
            await loadDetails();
          } else {
            setConfirmation(null);
            await handleWorkspaceResponseError(res);
          }
          break;
        }
      }
    } catch (err) {
      setError(typeof err === 'string' ? err : 'Workspace update failed.');
    } finally {
      setMutating(false);
    }
  };

  const copyInviteCode = async () => {
    if (!createdInvite) return;
    try {
      await navigator.clipboard.writeText(createdInvite.inviteCode);
      setCopiedCode(true);
    } catch {
      setCopiedCode(false);
    }
  };

  return (
    <div className="flex-1 min-h-0 flex flex-col bg-sthira-canvas text-sthira-text overflow-y-auto">
      {/* Header */}
      <header
        className="min-h-[56px] px-6 py-2.5 border-b border-sthira-border bg-sthira-canvas flex flex-wrap items-center justify-between gap-3"
        aria-label="Team workspace header"
      >
        <div className="flex flex-wrap items-center gap-2.5 min-w-0">
          <Users className="w-4 h-4 text-sthira-primary shrink-0" />
          <h1 className="text-base font-semibold truncate">{currentSummary.name}</h1>
          <span className="rounded px-2 py-0.5 text-[11px] font-medium border border-sthira-border bg-sthira-surface text-sthira-text-muted">
            {WORKSPACE_ROLE_LABELS[myRole]}
          </span>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <HelpTopicButton article="workspaces" label="Help" />
          <button
            type="button"
            onClick={() => {
              void loadDetails();
              setCw2RefreshSignal((n) => n + 1);
            }}
            disabled={loading || mutating}
            className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
            <span>Refresh</span>
          </button>
          <button
            type="button"
            onClick={() => onSelectPersonal()}
            className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
          >
            <HardDrive className="w-3.5 h-3.5" />
            <span>Switch to Personal</span>
          </button>
        </div>
      </header>

      {/* Sub-navigation */}
      <div className="px-6 pt-4 border-b border-sthira-border bg-sthira-canvas">
        <div role="tablist" aria-label="Workspace sections" className="flex flex-wrap gap-2 pb-3">
          <button
            type="button"
            role="tab"
            aria-selected={tab === 'overview'}
            onClick={() => void selectTab('overview')}
            className={`min-h-[38px] px-4 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
              tab === 'overview'
                ? 'bg-sthira-primary text-sthira-on-primary'
                : 'bg-sthira-surface border border-sthira-border text-sthira-text hover:bg-sthira-surface-muted'
            }`}
          >
            Overview
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === 'projects'}
            onClick={() => void selectTab('projects')}
            className={`min-h-[38px] px-4 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
              tab === 'projects'
                ? 'bg-sthira-primary text-sthira-on-primary'
                : 'bg-sthira-surface border border-sthira-border text-sthira-text hover:bg-sthira-surface-muted'
            }`}
          >
            Projects
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === 'pages'}
            onClick={() => void selectTab('pages')}
            className={`min-h-[38px] px-4 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
              tab === 'pages'
                ? 'bg-sthira-primary text-sthira-on-primary'
                : 'bg-sthira-surface border border-sthira-border text-sthira-text hover:bg-sthira-surface-muted'
            }`}
          >
            Pages
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === 'activity'}
            onClick={() => selectTab('activity')}
            className={`min-h-[38px] px-4 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
              tab === 'activity'
                ? 'bg-sthira-primary text-sthira-on-primary'
                : 'bg-sthira-surface border border-sthira-border text-sthira-text hover:bg-sthira-surface-muted'
            }`}
          >
            Activity
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === 'members'}
            onClick={() => selectTab('members')}
            className={`min-h-[38px] px-4 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
              tab === 'members'
                ? 'bg-sthira-primary text-sthira-on-primary'
                : 'bg-sthira-surface border border-sthira-border text-sthira-text hover:bg-sthira-surface-muted'
            }`}
          >
            Workspace settings · Members
          </button>
        </div>
      </div>

      {/* Main Content */}
      <div className="w-full max-w-4xl mx-auto p-6 space-y-5 text-xs">
        {notice && (
          <p
            role="status"
            className="rounded-xl border border-sthira-border bg-sthira-surface px-4 py-3 text-sthira-text"
          >
            {notice}
          </p>
        )}
        {error && (
          <div
            role="alert"
            className="rounded-xl border border-sthira-border bg-sthira-surface px-4 py-3 text-sthira-danger flex flex-wrap items-center justify-between gap-2"
          >
            <span>{error}</span>
            <button
              type="button"
              onClick={() => void loadDetails()}
              className="sthira-button-secondary cursor-pointer"
            >
              Retry
            </button>
          </div>
        )}

        {isTransferRecipient && pendingTransfer && (
          <section
            aria-label="Incoming ownership transfer"
            className="rounded-2xl border border-sthira-border bg-sthira-surface p-4 space-y-3"
          >
            <div className="flex items-start gap-2.5">
              <ArrowRightLeft className="w-4 h-4 text-sthira-primary shrink-0 mt-0.5" />
              <div className="space-y-1">
                <h2 className="text-sm font-semibold">Ownership transfer pending</h2>
                <p className="text-sthira-text-muted leading-relaxed">
                  <strong>{pendingTransfer.fromDisplayName}</strong> proposed transferring ownership of{' '}
                  <strong>{pendingTransfer.workspaceName}</strong> to you. Accepting makes you{' '}
                  <strong>Owner</strong> and moves them to <strong>Admin</strong>.
                </p>
              </div>
            </div>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                disabled={mutating}
                onClick={() =>
                  setConfirmation({
                    kind: 'transfer-accept',
                    transferId: pendingTransfer.transferId,
                    fromName: pendingTransfer.fromDisplayName,
                  })
                }
                className="sthira-button-primary cursor-pointer"
              >
                Accept ownership
              </button>
            </div>
          </section>
        )}

        {confirmation && (
          <div
            role="region"
            aria-label="Confirm workspace action"
            className="rounded-2xl border border-sthira-border bg-sthira-surface p-4 space-y-3 shadow-md"
          >
            <p className="text-sm font-medium text-sthira-text">
              {confirmation.kind === 'role' &&
                `Change ${confirmation.member.displayName}’s role from ${WORKSPACE_ROLE_LABELS[confirmation.member.role]} to ${WORKSPACE_ROLE_LABELS[confirmation.nextRole]}?`}
              {confirmation.kind === 'remove' &&
                `Remove ${confirmation.member.displayName} from ${currentSummary.name}? Their access ends on their next check, and a used invitation cannot restore membership.`}
              {confirmation.kind === 'leave' &&
                `Leave ${currentSummary.name}? You will return to Personal and need a new invitation to rejoin.`}
              {confirmation.kind === 'transfer-start' &&
                `Propose transferring ownership of ${currentSummary.name} to ${confirmation.member.displayName}? You remain Owner until they accept, after which you become Admin.`}
              {confirmation.kind === 'transfer-accept' &&
                `Accept ownership of ${currentSummary.name}? You will become Owner and ${confirmation.fromName} will become Admin.`}
              {confirmation.kind === 'transfer-cancel' &&
                `Cancel the pending ownership transfer to ${confirmation.toName}?`}
              {confirmation.kind === 'invite-revoke' &&
                `Revoke the pending invitation for ${confirmation.invitation.recipientEmail}?`}
              {confirmation.kind === 'invite-replace' &&
                `Revoke the current code for ${confirmation.invitation.recipientEmail} and issue a fresh 7-day code?`}
            </p>
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                disabled={mutating}
                onClick={() => void executeConfirmation()}
                className="sthira-button-primary cursor-pointer"
              >
                {mutating ? 'Applying…' : 'Confirm'}
              </button>
              <button
                type="button"
                disabled={mutating}
                onClick={() => setConfirmation(null)}
                className="sthira-button-secondary cursor-pointer"
              >
                Cancel
              </button>
            </div>
          </div>
        )}

        {tab === 'overview' && (
          <section className="rounded-2xl border border-sthira-border bg-sthira-surface p-6 space-y-4">
            <div className="space-y-1.5">
              <h2 className="text-lg font-semibold font-editorial">{currentSummary.name}</h2>
              <p className="text-sthira-text-muted leading-relaxed">
                Collaborate on shared pages, discussions, and reviewed changes. Your Personal vault stays local and is never uploaded or shared without an explicit reviewed copy.
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-2.5 pt-1">
              <button
                type="button"
                onClick={() => void selectTab('projects')}
                className="sthira-button-primary inline-flex items-center gap-1.5 cursor-pointer"
              >
                <FolderKanban className="w-4 h-4" />
                <span>Open Projects</span>
              </button>
              <button
                type="button"
                onClick={() => void selectTab('pages')}
                className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
              >
                <FileText className="w-4 h-4" />
                <span>Open Pages</span>
              </button>
              {canAuthorSharedContent(myRole) && (
                <>
                  <button
                    type="button"
                    onClick={() => {
                      setPendingProjectRequest({ mode: 'newProject' });
                      void selectTab('projects');
                    }}
                    className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
                  >
                    <FolderPlus className="w-4 h-4" />
                    <span>New project</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setPendingOpenRequest({ mode: 'newPage' });
                      void selectTab('pages');
                    }}
                    className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
                  >
                    <FilePlus className="w-4 h-4" />
                    <span>New shared page</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setPendingOpenRequest({ mode: 'publishCopy' });
                      selectTab('pages');
                    }}
                    className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
                  >
                    <CopyPlus className="w-4 h-4" />
                    <span>Publish a copy</span>
                  </button>
                </>
              )}
              <button
                type="button"
                onClick={() => selectTab('activity')}
                className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
              >
                <Activity className="w-4 h-4" />
                <span>Activity</span>
              </button>
              {canManageInvites ? (
                <button
                  type="button"
                  onClick={() => selectTab('members')}
                  className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
                >
                  <UserPlus className="w-4 h-4" />
                  <span>Invite people</span>
                </button>
              ) : (
                <button
                  type="button"
                  onClick={() => selectTab('members')}
                  className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
                >
                  <Users className="w-4 h-4" />
                  <span>View members</span>
                </button>
              )}
              <button
                type="button"
                onClick={() => onSelectPersonal()}
                className="sthira-button-secondary cursor-pointer"
              >
                Return to Personal
              </button>
            </div>
          </section>
        )}

        {tab === 'projects' && (
          <SharedProjectsPanel
            workspaceId={currentSummary.workspaceId}
            workspaceName={currentSummary.name}
            myRole={myRole}
            currentUid={currentUid ?? myMember?.uid ?? null}
            authorDisplayName={myMember?.displayName}
            refreshSignal={cw2RefreshSignal}
            pendingOpenRequest={pendingProjectRequest}
            onConsumedOpenRequest={() => setPendingProjectRequest(null)}
            onFallbackToPages={(pageId, proposalId) => {
              setPendingOpenRequest({ pageId, proposalId });
              void selectTab('pages');
            }}
            registerBeforeLeave={registerPagesBeforeLeave}
            onAccessLost={onAccessLost}
            onSessionChange={onSessionChange}
            onErrorNotice={setError}
          />
        )}

        {tab === 'pages' && (
          <SharedPagesPanel
            workspaceId={currentSummary.workspaceId}
            workspaceName={currentSummary.name}
            myRole={myRole}
            currentUid={currentUid ?? myMember?.uid ?? null}
            authorDisplayName={myMember?.displayName}
            refreshSignal={cw2RefreshSignal}
            pendingOpenRequest={pendingOpenRequest}
            onConsumedOpenRequest={() => setPendingOpenRequest(null)}
            registerBeforeLeave={registerPagesBeforeLeave}
            onAccessLost={onAccessLost}
            onSessionChange={onSessionChange}
            onErrorNotice={setError}
          />
        )}

        {tab === 'activity' && (
          <SharedActivityFeed
            workspaceId={currentSummary.workspaceId}
            refreshSignal={cw2RefreshSignal}
            onOpenPage={(pageId, proposalId) => {
              setPendingOpenRequest({ pageId, proposalId });
              void selectTab('pages');
            }}
            onOpenProject={(projectId, pageId, proposalId) => {
              setPendingProjectRequest({ projectId, pageId, proposalId });
              void selectTab('projects');
            }}
            onAccessLost={onAccessLost}
            onSessionChange={onSessionChange}
          />
        )}

        {tab === 'members' && (
          <div className="space-y-5">
            {/* Workspace Name / Settings */}
            <section className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-3">
              <h2 className="text-sm font-semibold">Workspace settings</h2>
              {canRename ? (
                <form onSubmit={(e) => void handleRenameSubmit(e)} className="flex flex-wrap items-end gap-3">
                  <label className="flex-1 min-w-[220px] space-y-1">
                    <span className="text-sthira-text-muted">Workspace name</span>
                    <input
                      aria-label="Workspace name"
                      type="text"
                      maxLength={80}
                      required
                      disabled={mutating}
                      value={renameDraft}
                      onChange={(e) => setRenameDraft(e.target.value)}
                      className="w-full min-h-[40px] rounded-lg border border-sthira-border bg-sthira-canvas px-3 text-sm text-sthira-text"
                    />
                  </label>
                  <button
                    type="submit"
                    disabled={mutating || !renameDraft.trim() || renameDraft.trim() === currentSummary.name}
                    className="sthira-button-primary cursor-pointer"
                  >
                    Save name
                  </button>
                </form>
              ) : (
                <p className="text-sthira-text-muted">
                  Workspace: <strong className="text-sthira-text">{currentSummary.name}</strong> · Your role:{' '}
                  <strong className="text-sthira-text">{WORKSPACE_ROLE_LABELS[myRole]}</strong>
                </p>
              )}
            </section>

            {/* Pending Transfer (Owner view) */}
            {pendingTransfer && myRole === 'owner' && (
              <section
                aria-label="Pending ownership transfer"
                className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 flex flex-wrap items-center justify-between gap-3"
              >
                <div className="space-y-0.5">
                  <h3 className="text-sm font-semibold">Ownership transfer pending</h3>
                  <p className="text-sthira-text-muted">
                    Waiting for <strong>{pendingTransfer.toDisplayName}</strong> to accept ownership. You remain Owner until they accept.
                  </p>
                </div>
                <button
                  type="button"
                  disabled={mutating}
                  onClick={() =>
                    setConfirmation({
                      kind: 'transfer-cancel',
                      transferId: pendingTransfer.transferId,
                      toName: pendingTransfer.toDisplayName,
                    })
                  }
                  className="sthira-button-secondary cursor-pointer"
                >
                  Cancel transfer
                </button>
              </section>
            )}

            {/* Members Roster */}
            <section className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-3">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <h2 className="text-sm font-semibold flex items-center gap-1.5">
                  <Shield className="w-4 h-4 text-sthira-primary" />
                  <span>Members ({details?.members.length ?? 0})</span>
                </h2>
              </div>

              {loading && !details ? (
                <p role="status" className="text-sthira-text-muted">
                  Loading members…
                </p>
              ) : (
                <ul className="space-y-2.5" aria-label="Workspace members">
                  {(details?.members || []).map((member) => {
                    const editableRoles = editableRolesForTarget(
                      myRole,
                      member.role,
                      member.isCurrentUser,
                    );
                    const removable = canRemoveTarget(myRole, member.role, member.isCurrentUser);
                    const canTransfer =
                      myRole === 'owner' && !member.isCurrentUser && !pendingTransfer;
                    const draftedRole = roleDrafts[member.membershipId] ?? member.role;

                    return (
                      <li
                        key={member.membershipId}
                        className="rounded-xl border border-sthira-border bg-sthira-canvas p-3.5 flex flex-wrap items-center justify-between gap-3"
                      >
                        <div className="min-w-0 space-y-0.5">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="font-medium text-sm text-sthira-text break-words">
                              {member.displayName}
                            </span>
                            <span className="rounded px-1.5 py-0.5 text-[11px] font-medium border border-sthira-border bg-sthira-surface text-sthira-text">
                              {WORKSPACE_ROLE_LABELS[member.role]}
                            </span>
                            {member.isCurrentUser && (
                              <span className="rounded px-1.5 py-0.5 text-[11px] border border-sthira-border text-sthira-text-muted">
                                You
                              </span>
                            )}
                          </div>
                          <p className="text-sthira-text-muted">
                            Joined {formatShortDate(member.joinedAtMs)}
                          </p>
                        </div>

                        <div className="flex flex-wrap items-center gap-2">
                          {editableRoles.length > 0 && (
                            <div className="flex flex-wrap items-center gap-1.5">
                              <select
                                aria-label={`Role for ${member.displayName}`}
                                disabled={mutating}
                                value={draftedRole}
                                onChange={(e) =>
                                  setRoleDrafts((prev) => ({
                                    ...prev,
                                    [member.membershipId]: e.target.value as WorkspaceRole,
                                  }))
                                }
                                className="min-h-[38px] rounded-lg border border-sthira-border bg-sthira-surface px-2.5 text-xs text-sthira-text"
                              >
                                {editableRoles.map((roleOpt) => (
                                  <option key={roleOpt} value={roleOpt}>
                                    {WORKSPACE_ROLE_LABELS[roleOpt]}
                                  </option>
                                ))}
                              </select>
                              <button
                                type="button"
                                aria-label={`Change role for ${member.displayName}`}
                                disabled={mutating || draftedRole === member.role}
                                onClick={() =>
                                  setConfirmation({
                                    kind: 'role',
                                    member,
                                    nextRole: draftedRole,
                                  })
                                }
                                className="sthira-button-secondary cursor-pointer"
                              >
                                Change role
                              </button>
                            </div>
                          )}

                          {canTransfer && (
                            <button
                              type="button"
                              disabled={mutating}
                              aria-label={`Transfer ownership to ${member.displayName}`}
                              onClick={() =>
                                setConfirmation({ kind: 'transfer-start', member })
                              }
                              className="sthira-button-secondary cursor-pointer"
                            >
                              Transfer ownership
                            </button>
                          )}

                          {removable && (
                            <button
                              type="button"
                              disabled={mutating}
                              aria-label={`Remove ${member.displayName}`}
                              onClick={() => setConfirmation({ kind: 'remove', member })}
                              className="sthira-button-secondary inline-flex items-center gap-1 cursor-pointer"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                              <span>Remove</span>
                            </button>
                          )}
                        </div>
                      </li>
                    );
                  })}
                </ul>
              )}
            </section>

            {/* Invitations (Managers only) */}
            {canManageInvites && (
              <section
                aria-label="Invitations"
                className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-4"
              >
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <h2 className="text-sm font-semibold flex items-center gap-1.5">
                    <Mail className="w-4 h-4 text-sthira-primary" />
                    <span>Invitations</span>
                  </h2>
                  <span className="text-sthira-text-muted">Single-use codes · 7-day expiry</span>
                </div>

                <form
                  onSubmit={(e) => void handleCreateInvite(e)}
                  className="flex flex-wrap items-end gap-3"
                >
                  <label className="flex-1 min-w-[220px] space-y-1">
                    <span className="text-sthira-text-muted">Recipient email</span>
                    <input
                      aria-label="Recipient email"
                      type="email"
                      required
                      disabled={mutating}
                      value={inviteEmail}
                      onChange={(e) => setInviteEmail(e.target.value)}
                      placeholder="teammate@example.com"
                      className="w-full min-h-[40px] rounded-lg border border-sthira-border bg-sthira-canvas px-3 text-sm text-sthira-text"
                    />
                  </label>

                  <label className="w-36 space-y-1">
                    <span className="text-sthira-text-muted">Role</span>
                    <select
                      aria-label="Invitation role"
                      disabled={mutating}
                      value={inviteRole}
                      onChange={(e) => setInviteRole(e.target.value as WorkspaceRole)}
                      className="w-full min-h-[40px] rounded-lg border border-sthira-border bg-sthira-surface px-2.5 text-xs text-sthira-text"
                    >
                      {allowedInviteRoles.map((roleOpt) => (
                        <option key={roleOpt} value={roleOpt}>
                          {WORKSPACE_ROLE_LABELS[roleOpt]}
                        </option>
                      ))}
                    </select>
                  </label>

                  <button
                    type="submit"
                    disabled={mutating || !inviteEmail.trim()}
                    className="sthira-button-primary cursor-pointer"
                  >
                    Create invitation
                  </button>
                </form>

                {createdInvite && (
                  <div
                    role="region"
                    aria-label="New invitation code"
                    className="rounded-xl border border-sthira-border bg-sthira-canvas p-4 space-y-2.5"
                  >
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <span className="font-semibold text-sthira-text">
                        Share invitation code for {createdInvite.invitation.recipientEmail} (
                        {WORKSPACE_ROLE_LABELS[createdInvite.invitation.offeredRole]})
                      </span>
                      <button
                        type="button"
                        onClick={() => setCreatedInvite(null)}
                        className="text-sthira-text-muted hover:text-sthira-text underline cursor-pointer"
                      >
                        Dismiss
                      </button>
                    </div>
                    <div className="flex flex-wrap items-center gap-2">
                      <code
                        data-testid="generated-invite-code"
                        className="flex-1 min-w-[200px] rounded-lg border border-sthira-border bg-sthira-surface px-3 py-2 font-mono text-xs break-all select-all"
                      >
                        {createdInvite.inviteCode}
                      </code>
                      <button
                        type="button"
                        onClick={() => void copyInviteCode()}
                        className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
                      >
                        {copiedCode ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
                        <span>{copiedCode ? 'Copied' : 'Copy code'}</span>
                      </button>
                    </div>
                    <p className="text-sthira-text-muted">
                      Expires {formatShortDate(createdInvite.invitation.expiresAtMs)}. Send this code through your own channel; it is only shown once.
                    </p>
                  </div>
                )}

                {details && details.invitations.length > 0 && (
                  <ul className="space-y-2" aria-label="Workspace invitations">
                    {details.invitations.map((inv) => (
                      <li
                        key={inv.invitationId}
                        className="rounded-xl border border-sthira-border bg-sthira-canvas p-3 flex flex-wrap items-center justify-between gap-2.5"
                      >
                        <div className="min-w-0 space-y-0.5">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="font-medium text-sthira-text break-all">
                              {inv.recipientEmail}
                            </span>
                            <span className="rounded px-1.5 py-0.5 text-[11px] border border-sthira-border text-sthira-text-muted">
                              {WORKSPACE_ROLE_LABELS[inv.offeredRole]}
                            </span>
                            <span className="rounded px-1.5 py-0.5 text-[11px] border border-sthira-border bg-sthira-surface text-sthira-text">
                              {INVITE_STATUS_LABELS[inv.status]}
                            </span>
                          </div>
                          <p className="text-sthira-text-muted">
                            Invited by {inv.invitedByDisplayName} · Expires{' '}
                            {formatShortDate(inv.expiresAtMs)}
                          </p>
                        </div>

                        {inv.status === 'pending' && (
                          <div className="flex flex-wrap items-center gap-2">
                            <button
                              type="button"
                              disabled={mutating}
                              onClick={() =>
                                setConfirmation({ kind: 'invite-replace', invitation: inv })
                              }
                              className="sthira-button-secondary cursor-pointer"
                            >
                              Replace
                            </button>
                            <button
                              type="button"
                              disabled={mutating}
                              onClick={() =>
                                setConfirmation({ kind: 'invite-revoke', invitation: inv })
                              }
                              className="sthira-button-secondary cursor-pointer"
                            >
                              Revoke
                            </button>
                          </div>
                        )}
                      </li>
                    ))}
                  </ul>
                )}
              </section>
            )}

            {/* Leave workspace */}
            <section className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 flex flex-wrap items-center justify-between gap-3">
              <div className="space-y-0.5">
                <h3 className="text-sm font-semibold">Leave workspace</h3>
                <p className="text-sthira-text-muted">
                  {myRole === 'owner'
                    ? 'Transfer ownership to another member before leaving this workspace.'
                    : 'Leave this workspace and return to Personal.'}
                </p>
              </div>
              <button
                type="button"
                disabled={mutating || myRole === 'owner'}
                onClick={() => setConfirmation({ kind: 'leave' })}
                className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
              >
                <LogOut className="w-3.5 h-3.5" />
                <span>Leave workspace</span>
              </button>
            </section>
          </div>
        )}
      </div>
    </div>
  );
};
