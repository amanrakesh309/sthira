import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  AlertTriangle,
  ArrowLeft,
  CheckCircle2,
  Copy,
  GitBranch,
  GitPullRequest,
  Lock,
  Redo2,
  RefreshCw,
  Undo2,
  Users,
  WifiOff,
} from 'lucide-react';
import * as Y from 'yjs';
import { Awareness } from 'y-protocols/awareness';
import {
  collabConnect,
  collabJoinDraft,
  collabLeaveDraft,
  collabPing,
  collabSendAwareness,
  collabSendUpdate,
  subscribeCollabEvents,
  type CollaboratorPresence,
} from '../api/liveCollab';
import {
  deleteSharedDraft,
  getLiveDraft,
  getSharedDraft,
  getSharedPage,
  makeRequestIdempotencyKey,
  openLiveDraft,
  rebaseLiveDraft,
  saveSharedDraft,
  submitLiveDraft,
  type LiveDraftContributorView,
  type LiveDraftRebaseMode,
  type SharedLiveDraftView,
  type SharedProposalDetails,
  type WorkspaceListResponse,
} from '../api/workspaces';
import type { GoogleSession } from './AccountModal';
import {
  DocumentEditor,
  type DocumentCollabConfig,
  type DocumentEditorHandle,
} from './DocumentEditor';
import { RichDocumentRenderer } from './RichDocumentRenderer';
import { plainPageMarkdown } from '../editor/plainPageMarkdown';
import {
  base64ToUint8,
  encodeSelectionRelativePositions,
  syncRemoteAwarenessPositions,
  syncStringToYText,
  uint8ToBase64,
} from '../editor/yjsCollab';

interface PendingOpEntry {
  requestId: string;
  opSeq: number;
  updateB64: string;
}

interface PendingRecoveryPayload {
  draftId: string;
  draftVersion: number;
  nextOpSeq: number;
  mergedPendingB64: string;
  previewMarkdown: string;
  summary: string;
  title?: string;
  baseTitleAtEdit?: string;
  baseMarkdownAtEdit?: string;
  fromRecovery?: boolean;
}

interface RecoverableEditsState {
  draftId: string;
  draftVersion: number;
  baseTitleAtEdit: string | null;
  baseMarkdownAtEdit: string | null;
  title: string;
  previewMarkdown: string;
  code: string;
  reason: string;
}

interface DraftRecoveryEnvelope {
  pending?: PendingRecoveryPayload | null;
  rejected?: RecoverableEditsState | null;
  earlierRejected?: RecoverableEditsState[];
}

interface BufferedRemoteUpdate {
  checkpointSeq: number;
  checkpointId: string;
  updateB64: string;
  contributors: LiveDraftContributorView[];
}

interface PublishedRevisionSnapshot {
  revisionNumber: number;
  title: string;
  contentMd: string;
}

function parseDraftRecoveryEnvelope(raw: string): DraftRecoveryEnvelope | null {
  try {
    const parsed = JSON.parse(raw) as DraftRecoveryEnvelope & Partial<PendingRecoveryPayload>;
    if (!parsed || typeof parsed !== 'object') return null;
    if ('pending' in parsed || 'rejected' in parsed) {
      return {
        pending: parsed.pending ?? null,
        rejected: parsed.rejected ?? null,
        earlierRejected: parsed.earlierRejected ?? [],
      };
    }
    if (typeof parsed.draftId === 'string' && typeof parsed.mergedPendingB64 === 'string') {
      return {
        pending: parsed as PendingRecoveryPayload,
        rejected: null,
      };
    }
    return null;
  } catch {
    return null;
  }
}

function extractAddedMarkdownBlocks(baseMd: string, editedMd: string): string {
  const cleanEdited = editedMd.trim();
  if (!cleanEdited) return '';
  const baseCounts = new Map<string, number>();
  for (const line of baseMd.split(/\n+/)) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    baseCounts.set(trimmed, (baseCounts.get(trimmed) ?? 0) + 1);
  }
  const added: string[] = [];
  const baseLines = baseMd
    .split(/\n+/)
    .map((l) => l.trim())
    .filter(Boolean);
  for (const line of cleanEdited.split(/\n+/)) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const avail = baseCounts.get(trimmed) ?? 0;
    if (avail > 0) {
      baseCounts.set(trimmed, avail - 1);
    } else {
      // If a user appended inline onto an existing baseline paragraph, keep only the new suffix
      // so bringing it over after peer edits doesn't duplicate the shared prefix.
      let suffixOnly: string | null = null;
      for (const bLine of baseLines) {
        if (trimmed.startsWith(bLine) && trimmed.length > bLine.length) {
          const rest = trimmed.slice(bLine.length).trim();
          if (rest) {
            suffixOnly = rest;
            break;
          }
        }
      }
      added.push(suffixOnly ?? line);
    }
  }
  return added.join('\n\n');
}

type LiveSyncStatus =
  | 'connecting'
  | 'synced'
  | 'saving'
  | 'unsynced'
  | 'disconnected'
  | 'frozen'
  | 'revoked';

interface LiveSharedDraftEditorProps {
  workspaceId: string;
  pageId: string;
  baseRevisionNumber: number;
  baseTitle: string;
  baseContentMd: string;
  initialCurrentRevisionNumber?: number;
  initialCurrentTitle?: string;
  initialCurrentContentMd?: string;
  currentUid?: string | null;
  authorDisplayName: string;
  backLabel?: string;
  registerBeforeLeave?: (guard: (() => Promise<boolean>) | null) => void;
  onSubmitted: (proposal: SharedProposalDetails) => void;
  onOpenProposal: (proposalId: string) => void;
  onBack: () => void;
  onAccessLost: (message: string, workspaces?: WorkspaceListResponse | null) => void;
  onSessionChange: (session: GoogleSession) => void;
}

export const liveDraftRecoveryKey = (pageId: string): string => `live_${pageId}`;

function createCollabPair(): { ydoc: Y.Doc; awareness: Awareness } {
  const ydoc = new Y.Doc();
  const awareness = new Awareness(ydoc);
  return { ydoc, awareness };
}

