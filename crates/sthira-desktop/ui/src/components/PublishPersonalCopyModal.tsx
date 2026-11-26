import React, { useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { AlertTriangle, ArrowLeft, Eye, FileText, ShieldCheck, X } from 'lucide-react';
import { useDialogFocus } from '../hooks/useDialogFocus';
import { getArtifactContext, getWorkspaceIndex } from '../api/ipc';
import { sanitizePersonalPageForShare, type SanitizedPersonalCopyPreview } from '../api/shareSanitizer';
import {
  createSharedPage,
  makeRequestIdempotencyKey,
  type SharedPageDetails,
  type WorkspaceListResponse,
} from '../api/workspaces';
import type { GoogleSession } from './AccountModal';
import type { WorkspaceIndexItem } from '../types/sthira';
import { HelpTopicButton } from './HelpTopicButton';
import { RichDocumentRenderer } from './RichDocumentRenderer';

interface PublishPersonalCopyModalProps {
  isOpen: boolean;
  workspaceId: string;
  workspaceName: string;
  authorDisplayName?: string;
  onPublished: (page: SharedPageDetails) => void;
  onAccessLost: (message: string, workspaces?: WorkspaceListResponse | null) => void;
  onSessionChange: (session: GoogleSession) => void;
  onClose: () => void;
}

export const PublishPersonalCopyModal: React.FC<PublishPersonalCopyModalProps> = ({
  isOpen,
  workspaceId,
  workspaceName,
  authorDisplayName,
  onPublished,
  onAccessLost,
  onSessionChange,
  onClose,
}) => {
  const [personalPages, setPersonalPages] = useState<WorkspaceIndexItem[]>([]);
  const [loadingList, setLoadingList] = useState(false);
  const [loadingPreview, setLoadingPreview] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [preview, setPreview] = useState<SanitizedPersonalCopyPreview | null>(null);
  const [publishing, setPublishing] = useState(false);
  const [error, setError] = useState('');

  const panelRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const [filterText, setFilterText] = useState('');
  const titleId = useId();
  const idempotencyKeyRef = useRef<string>(makeRequestIdempotencyKey('copy'));
  const reqSeqRef = useRef(0);

  const safeClose = () => {
    if (publishing) return;
    setSelectedId(null);
    setPreview(null);
    setError('');
    setFilterText('');
    onClose();
  };

  useDialogFocus(isOpen, panelRef, safeClose, { initialFocus: searchInputRef });

  useEffect(() => {
    if (!isOpen) return;
    const seq = ++reqSeqRef.current;
    setError('');
    setSelectedId(null);
    setPreview(null);
    setLoadingList(true);
    void getWorkspaceIndex('notes')
      .then((res) => {
        if (seq !== reqSeqRef.current) return;
        setPersonalPages(res.items || []);
      })
      .catch(() => {
        if (seq !== reqSeqRef.current) return;
        setError('Could not load your personal pages.');
      })
      .finally(() => {
        if (seq === reqSeqRef.current) setLoadingList(false);
      });
  }, [isOpen]);

  if (!isOpen) return null;

  const openPreview = async (artifactId: string) => {
    const seq = ++reqSeqRef.current;
    setLoadingPreview(true);
    setError('');
    setSelectedId(artifactId);
    try {
      // Strictly local vault read; zero network or service calls occur during preview.
      const ctx = await getArtifactContext(artifactId);
      if (seq !== reqSeqRef.current) return;
      const sanitized = sanitizePersonalPageForShare(ctx, authorDisplayName);
      setPreview(sanitized);
      idempotencyKeyRef.current = makeRequestIdempotencyKey('copy');
    } catch {
      if (seq !== reqSeqRef.current) return;
      setError('Could not inspect that personal page.');
      setSelectedId(null);
    } finally {
      if (seq === reqSeqRef.current) setLoadingPreview(false);
    }
  };

  const handleConfirmPublish = async () => {
    if (!selectedId || !preview || preview.exceedsLimit || publishing) return;
    setPublishing(true);
    setError('');
    try {
      const latestCtx = await getArtifactContext(selectedId);
      if (
        latestCtx.artifact.content_hash !== preview.sourceContentHash ||
        latestCtx.artifact.version !== preview.sourceVersion
      ) {
        setPreview(sanitizePersonalPageForShare(latestCtx, authorDisplayName));
        idempotencyKeyRef.current = makeRequestIdempotencyKey('copy');
        setError(
          'Your personal original changed after preview. Review the updated copy and omissions before publishing.',
        );
        return;
      }

      const res = await createSharedPage(workspaceId, {
        title: preview.title,
        contentMd: preview.sanitizedMarkdown,
        entryMode: 'copyPersonal',
        provenance: preview.provenance,
        omittedSummary: preview.omissions,
        idempotencyKey: idempotencyKeyRef.current,
      });
      if (res.outcome === 'ok') {
        onPublished(res.data.page);
        safeClose();
        return;
      }
      if (res.outcome === 'accessLost') {
        safeClose();
        onAccessLost(res.message, res.workspaces);
        return;
      }
      if (res.outcome === 'sessionUpdated') {
        onSessionChange(res.session);
        setError(
          res.session.status === 'unverified'
            ? res.session.message
            : 'Sign in with Google before publishing to this team workspace.',
        );
        return;
      }
      setError(res.message);
    } catch (err) {
      setError(typeof err === 'string' ? err : 'Could not publish copy. Your personal page is untouched.');
    } finally {
      setPublishing(false);
    }
  };

  const filteredPages = personalPages.filter((item) => {
    const q = filterText.trim().toLowerCase();
    if (!q) return true;
    return (
      item.title.toLowerCase().includes(q) ||
      (item.description || '').toLowerCase().includes(q)
    );
  });

  return createPortal(
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/40 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) safeClose();
      }}
    >
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="w-full max-w-3xl max-h-[88vh] flex flex-col rounded-2xl border border-sthira-border bg-sthira-surface text-sthira-text shadow-2xl overflow-hidden"
      >
        <div className="px-5 py-4 border-b border-sthira-border bg-sthira-canvas flex items-center justify-between gap-3 shrink-0">
          <div className="flex items-center gap-2.5 min-w-0">
            <div className="w-8 h-8 rounded-xl bg-sthira-surface-muted text-sthira-primary flex items-center justify-center shrink-0">
              <FileText className="w-4 h-4" />
            </div>
            <div className="min-w-0">
              <h2 id={titleId} className="text-sm font-semibold truncate">
                Publish a copy to {workspaceName}
              </h2>
              <p className="text-xs text-sthira-text-muted truncate">
                Your personal original stays untouched in your local vault
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <HelpTopicButton article="workspaces" label="Help" />
            <button
              type="button"
              onClick={safeClose}
              aria-label="Close Publish a copy"
              disabled={publishing}
              className="p-1.5 rounded-lg text-sthira-text-muted hover:text-sthira-text hover:bg-sthira-surface-muted cursor-pointer"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-5 space-y-4 text-xs">
          {error && (
            <p
              role="alert"
              className="rounded-xl border border-sthira-border bg-sthira-surface-muted px-3.5 py-2.5 text-sthira-danger"
            >
              {error}
            </p>
          )}

          {!preview ? (
            <div className="space-y-3">
              <label className="block space-y-1">
                <span className="font-medium text-sthira-text">Select one personal page</span>
                <input
                  ref={searchInputRef}
                  aria-label="Filter personal pages"
                  type="text"
                  value={filterText}
                  onChange={(e) => setFilterText(e.target.value)}
                  placeholder="Search personal pages by title…"
                  className="w-full min-h-[40px] rounded-lg border border-sthira-border bg-sthira-canvas px-3 text-sm text-sthira-text focus:outline-none"
                />
              </label>

              {loadingList ? (
                <p role="status" className="text-sthira-text-muted py-4">
                  Loading your personal pages…
                </p>
              ) : filteredPages.length === 0 ? (
                <p className="text-sthira-text-muted py-4">
                  No personal pages matched. Create a page in Personal or clear the search filter.
                </p>
              ) : (
                <ul className="space-y-2" aria-label="Personal pages to share">
                  {filteredPages.map((page) => (
                    <li
                      key={page.id}
                      className="rounded-xl border border-sthira-border bg-sthira-canvas p-3 flex flex-wrap items-center justify-between gap-2"
                    >
                      <div className="min-w-0 flex-1 space-y-0.5">
                        <p className="font-medium text-sm text-sthira-text truncate">
                          {page.title || 'Untitled page'}
                        </p>
                        {page.description && (
                          <p className="text-sthira-text-muted truncate">{page.description}</p>
                        )}
                      </div>
                      <button
                        type="button"
                        disabled={loadingPreview}
                        aria-label={`Preview copy of ${page.title || 'Untitled page'}`}
                        onClick={() => void openPreview(page.id)}
                        className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer shrink-0"
                      >
                        <Eye className="w-3.5 h-3.5" />
                        <span>{loadingPreview && selectedId === page.id ? 'Preparing…' : 'Review copy'}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          ) : (
            <div className="space-y-4">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <button
                  type="button"
                  disabled={publishing}
                  onClick={() => {
                    setPreview(null);
                    setSelectedId(null);
                    setError('');
                  }}
                  className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
                >
                  <ArrowLeft className="w-3.5 h-3.5" />
                  <span>Back to personal pages</span>
                </button>
                <span className="text-sthira-text-muted">
                  Size: {preview.byteLength.toLocaleString()} / {preview.maxBytes.toLocaleString()} bytes
                </span>
              </div>

              <div className="rounded-xl border border-sthira-border bg-sthira-canvas p-3.5 space-y-2">
                <div className="flex items-start gap-2">
                  <ShieldCheck className="w-4 h-4 text-sthira-primary shrink-0 mt-0.5" />
                  <div className="space-y-1">
                    <p className="font-medium text-sthira-text">
                      Destination: {workspaceName}
                    </p>
                    <p className="text-sthira-text-muted">
                      All active members of {workspaceName} (Owners, Admins, Editors, and Viewers) will be able to read this copied page. Your Personal original remains unchanged locally.
                    </p>
                    <p className="text-sthira-text-muted">
                      Shareable attribution: {preview.provenance.originalAuthorLabel || 'Original author'}
                      {preview.provenance.sourceNote ? ` · Source: ${preview.provenance.sourceNote}` : ''}
                    </p>
                  </div>
                </div>
              </div>

              <div
                role="region"
                aria-label="Omissions and redactions"
                className="rounded-xl border border-sthira-border bg-sthira-surface-muted p-3.5 space-y-1.5"
              >
                <p className="font-medium text-sthira-text flex items-center gap-1.5">
                  <AlertTriangle className="w-3.5 h-3.5 text-sthira-primary shrink-0" />
                  <span>Omitted or converted before sharing</span>
                </p>
                {preview.omissions.length > 0 ? (
                  <ul className="list-disc pl-5 space-y-0.5 text-sthira-text-muted">
                    {preview.omissions.map((item, idx) => (
                      <li key={idx}>{item}</li>
                    ))}
                  </ul>
                ) : (
                  <p className="text-sthira-text-muted">
                    No private vault references or local attachments were found in this page.
                  </p>
                )}
              </div>

              {preview.exceedsLimit && (
                <p role="alert" className="rounded-xl border border-sthira-border bg-sthira-surface-muted px-3.5 py-2.5 text-sthira-danger">
                  This page exceeds the {preview.maxBytes / 1024} KiB shared-page limit. Go back and shorten your personal page or publish an excerpt.
                </p>
              )}

              <div className="rounded-xl border border-sthira-border bg-sthira-canvas p-4 space-y-2">
                <p className="text-[11px] font-semibold uppercase tracking-wider text-sthira-text-subtle">
                  Reviewed copy preview
                </p>
                <h3 className="text-base font-semibold font-editorial text-sthira-text">
                  {preview.title}
                </h3>
                <div className="pt-1 border-t border-sthira-border">
                  <RichDocumentRenderer
                    content={preview.sanitizedMarkdown}
                    documentTitle={preview.title}
                  />
                </div>
              </div>
            </div>
          )}
        </div>

        <div className="px-5 py-3.5 border-t border-sthira-border bg-sthira-canvas flex flex-wrap items-center justify-end gap-2 shrink-0">
          <button
            type="button"
            disabled={publishing}
            onClick={safeClose}
            className="sthira-button-secondary cursor-pointer"
          >
            Cancel
          </button>
          {preview && (
            <button
              type="button"
              disabled={publishing || preview.exceedsLimit}
              onClick={() => void handleConfirmPublish()}
              className="sthira-button-primary cursor-pointer"
            >
              {publishing ? 'Publishing…' : `Publish copy to ${workspaceName}`}
            </button>
          )}
        </div>
      </div>
    </div>,
    document.body,
  );
};
