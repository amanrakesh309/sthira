import React, { useRef, useState } from 'react';
import { AlertTriangle, ArrowLeft, CheckCircle2, Edit3, GitCommit, RefreshCw, XCircle } from 'lucide-react';
import type {
  ProposalReviewAction,
  ProposalStatus,
  SharedProposalDetails,
} from '../api/workspaces';
import { makeRequestIdempotencyKey } from '../api/workspaces';
import { RichDocumentRenderer } from './RichDocumentRenderer';

interface SharedProposalReviewProps {
  details: SharedProposalDetails;
  currentUid?: string | null;
  busy: boolean;
  error: string;
  onRefresh: () => void;
  onBack: () => void;
  onReview: (
    action: ProposalReviewAction,
    note: string | null,
    idempotencyKey: string,
    expectedSubmissionNumber: number,
  ) => Promise<boolean>;
  onOpenRevise: (rebaseOntoCurrent: boolean) => void;
  onWithdraw: (idempotencyKey: string) => Promise<boolean>;
}

const PROPOSAL_STATUS_LABELS: Record<ProposalStatus, string> = {
  pending: 'Pending review',
  changesRequested: 'Changes requested',
  rejected: 'Rejected',
  applied: 'Applied',
  withdrawn: 'Withdrawn',
};

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

