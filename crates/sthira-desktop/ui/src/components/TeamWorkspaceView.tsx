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
  createInvitationBatch,
  editableRolesForTarget,
  getWorkspaceDetails,
  grantableInviteRoles,
  initiateOwnershipTransfer,
  leaveWorkspace,
  listRealtimeEvents,
  makeRequestIdempotencyKey,
  removeMember,
  renameWorkspace,
  replaceInvitation,
  revokeInvitation,
  WORKSPACE_ROLE_LABELS,
  type BatchInvitationOutcome,
  type CreatedInvitation,
  type InvitationStatus,
  type WorkspaceDetails,
  type WorkspaceInvitationView,
  type WorkspaceListResponse,
  type WorkspaceMemberView,
  type WorkspaceRole,
  type WorkspaceSummary,
} from '../api/workspaces';
import { collabConnect, collabDisconnect, subscribeCollabEvents } from '../api/liveCollab';
import type { GoogleSession } from './AccountModal';
import { HelpTopicButton } from './HelpTopicButton';
import { SharedActivityFeed, SharedPagesPanel } from './SharedPagesPanel';
import { SharedProjectsPanel, type ProjectOpenRequest } from './SharedProjectsPanel';

import { SharedAgentsPanel } from './SharedAgentsPanel';

export type TeamWorkspaceTab = 'overview' | 'projects' | 'pages' | 'activity' | 'members' | 'agents';

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
  const [agentProposalId, setAgentProposalId] = useState<string | null>(null);
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
    threadId?: string | null;
    mode?: 'newPage' | 'publishCopy';
  } | null>(null);
  const [pendingProjectRequest, setPendingProjectRequest] =
    useState<ProjectOpenRequest | null>(null);

  // Editable form state preserved across retryable failures
  const [renameDraft, setRenameDraft] = useState(workspace.name);
  const [inviteEmailsRaw, setInviteEmailsRaw] = useState('');
  const [inviteRole, setInviteRole] = useState<WorkspaceRole>('viewer');
  const [batchOutcome, setBatchOutcome] = useState<BatchInvitationOutcome | null>(null);
  const [copiedCodeKey, setCopiedCodeKey] = useState<string | null>(null);
  const [replacedInviteCodes, setReplacedInviteCodes] = useState<Record<string, CreatedInvitation>>({});
  const [roleDrafts, setRoleDrafts] = useState<Record<string, WorkspaceRole>>({});
  const [confirmation, setConfirmation] = useState<PendingConfirmation | null>(null);
  const lastEventSeqRef = useRef(0);
  const needsCatchupRef = useRef(false);
  const inviteBatchKeyRef = useRef<{ sig: string; key: string } | null>(null);

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
    let active = true;
    let revoked = false;
    let catchUpInFlight = false;
    let hasBaseline = false;
    let refreshPending = false;
    let retryAttempt = 0;
    let catchupAttempt = 0;
    let reconnectTimer: number | null = null;
    let catchupRetryTimer: number | null = null;
    const seenEventSeqs = new Set<number>();
    const bufferedEvents: Array<{ eventSeq: number }> = [];
    lastEventSeqRef.current = 0;
    needsCatchupRef.current = false;

    const DRAFT_ONLY_EVICT_CODES = new Set([
      'role_downgraded',
      'page_archived',
      'live_draft_closed',
      'shared_content_disabled',
      'stale_draft_scope',
    ]);

    const clearTimers = () => {
      if (reconnectTimer !== null) {
        window.clearTimeout(reconnectTimer);
        reconnectTimer = null;
      }
      if (catchupRetryTimer !== null) {
        window.clearTimeout(catchupRetryTimer);
        catchupRetryTimer = null;
      }
    };

    const recordSeenSeq = (seq: number) => {
      seenEventSeqs.add(seq);
      if (seenEventSeqs.size > 2000) {
        const oldest = seenEventSeqs.values().next();
        if (!oldest.done) seenEventSeqs.delete(oldest.value);
      }
      if (seq > lastEventSeqRef.current) {
        lastEventSeqRef.current = seq;
      }
    };

    const scheduleCatchupRetry = () => {
      if (!active || revoked || catchupRetryTimer !== null) return;
      const delays = [1000, 2000, 4000, 8000, 15000];
      const delayMs = delays[Math.min(catchupAttempt, delays.length - 1)];
      catchupAttempt += 1;
      catchupRetryTimer = window.setTimeout(() => {
        catchupRetryTimer = null;
        void runCatchup(false);
      }, delayMs);
    };

    const runCatchup = async (forceRefreshOnEmpty = false) => {
      if (!active || revoked || catchUpInFlight) return;
      if (catchupRetryTimer !== null) {
        window.clearTimeout(catchupRetryTimer);
        catchupRetryTimer = null;
      }
      catchUpInFlight = true;
      let cursor = lastEventSeqRef.current;
      let completedAllPages = true;
      const hadBaselineBefore = hasBaseline;
      try {
        while (active && !revoked) {
          const res = await listRealtimeEvents(workspace.workspaceId, cursor, 100);
          if (!active || revoked) return;
          if (res.outcome === 'accessLost') {
            revoked = true;
            clearTimers();
            callbacksRef.current.onAccessLost(res.message, res.workspaces);
            return;
          }
          if (res.outcome !== 'ok') {
            completedAllPages = false;
            break;
          }
          const batch = res.data;
          if (batch.length === 0) break;
          for (const missed of batch) {
            if (hadBaselineBefore && !seenEventSeqs.has(missed.eventSeq)) {
              refreshPending = true;
            }
            recordSeenSeq(missed.eventSeq);
            cursor = Math.max(cursor, missed.eventSeq);
          }
          if (batch.length < 100) break;
        }
        if (completedAllPages) {
          // Only fold buffered live events once catch-up pages succeeded contiguously;
          // draining buffered events across a failed page would skip intermediate sequences.
          while (bufferedEvents.length > 0) {
            const buffered = bufferedEvents.shift()!;
            if (!seenEventSeqs.has(buffered.eventSeq)) {
              refreshPending = true;
            }
            recordSeenSeq(buffered.eventSeq);
          }
          hasBaseline = true;
          catchupAttempt = 0;
          needsCatchupRef.current = false;
          if (
            active &&
            !revoked &&
            (refreshPending || (forceRefreshOnEmpty && hadBaselineBefore))
          ) {
            refreshPending = false;
            setCw2RefreshSignal((n) => n + 1);
            void loadDetails();
          }
        } else {
          needsCatchupRef.current = true;
          scheduleCatchupRetry();
        }
      } catch {
        needsCatchupRef.current = true;
        scheduleCatchupRetry();
      } finally {
        catchUpInFlight = false;
      }
    };

    const scheduleReconnect = () => {
      if (!active || revoked || reconnectTimer !== null) return;
      const delays = [1000, 2000, 4000, 8000, 15000];
      const delayMs = delays[Math.min(retryAttempt, delays.length - 1)];
      retryAttempt += 1;
      reconnectTimer = window.setTimeout(() => {
        reconnectTimer = null;
        void attemptConnect();
      }, delayMs);
    };

    const attemptConnect = async () => {
      if (!active || revoked) return;
      try {
        await collabConnect(workspace.workspaceId, currentUid);
        if (!active || revoked) return;
        retryAttempt = 0;
      } catch {
        if (!active || revoked) return;
        needsCatchupRef.current = true;
        scheduleReconnect();
      }
    };

    void runCatchup(false);
    void attemptConnect();

    let unlisten: (() => void) | null = null;
    void subscribeCollabEvents((ev) => {
      if (!active || revoked || ev.workspaceId !== workspace.workspaceId) return;
      if (ev.type === 'access_revoked') {
        if (DRAFT_ONLY_EVICT_CODES.has(ev.code)) {
          void loadDetails();
          return;
        }
        revoked = true;
        clearTimers();
        void loadDetails();
        return;
      }
      if (ev.type === 'ws_disconnected') {
        needsCatchupRef.current = true;
        scheduleReconnect();
      } else if (ev.type === 'connected') {
        retryAttempt = 0;
        if (reconnectTimer !== null) {
          window.clearTimeout(reconnectTimer);
          reconnectTimer = null;
        }
        if (needsCatchupRef.current) {
          void runCatchup(false);
        }
      } else if (ev.type === 'workspace_event') {
        if (catchUpInFlight || needsCatchupRef.current) {
          bufferedEvents.push({ eventSeq: ev.eventSeq });
        } else {
          recordSeenSeq(ev.eventSeq);
        }
      }
    }).then((fn) => {
      if (active) unlisten = fn;
      else fn();
    });

    return () => {
      active = false;
      clearTimers();
      unlisten?.();
      void collabDisconnect();
    };
  }, [currentUid, loadDetails, workspace.workspaceId]);

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

  const parseInviteInputEmails = (raw: string): string[] => {
    return raw
      .split(/[\n,;]+/)
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
  };

  const handleCreateInviteBatch = async (event: React.FormEvent) => {
    event.preventDefault();
    const emails = parseInviteInputEmails(inviteEmailsRaw);
    if (emails.length === 0 || mutating) return;
    const sig = `${currentSummary.workspaceId}:${inviteRole}:${emails.join('|').toLowerCase()}`;
    if (!inviteBatchKeyRef.current || inviteBatchKeyRef.current.sig !== sig) {
      inviteBatchKeyRef.current = { sig, key: makeRequestIdempotencyKey('invb') };
    }
    setMutating(true);
    setError('');
    setNotice('');
    try {
      const res = await createInvitationBatch(
        currentSummary.workspaceId,
        emails,
        inviteRole,
        inviteBatchKeyRef.current.key,
      );
      if (res.outcome === 'ok') {
        inviteBatchKeyRef.current = null;
        setBatchOutcome(res.data);
        setCopiedCodeKey(null);
        const createdCount = res.data.results.filter((r) => r.outcome === 'created').length;
        if (createdCount > 0) {
          setInviteEmailsRaw('');
        }
        setNotice(
          `Processed ${res.data.results.length} ${
            res.data.results.length === 1 ? 'recipient' : 'recipients'
          } (${createdCount} created as ${WORKSPACE_ROLE_LABELS[res.data.offeredRole]}).`,
        );
        await loadDetails();
      } else {
        await handleWorkspaceResponseError(res);
      }
    } catch (err) {
      setError(typeof err === 'string' ? err : 'Could not send workspace invitations.');
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
          const revokedId = confirmation.invitation.invitationId;
          const res = await revokeInvitation(
            currentSummary.workspaceId,
            revokedId,
          );
          if (res.outcome === 'ok') {
            setReplacedInviteCodes((prev) => {
              const next = { ...prev };
              delete next[revokedId];
              return next;
            });
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
          const oldId = confirmation.invitation.invitationId;
          const res = await replaceInvitation(
            currentSummary.workspaceId,
            oldId,
          );
          if (res.outcome === 'ok') {
            setReplacedInviteCodes((prev) => ({
              ...prev,
              [oldId]: res.data,
              [res.data.invitation.invitationId]: res.data,
            }));
            setBatchOutcome((prev) =>
              prev
                ? {
                    ...prev,
                    results: prev.results.map((item) =>
                      item.invitation?.invitationId === oldId
                        ? {
                            ...item,
                            invitation: res.data.invitation,
                            inviteCode: res.data.inviteCode,
                            message: `Issued fresh ${WORKSPACE_ROLE_LABELS[res.data.invitation.offeredRole]} code.`,
                          }
                        : item,
                    ),
                  }
                : null,
            );
            setCopiedCodeKey(null);
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

  const copyCodeForKey = async (key: string, code: string) => {
    try {
      await navigator.clipboard.writeText(code);
      setCopiedCodeKey(key);
    } catch {
      setCopiedCodeKey(null);
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
          <button type="button" role="tab" aria-selected={tab === 'agents'} onClick={() => void selectTab('agents')} className={`min-h-[38px] px-4 rounded-lg text-xs font-medium transition-colors cursor-pointer ${tab === 'agents' ? 'bg-sthira-primary text-sthira-on-primary' : 'bg-sthira-surface border border-sthira-border text-sthira-text hover:bg-sthira-surface-muted'}`}>Agents</button>
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

        {tab === 'agents' && <SharedAgentsPanel workspaceId={currentSummary.workspaceId} myRole={myRole} currentUid={currentUid ?? myMember?.uid ?? null} initialProposalId={agentProposalId} onSessionChange={onSessionChange} onAccessLost={onAccessLost} onOpenPage={(pageId, projectId) => {
          if(projectId) {setPendingProjectRequest({projectId,pageId});void selectTab('projects');}
          else {setPendingOpenRequest({pageId});void selectTab('pages');}
        }} />}
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
            onOpenAgentProposal={id => {setAgentProposalId(id);void selectTab('agents');}}
            workspaceId={currentSummary.workspaceId}
            refreshSignal={cw2RefreshSignal}
            onOpenPage={(pageId, proposalId, threadId) => {
              setPendingOpenRequest({ pageId, proposalId, threadId });
              void selectTab('pages');
            }}
            onOpenProject={(projectId, pageId, proposalId, threadId) => {
              setPendingProjectRequest({ projectId, pageId, proposalId, threadId });
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
                  <div className="space-y-0.5">
                    <h2 className="text-sm font-semibold flex items-center gap-1.5">
                      <Mail className="w-4 h-4 text-sthira-primary" />
                      <span>Invite people to {currentSummary.name}</span>
                    </h2>
                    <p className="text-xs text-sthira-text-muted">
                      Invitations grant access to the workspace (shared pages and projects). Enter one or more addresses separated by commas or new lines.
                    </p>
                  </div>
                  <span className="text-sthira-text-muted">Single-use codes · 7-day expiry</span>
                </div>

                <form
                  onSubmit={(e) => void handleCreateInviteBatch(e)}
                  className="space-y-3"
                >
                  <div className="flex flex-wrap items-start gap-3">
                    <label className="flex-1 min-w-[240px] space-y-1">
                      <span className="text-sthira-text-muted">Recipient email addresses</span>
                      <textarea
                        aria-label="Recipient email addresses"
                        rows={3}
                        required
                        disabled={mutating}
                        value={inviteEmailsRaw}
                        onChange={(e) => setInviteEmailsRaw(e.target.value)}
                        placeholder={'teammate1@example.com, teammate2@example.com\nteammate3@example.com'}
                        className="w-full rounded-lg border border-sthira-border bg-sthira-canvas p-2.5 text-sm text-sthira-text focus:outline-none"
                      />
                    </label>

                    <label className="w-40 space-y-1">
                      <span className="text-sthira-text-muted">Role for batch</span>
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
                  </div>

                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <span className="text-xs text-sthira-text-muted">
                      {parseInviteInputEmails(inviteEmailsRaw).length} address(es) entered
                    </span>
                    <button
                      type="submit"
                      disabled={mutating || parseInviteInputEmails(inviteEmailsRaw).length === 0}
                      className="sthira-button-primary cursor-pointer"
                    >
                      Send workspace invitations
                    </button>
                  </div>
                </form>

                {batchOutcome && (
                  <div
                    role="region"
                    aria-label="Batch invitation results"
                    className="rounded-xl border border-sthira-border bg-sthira-canvas p-4 space-y-3"
                  >
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <span className="font-semibold text-sthira-text">
                        Invitation results ({WORKSPACE_ROLE_LABELS[batchOutcome.offeredRole]})
                      </span>
                      <button
                        type="button"
                        onClick={() => setBatchOutcome(null)}
                        className="text-sthira-text-muted hover:text-sthira-text underline cursor-pointer"
                      >
                        Dismiss
                      </button>
                    </div>
                    <ul className="space-y-2.5" aria-label="Per-recipient invite results">
                      {batchOutcome.results.map((item, idx) => {
                        const rowInv = item.invitation
                          ? details?.invitations.find((d) => d.invitationId === item.invitation?.invitationId) ??
                            item.invitation
                          : null;
                        const activeCode =
                          (rowInv && replacedInviteCodes[rowInv.invitationId]?.inviteCode) ||
                          item.inviteCode ||
                          null;
                        const codeKey = `${item.email}-${idx}`;
                        return (
                          <li
                            key={codeKey}
                            className="rounded-lg border border-sthira-border bg-sthira-surface p-3 space-y-2"
                          >
                            <div className="flex flex-wrap items-center justify-between gap-2">
                              <div className="min-w-0 space-y-0.5">
                                <span className="font-medium text-sthira-text break-all">
                                  {item.email}
                                </span>
                                <p className="text-xs text-sthira-text-muted">{item.message}</p>
                              </div>
                              <div className="flex flex-wrap items-center gap-2">
                                {rowInv && rowInv.status === 'pending' && (
                                  <>
                                    <button
                                      type="button"
                                      disabled={mutating}
                                      aria-label={`Replace invite for ${item.email}`}
                                      onClick={() =>
                                        setConfirmation({ kind: 'invite-replace', invitation: rowInv })
                                      }
                                      className="sthira-button-secondary cursor-pointer"
                                    >
                                      Replace
                                    </button>
                                    <button
                                      type="button"
                                      disabled={mutating}
                                      aria-label={`Revoke invite for ${item.email}`}
                                      onClick={() =>
                                        setConfirmation({ kind: 'invite-revoke', invitation: rowInv })
                                      }
                                      className="sthira-button-secondary cursor-pointer"
                                    >
                                      Revoke
                                    </button>
                                  </>
                                )}
                              </div>
                            </div>

                            {activeCode && (
                              <div className="flex flex-wrap items-center gap-2">
                                <code
                                  data-testid="generated-invite-code"
                                  data-invite-email={item.email}
                                  className="flex-1 min-w-[200px] rounded-lg border border-sthira-border bg-sthira-canvas px-3 py-1.5 font-mono text-xs break-all select-all"
                                >
                                  {activeCode}
                                </code>
                                <button
                                  type="button"
                                  aria-label={`Copy invite code for ${item.email}`}
                                  onClick={() => {
                                    void copyCodeForKey(codeKey, activeCode);
                                  }}
                                  className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
                                >
                                  {copiedCodeKey === codeKey ? (
                                    <Check className="w-3.5 h-3.5" />
                                  ) : (
                                    <Copy className="w-3.5 h-3.5" />
                                  )}
                                  <span>{copiedCodeKey === codeKey ? 'Copied' : 'Copy code'}</span>
                                </button>
                              </div>
                            )}
                          </li>
                        );
                      })}
                    </ul>
                  </div>
                )}

                {details && details.invitations.length > 0 && (
                  <ul className="space-y-2" aria-label="Workspace invitations">
                    {details.invitations.map((inv) => {
                      const replaced = replacedInviteCodes[inv.invitationId];
                      const rowCodeKey = `inv-${inv.invitationId}`;
                      return (
                        <li
                          key={inv.invitationId}
                          className="rounded-xl border border-sthira-border bg-sthira-canvas p-3 space-y-2"
                        >
                          <div className="flex flex-wrap items-center justify-between gap-2.5">
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
                          </div>

                          {replaced && inv.status === 'pending' && (
                            <div className="flex flex-wrap items-center gap-2">
                              <code
                                data-testid="generated-invite-code"
                                data-invite-email={inv.recipientEmail}
                                className="flex-1 min-w-[200px] rounded-lg border border-sthira-border bg-sthira-surface px-3 py-1.5 font-mono text-xs break-all select-all"
                              >
                                {replaced.inviteCode}
                              </code>
                              <button
                                type="button"
                                onClick={() => void copyCodeForKey(rowCodeKey, replaced.inviteCode)}
                                className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
                              >
                                {copiedCodeKey === rowCodeKey ? (
                                  <Check className="w-3.5 h-3.5" />
                                ) : (
                                  <Copy className="w-3.5 h-3.5" />
                                )}
                                <span>{copiedCodeKey === rowCodeKey ? 'Copied' : 'Copy code'}</span>
                              </button>
                            </div>
                          )}
                        </li>
                      );
                    })}
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
