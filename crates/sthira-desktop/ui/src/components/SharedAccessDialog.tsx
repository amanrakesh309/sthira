import React, { useEffect, useId, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Info, Lock, ShieldCheck, Users, X } from 'lucide-react';
import { useDialogFocus } from '../hooks/useDialogFocus';
import {
  commitResourceAccess,
  getResourceAccess,
  listAccessMembers,
  listSharedProjects,
  makeRequestIdempotencyKey,
  previewResourceAccess,
  type AccessAudienceMember,
  type AccessChangePreview,
  type ResourceAccessPolicyView,
  type SharedProjectSummary,
  type WorkspaceListResponse,
} from '../api/workspaces';
import type { GoogleSession } from './AccountModal';

type ResourceKind = 'project' | 'page';

type PreviewState = {
  preview: AccessChangePreview;
  confirmed: boolean;
} | null;

interface SharedAccessDialogProps {
  workspaceId: string;
  resourceKind: ResourceKind;
  resourceId: string;
  resourceName: string;
  availableProjects?: SharedProjectSummary[];
  onClose: () => void;
  onCommitted: (policy: ResourceAccessPolicyView) => void;
  onAccessLost: (message: string, workspaces?: WorkspaceListResponse | null) => void;
  onSessionChange: (session: GoogleSession) => void;
}

