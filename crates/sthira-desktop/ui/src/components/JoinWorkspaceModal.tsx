import React, { useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { invoke } from '@tauri-apps/api/core';
import { UserPlus, X } from 'lucide-react';
import { useDialogFocus } from '../hooks/useDialogFocus';
import {
  acceptInvitation,
  previewInvitation,
  WORKSPACE_ROLE_LABELS,
  type InvitationPreview,
  type WorkspaceSummary,
} from '../api/workspaces';
import type { GoogleSession } from './AccountModal';
import { HelpTopicButton } from './HelpTopicButton';

interface JoinWorkspaceModalProps {
  isOpen: boolean;
  session: GoogleSession | null;
  onSessionChange: (session: GoogleSession) => void;
  onJoined: (workspace: WorkspaceSummary) => void;
  onClose: () => void;
  onOpenAccount: () => void;
}

export const JoinWorkspaceModal: React.FC<JoinWorkspaceModalProps> = ({
  isOpen,
  session,
  onSessionChange,
  onJoined,
  onClose,
  onOpenAccount,
}) => {
  const [inviteCode, setInviteCode] = useState('');
  const [preview, setPreview] = useState<InvitationPreview | null>(null);
  const [pending, setPending] = useState<'preview' | 'accept' | 'sign-in' | null>(null);
  const [error, setError] = useState('');
  const [errorCode, setErrorCode] = useState('');
  const panelRef = useRef<HTMLDivElement>(null);
  const codeInputRef = useRef<HTMLInputElement>(null);
  const titleId = useId();

  useEffect(() => {
    if (!isOpen) return;
    setPreview(null);
    setError('');
    setErrorCode('');
  }, [isOpen]);

  const safeClose = () => {
    if (pending) return;
    setInviteCode('');
    setPreview(null);
    setError('');
    setErrorCode('');
    onClose();
  };

  useDialogFocus(isOpen, panelRef, safeClose, { initialFocus: codeInputRef });

  if (!isOpen) return null;

  const isSignedOut = !session || session.status === 'signedOut';
  const isUnverified = session?.status === 'unverified';
  const isUnavailable = session?.status === 'unavailable';

  const lookupInvite = async (event?: React.FormEvent) => {
    event?.preventDefault();
    const trimmed = inviteCode.trim();
    if (!trimmed || pending) return;
    setPending('preview');
    setError('');
    setErrorCode('');
    try {
      const res = await previewInvitation(trimmed);
      if (res.outcome === 'ok') {
        setPreview(res.data);
        return;
      }
      if (res.outcome === 'sessionUpdated') {
        onSessionChange(res.session);
        setError(
          res.session.status === 'unverified'
            ? res.session.message
            : 'Sign in with Google to inspect this invitation.',
        );
        return;
      }
      if (res.outcome === 'error') {
        setErrorCode(res.code);
        setError(res.message);
        return;
      }
      setError(res.message);
    } catch (err) {
      setError(typeof err === 'string' ? err : 'Could not inspect this invitation. Try again.');
    } finally {
      setPending(null);
    }
  };

  const handleSignInAndCheck = async () => {
    if (pending) return;
    setPending('sign-in');
    setError('');
    setErrorCode('');
    try {
      const next = await invoke<GoogleSession>('cmd_google_sign_in');
      onSessionChange(next);
    } catch (err) {
      setError(typeof err === 'string' ? err : 'Google sign-in could not be completed.');
    } finally {
      setPending(null);
    }
  };

  const handleAccept = async () => {
    const trimmed = inviteCode.trim();
    if (!trimmed || !preview || pending) return;
    setPending('accept');
    setError('');
    setErrorCode('');
    try {
      const res = await acceptInvitation(trimmed);
      if (res.outcome === 'ok') {
        setInviteCode('');
        setPreview(null);
        onJoined(res.data.workspace);
        return;
      }
      if (res.outcome === 'sessionUpdated') {
        onSessionChange(res.session);
        setPreview(null);
        setError(
          res.session.status === 'unverified'
            ? res.session.message
            : 'Sign in with Google to accept this invitation.',
        );
        return;
      }
      if (res.outcome === 'error') {
        setErrorCode(res.code);
        setError(res.message);
        if (
          res.code === 'invitation_expired' ||
          res.code === 'invitation_revoked' ||
          res.code === 'invitation_already_used' ||
          res.code === 'invitation_wrong_account'
        ) {
          setPreview(null);
        }
        return;
      }
      setError(res.message);
    } catch (err) {
      setError(typeof err === 'string' ? err : 'Could not accept this invitation. Try again.');
    } finally {
      setPending(null);
    }
  };

  const expiryText = preview
    ? new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(
        new Date(preview.expiresAtMs),
      )
    : '';

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
        className="w-full max-w-md rounded-2xl border border-sthira-border bg-sthira-surface text-sthira-text shadow-2xl overflow-hidden"
      >
        <div className="px-5 py-4 border-b border-sthira-border bg-sthira-canvas flex items-center justify-between gap-2">
          <div className="flex items-center gap-2.5 min-w-0">
            <div className="w-8 h-8 rounded-xl bg-sthira-surface-muted text-sthira-primary flex items-center justify-center shrink-0">
              <UserPlus className="w-4 h-4" />
            </div>
            <div className="min-w-0">
              <h2 id={titleId} className="text-sm font-semibold truncate">
                Join workspace
              </h2>
              <p className="text-xs text-sthira-text-muted truncate">
                Single-use code verified against your Google account
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <HelpTopicButton article="workspaces" label="Help" />
            <button
              type="button"
              onClick={safeClose}
              aria-label="Close Join workspace"
              disabled={!!pending}
              className="p-1.5 rounded-lg text-sthira-text-muted hover:text-sthira-text hover:bg-sthira-surface-muted cursor-pointer"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        <div className="p-5 space-y-4 text-xs">
          {!preview ? (
            <form onSubmit={(e) => void lookupInvite(e)} className="space-y-4">
              <label className="block space-y-1.5">
                <span className="font-medium text-sthira-text">Invitation code</span>
                <input
                  ref={codeInputRef}
                  aria-label="Invitation code"
                  type="text"
                  required
                  disabled={!!pending}
                  value={inviteCode}
                  onChange={(event) => {
                    setInviteCode(event.target.value);
                    setError('');
                    setErrorCode('');
                  }}
                  placeholder="Paste single-use invitation code"
                  className="w-full min-h-[42px] rounded-lg border border-sthira-border bg-sthira-canvas px-3 font-mono text-xs text-sthira-text focus:outline-none"
                />
              </label>

              {(isSignedOut || isUnverified) && (
                <div className="rounded-lg border border-sthira-border bg-sthira-surface-muted p-3 space-y-2">
                  <p className="text-sthira-text">
                    {isUnverified
                      ? 'Verify your saved session before inspecting this invitation.'
                      : 'Sign in with the invited Google account before checking this invitation.'}
                  </p>
                  <div className="flex flex-wrap gap-2">
                    {isSignedOut ? (
                      <button
                        type="button"
                        disabled={!!pending}
                        onClick={() => void handleSignInAndCheck()}
                        className="sthira-button-primary cursor-pointer"
                      >
                        {pending === 'sign-in' ? 'Waiting for Google…' : 'Continue with Google'}
                      </button>
                    ) : (
                      <button
                        type="button"
                        onClick={onOpenAccount}
                        className="sthira-button-secondary cursor-pointer"
                      >
                        Open Account
                      </button>
                    )}
                  </div>
                </div>
              )}

              {isUnavailable && (
                <p role="alert" className="rounded-lg border border-sthira-border bg-sthira-surface-muted px-3 py-2 text-sthira-text">
                  Team workspaces require a configured Sthira session service.
                </p>
              )}

              {error && (
                <div role="alert" className="rounded-lg border border-sthira-border bg-sthira-surface-muted p-3 space-y-2">
                  <p className="text-sthira-danger">{error}</p>
                  {errorCode === 'invitation_wrong_account' && (
                    <div className="flex flex-wrap gap-2">
                      <button
                        type="button"
                        disabled={!!pending}
                        onClick={() => void handleSignInAndCheck()}
                        className="sthira-button-secondary cursor-pointer"
                      >
                        Switch Google account
                      </button>
                      <button
                        type="button"
                        onClick={onOpenAccount}
                        className="sthira-button-secondary cursor-pointer"
                      >
                        Open Account
                      </button>
                    </div>
                  )}
                </div>
              )}

              <div className="flex flex-wrap justify-end gap-2 pt-1">
                <button
                  type="button"
                  onClick={safeClose}
                  disabled={!!pending}
                  className="sthira-button-secondary cursor-pointer"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={!!pending || !inviteCode.trim() || isSignedOut || isUnavailable}
                  className="sthira-button-primary cursor-pointer"
                >
                  {pending === 'preview' ? 'Checking…' : 'Continue'}
                </button>
              </div>
            </form>
          ) : (
            <div className="space-y-4">
              <div className="rounded-xl border border-sthira-border bg-sthira-canvas p-4 space-y-2">
                <p className="text-[11px] uppercase tracking-wider text-sthira-text-subtle font-semibold">
                  Workspace invitation
                </p>
                <p className="text-base font-semibold text-sthira-text break-words">
                  {preview.workspaceName}
                </p>
                <p className="text-sthira-text">
                  Offered role: <strong>{WORKSPACE_ROLE_LABELS[preview.offeredRole]}</strong>
                </p>
                <p className="text-sthira-text-muted">Expires {expiryText}</p>
                {preview.alreadyMember && preview.existingRole && (
                  <p role="status" className="rounded-lg border border-sthira-border bg-sthira-surface px-3 py-2 text-sthira-text">
                    You are already a member with role{' '}
                    <strong>{WORKSPACE_ROLE_LABELS[preview.existingRole]}</strong>. Accepting will keep your existing role and open this workspace.
                  </p>
                )}
              </div>

              {error && (
                <p role="alert" className="rounded-lg border border-sthira-border bg-sthira-surface-muted px-3 py-2 text-sthira-danger">
                  {error}
                </p>
              )}

              <div className="flex flex-wrap justify-end gap-2">
                <button
                  type="button"
                  onClick={() => {
                    setPreview(null);
                    setError('');
                  }}
                  disabled={!!pending}
                  className="sthira-button-secondary cursor-pointer"
                >
                  Back
                </button>
                <button
                  type="button"
                  onClick={() => void handleAccept()}
                  disabled={!!pending}
                  className="sthira-button-primary cursor-pointer"
                >
                  {pending === 'accept' ? 'Joining…' : 'Accept invitation'}
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>,
    document.body,
  );
};
