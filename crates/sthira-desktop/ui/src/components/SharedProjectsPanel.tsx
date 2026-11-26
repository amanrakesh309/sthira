import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Archive,
  ArrowLeft,
  Edit3,
  FileText,
  FolderKanban,
  FolderPlus,
  Plus,
  RefreshCw,
  RotateCcw,
  Search,
  Trash2,
  X,
} from 'lucide-react';
import {
  archiveSharedProject,
  attachPageToProject,
  canAuthorSharedContent,
  canManageSharedProject,
  createSharedProject,
  detachPageFromProject,
  getSharedProject,
  listProjectPickerPages,
  listSharedProjects,
  makeRequestIdempotencyKey,
  restoreSharedProject,
  updateSharedProject,
  type SharedPageSummary,
  type SharedProjectDetails,
  type SharedProjectSummary,
  type WorkspaceListResponse,
  type WorkspaceResponse,
  type WorkspaceRole,
} from '../api/workspaces';
import type { GoogleSession } from './AccountModal';
import { SharedPagesPanel } from './SharedPagesPanel';

export interface ProjectOpenRequest {
  projectId?: string | null;
  pageId?: string | null;
  proposalId?: string | null;
  mode?: 'newProject';
}

interface SharedProjectsPanelProps {
  workspaceId: string;
  workspaceName: string;
  myRole: WorkspaceRole;
  currentUid?: string | null;
  authorDisplayName?: string;
  refreshSignal: number;
  pendingOpenRequest?: ProjectOpenRequest | null;
  onConsumedOpenRequest?: () => void;
  onFallbackToPages?: (pageId: string, proposalId?: string | null) => void;
  registerBeforeLeave?: (guard: (() => Promise<boolean>) | null) => void;
  onAccessLost: (message: string, workspaces?: WorkspaceListResponse | null) => void;
  onSessionChange: (session: GoogleSession) => void;
  onErrorNotice: (message: string) => void;
}