export const SharedAccessDialog: React.FC<SharedAccessDialogProps> = ({
  workspaceId,
  resourceKind,
  resourceId,
  resourceName,
  availableProjects = [],
  onClose,
  onCommitted,
  onAccessLost,
  onSessionChange,
}) => {
  const panelRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const titleId = useId();
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState('');
  const [policy, setPolicy] = useState<ResourceAccessPolicyView | null>(null);
  const [members, setMembers] = useState<AccessAudienceMember[]>([]);
  const [projects, setProjects] = useState<SharedProjectSummary[]>(availableProjects);
  const [mode, setMode] = useState<string>('workspace');
  const [selected, setSelected] = useState<string[]>([]);
  const [followProjectId, setFollowProjectId] = useState<string>('');
  const [convertPages, setConvertPages] = useState<string[]>([]);
  const [previewState, setPreviewState] = useState<PreviewState>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState('');
  const commitKeyRef = useRef<string>('');
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  useDialogFocus(true, panelRef, () => {
    if (!pending) onClose();
  }, { initialFocus: closeRef });

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setLoadError('');
    setPolicy(null);
    setPreviewState(null);
    setConvertPages([]);
    commitKeyRef.current = '';
    const loadProjects = async () => {
      if (resourceKind !== 'page') return { outcome: 'ok' as const, data: [] as SharedProjectSummary[] };
      const all: SharedProjectSummary[] = [];
      let cursor: string | null = null;
      do {
        const response = await listSharedProjects(workspaceId, false, null, cursor, 100);
        if (cancelled || response.outcome !== 'ok') return response;
        all.push(...response.data.items);
        cursor = response.data.nextCursor ?? null;
      } while (cursor);
      return { outcome: 'ok' as const, data: all };
    };
    Promise.all([
      getResourceAccess(workspaceId, resourceKind, resourceId),
      listAccessMembers(workspaceId),
      loadProjects(),
    ])
      .then(([policyRes, membersRes, projectsRes]) => {
        if (cancelled) return;
        if (policyRes.outcome !== 'ok') {
          if (policyRes.outcome === 'error') setLoadError(policyRes.message);
          else setLoadError('Sharing controls are unavailable for this resource.');
          return;
        }
        if (membersRes.outcome !== 'ok') {
          if (membersRes.outcome === 'error') setLoadError(membersRes.message);
          else setLoadError('Members could not be loaded.');
          return;
        }
        if (projectsRes.outcome !== 'ok') {
          setLoadError(projectsRes.outcome === 'error' ? projectsRes.message : 'Projects could not be loaded.');
          return;
        }
        setProjects(projectsRes.data as SharedProjectSummary[]);
        setPolicy(policyRes.data);
        setMembers(membersRes.data);
        setMode(policyRes.data.accessMode);
        setSelected(policyRes.data.audience.map((m) => m.membershipId));
        setFollowProjectId(policyRes.data.accessProjectId ?? '');
      })
      .catch(() => {
        if (!cancelled) setLoadError('Sharing controls could not be loaded. Try again.');
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceId, resourceKind, resourceId]);

  const modes = useMemo(() => {
    if (resourceKind === 'project') {
      return [
        { value: 'workspace', label: 'Workspace access', hint: 'Every active workspace member can open it.' },
        { value: 'restricted', label: 'Selected members', hint: 'Only chosen members and workspace administrators.' },
      ];
    }
    return [
      { value: 'workspace', label: 'Workspace access', hint: 'Every active workspace member can open it.' },
      { value: 'selected', label: 'Selected members', hint: 'Only chosen members and workspace administrators.' },
      { value: 'project', label: 'Access follows a project', hint: 'The page follows one project’s current audience.' },
    ];
  }, [resourceKind]);

  const invalidatePreview = () => {
    setPreviewState(null);
    setError('');
  };

  const toggleMember = (membershipId: string) => {
    setSelected((prev) =>
      prev.includes(membershipId) ? prev.filter((id) => id !== membershipId) : [...prev, membershipId],
    );
    invalidatePreview();
  };

  const handlePreview = async () => {
    if (pending) return;
    setPending(true);
    setError('');
    try {
      const res = await previewResourceAccess(workspaceId, resourceKind, resourceId, {
        proposedMode: mode as 'workspace' | 'restricted' | 'project' | 'selected',
        selectedMembershipIds: selected,
        followProjectId: followProjectId || null,
        convertPageIds: convertPages,
        confirmExpansion: false,
      });
      if (!alive.current) return;
      if (res.outcome === 'sessionUpdated') {
        onSessionChange(res.session);
        setError('Sign in again to manage sharing.');
        return;
      }
      if (res.outcome === 'accessLost') {
        onAccessLost(res.message, res.workspaces);
        return;
      }
      if (res.outcome === 'error') {
        setError(res.message);
        return;
      }
      commitKeyRef.current = makeRequestIdempotencyKey();
      setPreviewState({ preview: res.data, confirmed: false });
    } catch {
      setError('The sharing preview could not be prepared. Try again.');
    } finally {
      setPending(false);
    }
  };

  const handleCommit = async () => {
    if (!previewState || pending) return;
    if (previewState.preview.expansionRequiresConfirmation && !previewState.confirmed) {
      setError('Confirm that this change shares the resource with more members.');
      return;
    }
    if (!commitKeyRef.current) commitKeyRef.current = makeRequestIdempotencyKey();
    setPending(true);
    setError('');
    try {
      const res = await commitResourceAccess(
        workspaceId,
        resourceKind,
        resourceId,
        previewState.preview.previewId,
        commitKeyRef.current,
        previewState.confirmed,
      );
      if (!alive.current) return;
      if (res.outcome === 'sessionUpdated') {
        onSessionChange(res.session);
        setError('Sign in again to finish the sharing change.');
        return;
      }
      if (res.outcome === 'accessLost') {
        onAccessLost(res.message, res.workspaces);
        return;
      }
      if (res.outcome === 'error') {
        setError(res.message);
        setPreviewState(null);
        commitKeyRef.current = '';
        return;
      }
      const policy = res.data.policy;
      setPolicy(policy);
      setMode(policy.accessMode);
      setSelected(policy.audience.map((m) => m.membershipId));
      setPreviewState(null);
      commitKeyRef.current = '';
      onCommitted(policy);
    } catch {
      setError('The sharing result could not be confirmed. Retry Save sharing to check the same request.');
    } finally {
      setPending(false);
    }
  };

  const selectedFollowers = policy?.followingPages ?? [];

  return createPortal(
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/40 backdrop-blur-sm"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !pending) onClose();
      }}
    >
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="w-full max-w-lg max-h-[90vh] overflow-y-auto rounded-2xl border border-sthira-border bg-sthira-surface text-sthira-text shadow-2xl"
      >
        <div className="px-5 py-4 border-b border-sthira-border bg-sthira-canvas flex items-start justify-between gap-3">
          <div className="flex items-start gap-2.5 min-w-0">
            <div className="w-8 h-8 rounded-xl bg-sthira-surface-muted text-sthira-primary flex items-center justify-center shrink-0">
              {typeof mode === 'string' && mode !== 'workspace' ? (
                <Lock className="w-4 h-4" />
              ) : (
                <Users className="w-4 h-4" />
              )}
            </div>
            <div className="min-w-0">
              <h2 id={titleId} className="text-sm font-semibold truncate">
                Share {resourceKind === 'project' ? 'project' : 'page'}
              </h2>
              <p className="text-xs text-sthira-text-muted truncate">{resourceName}</p>
            </div>
          </div>
          <button
            ref={closeRef}
            type="button"
            className="w-8 h-8 rounded-lg text-sthira-text-muted hover:text-sthira-text hover:bg-sthira-surface-muted flex items-center justify-center"
            onClick={() => !pending && onClose()}
            aria-label="Close sharing controls"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-5 space-y-4">
          <div className="flex items-start gap-2 rounded-xl border border-sthira-border bg-sthira-canvas px-3 py-2.5 text-xs text-sthira-text-muted">
            <Info className="w-4 h-4 shrink-0 mt-0.5 text-sthira-primary" aria-hidden="true" />
            <p>
              {policy?.administrativeDisclosure ??
                'Owners and Admins can always open restricted resources and full workspace backups. Restricting a resource does not hide it from workspace administrators.'}
            </p>
          </div>

          {loading && <p className="text-sm text-sthira-text-muted">Loading sharing controls…</p>}
          {loadError && (
            <div className="rounded-xl border border-sthira-danger/40 bg-sthira-danger/10 px-3 py-2 text-xs text-sthira-danger">
              {loadError}
            </div>
          )}

          {!loading && policy && (
            <>
              <fieldset className="space-y-2" disabled={pending || !policy.canManage}>
                <legend className="text-xs font-medium text-sthira-text-muted">Workspace access</legend>
                {modes.map((option) => (
                  <label
                    key={option.value}
                    className="flex items-start gap-2.5 rounded-xl border border-sthira-border px-3 py-2.5 cursor-pointer hover:bg-sthira-canvas"
                  >
                    <input
                      type="radio"
                      name="access-mode"
                      className="mt-0.5"
                      value={option.value}
                      checked={mode === option.value}
                      onChange={() => {
                        setMode(option.value);
                        invalidatePreview();
                      }}
                    />
                    <span className="min-w-0">
                      <span className="block text-sm">{option.label}</span>
                      <span className="block text-xs text-sthira-text-muted">{option.hint}</span>
                    </span>
                  </label>
                ))}
              </fieldset>

              {mode === 'project' && resourceKind === 'page' && (
                <label className="block text-xs text-sthira-text-muted">
                  Access follows
                  <select
                    className="mt-1 w-full rounded-lg border border-sthira-border bg-sthira-surface px-2.5 py-2 text-sm text-sthira-text"
                    value={followProjectId}
                    disabled={pending}
                    onChange={(event) => {
                      setFollowProjectId(event.target.value);
                      invalidatePreview();
                    }}
                  >
                    <option value="">Choose a project</option>
                    {projects.map((project) => (
                      <option key={project.projectId} value={project.projectId}>
                        {project.name}
                      </option>
                    ))}
                  </select>
                  {policy.accessProjectName && (
                    <span className="mt-1 block">Currently follows {policy.accessProjectName}</span>
                  )}
                </label>
              )}

              {(mode === 'selected' || mode === 'restricted') && (
                <div className="space-y-2">
                  <p className="text-xs font-medium text-sthira-text-muted">Selected members</p>
                  {members.length === 0 ? (
                    <p className="text-xs text-sthira-text-muted">No other active members are available.</p>
                  ) : (
                    <div className="max-h-44 overflow-y-auto rounded-xl border border-sthira-border divide-y divide-sthira-border">
                      {members.map((member) => (
                        <label key={member.membershipId} className="flex items-center gap-2.5 px-3 py-2 text-sm cursor-pointer hover:bg-sthira-canvas">
                          <input
                            type="checkbox"
                            disabled={pending}
                            checked={selected.includes(member.membershipId)}
                            onChange={() => toggleMember(member.membershipId)}
                          />
                          <span className="truncate">{member.displayName}</span>
                          <span className="ml-auto text-xs text-sthira-text-muted capitalize">{member.role}</span>
                        </label>
                      ))}
                    </div>
                  )}
                </div>
              )}

              {resourceKind === 'project' && mode === 'restricted' && selectedFollowers.length > 0 && (
                <div className="space-y-2">
                  <p className="text-xs font-medium text-sthira-text-muted">
                    Page access policies
                  </p>
                  <div className="max-h-40 overflow-y-auto rounded-xl border border-sthira-border divide-y divide-sthira-border">
                    {selectedFollowers.map((page) => (
                      <label key={page.pageId} className="flex items-center gap-2.5 px-3 py-2 text-sm cursor-pointer hover:bg-sthira-canvas">
                        <input
                          type="checkbox"
                          disabled={pending || (page.accessMode === 'project' && page.accessProjectId === resourceId)}
                          checked={convertPages.includes(page.pageId)}
                          onChange={() => {
                            setConvertPages((prev) =>
                              prev.includes(page.pageId)
                                ? prev.filter((id) => id !== page.pageId)
                                : [...prev, page.pageId],
                            );
                            invalidatePreview();
                          }}
                        />
                        <span className="truncate">{page.title}</span>
                        <span className="ml-auto text-xs text-sthira-text-muted">
                          {page.accessMode === 'project'
                            ? 'Access follows a project'
                            : page.accessMode === 'selected'
                              ? 'Selected members'
                              : 'Workspace access'}
                        </span>
                      </label>
                    ))}
                  </div>
                  <p className="text-xs text-sthira-text-muted">
                    Select pages to change them to “Access follows this project”. Unchecked pages keep
                    their current policy. Saving is one atomic, version-checked change.
                  </p>
                </div>
              )}

              <div className="flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  className="rounded-lg border border-sthira-border px-3 py-2 text-sm hover:bg-sthira-canvas disabled:opacity-50"
                  onClick={() => void handlePreview()}
                  disabled={pending || !policy.canManage || (mode === 'project' && !followProjectId)}
                >
                  {pending ? 'Checking…' : 'Preview change'}
                </button>
                {previewState && (
                  <button
                    type="button"
                    className="rounded-lg bg-sthira-primary px-3 py-2 text-sm text-white hover:opacity-90 disabled:opacity-50"
                    onClick={handleCommit}
                    disabled={pending}
                  >
                    {pending ? 'Saving…' : 'Save sharing'}
                  </button>
                )}
              </div>

              {previewState && (
                <div className="rounded-xl border border-sthira-border bg-sthira-canvas p-3 space-y-2 text-xs">
                  <p className="flex items-center gap-1.5 font-medium text-sthira-text">
                    <ShieldCheck className="w-3.5 h-3.5 text-sthira-primary" />
                    {previewState.preview.currentMode} → {previewState.preview.proposedMode}
                  </p>
                  {previewState.preview.audienceChange.added.length > 0 && (
                    <p>Added: {previewState.preview.audienceChange.added.map((m) => m.displayName).join(', ')}</p>
                  )}
                  {previewState.preview.audienceChange.removed.length > 0 && (
                    <p>Removed: {previewState.preview.audienceChange.removed.map((m) => m.displayName).join(', ')}</p>
                  )}
                  {previewState.preview.followProjectName && (
                    <p>Access follows {previewState.preview.followProjectName}.</p>
                  )}
                  {previewState.preview.conversionPageIds.length > 0 && (
                    <p>{previewState.preview.conversionPageIds.length} page(s) will follow this project.</p>
                  )}
                  <p className="text-sthira-text-muted">{previewState.preview.sharesHistory}</p>
                  {previewState.preview.expansionRequiresConfirmation && (
                    <label className="flex items-start gap-2 rounded-lg border border-sthira-warning/40 bg-sthira-warning/10 p-2">
                      <input
                        type="checkbox"
                        disabled={pending}
                        checked={previewState.confirmed}
                        onChange={(event) =>
                          setPreviewState({ ...previewState, confirmed: event.target.checked })
                        }
                      />
                      <span>I understand this shares the resource with more members.</span>
                    </label>
                  )}
                </div>
              )}

              {error && (
                <div className="rounded-xl border border-sthira-danger/40 bg-sthira-danger/10 px-3 py-2 text-xs text-sthira-danger">
                  {error}
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>,
    document.body,
  );
};
