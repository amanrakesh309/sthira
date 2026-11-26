import React, { useCallback, useEffect, useId, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { isTauri } from '../api/ipc';
import { User, X, Shield, Lock, HardDrive, Laptop, RefreshCw } from 'lucide-react';

interface AccountModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSessionChange?: (session: GoogleSession) => void;
}

export interface DeviceInfo {
  sessionId: string;
  deviceId: string;
  deviceLabel: string;
  createdAtMs: number;
  lastActiveAtMs: number;
  expiresAtMs: number;
  isCurrent: boolean;
}

export type SignOutReason = 'revoked' | 'expired' | 'invalid' | 'migrationRequired';

export type RevokeDeviceOutcome =
  | { outcome: 'revoked'; alreadyRevoked: boolean; session: GoogleSession }
  | {
      outcome: 'staleReplaced';
      message: string;
      replacementSessionId?: string | null;
      session: GoogleSession;
    }
  | { outcome: 'missing'; message: string; session: GoogleSession }
  | { outcome: 'callerInvalid'; session: GoogleSession }
  | { outcome: 'unverified'; session: GoogleSession };

export type GoogleSession =
  | { status: 'unavailable' }
  | { status: 'signedOut'; reason?: SignOutReason }
  | { status: 'unverified'; message: string }
  | {
      status: 'signedIn';
      uid?: string;
      email: string;
      currentSessionId: string;
      currentDeviceLabel: string;
      devices: DeviceInfo[];
    };

function formatDeviceTime(epochMs: number): string {
  if (!Number.isFinite(epochMs) || epochMs <= 0) return 'Recently';
  try {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(new Date(epochMs));
  } catch {
    return new Date(epochMs).toISOString();
  }
}

function signOutReasonCopy(reason?: SignOutReason): string | null {
  switch (reason) {
    case 'revoked':
      return 'This device was signed out.';
    case 'expired':
      return 'Your saved session expired. Sign in again with Google.';
    case 'invalid':
      return 'This device session is invalid. Sign in again with Google.';
    case 'migrationRequired':
      return 'Sign in again with Google to connect this device to Sthira sessions.';
    default:
      return null;
  }
}