export const LiveSharedDraftEditor: React.FC<LiveSharedDraftEditorProps> = ({
  workspaceId,
  pageId,
  baseRevisionNumber,
  baseTitle,
  baseContentMd,
  initialCurrentRevisionNumber,
  initialCurrentTitle,
  initialCurrentContentMd,
  currentUid = null,
  authorDisplayName,
  backLabel = 'Back to page',
  registerBeforeLeave,
  onSubmitted,
  onOpenProposal,
  onBack,
  onAccessLost,
  onSessionChange,
}) => {
  const [collabPair, setCollabPair] = useState<{ ydoc: Y.Doc; awareness: Awareness }>(
    createCollabPair,
  );
  const collabPairRef = useRef(collabPair);
  collabPairRef.current = collabPair;

  const [draft, setDraft] = useState<SharedLiveDraftView | null>(null);
  const [title, setTitle] = useState(baseTitle);
  const [summary, setSummary] = useState('');
  const [contributors, setContributors] = useState<LiveDraftContributorView[]>([]);
  const [participants, setParticipants] = useState<CollaboratorPresence[]>([]);
  const [myConnectionId, setMyConnectionId] = useState<string | null>(null);
  const [syncStatus, setSyncStatus] = useState<LiveSyncStatus>('connecting');
  const [statusNotice, setStatusNotice] = useState('');
  const [errorMsg, setErrorMsg] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [recoveryNotice, setRecoveryNotice] = useState('');
  const [recoverableEdits, setRecoverableEdits] = useState<RecoverableEditsState | null>(null);
  const [recoveryApplyMode, setRecoveryApplyMode] = useState<'idle' | 'applying'>('idle');
  const [copiedRecoverable, setCopiedRecoverable] = useState(false);
  const [persistWarning, setPersistWarning] = useState('');
  const persistWarningRef = useRef('');
  persistWarningRef.current = persistWarning;

  const [latestPublished, setLatestPublished] = useState<PublishedRevisionSnapshot | null>(() =>
    initialCurrentRevisionNumber && initialCurrentRevisionNumber > baseRevisionNumber
      ? {
          revisionNumber: initialCurrentRevisionNumber,
          title: initialCurrentTitle ?? baseTitle,
          contentMd: initialCurrentContentMd ?? baseContentMd,
        }
      : null,
  );
  const [showRebaseCompare, setShowRebaseCompare] = useState(false);
  const [rebaseComparison, setRebaseComparison] = useState<{
    checkpointId: string;
    publishedRevisionNumber: number;
  } | null>(null);
  const [rebaseConflict, setRebaseConflict] = useState(false);
  const [carryTitleDraft, setCarryTitleDraft] = useState(baseTitle);
  const [carryContentDraft, setCarryContentDraft] = useState(baseContentMd);
  const [rebasing, setRebasing] = useState(false);
  const [hasCheckpointGap, setHasCheckpointGap] = useState(false);

  const editorRef = useRef<DocumentEditorHandle | null>(null);
  const draftRef = useRef<SharedLiveDraftView | null>(null);
  draftRef.current = draft;
  const checkpointIdRef = useRef('');
  const draftVersionRef = useRef<number | null>(null);
  const contiguousSeqRef = useRef(0);
  const maxSeenSeqRef = useRef(0);
  const pendingRemoteBySeqRef = useRef<Map<number, BufferedRemoteUpdate>>(new Map());
  const ackedSeqCheckpointsRef = useRef<
    Map<number, { checkpointId: string; contributors: LiveDraftContributorView[] }>
  >(new Map());
  const resyncInFlightRef = useRef(false);
  const syncStatusRef = useRef<LiveSyncStatus>(syncStatus);
  syncStatusRef.current = syncStatus;
  const myConnIdRef = useRef<string | null>(null);
  myConnIdRef.current = myConnectionId;
  const participantsRef = useRef<CollaboratorPresence[]>([]);
  participantsRef.current = participants;

  const generationRef = useRef(0);
  const nextOpSeqRef = useRef(1);
  const pendingOpsRef = useRef<Map<string, PendingOpEntry>>(new Map());
  const resetRejectedSnapshotRef = useRef(false);
  const rejectedRecoveryRef = useRef<RecoverableEditsState | null>(null);
  const earlierRejectedRef = useRef<RecoverableEditsState[]>([]);
  const applyingRecoveryRef = useRef(false);
  const lastServerTitleRef = useRef(baseTitle);
  const lastServerMarkdownRef = useRef(baseContentMd);
  const persistenceQueueRef = useRef<Promise<void>>(Promise.resolve());
  const titleRef = useRef(title);
  titleRef.current = title;
  const summaryRef = useRef(summary);
  summaryRef.current = summary;
  const markdownPreviewRef = useRef(baseContentMd);
  const submitKeyRef = useRef(makeRequestIdempotencyKey('livedraft'));
  const rebaseKeyRef = useRef<{ sig: string; key: string } | null>(null);
  const mountedRef = useRef(true);
  const lastAwarenessSentRef = useRef(0);
  const pendingSelectionRef = useRef<{ from: number; to: number } | null>(null);
  const awarenessTimerRef = useRef<number | null>(null);
  const hasConnectedOnceRef = useRef(false);

  const collabConfig: DocumentCollabConfig = useMemo(
    () => ({
      ydoc: collabPair.ydoc,
      awareness: collabPair.awareness,
      user: {
        name: authorDisplayName || 'Collaborator',
        color: '#0B6E4F',
      },
      field: 'default',
    }),
    [collabPair, authorDisplayName],
  );

  const resetCollabDocWithSnapshot = useCallback(
    (snapshotStateB64?: string | null): { ydoc: Y.Doc; awareness: Awareness } => {
      const pair = createCollabPair();
      if (snapshotStateB64 && snapshotStateB64.trim()) {
        const bytes = base64ToUint8(snapshotStateB64);
        if (bytes.length > 0) {
          Y.applyUpdate(pair.ydoc, bytes, 'server');
        }
      }
      collabPairRef.current = pair;
      setCollabPair(pair);
      return pair;
    },
    [],
  );

  const enqueueRecoverySync = useCallback(
    (curDraft: SharedLiveDraftView | null, gen: number): Promise<void> => {
      const uid = currentUid;
      const snapTitle = curDraft ? collabPairRef.current.ydoc.getText('title').toString() : titleRef.current;
      const snapMd = editorRef.current?.getEditor()?.getMarkdown() ?? markdownPreviewRef.current;
      const snapSummary = summaryRef.current;
      const snapNextOpSeq = nextOpSeqRef.current;
      const entries = Array.from(pendingOpsRef.current.values());
      const rejectedSnap = rejectedRecoveryRef.current;
      const earlierRejected = earlierRejectedRef.current;
      const baseTitleAtEdit = lastServerTitleRef.current;
      const baseMarkdownAtEdit = lastServerMarkdownRef.current;
      const isFromRecovery = applyingRecoveryRef.current;

      persistenceQueueRef.current = persistenceQueueRef.current
        .catch(() => {})
        .then(async () => {
          if (gen !== generationRef.current) return;
          if (entries.length === 0 && !rejectedSnap) {
            await deleteSharedDraft(workspaceId, liveDraftRecoveryKey(pageId), uid);
            if (gen === generationRef.current && mountedRef.current) {
              setPersistWarning('');
            }
            return;
          }
          if (entries.length > 0 && !curDraft && !rejectedSnap) return;
          let pendingPayload: PendingRecoveryPayload | null = null;
          if (entries.length > 0 && curDraft) {
            const updates = entries.map((op) => base64ToUint8(op.updateB64));
            const merged = Y.mergeUpdates(updates);
            pendingPayload = {
              draftId: curDraft.draftId,
              draftVersion: curDraft.draftVersion,
              nextOpSeq: snapNextOpSeq,
              mergedPendingB64: uint8ToBase64(merged),
              previewMarkdown: snapMd,
              summary: snapSummary,
              title: snapTitle,
              baseTitleAtEdit,
              baseMarkdownAtEdit,
              fromRecovery: isFromRecovery,
            };
          }
          const envelope: DraftRecoveryEnvelope = {
            pending: pendingPayload,
            rejected: rejectedSnap,
            earlierRejected,
          };
          await saveSharedDraft({
            workspaceId,
            draftKey: liveDraftRecoveryKey(pageId),
            pageId,
            proposalId: curDraft?.pendingProposalId ?? null,
            baseRevisionNumber: curDraft?.baseRevisionNumber ?? baseRevisionNumber,
            title: rejectedSnap?.title || snapTitle,
            contentMd: JSON.stringify(envelope),
            summary: snapSummary,
            accountUid: uid,
          });
          if (gen !== generationRef.current || !mountedRef.current) return;
          setPersistWarning('');
        })
        .catch((err) => {
          if (
            gen === generationRef.current &&
            mountedRef.current &&
            (pendingOpsRef.current.size > 0 || rejectedRecoveryRef.current !== null)
          ) {
            const detail =
              typeof err === 'string'
                ? err
                : err instanceof Error
                  ? err.message
                  : 'Disk storage rejected backup';
            setPersistWarning(
              `Offline backup failed (${detail}). Copy your unsent or recovered edits or keep this window open.`,
            );
          }
        });
      return persistenceQueueRef.current;
    },
    [baseRevisionNumber, currentUid, pageId, workspaceId],
  );

  const preserveRejectedEdits = useCallback((entry: RecoverableEditsState) => {
    const current = rejectedRecoveryRef.current;
    if (current && (current.previewMarkdown !== entry.previewMarkdown || current.title !== entry.title)) {
      earlierRejectedRef.current = [current, ...earlierRejectedRef.current];
    }
    rejectedRecoveryRef.current = entry;
    applyingRecoveryRef.current = false;
    setRecoverableEdits(entry);
    setRecoveryApplyMode('idle');
  }, []);

  const resolveRejectedEdits = useCallback(() => {
    const [next, ...earlier] = earlierRejectedRef.current;
    rejectedRecoveryRef.current = next ?? null;
    earlierRejectedRef.current = earlier;
    applyingRecoveryRef.current = false;
    setRecoverableEdits(next ?? null);
    setRecoveryApplyMode('idle');
  }, []);

  const flushPendingOpsOverWs = useCallback(
    async (curDraft: SharedLiveDraftView, gen: number) => {
      const entries = Array.from(pendingOpsRef.current.values()).sort(
        (a, b) => a.opSeq - b.opSeq,
      );
      if (entries.length === 0) return;
      setSyncStatus('saving');
      for (const op of entries) {
        if (gen !== generationRef.current || !mountedRef.current) return;
        try {
          await collabSendUpdate({
            workspaceId,
            requestId: op.requestId,
            pageId,
            draftId: curDraft.draftId,
            draftVersion: curDraft.draftVersion,
            clientOpSeq: op.opSeq,
            updateB64: op.updateB64,
            accountUid: currentUid,
          });
        } catch (err) {
          if (gen === generationRef.current && mountedRef.current) {
            setSyncStatus('unsynced');
            setErrorMsg(
              typeof err === 'string' ? err : 'Live edits paused while disconnected.',
            );
          }
          break;
        }
      }
    },
    [currentUid, pageId, workspaceId],
  );

  const refreshPublishedRevision = useCallback(
    async (draftBaseRev: number, gen: number) => {
      try {
        const pageRes = await getSharedPage(workspaceId, pageId);
        if (gen !== generationRef.current || !mountedRef.current) return;
        if (pageRes.outcome === 'ok') {
          const cur = pageRes.data.currentRevision;
          if (cur.revisionNumber > draftBaseRev) {
            setLatestPublished({
              revisionNumber: cur.revisionNumber,
              title: cur.title,
              contentMd: cur.contentMd,
            });
          } else {
            setLatestPublished(null);
          }
        }
      } catch {
        // Ignore transient failure when refreshing latest page revision.
      }
    },
    [pageId, workspaceId],
  );

  const drainContiguousCheckpoints = useCallback(
    (expectedDraftId: string, expectedDraftVersion: number): boolean => {
      const cur = draftRef.current;
      if (!cur || cur.draftId !== expectedDraftId || cur.draftVersion !== expectedDraftVersion) {
        return false;
      }
      let advanced = false;
      let latestContributors: LiveDraftContributorView[] | null = null;
      while (true) {
        const nextSeq = contiguousSeqRef.current + 1;
        const remote = pendingRemoteBySeqRef.current.get(nextSeq);
        if (remote) {
          pendingRemoteBySeqRef.current.delete(nextSeq);
          const bytes = base64ToUint8(remote.updateB64);
          if (bytes.length > 0) {
            Y.applyUpdate(collabPairRef.current.ydoc, bytes, 'remote');
          }
          contiguousSeqRef.current = nextSeq;
          checkpointIdRef.current = remote.checkpointId;
          latestContributors = remote.contributors;
          advanced = true;
          continue;
        }
        const acked = ackedSeqCheckpointsRef.current.get(nextSeq);
        if (acked) {
          ackedSeqCheckpointsRef.current.delete(nextSeq);
          contiguousSeqRef.current = nextSeq;
          checkpointIdRef.current = acked.checkpointId;
          latestContributors = acked.contributors;
          advanced = true;
          continue;
        }
        break;
      }
      if (advanced) {
        const currentTitle = collabPairRef.current.ydoc.getText('title').toString() || titleRef.current;
        setTitle(currentTitle);
        const currentContentMd = editorRef.current?.getEditor()?.getMarkdown() ?? markdownPreviewRef.current;
        const appliedSeq = contiguousSeqRef.current;
        const appliedChk = checkpointIdRef.current;
        const nextContribs = latestContributors;
        setDraft((prev) =>
          prev && prev.draftId === expectedDraftId && prev.draftVersion === expectedDraftVersion
            ? {
                ...prev,
                checkpointSeq: appliedSeq,
                checkpointId: appliedChk,
                currentTitle,
                currentContentMd,
                contributors: nextContribs ?? prev.contributors,
              }
            : prev,
        );
        if (nextContribs) {
          setContributors(nextContribs);
        }
        syncRemoteAwarenessPositions(
          editorRef.current?.getEditor() ?? null,
          collabPairRef.current.awareness,
          myConnIdRef.current,
          participantsRef.current,
        );
      }
      const gapRemaining = maxSeenSeqRef.current > contiguousSeqRef.current;
      setHasCheckpointGap(gapRemaining);
      return gapRemaining;
    },
    [],
  );

  const applyDraftFromServer = useCallback(
    (
      nextDraft: SharedLiveDraftView,
      missingUpdateB64: string | null | undefined,
      source: 'http' | 'ws',
    ) => {
      const gen = generationRef.current;
      const current = draftRef.current;
      if (
        current?.draftId === nextDraft.draftId &&
        (nextDraft.draftVersion < current.draftVersion ||
          (nextDraft.draftVersion === current.draftVersion &&
            nextDraft.checkpointSeq < contiguousSeqRef.current))
      ) {
        return false;
      }
      const versionChanged =
        (current && current.draftId !== nextDraft.draftId) ||
        (draftVersionRef.current !== null && draftVersionRef.current !== nextDraft.draftVersion);

      let activeYdoc = collabPairRef.current.ydoc;
      if (versionChanged || resetRejectedSnapshotRef.current) {
        resetRejectedSnapshotRef.current = false;
        pendingRemoteBySeqRef.current.clear();
        ackedSeqCheckpointsRef.current.clear();
        contiguousSeqRef.current = nextDraft.checkpointSeq;
        maxSeenSeqRef.current = nextDraft.checkpointSeq;
        setHasCheckpointGap(false);
        if (pendingOpsRef.current.size > 0) {
          preserveRejectedEdits({
            draftId: current?.draftId ?? nextDraft.draftId,
            draftVersion: draftVersionRef.current ?? nextDraft.draftVersion,
            baseTitleAtEdit: lastServerTitleRef.current,
            baseMarkdownAtEdit: lastServerMarkdownRef.current,
            title: titleRef.current,
            previewMarkdown: markdownPreviewRef.current,
            code: 'stale_draft_version',
            reason:
              'The draft version advanced (rebased or restarted) before your pending edits were confirmed. Compare or bring over your additions below.',
          });
          pendingOpsRef.current.clear();
          void enqueueRecoverySync(nextDraft, gen);
        }
        activeYdoc = resetCollabDocWithSnapshot(nextDraft.snapshotStateB64).ydoc;
      } else {
        const bytesToApply =
          missingUpdateB64 && missingUpdateB64.trim()
            ? base64ToUint8(missingUpdateB64)
            : base64ToUint8(nextDraft.snapshotStateB64);
        if (bytesToApply.length > 0) {
          Y.applyUpdate(activeYdoc, bytesToApply, 'server');
        }
      }

      const nextTitle =
        activeYdoc.getText('title').toString() || nextDraft.currentTitle || baseTitle;
      setTitle(nextTitle);
      lastServerTitleRef.current = nextDraft.currentTitle || nextTitle;
      lastServerMarkdownRef.current = nextDraft.currentContentMd;
      if (pendingOpsRef.current.size === 0) {
        markdownPreviewRef.current = nextDraft.currentContentMd;
      }
      setDraft(nextDraft);
      draftRef.current = nextDraft;
      checkpointIdRef.current = nextDraft.checkpointId;
      draftVersionRef.current = nextDraft.draftVersion;
      if (!versionChanged) {
        contiguousSeqRef.current = Math.max(contiguousSeqRef.current, nextDraft.checkpointSeq);
        maxSeenSeqRef.current = Math.max(maxSeenSeqRef.current, contiguousSeqRef.current);
        for (const seq of Array.from(pendingRemoteBySeqRef.current.keys())) {
          if (seq <= contiguousSeqRef.current) pendingRemoteBySeqRef.current.delete(seq);
        }
        for (const seq of Array.from(ackedSeqCheckpointsRef.current.keys())) {
          if (seq <= contiguousSeqRef.current) ackedSeqCheckpointsRef.current.delete(seq);
        }
      }
      setContributors(nextDraft.contributors || []);
      const gapExists = drainContiguousCheckpoints(nextDraft.draftId, nextDraft.draftVersion);

      if (nextDraft.status === 'pendingReview') {
        setSyncStatus('frozen');
        setStatusNotice('Live draft is frozen while pending review.');
      } else if (source === 'ws') {
        if (pendingOpsRef.current.size === 0 && !gapExists) {
          setSyncStatus('synced');
        }
      }
      return true;
    },
    [baseTitle, drainContiguousCheckpoints, enqueueRecoverySync, preserveRejectedEdits, resetCollabDocWithSnapshot],
  );

  const resyncCheckpointGap = useCallback(
    async (gen: number) => {
      if (resyncInFlightRef.current || gen !== generationRef.current || !mountedRef.current) {
        return;
      }
      resyncInFlightRef.current = true;
      try {
        const res = await getLiveDraft(workspaceId, pageId);
        if (gen !== generationRef.current || !mountedRef.current) return;
        if (res.outcome === 'ok' && res.data) {
          applyDraftFromServer(res.data, res.data.snapshotStateB64, 'ws');
          const sv = uint8ToBase64(Y.encodeStateVector(collabPairRef.current.ydoc));
          await collabJoinDraft({
            workspaceId,
            pageId,
            clientStateVectorB64: sv,
            initialUpdateB64: null,
            expectedBaseRevisionNumber: res.data.baseRevisionNumber,
            accountUid: currentUid,
          }).catch(() => {});
        }
      } catch {
        // Retry cleanly on next update or manual reconnect.
      } finally {
        resyncInFlightRef.current = false;
      }
    },
    [applyDraftFromServer, currentUid, pageId, workspaceId],
  );

  const connectAndSync = useCallback(
    async (isReconnect = false) => {
      const gen = generationRef.current;
      setErrorMsg('');
      setSyncStatus('connecting');
      try {
        // Authoritative server seeding: pass expectedBaseRevisionNumber instead of trusting unverified client initial seed.
        const openRes = await openLiveDraft(workspaceId, pageId, null, baseRevisionNumber);
        if (gen !== generationRef.current || !mountedRef.current) return;
        if (openRes.outcome === 'accessLost') {
          onAccessLost(openRes.message, openRes.workspaces);
          return;
        }
        if (openRes.outcome === 'sessionUpdated') {
          onSessionChange(openRes.session);
          setSyncStatus('revoked');
          setErrorMsg(
            openRes.session.status === 'unverified'
              ? openRes.session.message
              : 'Sign in with Google before joining live collaboration.',
          );
          return;
        }
        if (openRes.outcome === 'error') {
          setSyncStatus(
            openRes.code === 'forbidden' || openRes.code === 'page_archived'
              ? 'revoked'
              : 'disconnected',
          );
          setErrorMsg(openRes.message);
          return;
        }

        const fetchedDraft = openRes.data.draft;
        const missingOrFull = !hasConnectedOnceRef.current ? fetchedDraft.snapshotStateB64 : null;
        applyDraftFromServer(fetchedDraft, missingOrFull, 'http');
        void refreshPublishedRevision(fetchedDraft.baseRevisionNumber, gen);

        // Restore verified account-scoped recovery ONLY if accountUid matches strictly.
        if (!hasConnectedOnceRef.current) {
          const recovered = await getSharedDraft(
            workspaceId,
            liveDraftRecoveryKey(pageId),
            currentUid,
          ).catch(() => null);
          if (gen !== generationRef.current || !mountedRef.current) return;
          if (recovered) {
            const uidMatches = currentUid
              ? recovered.accountUid === currentUid
              : !recovered.accountUid;
            if (uidMatches) {
              const envelope = parseDraftRecoveryEnvelope(recovered.contentMd);
              earlierRejectedRef.current = envelope?.earlierRejected ?? [];
              if (envelope?.rejected) {
                rejectedRecoveryRef.current = envelope.rejected;
                setRecoverableEdits(envelope.rejected);
              }
              const pending = envelope?.pending;
              if (
                pending &&
                fetchedDraft.status === 'editing' &&
                pending.draftId === fetchedDraft.draftId &&
                pending.draftVersion === fetchedDraft.draftVersion &&
                pending.mergedPendingB64
              ) {
                applyingRecoveryRef.current = Boolean(pending.fromRecovery);
                if (applyingRecoveryRef.current) {
                  setRecoveryApplyMode('applying');
                }
                nextOpSeqRef.current = Math.max(nextOpSeqRef.current, pending.nextOpSeq || 1);
                const opSeq = nextOpSeqRef.current++;
                const requestId = makeRequestIdempotencyKey('rec');
                pendingOpsRef.current.set(requestId, {
                  requestId,
                  opSeq,
                  updateB64: pending.mergedPendingB64,
                });
                Y.applyUpdate(
                  collabPairRef.current.ydoc,
                  base64ToUint8(pending.mergedPendingB64),
                  'local-recovery',
                );
                setTitle(
                  collabPairRef.current.ydoc.getText('title').toString() ||
                    recovered.title ||
                    baseTitle,
                );
                if (pending.previewMarkdown) {
                  markdownPreviewRef.current = pending.previewMarkdown;
                }
                if (pending.summary) setSummary(pending.summary);
                setRecoveryNotice('Recovered your unsynced local edits; syncing now…');
              } else if (pending) {
                preserveRejectedEdits({
                  draftId: pending.draftId,
                  draftVersion: pending.draftVersion,
                  baseTitleAtEdit: pending.baseTitleAtEdit ?? null,
                  baseMarkdownAtEdit: pending.baseMarkdownAtEdit ?? null,
                  title: pending.title ?? recovered.title,
                  previewMarkdown: pending.previewMarkdown,
                  code: fetchedDraft.status === 'pendingReview' ? 'live_draft_frozen' : 'stale_draft_version',
                  reason: 'The shared draft changed while you were away. Compare or copy your unsent edits.',
                });
                await enqueueRecoverySync(fetchedDraft, gen);
                if (gen !== generationRef.current || !mountedRef.current) return;
              }
            }
          }
        }

        const joinSv = uint8ToBase64(Y.encodeStateVector(collabPairRef.current.ydoc));
        await collabConnect(workspaceId, currentUid);
        if (gen !== generationRef.current || !mountedRef.current) return;
        await collabJoinDraft({
          workspaceId,
          pageId,
          clientStateVectorB64: joinSv,
          initialUpdateB64: null,
          expectedBaseRevisionNumber: fetchedDraft.baseRevisionNumber,
          accountUid: currentUid,
        });
        if (gen !== generationRef.current || !mountedRef.current) return;
        hasConnectedOnceRef.current = true;
        if (isReconnect) {
          setStatusNotice('Reconnected — syncing latest live draft.');
        }
      } catch (err) {
        if (gen !== generationRef.current || !mountedRef.current) return;
        setSyncStatus(pendingOpsRef.current.size > 0 ? 'unsynced' : 'disconnected');
        setErrorMsg(
          typeof err === 'string'
            ? err
            : 'Could not connect to live shared draft channel.',
        );
      }
    },
    [
      applyDraftFromServer,
      baseRevisionNumber,
      baseTitle,
      currentUid,
      enqueueRecoverySync,
      onAccessLost,
      onSessionChange,
      pageId,
      preserveRejectedEdits,
      refreshPublishedRevision,
      workspaceId,
    ],
  );

  // Full reset + WS lifecycle scoped strictly to (workspaceId, pageId, currentUid).
  useEffect(() => {
    mountedRef.current = true;
    generationRef.current += 1;
    const gen = generationRef.current;

    pendingOpsRef.current.clear();
    resetRejectedSnapshotRef.current = false;
    nextOpSeqRef.current = 1;
    hasConnectedOnceRef.current = false;
    checkpointIdRef.current = '';
    draftVersionRef.current = null;
    contiguousSeqRef.current = 0;
    maxSeenSeqRef.current = 0;
    pendingRemoteBySeqRef.current.clear();
    ackedSeqCheckpointsRef.current.clear();
    resyncInFlightRef.current = false;
    rejectedRecoveryRef.current = null;
    earlierRejectedRef.current = [];
    applyingRecoveryRef.current = false;
    lastServerTitleRef.current = baseTitle;
    lastServerMarkdownRef.current = baseContentMd;
    draftRef.current = null;
    pendingSelectionRef.current = null;
    if (awarenessTimerRef.current !== null) {
      window.clearTimeout(awarenessTimerRef.current);
      awarenessTimerRef.current = null;
    }

    resetCollabDocWithSnapshot(null);
    setDraft(null);
    setTitle(baseTitle);
    markdownPreviewRef.current = baseContentMd;
    setSummary('');
    setContributors([]);
    setParticipants([]);
    setMyConnectionId(null);
    setSyncStatus('connecting');
    setStatusNotice('');
    setErrorMsg('');
    setRecoveryNotice('');
    setRecoverableEdits(null);
    setRecoveryApplyMode('idle');
    setPersistWarning('');
    setShowRebaseCompare(false);
    setRebaseComparison(null);
    setRebaseConflict(false);
    setHasCheckpointGap(false);
    setCarryTitleDraft(baseTitle);
    setCarryContentDraft(baseContentMd);
    setRebasing(false);
    setLatestPublished(
      initialCurrentRevisionNumber && initialCurrentRevisionNumber > baseRevisionNumber
        ? {
            revisionNumber: initialCurrentRevisionNumber,
            title: initialCurrentTitle ?? baseTitle,
            contentMd: initialCurrentContentMd ?? baseContentMd,
          }
        : null,
    );

    void connectAndSync(false);

    const refreshDraftSnapshot = async () => {
      const before = draftRef.current;
      const response = await getLiveDraft(workspaceId, pageId).catch(() => null);
      if (
        gen !== generationRef.current ||
        !mountedRef.current ||
        response?.outcome !== 'ok' ||
        !response.data
      ) return;

      const next = response.data;
      const current = draftRef.current;
      if (current?.draftId !== before?.draftId && current?.draftId !== next.draftId) return;
      const roomChanged =
        current?.draftId !== next.draftId || current?.draftVersion !== next.draftVersion;
      if (!applyDraftFromServer(next, null, 'ws')) return;
      if (next.status !== 'editing' || (!roomChanged && current?.status !== 'pendingReview')) return;

      setSyncStatus('connecting');
      setStatusNotice('The shared draft reopened. Syncing the latest edits…');
      const vector = Y.encodeStateVector(collabPairRef.current.ydoc);
      await collabJoinDraft({
        workspaceId,
        pageId,
        clientStateVectorB64: uint8ToBase64(vector),
        initialUpdateB64: null,
        expectedBaseRevisionNumber: next.baseRevisionNumber,
        accountUid: currentUid,
      }).catch(() => {
        if (gen === generationRef.current && mountedRef.current) {
          setSyncStatus('disconnected');
          setErrorMsg('Could not rejoin the shared draft. Reconnect to continue editing.');
        }
      });
    };

    let unlisten: (() => void) | null = null;
    void subscribeCollabEvents((ev) => {
      if (gen !== generationRef.current || !mountedRef.current) return;
      if (ev.workspaceId && ev.workspaceId !== workspaceId) return;
      if (ev.accountUid && currentUid && ev.accountUid !== currentUid) return;

      switch (ev.type) {
        case 'connected': {
          setMyConnectionId(ev.connectionId);
          break;
        }
        case 'draft_state': {
          if (ev.draft.pageId !== pageId) return;
          if (!applyDraftFromServer(ev.draft, ev.missingUpdateB64, 'ws')) return;
          setParticipants(ev.presence || []);
          syncRemoteAwarenessPositions(
            editorRef.current?.getEditor() ?? null,
            collabPairRef.current.awareness,
            myConnIdRef.current,
            ev.presence || [],
          );
          if (ev.draft.status === 'editing' && pendingOpsRef.current.size > 0) {
            void flushPendingOpsOverWs(ev.draft, gen);
          }
          break;
        }
        case 'draft_update_ack': {
          if (
            !draftRef.current ||
            ev.draftId !== draftRef.current.draftId ||
            ev.draftVersion !== draftRef.current.draftVersion
          ) {
            return;
          }
          if (!pendingOpsRef.current.has(ev.requestId)) return;
          pendingOpsRef.current.delete(ev.requestId);
          ackedSeqCheckpointsRef.current.set(ev.checkpointSeq, {
            checkpointId: ev.checkpointId,
            contributors: ev.contributors || [],
          });
          maxSeenSeqRef.current = Math.max(maxSeenSeqRef.current, ev.checkpointSeq);
          const gapExists = drainContiguousCheckpoints(ev.draftId, ev.draftVersion);
          if (pendingOpsRef.current.size === 0) {
            if (applyingRecoveryRef.current && rejectedRecoveryRef.current) {
              resolveRejectedEdits();
            }
            if (!gapExists) {
              lastServerTitleRef.current = titleRef.current;
              lastServerMarkdownRef.current = markdownPreviewRef.current;
            }
            setSyncStatus(draftRef.current.status === 'pendingReview' ? 'frozen' : 'synced');
            setRecoveryNotice('');
            void enqueueRecoverySync(draftRef.current, gen);
            if (gapExists) {
              void resyncCheckpointGap(gen);
            }
          } else {
            // Keep saving until remaining newer pending ops acknowledge too.
            setSyncStatus('saving');
            void enqueueRecoverySync(draftRef.current, gen);
          }
          break;
        }
        case 'draft_update_nack': {
          const rejectedOp = pendingOpsRef.current.get(ev.requestId);
          pendingOpsRef.current.delete(ev.requestId);
          setErrorMsg(ev.error);

          if (rejectedOp) {
            preserveRejectedEdits({
              draftId: draftRef.current?.draftId ?? '',
              draftVersion: draftRef.current?.draftVersion ?? 0,
              baseTitleAtEdit: lastServerTitleRef.current,
              baseMarkdownAtEdit: lastServerMarkdownRef.current,
              title: titleRef.current,
              previewMarkdown: markdownPreviewRef.current,
              code: ev.code,
              reason:
                ev.error ||
                'Local edits could not be applied to this draft version. Compare, copy, or bring over your edits below.',
            });
            pendingOpsRef.current.clear();
            resetRejectedSnapshotRef.current = true;
          }
          void enqueueRecoverySync(draftRef.current, gen);

          if (ev.code === 'live_draft_frozen') {
            setSyncStatus('frozen');
            void refreshDraftSnapshot();
          } else if (ev.code === 'stale_draft_version') {
            void connectAndSync(true);
          } else if (ev.fatal) {
            setSyncStatus('revoked');
          } else {
            setSyncStatus(pendingOpsRef.current.size > 0 ? 'unsynced' : 'disconnected');
          }
          break;
        }
        case 'draft_remote_update': {
          if (ev.pageId !== pageId) return;
          const curDraft = draftRef.current;
          if (!curDraft || ev.draftId !== curDraft.draftId) return;
          if (ev.draftVersion !== curDraft.draftVersion) {
            void connectAndSync(true);
            return;
          }
          if (ev.checkpointSeq <= contiguousSeqRef.current) {
            return;
          }
          pendingRemoteBySeqRef.current.set(ev.checkpointSeq, {
            checkpointSeq: ev.checkpointSeq,
            checkpointId: ev.checkpointId,
            updateB64: ev.updateB64,
            contributors: ev.contributors || [],
          });
          maxSeenSeqRef.current = Math.max(maxSeenSeqRef.current, ev.checkpointSeq);
          const gapExists = drainContiguousCheckpoints(ev.draftId, ev.draftVersion);
          if (!gapExists) {
            lastServerTitleRef.current =
              collabPairRef.current.ydoc.getText('title').toString() || curDraft.currentTitle;
            lastServerMarkdownRef.current = markdownPreviewRef.current;
          } else {
            void resyncCheckpointGap(gen);
          }
          break;
        }
        case 'draft_rebased': {
          if (ev.draft.pageId !== pageId) return;
          if (!applyDraftFromServer(ev.draft, null, 'ws')) return;
          void refreshPublishedRevision(ev.draft.baseRevisionNumber, gen);
          setStatusNotice(
            `Live draft rebased onto rev ${ev.draft.baseRevisionNumber} (v${ev.draft.draftVersion}).`,
          );
          const pair = collabPairRef.current;
          void collabJoinDraft({
            workspaceId,
            pageId,
            clientStateVectorB64: uint8ToBase64(Y.encodeStateVector(pair.ydoc)),
            initialUpdateB64: null,
            expectedBaseRevisionNumber: ev.draft.baseRevisionNumber,
            accountUid: currentUid,
          }).catch(() => {});
          break;
        }
        case 'presence_snapshot': {
          if (ev.pageId !== pageId) return;
          setParticipants(ev.participants || []);
          syncRemoteAwarenessPositions(
            editorRef.current?.getEditor() ?? null,
            collabPairRef.current.awareness,
            myConnIdRef.current,
            ev.participants || [],
          );
          break;
        }
        case 'presence_notice': {
          if (ev.pageId !== pageId) return;
          if (ev.kind === 'joined') {
            setStatusNotice(`${ev.displayName} joined live editing.`);
          } else if (ev.kind === 'left') {
            setStatusNotice(`${ev.displayName} left live editing.`);
          }
          break;
        }
        case 'workspace_event': {
          if (ev.pageId !== pageId) break;
          if (ev.eventType === 'page_archived') {
            setSyncStatus('revoked');
            setErrorMsg('This page was archived while you were editing.');
            setParticipants([]);
            syncRemoteAwarenessPositions(
              editorRef.current?.getEditor() ?? null,
              collabPairRef.current.awareness,
              myConnIdRef.current,
              [],
            );
          } else if (ev.eventType === 'proposal_updated') {
            if (draftRef.current) {
              void refreshPublishedRevision(draftRef.current.baseRevisionNumber, gen);
            }
            void refreshDraftSnapshot();
          }
          break;
        }
        case 'access_revoked': {
          setSyncStatus('revoked');
          setErrorMsg(ev.reason || 'Your edit session ended.');
          setParticipants([]);
          syncRemoteAwarenessPositions(
            editorRef.current?.getEditor() ?? null,
            collabPairRef.current.awareness,
            myConnIdRef.current,
            [],
          );
          break;
        }
        case 'ws_disconnected': {
          setParticipants([]);
          syncRemoteAwarenessPositions(
            editorRef.current?.getEditor() ?? null,
            collabPairRef.current.awareness,
            myConnIdRef.current,
            [],
          );
          setSyncStatus((prev) =>
            prev === 'frozen' || prev === 'revoked'
              ? prev
              : pendingOpsRef.current.size > 0
                ? 'unsynced'
                : 'disconnected',
          );
          setStatusNotice('Disconnected — live edits paused until reconnected.');
          break;
        }
      }
    }).then((fn) => {
      if (gen === generationRef.current && mountedRef.current) unlisten = fn;
      else fn();
    });

    const pingTimer = window.setInterval(() => {
      if (
        gen === generationRef.current &&
        syncStatusRef.current !== 'disconnected' &&
        syncStatusRef.current !== 'unsynced' &&
        syncStatusRef.current !== 'revoked'
      ) {
        void collabPing(workspaceId, currentUid).catch(() => {});
      }
    }, 12_000);

    return () => {
      mountedRef.current = false;
      generationRef.current += 1;
      if (awarenessTimerRef.current !== null) {
        window.clearTimeout(awarenessTimerRef.current);
        awarenessTimerRef.current = null;
      }
      window.clearInterval(pingTimer);
      unlisten?.();
      void collabLeaveDraft(workspaceId, pageId, currentUid);
    };
  }, [
    applyDraftFromServer,
    baseContentMd,
    baseRevisionNumber,
    baseTitle,
    connectAndSync,
    currentUid,
    drainContiguousCheckpoints,
    enqueueRecoverySync,
    flushPendingOpsOverWs,
    initialCurrentContentMd,
    initialCurrentRevisionNumber,
    initialCurrentTitle,
    pageId,
    preserveRejectedEdits,
    refreshPublishedRevision,
    resetCollabDocWithSnapshot,
    resolveRejectedEdits,
    resyncCheckpointGap,
    workspaceId,
  ]);

  // Listen to local Y.Text("title") and local Y.Doc CRDT updates on the active collabPair.
  useEffect(() => {
    const activeYdoc = collabPair.ydoc;
    const gen = generationRef.current;
    const titleYText = activeYdoc.getText('title');
    const onTitleChange = (_event: Y.YTextEvent, tx: Y.Transaction) => {
      if (tx.origin !== 'local-title') {
        setTitle(titleYText.toString());
      }
    };
    const onDocUpdate = (update: Uint8Array, origin: unknown) => {
      if (origin === 'remote' || origin === 'server' || origin === 'local-recovery') {
        return;
      }
      const curDraft = draftRef.current;
      if (!curDraft || curDraft.status !== 'editing') {
        return;
      }
      if (
        syncStatusRef.current === 'disconnected' ||
        syncStatusRef.current === 'unsynced' ||
        syncStatusRef.current === 'revoked'
      ) {
        return;
      }
      const updateB64 = uint8ToBase64(update);
      const opSeq = nextOpSeqRef.current++;
      const requestId = makeRequestIdempotencyKey('op');
      pendingOpsRef.current.set(requestId, { requestId, opSeq, updateB64 });
      setSyncStatus('saving');
      void enqueueRecoverySync(curDraft, gen);
      void collabSendUpdate({
        workspaceId,
        requestId,
        pageId,
        draftId: curDraft.draftId,
        draftVersion: curDraft.draftVersion,
        clientOpSeq: opSeq,
        updateB64,
        accountUid: currentUid,
      }).catch((err) => {
        if (gen === generationRef.current && mountedRef.current) {
          setSyncStatus('unsynced');
          setErrorMsg(
            typeof err === 'string'
              ? err
              : 'Disconnected while saving. Pending edits kept locally.',
          );
        }
      });
    };
    titleYText.observe(onTitleChange);
    activeYdoc.on('update', onDocUpdate);
    return () => {
      titleYText.unobserve(onTitleChange);
      activeYdoc.off('update', onDocUpdate);
    };
  }, [collabPair, currentUid, enqueueRecoverySync, pageId, workspaceId]);

  // Guard navigation / window close while unacknowledged ops, backup failures, or unapplied edits exist.
  useEffect(() => {
    if (!registerBeforeLeave) return;
    const gen = generationRef.current;
    const guard = async (): Promise<boolean> => {
      if (
        pendingOpsRef.current.size === 0 &&
        !persistWarningRef.current &&
        !recoverableEdits
      ) {
        return true;
      }
      await enqueueRecoverySync(draftRef.current, gen);
      if (syncStatusRef.current === 'saving') {
        const deadline = Date.now() + 1500;
        while (pendingOpsRef.current.size > 0 && Date.now() < deadline) {
          await new Promise((r) => setTimeout(r, 75));
        }
      }
      if (persistWarningRef.current) {
        return window.confirm(
          `${persistWarningRef.current} Leave live editing anyway? Unsynced edits in memory could be lost.`,
        );
      }
      if (pendingOpsRef.current.size > 0) {
        return window.confirm(
          'You still have Unsynced local edits. Keep them stored locally for this account and leave live editing?',
        );
      }
      if (recoverableEdits) {
        return window.confirm(
          'You have recoverable unapplied edits that were not copied or applied yet. Leave live editing anyway?',
        );
      }
      return true;
    };
    registerBeforeLeave(guard);
    return () => registerBeforeLeave(null);
  }, [enqueueRecoverySync, recoverableEdits, registerBeforeLeave]);

  const handleBack = async () => {
    const gen = generationRef.current;
    if (
      pendingOpsRef.current.size > 0 ||
      persistWarningRef.current ||
      recoverableEdits
    ) {
      await enqueueRecoverySync(draftRef.current, gen);
      if (syncStatusRef.current === 'saving') {
        const deadline = Date.now() + 1200;
        while (pendingOpsRef.current.size > 0 && Date.now() < deadline) {
          await new Promise((r) => setTimeout(r, 60));
        }
      }
      if (persistWarningRef.current) {
        const ok = window.confirm(
          `${persistWarningRef.current} Leave live editing anyway? Unsynced edits in memory could be lost.`,
        );
        if (!ok) return;
      } else if (pendingOpsRef.current.size > 0) {
        const ok = window.confirm(
          'Some live edits are Unsynced offline and saved locally for this account. Leave live editing now?',
        );
        if (!ok) return;
      } else if (recoverableEdits) {
        const ok = window.confirm(
          'You have unapplied edits saved in the recovery panel below. Leave without copying or applying them?',
        );
        if (!ok) return;
      }
    }
    await collabLeaveDraft(workspaceId, pageId, currentUid);
    onBack();
  };

  const sendAwarenessNow = useCallback(
    (from: number, to: number) => {
      const curDraft = draftRef.current;
      if (
        !curDraft ||
        syncStatusRef.current === 'disconnected' ||
        syncStatusRef.current === 'unsynced' ||
        syncStatusRef.current === 'revoked'
      ) {
        return;
      }
      lastAwarenessSentRef.current = Date.now();
      const ed = editorRef.current?.getEditor() ?? null;
      const { relAnchor, relHead } = encodeSelectionRelativePositions(ed, from, to);
      void collabSendAwareness({
        workspaceId,
        pageId,
        draftId: curDraft.draftId,
        anchor: from,
        head: to,
        relAnchor,
        relHead,
        accountUid: currentUid,
      });
    },
    [currentUid, pageId, workspaceId],
  );

  const handleSelectionRange = useCallback(
    (from: number, to: number) => {
      pendingSelectionRef.current = { from, to };
      const now = Date.now();
      const elapsed = now - lastAwarenessSentRef.current;
      if (elapsed >= 80) {
        if (awarenessTimerRef.current !== null) {
          window.clearTimeout(awarenessTimerRef.current);
          awarenessTimerRef.current = null;
        }
        sendAwarenessNow(from, to);
        return;
      }
      if (awarenessTimerRef.current === null) {
        awarenessTimerRef.current = window.setTimeout(() => {
          awarenessTimerRef.current = null;
          const latestSel = pendingSelectionRef.current;
          if (latestSel) {
            sendAwarenessNow(latestSel.from, latestSel.to);
          }
        }, 80 - elapsed);
      }
    },
    [sendAwarenessNow],
  );

  const handleCopyRecoverable = async () => {
    const text =
      recoverableEdits?.previewMarkdown ?? markdownPreviewRef.current;
    try {
      await navigator.clipboard.writeText(text);
      setCopiedRecoverable(true);
      window.setTimeout(() => {
        if (mountedRef.current) setCopiedRecoverable(false);
      }, 2500);
    } catch {
      setErrorMsg('Clipboard access unavailable; select text manually.');
    }
  };

  const handleDiscardRecoverable = () => {
    resolveRejectedEdits();
    setRecoveryNotice('');
    void enqueueRecoverySync(draftRef.current, generationRef.current);
  };

  const handleApplyRecoverableAppend = () => {
    if (!recoverableEdits || recoverableEdits.baseMarkdownAtEdit === null || draft?.status !== 'editing') return;
    if (syncStatus !== 'synced' || hasCheckpointGap) {
      setErrorMsg('Reconnect and wait until Synced before applying recovered edits.');
      return;
    }
    const ed = editorRef.current?.getEditor() ?? null;
    const additionsMd = extractAddedMarkdownBlocks(
      recoverableEdits.baseMarkdownAtEdit,
      recoverableEdits.previewMarkdown,
    );
    const currentTitleMatchesBase =
      titleRef.current.trim() === recoverableEdits.baseTitleAtEdit?.trim();
    const recoveredTitle = recoverableEdits.title.trim();
    const applyTitle =
      currentTitleMatchesBase &&
      recoveredTitle &&
      recoveredTitle !== titleRef.current.trim();
    if (!applyTitle && !additionsMd.trim()) {
      setErrorMsg('There are no additions to append. Compare or copy the recovered version to recover other changes.');
      return;
    }
    applyingRecoveryRef.current = true;
    setRecoveryApplyMode('applying');
    if (applyTitle) {
      setTitle(recoveredTitle);
      syncStringToYText(
        collabPairRef.current.ydoc.getText('title'),
        recoveredTitle,
        'local-title',
      );
    }
    if (ed && !ed.isDestroyed && additionsMd.trim()) {
      const endPos = ed.state.doc.content.size;
      ed.chain()
        .focus()
        .insertContentAt(endPos, plainPageMarkdown(additionsMd), {
          contentType: 'markdown',
        })
        .run();
    }
    void enqueueRecoverySync(draftRef.current, generationRef.current);
    setRecoveryNotice(
      'Appending your recovered additions while preserving peer edits; confirming sync…',
    );
  };

  const handleApplyRecoverableReplace = () => {
    if (!recoverableEdits || !draft || draft.status !== 'editing') return;
    if (syncStatus !== 'synced' || hasCheckpointGap) {
      setErrorMsg('Reconnect and wait until Synced before replacing draft content.');
      return;
    }
    const ed = editorRef.current?.getEditor() ?? null;
    const nextTitle = recoverableEdits.title.trim();
    applyingRecoveryRef.current = true;
    setRecoveryApplyMode('applying');
    if (nextTitle) {
      setTitle(nextTitle);
      syncStringToYText(collabPairRef.current.ydoc.getText('title'), nextTitle, 'local-title');
    }
    if (ed && !ed.isDestroyed) {
      ed.chain()
        .focus()
        .selectAll()
        .insertContent(plainPageMarkdown(recoverableEdits.previewMarkdown), {
          contentType: 'markdown',
        })
        .run();
    }
    void enqueueRecoverySync(draftRef.current, generationRef.current);
    setRecoveryNotice('Replacing live draft with recovered version; confirming sync…');
  };

  const handleSubmitForReview = async (e: React.FormEvent) => {
    e.preventDefault();
    const curDraft = draftRef.current;
    if (!curDraft || !summary.trim() || submitting) return;
    if (
      pendingOpsRef.current.size > 0 ||
      maxSeenSeqRef.current > contiguousSeqRef.current ||
      syncStatus !== 'synced'
    ) {
      setErrorMsg('Wait until all live edits and peer checkpoints finish syncing before submitting.');
      return;
    }
    setSubmitting(true);
    setErrorMsg('');
    try {
      const res = await submitLiveDraft(
        workspaceId,
        pageId,
        curDraft.draftId,
        curDraft.draftVersion,
        checkpointIdRef.current || curDraft.checkpointId,
        summary.trim(),
        submitKeyRef.current,
      );
      if (!mountedRef.current) return;
      if (res.outcome === 'ok') {
        if (!rejectedRecoveryRef.current) {
          await deleteSharedDraft(workspaceId, liveDraftRecoveryKey(pageId), currentUid).catch(
            () => {},
          );
        }
        submitKeyRef.current = makeRequestIdempotencyKey('livedraft');
        onSubmitted(res.data);
        return;
      }
      if (res.outcome === 'accessLost') {
        onAccessLost(res.message, res.workspaces);
        return;
      }
      if (res.outcome === 'sessionUpdated') {
        onSessionChange(res.session);
        setSyncStatus('revoked');
        setErrorMsg(
          res.session.status === 'unverified'
            ? res.session.message
            : 'Session changed. Sign in again to submit.',
        );
        return;
      }
      if (
        res.code === 'stale_draft_checkpoint' ||
        res.code === 'stale_draft_version' ||
        res.code === 'stale_draft_base'
      ) {
        submitKeyRef.current = makeRequestIdempotencyKey('livedraft');
        void connectAndSync(true);
      }
      setErrorMsg(res.message);
    } catch (err) {
      if (mountedRef.current) {
        setErrorMsg(
          typeof err === 'string' ? err : 'Could not submit shared draft for review.',
        );
      }
    } finally {
      if (mountedRef.current) setSubmitting(false);
    }
  };

  const rebaseNeedsReview = rebaseConflict || (
    rebaseComparison !== null &&
    (rebaseComparison.checkpointId !== draft?.checkpointId ||
      rebaseComparison.publishedRevisionNumber !== latestPublished?.revisionNumber)
  );
  const canRebase = syncStatus === 'synced' && !hasCheckpointGap && !rebasing;

  const handleRebaseDraft = async (mode: LiveDraftRebaseMode) => {
    const curDraft = draftRef.current;
    if (!curDraft || !latestPublished || !rebaseComparison || !canRebase) return;
    if (
      rebaseNeedsReview ||
      rebaseComparison.checkpointId !== checkpointIdRef.current ||
      pendingOpsRef.current.size > 0 ||
      maxSeenSeqRef.current > contiguousSeqRef.current
    ) {
      setErrorMsg('Compare the updated shared draft before rebasing.');
      return;
    }
    const gen = generationRef.current;
    const cleanTitle = carryTitleDraft.trim() || curDraft.currentTitle;
    const cleanMd = carryContentDraft;
    const expectedCheckpointId = rebaseComparison.checkpointId;
    const sig = `${curDraft.draftId}:${curDraft.draftVersion}:${expectedCheckpointId}:${latestPublished.revisionNumber}:${mode}:${cleanTitle}:${cleanMd}`;
    if (!rebaseKeyRef.current || rebaseKeyRef.current.sig !== sig) {
      rebaseKeyRef.current = { sig, key: makeRequestIdempotencyKey('rebase') };
    }
    setRebasing(true);
    setErrorMsg('');
    try {
      const res = await rebaseLiveDraft(workspaceId, pageId, {
        expectedDraftId: curDraft.draftId,
        expectedDraftVersion: curDraft.draftVersion,
        expectedCheckpointId,
        expectedBaseRevisionNumber: latestPublished.revisionNumber,
        mode,
        rebasedTitle: mode === 'carryDraft' ? cleanTitle : null,
        rebasedContentMd: mode === 'carryDraft' ? cleanMd : null,
        idempotencyKey: rebaseKeyRef.current.key,
      });
      if (gen !== generationRef.current || !mountedRef.current) return;
      if (res.outcome === 'ok') {
        rebaseKeyRef.current = null;
        setRebaseConflict(false);
        applyDraftFromServer(res.data, null, 'ws');
        setLatestPublished(null);
        setShowRebaseCompare(false);
        setRebaseComparison(null);
        setStatusNotice(
          mode === 'carryDraft'
            ? `Carried draft onto published rev ${res.data.baseRevisionNumber}. Review and submit when ready.`
            : `Restarted draft from published rev ${res.data.baseRevisionNumber}.`,
        );
        await collabJoinDraft({
          workspaceId,
          pageId,
          clientStateVectorB64: uint8ToBase64(Y.encodeStateVector(collabPairRef.current.ydoc)),
          initialUpdateB64: null,
          expectedBaseRevisionNumber: res.data.baseRevisionNumber,
          accountUid: currentUid,
        }).catch(() => {});
        return;
      }
      if (res.outcome === 'accessLost') {
        onAccessLost(res.message, res.workspaces);
        return;
      }
      if (res.outcome === 'sessionUpdated') {
        onSessionChange(res.session);
        setSyncStatus('revoked');
        setErrorMsg(
          res.session.status === 'unverified'
            ? res.session.message
            : 'Session changed. Sign in again to rebase.',
        );
        return;
      }
      if (
        res.code === 'stale_draft_checkpoint' ||
        res.code === 'stale_draft_base' ||
        res.code === 'stale_draft_version'
      ) {
        rebaseKeyRef.current = null;
        setRebaseConflict(true);
        void refreshPublishedRevision(curDraft.baseRevisionNumber, gen);
        void connectAndSync(true);
      }
      setErrorMsg(res.message);
    } catch (err) {
      if (gen === generationRef.current && mountedRef.current) {
        setErrorMsg(typeof err === 'string' ? err : 'Could not rebase live draft.');
      }
    } finally {
      if (gen === generationRef.current && mountedRef.current) {
        setRebasing(false);
      }
    }
  };

  const isStaleBase =
    !!draft &&
    !!latestPublished &&
    latestPublished.revisionNumber > draft.baseRevisionNumber;

  const editingDisabled =
    !draft ||
    draft.status !== 'editing' ||
    syncStatus === 'connecting' ||
    syncStatus === 'disconnected' ||
    syncStatus === 'unsynced' ||
    syncStatus === 'frozen' ||
    syncStatus === 'revoked' ||
    submitting ||
    rebasing;

  const statusBadgeText = (() => {
    switch (syncStatus) {
      case 'connecting':
        return 'Connecting…';
      case 'synced':
        return 'Synced';
      case 'saving':
        return 'Saving…';
      case 'unsynced':
        return 'Unsynced';
      case 'disconnected':
        return 'Disconnected';
      case 'frozen':
        return 'Frozen for review';
      case 'revoked':
        return 'Edit access ended';
    }
  })();

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <button
          type="button"
          onClick={() => void handleBack()}
          className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
        >
          <ArrowLeft className="w-3.5 h-3.5" />
          <span>{backLabel}</span>
        </button>

        <div className="flex flex-wrap items-center gap-2">
          <span
            role="status"
            aria-label={statusBadgeText}
            data-testid="live-sync-status"
            className="rounded-lg border border-sthira-border bg-sthira-surface px-2.5 py-1 text-xs font-medium text-sthira-text inline-flex items-center gap-1.5"
          >
            {syncStatus === 'synced' && <CheckCircle2 className="w-3.5 h-3.5 text-sthira-primary" />}
            {(syncStatus === 'disconnected' || syncStatus === 'unsynced') && (
              <WifiOff className="w-3.5 h-3.5 text-sthira-danger" />
            )}
            {syncStatus === 'frozen' && <Lock className="w-3.5 h-3.5 text-sthira-primary" />}
            <span>{statusBadgeText}</span>
          </span>

          <button
            type="button"
            disabled={editingDisabled}
            aria-label="Undo my last edit"
            onClick={() => editorRef.current?.undo()}
            className="sthira-button-secondary inline-flex items-center gap-1 cursor-pointer"
          >
            <Undo2 className="w-3.5 h-3.5" />
            <span>Undo</span>
          </button>
          <button
            type="button"
            disabled={editingDisabled}
            aria-label="Redo my edit"
            onClick={() => editorRef.current?.redo()}
            className="sthira-button-secondary inline-flex items-center gap-1 cursor-pointer"
          >
            <Redo2 className="w-3.5 h-3.5" />
            <span>Redo</span>
          </button>

          {(syncStatus === 'disconnected' || syncStatus === 'unsynced') && (
            <button
              type="button"
              onClick={() => void connectAndSync(true)}
              className="sthira-button-primary inline-flex items-center gap-1.5 cursor-pointer"
            >
              <RefreshCw className="w-3.5 h-3.5" />
              <span>Reconnect & sync</span>
            </button>
          )}
        </div>
      </div>

      {/* Live draft status + collaborator presence */}
      <section className="rounded-2xl border border-sthira-border bg-sthira-surface p-4 space-y-2.5">
        <div className="flex flex-wrap items-center justify-between gap-2 text-xs">
          <span className="text-sthira-text-muted">
            Live shared draft based on published{' '}
            <strong className="text-sthira-text">
              rev {draft?.baseRevisionNumber ?? baseRevisionNumber}
            </strong>
            {draft ? ` · Draft v${draft.draftVersion}` : ''} · Edits stay in draft until reviewed
          </span>

          <div
            role="list"
            className="flex flex-wrap items-center gap-1.5"
            aria-label="Active collaborators"
          >
            <Users className="w-3.5 h-3.5 text-sthira-primary" />
            {participants.length === 0 ? (
              <span className="text-sthira-text-muted">Only you right now</span>
            ) : (
              participants.map((p) => {
                const isSelf = myConnectionId != null && p.connectionId === myConnectionId;
                return (
                  <span
                    key={p.connectionId}
                    role="listitem"
                    aria-label={`${p.displayName}${isSelf ? ' (You)' : ''}`}
                    data-collaborator-chip={p.displayName}
                    className="inline-flex items-center gap-1.5 rounded-full border bg-sthira-canvas px-2.5 py-0.5 text-[11px] font-medium text-sthira-text"
                    style={{ borderColor: p.color }}
                  >
                    <span
                      className="w-2 h-2 rounded-full shrink-0"
                      style={{ backgroundColor: p.color }}
                      aria-hidden="true"
                    />
                    <span>
                      {p.displayName}
                      {isSelf ? ' (You)' : ''}
                    </span>
                  </span>
                );
              })
            )}
          </div>
        </div>

        {contributors.length > 0 && (
          <p className="text-[11px] text-sthira-text-muted">
            Contributors to this draft:{' '}
            <strong className="text-sthira-text">
              {contributors.map((c) => c.displayName).join(', ')}
            </strong>
          </p>
        )}

        {recoveryNotice && (
          <p role="status" className="text-xs text-sthira-primary">
            {recoveryNotice}
          </p>
        )}
        {statusNotice && (
          <p role="status" className="text-xs text-sthira-text-muted">
            {statusNotice}
          </p>
        )}
      </section>

      {/* Stale base revision rebase assistant */}
      {isStaleBase && draft && latestPublished && (
        <section
          role="status"
          data-testid="live-draft-stale-banner"
          className="rounded-2xl border border-sthira-border bg-sthira-surface-muted p-4 space-y-3 text-xs"
        >
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="space-y-0.5">
              <p className="font-semibold text-sthira-text flex items-center gap-1.5">
                <GitBranch className="w-4 h-4 text-sthira-danger" />
                <span>
                  Published rev {latestPublished.revisionNumber} landed while this draft is based on rev {draft.baseRevisionNumber}
                </span>
              </p>
              <p className="text-sthira-text-muted">
                Compare published rev {latestPublished.revisionNumber} against the draft, then carry edits forward or restart cleanly without losing decision history.
              </p>
            </div>
            <button
              type="button"
              onClick={() => {
                if (!showRebaseCompare && !rebaseComparison) {
                  setRebaseComparison({
                    checkpointId: draft.checkpointId,
                    publishedRevisionNumber: latestPublished.revisionNumber,
                  });
                  setCarryTitleDraft(title);
                  setCarryContentDraft(markdownPreviewRef.current);
                }
                setShowRebaseCompare(!showRebaseCompare);
              }}
              className="sthira-button-secondary cursor-pointer"
            >
              {showRebaseCompare
                ? 'Hide rebase comparison'
                : `Compare & rebase onto rev ${latestPublished.revisionNumber}`}
            </button>
          </div>

          {showRebaseCompare && (
            <div className="space-y-3 pt-2 border-t border-sthira-border">
              <div className="flex flex-wrap items-center justify-between gap-2 text-[11px] text-sthira-text-muted">
                <span>
                  Comparing against checkpoint{' '}
                  <code className="text-sthira-text">
                    {rebaseComparison?.checkpointId}
                  </code>
                </span>
              </div>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                <div className="rounded-xl border border-sthira-border bg-sthira-canvas p-3 space-y-2">
                  <div className="font-semibold text-sthira-text">
                    Published rev {latestPublished.revisionNumber}: {latestPublished.title}
                  </div>
                  <div className="max-h-32 overflow-auto text-xs">
                    <RichDocumentRenderer
                      content={latestPublished.contentMd}
                      objectTypes={[]}
                      onUpdateObject={() => {}}
                      onOpenEntity={() => {}}
                    />
                  </div>
                  <div className="pt-2 border-t border-sthira-border space-y-1">
                    <div className="font-semibold text-sthira-text">
                      Latest live draft (v{draft.draftVersion} · {draft.checkpointId}) — {draft.currentTitle}
                    </div>
                    <pre
                      data-testid="rebase-latest-draft-preview"
                      className="max-h-24 overflow-auto text-[11px] text-sthira-text-muted whitespace-pre-wrap"
                    >
                      {draft.currentContentMd}
                    </pre>
                  </div>
                </div>
                <div className="rounded-xl border border-sthira-border bg-sthira-canvas p-3 space-y-2">
                  <div className="font-semibold text-sthira-text">
                    Carried draft (v{draft.draftVersion} → rev {latestPublished.revisionNumber})
                  </div>
                  <input
                    aria-label="Rebased draft title"
                    type="text"
                    maxLength={160}
                    disabled={rebasing}
                    value={carryTitleDraft}
                    onChange={(e) => setCarryTitleDraft(e.target.value)}
                    className="w-full min-h-[34px] rounded-lg border border-sthira-border bg-sthira-surface px-2.5 text-xs text-sthira-text"
                  />
                  <textarea
                    aria-label="Rebased draft Markdown"
                    rows={5}
                    disabled={rebasing}
                    value={carryContentDraft}
                    onChange={(e) => setCarryContentDraft(e.target.value)}
                    className="w-full rounded-lg border border-sthira-border bg-sthira-surface p-2 text-xs font-mono text-sthira-text"
                  />
                </div>
              </div>

              {rebaseNeedsReview && (
                <div className="flex flex-wrap items-center gap-2">
                  <p role="status" className="text-xs text-sthira-text-muted">
                    The shared draft changed. Bring any newer edits you want to keep into the carried draft before confirming.
                  </p>
                  <button
                    type="button"
                    disabled={!canRebase}
                    onClick={() => {
                      setRebaseComparison({
                        checkpointId: draft.checkpointId,
                        publishedRevisionNumber: latestPublished.revisionNumber,
                      });
                      setRebaseConflict(false);
                      setErrorMsg('');
                    }}
                    className="sthira-button-secondary cursor-pointer"
                  >
                    Confirm updated comparison
                  </button>
                </div>
              )}

              <div className="flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  disabled={
                    !canRebase ||
                    rebaseNeedsReview ||
                    !carryTitleDraft.trim()
                  }
                  onClick={() => void handleRebaseDraft('carryDraft')}
                  className="sthira-button-primary cursor-pointer"
                >
                  {rebasing
                    ? 'Rebasing…'
                    : `Carry draft onto rev ${latestPublished.revisionNumber}`}
                </button>
                <button
                  type="button"
                  disabled={!canRebase || rebaseNeedsReview}
                  onClick={() => void handleRebaseDraft('restartFromCurrent')}
                  className="sthira-button-secondary cursor-pointer"
                >
                  Restart from rev {latestPublished.revisionNumber}
                </button>
              </div>
            </div>
          )}
        </section>
      )}

      {draft?.status === 'pendingReview' && draft.pendingProposalId && (
        <div
          role="status"
          className="rounded-xl border border-sthira-border bg-sthira-surface-muted p-4 flex flex-wrap items-center justify-between gap-3 text-xs"
        >
          <div className="space-y-0.5">
            <p className="font-semibold text-sthira-text">
              Submitted for review — editing frozen
            </p>
            <p className="text-sthira-text-muted">
              An eligible Owner or Admin must inspect and apply or request changes on this submission.
            </p>
          </div>
          <button
            type="button"
            onClick={() => onOpenProposal(draft.pendingProposalId!)}
            className="sthira-button-primary inline-flex items-center gap-1.5 cursor-pointer"
          >
            <GitPullRequest className="w-3.5 h-3.5" />
            <span>Open review</span>
          </button>
        </div>
      )}

      {(syncStatus === 'disconnected' || syncStatus === 'unsynced') && (
        <div
          role="alert"
          className="rounded-xl border border-sthira-border bg-sthira-surface-muted p-3.5 flex items-start gap-2.5 text-xs"
        >
          <AlertTriangle className="w-4 h-4 text-sthira-danger shrink-0 mt-0.5" />
          <div className="space-y-1">
            <p className="font-semibold text-sthira-text">
              Disconnected — new edits are paused
            </p>
            <p className="text-sthira-text-muted">
              {persistWarning
                ? persistWarning
                : pendingOpsRef.current.size > 0
                  ? 'Unacknowledged edits are marked Unsynced and stored locally for your signed-in account. Click Reconnect & sync when connected.'
                  : 'Live updates are unavailable right now. Reconnect when ready to resume collaborative editing.'}
            </p>
          </div>
        </div>
      )}

      {persistWarning && syncStatus !== 'disconnected' && syncStatus !== 'unsynced' && (
        <div
          role="alert"
          data-testid="persist-warning-banner"
          className="rounded-xl border border-sthira-border bg-sthira-surface-muted p-3.5 flex flex-wrap items-center justify-between gap-2 text-xs"
        >
          <span className="text-sthira-danger font-medium">{persistWarning}</span>
          <button
            type="button"
            onClick={() => void handleCopyRecoverable()}
            className="sthira-button-secondary inline-flex items-center gap-1 cursor-pointer"
          >
            <Copy className="w-3.5 h-3.5" />
            <span>{copiedRecoverable ? 'Copied!' : 'Copy current edits'}</span>
          </button>
        </div>
      )}

      {recoverableEdits && (
        <section
          role="alert"
          data-testid="recoverable-edits-banner"
          className="rounded-xl border border-sthira-border bg-sthira-surface-muted p-4 space-y-3 text-xs"
        >
          <div className="space-y-0.5">
            <p className="font-semibold text-sthira-text">
              Unapplied local edits preserved for recovery
            </p>
            <p className="text-sthira-text-muted">
              {draft?.status === 'editing' && recoverableEdits.code === 'live_draft_frozen'
                ? 'Your earlier edits were rejected while the draft was locked for review. The draft is now reopened—compare and bring over your additions below.'
                : recoverableEdits.reason}
            </p>
          </div>
          <div
            data-testid="recovery-compare-panel"
            className="grid grid-cols-1 md:grid-cols-2 gap-2.5"
          >
            <div className="rounded-lg border border-sthira-border bg-sthira-canvas p-2.5 space-y-1">
              <div className="font-semibold text-sthira-text text-[11px]">
                Current draft ({title})
              </div>
              <pre className="max-h-28 overflow-auto text-[11px] text-sthira-text-muted whitespace-pre-wrap">
                {plainPageMarkdown(draft?.currentContentMd || markdownPreviewRef.current)}
              </pre>
            </div>
            <div className="rounded-lg border border-sthira-border bg-sthira-canvas p-2.5 space-y-1">
              <div className="font-semibold text-sthira-text text-[11px]">
                Your recovered edits ({recoverableEdits.title})
              </div>
              <pre className="max-h-28 overflow-auto text-[11px] text-sthira-text whitespace-pre-wrap">
                {plainPageMarkdown(recoverableEdits.previewMarkdown)}
              </pre>
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              onClick={() => void handleCopyRecoverable()}
              className="sthira-button-secondary inline-flex items-center gap-1 cursor-pointer"
            >
              <Copy className="w-3.5 h-3.5" />
              <span>{copiedRecoverable ? 'Copied!' : 'Copy unsent edits'}</span>
            </button>
            {draft?.status === 'editing' && syncStatus === 'synced' && !hasCheckpointGap && (
              <>
                {recoverableEdits.baseMarkdownAtEdit !== null && <button
                  type="button"
                  data-testid="recovery-append-btn"
                  disabled={recoveryApplyMode === 'applying'}
                  onClick={handleApplyRecoverableAppend}
                  className="sthira-button-primary cursor-pointer"
                >
                  {recoveryApplyMode === 'applying'
                    ? 'Applying additions…'
                    : 'Bring over my additions (keep peer edits)'}
                </button>}
                <button
                  type="button"
                  data-testid="recovery-replace-btn"
                  disabled={recoveryApplyMode === 'applying'}
                  onClick={handleApplyRecoverableReplace}
                  className="sthira-button-secondary cursor-pointer"
                >
                  Replace draft with my version
                </button>
              </>
            )}
            <button
              type="button"
              onClick={handleDiscardRecoverable}
              className="sthira-button-secondary cursor-pointer"
            >
              Discard recovered copy
            </button>
          </div>
        </section>
      )}

      {hasCheckpointGap && (
        <div
          role="status"
          data-testid="checkpoint-gap-banner"
          className="rounded-xl border border-sthira-border bg-sthira-surface-muted px-4 py-2.5 flex flex-wrap items-center justify-between gap-2 text-xs"
        >
          <span className="text-sthira-text-muted">
            Waiting for missing collaborative checkpoint before submitting…
          </span>
          <button
            type="button"
            onClick={() => void resyncCheckpointGap(generationRef.current)}
            className="sthira-button-secondary cursor-pointer"
          >
            Resync checkpoints
          </button>
        </div>
      )}

      {errorMsg && (
        <p
          role="alert"
          className="rounded-xl border border-sthira-border bg-sthira-surface-muted px-4 py-2.5 text-xs text-sthira-danger"
        >
          {errorMsg}
        </p>
      )}

      {/* Title & Shared Editor */}
      <section className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-4">
        <label className="block space-y-1">
          <span className="text-xs text-sthira-text-muted">Page title (live draft)</span>
          <input
            aria-label="Live draft title"
            type="text"
            maxLength={160}
            disabled={editingDisabled}
            value={title}
            onChange={(e) => {
              const nextVal = e.target.value;
              setTitle(nextVal);
              syncStringToYText(
                collabPairRef.current.ydoc.getText('title'),
                nextVal,
                'local-title',
              );
            }}
            className="w-full min-h-[40px] rounded-lg border border-sthira-border bg-sthira-canvas px-3 text-base font-semibold font-editorial text-sthira-text focus:outline-none"
          />
        </label>

        <div className="space-y-1">
          <span className="text-xs text-sthira-text-muted">Collaborative page content</span>
          <div className="min-h-[260px] rounded-xl border border-sthira-border bg-sthira-canvas p-4">
            <DocumentEditor
              ref={editorRef}
              content=""
              title={title}
              disabled={editingDisabled}
              collab={collabConfig}
              objectTypes={[]}
              onChange={(md) => {
                markdownPreviewRef.current = md;
              }}
              onBlur={() => {}}
              onOpenEntity={() => {}}
              onSlash={() => {}}
              onSelectionRange={handleSelectionRange}
            />
          </div>
        </div>
      </section>

      {/* Submit exact immutable checkpoint for independent review */}
      {draft?.status === 'editing' && (
        <form
          onSubmit={(e) => void handleSubmitForReview(e)}
          className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-3"
        >
          <div className="space-y-0.5">
            <h3 className="text-sm font-semibold">Submit live draft for review</h3>
            <p className="text-xs text-sthira-text-muted">
              Publishing requires independent Owner or Admin approval of this exact snapshot. Submitters and draft contributors cannot approve their own changes.
            </p>
          </div>
          <div className="flex flex-wrap items-end gap-3">
            <label className="flex-1 min-w-[240px] space-y-1">
              <span className="text-xs text-sthira-text-muted">Change summary</span>
              <input
                aria-label="Live draft change summary"
                type="text"
                required
                maxLength={280}
                disabled={submitting || syncStatus !== 'synced'}
                value={summary}
                onChange={(e) => setSummary(e.target.value)}
                placeholder="Describe what you and your collaborators updated…"
                className="w-full min-h-[40px] rounded-lg border border-sthira-border bg-sthira-canvas px-3 text-xs text-sthira-text focus:outline-none"
              />
            </label>
            <button
              type="submit"
              disabled={
                submitting ||
                syncStatus !== 'synced' ||
                hasCheckpointGap ||
                pendingOpsRef.current.size > 0 ||
                !summary.trim() ||
                !title.trim()
              }
              className="sthira-button-primary inline-flex items-center gap-1.5 cursor-pointer"
            >
              <GitPullRequest className="w-3.5 h-3.5" />
              <span>{submitting ? 'Submitting…' : 'Submit for review'}</span>
            </button>
          </div>
        </form>
      )}
    </div>
  );
};
