import { AgentTasksProvider } from './context/AgentTasksContext';
import { AgentTaskCenter } from './components/AgentTaskCenter';
import { WorkspaceGuide, hasSeenPageTour, hasSeenWorkspaceWelcome, rememberPageTour, rememberWorkspaceWelcome } from './components/WorkspaceGuide';
import { pageTours, type PageTourId } from './components/workspaceGuideContent';
import { OPEN_HELP_ARTICLE } from './components/HelpTopicButton';
import { isHelpArticleId, type HelpArticleId } from './components/workspaceHelpArticles';
import { OPEN_AGENT_TASKS } from './api/workspaceAgent';
import { DocumentTabs } from './components/DocumentTabs';
import { useDocumentTabs, openDocumentTab, closeDocumentTab, type DocumentTabsState, type DocumentTab } from './hooks/useDocumentTabs';
import { useWorkflowSettings } from './hooks/useWorkflowSettings';
import type { SettingsSection } from './components/ModelSettingsModal';
import { getDocumentConversation } from './api/conversation';
import { getSynthesisTemplate } from './api/synthesisTemplates';
import React, { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { HomeView } from './components/HomeView';
import { FocusView } from './components/FocusView';
import { CanvasView } from './components/CanvasView';
import { LeftSidebar } from './components/LeftSidebar';
import { SearchModal } from './components/SearchModal';
import { ModelSettingsModal } from './components/ModelSettingsModal';
import { AccountModal, type GoogleSession } from './components/AccountModal';
import { CreateWorkspaceModal } from './components/CreateWorkspaceModal';
import { JoinWorkspaceModal } from './components/JoinWorkspaceModal';
import { TeamWorkspaceView, type TeamWorkspaceTab } from './components/TeamWorkspaceView';
import {
  getActiveWorkspaceId,
  listWorkspaces,
  setActiveWorkspaceId,
  type OwnershipTransferView,
  type WorkspaceListResponse,
  type WorkspaceSummary,
} from './api/workspaces';
import { InteractiveFlowModal, type FlowMode } from './components/InteractiveFlowModal';
import { ObjectCollectionContainer } from './components/objects/ObjectCollectionContainer';
import { ObjectTypeBuilderModal } from './components/objects/ObjectTypeBuilderModal';
import { ObjectTypeManagerModal } from './components/objects/ObjectTypeManagerModal';
import { AgentView } from './views/AgentView';
import { MemoryView } from './views/MemoryView';
import { GraphView } from './views/GraphView';
import { WorkspaceIndexView } from './views/WorkspaceIndexView';
import { ProjectOverviewView, DEFAULT_PROJECT_BROWSE } from './views/ProjectOverviewView';
import { ExplorePlanView } from './views/ExplorePlanView';
import {
  getHomeState,
  promoteToProject,
  listObjectTypes,
  isTauri,
  createResearchPage,
  createPage,
  runResearchIntoPage,
  saveArtifact,
  saveGeneratedArtifact,
  streamAdaptiveRun,
} from './api/ipc';
import {
  type HomeStateView,
  type ObjectType,
  type WorkspaceIndexTab,
  type NoteScopeFilter,
  type ExplorePlanTab,
  type DocumentRunState,
  type WebSearchMode,
  type WebResearchResult,
} from './types/sthira';

const WorkspaceApp: React.FC = () => {
  const [taskCenterOpen,setTaskCenterOpen]=useState(false);
  const [agentRunToOpen,setAgentRunToOpen]=useState<string|null>(null);
  const workflows = useWorkflowSettings();
  const [settingsSection, setSettingsSection] = useState<SettingsSection>('models');
  const [settingsTabRequest, setSettingsTabRequest] = useState(0);
  useEffect(() => {
    const open = (event: Event) => {
      setSettingsSection((event as CustomEvent<SettingsSection>).detail || 'workflows');
      setSettingsTabRequest(request => request + 1);
      setIsSettingsOpen(true);
    };
    window.addEventListener('sthira:open-settings', open);
    return () => window.removeEventListener('sthira:open-settings', open);
  }, []);
  useEffect(() => {
    const onVaultReset = () => {
      setTaskCenterOpen(false);setAgentRunToOpen(null);
      navigating.current = false;
      beforeLeaveRef.current = null;
      newTabIntent.current = false;
      searchNewTab.current = false;
      projectBrowseById.current = {};
      setProjectBrowse(DEFAULT_PROJECT_BROWSE);
      setOverviewProjectId(null);
      setProjectReturnId(null);
      setActiveProjectId(null);
      setActiveArtifactId(null);
      setActiveCanvasId(null);
      setActiveObjectTypeId(null);
      setDocumentRun(null);
      setRealtimeTitleUpdate(null);
      setCurrentView('home');
      void loadState();
    };
    window.addEventListener('sthira:vault-reset', onVaultReset);
    return () => window.removeEventListener('sthira:vault-reset', onVaultReset);
  }, []);
  const [currentView, setCurrentView] = useState<
    | 'home'
    | 'focus'
    | 'project'
    | 'canvas'
    | 'objects'
    | 'graph'
    | 'memory'
    | 'agent'
    | 'workspace_index'
    | 'scheduled_topics'
    | 'explore_plan'
  >(() => {
    if (typeof window !== 'undefined') {
      const params = new URLSearchParams(window.location.search);
      const v = params.get('view');
      if (v === 'workspace_index' || v === 'all_files' || v === 'all-files') return 'workspace_index';
      if (v === 'focus') return 'focus';
      if (v === 'project' && params.get('projectId')) return 'project';
      if (v === 'graph') return 'graph';
      if (v === 'agent') return 'agent';
      if (v === 'memory') return 'memory';
      if (v === 'objects') return 'objects';
      if (v === 'explore_plan') return 'explore_plan';
    }
    return 'home';
  });
  const [workspaceIndexTab, setWorkspaceIndexTab] = useState<WorkspaceIndexTab>(() => {
    if (typeof window !== 'undefined') {
      const params = new URLSearchParams(window.location.search);
      const tab = params.get('tab') as WorkspaceIndexTab;
      if (tab === 'all' || tab === 'projects' || tab === 'notes' || tab === 'objects') return tab;
    }
    return 'all';
  });
  const [workspaceNoteScope, setWorkspaceNoteScope] = useState<NoteScopeFilter>('all');
  const [explorePlanTab, setExplorePlanTab] = useState<ExplorePlanTab>('research');
  const [activeProjectId, setActiveProjectId] = useState<string | null>(() => new URLSearchParams(window.location.search).get('view') === 'project' ? new URLSearchParams(window.location.search).get('projectId') : null);
  const [overviewProjectId, setOverviewProjectId] = useState<string | null>(() => new URLSearchParams(window.location.search).get('projectId'));
  const [projectReturnId, setProjectReturnId] = useState<string | null>(null);
  const [projectBrowse, setProjectBrowse] = useState(DEFAULT_PROJECT_BROWSE);
  const projectBrowseById = useRef<Record<string, typeof DEFAULT_PROJECT_BROWSE>>({});
  const [activeArtifactId, setActiveArtifactId] = useState<string | null>(() => new URLSearchParams(window.location.search).get('artifactId'));
  const [activeCanvasId, setActiveCanvasId] = useState<string | null>(null);
  const [activeObjectTypeId, setActiveObjectTypeId] = useState<string | null>(null);
  /**
   * Phase 20: live streaming document run, owned here so `FocusView` (the full-width
   * center workspace) can render both the progress banner and the live token stream.
   * There is no right-hand drawer any more — this is the only place runs surface.
   */
  const [documentRun, setDocumentRun] = useState<DocumentRunState | null>(null);
  const [isLeftSidebarCollapsed, setIsLeftSidebarCollapsed] = useState(false);
  const [treeRefreshTrigger, setTreeRefreshTrigger] = useState(0);
  const [realtimeTitleUpdate, setRealtimeTitleUpdate] = useState<{ id: string; title: string } | null>(null);

  const activeRouteRef = useRef({artifactId:activeArtifactId,view:currentView});
  activeRouteRef.current={artifactId:activeArtifactId,view:currentView};
  const beforeLeaveRef = useRef<(() => Promise<boolean>) | null>(null);
  const navigating = useRef(false);
  const newTabIntent = useRef(false);
  const searchNewTab = useRef(false);
  const canvasReturnRef = useRef<{
    artifactId: string; canvasId: string; projectId?: string; returnProjectId: string | null;
  } | null>(null);
  const registerBeforeLeave = useCallback((guard: (() => Promise<boolean>) | null) => { beforeLeaveRef.current = guard; }, []);
  const applyTab = (tab: DocumentTab) => {
    setActiveArtifactId(tab.id); setActiveProjectId(tab.projectId); setProjectReturnId(tab.returnProjectId); setCurrentView('focus');
  };
  const documentTabs = useDocumentTabs((saved: DocumentTabsState) => {
    if (window.location.search) return;
    const tab = saved.tabs.find(item => item.id === saved.activeId);
    if (saved.resume && tab) applyTab(tab);
  });
  const navigate = async (action: () => void) => {
    if (navigating.current) return false;
    navigating.current = true;
    try {
      if (beforeLeaveRef.current && !(await beforeLeaveRef.current())) return false;
      action(); return true;
    } finally { navigating.current = false; }
  };
  const selectTab = (id: string) => {
    const tab = documentTabs.stateRef.current.tabs.find(item => item.id === id);
    if (!tab) return;
    void navigate(() => { documentTabs.update(previous => ({...previous,activeId:id,resume:true})); applyTab(tab); });
  };
  const closeTab = (id: string) => {
    const close = () => {
      const previous = documentTabs.stateRef.current;
      const next = closeDocumentTab(previous,id);
      documentTabs.update(next);
      if (currentView === 'focus' && previous.activeId === id) {
        const tab = next.tabs.find(item => item.id === next.activeId);
        if (tab) applyTab(tab); else {setCurrentView('home');setActiveArtifactId(null);setActiveProjectId(null);}
      }
    };
    if (currentView === 'focus' && documentTabs.stateRef.current.activeId === id) void navigate(close); else close();
  };
  const openAnotherDocument = () => { searchNewTab.current = true; setIsSearchOpen(true); };
  useEffect(() => {
    if (currentView !== 'focus' && documentTabs.stateRef.current.resume) documentTabs.update(previous => ({...previous,resume:false}));
  }, [currentView]);
  // Modals state
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [guideEntry, setGuideEntry] = useState<'welcome' | 'help' | 'tour' | null>(null);
  const [activePageTour, setActivePageTour] = useState<PageTourId>('home');
  const [helpArticle, setHelpArticle] = useState<HelpArticleId>();
  const welcomeOffered = useRef(false);
  const pageToursOffered = useRef(new Set<PageTourId>());
  useEffect(() => {
    const openArticle = (event: Event) => {
      const id = (event as CustomEvent<string>).detail;
      if (typeof id !== 'string' || !isHelpArticleId(id)) return;
      setHelpArticle(id);
      setGuideEntry('help');
    };
    window.addEventListener(OPEN_HELP_ARTICLE, openArticle);
    return () => window.removeEventListener(OPEN_HELP_ARTICLE, openArticle);
  }, []);
  const [isSettingsOpen, setIsSettingsOpen] = useState(() => {
    if (typeof window !== 'undefined') {
      const params = new URLSearchParams(window.location.search);
      return params.get('settings') === '1' || params.get('modal') === 'settings';
    }
    return false;
  });
  const [isAccountOpen, setIsAccountOpen] = useState(false);
  const [isCreateWorkspaceOpen, setIsCreateWorkspaceOpen] = useState(false);
  const [isJoinWorkspaceOpen, setIsJoinWorkspaceOpen] = useState(false);
  const [googleSession, setGoogleSession] = useState<GoogleSession | null>(null);
  const [workspaces, setWorkspaces] = useState<WorkspaceSummary[]>([]);
  const [incomingTransfers, setIncomingTransfers] = useState<OwnershipTransferView[]>([]);
  const [workspacesLoading, setWorkspacesLoading] = useState(false);
  const [activeWorkspace, setActiveWorkspace] = useState<WorkspaceSummary | null>(null);
  const [teamWorkspaceTab, setTeamWorkspaceTab] = useState<TeamWorkspaceTab>('overview');
  const [workspaceNotice, setWorkspaceNotice] = useState('');
  const [isObjectTypeBuilderOpen, setIsObjectTypeBuilderOpen] = useState(false);
  const [isObjectTypeManagerOpen, setIsObjectTypeManagerOpen] = useState(false);
  const [activeFlow, setActiveFlow] = useState<{ mode: FlowMode; initialPrompt?: string } | null>(null);

  useEffect(() => {
    const keyboard = (event: KeyboardEvent) => {
      if (isSettingsOpen || isSearchOpen || isAccountOpen || isCreateWorkspaceOpen || isJoinWorkspaceOpen || activeFlow || isObjectTypeBuilderOpen || isObjectTypeManagerOpen || document.querySelector('[role="dialog"][aria-modal="true"]')) return;
      if ((event.ctrlKey || event.metaKey) && event.key === 'Tab' && documentTabs.stateRef.current.tabs.length > 1) {
        event.preventDefault();
        const {tabs,activeId} = documentTabs.stateRef.current;
        const index = tabs.findIndex(tab => tab.id === activeId);
        selectTab(tabs[(index + (event.shiftKey ? -1 : 1) + tabs.length) % tabs.length].id);
      }
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'w' && currentView === 'focus' && documentTabs.stateRef.current.activeId) {
        event.preventDefault(); closeTab(documentTabs.stateRef.current.activeId);
      }
    };
    window.addEventListener('keydown', keyboard);
    return () => window.removeEventListener('keydown',keyboard);
  });

  const [objectTypes, setObjectTypes] = useState<ObjectType[]>([]);
  const [homeLoadStatus, setHomeLoadStatus] = useState<'loading' | 'ready' | 'error'>('loading');
  const [homeState, setHomeState] = useState<HomeStateView>({
    projects: [],
    recent_items: [],
    loose_captures: [],
  });
  const tourPage: PageTourId = isSettingsOpen ? 'settings' : activeWorkspace ? 'team'
    : currentView === 'scheduled_topics' ? 'explore_plan' : currentView;
  const openPageTour = useCallback((page: PageTourId) => {
    pageToursOffered.current.add(page);
    rememberPageTour(page);
    setActivePageTour(page);
    setHelpArticle(undefined);
    setGuideEntry('tour');
  }, []);

  useEffect(() => {
    if (!isTauri || homeLoadStatus === 'loading' || guideEntry || isSearchOpen || isAccountOpen
      || isCreateWorkspaceOpen || isJoinWorkspaceOpen || isObjectTypeBuilderOpen
      || isObjectTypeManagerOpen || activeFlow || taskCenterOpen) return;
    const dialogIsOpen = Array.from(document.querySelectorAll('[role="dialog"][aria-modal="true"]')).some(
      dialog => !isSettingsOpen || !dialog.closest('[data-testid="model-settings-modal"]'),
    );
    if (dialogIsOpen) return;
    if (!welcomeOffered.current && !hasSeenWorkspaceWelcome()) {
      if (isSettingsOpen) return;
      welcomeOffered.current = true;
      rememberWorkspaceWelcome();
      if (tourPage === 'home') {
        pageToursOffered.current.add('home');
        rememberPageTour('home');
      }
      setGuideEntry('welcome');
    } else if (!pageToursOffered.current.has(tourPage) && !hasSeenPageTour(tourPage)) {
      openPageTour(tourPage);
    }
  }, [activeFlow, guideEntry, homeLoadStatus, isAccountOpen, isCreateWorkspaceOpen, isJoinWorkspaceOpen,
    isObjectTypeBuilderOpen, isObjectTypeManagerOpen, isSearchOpen, isSettingsOpen, openPageTour,
    taskCenterOpen, tourPage]);

  const activeWorkspaceRef = useRef<WorkspaceSummary | null>(null);
  const workspaceSelectionRevision = useRef(0);
  const workspaceListRequest = useRef(0);
  const accountCheckRequest = useRef(0);

  const updateActiveWorkspace = useCallback((workspace: WorkspaceSummary | null) => {
    if (workspace?.workspaceId !== activeWorkspaceRef.current?.workspaceId) {
      workspaceSelectionRevision.current += 1;
    }
    activeWorkspaceRef.current = workspace;
    setActiveWorkspace(workspace);
  }, []);

  const applyWorkspaceList = useCallback((list: WorkspaceListResponse, targetId: string | null) => {
    setWorkspaces(list.workspaces);
    setIncomingTransfers(list.incomingTransfers);
    if (targetId) {
      const match = list.workspaces.find((w) => w.workspaceId === targetId);
      if (match) {
        updateActiveWorkspace(match);
        setIsSearchOpen(false);
      } else {
        updateActiveWorkspace(null);
        void setActiveWorkspaceId(null);
        setWorkspaceNotice('You no longer have access to that workspace. Returned to Personal.');
      }
    }
  }, [updateActiveWorkspace]);

  const refreshWorkspaces = useCallback(async (restoreSavedActive = false) => {
    if (!isTauri || typeof invoke !== 'function') return;
    const request = ++workspaceListRequest.current;
    const selectionRevision = workspaceSelectionRevision.current;
    setWorkspacesLoading(true);
    try {
      const savedId = restoreSavedActive
        ? await getActiveWorkspaceId().catch(() => null)
        : activeWorkspaceRef.current?.workspaceId ?? null;
      const res = await listWorkspaces();
      if (request !== workspaceListRequest.current) return;
      if (res.outcome === 'ok') {
        applyWorkspaceList(
          res.data,
          selectionRevision === workspaceSelectionRevision.current ? savedId : null,
        );
      } else if (res.outcome === 'sessionUpdated') {
        setGoogleSession(res.session);
        if (res.session.status === 'signedOut' || res.session.status === 'unavailable') {
          setWorkspaces([]);
          setIncomingTransfers([]);
          updateActiveWorkspace(null);
          void setActiveWorkspaceId(null);
        }
      }
    } catch {
      // Offline / unreachable: Personal remains usable offline without clearing saved state.
    } finally {
      if (request === workspaceListRequest.current) setWorkspacesLoading(false);
    }
  }, [applyWorkspaceList, updateActiveWorkspace]);

  const googleSessionRef = useRef<GoogleSession | null>(null);
  googleSessionRef.current = googleSession;

  const handleSessionUpdated = useCallback((nextSession: GoogleSession) => {
    accountCheckRequest.current += 1;
    const prevUid =
      googleSessionRef.current?.status === 'signedIn' ? googleSessionRef.current.uid : undefined;
    const nextUid = nextSession.status === 'signedIn' ? nextSession.uid : undefined;
    setGoogleSession(nextSession);
    if (prevUid !== nextUid) window.dispatchEvent(new Event("sthira:account-context-changed"));
    if (nextSession.status === 'signedIn') {
      if (prevUid && nextUid && prevUid !== nextUid) {
        workspaceSelectionRevision.current += 1;
        setWorkspaces([]);
        setIncomingTransfers([]);
        updateActiveWorkspace(null);
        setTeamWorkspaceTab('overview');
        void setActiveWorkspaceId(null);
      }
      void refreshWorkspaces(false);
    } else if (nextSession.status === 'signedOut' || nextSession.status === 'unavailable') {
      workspaceListRequest.current += 1;
      setWorkspacesLoading(false);
      setWorkspaces([]);
      setIncomingTransfers([]);
      if (activeWorkspaceRef.current) {
        updateActiveWorkspace(null);
        void setActiveWorkspaceId(null);
        setWorkspaceNotice('Signed out. Returned to Personal workspace.');
      }
    }
  }, [refreshWorkspaces, updateActiveWorkspace]);

  const selectPersonalWorkspace = useCallback((noticeText?: string) => {
    void navigate(() => {
      workspaceSelectionRevision.current += 1;
      updateActiveWorkspace(null);
      setTeamWorkspaceTab('overview');
      if (noticeText) setWorkspaceNotice(noticeText);
      else setWorkspaceNotice('');
      void setActiveWorkspaceId(null);
    });
  }, [updateActiveWorkspace]);

  const selectTeamWorkspace = useCallback((ws: WorkspaceSummary, tab: TeamWorkspaceTab = 'overview') => {
    void navigate(() => {
      setWorkspaceNotice('');
      updateActiveWorkspace(ws);
      setIsSearchOpen(false);
      setTeamWorkspaceTab(tab);
      void setActiveWorkspaceId(ws.workspaceId);
    });
  }, [updateActiveWorkspace]);

  useEffect(() => {
    const checkAccount = () => {
      if (!isTauri || typeof invoke !== 'function') return;
      const request = ++accountCheckRequest.current;
      void Promise.resolve(invoke<GoogleSession>('cmd_get_google_session'))
        .then((status) => {
          if (!status || request !== accountCheckRequest.current) return;
          setGoogleSession(status);
          if (status.status === 'signedIn') {
            void refreshWorkspaces(true);
          } else if (status.status === 'signedOut' || status.status === 'unavailable') {
            workspaceListRequest.current += 1;
            setWorkspacesLoading(false);
            setWorkspaces([]);
            setIncomingTransfers([]);
            updateActiveWorkspace(null);
          }
        })
        .catch(() => {});
    };
    checkAccount();
    window.addEventListener('online', checkAccount);
    return () => {
      accountCheckRequest.current += 1;
      workspaceListRequest.current += 1;
      window.removeEventListener('online', checkAccount);
    };
  }, [refreshWorkspaces, updateActiveWorkspace]);

  const loadState = async () => {
    try {
      const [state, types] = await Promise.all([
        getHomeState(),
        listObjectTypes(),
      ]);
      setHomeState(state);
      setHomeLoadStatus('ready');
      setObjectTypes(types);
      setTreeRefreshTrigger((prev) => prev + 1);
    } catch (err) {
      console.error('Failed to load home state', err);
      setHomeLoadStatus('error');
    }
  };

  useEffect(() => {
    loadState();

    // Keydown shortcut: Cmd+K / Ctrl+K opens search unless another modal dialog owns focus
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        if (activeWorkspaceRef.current) return;
        const activeModal = document.querySelector('[role="dialog"][aria-modal="true"]');
        if (activeModal && activeModal.getAttribute('data-testid') !== 'search-workspace-dialog') {
          return;
        }
        e.preventDefault();
        searchNewTab.current = false;
        setIsSearchOpen((prev) => !prev);
      }
    };
    window.addEventListener('keydown', handleKeyDown);

    const handleBeforeUnload = () => {
      if (beforeLeaveRef.current) {
        void beforeLeaveRef.current();
      }
    };
    window.addEventListener('beforeunload', handleBeforeUnload);

    // Listen to live Tauri core events if running inside desktop app
    if (isTauri) {
      let cancelled = false;
      let unlistenFn: (() => void) | undefined;
      let unlistenCloseFn: (() => void) | undefined;
      import('@tauri-apps/api/event')
        .then(({ listen }) => {
          listen('sthira://core-event', (event) => {
            console.log('Received core event:', event);
            loadState();
          })
            .then((unlisten) => {
              if (cancelled) unlisten();
              else unlistenFn = unlisten;
            })
            .catch(() => {});
          listen('sthira:close-requested', () => {
            void (async () => {
              try {
                const ok = !beforeLeaveRef.current || (await beforeLeaveRef.current());
                if (ok) {
                  await Promise.resolve(invoke('cmd_confirm_window_close')).catch(() => {});
                }
              } catch {
                // Keep window open if draft flush fails before confirming close.
              }
            })();
          })
            .then((unlisten) => {
              if (cancelled) {
                unlisten();
                return;
              }
              unlistenCloseFn = unlisten;
              void Promise.resolve(invoke('cmd_register_close_guard', { ready: true })).catch(() => {});
            })
            .catch(() => {});
        })
        .catch(() => {});

      return () => {
        cancelled = true;
        window.removeEventListener('keydown', handleKeyDown);
        window.removeEventListener('beforeunload', handleBeforeUnload);
        void Promise.resolve(invoke('cmd_register_close_guard', { ready: false })).catch(() => {});
        if (unlistenFn) unlistenFn();
        if (unlistenCloseFn) unlistenCloseFn();
      };
    }

    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('beforeunload', handleBeforeUnload);
    };
  }, []);

  const handleOpenProject = (projectId: string) => {
    void navigate(() => {
    if (overviewProjectId !== projectId) setProjectBrowse(projectBrowseById.current[projectId] || DEFAULT_PROJECT_BROWSE);
    setOverviewProjectId(projectId);
    setProjectReturnId(null);
    setActiveProjectId(projectId);
    setActiveArtifactId(null);
    setActiveCanvasId(null);
    setCurrentView('project');
    });
  };

  const handleOpenArtifact = (artifactId: string, projectId?: string | null, inNewTab?: boolean, returnProjectId: string | null = null,
    returnCanvas?: { canvasId: string; projectId?: string; returnProjectId: string | null }) => {
    const newTab = inNewTab ?? (newTabIntent.current || searchNewTab.current);
    searchNewTab.current = false;
    const recent = homeState.recent_items.find(item => item.artifact_id === artifactId);
    const project = projectId !== undefined ? projectId : null;
    void navigate(() => {
      canvasReturnRef.current = returnCanvas ? { artifactId, ...returnCanvas } : null;
      const next = openDocumentTab(documentTabs.stateRef.current, {id:artifactId,title:recent?.title || 'Loading page…',projectId:project,returnProjectId,scrollTop:0},newTab);
      documentTabs.update(next);
      applyTab(next.tabs.find(tab => tab.id === artifactId)!);
    });
  };

  const handleOpenCanvas = (canvasId?: string, projectId?: string, returnProjectId: string | null = null) => {
    void navigate(() => {
    setProjectReturnId(returnProjectId);
    setActiveProjectId(projectId ?? null);
    setActiveCanvasId(canvasId || null);
    setCurrentView('canvas');
    });
  };

  const handleOpenObjectCollection = (typeId: string) => {
    void navigate(() => {
    setProjectReturnId(null);
    setActiveObjectTypeId(typeId);
    setCurrentView('objects');
    });
  };

  const handleOpenCreateObjectType = () => {
    setIsObjectTypeBuilderOpen(true);
  };

  const handleOpenObjectTypeManager = () => {
    setIsObjectTypeManagerOpen(true);
  };

  const handleTypeCreated = async (newType: ObjectType) => {
    await loadState();
    setActiveObjectTypeId(newType.id);
    setCurrentView('objects');
  };

  const handleNavigateWorkspaceIndex = (tab?: WorkspaceIndexTab, scope: NoteScopeFilter = 'all') => {
    void navigate(() => {
    setWorkspaceNoteScope(scope);
    setProjectReturnId(null);
    setWorkspaceIndexTab(tab || 'all');
    setCurrentView('workspace_index');
    });
  };

  const handleNavigateExplorePlan = (tab?: ExplorePlanTab) => {
    void navigate(() => {
    setProjectReturnId(null);
    setActiveProjectId(null);
    setActiveArtifactId(null);
    setActiveCanvasId(null);
    setActiveObjectTypeId(null);
    if (typeof tab === 'string') {
      setExplorePlanTab(tab);
    } else {
      setExplorePlanTab('research');
    }
    setCurrentView('explore_plan');
    });
  };

  const handleNavigateScheduledTopics = () => {
    handleNavigateExplorePlan('trackers');
  };

  /**
   * Phase 19/20: the single entry point for user-facing research.
   *
   * Creates a draft research page, **immediately switches the center workspace to
   * `FocusView`** with that page loaded, then runs the autonomous pipeline into it while
   * streaming real progress stages. Nothing is ever routed to a side drawer.
   */
  const handleStartResearch = async (
    query: string,
    mode: WebSearchMode = 'auto',
    projectId?: string | null,
    templateId?: string | null,
    modelId?: string | null
  ): Promise<WebResearchResult | null> => {
    const q = query.trim();
    if (!q) return null;
    if (workflows.loading || workflows.error || !workflows.settings.research.enabled) { setSettingsSection('workflows'); setIsSettingsOpen(true); return null; }
    modelId ||= workflows.settings.research.model_id;
    if (!workflows.settings.research.templates_enabled) templateId = null;
    if (templateId && templateId !== '__none__') {
      const template = await getSynthesisTemplate(templateId).catch(() => null);
      if (!template || template.archived_at !== null || template.target_mode !== 'research') {
        window.alert('The selected Research template is missing, archived, or incompatible. Choose another template or No template. Your inquiry is preserved.');
        return null;
      }
    }

    const targetProject = projectId !== undefined ? projectId : activeProjectId || null;
    let artifactId: string | null = null;

    try {
      artifactId = await createResearchPage(q, targetProject);
      await getDocumentConversation(artifactId, 'research', q, modelId);

      // 1. Center-page navigation happens immediately, before any slow work.
      handleOpenArtifact(artifactId,targetProject,false);

      setDocumentRun({
        artifactId,
        query: q,
        kind: 'research',
        mode,
        status: 'running',
        message: 'Creating research page…',
        streamedMarkdown: '',
        refreshToken: 0,
      });

      // 2. Stream the pipeline's real stages into the banner.
      const onStageCallback = (stage: any) => {
        setDocumentRun((prev) =>
          prev && prev.artifactId === artifactId ? { ...prev, message: stage.message } : prev
        );
      };
      const result = await runResearchIntoPage(artifactId, q, mode, onStageCallback, templateId, modelId);

      // 3. Mark complete and ask the editor to reload the now-populated page.
      const isFailedSearch = result.search_status === 'web_search_failed' || result.search_status === 'unsupported_model';
      const lastStage = result.stages && result.stages.length > 0 ? result.stages[result.stages.length - 1] : null;
      const lastStageMsg = lastStage?.message?.trim() || null;
      const isPartialCoverage = Boolean(
        lastStage?.stage === 'partial_search_failure' ||
          lastStageMsg?.toLowerCase().includes('partial search coverage')
      );

      setDocumentRun((prev) =>
        prev && prev.artifactId === artifactId
          ? {
              ...prev,
              status: isFailedSearch || isPartialCoverage ? 'warning' : 'done',
              mode: result.mode_used,
              search_status: result.search_status,
              external_sources_count: result.external_sources_count,
              message:
                lastStageMsg ||
                (isFailedSearch
                  ? '⚠️ Web search returned no results; synthesized using local context.'
                  : 'Research complete.'),
              refreshToken: prev.refreshToken + 1,
            }
          : prev
      );

      await loadState();
      setTreeRefreshTrigger((prev) => prev + 1);
      return result;
    } catch (err: any) {
      console.error('Research run failed', err);
      const message = err?.toString?.() || 'Research failed';
      setDocumentRun((prev) =>
        prev && artifactId && prev.artifactId === artifactId
          ? { ...prev, status: 'error', error: message, message, streamedMarkdown: '' }
          : prev
      );
      return null;
    }
  };

  /**
   * Phase 20: `Plan` mode. Creates a `Plan: <Goal>` draft, navigates to it immediately,
   * then streams LLM tokens straight into the page body — the live stream renders in the
   * center document canvas rather than a rail panel.
   */
  const handleStartPlan = async (
    goal: string,
    projectId?: string | null,
    templateId?: string | null,
    modelId?: string | null
  ): Promise<string | null> => {
    const g = goal.trim();
    if (!g) return null;
    if (workflows.loading || workflows.error || !workflows.settings.plan.enabled) { setSettingsSection('workflows'); setIsSettingsOpen(true); return null; }
    modelId ||= workflows.settings.plan.model_id;
    if (!workflows.settings.plan.templates_enabled) templateId = null;
    if (templateId && templateId !== '__none__') {
      const template = await getSynthesisTemplate(templateId).catch(() => null);
      if (!template || template.archived_at !== null || template.target_mode !== 'plan') {
        window.alert('The selected Plan template is missing, archived, or incompatible. Choose another template or No template. Your goal is preserved.');
        return null;
      }
    }

    const targetProject = projectId !== undefined ? projectId : activeProjectId || null;
    const title = `Plan: ${g.length > 60 ? `${g.slice(0, 60)}...` : g}`;
    let artifactId: string | null = null;

    try {
      const draft = await createPage(targetProject, title);
      artifactId = draft.id;
      await getDocumentConversation(artifactId, 'plan', g, modelId);

      // 1. Navigate to the center workspace before streaming begins.
      handleOpenArtifact(artifactId,targetProject,false);

      const draftBody = `# ${title}\n\n> **Status:** drafting — planning…\n\n## Goal\n${g}\n`;
      await saveArtifact(artifactId, draftBody, title, targetProject);

      setDocumentRun({
        artifactId,
        query: g,
        kind: 'plan',
        status: 'running',
        message: 'Planning…',
        streamedMarkdown: '',
        refreshToken: 0,
      });

      // 2. Stream tokens into the page as they arrive. The receipt captures the
      // dispatch-time template/model decision and is saved with the completed page.
      let accumulated = '';
      const generation = await streamAdaptiveRun(g, 'plan', modelId, (chunk) => {
        if (chunk.type !== 'text_token') return;
        accumulated += chunk.payload;
        const live = accumulated;
        setDocumentRun((prev) =>
          prev && prev.artifactId === artifactId
            ? { ...prev, streamedMarkdown: live, message: 'Drafting plan…' }
            : prev
        );
      }, templateId);

      // 3. Persist the finished plan and reload the page.
      if (!accumulated.trim()) throw new Error('The selected model returned no plan. Your goal remains saved as a draft.');
      const body = accumulated.trim().startsWith('#') ? accumulated.trim() : `# ${title}\n\n${accumulated.trim()}`;

      await saveGeneratedArtifact(artifactId, body, title, targetProject, generation);

      setDocumentRun((prev) =>
        prev && prev.artifactId === artifactId
          ? {
              ...prev,
              status: 'done',
              message: 'Plan complete.',
              streamedMarkdown: '',
              refreshToken: prev.refreshToken + 1,
            }
          : prev
      );

      await loadState();
      setTreeRefreshTrigger((prev) => prev + 1);
      return artifactId;
    } catch (err: any) {
      console.error('Plan run failed', err);
      const message = err?.toString?.() || 'Planning failed';
      setDocumentRun((prev) =>
        prev && artifactId && prev.artifactId === artifactId
          ? { ...prev, status: 'error', error: message, message, streamedMarkdown: '' }
          : prev
      );
      return null;
    }
  };

  const handlePromoteLooseCapture = async (artifactId: string) => {
    try {
      const newProjectId = await promoteToProject([artifactId], 'Promoted Exploration');
      await loadState();
      handleOpenProject(newProjectId);
    } catch (err) {
      console.error('Failed to promote loose capture', err);
    }
  };

  const openAgentRun = useCallback((id:string) => { void navigate(()=>{
    setCurrentView('agent');setAgentRunToOpen(id);setProjectReturnId(null);setActiveArtifactId(null);setActiveProjectId(null);setActiveObjectTypeId(null);setTaskCenterOpen(false);
  }); },[navigate]);
  useEffect(()=>{const open=()=>setTaskCenterOpen(true);const run=(event:Event)=>{const id=(event as CustomEvent<string>).detail;if(typeof id==='string')openAgentRun(id);};window.addEventListener(OPEN_AGENT_TASKS,open);window.addEventListener('sthira:open-agent-run',run);return()=>{window.removeEventListener(OPEN_AGENT_TASKS,open);window.removeEventListener('sthira:open-agent-run',run);};},[openAgentRun]);
  return (
    <div className="sthira-shell flex h-screen w-screen overflow-hidden" onClickCapture={event => { newTabIntent.current = event.ctrlKey || event.metaKey; }} onKeyDownCapture={event => { if(event.key === 'Enter') newTabIntent.current = event.ctrlKey || event.metaKey; }}>
      {/* Step 2: Left Hierarchy Sidebar */}
      <LeftSidebar
        activeProjectId={activeProjectId}
        activeArtifactId={activeArtifactId}
        activeCanvasId={activeCanvasId}
        activeObjectTypeId={activeObjectTypeId}
        currentView={currentView}
        onNavigateHome={() => { void navigate(() => {
          setProjectReturnId(null);
          setCurrentView('home');
          setActiveProjectId(null);
          setActiveArtifactId(null);
          setActiveObjectTypeId(null);
          loadState();
          });
        }}
        onNavigateGraph={() => { void navigate(() => {
          setProjectReturnId(null);
          setCurrentView('graph');
          setActiveArtifactId(null);
          setActiveObjectTypeId(null);
          });
        }}
        onNavigateAgent={() => { void navigate(() => { setCurrentView('agent'); setAgentRunToOpen(null); setProjectReturnId(null); setActiveArtifactId(null); setActiveProjectId(null); setActiveObjectTypeId(null); }); }}
        onNavigateMemory={() => { void navigate(() => { setCurrentView('memory'); setProjectReturnId(null); setActiveArtifactId(null); setActiveProjectId(null); setActiveObjectTypeId(null); }); }}
        onNavigateWorkspaceIndex={handleNavigateWorkspaceIndex}
        onNavigateExplorePlan={handleNavigateExplorePlan}
        onNavigateScheduledTopics={handleNavigateScheduledTopics}
        onOpenProject={handleOpenProject}
        onOpenArtifact={handleOpenArtifact}
        onOpenCanvas={handleOpenCanvas}
        onOpenObjectCollection={handleOpenObjectCollection}
        onOpenCreateObjectType={handleOpenCreateObjectType}
        onOpenObjectTypeManager={handleOpenObjectTypeManager}
        onOpenSearch={() => setIsSearchOpen(true)}
        onOpenSettings={() => setIsSettingsOpen(true)}
        onOpenAccount={() => setIsAccountOpen(true)}
        onOpenHelp={() => { setHelpArticle(undefined); setGuideEntry('help'); }}
        onOpenTour={() => openPageTour(tourPage)}
        isCollapsed={isLeftSidebarCollapsed}
        onToggleCollapse={() => setIsLeftSidebarCollapsed((prev) => !prev)}
        refreshTrigger={treeRefreshTrigger}
        realtimeTitleUpdate={realtimeTitleUpdate}
        onArtifactRenamed={(id, newTitle) => {
          documentTabs.update(previous => ({...previous,tabs:previous.tabs.map(tab => tab.id === id ? {...tab,title:newTitle} : tab)}));
          setRealtimeTitleUpdate({ id, title: newTitle });
          loadState();
        }}
        activeWorkspace={activeWorkspace}
        workspaces={workspaces}
        incomingTransfers={incomingTransfers}
        googleSession={googleSession}
        workspacesLoading={workspacesLoading}
        teamWorkspaceTab={teamWorkspaceTab}
        onSelectTeamTab={(tab) => { void navigate(() => setTeamWorkspaceTab(tab)); }}
        onSelectPersonal={() => selectPersonalWorkspace()}
        onSelectWorkspace={(ws) => selectTeamWorkspace(ws, 'overview')}
        onOpenCreateWorkspace={() => setIsCreateWorkspaceOpen(true)}
        onOpenJoinWorkspace={() => setIsJoinWorkspaceOpen(true)}
        onRefreshWorkspaces={() => void refreshWorkspaces(false)}
      />

      {/* Main Content Area — the full-width center workspace. There is deliberately no
          persistent right-hand drawer; every mode (research, plan, synthesis) streams
          into this container. */}
      <main
        data-testid="center-workspace"
        data-guide-target="current-screen"
        className={`sthira-canvas flex-1 h-full min-w-0 ${currentView === 'focus' || currentView === 'project' || currentView === 'graph' || currentView === 'canvas' || activeWorkspace ? 'overflow-hidden' : 'overflow-y-auto'} flex flex-col relative`}
      >
        {workspaceNotice && !activeWorkspace && (
          <div
            role="status"
            className="mx-6 mt-3 rounded-xl border border-sthira-border bg-sthira-surface px-4 py-2.5 text-xs text-sthira-text flex items-center justify-between gap-2"
          >
            <span>{workspaceNotice}</span>
            <button
              type="button"
              onClick={() => setWorkspaceNotice('')}
              className="text-sthira-text-muted hover:text-sthira-text underline cursor-pointer"
            >
              Dismiss
            </button>
          </div>
        )}
        {activeWorkspace ? (
          <TeamWorkspaceView
            key={activeWorkspace.workspaceId}
            workspace={activeWorkspace}
            initialTab={teamWorkspaceTab}
            currentUid={googleSession?.status === 'signedIn' ? (googleSession.uid ?? null) : null}
            onTabChange={setTeamWorkspaceTab}
            registerBeforeLeave={registerBeforeLeave}
            onSelectPersonal={(noticeText) => {
              if (activeWorkspaceRef.current?.workspaceId === activeWorkspace.workspaceId) {
                selectPersonalWorkspace(noticeText);
              }
            }}
            onWorkspaceUpdated={(updated) => {
              if (activeWorkspaceRef.current?.workspaceId !== updated.workspaceId) return;
              updateActiveWorkspace(updated);
              setWorkspaces((prev) =>
                prev.map((w) => (w.workspaceId === updated.workspaceId ? updated : w)),
              );
            }}
            onAccessLost={(message, list) => {
              if (activeWorkspaceRef.current?.workspaceId !== activeWorkspace.workspaceId) return;
              workspaceListRequest.current += 1;
              setWorkspacesLoading(false);
              if (list) {
                setWorkspaces(list.workspaces);
                setIncomingTransfers(list.incomingTransfers);
              } else {
                setWorkspaces((prev) =>
                  prev.filter((w) => w.workspaceId !== activeWorkspace.workspaceId),
                );
              }
              updateActiveWorkspace(null);
              void setActiveWorkspaceId(null);
              setWorkspaceNotice(message);
            }}
            onSessionChange={handleSessionUpdated}
            onRefreshList={() => refreshWorkspaces(false)}
          />
        ) : (
        <>
        <DocumentTabs tabs={documentTabs.state.tabs} activeId={currentView === 'focus' ? documentTabs.state.activeId : null} onSelect={selectTab} onClose={closeTab} onMove={(id,offset) => documentTabs.update(previous => {
          const tabs=[...previous.tabs]; const index=tabs.findIndex(tab=>tab.id===id);const target=index+offset;
          if(index<0||target<0||target>=tabs.length)return previous;
          [tabs[index],tabs[target]]=[tabs[target],tabs[index]];return {...previous,tabs};
        })}/>
        {currentView === 'home' && (
          <HomeView
            homeState={homeState}
            loadStatus={homeLoadStatus}
            onOpenProject={handleOpenProject}
            onOpenArtifact={handleOpenArtifact}
            onOpenCanvas={handleOpenCanvas}
            onPromoteLooseCapture={handlePromoteLooseCapture}
            onRefresh={loadState}
            onOpenSearch={() => setIsSearchOpen(true)}
            onOpenSettings={() => setIsSettingsOpen(true)}
            onOpenAccount={() => setIsAccountOpen(true)}
            onOpenFlow={(mode, initialPrompt) => setActiveFlow({ mode, initialPrompt })}
            onNavigateWorkspaceIndex={handleNavigateWorkspaceIndex}
            onStartResearch={handleStartResearch}
            onStartPlan={handleStartPlan}
          />
        )}

        {currentView === 'agent' && <AgentView initialRunId={agentRunToOpen} onOpenProject={handleOpenProject} onOpenArtifact={handleOpenArtifact} registerBeforeLeave={registerBeforeLeave} />}
        {currentView === 'memory' && <MemoryView onOpenArtifact={handleOpenArtifact} registerBeforeLeave={registerBeforeLeave} />}

        {currentView === 'workspace_index' && (
          <WorkspaceIndexView
            initialTab={workspaceIndexTab}
            initialNoteScope={workspaceNoteScope}
            onOpenProject={handleOpenProject}
            onOpenArtifact={handleOpenArtifact}
            onOpenObjectCollection={handleOpenObjectCollection}
            onOpenObjectTypeManager={handleOpenObjectTypeManager}
            onBack={() => {
              setCurrentView('home');
            }}
            onWorkspaceMutated={loadState}
          />
        )}

        {currentView === 'project' && overviewProjectId && <ProjectOverviewView
          key={overviewProjectId}
          projectId={overviewProjectId}
          initialBrowse={projectBrowse}
          onBrowseChange={value => {setProjectBrowse(value);if(overviewProjectId)projectBrowseById.current[overviewProjectId]=value;}}
          refreshTrigger={treeRefreshTrigger}
          onBack={() => handleNavigateWorkspaceIndex('projects')}
          onOpenProject={handleOpenProject}
          onOpenArtifact={(id, pid) => handleOpenArtifact(id, pid, undefined, overviewProjectId)}
          onOpenCanvas={(id, pid) => handleOpenCanvas(id, pid, overviewProjectId)}
          onWorkspaceMutated={loadState}
        />}

        {currentView === 'focus' && (activeProjectId || activeArtifactId) && (
          <FocusView
            key={`${activeArtifactId || 'draft'}:${activeProjectId || 'standalone'}`}
            registerBeforeLeave={registerBeforeLeave}
            sessionNamespace={documentTabs.namespace}
            initialScrollTop={documentTabs.state.tabs.find(tab => tab.id === activeArtifactId)?.scrollTop || 0}
            onScrollPosition={top => { if(activeArtifactId) documentTabs.update(previous => ({...previous,tabs:previous.tabs.map(tab => tab.id === activeArtifactId ? {...tab,scrollTop:top} : tab)})); }}
            onOpenInNewTab={openAnotherDocument}
            onCloseUnavailable={activeArtifactId ? () => {
              const previous = documentTabs.stateRef.current;
              const next = closeDocumentTab(previous, activeArtifactId);
              documentTabs.update(next);
              const tab = next.tabs.find(item => item.id === next.activeId);
              if (tab) {
                applyTab(tab);
              } else {
                setCurrentView('home');
                setActiveArtifactId(null);
                setActiveProjectId(null);
                void loadState();
              }
            } : undefined}
            onDocumentLoaded={(artifact,resolvedProjectId) => {
              if(!artifact.id)return;
              const current=activeRouteRef.current.view==='focus'&&activeRouteRef.current.artifactId===artifact.id;
              documentTabs.update(previous => {
                if(!previous.tabs.some(tab=>tab.id===artifact.id))return current?openDocumentTab(previous,{id:artifact.id,title:artifact.title||'Untitled page',projectId:activeProjectId ?? resolvedProjectId,returnProjectId:projectReturnId,scrollTop:0},true):previous;
                return {...previous,tabs:previous.tabs.map(tab => tab.id === artifact.id ? {...tab,title:artifact.title || 'Untitled page'} : tab)};
              });
            }}
            projectId={activeProjectId || undefined}
            artifactId={activeArtifactId || undefined}
            backLabel={canvasReturnRef.current?.artifactId === activeArtifactId ? 'Canvas' : projectReturnId ? 'Project' : 'Home'}
            onBack={async () => {
              const origin = canvasReturnRef.current;
              if (origin && origin.artifactId === activeArtifactId) {
                handleOpenCanvas(origin.canvasId, origin.projectId, origin.returnProjectId);
                await loadState();
                return;
              }
              if (projectReturnId) {
                handleOpenProject(projectReturnId);
                await loadState();
                return;
              }
              setCurrentView('home');
              setActiveArtifactId(null);
              setActiveProjectId(null);
              await loadState();
              setTreeRefreshTrigger((prev) => prev + 1);
            }}
            onOpenStandaloneNotes={() => handleNavigateWorkspaceIndex('notes', 'standalone')}
            onOpenCanvas={handleOpenCanvas}
            onOpenSearch={() => setIsSearchOpen(true)}
            onOpenArtifact={(id: string, pid?: string | null) => handleOpenArtifact(id, pid, undefined, projectReturnId)}
            onOpenProject={handleOpenProject}
            onContentUpdate={() => {}}
            documentRun={
              documentRun &&
              documentRun.artifactId ===
                (activeArtifactId || (homeState as any)?.active_artifact_id)
                ? documentRun
                : null
            }
            onDismissDocumentRun={() => setDocumentRun(null)}
            onSave={(art) => {
              if(!activeArtifactId) {
                setActiveArtifactId(art.id);
                documentTabs.update(previous => openDocumentTab(previous,{id:art.id,title:art.title || 'Untitled page',projectId:activeProjectId,returnProjectId:projectReturnId,scrollTop:0},false));
              } else documentTabs.update(previous => ({...previous,tabs:previous.tabs.map(tab => tab.id === art.id ? {...tab,title:art.title || 'Untitled page'} : tab)}));
              loadState();
              setTreeRefreshTrigger((prev) => prev + 1);
            }}
            onTitleUpdate={(artId, newTitle) => {
              documentTabs.update(previous => ({...previous,tabs:previous.tabs.map(tab => tab.id === artId ? {...tab,title:newTitle} : tab)}));
              setRealtimeTitleUpdate({ id: artId, title: newTitle });
              setHomeState((prev) => ({
                ...prev,
                recent_items: prev.recent_items.map((item) =>
                  item.artifact_id === artId || item.id === artId
                    ? { ...item, title: newTitle }
                    : item
                ),
                loose_captures: prev.loose_captures.map((cap) =>
                  cap.id === artId
                    ? { ...cap, title: newTitle }
                    : cap
                ),
              }));
            }}
            realtimeTitleUpdate={realtimeTitleUpdate}
          />
        )}

        {currentView === 'canvas' && (
          <CanvasView
            key={activeCanvasId ?? activeProjectId ?? 'default'}
            canvasId={activeCanvasId || undefined}
            projectId={activeProjectId || undefined}
            onBack={() => {
              if (projectReturnId) { handleOpenProject(projectReturnId); return; }
              setCurrentView('home');
              setActiveArtifactId(null);
            }}
            onOpenArtifact={(id, canvas) => handleOpenArtifact(id, undefined, false, projectReturnId, {
              canvasId: canvas.id,
              projectId: canvas.project_id ?? undefined,
              returnProjectId: projectReturnId,
            })}
          />
        )}

        {currentView === 'objects' && activeObjectTypeId && (
          (() => {
            const selectedType = objectTypes.find((t) => t.id === activeObjectTypeId);
            if (!selectedType) return null;
            return (
              <ObjectCollectionContainer
                objectType={selectedType}
                onBack={() => {
                  setCurrentView('home');
                  setActiveArtifactId(null);
                  setActiveObjectTypeId(null);
                }}
                onOpenArtifact={handleOpenArtifact}
                onOpenCanvas={handleOpenCanvas}
                onWorkspaceMutated={loadState}
              />
            );
          })()
        )}

        {(currentView === 'explore_plan' || currentView === 'scheduled_topics') && (
          <ExplorePlanView
            initialTab={explorePlanTab}
            onOpenArtifact={handleOpenArtifact}
            onStartResearch={handleStartResearch}
            onStartPlan={handleStartPlan}
            onBack={() => setCurrentView('home')}
            onWorkspaceMutated={loadState}
          />
        )}

        {currentView === 'graph' && (
          <GraphView
            onOpenProject={handleOpenProject}
            onOpenArtifact={handleOpenArtifact}
            onBack={() => {
              setCurrentView('home');
            }}
          />
        )}
        </>
        )}
      </main>

      <AgentTaskCenter open={taskCenterOpen} onClose={()=>setTaskCenterOpen(false)} onOpenRun={openAgentRun} onOpenDocument={id=>{setTaskCenterOpen(false);handleOpenArtifact(id);}} />
      {guideEntry && <WorkspaceGuide key={guideEntry === 'tour' ? activePageTour : helpArticle ?? guideEntry}
        entry={guideEntry} initialArticle={helpArticle}
        initialTour={guideEntry === 'tour' ? pageTours[activePageTour] : undefined}
        onClose={() => { setGuideEntry(null); setHelpArticle(undefined); }}
        onOpenSettings={section => {
          setGuideEntry(null);
          setHelpArticle(undefined);
          setSettingsSection(section);
          setSettingsTabRequest(request => request + 1);
          setIsSettingsOpen(true);
        }} />}
      {/* Global Interactive Modals */}
      <SearchModal
        isOpen={isSearchOpen}
        onClose={() => { searchNewTab.current=false; setIsSearchOpen(false); }}
        onOpenProject={handleOpenProject}
        onOpenArtifact={handleOpenArtifact}
      />

      <ModelSettingsModal
        initialTab={settingsSection}
        tabRequest={settingsTabRequest}
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
        onOpenTour={() => openPageTour('settings')}
        onVaultMutated={loadState}
      />

      <AccountModal
        isOpen={isAccountOpen}
        onClose={() => setIsAccountOpen(false)}
        onSessionChange={handleSessionUpdated}
      />

      <CreateWorkspaceModal
        isOpen={isCreateWorkspaceOpen}
        session={googleSession}
        onSessionChange={handleSessionUpdated}
        onClose={() => setIsCreateWorkspaceOpen(false)}
        onOpenAccount={() => {
          setIsCreateWorkspaceOpen(false);
          setIsAccountOpen(true);
        }}
        onCreated={(ws) => {
          setIsCreateWorkspaceOpen(false);
          setWorkspaces((prev) => [ws, ...prev.filter((w) => w.workspaceId !== ws.workspaceId)]);
          selectTeamWorkspace(ws, 'overview');
          void refreshWorkspaces(false);
        }}
      />

      <JoinWorkspaceModal
        isOpen={isJoinWorkspaceOpen}
        session={googleSession}
        onSessionChange={handleSessionUpdated}
        onClose={() => setIsJoinWorkspaceOpen(false)}
        onOpenAccount={() => {
          setIsJoinWorkspaceOpen(false);
          setIsAccountOpen(true);
        }}
        onJoined={(ws) => {
          setIsJoinWorkspaceOpen(false);
          setWorkspaces((prev) => [ws, ...prev.filter((w) => w.workspaceId !== ws.workspaceId)]);
          selectTeamWorkspace(ws, 'overview');
          void refreshWorkspaces(false);
        }}
      />

      <InteractiveFlowModal
        mode={activeFlow?.mode || null}
        initialPrompt={activeFlow?.initialPrompt}
        onClose={() => setActiveFlow(null)}
        onProjectCreated={(newProjectId) => {
          loadState();
          handleOpenProject(newProjectId);
        }}
        onArtifactSaved={(artId) => {
          loadState();
          if (artId) {
            handleOpenArtifact(artId, null);
          }
        }}
      />

      <ObjectTypeBuilderModal
        isOpen={isObjectTypeBuilderOpen}
        onClose={() => setIsObjectTypeBuilderOpen(false)}
        onTypeCreated={handleTypeCreated}
      />

      <ObjectTypeManagerModal
        isOpen={isObjectTypeManagerOpen}
        onClose={() => setIsObjectTypeManagerOpen(false)}
        onTypesChanged={loadState}
      />
    </div>
  );
};

export const App: React.FC = () => <AgentTasksProvider><WorkspaceApp /></AgentTasksProvider>;
export default App;
