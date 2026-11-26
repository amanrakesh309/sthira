import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  Archive,
  ArrowLeft,
  CheckCircle2,
  CopyPlus,
  Edit3,
  FilePlus,
  FileText,
  GitPullRequest,
  History,
  MessageSquare,
  RotateCcw,
} from 'lucide-react';
import {
  archiveSharedPage,
  canAuthorSharedContent,
  canReviewOrArchiveSharedContent,
  createPageThread,
  createProposal,
  createSharedPage,
  getProposalDetails,
  getSharedPage,
  listSharedPages,
  listSharedRevisions,
  listWorkspaceActivity,
  makeRequestIdempotencyKey,
  reopenPageThread,
  replyPageThread,
  resolvePageThread,
  resubmitProposal,
  restoreSharedPage,
  reviewProposal,
  withdrawProposal,
  type PagedList,
  type ProposalReviewAction,
  type SharedActivityItem,
  type SharedCommentThreadView,
  type SharedPageDetails,
  type SharedPageRevisionView,
  type SharedPageSummary,
  type SharedProposalDetails,
  type WorkspaceListResponse,
  type WorkspaceResponse,
  type WorkspaceRole,
} from '../api/workspaces';
import type { GoogleSession } from './AccountModal';
import { PublishPersonalCopyModal } from './PublishPersonalCopyModal';
import { RichDocumentRenderer } from './RichDocumentRenderer';
import { SharedPageDraftEditor, sharedDraftKeyForMode, type SharedDraftEditorMode } from './SharedPageDraftEditor';
import { SharedProposalReview } from './SharedProposalReview';

interface SharedPagesPanelProps {
  workspaceId: string;
  workspaceName: string;
  myRole: WorkspaceRole;
  currentUid?: string | null;
  authorDisplayName?: string;
  refreshSignal: number;
  pendingOpenRequest?: { pageId?: string | null; proposalId?: string | null; mode?: 'newPage' | 'publishCopy' } | null;
  onConsumedOpenRequest?: () => void;
  registerBeforeLeave?: (guard: (() => Promise<boolean>) | null) => void;
  onAccessLost: (message: string, workspaces?: WorkspaceListResponse | null) => void;
  onSessionChange: (session: GoogleSession) => void;
  onErrorNotice: (message: string) => void;
}

function formatDateTime(epochMs: number): string {
  if (!Number.isFinite(epochMs) || epochMs <= 0) return '—';
  try {
    return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(
      new Date(epochMs),
    );
  } catch {
    return new Date(epochMs).toISOString();
  }
}

