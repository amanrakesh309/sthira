import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  AlertTriangle,
  ArrowLeft,
  Code,
  GitBranch,
  Heading1,
  Heading2,
  ListTodo,
  Send,
  Table,
} from 'lucide-react';
import { DocumentEditor, type DocumentEditorHandle } from './DocumentEditor';
import { RichDocumentRenderer } from './RichDocumentRenderer';
import {
  MAX_SHARED_PAGE_CONTENT_BYTES,
  MAX_SHARED_PROPOSAL_SUMMARY_LEN,
  utf8ByteLength,
  validateSharedPageInput,
} from '../api/shareSanitizer';
import {
  deleteSharedDraft,
  getSharedDraft,
  makeRequestIdempotencyKey,
  saveSharedDraft,
} from '../api/workspaces';

export type SharedDraftEditorMode =
  | { kind: 'newPage' }
  | {
      kind: 'proposeEdit';
      pageId: string;
      baseRevisionNumber: number;
      initialTitle: string;
      initialContentMd: string;
    }
  | {
      kind: 'reviseProposal';
      pageId: string;
      proposalId: string;
      baseRevisionNumber: number;
      initialTitle: string;
      initialContentMd: string;
      initialSummary: string;
    };

export interface CurrentPublishedDocInfo {
  revisionNumber: number;
  title: string;
  contentMd: string;
}

interface SharedPageDraftEditorProps {
  workspaceId: string;
  accountUid?: string | null;
  mode: SharedDraftEditorMode;
  currentPublishedDoc?: CurrentPublishedDocInfo | null;
  registerBeforeLeave?: (guard: (() => Promise<boolean>) | null) => void;
  submitting: boolean;
  error: string;
  onClearError: () => void;
  onCancel: () => void;
  onSubmit: (payload: {
    title: string;
    contentMd: string;
    summary: string;
    baseRevisionNumber?: number;
    idempotencyKey: string;
  }) => Promise<boolean>;
}

export function sharedDraftKeyForMode(mode: SharedDraftEditorMode): string {
  switch (mode.kind) {
    case 'newPage':
      return 'new_page';
    case 'proposeEdit':
      return `page_${mode.pageId}`;
    case 'reviseProposal':
      return `proposal_${mode.proposalId}`;
  }
}

