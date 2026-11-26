import React, { useEffect, useId, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { invoke } from '@tauri-apps/api/core';
import { Users, X } from 'lucide-react';
import { useDialogFocus } from '../hooks/useDialogFocus';
import { createWorkspace, type WorkspaceSummary } from '../api/workspaces';
import type { GoogleSession } from './AccountModal';
import { HelpTopicButton } from './HelpTopicButton';

interface CreateWorkspaceModalProps {
  isOpen: boolean;
  session: GoogleSession | null;
  onSessionChange: (session: GoogleSession) => void;
  onCreated: (workspace: WorkspaceSummary) => void;
  onClose: () => void;
  onOpenAccount: () => void;
}

function makeIdempotencyKey(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `ws_${Date.now()}_${Math.random().toString(36).slice(2, 10)}`;
}

export const CreateWorkspaceModal: React.FC<CreateWorkspaceModalProps> = ({
  isOpen,
  session,
  onSessionChange,
  onCreated,
  onClose,
  onOpenAccount,
}) => {
  const [name, setName] = useState('');
  const [pending, setPending] = useState(false);
  const [signingIn, setSigningIn] = useState(false);
  const [error, setError] = useState('');
  const idempotencyKeyRef = useRef<string>(makeIdempotencyKey());
  const panelRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const titleId = useId();

  useEffect(() => {
    if (!isOpen) return;
    setError('');
    idempotencyKeyRef.current = makeIdempotencyKey();
  }, [isOpen]);

  const safeClose = () => {
    if (pending || signingIn) return;
    setName('');
    setError('');
    onClose();
  };

  useDialogFocus(isOpen, panelRef, safeClose, { initialFocus: inputRef });

  if (!isOpen) return null;

  const isSignedOut = !session || session.status === 'signedOut';
  const isUnverified = session?.status === 'unverified';
  const isUnavailable = session?.status === 'unavailable';

  const handleSignIn = async () => {
    setSigningIn(true);
    setError('');
    try {
      const next = await invoke<GoogleSession>('cmd_google_sign_in');
      onSessionChange(next);
    } catch (err) {
      setError(typeof err === 'string' ? err : 'Google sign-in could not be completed.');
    } finally {
      setSigningIn(false);
    }
  };

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    const trimmed = name.trim();
    if (!trimmed || pending || signingIn) return;
    setPending(true);
    setError('');
    try {
      const res = await createWorkspace(trimmed, idempotencyKeyRef.current);
      if (res.outcome === 'ok') {
        setName('');
        idempotencyKeyRef.current = makeIdempotencyKey();
        onCreated(res.data.workspace);
        return;
      }
      if (res.outcome === 'sessionUpdated') {
        onSessionChange(res.session);
        setError(
          res.session.status === 'unverified'
            ? res.session.message
            : 'Sign in with Google to create a team workspace.',
        );
        return;
      }
      if (res.outcome === 'error') {
        setError(res.message);
        return;
      }
      if (res.outcome === 'accessLost') {
        setError(res.message);
      }
    } catch (err) {
      setError(typeof err === 'string' ? err : 'Could not create workspace. Try again.');
    } finally {
      setPending(false);
    }
  };

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
              <Users className="w-4 h-4" />
            </div>
            <div className="min-w-0">
              <h2 id={titleId} className="text-sm font-semibold truncate">
                Create workspace
              </h2>
              <p className="text-xs text-sthira-text-muted truncate">
                Starts empty · you join as Owner
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <HelpTopicButton article="workspaces" label="Help" />
            <button
              type="button"
              onClick={safeClose}
              aria-label="Close Create workspace"
              disabled={pending || signingIn}
              className="p-1.5 rounded-lg text-sthira-text-muted hover:text-sthira-text hover:bg-sthira-surface-muted cursor-pointer"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        <form onSubmit={(e) => void handleSubmit(e)} className="p-5 space-y-4 text-xs">
          <p className="text-sthira-text-muted leading-relaxed">
            New team workspaces start empty and do not upload your Personal vault.
          </p>

          <label className="block space-y-1.5">
            <span className="font-medium text-sthira-text">Workspace name</span>
            <input
              ref={inputRef}
              aria-label="Workspace name"
              type="text"
              required
              maxLength={80}
              disabled={pending || signingIn}
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="e.g. Research Team"
              className="w-full min-h-[42px] rounded-lg border border-sthira-border bg-sthira-canvas px-3 text-sm text-sthira-text focus:outline-none"
            />
          </label>

          {isUnavailable && (
            <p role="alert" className="rounded-lg border border-sthira-border bg-sthira-surface-muted px-3 py-2 text-sthira-text">
              Team workspaces require a configured Sthira session service. Your Personal workspace remains available offline.
            </p>
          )}

          {(isSignedOut || isUnverified) && (
            <div className="rounded-lg border border-sthira-border bg-sthira-surface-muted p-3 space-y-2">
              <p className="text-sthira-text">
                {isUnverified
                  ? 'Verify your saved Google session before creating a team workspace.'
                  : 'Sign in with Google to create a team workspace.'}
              </p>
              <div className="flex flex-wrap gap-2">
                {isSignedOut ? (
                  <button
                    type="button"
                    disabled={signingIn}
                    onClick={() => void handleSignIn()}
                    className="sthira-button-primary cursor-pointer"
                  >
                    {signingIn ? 'Waiting for Google…' : 'Continue with Google'}
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

          {error && (
            <p role="alert" className="rounded-lg border border-sthira-border bg-sthira-surface-muted px-3 py-2 text-sthira-danger">
              {error}
            </p>
          )}

          <div className="flex flex-wrap items-center justify-end gap-2 pt-2">
            <button
              type="button"
              onClick={safeClose}
              disabled={pending || signingIn}
              className="sthira-button-secondary cursor-pointer"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={pending || signingIn || !name.trim() || isUnavailable}
              className="sthira-button-primary cursor-pointer"
            >
              {pending ? 'Creating…' : 'Create workspace'}
            </button>
          </div>
        </form>
      </div>
    </div>,
    document.body,
  );
};