export const SharedPagesPanel: React.FC<SharedPagesPanelProps> = ({
  workspaceId,
  workspaceName,
  myRole,
  currentUid,
  authorDisplayName,
  refreshSignal,
  pendingOpenRequest,
  onConsumedOpenRequest,
  registerBeforeLeave,
  onAccessLost,
  onSessionChange,
  onErrorNotice,
}) => {
  const [pages, setPages] = useState<SharedPageSummary[]>([]);
  const [pagesNextCursor, setPagesNextCursor] = useState<string | null>(null);
  const [includeArchived, setIncludeArchived] = useState(false);
  const [loadingList, setLoadingList] = useState(true);
  const [selectedPage, setSelectedPage] = useState<SharedPageDetails | null>(null);
  const [inspectedRevision, setInspectedRevision] = useState<SharedPageRevisionView | null>(null);
  const [revisionsList, setRevisionsList] = useState<SharedPageRevisionView[]>([]);
  const [revisionsCursor, setRevisionsCursor] = useState<string | null>(null);
  const [showHistory, setShowHistory] = useState(false);

  const [selectedProposal, setSelectedProposal] = useState<SharedProposalDetails | null>(null);
  const [draftMode, setDraftMode] = useState<SharedDraftEditorMode | null>(null);
  const [publishCopyOpen, setPublishCopyOpen] = useState(false);

  const [newThreadBody, setNewThreadBody] = useState('');
  const [replyDrafts, setReplyDrafts] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [panelError, setPanelError] = useState('');
  const [panelNotice, setPanelNotice] = useState('');

  const reqSeqRef = useRef(0);
  const newThreadKeyRef = useRef<{ sig: string; key: string } | null>(null);
  const replyKeysRef = useRef<Record<string, { sig: string; key: string }>>({});

  const callbacksRef = useRef({ onAccessLost, onSessionChange, onErrorNotice });
  callbacksRef.current = { onAccessLost, onSessionChange, onErrorNotice };

  const handleFailureResponse = useCallback(
    <T,>(
      res: WorkspaceResponse<T>,
      fallbackMsg: string,
      setErrorFn: (msg: string) => void = setPanelError,
    ) => {
      if (res.outcome === 'accessLost') {
        setSelectedPage(null);
        setSelectedProposal(null);
        setDraftMode(null);
        callbacksRef.current.onAccessLost(res.message, res.workspaces);
        return;
      }
      if (res.outcome === 'sessionUpdated') {
        callbacksRef.current.onSessionChange(res.session);
        const msg =
          res.session.status === 'unverified'
            ? res.session.message
            : 'Sign in with Google to access team pages.';
        setErrorFn(msg);
        return;
      }
      if (res.outcome === 'error') {
        setErrorFn(res.message || fallbackMsg);
      }
    },
    [],
  );

  const loadPagesList = useCallback(
    async (archivedFlag = includeArchived, appendCursor?: string | null) => {
      const seq = ++reqSeqRef.current;
      setLoadingList(true);
      if (!appendCursor) setPanelError('');
      try {
        const res = await listSharedPages(workspaceId, archivedFlag, appendCursor ?? null);
        if (seq !== reqSeqRef.current) return;
        if (res.outcome === 'ok') {
          const data: PagedList<SharedPageSummary> = res.data;
          setPages((prev) => (appendCursor ? [...prev, ...data.items] : data.items));
          setPagesNextCursor(data.nextCursor ?? null);
          return;
        }
        handleFailureResponse(res, 'Could not load shared pages.');
      } catch (err) {
        if (seq !== reqSeqRef.current) return;
        setPanelError(typeof err === 'string' ? err : 'Could not load shared pages.');
      } finally {
        if (seq === reqSeqRef.current) setLoadingList(false);
      }
    },
    [handleFailureResponse, includeArchived, workspaceId],
  );

  const openPageById = useCallback(
    async (pageId: string) => {
      const seq = ++reqSeqRef.current;
      setBusy(true);
      setPanelError('');
      try {
        const res = await getSharedPage(workspaceId, pageId);
        if (seq !== reqSeqRef.current) return;
        if (res.outcome === 'ok') {
          setSelectedPage(res.data);
          setRevisionsList(res.data.revisions);
          setRevisionsCursor(res.data.revisionsNextCursor ?? null);
          setInspectedRevision(null);
          setSelectedProposal(null);
          setDraftMode(null);
          return;
        }
        handleFailureResponse(res, 'Could not open shared page.');
      } catch (err) {
        if (seq !== reqSeqRef.current) return;
        setPanelError(typeof err === 'string' ? err : 'Could not open shared page.');
      } finally {
        if (seq === reqSeqRef.current) setBusy(false);
      }
    },
    [handleFailureResponse, workspaceId],
  );

  const openProposalById = useCallback(
    async (proposalId: string) => {
      const seq = ++reqSeqRef.current;
      setBusy(true);
      setPanelError('');
      try {
        const res = await getProposalDetails(workspaceId, proposalId);
        if (seq !== reqSeqRef.current) return;
        if (res.outcome === 'ok') {
          setSelectedProposal(res.data);
          setDraftMode(null);
          return;
        }
        handleFailureResponse(res, 'Could not load proposal details.');
      } catch (err) {
        if (seq !== reqSeqRef.current) return;
        setPanelError(typeof err === 'string' ? err : 'Could not load proposal details.');
      } finally {
        if (seq === reqSeqRef.current) setBusy(false);
      }
    },
    [handleFailureResponse, workspaceId],
  );

  const selectedPageRef = useRef(selectedPage);
  const selectedProposalRef = useRef(selectedProposal);
  const draftModeRef = useRef(draftMode);
  selectedPageRef.current = selectedPage;
  selectedProposalRef.current = selectedProposal;
  draftModeRef.current = draftMode;
  const prevRefreshSignalRef = useRef(refreshSignal);

  useEffect(() => {
    setSelectedPage(null);
    setSelectedProposal(null);
    setDraftMode(null);
    setInspectedRevision(null);
    setShowHistory(false);
    setPanelNotice('');
    setPanelError('');
    void loadPagesList(includeArchived, null);
  }, [includeArchived, loadPagesList, workspaceId]);

  useEffect(() => {
    if (refreshSignal === prevRefreshSignalRef.current) return;
    prevRefreshSignalRef.current = refreshSignal;
    if (draftModeRef.current) return;
    if (selectedProposalRef.current) {
      void openProposalById(selectedProposalRef.current.proposal.proposalId);
    } else if (selectedPageRef.current) {
      void openPageById(selectedPageRef.current.page.pageId);
    } else {
      void loadPagesList(includeArchived, null);
    }
  }, [includeArchived, loadPagesList, openPageById, openProposalById, refreshSignal]);

  useEffect(() => {
    if (!pendingOpenRequest) return;
    if (pendingOpenRequest.mode === 'newPage') {
      setDraftMode({ kind: 'newPage' });
      onConsumedOpenRequest?.();
      return;
    }
    if (pendingOpenRequest.mode === 'publishCopy') {
      setPublishCopyOpen(true);
      onConsumedOpenRequest?.();
      return;
    }
    if (pendingOpenRequest.proposalId) {
      void (async () => {
        if (pendingOpenRequest.pageId) {
          await openPageById(pendingOpenRequest.pageId);
        }
        await openProposalById(pendingOpenRequest.proposalId!);
      })();
      onConsumedOpenRequest?.();
      return;
    }
    if (pendingOpenRequest.pageId) {
      void openPageById(pendingOpenRequest.pageId);
      onConsumedOpenRequest?.();
    }
  }, [onConsumedOpenRequest, openPageById, openProposalById, pendingOpenRequest]);

  const canAuthor = canAuthorSharedContent(myRole);
  const canReviewOrArchive = canReviewOrArchiveSharedContent(myRole);

  const applyUpdatedThread = (updated: SharedCommentThreadView) => {
    setSelectedPage((prev) => {
      if (!prev) return prev;
      const exists = prev.threads.some((t) => t.threadId === updated.threadId);
      const threads = exists
        ? prev.threads.map((t) => (t.threadId === updated.threadId ? updated : t))
        : [updated, ...prev.threads];
      return { ...prev, threads };
    });
  };

  const handleStartThread = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!selectedPage || !newThreadBody.trim() || busy) return;
    const body = newThreadBody.trim();
    const rev = selectedPage.currentRevision.revisionNumber;
    const sig = `${selectedPage.page.pageId}:${rev}:${body}`;
    if (!newThreadKeyRef.current || newThreadKeyRef.current.sig !== sig) {
      newThreadKeyRef.current = { sig, key: makeRequestIdempotencyKey('thr') };
    }
    setBusy(true);
    setPanelError('');
    try {
      const res = await createPageThread(
        workspaceId,
        selectedPage.page.pageId,
        rev,
        body,
        newThreadKeyRef.current.key,
      );
      if (res.outcome === 'ok') {
        newThreadKeyRef.current = null;
        setNewThreadBody('');
        applyUpdatedThread(res.data);
        setPanelNotice(`Started discussion on rev ${rev}.`);
        return;
      }
      handleFailureResponse(res, 'Could not start discussion thread.');
    } catch (err) {
      setPanelError(typeof err === 'string' ? err : 'Could not start discussion thread.');
    } finally {
      setBusy(false);
    }
  };

  const handleReplyThread = async (threadId: string) => {
    if (!selectedPage || busy) return;
    const body = (replyDrafts[threadId] || '').trim();
    if (!body) return;
    const sig = `${threadId}:${body}`;
    if (!replyKeysRef.current[threadId] || replyKeysRef.current[threadId].sig !== sig) {
      replyKeysRef.current[threadId] = { sig, key: makeRequestIdempotencyKey('rep') };
    }
    setBusy(true);
    setPanelError('');
    try {
      const res = await replyPageThread(
        workspaceId,
        selectedPage.page.pageId,
        threadId,
        body,
        replyKeysRef.current[threadId].key,
      );
      if (res.outcome === 'ok') {
        delete replyKeysRef.current[threadId];
        setReplyDrafts((prev) => ({ ...prev, [threadId]: '' }));
        applyUpdatedThread(res.data);
        return;
      }
      handleFailureResponse(res, 'Could not post reply.');
    } catch (err) {
      setPanelError(typeof err === 'string' ? err : 'Could not post reply.');
    } finally {
      setBusy(false);
    }
  };

  const handleThreadStatus = async (threadId: string, resolve: boolean) => {
    if (!selectedPage || busy) return;
    setBusy(true);
    setPanelError('');
    try {
      const key = makeRequestIdempotencyKey(resolve ? 'res' : 'reopen');
      const res = resolve
        ? await resolvePageThread(workspaceId, selectedPage.page.pageId, threadId, key)
        : await reopenPageThread(workspaceId, selectedPage.page.pageId, threadId, key);
      if (res.outcome === 'ok') {
        applyUpdatedThread(res.data);
        return;
      }
      handleFailureResponse(res, 'Could not update thread status.');
    } catch (err) {
      setPanelError(typeof err === 'string' ? err : 'Could not update thread status.');
    } finally {
      setBusy(false);
    }
  };

  const handleArchiveToggle = async (archive: boolean) => {
    if (!selectedPage || busy) return;
    setBusy(true);
    setPanelError('');
    try {
      const key = makeRequestIdempotencyKey(archive ? 'arch' : 'rest');
      const res = archive
        ? await archiveSharedPage(workspaceId, selectedPage.page.pageId, key)
        : await restoreSharedPage(workspaceId, selectedPage.page.pageId, key);
      if (res.outcome === 'ok') {
        setSelectedPage(res.data);
        setRevisionsList(res.data.revisions);
        setRevisionsCursor(res.data.revisionsNextCursor ?? null);
        setPanelNotice(archive ? 'Page archived.' : 'Page restored.');
        void loadPagesList(includeArchived, null);
        return;
      }
      handleFailureResponse(res, 'Could not update archive status.');
    } catch (err) {
      setPanelError(typeof err === 'string' ? err : 'Could not update archive status.');
    } finally {
      setBusy(false);
    }
  };

  const handleLoadMoreRevisions = async () => {
    if (!selectedPage || !revisionsCursor || busy) return;
    setBusy(true);
    try {
      const res = await listSharedRevisions(workspaceId, selectedPage.page.pageId, revisionsCursor);
      if (res.outcome === 'ok') {
        setRevisionsList((prev) => [...prev, ...res.data.items]);
        setRevisionsCursor(res.data.nextCursor ?? null);
      } else {
        handleFailureResponse(res, 'Could not load earlier revisions.');
      }
    } finally {
      setBusy(false);
    }
  };

  if (draftMode) {
    const currentPublishedDoc =
      draftMode.kind === 'proposeEdit' && selectedPage
        ? {
            revisionNumber: selectedPage.currentRevision.revisionNumber,
            title: selectedPage.currentRevision.title,
            contentMd: selectedPage.currentRevision.contentMd,
          }
        : draftMode.kind === 'reviseProposal' && selectedProposal
          ? {
              revisionNumber: selectedProposal.currentPageRevisionNumber,
              title: selectedProposal.currentPageTitle,
              contentMd: selectedProposal.currentPageContentMd,
            }
          : null;
    return (
      <SharedPageDraftEditor
        key={JSON.stringify([workspaceId, currentUid, sharedDraftKeyForMode(draftMode)])}
        workspaceId={workspaceId}
        accountUid={currentUid}
        mode={draftMode}
        currentPublishedDoc={currentPublishedDoc}
        registerBeforeLeave={registerBeforeLeave}
        submitting={busy}
        error={panelError}
        onClearError={() => setPanelError('')}
        onCancel={() => {
          setPanelError('');
          setDraftMode(null);
        }}
        onSubmit={async (payload) => {
          setBusy(true);
          setPanelError('');
          try {
            if (draftMode.kind === 'newPage') {
              const res = await createSharedPage(workspaceId, {
                title: payload.title,
                contentMd: payload.contentMd,
                entryMode: 'new',
                provenance: {
                  originKind: 'userAuthored',
                  originalAuthorLabel: authorDisplayName || 'Team member',
                },
                omittedSummary: [],
                idempotencyKey: payload.idempotencyKey,
              });
              if (res.outcome === 'ok') {
                setDraftMode(null);
                setSelectedPage(res.data.page);
                setRevisionsList(res.data.page.revisions);
                setRevisionsCursor(res.data.page.revisionsNextCursor ?? null);
                setPanelNotice(`Published “${res.data.page.page.title}”.`);
                void loadPagesList(includeArchived, null);
                return true;
              }
              handleFailureResponse(res, 'Could not publish shared page.');
              return false;
            }
            if (draftMode.kind === 'proposeEdit') {
              const res = await createProposal(
                workspaceId,
                draftMode.pageId,
                payload.baseRevisionNumber ?? draftMode.baseRevisionNumber,
                payload.title,
                payload.contentMd,
                payload.summary,
                payload.idempotencyKey,
              );
              if (res.outcome === 'ok') {
                setDraftMode(null);
                setSelectedProposal(res.data);
                setPanelNotice('Submitted proposal for review.');
                void openPageById(draftMode.pageId).then(() => setSelectedProposal(res.data));
                return true;
              }
              handleFailureResponse(res, 'Could not submit proposal.');
              return false;
            }
            const res = await resubmitProposal(
              workspaceId,
              draftMode.proposalId,
              payload.baseRevisionNumber ?? draftMode.baseRevisionNumber,
              payload.title,
              payload.contentMd,
              payload.summary,
              payload.idempotencyKey,
            );
            if (res.outcome === 'ok') {
              setDraftMode(null);
              setSelectedProposal(res.data);
              setPanelNotice('Resubmitted proposal for fresh review.');
              return true;
            }
            handleFailureResponse(res, 'Could not resubmit proposal.');
            return false;
          } catch (err) {
            setPanelError(typeof err === 'string' ? err : 'Submission failed; your local draft is preserved.');
            return false;
          } finally {
            setBusy(false);
          }
        }}
      />
    );
  }

  if (selectedProposal) {
    return (
      <SharedProposalReview
        details={selectedProposal}
        currentUid={currentUid}
        busy={busy}
        error={panelError}
        onRefresh={() => void openProposalById(selectedProposal.proposal.proposalId)}
        onBack={() => {
          const pageId = selectedProposal.proposal.pageId;
          setSelectedProposal(null);
          setPanelError('');
          void openPageById(pageId);
        }}
        onReview={async (action: ProposalReviewAction, note, idempotencyKey, expectedSubmissionNumber) => {
          setBusy(true);
          setPanelError('');
          try {
            const res = await reviewProposal(
              workspaceId,
              selectedProposal.proposal.proposalId,
              action,
              selectedProposal.currentPageRevisionNumber,
              expectedSubmissionNumber,
              note,
              idempotencyKey,
            );
            if (res.outcome === 'ok') {
              setSelectedProposal(res.data);
              setPanelNotice(
                action === 'apply'
                  ? `Applied proposal to create rev ${res.data.currentPageRevisionNumber}.`
                  : action === 'requestChanges'
                    ? 'Requested changes on proposal.'
                    : 'Rejected proposal.',
              );
              return true;
            }
            handleFailureResponse(res, 'Could not record review decision.');
            return false;
          } catch (err) {
            setPanelError(typeof err === 'string' ? err : 'Review request failed.');
            return false;
          } finally {
            setBusy(false);
          }
        }}
        onOpenRevise={(rebaseOntoCurrent) => {
          setPanelError('');
          setDraftMode({
            kind: 'reviseProposal',
            pageId: selectedProposal.proposal.pageId,
            proposalId: selectedProposal.proposal.proposalId,
            baseRevisionNumber: rebaseOntoCurrent
              ? selectedProposal.currentPageRevisionNumber
              : selectedProposal.proposal.baseRevisionNumber,
            initialTitle: rebaseOntoCurrent
              ? selectedProposal.currentPageTitle
              : selectedProposal.proposedTitle,
            initialContentMd: rebaseOntoCurrent
              ? selectedProposal.currentPageContentMd
              : selectedProposal.proposedContentMd,
            initialSummary: selectedProposal.proposal.summary,
          });
        }}
        onWithdraw={async (idempotencyKey) => {
          setBusy(true);
          setPanelError('');
          try {
            const res = await withdrawProposal(
              workspaceId,
              selectedProposal.proposal.proposalId,
              idempotencyKey,
            );
            if (res.outcome === 'ok') {
              setSelectedProposal(res.data);
              setPanelNotice('Proposal withdrawn.');
              return true;
            }
            handleFailureResponse(res, 'Could not withdraw proposal.');
            return false;
          } catch (err) {
            setPanelError(typeof err === 'string' ? err : 'Could not withdraw proposal.');
            return false;
          } finally {
            setBusy(false);
          }
        }}
      />
    );
  }

  if (selectedPage) {
    const { page, currentRevision, threads, proposals } = selectedPage;
    const isArchived = page.archivedAtMs != null;
    const activeDoc = inspectedRevision ?? currentRevision;

    return (
      <div className="space-y-5">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <button
            type="button"
            onClick={() => {
              setSelectedPage(null);
              setInspectedRevision(null);
              setShowHistory(false);
              setPanelError('');
              void loadPagesList(includeArchived, null);
            }}
            className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            <span>Back to Pages</span>
          </button>

          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              onClick={() => setShowHistory((prev) => !prev)}
              className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
            >
              <History className="w-3.5 h-3.5" />
              <span>History ({revisionsList.length})</span>
            </button>
            {canAuthor && !isArchived && (
              <button
                type="button"
                onClick={() => {
                  setPanelError('');
                  setDraftMode({
                    kind: 'proposeEdit',
                    pageId: page.pageId,
                    baseRevisionNumber: page.currentRevisionNumber,
                    initialTitle: page.title,
                    initialContentMd: currentRevision.contentMd,
                  });
                }}
                className="sthira-button-primary inline-flex items-center gap-1.5 cursor-pointer"
              >
                <Edit3 className="w-3.5 h-3.5" />
                <span>Propose edit</span>
              </button>
            )}
            {canReviewOrArchive && (
              <button
                type="button"
                disabled={busy}
                onClick={() => void handleArchiveToggle(!isArchived)}
                className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
              >
                {isArchived ? <RotateCcw className="w-3.5 h-3.5" /> : <Archive className="w-3.5 h-3.5" />}
                <span>{isArchived ? 'Restore page' : 'Archive page'}</span>
              </button>
            )}
          </div>
        </div>

        {panelNotice && (
          <p role="status" className="rounded-xl border border-sthira-border bg-sthira-surface px-4 py-2.5 text-xs text-sthira-text">
            {panelNotice}
          </p>
        )}
        {panelError && (
          <p role="alert" className="rounded-xl border border-sthira-border bg-sthira-surface-muted px-4 py-2.5 text-xs text-sthira-danger">
            {panelError}
          </p>
        )}

        <article className="rounded-2xl border border-sthira-border bg-sthira-surface p-6 space-y-4">
          <div className="space-y-1.5 border-b border-sthira-border pb-4">
            <div className="flex flex-wrap items-center gap-2">
              <h2 className="text-lg font-semibold font-editorial text-sthira-text">
                {activeDoc.title}
              </h2>
              <span className="rounded px-2 py-0.5 text-[11px] font-medium border border-sthira-border bg-sthira-canvas text-sthira-text">
                Rev {activeDoc.revisionNumber}
              </span>
              {isArchived && (
                <span className="rounded px-2 py-0.5 text-[11px] font-medium border border-sthira-border bg-sthira-surface-muted text-sthira-danger">
                  Archived
                </span>
              )}
              {inspectedRevision && (
                <button
                  type="button"
                  onClick={() => setInspectedRevision(null)}
                  className="text-xs underline text-sthira-primary cursor-pointer"
                >
                  Return to current rev {page.currentRevisionNumber}
                </button>
              )}
            </div>
            <p className="text-xs text-sthira-text-muted">
              Origin: {page.provenance.originalAuthorLabel || page.publisherDisplayName}
              {page.provenance.sourceNote ? ` (${page.provenance.sourceNote})` : ''} · Published by{' '}
              {page.publisherDisplayName} · Revision author: {activeDoc.authorDisplayName}
              {activeDoc.reviewerDisplayName ? ` · Approved by ${activeDoc.reviewerDisplayName}` : ''} ·{' '}
              {formatDateTime(activeDoc.createdAtMs)}
            </p>
            {page.omittedSummary.length > 0 && (
              <p className="text-[11px] text-sthira-text-muted">
                Omitted on copy: {page.omittedSummary.join(' · ')}
              </p>
            )}
          </div>

          <RichDocumentRenderer content={activeDoc.contentMd} documentTitle={activeDoc.title} />
        </article>

        {showHistory && (
          <section
            aria-label="Revision history"
            className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-3"
          >
            <h3 className="text-sm font-semibold">Revision history</h3>
            <ul className="space-y-2">
              {revisionsList.map((rev) => (
                <li
                  key={rev.revisionId}
                  className="rounded-xl border border-sthira-border bg-sthira-canvas p-3 flex flex-wrap items-center justify-between gap-2 text-xs"
                >
                  <div className="min-w-0 space-y-0.5">
                    <p className="font-medium text-sthira-text">
                      Rev {rev.revisionNumber}: {rev.changeSummary}
                    </p>
                    <p className="text-sthira-text-muted">
                      Author: {rev.authorDisplayName}
                      {rev.reviewerDisplayName ? ` · Reviewer: ${rev.reviewerDisplayName}` : ''} ·{' '}
                      {formatDateTime(rev.createdAtMs)}
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={() => setInspectedRevision(rev)}
                    className="sthira-button-secondary cursor-pointer"
                  >
                    {inspectedRevision?.revisionId === rev.revisionId ? 'Viewing' : 'Inspect'}
                  </button>
                </li>
              ))}
            </ul>
            {revisionsCursor && (
              <button
                type="button"
                disabled={busy}
                onClick={() => void handleLoadMoreRevisions()}
                className="sthira-button-secondary cursor-pointer"
              >
                Load older revisions
              </button>
            )}
          </section>
        )}

        {/* Proposals section */}
        <section
          aria-label="Page change proposals"
          className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-3"
        >
          <div className="flex flex-wrap items-center justify-between gap-2">
            <h3 className="text-sm font-semibold flex items-center gap-1.5">
              <GitPullRequest className="w-4 h-4 text-sthira-primary" />
              <span>Change proposals ({proposals.length})</span>
            </h3>
          </div>
          {proposals.length === 0 ? (
            <p className="text-xs text-sthira-text-muted">
              No change proposals yet. Editors, Admins, and Owners can use Propose edit to suggest a reviewed revision.
            </p>
          ) : (
            <ul className="space-y-2">
              {proposals.map((pr) => (
                <li
                  key={pr.proposalId}
                  className="rounded-xl border border-sthira-border bg-sthira-canvas p-3 flex flex-wrap items-center justify-between gap-2 text-xs"
                >
                  <div className="min-w-0 space-y-0.5">
                    <div className="flex flex-wrap items-center gap-1.5">
                      <span className="font-medium text-sthira-text">{pr.summary}</span>
                      <span className="rounded px-1.5 py-0.5 text-[11px] border border-sthira-border bg-sthira-surface">
                        {pr.status}
                      </span>
                      {pr.isStale && (
                        <span className="rounded px-1.5 py-0.5 text-[11px] border border-sthira-border text-sthira-danger">
                          Stale (base rev {pr.baseRevisionNumber})
                        </span>
                      )}
                    </div>
                    <p className="text-sthira-text-muted">
                      By {pr.authorDisplayName} · Submission #{pr.submissionCount} · Base rev {pr.baseRevisionNumber}
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={() => void openProposalById(pr.proposalId)}
                    className="sthira-button-secondary cursor-pointer"
                  >
                    Open review
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>

        {/* Discussions section */}
        <section
          aria-label="Page discussion"
          className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-4"
        >
          <h3 className="text-sm font-semibold flex items-center gap-1.5">
            <MessageSquare className="w-4 h-4 text-sthira-primary" />
            <span>Discussion ({threads.length})</span>
          </h3>

          {canAuthor && !isArchived && (
            <form onSubmit={(e) => void handleStartThread(e)} className="space-y-2">
              <label className="block space-y-1">
                <span className="text-xs text-sthira-text-muted">
                  Start a comment thread (recorded against rev {currentRevision.revisionNumber})
                </span>
                <textarea
                  aria-label="New comment thread"
                  rows={2}
                  disabled={busy}
                  value={newThreadBody}
                  onChange={(e) => setNewThreadBody(e.target.value)}
                  placeholder="Share feedback or ask a question about this page…"
                  className="w-full rounded-lg border border-sthira-border bg-sthira-canvas p-2.5 text-xs text-sthira-text focus:outline-none"
                />
              </label>
              <button
                type="submit"
                disabled={busy || !newThreadBody.trim()}
                className="sthira-button-primary cursor-pointer"
              >
                Start thread
              </button>
            </form>
          )}

          {threads.length === 0 ? (
            <p className="text-xs text-sthira-text-muted">No comments yet.</p>
          ) : (
            <ul className="space-y-3" aria-label="Comment threads">
              {threads.map((thr) => {
                const canToggleResolve =
                  canAuthor && (canReviewOrArchive || (!!currentUid && thr.authorUid === currentUid));
                const isResolved = thr.status === 'resolved';
                return (
                  <li
                    key={thr.threadId}
                    className="rounded-xl border border-sthira-border bg-sthira-canvas p-3.5 space-y-2.5 text-xs"
                  >
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <div className="flex flex-wrap items-center gap-1.5">
                        <span className="font-semibold text-sthira-text">{thr.authorDisplayName}</span>
                        <span className="rounded px-1.5 py-0.5 text-[11px] border border-sthira-border bg-sthira-surface text-sthira-text-muted">
                          Started on rev {thr.baseRevisionNumber}
                        </span>
                        <span className="rounded px-1.5 py-0.5 text-[11px] border border-sthira-border bg-sthira-surface text-sthira-text inline-flex items-center gap-1">
                          {isResolved && <CheckCircle2 className="w-3 h-3 text-sthira-primary" />}
                          <span>{isResolved ? 'Resolved' : 'Open'}</span>
                        </span>
                      </div>
                      {canToggleResolve && (
                        <button
                          type="button"
                          disabled={busy}
                          aria-label={`${isResolved ? 'Reopen' : 'Resolve'} thread by ${thr.authorDisplayName}`}
                          onClick={() => void handleThreadStatus(thr.threadId, !isResolved)}
                          className="sthira-button-secondary cursor-pointer"
                        >
                          {isResolved ? 'Reopen thread' : 'Resolve thread'}
                        </button>
                      )}
                    </div>

                    <div className="space-y-2 pl-2 border-l-2 border-sthira-border">
                      {thr.messages.map((msg) => (
                        <div key={msg.messageId} className="space-y-0.5">
                          <p className="text-[11px] text-sthira-text-muted">
                            <strong className="text-sthira-text">{msg.authorDisplayName}</strong> ·{' '}
                            {formatDateTime(msg.createdAtMs)}
                          </p>
                          <p className="text-sthira-text whitespace-pre-wrap">{msg.body}</p>
                        </div>
                      ))}
                    </div>

                    {canAuthor && !isArchived && (
                      <div className="flex flex-wrap items-center gap-2 pt-1">
                        <input
                          aria-label={`Reply to ${thr.authorDisplayName}`}
                          type="text"
                          disabled={busy}
                          value={replyDrafts[thr.threadId] || ''}
                          onChange={(e) =>
                            setReplyDrafts((prev) => ({ ...prev, [thr.threadId]: e.target.value }))
                          }
                          placeholder="Write a reply…"
                          className="flex-1 min-w-[180px] min-h-[36px] rounded-lg border border-sthira-border bg-sthira-surface px-2.5 text-xs text-sthira-text focus:outline-none"
                        />
                        <button
                          type="button"
                          disabled={busy || !(replyDrafts[thr.threadId] || '').trim()}
                          aria-label={`Post reply to ${thr.authorDisplayName}`}
                          onClick={() => void handleReplyThread(thr.threadId)}
                          className="sthira-button-secondary cursor-pointer"
                        >
                          Reply
                        </button>
                      </div>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
        </section>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap items-center gap-2">
          {canAuthor && (
            <>
              <button
                type="button"
                onClick={() => {
                  setPanelError('');
                  setDraftMode({ kind: 'newPage' });
                }}
                className="sthira-button-primary inline-flex items-center gap-1.5 cursor-pointer"
              >
                <FilePlus className="w-3.5 h-3.5" />
                <span>New shared page</span>
              </button>
              <button
                type="button"
                onClick={() => setPublishCopyOpen(true)}
                className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
              >
                <CopyPlus className="w-3.5 h-3.5" />
                <span>Publish a copy</span>
              </button>
            </>
          )}
        </div>

        <label className="inline-flex items-center gap-2 text-xs text-sthira-text-muted cursor-pointer">
          <input
            type="checkbox"
            checked={includeArchived}
            onChange={(e) => setIncludeArchived(e.target.checked)}
          />
          <span>Show archived</span>
        </label>
      </div>

      {panelError && (
        <p role="alert" className="rounded-xl border border-sthira-border bg-sthira-surface-muted px-4 py-2.5 text-xs text-sthira-danger">
          {panelError}
        </p>
      )}

      <section className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-3">
        <div className="flex items-center justify-between gap-2">
          <h2 className="text-sm font-semibold flex items-center gap-1.5">
            <FileText className="w-4 h-4 text-sthira-primary" />
            <span>Shared pages ({pages.length})</span>
          </h2>
          <span className="text-xs text-sthira-text-muted">
            {canAuthor
              ? 'All workspace members can read published pages'
              : 'Read-only access (Viewer)'}
          </span>
        </div>

        {loadingList && pages.length === 0 ? (
          <p role="status" className="text-xs text-sthira-text-muted py-3">
            Loading shared pages…
          </p>
        ) : pages.length === 0 ? (
          <p className="text-xs text-sthira-text-muted py-3">
            {includeArchived
              ? 'No shared or archived pages in this workspace yet.'
              : 'No shared pages yet. Use New shared page or Publish a copy from Personal.'}
          </p>
        ) : (
          <ul className="space-y-2" aria-label="Shared pages list">
            {pages.map((p) => (
              <li
                key={p.pageId}
                className="rounded-xl border border-sthira-border bg-sthira-canvas p-3.5 flex flex-wrap items-center justify-between gap-3"
              >
                <div className="min-w-0 flex-1 space-y-0.5">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-medium text-sm text-sthira-text truncate">{p.title}</span>
                    <span className="rounded px-1.5 py-0.5 text-[11px] border border-sthira-border bg-sthira-surface text-sthira-text-muted">
                      Rev {p.currentRevisionNumber}
                    </span>
                    {p.archivedAtMs && (
                      <span className="rounded px-1.5 py-0.5 text-[11px] border border-sthira-border text-sthira-danger">
                        Archived
                      </span>
                    )}
                  </div>
                  <p className="text-xs text-sthira-text-muted">
                    Published by {p.publisherDisplayName} · Last edited by {p.lastEditorDisplayName} ·{' '}
                    {p.unresolvedThreadCount} open threads · {p.pendingProposalCount} pending proposals
                  </p>
                </div>
                <button
                  type="button"
                  aria-label={`Open ${p.title}`}
                  onClick={() => void openPageById(p.pageId)}
                  className="sthira-button-secondary cursor-pointer"
                >
                  Open
                </button>
              </li>
            ))}
          </ul>
        )}

        {pagesNextCursor && (
          <button
            type="button"
            disabled={loadingList}
            onClick={() => void loadPagesList(includeArchived, pagesNextCursor)}
            className="sthira-button-secondary cursor-pointer"
          >
            Load more pages
          </button>
        )}
      </section>

      <PublishPersonalCopyModal
        isOpen={publishCopyOpen}
        workspaceId={workspaceId}
        workspaceName={workspaceName}
        authorDisplayName={authorDisplayName}
        onPublished={(published) => {
          setPublishCopyOpen(false);
          setSelectedPage(published);
          setRevisionsList(published.revisions);
          setRevisionsCursor(published.revisionsNextCursor ?? null);
          setPanelNotice(`Published copy “${published.page.title}”.`);
          void loadPagesList(includeArchived, null);
        }}
        onAccessLost={onAccessLost}
        onSessionChange={onSessionChange}
        onClose={() => setPublishCopyOpen(false)}
      />
    </div>
  );
};

interface SharedActivityFeedProps {
  workspaceId: string;
  refreshSignal: number;
  onOpenPage: (pageId: string, proposalId?: string | null) => void;
  onAccessLost: (message: string, workspaces?: WorkspaceListResponse | null) => void;
  onSessionChange: (session: GoogleSession) => void;
}

export const SharedActivityFeed: React.FC<SharedActivityFeedProps> = ({
  workspaceId,
  refreshSignal,
  onOpenPage,
  onAccessLost,
  onSessionChange,
}) => {
  const [items, setItems] = useState<SharedActivityItem[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const seqRef = useRef(0);
  const callbacksRef = useRef({ onAccessLost, onSessionChange });
  callbacksRef.current = { onAccessLost, onSessionChange };

  const loadFeed = useCallback(
    async (cursor?: string | null) => {
      const seq = ++seqRef.current;
      setLoading(true);
      if (!cursor) setError('');
      try {
        const res = await listWorkspaceActivity(workspaceId, cursor ?? null);
        if (seq !== seqRef.current) return;
        if (res.outcome === 'ok') {
          setItems((prev) => (cursor ? [...prev, ...res.data.items] : res.data.items));
          setNextCursor(res.data.nextCursor ?? null);
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
              : 'Sign in with Google to view team activity.',
          );
          return;
        }
        setError(res.message);
      } catch (err) {
        if (seq !== seqRef.current) return;
        setError(typeof err === 'string' ? err : 'Could not load workspace activity.');
      } finally {
        if (seq === seqRef.current) setLoading(false);
      }
    },
    [workspaceId],
  );

  useEffect(() => {
    setItems([]);
    void loadFeed(null);
  }, [loadFeed, refreshSignal, workspaceId]);

  return (
    <section className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="text-sm font-semibold">Workspace activity ({items.length})</h2>
        <button
          type="button"
          disabled={loading}
          onClick={() => void loadFeed(null)}
          className="sthira-button-secondary cursor-pointer"
        >
          Refresh activity
        </button>
      </div>

      {error && (
        <p role="alert" className="rounded-xl border border-sthira-border bg-sthira-surface-muted px-3.5 py-2.5 text-xs text-sthira-danger">
          {error}
        </p>
      )}

      {loading && items.length === 0 ? (
        <p role="status" className="text-xs text-sthira-text-muted">Loading activity…</p>
      ) : items.length === 0 ? (
        <p className="text-xs text-sthira-text-muted">No workspace activity yet.</p>
      ) : (
        <ul className="space-y-2" aria-label="Activity list">
          {items.map((item) => (
            <li
              key={item.activityId}
              className="rounded-xl border border-sthira-border bg-sthira-canvas p-3 flex flex-wrap items-center justify-between gap-2 text-xs"
            >
              <div className="min-w-0 space-y-0.5">
                <p className="text-sthira-text font-medium">{item.detailText}</p>
                <p className="text-sthira-text-muted">
                  {item.actorDisplayName} · {formatDateTime(item.createdAtMs)}
                </p>
              </div>
              {item.pageId && (
                <button
                  type="button"
                  onClick={() => onOpenPage(item.pageId!, item.proposalId)}
                  className="sthira-button-secondary cursor-pointer"
                >
                  {item.proposalId ? 'Open proposal' : 'Open page'}
                </button>
              )}
            </li>
          ))}
        </ul>
      )}

      {nextCursor && (
        <button
          type="button"
          disabled={loading}
          onClick={() => void loadFeed(nextCursor)}
          className="sthira-button-secondary cursor-pointer"
        >
          Load more activity
        </button>
      )}
    </section>
  );
};