export const SharedPageDraftEditor: React.FC<SharedPageDraftEditorProps> = ({
  workspaceId,
  accountUid,
  mode,
  currentPublishedDoc,
  registerBeforeLeave,
  submitting,
  error,
  onClearError,
  onCancel,
  onSubmit,
}) => {
  const draftKey = sharedDraftKeyForMode(mode);
  const currentTargetRevision =
    mode.kind === 'newPage'
      ? undefined
      : (currentPublishedDoc?.revisionNumber ?? mode.baseRevisionNumber);
  const initialBaseRevision =
    mode.kind === 'newPage' ? undefined : mode.baseRevisionNumber;
  const initialTitle = mode.kind === 'newPage' ? '' : mode.initialTitle;
  const initialContentMd = mode.kind === 'newPage' ? '' : mode.initialContentMd;
  const initialSummary =
    mode.kind === 'reviseProposal' ? mode.initialSummary : '';

  const [title, setTitle] = useState(initialTitle);
  const [contentMd, setContentMd] = useState(initialContentMd);
  const [summary, setSummary] = useState(initialSummary);
  const [effectiveBaseRevision, setEffectiveBaseRevision] = useState<number | undefined>(
    initialBaseRevision,
  );
  const [localError, setLocalError] = useState('');
  const [draftNotice, setDraftNotice] = useState('');
  const [restoredFromDraft, setRestoredFromDraft] = useState(false);
  const [showCompare, setShowCompare] = useState(false);
  const [restoreState, setRestoreState] = useState<'loading' | 'ready' | 'error'>('loading');
  const [restoreAttempt, setRestoreAttempt] = useState(0);
  const [finishing, setFinishing] = useState(false);
  const [writeFailed, setWriteFailed] = useState(false);

  const editorRef = useRef<DocumentEditorHandle>(null);
  const idempotencyKeyRef = useRef<string>(makeRequestIdempotencyKey('draft'));
  const fingerprintRef = useRef<string>('');
  const mountedRef = useRef(true);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const contextEpochRef = useRef(0);
  const writeSeqRef = useRef(0);
  const pendingDirtyRef = useRef(false);
  const terminalClearedRef = useRef(false);
  const finishingRef = useRef(false);
  const writeChainRef = useRef<Promise<void>>(Promise.resolve());

  const latestDraftRef = useRef({
    title: initialTitle,
    contentMd: initialContentMd,
    summary: initialSummary,
    baseRevisionNumber: initialBaseRevision,
  });
  const targetRevisionRef = useRef(currentTargetRevision);
  targetRevisionRef.current = currentTargetRevision;

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      if (saveTimerRef.current) {
        clearTimeout(saveTimerRef.current);
        saveTimerRef.current = null;
      }
    };
  }, []);

  const enqueueSerializedOp = useCallback(
    (op: () => Promise<void>): Promise<void> => {
      const next = writeChainRef.current.then(op, op);
      writeChainRef.current = next.catch(() => {});
      return next;
    },
    [],
  );

  const executeSerializedSave = useCallback(
    async (
      epoch: number,
      seq: number,
      draft: {
        title: string;
        contentMd: string;
        summary: string;
        baseRevisionNumber?: number;
      },
    ): Promise<boolean> => {
      let ok = false;
      await enqueueSerializedOp(async () => {
        if (!mountedRef.current || epoch !== contextEpochRef.current || terminalClearedRef.current) {
          return;
        }
        if (seq !== writeSeqRef.current) return;
        if (!pendingDirtyRef.current) {
          ok = true;
          return;
        }
        try {
          const rec = await saveSharedDraft({
            workspaceId,
            draftKey,
            pageId: mode.kind === 'newPage' ? null : mode.pageId,
            proposalId: mode.kind === 'reviseProposal' ? mode.proposalId : null,
            baseRevisionNumber: draft.baseRevisionNumber ?? null,
            title: draft.title,
            contentMd: draft.contentMd,
            summary: draft.summary,
            accountUid,
          });
          if (epoch !== contextEpochRef.current || terminalClearedRef.current) return;
          if (!rec) return;
          if (seq === writeSeqRef.current) {
            pendingDirtyRef.current = false;
          }
          if (mountedRef.current) {
            if (seq === writeSeqRef.current) {
              setLocalError('');
              setWriteFailed(false);
              setDraftNotice(
                rec.baseRevisionNumber !== null && rec.baseRevisionNumber !== undefined
                  ? `Saved private draft locally (base rev ${rec.baseRevisionNumber})`
                  : 'Saved private draft locally',
              );
            }
          }
          ok = true;
        } catch (err) {
          if (epoch !== contextEpochRef.current || seq !== writeSeqRef.current || !mountedRef.current) return;
          const msg =
            typeof err === 'string'
              ? err
              : 'Could not save private local draft; keep this editor open to preserve your work.';
          setDraftNotice('');
          setLocalError(msg);
          setWriteFailed(true);
          ok = false;
        }
      });
      return ok;
    },
    [accountUid, draftKey, enqueueSerializedOp, mode, workspaceId],
  );

  // Load any previously saved private local draft for (accountUid, workspaceId, draftKey).
  useEffect(() => {
    const epoch = ++contextEpochRef.current;
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
      saveTimerRef.current = null;
    }
    pendingDirtyRef.current = false;
    terminalClearedRef.current = false;
    writeSeqRef.current += 1;

    setLocalError('');
    setWriteFailed(false);
    setRestoreState('loading');
    setRestoredFromDraft(false);
    setDraftNotice('');
    setShowCompare(false);
    setTitle(initialTitle);
    setContentMd(initialContentMd);
    setSummary(initialSummary);
    setEffectiveBaseRevision(initialBaseRevision);
    latestDraftRef.current = {
      title: initialTitle,
      contentMd: initialContentMd,
      summary: initialSummary,
      baseRevisionNumber: initialBaseRevision,
    };
    idempotencyKeyRef.current = makeRequestIdempotencyKey('draft');

    void (async () => {
      if (epoch !== contextEpochRef.current) return;
      try {
        const saved = await getSharedDraft(workspaceId, draftKey, accountUid);
        if (epoch !== contextEpochRef.current || !mountedRef.current) return;
        setRestoreState('ready');
        if (!saved) return;
        const restoredBase =
          saved.baseRevisionNumber !== undefined && saved.baseRevisionNumber !== null
            ? saved.baseRevisionNumber
            : initialBaseRevision;
        if (
          targetRevisionRef.current !== undefined &&
          restoredBase !== undefined &&
          restoredBase !== targetRevisionRef.current
        ) {
          setDraftNotice(
            `Recovered unsent local draft based on rev ${restoredBase}.`,
          );
        } else {
          setDraftNotice(
            restoredBase !== undefined
              ? `Restored private local draft (based on rev ${restoredBase}).`
              : 'Restored private local draft.',
          );
        }
        setEffectiveBaseRevision(restoredBase);
        setTitle(saved.title);
        setContentMd(saved.contentMd);
        setSummary(saved.summary);
        latestDraftRef.current = {
          title: saved.title,
          contentMd: saved.contentMd,
          summary: saved.summary,
          baseRevisionNumber: restoredBase,
        };
        setRestoredFromDraft(true);
      } catch (err) {
        if (epoch !== contextEpochRef.current || !mountedRef.current) return;
        const msg =
          typeof err === 'string'
            ? err
            : 'Could not read saved local draft.';
        setLocalError(msg);
        setRestoreState('error');
      }
    })();

    return () => {
      contextEpochRef.current += 1;
      if (saveTimerRef.current) {
        clearTimeout(saveTimerRef.current);
        saveTimerRef.current = null;
      }
    };
  }, [
    accountUid,
    draftKey,
    initialBaseRevision,
    initialContentMd,
    initialSummary,
    initialTitle,
    restoreAttempt,
    workspaceId,
  ]);

  const flushLatestDraft = useCallback(async (): Promise<boolean> => {
    const epoch = contextEpochRef.current;
    while (mountedRef.current && epoch === contextEpochRef.current) {
      if (saveTimerRef.current) {
        clearTimeout(saveTimerRef.current);
        saveTimerRef.current = null;
      }
      if (terminalClearedRef.current || !pendingDirtyRef.current) return true;
      const seq = writeSeqRef.current;
      const saved = await executeSerializedSave(epoch, seq, latestDraftRef.current);
      if (epoch === contextEpochRef.current && seq === writeSeqRef.current && !saved) {
        return false;
      }
    }
    return false;
  }, [executeSerializedSave]);

  useEffect(() => {
    registerBeforeLeave?.(async () => !finishingRef.current && flushLatestDraft());
    return () => registerBeforeLeave?.(null);
  }, [flushLatestDraft, registerBeforeLeave]);

  const persistLocalDraft = useCallback(
    (
      nextTitle: string,
      nextContent: string,
      nextSummary: string,
      nextBaseRev: number | undefined = effectiveBaseRevision,
    ) => {
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      const epoch = contextEpochRef.current;
      const seq = ++writeSeqRef.current;
      pendingDirtyRef.current = true;
      terminalClearedRef.current = false;
      saveTimerRef.current = setTimeout(() => {
        saveTimerRef.current = null;
        void executeSerializedSave(epoch, seq, {
          title: nextTitle,
          contentMd: nextContent,
          summary: nextSummary,
          baseRevisionNumber: nextBaseRev,
        });
      }, 180);
    },
    [effectiveBaseRevision, executeSerializedSave],
  );

  const handleFieldChange = (nextTitle: string, nextContent: string, nextSummary: string) => {
    if (restoreState !== 'ready' || finishingRef.current) return;
    latestDraftRef.current = {
      title: nextTitle,
      contentMd: nextContent,
      summary: nextSummary,
      baseRevisionNumber: effectiveBaseRevision,
    };
    setLocalError('');
    setDraftNotice('');
    setWriteFailed(false);
    onClearError();
    const fp = `${effectiveBaseRevision ?? 0}:${nextTitle}:${nextContent}:${nextSummary}`;
    if (fp !== fingerprintRef.current) {
      fingerprintRef.current = fp;
      idempotencyKeyRef.current = makeRequestIdempotencyKey('draft');
    }
    persistLocalDraft(nextTitle, nextContent, nextSummary, effectiveBaseRevision);
  };

  const handleExplicitRebase = async () => {
    if (currentTargetRevision === undefined || submitting || finishingRef.current || restoreState !== 'ready') return;
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
      saveTimerRef.current = null;
    }
    const nextBase = currentTargetRevision;
    setEffectiveBaseRevision(nextBase);
    latestDraftRef.current = { ...latestDraftRef.current, baseRevisionNumber: nextBase };
    fingerprintRef.current = `${nextBase}:${title}:${contentMd}:${summary}`;
    idempotencyKeyRef.current = makeRequestIdempotencyKey('draft');
    const epoch = contextEpochRef.current;
    const seq = ++writeSeqRef.current;
    pendingDirtyRef.current = true;
    const ok = await executeSerializedSave(epoch, seq, {
      title,
      contentMd,
      summary,
      baseRevisionNumber: nextBase,
    });
    if (ok && mountedRef.current && seq === writeSeqRef.current) {
      setShowCompare(false);
      setDraftNotice(
        `Rebased local draft onto rev ${nextBase} with your unsent text intact.`,
      );
    }
  };

  const byteCount = utf8ByteLength(contentMd.replace(/\r\n/g, '\n'));
  const needsSummary = mode.kind !== 'newPage';
  const editorLocked = submitting || finishing || restoreState !== 'ready';
  const needsExplicitRebase =
    mode.kind !== 'newPage' &&
    effectiveBaseRevision !== undefined &&
    currentTargetRevision !== undefined &&
    effectiveBaseRevision !== currentTargetRevision;

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (submitting || finishingRef.current || restoreState !== 'ready') return;
    if (needsExplicitRebase) {
      setLocalError(
        `Compare with current rev ${currentTargetRevision} and click Rebase onto rev ${currentTargetRevision} before submitting.`,
      );
      return;
    }
    const validationErr = validateSharedPageInput(title, contentMd);
    if (validationErr) {
      setLocalError(validationErr);
      return;
    }
    const cleanSummary = summary.trim();
    if (needsSummary && !cleanSummary) {
      setLocalError('Change summary is required for proposals.');
      return;
    }
    if (cleanSummary.length > MAX_SHARED_PROPOSAL_SUMMARY_LEN) {
      setLocalError(`Change summary must be at most ${MAX_SHARED_PROPOSAL_SUMMARY_LEN} characters.`);
      return;
    }

    const draft = {
      title: title.trim(),
      contentMd,
      summary: cleanSummary,
      baseRevisionNumber: effectiveBaseRevision,
    };
    const epoch = contextEpochRef.current;
    const idempotencyKey = idempotencyKeyRef.current;
    finishingRef.current = true;
    setFinishing(true);
    try {
      latestDraftRef.current = draft;
      writeSeqRef.current += 1;
      pendingDirtyRef.current = true;
      if (!(await flushLatestDraft()) || !mountedRef.current || epoch !== contextEpochRef.current) return;

      if (await onSubmit({ ...draft, idempotencyKey })) {
        terminalClearedRef.current = true;
        pendingDirtyRef.current = false;
        writeSeqRef.current += 1;
        await enqueueSerializedOp(async () => {
          await deleteSharedDraft(workspaceId, draftKey, accountUid).catch(() => {});
        });
      }
    } finally {
      finishingRef.current = false;
      if (mountedRef.current) setFinishing(false);
    }
  };

  const handleDiscardDraft = async () => {
    if (submitting || finishingRef.current || restoreState !== 'ready') return;
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
      saveTimerRef.current = null;
    }
    const wasDirty = pendingDirtyRef.current;
    finishingRef.current = true;
    setFinishing(true);
    terminalClearedRef.current = true;
    pendingDirtyRef.current = false;
    writeSeqRef.current += 1;
    try {
      await enqueueSerializedOp(() => deleteSharedDraft(workspaceId, draftKey, accountUid));
      if (mountedRef.current) onCancel();
    } catch {
      terminalClearedRef.current = false;
      pendingDirtyRef.current = wasDirty;
      if (mountedRef.current) setLocalError('Could not clear the local draft. Try again.');
    } finally {
      finishingRef.current = false;
      if (mountedRef.current) setFinishing(false);
    }
  };

  const handleBack = async () => {
    if (submitting || finishingRef.current) return;
    const ok = await flushLatestDraft();
    if (ok) onCancel();
  };

  return (
    <form onSubmit={(e) => void handleSubmit(e)} className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <button
          type="button"
          disabled={submitting || finishing}
          onClick={() => void handleBack()}
          className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
        >
          <ArrowLeft className="w-3.5 h-3.5" />
          <span>Back</span>
        </button>

        <div className="flex flex-wrap items-center gap-2 text-[11px] text-sthira-text-muted">
          {effectiveBaseRevision !== undefined && (
            <span className="rounded px-2 py-0.5 border border-sthira-border bg-sthira-canvas">
              Base rev {effectiveBaseRevision}
            </span>
          )}
          <span
            className={
              byteCount > MAX_SHARED_PAGE_CONTENT_BYTES ? 'text-sthira-danger font-medium' : ''
            }
          >
            {byteCount.toLocaleString()} / {MAX_SHARED_PAGE_CONTENT_BYTES.toLocaleString()} B
          </span>
        </div>
      </div>

      {needsExplicitRebase && (
        <div
          role="status"
          className="rounded-xl border border-sthira-border bg-sthira-surface-muted p-4 space-y-3 text-xs"
        >
          <div className="flex items-start gap-2.5">
            <AlertTriangle className="w-4 h-4 text-sthira-danger shrink-0 mt-0.5" />
            <div className="space-y-1 flex-1">
              <p className="font-semibold text-sthira-text">
                Draft base rev {effectiveBaseRevision} differs from current published rev{' '}
                {currentTargetRevision}
              </p>
              <p className="text-sthira-text-muted">
                Compare your unsent draft against rev {currentTargetRevision} and confirm rebase before submitting. Your draft edits stay preserved.
              </p>
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            {currentPublishedDoc && (
              <button
                type="button"
                disabled={editorLocked}
                onClick={() => setShowCompare((prev) => !prev)}
                className="sthira-button-secondary cursor-pointer"
              >
                {showCompare
                  ? 'Hide rev comparison'
                  : `Compare draft (rev ${effectiveBaseRevision}) with rev ${currentTargetRevision}`}
              </button>
            )}
            <button
              type="button"
              disabled={editorLocked}
              onClick={() => void handleExplicitRebase()}
              className="sthira-button-primary inline-flex items-center gap-1.5 cursor-pointer"
            >
              <GitBranch className="w-3.5 h-3.5" />
              <span>Rebase onto rev {currentTargetRevision}</span>
            </button>
          </div>
          {showCompare && currentPublishedDoc && (
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-3 pt-1">
              <div className="rounded-xl border border-sthira-border bg-sthira-canvas p-3 space-y-1.5 min-w-0">
                <p className="text-[11px] font-semibold uppercase tracking-wider text-sthira-text-subtle">
                  Current published (rev {currentPublishedDoc.revisionNumber})
                </p>
                <p className="font-semibold text-sthira-text">{currentPublishedDoc.title}</p>
                <RichDocumentRenderer
                  content={currentPublishedDoc.contentMd}
                  documentTitle={currentPublishedDoc.title}
                />
              </div>
              <div className="rounded-xl border border-sthira-border bg-sthira-canvas p-3 space-y-1.5 min-w-0">
                <p className="text-[11px] font-semibold uppercase tracking-wider text-sthira-primary">
                  Your unsent draft (base rev {effectiveBaseRevision})
                </p>
                <p className="font-semibold text-sthira-text">{title || 'Untitled'}</p>
                <RichDocumentRenderer content={contentMd} documentTitle={title} />
              </div>
            </div>
          )}
        </div>
      )}

      {(draftNotice || restoredFromDraft) && (
        <div
          role={draftNotice ? 'status' : undefined}
          className="rounded-xl border border-sthira-border bg-sthira-surface px-3.5 py-2 text-xs text-sthira-text-muted flex flex-wrap items-center justify-between gap-2"
        >
          {draftNotice && <span>{draftNotice}</span>}
          {restoredFromDraft && (
            <button
              type="button"
              disabled={editorLocked}
              onClick={() => void handleDiscardDraft()}
              className="underline text-sthira-text cursor-pointer"
            >
              Discard local draft
            </button>
          )}
        </div>
      )}

      {(localError || error) && (
        <p
          role="alert"
          className="rounded-xl border border-sthira-border bg-sthira-surface-muted px-3.5 py-2.5 text-xs text-sthira-danger"
        >
          {localError || error}
        </p>
      )}

      {restoreState === 'loading' && <p role="status" className="text-xs text-sthira-text-muted">Loading local draft…</p>}
      {restoreState === 'error' && (
        <button type="button" className="sthira-button-secondary" onClick={() => setRestoreAttempt(n => n + 1)}>
          Retry draft recovery
        </button>
      )}
      {writeFailed && (
        <button type="button" className="sthira-button-secondary" disabled={editorLocked} onClick={() => void flushLatestDraft()}>
          Retry saving draft
        </button>
      )}

      <div className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-4">
        <label className="block space-y-1">
          <span className="text-xs font-medium text-sthira-text">Page title</span>
          <input
            aria-label="Shared page title"
            type="text"
            required
            maxLength={160}
            disabled={editorLocked}
            value={title}
            onChange={(e) => {
              const next = e.target.value;
              setTitle(next);
              handleFieldChange(next, contentMd, summary);
            }}
            placeholder="Title for this shared page"
            className="w-full min-h-[42px] rounded-lg border border-sthira-border bg-sthira-canvas px-3 text-sm font-semibold text-sthira-text focus:outline-none"
          />
        </label>

        {needsSummary && (
          <label className="block space-y-1">
            <span className="text-xs font-medium text-sthira-text">Change summary</span>
            <input
              aria-label="Change summary"
              type="text"
              required
              maxLength={MAX_SHARED_PROPOSAL_SUMMARY_LEN}
              disabled={editorLocked}
              value={summary}
              onChange={(e) => {
                const next = e.target.value;
                setSummary(next);
                handleFieldChange(title, contentMd, next);
              }}
              placeholder="Briefly describe what changed and why"
              className="w-full min-h-[40px] rounded-lg border border-sthira-border bg-sthira-canvas px-3 text-xs text-sthira-text focus:outline-none"
            />
          </label>
        )}

        <div className="space-y-1.5">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <span className="text-xs font-medium text-sthira-text">Content</span>
            <div className="flex flex-wrap items-center gap-1" role="toolbar" aria-label="Formatting">
              <button
                type="button"
                disabled={editorLocked}
                aria-label="Heading 1"
                onClick={() => editorRef.current?.format('h1')}
                className="p-1.5 rounded border border-sthira-border bg-sthira-canvas hover:bg-sthira-surface-muted cursor-pointer"
              >
                <Heading1 className="w-3.5 h-3.5" />
              </button>
              <button
                type="button"
                disabled={editorLocked}
                aria-label="Heading 2"
                onClick={() => editorRef.current?.format('h2')}
                className="p-1.5 rounded border border-sthira-border bg-sthira-canvas hover:bg-sthira-surface-muted cursor-pointer"
              >
                <Heading2 className="w-3.5 h-3.5" />
              </button>
              <button
                type="button"
                disabled={editorLocked}
                aria-label="Task list"
                onClick={() => editorRef.current?.format('todo')}
                className="p-1.5 rounded border border-sthira-border bg-sthira-canvas hover:bg-sthira-surface-muted cursor-pointer"
              >
                <ListTodo className="w-3.5 h-3.5" />
              </button>
              <button
                type="button"
                disabled={editorLocked}
                aria-label="Table"
                onClick={() => editorRef.current?.format('table')}
                className="p-1.5 rounded border border-sthira-border bg-sthira-canvas hover:bg-sthira-surface-muted cursor-pointer"
              >
                <Table className="w-3.5 h-3.5" />
              </button>
              <button
                type="button"
                disabled={editorLocked}
                aria-label="Code block"
                onClick={() => editorRef.current?.format('code')}
                className="p-1.5 rounded border border-sthira-border bg-sthira-canvas hover:bg-sthira-surface-muted cursor-pointer"
              >
                <Code className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>

          <div className="focus-editor-body min-h-[240px] rounded-xl border border-sthira-border bg-sthira-canvas p-4">
            <DocumentEditor
              ref={editorRef}
              content={contentMd}
              title={title}
              disabled={editorLocked}
              objectTypes={[]}
              onChange={(nextMd) => {
                setContentMd(nextMd);
                handleFieldChange(title, nextMd, summary);
              }}
              onBlur={() => {
                void flushLatestDraft();
              }}
              onOpenEntity={() => {}}
              onSlash={() => {}}
            />
          </div>
        </div>

        <div className="flex flex-wrap items-center justify-end gap-2 pt-1">
          <button
            type="button"
            disabled={editorLocked}
            onClick={() => void handleDiscardDraft()}
            className="sthira-button-secondary cursor-pointer"
          >
            Cancel & clear draft
          </button>
          <button
            type="submit"
            disabled={editorLocked || needsExplicitRebase}
            className="sthira-button-primary inline-flex items-center gap-1.5 cursor-pointer"
          >
            <Send className="w-3.5 h-3.5" />
            <span>
              {submitting || finishing
                ? 'Submitting…'
                : mode.kind === 'newPage'
                  ? 'Publish shared page'
                  : mode.kind === 'reviseProposal'
                    ? 'Resubmit for review'
                    : 'Submit proposal'}
            </span>
          </button>
        </div>
      </div>
    </form>
  );
};