export const AccountModal: React.FC<AccountModalProps> = ({ isOpen, onClose, onSessionChange }) => {
  const titleId = useId();
  const [session, setSession] = useState<GoogleSession | null>(null);
  const [pending, setPending] = useState<'sign-in' | 'sign-out' | 'refresh' | 'revoke' | null>(null);
  const [error, setError] = useState('');
  const [confirmTarget, setConfirmTarget] = useState<DeviceInfo | null>(null);
  const [revocationNotice, setRevocationNotice] = useState('');
  const request = useRef(0);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  const sessionChangeRef = useRef(onSessionChange);
  sessionChangeRef.current = onSessionChange;

  const commitSession = useCallback((next: GoogleSession) => {
    setSession(next);
    sessionChangeRef.current?.(next);
  }, []);

  const loadSession = useCallback(async (keepExisting = false) => {
    const run = ++request.current;
    if (!keepExisting) setSession(null);
    setError('');
    setRevocationNotice('');
    try {
      const status = await invoke<GoogleSession>('cmd_get_google_session');
      if (request.current === run) {
        commitSession(status);
        setConfirmTarget(null);
      }
    } catch (reason) {
      if (request.current === run) {
        setError(typeof reason === 'string' ? reason : 'Could not check your saved session.');
      }
    }
  }, [commitSession]);

  useEffect(() => {
    if (!isOpen || !isTauri) return;
    setConfirmTarget(null);
    void loadSession();
    return () => { request.current++; };
  }, [isOpen, loadSession]);

  // Retry saved-session verification automatically when connectivity returns or the window regains focus.
  useEffect(() => {
    if (!isOpen || !isTauri || session?.status !== 'unverified') return;
    const retryOnReconnect = () => {
      if (pending) return;
      void loadSession(true);
    };
    window.addEventListener('online', retryOnReconnect);
    window.addEventListener('focus', retryOnReconnect);
    return () => {
      window.removeEventListener('online', retryOnReconnect);
      window.removeEventListener('focus', retryOnReconnect);
    };
  }, [isOpen, loadSession, pending, session?.status]);

  useEffect(() => {
    if (!isOpen) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      event.stopPropagation();
      if (confirmTarget) {
        setConfirmTarget(null);
        return;
      }
      request.current++;
      if (pending === 'sign-in') void invoke('cmd_cancel_google_sign_in').catch(() => {});
      setPending(null);
      closeRef.current();
    };
    document.addEventListener('keydown', onKeyDown, true);
    return () => document.removeEventListener('keydown', onKeyDown, true);
  }, [confirmTarget, isOpen, pending]);

  const startSignIn = async () => {
    const run = ++request.current;
    setPending('sign-in');
    setError('');
    setRevocationNotice('');
    setConfirmTarget(null);
    try {
      const status = await invoke<GoogleSession>('cmd_google_sign_in');
      if (request.current === run) commitSession(status);
    } catch (reason) {
      if (request.current === run) setError(typeof reason === 'string' ? reason : 'Google sign-in failed.');
    } finally {
      if (request.current === run) setPending(null);
    }
  };

  const cancelSignIn = () => {
    void invoke('cmd_cancel_google_sign_in').catch(() => setError('Could not cancel sign-in.'));
  };

  const signOut = async () => {
    const run = ++request.current;
    setPending('sign-out');
    setError('');
    setRevocationNotice('');
    setConfirmTarget(null);
    try {
      await invoke('cmd_google_sign_out');
      if (request.current === run) commitSession({ status: 'signedOut' });
    } catch (reason) {
      if (request.current === run) setError(typeof reason === 'string' ? reason : 'Could not sign out.');
    } finally {
      if (request.current === run) setPending(null);
    }
  };

  const refreshDevices = async () => {
    const run = ++request.current;
    setPending('refresh');
    setError('');
    setRevocationNotice('');
    try {
      const status = await invoke<GoogleSession>('cmd_list_devices');
      if (request.current === run) {
        commitSession(status);
        setConfirmTarget(null);
      }
    } catch (reason) {
      if (request.current === run) {
        setError(typeof reason === 'string' ? reason : 'Could not refresh active devices.');
      }
    } finally {
      if (request.current === run) setPending(null);
    }
  };

  const confirmRevokeDevice = async () => {
    if (!confirmTarget) return;
    const targetLabel = confirmTarget.deviceLabel;
    const run = ++request.current;
    setPending('revoke');
    setError('');
    setRevocationNotice('');
    try {
      const result = await invoke<RevokeDeviceOutcome>('cmd_revoke_device', {
        sessionId: confirmTarget.sessionId,
      });
      if (request.current !== run) return;
      commitSession(result.session);
      setConfirmTarget(null);
      if (result.outcome === 'revoked') {
        setRevocationNotice(`Signed out “${targetLabel}”.`);
      } else if (result.outcome === 'staleReplaced') {
        setError(result.message);
      } else if (result.outcome === 'missing') {
        setRevocationNotice(result.message);
      }
    } catch (reason) {
      if (request.current === run) {
        setError(typeof reason === 'string' ? reason : 'Could not sign out that device.');
      }
    } finally {
      if (request.current === run) setPending(null);
    }
  };

  const close = () => {
    request.current++;
    if (pending === 'sign-in') void invoke('cmd_cancel_google_sign_in').catch(() => {});
    setPending(null);
    setConfirmTarget(null);
    onClose();
  };

  if (!isOpen) return null;

  const reasonBanner = session?.status === 'signedOut' ? signOutReasonCopy(session.reason) : null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40 backdrop-blur-sm animate-in fade-in duration-150">
      <div role="dialog" aria-modal="true" aria-labelledby={titleId} className="w-full max-w-lg max-h-full bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl shadow-2xl overflow-auto flex flex-col">
        {/* Header */}
        <div className="px-6 py-4 border-b border-[#ECE9E0] dark:border-[#2C2E27] flex items-center justify-between bg-[#FAF8F3] dark:bg-[#161714]">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-xl bg-[#EFECE4] dark:bg-[#252820] text-[#282C25] dark:text-[#EDEAE2] flex items-center justify-center font-bold text-sm">
              <User className="w-4 h-4" />
            </div>
            <div>
              <h3 id={titleId} className="text-base font-bold text-[#1E201C] dark:text-[#EDEAE2]">Local Account &amp; Integrity</h3>
              <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93]">Authorship provenance &amp; security guarantees</p>
            </div>
          </div>
          <button
            type="button"
            onClick={close}
            aria-label="Close"
            className="p-1.5 rounded-lg text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] hover:bg-[#EFECE4] dark:hover:bg-[#252820] transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-4 text-xs">
          {/* Identity Card */}
          <div className="bg-[#FAF8F3] dark:bg-[#161714] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl p-4 flex items-center gap-3.5">
            <div className="w-10 h-10 rounded-full bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] flex items-center justify-center font-bold text-base">
              A
            </div>
            <div>
              <h4 className="font-bold text-sm text-[#1E201C] dark:text-[#EDEAE2]">Local Architect</h4>
              <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93]">Full sovereignty · Local Vault User</p>
            </div>
          </div>

          {isTauri && (
            <section className="rounded-xl border border-sthira-border p-4 text-sthira-text space-y-3">
              <h4 className="font-semibold">Google account</h4>

              {!session && !error && !pending && (
                <p role="status" className="text-sthira-text-muted">Checking saved session…</p>
              )}

              {session?.status === 'unavailable' && (
                <p className="text-sthira-text-muted">
                  Configure a Firebase test project and Sthira session service to try sign-in on Linux.
                </p>
              )}

              {reasonBanner && (
                <p
                  role={
                    session?.status === 'signedOut' &&
                    (session.reason === 'revoked' ||
                      session.reason === 'expired' ||
                      session.reason === 'invalid')
                      ? 'alert'
                      : 'status'
                  }
                  className="rounded-lg border border-sthira-border bg-sthira-surface-muted px-3 py-2 text-sthira-text"
                >
                  {reasonBanner}
                </p>
              )}

              {session?.status === 'signedOut' && (
                <div className="flex flex-wrap items-center gap-3">
                  <button
                    type="button"
                    className="sthira-button-primary"
                    disabled={!!pending}
                    onClick={() => void startSignIn()}
                  >
                    Continue with Google
                  </button>
                  {pending === 'sign-in' && (
                    <button
                      type="button"
                      className="sthira-button-secondary"
                      onClick={cancelSignIn}
                    >
                      Cancel sign-in
                    </button>
                  )}
                </div>
              )}

              {session?.status === 'unverified' && (
                <div className="space-y-3">
                  <p role="alert" className="rounded-lg border border-sthira-border bg-sthira-surface-muted px-3 py-2 text-sthira-text">
                    {session.message}
                  </p>
                  <div className="flex flex-wrap items-center gap-3">
                    <button
                      type="button"
                      className="sthira-button-primary"
                      disabled={!!pending}
                      onClick={() => void loadSession()}
                    >
                      Retry verification
                    </button>
                    <button
                      type="button"
                      className="sthira-button-secondary"
                      disabled={!!pending}
                      onClick={() => void signOut()}
                    >
                      {pending === 'sign-out' ? 'Signing out…' : 'Sign out'}
                    </button>
                  </div>
                </div>
              )}

              {session?.status === 'signedIn' && (
                <div className="space-y-3">
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <p role="status" className="break-words">
                      Signed in as {session.email}
                    </p>
                    <button
                      type="button"
                      className="sthira-button-secondary"
                      disabled={!!pending}
                      onClick={() => void signOut()}
                    >
                      {pending === 'sign-out' ? 'Signing out…' : 'Sign out'}
                    </button>
                  </div>

                  {/* Account -> Devices */}
                  <div className="pt-3 border-t border-sthira-border space-y-2.5" aria-label="Devices">
                    <div className="flex items-center justify-between gap-2">
                      <h5 className="font-semibold flex items-center gap-1.5 text-sthira-text">
                        <Laptop className="w-3.5 h-3.5" />
                        Devices
                      </h5>
                      <button
                        type="button"
                        className="sthira-button-secondary inline-flex items-center gap-1"
                        disabled={!!pending}
                        onClick={() => void refreshDevices()}
                      >
                        <RefreshCw className="w-3 h-3" />
                        {pending === 'refresh' ? 'Refreshing…' : 'Refresh'}
                      </button>
                    </div>

                    {revocationNotice && (
                      <p role="status" className="text-sthira-text-muted">
                        {revocationNotice}
                      </p>
                    )}

                    <ul className="space-y-2" aria-label="Signed-in devices">
                      {session.devices.map((device) => (
                        <li
                          key={device.sessionId}
                          className="rounded-lg border border-sthira-border bg-sthira-surface-muted p-3 flex items-start justify-between gap-3"
                        >
                          <div className="min-w-0 space-y-0.5">
                            <div className="flex flex-wrap items-center gap-2">
                              <span className="font-medium text-sthira-text break-words">
                                {device.deviceLabel}
                              </span>
                              {device.isCurrent && (
                                <span className="rounded px-1.5 py-0.5 text-[11px] font-medium border border-sthira-border text-sthira-text">
                                  Current device
                                </span>
                              )}
                            </div>
                            <p className="text-sthira-text-muted">
                              Last active {formatDeviceTime(device.lastActiveAtMs)}
                            </p>
                            <p className="text-sthira-text-muted">
                              Signed in {formatDeviceTime(device.createdAtMs)}
                            </p>
                          </div>

                          {!device.isCurrent && (
                            <button
                              type="button"
                              className="sthira-button-secondary shrink-0"
                              aria-label={`Sign out device ${device.deviceLabel}`}
                              disabled={!!pending}
                              onClick={() => {
                                setError('');
                                setRevocationNotice('');
                                setConfirmTarget(device);
                              }}
                            >
                              Sign out device
                            </button>
                          )}
                        </li>
                      ))}
                    </ul>

                    {confirmTarget && (
                      <div
                        role="region"
                        aria-label={`Confirm sign out for ${confirmTarget.deviceLabel}`}
                        className="rounded-lg border border-sthira-border p-3 space-y-2.5 bg-sthira-surface"
                      >
                        <p className="text-sthira-text">
                          Sign out <strong>{confirmTarget.deviceLabel}</strong>? This device will stay signed out on its next check and will need a fresh Google sign-in to reconnect.
                        </p>
                        <div className="flex flex-wrap items-center gap-2">
                          <button
                            type="button"
                            className="sthira-button-primary"
                            disabled={!!pending}
                            onClick={() => void confirmRevokeDevice()}
                          >
                            {pending === 'revoke' ? 'Signing out device…' : `Confirm sign out`}
                          </button>
                          <button
                            type="button"
                            className="sthira-button-secondary"
                            disabled={!!pending}
                            onClick={() => setConfirmTarget(null)}
                          >
                            Cancel
                          </button>
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              )}

              {pending === 'sign-in' && (
                <p role="status">Finish sign-in in your browser.</p>
              )}

              {error && (
                <p role="alert" className="text-sthira-text">{error}</p>
              )}

              {!session && error && (
                <div className="flex flex-wrap items-center gap-3">
                  <button
                    type="button"
                    className="sthira-button-primary"
                    disabled={!!pending}
                    onClick={() => void loadSession()}
                  >
                    Retry session check
                  </button>
                  <button
                    type="button"
                    className="sthira-button-secondary"
                    disabled={!!pending}
                    onClick={() => void signOut()}
                  >
                    {pending === 'sign-out' ? 'Signing out…' : 'Sign out'}
                  </button>
                </div>
              )}
            </section>
          )}

          {/* Guarantees */}
          <div className="space-y-3 pt-2">
            <h5 className="font-bold text-xs uppercase tracking-wider text-[#6E6D66] dark:text-[#9E9C93]">
              Architectural Guarantees
            </h5>

            <div className="flex items-start gap-3 p-3 rounded-xl bg-[#FAF8F3] dark:bg-[#161714] border border-[#E8E5DC] dark:border-[#2C2E27]">
              <Shield className="w-4 h-4 text-[#2D5A27] dark:text-[#88D080] shrink-0 mt-0.5" />
              <div>
                <strong className="text-[#1E201C] dark:text-[#EDEAE2] font-semibold block mb-0.5">
                  Anti-Laundering Trigger
                </strong>
                <span className="text-[#6E6D66] dark:text-[#9E9C93] leading-relaxed">
                  Database trigger <code className="bg-[#EFECE4] dark:bg-[#252820] text-[#1E201C] dark:text-[#EDEAE2] px-1 py-0.5 rounded text-[11px]">nodes_origin_matches_authorship</code> strictly prohibits AI from forging human decisions.
                </span>
              </div>
            </div>

            <div className="flex items-start gap-3 p-3 rounded-xl bg-[#FAF8F3] dark:bg-[#161714] border border-[#E8E5DC] dark:border-[#2C2E27]">
              <Lock className="w-4 h-4 text-[#2B4C6F] dark:text-[#82B1FF] shrink-0 mt-0.5" />
              <div>
                <strong className="text-[#1E201C] dark:text-[#EDEAE2] font-semibold block mb-0.5">
                  Verbatim Quote Anchoring
                </strong>
                <span className="text-[#6E6D66] dark:text-[#9E9C93] leading-relaxed">
                  Claims survive only if verified directly against original text with exact boundary preservation.
                </span>
              </div>
            </div>

            <div className="flex items-start gap-3 p-3 rounded-xl bg-[#FAF8F3] dark:bg-[#161714] border border-[#E8E5DC] dark:border-[#2C2E27]">
              <HardDrive className="w-4 h-4 text-[#8C6D1F] dark:text-[#F3BA59] shrink-0 mt-0.5" />
              <div>
                <strong className="text-[#1E201C] dark:text-[#EDEAE2] font-semibold block mb-0.5">
                  100% Local-First Storage
                </strong>
                <span className="text-[#6E6D66] dark:text-[#9E9C93] leading-relaxed">
                  All knowledge resides in your local SQLite WAL database and human-readable Markdown files. Zero telemetry.
                </span>
              </div>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="px-6 py-3.5 bg-[#FAF8F3] dark:bg-[#161714] border-t border-[#ECE9E0] dark:border-[#2C2E27] flex justify-end">
          <button
            type="button"
            onClick={close}
            className="px-4 py-1.5 rounded-xl bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] font-medium hover:opacity-90 transition-opacity"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
};