export const SharedProjectsPanel: React.FC<SharedProjectsPanelProps> = ({
  workspaceId,
  workspaceName,
  myRole,
  currentUid,
  authorDisplayName,
  refreshSignal,
  pendingOpenRequest = null,
  onConsumedOpenRequest,
  onFallbackToPages,
  registerBeforeLeave,
  onAccessLost,
  onSessionChange,
  onErrorNotice,
}) => {
  const [projects, setProjects] = useState<SharedProjectSummary[]>([]);
  const [projectsNextCursor, setProjectsNextCursor] = useState<string | null>(null);
  const [includeArchived, setIncludeArchived] = useState(false);
  const [directoryQuery, setDirectoryQuery] = useState('');
  const [loadingList, setLoadingList] = useState(true);

  const [selectedProject, setSelectedProject] = useState<SharedProjectDetails | null>(null);
  const [projectPageQuery, setProjectPageQuery] = useState('');
  const [openedPage, setOpenedPage] = useState<{
    pageId: string;
    proposalId?: string | null;
  } | null>(null);

  const [createOpen, setCreateOpen] = useState(false);
  const [newName, setNewName] = useState('');
  const [newPurpose, setNewPurpose] = useState('');
  const createKeyRef = useRef<{ sig: string; key: string } | null>(null);

  const [editingMeta, setEditingMeta] = useState(false);
  const [editName, setEditName] = useState('');
  const [editPurpose, setEditPurpose] = useState('');
  const updateKeyRef = useRef<{ sig: string; key: string } | null>(null);

  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerQuery, setPickerQuery] = useState('');
  const [pickerPages, setPickerPages] = useState<SharedPageSummary[]>([]);
  const [pickerNextCursor, setPickerNextCursor] = useState<string | null>(null);
  const [pickerLoading, setPickerLoading] = useState(false);
  const [pickerError, setPickerError] = useState('');

  const [busy, setBusy] = useState(false);
  const [panelError, setPanelError] = useState('');
  const [panelNotice, setPanelNotice] = useState('');

  const listReqRef = useRef(0);
  const detailReqRef = useRef(0);
  const pickerReqRef = useRef(0);
  const mountedRef = useRef(true);
  const readerBeforeLeaveRef = useRef<(() => Promise<boolean>) | null>(null);
  const callbacksRef = useRef({
    onAccessLost,
    onSessionChange,
    onErrorNotice,
    onFallbackToPages,
  });
  callbacksRef.current = {
    onAccessLost,
    onSessionChange,
    onErrorNotice,
    onFallbackToPages,
  };

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const canAuthor = canAuthorSharedContent(myRole);
  const canManage = canManageSharedProject(myRole);

  const registerReaderBeforeLeave = useCallback(
    (guard: (() => Promise<boolean>) | null) => {
      readerBeforeLeaveRef.current = guard;
      registerBeforeLeave?.(guard);
    },
    [registerBeforeLeave],
  );

  const handleFailureResponse = useCallback(
    <T,>(
      res: WorkspaceResponse<T>,
      fallbackMsg: string,
      setErrorFn: (msg: string) => void = setPanelError,
    ) => {
      if (res.outcome === 'accessLost') {
        setSelectedProject(null);
        setOpenedPage(null);
        setPickerOpen(false);
        callbacksRef.current.onAccessLost(res.message, res.workspaces);
        return;
      }
      if (res.outcome === 'sessionUpdated') {
        callbacksRef.current.onSessionChange(res.session);
        const msg =
          res.session.status === 'unverified'
            ? res.session.message
            : 'Sign in with Google to access team projects.';
        setErrorFn(msg);
        return;
      }
      if (res.outcome === 'error') {
        setErrorFn(res.message || fallbackMsg);
      }
    },
    [],
  );

  const loadProjectsList = useCallback(
    async (
      archivedFlag = includeArchived,
      queryText = directoryQuery,
      appendCursor?: string | null,
    ) => {
      const seq = ++listReqRef.current;
      setLoadingList(true);
      if (!appendCursor) setPanelError('');
      try {
        const res = await listSharedProjects(
          workspaceId,
          archivedFlag,
          queryText.trim() || null,
          appendCursor ?? null,
        );
        if (seq !== listReqRef.current || !mountedRef.current) return;
        if (res.outcome === 'ok') {
          setProjects((prev) =>
            appendCursor ? [...prev, ...res.data.items] : res.data.items,
          );
          setProjectsNextCursor(res.data.nextCursor ?? null);
          return;
        }
        handleFailureResponse(res, 'Could not load shared projects.');
      } catch (err) {
        if (seq !== listReqRef.current || !mountedRef.current) return;
        setPanelError(
          typeof err === 'string' ? err : 'Could not load shared projects.',
        );
      } finally {
        if (seq === listReqRef.current && mountedRef.current) {
          setLoadingList(false);
        }
      }
    },
    [directoryQuery, handleFailureResponse, includeArchived, workspaceId],
  );

  const openProjectById = useCallback(
    async (
      projectId: string,
      pageQuery = projectPageQuery,
      appendCursor?: string | null,
    ): Promise<SharedProjectDetails | null> => {
      const seq = ++detailReqRef.current;
      setBusy(true);
      if (!appendCursor) setPanelError('');
      try {
        const res = await getSharedProject(
          workspaceId,
          projectId,
          pageQuery.trim() || null,
          appendCursor ?? null,
        );
        if (seq !== detailReqRef.current || !mountedRef.current) return null;
        if (res.outcome === 'ok') {
          let nextDetails = res.data;
          if (appendCursor) {
            setSelectedProject((prev) => {
              if (!prev || prev.project.projectId !== res.data.project.projectId) {
                nextDetails = res.data;
                return res.data;
              }
              nextDetails = {
                ...res.data,
                pages: [...prev.pages, ...res.data.pages],
              };
              return nextDetails;
            });
          } else {
            setSelectedProject(res.data);
          }
          return nextDetails;
        }
        handleFailureResponse(res, 'Could not load project details.');
        return null;
      } catch (err) {
        if (seq !== detailReqRef.current || !mountedRef.current) return null;
        setPanelError(
          typeof err === 'string' ? err : 'Could not load project details.',
        );
        return null;
      } finally {
        if (seq === detailReqRef.current && mountedRef.current) {
          setBusy(false);
        }
      }
    },
    [handleFailureResponse, projectPageQuery, workspaceId],
  );

  const loadPickerPages = useCallback(
    async (
      projectId: string,
      searchText = pickerQuery,
      appendCursor?: string | null,
    ) => {
      const seq = ++pickerReqRef.current;
      setPickerLoading(true);
      if (!appendCursor) setPickerError('');
      try {
        const res = await listProjectPickerPages(
          workspaceId,
          projectId,
          searchText.trim() || null,
          appendCursor ?? null,
        );
        if (seq !== pickerReqRef.current || !mountedRef.current) return;
        if (res.outcome === 'ok') {
          setPickerPages((prev) =>
            appendCursor ? [...prev, ...res.data.items] : res.data.items,
          );
          setPickerNextCursor(res.data.nextCursor ?? null);
          return;
        }
        handleFailureResponse(
          res,
          'Could not load available workspace pages.',
          setPickerError,
        );
      } catch (err) {
        if (seq !== pickerReqRef.current || !mountedRef.current) return;
        setPickerError(
          typeof err === 'string'
            ? err
            : 'Could not load available workspace pages.',
        );
      } finally {
        if (seq === pickerReqRef.current && mountedRef.current) {
          setPickerLoading(false);
        }
      }
    },
    [handleFailureResponse, pickerQuery, workspaceId],
  );

  useEffect(() => {
    listReqRef.current += 1;
    detailReqRef.current += 1;
    pickerReqRef.current += 1;
    setSelectedProject(null);
    setOpenedPage(null);
    setCreateOpen(false);
    setEditingMeta(false);
    setPickerOpen(false);
    setPanelError('');
    setPanelNotice('');
    setDirectoryQuery('');
    setProjectPageQuery('');
    void loadProjectsList(includeArchived, '', null);
  }, [workspaceId]); // eslint-disable-line react-hooks/exhaustive-deps

  const selectedProjectRef = useRef(selectedProject);
  const openedPageRef = useRef(openedPage);
  const projectPageQueryRef = useRef(projectPageQuery);
  const directoryQueryRef = useRef(directoryQuery);
  const includeArchivedRef = useRef(includeArchived);
  selectedProjectRef.current = selectedProject;
  openedPageRef.current = openedPage;
  projectPageQueryRef.current = projectPageQuery;
  directoryQueryRef.current = directoryQuery;
  includeArchivedRef.current = includeArchived;

  const verifyMembershipOnRefresh = useCallback(
    async (projectId: string, pageId: string): Promise<'active' | 'detached' | 'projectGone'> => {
      try {
        const res = await getSharedProject(workspaceId, projectId, null, null, 100);
        if (!mountedRef.current) return 'active';
        if (res.outcome === 'ok') {
          const exists = res.data.pages.some((m) => m.page.pageId === pageId);
          if (exists) return 'active';
          if (!res.data.pagesNextCursor) return 'detached';
          return 'active';
        }
        if (res.outcome === 'accessLost') {
          callbacksRef.current.onAccessLost(res.message, res.workspaces);
          return 'projectGone';
        }
        if (res.outcome === 'error' && res.code === 'project_not_found') {
          return 'projectGone';
        }
      } catch {
        // Keep user safely on current view during transient connectivity hiccup.
      }
      return 'active';
    },
    [workspaceId],
  );

  const prevRefreshRef = useRef(refreshSignal);
  useEffect(() => {
    if (refreshSignal === prevRefreshRef.current) return;
    prevRefreshRef.current = refreshSignal;
    const curProj = selectedProjectRef.current;
    const curPage = openedPageRef.current;
    if (curProj && curPage) {
      void (async () => {
        const state = await verifyMembershipOnRefresh(
          curProj.project.projectId,
          curPage.pageId,
        );
        if (!mountedRef.current) return;
        if (state === 'detached') {
          const ok = readerBeforeLeaveRef.current
            ? await readerBeforeLeaveRef.current()
            : true;
          if (!ok || !mountedRef.current) return;
          setOpenedPage(null);
          await openProjectById(curProj.project.projectId, projectPageQueryRef.current, null);
          setPanelNotice(
            `“${curProj.project.name}” no longer includes that page in its active list.`,
          );
        } else if (state === 'projectGone') {
          const ok = readerBeforeLeaveRef.current
            ? await readerBeforeLeaveRef.current()
            : true;
          if (!ok || !mountedRef.current) return;
          setOpenedPage(null);
          setSelectedProject(null);
          await loadProjectsList(includeArchivedRef.current, directoryQueryRef.current, null);
          setPanelNotice('That project is no longer available.');
        }
      })();
      return;
    }
    if (curProj) {
      void openProjectById(curProj.project.projectId, projectPageQueryRef.current, null);
    } else {
      void loadProjectsList(includeArchivedRef.current, directoryQueryRef.current, null);
    }
  }, [
    loadProjectsList,
    openProjectById,
    refreshSignal,
    verifyMembershipOnRefresh,
  ]);

  useEffect(() => {
    if (!pendingOpenRequest) return;
    if (pendingOpenRequest.mode === 'newProject') {
      setOpenedPage(null);
      setSelectedProject(null);
      setCreateOpen(true);
      onConsumedOpenRequest?.();
      return;
    }
    if (pendingOpenRequest.projectId) {
      const targetProjectId = pendingOpenRequest.projectId;
      const targetPageId = pendingOpenRequest.pageId ?? null;
      const targetProposalId = pendingOpenRequest.proposalId ?? null;
      onConsumedOpenRequest?.();
      void (async () => {
        const seq = ++detailReqRef.current;
        setBusy(true);
        setPanelError('');
        try {
          const res = await getSharedProject(
            workspaceId,
            targetProjectId,
            null,
            null,
            targetPageId ? 100 : undefined,
          );
          if (seq !== detailReqRef.current || !mountedRef.current) return;
          if (res.outcome === 'ok') {
            setProjectPageQuery('');
            setPickerOpen(false);
            setEditingMeta(false);
            setSelectedProject(res.data);
            if (targetPageId) {
              const hasMembership = res.data.pages.some(
                (item) => item.page.pageId === targetPageId,
              );
              if (hasMembership || res.data.pagesNextCursor) {
                setOpenedPage({
                  pageId: targetPageId,
                  proposalId: targetProposalId,
                });
              } else if (callbacksRef.current.onFallbackToPages) {
                setSelectedProject(null);
                setOpenedPage(null);
                callbacksRef.current.onFallbackToPages(targetPageId, targetProposalId);
              } else {
                setOpenedPage(null);
                setPanelNotice(
                  `Showing “${res.data.project.name}” — that page membership has ended.`,
                );
              }
            } else {
              setOpenedPage(null);
            }
            return;
          }
          if (targetPageId && callbacksRef.current.onFallbackToPages) {
            callbacksRef.current.onFallbackToPages(targetPageId, targetProposalId);
            return;
          }
          handleFailureResponse(res, 'Could not open that shared project.');
        } catch (err) {
          if (seq !== detailReqRef.current || !mountedRef.current) return;
          setPanelError(
            typeof err === 'string' ? err : 'Could not open that shared project.',
          );
        } finally {
          if (seq === detailReqRef.current && mountedRef.current) setBusy(false);
        }
      })();
    }
  }, [handleFailureResponse, onConsumedOpenRequest, pendingOpenRequest, workspaceId]);

  const handleBackFromPage = async () => {
    if (readerBeforeLeaveRef.current && !(await readerBeforeLeaveRef.current())) {
      return;
    }
    const closingPageId = openedPage?.pageId ?? null;
    const projectId = selectedProject?.project.projectId ?? null;
    const projectName = selectedProject?.project.name ?? 'project';
    setOpenedPage(null);
    if (!projectId) {
      void loadProjectsList(includeArchived, directoryQuery, null);
      return;
    }
    const refreshed = await openProjectById(projectId, projectPageQuery, null);
    if (!refreshed) {
      setSelectedProject(null);
      void loadProjectsList(includeArchived, directoryQuery, null);
      return;
    }
    if (closingPageId) {
      const check = await verifyMembershipOnRefresh(projectId, closingPageId);
      if (check === 'detached' && mountedRef.current) {
        setPanelNotice(`This page was removed from “${projectName}”.`);
      }
    }
    void loadProjectsList(includeArchived, directoryQuery, null);
  };

  const handleCreateProject = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!newName.trim() || busy) return;
    const name = newName.trim();
    const purpose = newPurpose.trim();
    const sig = `${workspaceId}:${name}:${purpose}`;
    if (!createKeyRef.current || createKeyRef.current.sig !== sig) {
      createKeyRef.current = { sig, key: makeRequestIdempotencyKey('prj') };
    }
    setBusy(true);
    setPanelError('');
    setPanelNotice('');
    try {
      const res = await createSharedProject(workspaceId, {
        name,
        purpose,
        idempotencyKey: createKeyRef.current.key,
      });
      if (!mountedRef.current) return;
      if (res.outcome === 'ok') {
        createKeyRef.current = null;
        setNewName('');
        setNewPurpose('');
        setCreateOpen(false);
        setProjectPageQuery('');
        setSelectedProject(res.data.project);
        setPanelNotice(`Created project “${res.data.project.project.name}”.`);
        void loadProjectsList(includeArchived, directoryQuery, null);
        return;
      }
      handleFailureResponse(res, 'Could not create project.');
    } catch (err) {
      if (!mountedRef.current) return;
      setPanelError(typeof err === 'string' ? err : 'Could not create project.');
    } finally {
      if (mountedRef.current) setBusy(false);
    }
  };

  const handleSaveMetadata = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!selectedProject || !editName.trim() || busy) return;
    const projectId = selectedProject.project.projectId;
    const name = editName.trim();
    const purpose = editPurpose.trim();
    const version = selectedProject.project.version;
    const sig = `${projectId}:${version}:${name}:${purpose}`;
    if (!updateKeyRef.current || updateKeyRef.current.sig !== sig) {
      updateKeyRef.current = { sig, key: makeRequestIdempotencyKey('prjupd') };
    }
    setBusy(true);
    setPanelError('');
    setPanelNotice('');
    try {
      const res = await updateSharedProject(
        workspaceId,
        projectId,
        name,
        purpose,
        version,
        updateKeyRef.current.key,
      );
      if (!mountedRef.current) return;
      if (res.outcome === 'ok') {
        updateKeyRef.current = null;
        setSelectedProject(res.data);
        setEditingMeta(false);
        setPanelNotice(`Saved project details for “${res.data.project.name}”.`);
        void loadProjectsList(includeArchived, directoryQuery, null);
        return;
      }
      handleFailureResponse(res, 'Could not update project details.');
    } catch (err) {
      if (!mountedRef.current) return;
      setPanelError(typeof err === 'string' ? err : 'Could not update project details.');
    } finally {
      if (mountedRef.current) setBusy(false);
    }
  };

  const handleArchiveToggle = async (archive: boolean) => {
    if (!selectedProject || busy) return;
    setBusy(true);
    setPanelError('');
    setPanelNotice('');
    try {
      const key = makeRequestIdempotencyKey(archive ? 'prjarch' : 'prjrest');
      const res = archive
        ? await archiveSharedProject(workspaceId, selectedProject.project.projectId, key)
        : await restoreSharedProject(workspaceId, selectedProject.project.projectId, key);
      if (!mountedRef.current) return;
      if (res.outcome === 'ok') {
        setSelectedProject(res.data);
        setEditingMeta(false);
        setPickerOpen(false);
        setPanelNotice(archive ? 'Project archived.' : 'Project restored.');
        void loadProjectsList(includeArchived, directoryQuery, null);
        return;
      }
      handleFailureResponse(res, 'Could not update project archive state.');
    } catch (err) {
      if (!mountedRef.current) return;
      setPanelError(
        typeof err === 'string' ? err : 'Could not update project archive state.',
      );
    } finally {
      if (mountedRef.current) setBusy(false);
    }
  };

  const handleAttachPage = async (pageId: string) => {
    if (!selectedProject || busy) return;
    const projectId = selectedProject.project.projectId;
    setBusy(true);
    setPickerError('');
    setPanelError('');
    try {
      const key = makeRequestIdempotencyKey('prjatt');
      const res = await attachPageToProject(workspaceId, projectId, pageId, key);
      if (!mountedRef.current) return;
      if (res.outcome === 'ok') {
        setSelectedProject(res.data.project);
        setPanelNotice('Added page to project.');
        void loadPickerPages(projectId, pickerQuery, null);
        void loadProjectsList(includeArchived, directoryQuery, null);
        return;
      }
      handleFailureResponse(res, 'Could not add page to project.', setPickerError);
    } catch (err) {
      if (!mountedRef.current) return;
      setPickerError(typeof err === 'string' ? err : 'Could not add page to project.');
    } finally {
      if (mountedRef.current) setBusy(false);
    }
  };

  const handleDetachPage = async (pageId: string, membershipId: string, pageTitle: string) => {
    if (!selectedProject || busy) return;
    const projectId = selectedProject.project.projectId;
    setBusy(true);
    setPanelError('');
    setPanelNotice('');
    try {
      const key = makeRequestIdempotencyKey('prjdet');
      const res = await detachPageFromProject(
        workspaceId,
        projectId,
        pageId,
        membershipId,
        key,
      );
      if (!mountedRef.current) return;
      if (res.outcome === 'ok') {
        setSelectedProject(res.data);
        setPanelNotice(`Removed “${pageTitle}” from this project.`);
        if (pickerOpen) {
          void loadPickerPages(projectId, pickerQuery, null);
        }
        void loadProjectsList(includeArchived, directoryQuery, null);
        return;
      }
      handleFailureResponse(res, 'Could not remove page from project.');
    } catch (err) {
      if (!mountedRef.current) return;
      setPanelError(
        typeof err === 'string' ? err : 'Could not remove page from project.',
      );
    } finally {
      if (mountedRef.current) setBusy(false);
    }
  };

  const handleBackFromPageRef = useRef<() => void>(() => {});
  handleBackFromPageRef.current = () => {
    void handleBackFromPage();
  };

  const readerOpenRequest = useMemo(
    () =>
      openedPage
        ? {
            pageId: openedPage.pageId,
            proposalId: openedPage.proposalId ?? null,
          }
        : null,
    [openedPage?.pageId, openedPage?.proposalId],
  );

  const selectedProjectId = selectedProject?.project.projectId ?? null;
  const selectedProjectName = selectedProject?.project.name ?? '';
  const readerProjectContext = useMemo(
    () =>
      selectedProjectId
        ? {
            projectId: selectedProjectId,
            projectName: selectedProjectName,
            onBackToProject: () => handleBackFromPageRef.current(),
          }
        : null,
    [selectedProjectId, selectedProjectName],
  );

  if (selectedProject && openedPage && readerOpenRequest && readerProjectContext) {
    return (
      <SharedPagesPanel
        key={`${selectedProject.project.projectId}:${openedPage.pageId}`}
        workspaceId={workspaceId}
        workspaceName={workspaceName}
        myRole={myRole}
        currentUid={currentUid}
        authorDisplayName={authorDisplayName}
        refreshSignal={refreshSignal}
        pendingOpenRequest={readerOpenRequest}
        originProjectContext={readerProjectContext}
        registerBeforeLeave={registerReaderBeforeLeave}
        onAccessLost={onAccessLost}
        onSessionChange={onSessionChange}
        onErrorNotice={onErrorNotice}
      />
    );
  }

  if (selectedProject) {
    const { project, pages, pagesNextCursor } = selectedProject;
    const isArchived = project.archivedAtMs != null;

    return (
      <div className="space-y-5" data-project-id={project.projectId}>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <button
            type="button"
            onClick={() => {
              setSelectedProject(null);
              setEditingMeta(false);
              setPickerOpen(false);
              setPanelError('');
              setPanelNotice('');
              void loadProjectsList(includeArchived, directoryQuery, null);
            }}
            className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            <span>Back to Projects</span>
          </button>

          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={() =>
                void openProjectById(project.projectId, projectPageQuery, null)
              }
              className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${busy ? 'animate-spin' : ''}`} />
              <span>Refresh</span>
            </button>
            {canManage && !isArchived && (
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  setEditName(project.name);
                  setEditPurpose(project.purpose);
                  setEditingMeta((prev) => !prev);
                  setPanelError('');
                }}
                className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
              >
                <Edit3 className="w-3.5 h-3.5" />
                <span>Edit project</span>
              </button>
            )}
            {canManage && (
              <button
                type="button"
                disabled={busy}
                onClick={() => void handleArchiveToggle(!isArchived)}
                className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
              >
                {isArchived ? (
                  <RotateCcw className="w-3.5 h-3.5" />
                ) : (
                  <Archive className="w-3.5 h-3.5" />
                )}
                <span>{isArchived ? 'Restore project' : 'Archive project'}</span>
              </button>
            )}
          </div>
        </div>

        {panelNotice && (
          <p
            role="status"
            className="rounded-xl border border-sthira-border bg-sthira-surface px-4 py-2.5 text-xs text-sthira-text"
          >
            {panelNotice}
          </p>
        )}
        {panelError && (
          <div
            role="alert"
            className="rounded-xl border border-sthira-border bg-sthira-surface-muted px-4 py-2.5 text-xs text-sthira-danger flex flex-wrap items-center justify-between gap-2"
          >
            <span>{panelError}</span>
            <button
              type="button"
              onClick={() => void openProjectById(project.projectId, projectPageQuery, null)}
              className="sthira-button-secondary cursor-pointer"
            >
              Refresh project
            </button>
          </div>
        )}

        <section className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-2.5">
          <div className="flex flex-wrap items-center gap-2">
            <FolderKanban className="w-4 h-4 text-sthira-primary" />
            <h2 className="text-lg font-semibold font-editorial text-sthira-text">
              {project.name}
            </h2>
            <span className="rounded px-2 py-0.5 text-[11px] font-medium border border-sthira-border bg-sthira-canvas text-sthira-text-muted">
              {project.activePageCount} active{' '}
              {project.activePageCount === 1 ? 'page' : 'pages'}
            </span>
            <span className="rounded px-2 py-0.5 text-[11px] border border-sthira-border bg-sthira-canvas text-sthira-text-muted">
              v{project.version}
            </span>
            {isArchived && (
              <span className="rounded px-2 py-0.5 text-[11px] border border-sthira-border text-sthira-danger">
                Archived
              </span>
            )}
          </div>
          <p className="text-xs text-sthira-text leading-relaxed">
            {project.purpose || 'No project purpose added yet.'}
          </p>
          <p className="text-xs text-sthira-text-muted">
            Created by {project.createdByDisplayName} · Last updated by{' '}
            {project.updatedByDisplayName}
            {project.archivedByDisplayName
              ? ` · Archived by ${project.archivedByDisplayName}`
              : ''}
          </p>
        </section>

        {editingMeta && canManage && !isArchived && (
          <form
            onSubmit={(e) => void handleSaveMetadata(e)}
            aria-label="Edit project details"
            className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-3"
          >
            <h3 className="text-sm font-semibold">Edit project details</h3>
            <label className="block space-y-1">
              <span className="text-xs text-sthira-text-muted">Project name</span>
              <input
                aria-label="Edit project name"
                type="text"
                maxLength={120}
                required
                disabled={busy}
                value={editName}
                onChange={(e) => setEditName(e.target.value)}
                className="w-full min-h-[40px] rounded-lg border border-sthira-border bg-sthira-canvas px-3 text-sm text-sthira-text focus:outline-none"
              />
            </label>
            <label className="block space-y-1">
              <span className="text-xs text-sthira-text-muted">
                Project purpose (optional)
              </span>
              <textarea
                aria-label="Edit project purpose"
                rows={2}
                maxLength={500}
                disabled={busy}
                value={editPurpose}
                onChange={(e) => setEditPurpose(e.target.value)}
                className="w-full rounded-lg border border-sthira-border bg-sthira-canvas p-2.5 text-xs text-sthira-text focus:outline-none"
              />
            </label>
            <div className="flex flex-wrap gap-2">
              <button
                type="submit"
                disabled={busy || !editName.trim()}
                className="sthira-button-primary cursor-pointer"
              >
                Save project details
              </button>
              <button
                type="button"
                disabled={busy}
                onClick={() => setEditingMeta(false)}
                className="sthira-button-secondary cursor-pointer"
              >
                Cancel
              </button>
            </div>
          </form>
        )}

        {pickerOpen && canAuthor && !isArchived && (
          <section
            aria-label="Add existing page picker"
            className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-3"
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="space-y-0.5">
                <h3 className="text-sm font-semibold">
                  Add existing page to “{project.name}”
                </h3>
                <p className="text-xs text-sthira-text-muted">
                  Workspace: {workspaceName} · Active pages not yet in this project
                </p>
              </div>
              <button
                type="button"
                onClick={() => setPickerOpen(false)}
                aria-label="Close page picker"
                className="sthira-button-secondary inline-flex items-center gap-1 cursor-pointer"
              >
                <X className="w-3.5 h-3.5" />
                <span>Done</span>
              </button>
            </div>

            <form
              onSubmit={(e) => {
                e.preventDefault();
                void loadPickerPages(project.projectId, pickerQuery, null);
              }}
              className="flex flex-wrap items-center gap-2"
            >
              <input
                aria-label="Search workspace pages"
                type="search"
                value={pickerQuery}
                onChange={(e) => {
                  const next = e.target.value;
                  setPickerQuery(next);
                  void loadPickerPages(project.projectId, next, null);
                }}
                placeholder="Search workspace pages by title…"
                className="flex-1 min-w-[200px] min-h-[38px] rounded-lg border border-sthira-border bg-sthira-canvas px-3 text-xs text-sthira-text focus:outline-none"
              />
              <button
                type="submit"
                disabled={pickerLoading}
                className="sthira-button-secondary inline-flex items-center gap-1 cursor-pointer"
              >
                <Search className="w-3.5 h-3.5" />
                <span>Search</span>
              </button>
            </form>

            {pickerError && (
              <p
                role="alert"
                className="rounded-xl border border-sthira-border bg-sthira-surface-muted px-3.5 py-2 text-xs text-sthira-danger"
              >
                {pickerError}
              </p>
            )}

            {pickerLoading && pickerPages.length === 0 ? (
              <p role="status" className="text-xs text-sthira-text-muted py-2">
                Loading available workspace pages…
              </p>
            ) : pickerPages.length === 0 ? (
              <p className="text-xs text-sthira-text-muted py-2">
                No matching active workspace pages remain to attach.
              </p>
            ) : (
              <ul className="space-y-2" aria-label="Available workspace pages">
                {pickerPages.map((pg) => (
                  <li
                    key={pg.pageId}
                    data-page-id={pg.pageId}
                    className="rounded-xl border border-sthira-border bg-sthira-canvas p-3 flex flex-wrap items-center justify-between gap-2"
                  >
                    <div className="min-w-0 flex-1 space-y-0.5">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="font-medium text-xs text-sthira-text truncate">
                          {pg.title}
                        </span>
                        <span className="rounded px-1.5 py-0.5 text-[11px] border border-sthira-border bg-sthira-surface text-sthira-text-muted">
                          Rev {pg.currentRevisionNumber}
                        </span>
                      </div>
                      <p className="text-[11px] text-sthira-text-muted">
                        Published by {pg.publisherDisplayName} · Last edited by{' '}
                        {pg.lastEditorDisplayName}
                      </p>
                    </div>
                    <button
                      type="button"
                      disabled={busy}
                      aria-label={`Attach ${pg.title}`}
                      onClick={() => void handleAttachPage(pg.pageId)}
                      className="sthira-button-primary inline-flex items-center gap-1 cursor-pointer"
                    >
                      <Plus className="w-3.5 h-3.5" />
                      <span>Attach</span>
                    </button>
                  </li>
                ))}
              </ul>
            )}

            {pickerNextCursor && (
              <button
                type="button"
                disabled={pickerLoading}
                onClick={() =>
                  void loadPickerPages(project.projectId, pickerQuery, pickerNextCursor)
                }
                className="sthira-button-secondary cursor-pointer"
              >
                Load more available pages
              </button>
            )}
          </section>
        )}

        <section className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-3">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <h3 className="text-sm font-semibold flex items-center gap-1.5">
              <FileText className="w-4 h-4 text-sthira-primary" />
              <span>Project pages ({project.activePageCount})</span>
            </h3>
            {canAuthor && !isArchived && (
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  const nextOpen = !pickerOpen;
                  setPickerOpen(nextOpen);
                  if (nextOpen) {
                    setPickerQuery('');
                    void loadPickerPages(project.projectId, '', null);
                  }
                }}
                className="sthira-button-primary inline-flex items-center gap-1.5 cursor-pointer"
              >
                <Plus className="w-3.5 h-3.5" />
                <span>Add existing page</span>
              </button>
            )}
          </div>

          <form
            onSubmit={(e) => {
              e.preventDefault();
              void openProjectById(project.projectId, projectPageQuery, null);
            }}
            className="flex flex-wrap items-center gap-2"
          >
            <input
              aria-label="Filter pages in project"
              type="search"
              value={projectPageQuery}
              onChange={(e) => {
                const next = e.target.value;
                setProjectPageQuery(next);
                void openProjectById(project.projectId, next, null);
              }}
              placeholder="Search pages in this project…"
              className="flex-1 min-w-[200px] min-h-[38px] rounded-lg border border-sthira-border bg-sthira-canvas px-3 text-xs text-sthira-text focus:outline-none"
            />
            {projectPageQuery && (
              <button
                type="button"
                onClick={() => {
                  setProjectPageQuery('');
                  void openProjectById(project.projectId, '', null);
                }}
                className="sthira-button-secondary cursor-pointer"
              >
                Clear filter
              </button>
            )}
          </form>

          {pages.length === 0 ? (
            <p className="text-xs text-sthira-text-muted py-3">
              {projectPageQuery
                ? 'No active pages in this project match your search.'
                : isArchived
                  ? 'This archived project has no active shared pages.'
                  : 'No shared pages in this project yet. Use Add existing page to attach one.'}
            </p>
          ) : (
            <ul className="space-y-2" aria-label="Project pages list">
              {pages.map((item) => (
                <li
                  key={item.membershipId}
                  data-page-id={item.page.pageId}
                  data-membership-id={item.membershipId}
                  className="rounded-xl border border-sthira-border bg-sthira-canvas p-3.5 flex flex-wrap items-center justify-between gap-3"
                >
                  <div className="min-w-0 flex-1 space-y-0.5">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-medium text-sm text-sthira-text truncate">
                        {item.page.title}
                      </span>
                      <span className="rounded px-1.5 py-0.5 text-[11px] border border-sthira-border bg-sthira-surface text-sthira-text-muted">
                        Rev {item.page.currentRevisionNumber}
                      </span>
                    </div>
                    <p className="text-xs text-sthira-text-muted">
                      Added by {item.attachedByDisplayName} · Last edited by{' '}
                      {item.page.lastEditorDisplayName} · {item.page.unresolvedThreadCount} open
                      threads · {item.page.pendingProposalCount} pending proposals
                    </p>
                  </div>
                  <div className="flex flex-wrap items-center gap-2">
                    <button
                      type="button"
                      aria-label={`Open ${item.page.title}`}
                      onClick={() =>
                        setOpenedPage({ pageId: item.page.pageId, proposalId: null })
                      }
                      className="sthira-button-secondary cursor-pointer"
                    >
                      Open
                    </button>
                    {canAuthor && !isArchived && (
                      <button
                        type="button"
                        disabled={busy}
                        aria-label={`Remove ${item.page.title} from project`}
                        onClick={() =>
                          void handleDetachPage(
                            item.page.pageId,
                            item.membershipId,
                            item.page.title,
                          )
                        }
                        className="sthira-button-secondary inline-flex items-center gap-1 cursor-pointer"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                        <span>Remove from project</span>
                      </button>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          )}

          {pagesNextCursor && (
            <button
              type="button"
              disabled={busy}
              onClick={() =>
                void openProjectById(
                  project.projectId,
                  projectPageQuery,
                  pagesNextCursor,
                )
              }
              className="sthira-button-secondary cursor-pointer"
            >
              Load more project pages
            </button>
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
            <button
              type="button"
              onClick={() => {
                setPanelError('');
                setCreateOpen((prev) => !prev);
              }}
              className="sthira-button-primary inline-flex items-center gap-1.5 cursor-pointer"
            >
              <FolderPlus className="w-3.5 h-3.5" />
              <span>New project</span>
            </button>
          )}
          <button
            type="button"
            disabled={loadingList}
            onClick={() => void loadProjectsList(includeArchived, directoryQuery, null)}
            className="sthira-button-secondary inline-flex items-center gap-1.5 cursor-pointer"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${loadingList ? 'animate-spin' : ''}`} />
            <span>Refresh projects</span>
          </button>
        </div>

        <label className="inline-flex items-center gap-2 text-xs text-sthira-text-muted cursor-pointer">
          <input
            type="checkbox"
            checked={includeArchived}
            onChange={(e) => {
              const checked = e.target.checked;
              setIncludeArchived(checked);
              void loadProjectsList(checked, directoryQuery, null);
            }}
          />
          <span>Show archived projects</span>
        </label>
      </div>

      {panelNotice && (
        <p
          role="status"
          className="rounded-xl border border-sthira-border bg-sthira-surface px-4 py-2.5 text-xs text-sthira-text"
        >
          {panelNotice}
        </p>
      )}
      {panelError && (
        <p
          role="alert"
          className="rounded-xl border border-sthira-border bg-sthira-surface-muted px-4 py-2.5 text-xs text-sthira-danger"
        >
          {panelError}
        </p>
      )}

      {createOpen && canAuthor && (
        <form
          onSubmit={(e) => void handleCreateProject(e)}
          aria-label="New project"
          className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-3"
        >
          <h3 className="text-sm font-semibold">Create shared project</h3>
          <label className="block space-y-1">
            <span className="text-xs text-sthira-text-muted">Project name</span>
            <input
              aria-label="Project name"
              type="text"
              maxLength={120}
              required
              disabled={busy}
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="e.g. Launch Planning"
              className="w-full min-h-[40px] rounded-lg border border-sthira-border bg-sthira-canvas px-3 text-sm text-sthira-text focus:outline-none"
            />
          </label>
          <label className="block space-y-1">
            <span className="text-xs text-sthira-text-muted">
              Project purpose (optional)
            </span>
            <textarea
              aria-label="Project purpose (optional)"
              rows={2}
              maxLength={500}
              disabled={busy}
              value={newPurpose}
              onChange={(e) => setNewPurpose(e.target.value)}
              placeholder="Describe what shared pages this project groups together"
              className="w-full rounded-lg border border-sthira-border bg-sthira-canvas p-2.5 text-xs text-sthira-text focus:outline-none"
            />
          </label>
          <div className="flex flex-wrap gap-2">
            <button
              type="submit"
              disabled={busy || !newName.trim()}
              className="sthira-button-primary cursor-pointer"
            >
              Create project
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => setCreateOpen(false)}
              className="sthira-button-secondary cursor-pointer"
            >
              Cancel
            </button>
          </div>
        </form>
      )}

      <section className="rounded-2xl border border-sthira-border bg-sthira-surface p-5 space-y-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h2 className="text-sm font-semibold flex items-center gap-1.5">
            <FolderKanban className="w-4 h-4 text-sthira-primary" />
            <span>Projects ({projects.length})</span>
          </h2>
          <span className="text-xs text-sthira-text-muted">
            {canAuthor
              ? 'All workspace members can read shared projects'
              : 'Read-only access (Viewer)'}
          </span>
        </div>

        <form
          onSubmit={(e) => {
            e.preventDefault();
            void loadProjectsList(includeArchived, directoryQuery, null);
          }}
          className="flex flex-wrap items-center gap-2"
        >
          <input
            aria-label="Search projects"
            type="search"
            value={directoryQuery}
            onChange={(e) => {
              const next = e.target.value;
              setDirectoryQuery(next);
              void loadProjectsList(includeArchived, next, null);
            }}
            placeholder="Search projects by name or purpose…"
            className="flex-1 min-w-[200px] min-h-[38px] rounded-lg border border-sthira-border bg-sthira-canvas px-3 text-xs text-sthira-text focus:outline-none"
          />
          {directoryQuery && (
            <button
              type="button"
              onClick={() => {
                setDirectoryQuery('');
                void loadProjectsList(includeArchived, '', null);
              }}
              className="sthira-button-secondary cursor-pointer"
            >
              Clear search
            </button>
          )}
        </form>

        {loadingList && projects.length === 0 ? (
          <p role="status" className="text-xs text-sthira-text-muted py-3">
            Loading shared projects…
          </p>
        ) : projects.length === 0 ? (
          <p className="text-xs text-sthira-text-muted py-3">
            {directoryQuery
              ? 'No projects match your search.'
              : includeArchived
                ? 'No shared or archived projects in this workspace yet.'
                : 'No shared projects yet. Create a project to group related shared pages.'}
          </p>
        ) : (
          <ul className="space-y-2" aria-label="Shared projects list">
            {projects.map((prj) => (
              <li
                key={prj.projectId}
                data-project-id={prj.projectId}
                className="rounded-xl border border-sthira-border bg-sthira-canvas p-3.5 flex flex-wrap items-center justify-between gap-3"
              >
                <div className="min-w-0 flex-1 space-y-0.5">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-medium text-sm text-sthira-text truncate">
                      {prj.name}
                    </span>
                    <span className="rounded px-1.5 py-0.5 text-[11px] border border-sthira-border bg-sthira-surface text-sthira-text-muted">
                      {prj.activePageCount}{' '}
                      {prj.activePageCount === 1 ? 'page' : 'pages'}
                    </span>
                    {prj.archivedAtMs && (
                      <span className="rounded px-1.5 py-0.5 text-[11px] border border-sthira-border text-sthira-danger">
                        Archived
                      </span>
                    )}
                  </div>
                  {prj.purpose && (
                    <p className="text-xs text-sthira-text truncate">{prj.purpose}</p>
                  )}
                  <p className="text-xs text-sthira-text-muted">
                    Created by {prj.createdByDisplayName} · Updated by{' '}
                    {prj.updatedByDisplayName}
                  </p>
                </div>
                <button
                  type="button"
                  aria-label={`Open ${prj.name}`}
                  onClick={() => {
                    setProjectPageQuery('');
                    setOpenedPage(null);
                    setEditingMeta(false);
                    setPickerOpen(false);
                    void openProjectById(prj.projectId, '', null);
                  }}
                  className="sthira-button-secondary cursor-pointer"
                >
                  Open
                </button>
              </li>
            ))}
          </ul>
        )}

        {projectsNextCursor && (
          <button
            type="button"
            disabled={loadingList}
            onClick={() =>
              void loadProjectsList(
                includeArchived,
                directoryQuery,
                projectsNextCursor,
              )
            }
            className="sthira-button-secondary cursor-pointer"
          >
            Load more projects
          </button>
        )}
      </section>
    </div>
  );
};