export const SharedProposalReview: React.FC<SharedProposalReviewProps> = ({
  details,
  currentUid,
  busy,
  error,
  onRefresh,
  onBack,
  onReview,
  onOpenRevise,
  onWithdraw,
}) => {
  const [note, setNote] = useState('');
  const [compareTab, setCompareTab] = useState<'base-vs-proposed' | 'current-vs-proposed'>(
    details.proposal.isStale ? 'current-vs-proposed' : 'base-vs-proposed',
  );
  const reviewKeyRef = useRef<{ sig: string; key: string } | null>(null);
  const withdrawKeyRef = useRef<string>(makeRequestIdempotencyKey('wd'));

  const { proposal } = details;
  const isOwnProposal = !!currentUid && proposal.authorUid === currentUid;

  const submitReview = async (action: ProposalReviewAction) => {
    const cleanNote = note.trim() || null;
    const sig = `${proposal.proposalId}:${proposal.submissionCount}:${action}:${details.currentPageRevisionNumber}:${cleanNote ?? ''}`;
    if (!reviewKeyRef.current || reviewKeyRef.current.sig !== sig) {
      reviewKeyRef.current = { sig, key: makeRequestIdempotencyKey('rev') };
    }
    const ok = await onReview(action, cleanNote, reviewKeyRef.current.key, proposal.submissionCount);
    if (ok) {
      reviewKeyRef.current = null;
      setNote('');
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <button
          type="button"
          onClick={onBack}
          className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
        >
          <ArrowLeft className="w-3.5 h-3.5" />
          <span>Back to page</span>
        </button>

        <button
          type="button"
          disabled={busy}
          onClick={onRefresh}
          className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
        >
          <RefreshCw className={`w-3.5 h-3.5 ${busy ? 'animate-spin' : ''}`} />
          <span>Refresh review</span>
        </button>
      </div>

      {error && (
        <div className="space-y-2">
          <p
            role="alert"
            className="rounded-xl border border-sthira-border bg-sthira-surface-muted px-4 py-3 text-xs text-sthira-danger"
          >
            {error}
          </p>
          {error.toLowerCase().includes('replaced by submission') && (
            <button
              type="button"
              disabled={busy}
              onClick={onRefresh}
              className="sthira-button-secondary cursor-pointer"
            >
              Refresh to latest submission
            </button>
          )}
        </div>
      )}

      <section
        aria-label="Proposal overview"
        className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-3"
      >
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="space-y-0.5 min-w-0">
            <p className="text-[11px] font-semibold uppercase tracking-wider text-sthira-text-subtle">
              Team change review · Submission #{proposal.submissionCount}
            </p>
            <h2 className="text-base font-semibold text-sthira-text">
              {proposal.summary}
            </h2>
            <p className="text-xs text-sthira-text-muted">
              Author: <strong className="text-sthira-text">{proposal.authorDisplayName}</strong> · Base rev{' '}
              {proposal.baseRevisionNumber} · Current page rev {details.currentPageRevisionNumber}
            </p>
          </div>
          <span className="rounded-lg px-2.5 py-1 text-xs font-medium border border-sthira-border bg-sthira-canvas text-sthira-text">
            {PROPOSAL_STATUS_LABELS[proposal.status]}
          </span>
        </div>

        {proposal.isStale && (
          <div
            role="status"
            className="rounded-xl border border-sthira-border bg-sthira-surface-muted p-3.5 flex items-start gap-2.5 text-xs"
          >
            <AlertTriangle className="w-4 h-4 text-sthira-danger shrink-0 mt-0.5" />
            <div className="space-y-1">
              <p className="font-semibold text-sthira-text">
                Stale base revision (based on rev {proposal.baseRevisionNumber}; current page is rev {details.currentPageRevisionNumber})
              </p>
              <p className="text-sthira-text-muted">
                This proposal cannot be applied until the author inspects current rev {details.currentPageRevisionNumber}, rebases, and submits a fresh revision.
              </p>
            </div>
          </div>
        )}

        {details.pageArchived && (
          <p role="status" className="rounded-xl border border-sthira-border bg-sthira-surface-muted px-3.5 py-2.5 text-xs text-sthira-text">
            This page is currently archived. Restore the page before reviewing or resubmitting proposals.
          </p>
        )}

        {proposal.status === 'pending' && isOwnProposal && !details.canReview && (
          <p className="rounded-xl border border-sthira-border bg-sthira-canvas px-3.5 py-2.5 text-xs text-sthira-text-muted">
            Another Owner or Admin must review and apply your proposal.
          </p>
        )}
      </section>

      {/* Rich-text Comparison */}
      <section className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h3 className="text-sm font-semibold">Content comparison</h3>
          <div className="flex flex-wrap gap-1.5">
            <button
              type="button"
              onClick={() => setCompareTab('base-vs-proposed')}
              className={`px-3 py-1.5 rounded-lg text-xs cursor-pointer ${
                compareTab === 'base-vs-proposed'
                  ? 'bg-sthira-primary text-sthira-on-primary'
                  : 'bg-sthira-canvas border border-sthira-border text-sthira-text'
              }`}
            >
              Base (rev {proposal.baseRevisionNumber}) vs Proposed
            </button>
            <button
              type="button"
              onClick={() => setCompareTab('current-vs-proposed')}
              className={`px-3 py-1.5 rounded-lg text-xs cursor-pointer ${
                compareTab === 'current-vs-proposed'
                  ? 'bg-sthira-primary text-sthira-on-primary'
                  : 'bg-sthira-canvas border border-sthira-border text-sthira-text'
              }`}
            >
              Current (rev {details.currentPageRevisionNumber}) vs Proposed
            </button>
          </div>
        </div>

        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <div className="rounded-xl border border-sthira-border bg-sthira-canvas p-4 space-y-2 min-w-0">
            <p className="text-[11px] font-semibold uppercase tracking-wider text-sthira-text-subtle">
              {compareTab === 'current-vs-proposed'
                ? `Current published (rev ${details.currentPageRevisionNumber})`
                : `Submitted base (rev ${proposal.baseRevisionNumber})`}
            </p>
            <h4 className="text-sm font-semibold font-editorial">
              {compareTab === 'current-vs-proposed'
                ? details.currentPageTitle
                : details.baseTitle}
            </h4>
            <div className="pt-2 border-t border-sthira-border">
              <RichDocumentRenderer
                content={
                  compareTab === 'current-vs-proposed'
                    ? details.currentPageContentMd
                    : details.baseContentMd
                }
                documentTitle={
                  compareTab === 'current-vs-proposed'
                    ? details.currentPageTitle
                    : details.baseTitle
                }
              />
            </div>
          </div>

          <div className="rounded-xl border border-sthira-border bg-sthira-canvas p-4 space-y-2 min-w-0">
            <p className="text-[11px] font-semibold uppercase tracking-wider text-sthira-primary">
              Proposed change
            </p>
            <h4 className="text-sm font-semibold font-editorial">{details.proposedTitle}</h4>
            <div className="pt-2 border-t border-sthira-border">
              <RichDocumentRenderer
                content={details.proposedContentMd}
                documentTitle={details.proposedTitle}
              />
            </div>
          </div>
        </div>
      </section>

      {/* Reviewer Actions */}
      {details.canReview && (
        <section
          aria-label="Reviewer decision"
          className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-3"
        >
          <h3 className="text-sm font-semibold">Review decision</h3>
          <label className="block space-y-1">
            <span className="text-xs text-sthira-text-muted">Review note (optional)</span>
            <textarea
              aria-label="Review note"
              rows={2}
              disabled={busy}
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder="Explain requested changes or context for your decision…"
              className="w-full rounded-lg border border-sthira-border bg-sthira-canvas p-2.5 text-xs text-sthira-text focus:outline-none"
            />
          </label>
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              disabled={busy || proposal.isStale}
              onClick={() => void submitReview('apply')}
              className="sthira-button-primary inline-flex items-center gap-1.5 cursor-pointer"
            >
              <CheckCircle2 className="w-3.5 h-3.5" />
              <span>{busy ? 'Applying…' : 'Apply change'}</span>
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => void submitReview('requestChanges')}
              className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
            >
              <Edit3 className="w-3.5 h-3.5" />
              <span>Request changes</span>
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => void submitReview('reject')}
              className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
            >
              <XCircle className="w-3.5 h-3.5" />
              <span>Reject</span>
            </button>
          </div>
        </section>
      )}

      {/* Author Controls */}
      {details.canAuthorEdit && (
        <section
          aria-label="Author proposal actions"
          className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 flex flex-wrap items-center justify-between gap-3"
        >
          <div className="space-y-0.5">
            <h3 className="text-sm font-semibold">Your proposal</h3>
            <p className="text-xs text-sthira-text-muted">
              {proposal.isStale
                ? `Rebase onto current rev ${details.currentPageRevisionNumber} to continue.`
                : 'Revise and resubmit for fresh review, or withdraw without altering the published page.'}
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={() => onOpenRevise(proposal.isStale)}
              className="sthira-button-primary cursor-pointer"
            >
              {proposal.isStale
                ? `Rebase & revise (rev ${details.currentPageRevisionNumber})`
                : 'Revise & resubmit'}
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => void onWithdraw(withdrawKeyRef.current)}
              className="sthira-button-secondary cursor-pointer"
            >
              Withdraw proposal
            </button>
          </div>
        </section>
      )}

      {/* Decision & Submission History */}
      <section className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-3">
        <h3 className="text-sm font-semibold flex items-center gap-1.5">
          <GitCommit className="w-4 h-4 text-sthira-primary" />
          <span>Submission & decision history ({details.decisions.length})</span>
        </h3>
        <ul className="space-y-2" aria-label="Proposal history">
          {details.decisions.map((dec) => (
            <li
              key={dec.decisionId}
              className="rounded-xl border border-sthira-border bg-sthira-canvas p-3 space-y-1 text-xs"
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <span className="font-medium text-sthira-text">
                  #{dec.submissionNumber} · {dec.action.replace(/_/g, ' ')} · {dec.actorDisplayName}
                </span>
                <span className="text-sthira-text-muted">{formatDateTime(dec.createdAtMs)}</span>
              </div>
              <p className="text-sthira-text-muted">
                Base rev {dec.baseRevisionNumber}
                {dec.resultingRevisionNumber ? ` → Created rev ${dec.resultingRevisionNumber}` : ''}
              </p>
              {dec.note && <p className="text-sthira-text">Note: {dec.note}</p>}
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
};
