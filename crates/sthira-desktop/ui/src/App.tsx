import React, { useState, useEffect } from 'react';
import { HomeView } from './components/HomeView';
import { FocusView } from './components/FocusView';
import { CanvasView } from './components/CanvasView';
import { LeftSidebar } from './components/LeftSidebar';
import { SearchModal } from './components/SearchModal';
import { ModelSettingsModal } from './components/ModelSettingsModal';
import { AccountModal } from './components/AccountModal';
import { InteractiveFlowModal, type FlowMode } from './components/InteractiveFlowModal';
import { ObjectCollectionContainer } from './components/objects/ObjectCollectionContainer';
import { ObjectTypeBuilderModal } from './components/objects/ObjectTypeBuilderModal';
import { ObjectTypeManagerModal } from './components/objects/ObjectTypeManagerModal';
import { GraphView } from './views/GraphView';
import { WorkspaceIndexView } from './views/WorkspaceIndexView';
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
  streamAdaptiveRun,
} from './api/ipc';
import {
  type HomeStateView,
  type ObjectType,
  type WorkspaceIndexTab,
  type ExplorePlanTab,
  type DocumentRunState,
  type WebSearchMode,
  type WebResearchResult,
} from './types/sthira';

export const App: React.FC = () => {
  const [currentView, setCurrentView] = useState<
    | 'home'
    | 'focus'
    | 'canvas'
    | 'objects'
    | 'graph'
    | 'workspace_index'
    | 'scheduled_topics'
    | 'explore_plan'
  >('home');
  const [workspaceIndexTab, setWorkspaceIndexTab] = useState<WorkspaceIndexTab>('all');
  const [explorePlanTab, setExplorePlanTab] = useState<ExplorePlanTab>('research');
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null);
  const [activeArtifactId, setActiveArtifactId] = useState<string | null>(null);
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

  // Modals state
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isAccountOpen, setIsAccountOpen] = useState(false);
  const [isObjectTypeBuilderOpen, setIsObjectTypeBuilderOpen] = useState(false);
  const [isObjectTypeManagerOpen, setIsObjectTypeManagerOpen] = useState(false);
  const [activeFlow, setActiveFlow] = useState<{ mode: FlowMode; initialPrompt?: string } | null>(null);

  const [objectTypes, setObjectTypes] = useState<ObjectType[]>([]);
  const [homeState, setHomeState] = useState<HomeStateView>({
    projects: [],
    recent_items: [],
    loose_captures: [],
  });

  const loadState = async () => {
    try {
      const [state, types] = await Promise.all([
        getHomeState(),
        listObjectTypes(),
      ]);
      setHomeState(state);
      setObjectTypes(types);
      setTreeRefreshTrigger((prev) => prev + 1);
    } catch (err) {
      console.error('Failed to load home state', err);
    }
  };

  useEffect(() => {
    loadState();

    // Keydown shortcut: Cmd+K / Ctrl+K opens search
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        setIsSearchOpen((prev) => !prev);
      }
    };
    window.addEventListener('keydown', handleKeyDown);

    // Listen to live Tauri core events if running inside desktop app
    if (isTauri) {
      let unlistenFn: (() => void) | undefined;
      import('@tauri-apps/api/event').then(({ listen }) => {
        listen('sthira://core-event', (event) => {
          console.log('Received core event:', event);
          loadState();
        }).then((unlisten) => {
          unlistenFn = unlisten;
        });
      });

      return () => {
        window.removeEventListener('keydown', handleKeyDown);
        if (unlistenFn) unlistenFn();
      };
    }

    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, []);

  const handleOpenProject = (projectId: string) => {
    setActiveProjectId(projectId);
    setActiveArtifactId(null);
    setCurrentView('focus');
  };

  const handleOpenArtifact = (artifactId: string, projectId?: string | null) => {
    setActiveArtifactId(artifactId);
    if (projectId) {
      setActiveProjectId(projectId);
    } else if (projectId === null) {
      setActiveProjectId(null);
    } else {
      // Find project if attached in recent_items
      const recent = homeState.recent_items.find((r) => r.artifact_id === artifactId);
      if (recent && recent.project_id) {
        setActiveProjectId(recent.project_id);
      } else {
        setActiveProjectId(null);
      }
    }
    setCurrentView('focus');
  };

  const handleOpenCanvas = (canvasId?: string, projectId?: string) => {
    if (projectId) setActiveProjectId(projectId);
    setActiveCanvasId(canvasId || null);
    setCurrentView('canvas');
  };

  const handleOpenObjectCollection = (typeId: string) => {
    setActiveObjectTypeId(typeId);
    setCurrentView('objects');
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

  const handleNavigateWorkspaceIndex = (tab?: WorkspaceIndexTab) => {
    setWorkspaceIndexTab(tab || 'all');
    setCurrentView('workspace_index');
  };

  const handleNavigateExplorePlan = (tab?: ExplorePlanTab) => {
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
    projectId?: string | null
  ): Promise<WebResearchResult | null> => {
    const q = query.trim();
    if (!q) return null;

    const targetProject = projectId !== undefined ? projectId : activeProjectId || null;
    let artifactId: string | null = null;

    try {
      artifactId = await createResearchPage(q, targetProject);

      // 1. Center-page navigation happens immediately, before any slow work.
      setActiveArtifactId(artifactId);
      if (targetProject) setActiveProjectId(targetProject);
      setCurrentView('focus');

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
      const result = await runResearchIntoPage(artifactId, q, mode, (stage) => {
        setDocumentRun((prev) =>
          prev && prev.artifactId === artifactId ? { ...prev, message: stage.message } : prev
        );
      });

      // 3. Mark complete and ask the editor to reload the now-populated page.
      const isFailedSearch =
        mode !== 'local_vault_only' &&
        (result.external_sources_count === 0 ||
          result.search_status === 'web_search_failed' ||
          result.mode_used === 'local_vault_only');

      setDocumentRun((prev) =>
        prev && prev.artifactId === artifactId
          ? {
              ...prev,
              status: isFailedSearch ? 'warning' : 'done',
              mode: result.mode_used,
              search_status: result.search_status,
              external_sources_count: result.external_sources_count,
              message: isFailedSearch
                ? '⚠️ Web search returned no results; synthesized using local context.'
                : 'Research complete.',
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
          ? { ...prev, status: 'error', error: message, message }
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
    projectId?: string | null
  ): Promise<string | null> => {
    const g = goal.trim();
    if (!g) return null;

    const targetProject = projectId !== undefined ? projectId : activeProjectId || null;
    const title = `Plan: ${g.length > 60 ? `${g.slice(0, 60)}...` : g}`;
    let artifactId: string | null = null;

    try {
      const draft = await createPage(targetProject, title);
      artifactId = draft.id;

      // 1. Navigate to the center workspace before streaming begins.
      setActiveArtifactId(artifactId);
      if (targetProject) setActiveProjectId(targetProject);
      setCurrentView('focus');

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

      // 2. Stream tokens into the page as they arrive.
      let accumulated = '';
      await streamAdaptiveRun(g, 'plan', null, (chunk) => {
        if (chunk.type !== 'text_token') return;
        accumulated += chunk.payload;
        const live = accumulated;
        setDocumentRun((prev) =>
          prev && prev.artifactId === artifactId
            ? { ...prev, streamedMarkdown: live, message: 'Drafting plan…' }
            : prev
        );
      });

      // 3. Persist the finished plan and reload the page.
      const body = accumulated.trim()
        ? accumulated.trim().startsWith('#')
          ? accumulated.trim()
          : `# ${title}\n\n${accumulated.trim()}`
        : `${draftBody}\n## Plan\n_No streamed content was returned._\n`;

      await saveArtifact(artifactId, body, title, targetProject);

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
          ? { ...prev, status: 'error', error: message, message }
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

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[#F5F3EC] dark:bg-[#121310] text-[#1E201C] dark:text-[#EDEAE2]">
      {/* Step 2: Left Hierarchy Sidebar */}
      <LeftSidebar
        activeProjectId={activeProjectId}
        activeArtifactId={activeArtifactId}
        activeCanvasId={activeCanvasId}
        activeObjectTypeId={activeObjectTypeId}
        currentView={currentView}
        onNavigateHome={() => {
          setCurrentView('home');
          setActiveProjectId(null);
          setActiveArtifactId(null);
          setActiveObjectTypeId(null);
        }}
        onNavigateGraph={() => {
          setCurrentView('graph');
          setActiveArtifactId(null);
          setActiveObjectTypeId(null);
        }}
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
        isCollapsed={isLeftSidebarCollapsed}
        onToggleCollapse={() => setIsLeftSidebarCollapsed((prev) => !prev)}
        refreshTrigger={treeRefreshTrigger}
        realtimeTitleUpdate={realtimeTitleUpdate}
        onArtifactRenamed={(id, newTitle) => {
          setRealtimeTitleUpdate({ id, title: newTitle });
          loadState();
        }}
      />

      {/* Main Content Area — the full-width center workspace. There is deliberately no
          persistent right-hand drawer; every mode (research, plan, synthesis) streams
          into this container. */}
      <main
        data-testid="center-workspace"
        className={`flex-1 h-full min-w-0 ${currentView === 'focus' ? 'overflow-hidden' : 'overflow-y-auto'} bg-[#FAF8F3] dark:bg-[#121310] flex flex-col relative`}
      >
        {currentView === 'home' && (
          <HomeView
            homeState={homeState}
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

        {currentView === 'workspace_index' && (
          <WorkspaceIndexView
            initialTab={workspaceIndexTab}
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

        {currentView === 'focus' && (activeProjectId || activeArtifactId) && (
          <FocusView
            projectId={activeProjectId || undefined}
            artifactId={activeArtifactId || undefined}
            onBack={async () => {
              setCurrentView('home');
              setActiveArtifactId(null);
              setActiveProjectId(null);
              await loadState();
              setTreeRefreshTrigger((prev) => prev + 1);
            }}
            onOpenCanvas={handleOpenCanvas}
            onOpenSearch={() => setIsSearchOpen(true)}
            onOpenArtifact={handleOpenArtifact}
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
              setActiveArtifactId(art.id);
              loadState();
              setTreeRefreshTrigger((prev) => prev + 1);
            }}
            onTitleUpdate={(artId, newTitle) => {
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
            canvasId={activeCanvasId || undefined}
            projectId={activeProjectId || undefined}
            onBack={() => {
              setCurrentView('home');
              setActiveArtifactId(null);
            }}
            onOpenArtifact={handleOpenArtifact}
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
      </main>

      {/* Global Interactive Modals */}
      <SearchModal
        isOpen={isSearchOpen}
        onClose={() => setIsSearchOpen(false)}
        onOpenProject={handleOpenProject}
        onOpenArtifact={handleOpenArtifact}
      />

      <ModelSettingsModal
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
        onVaultMutated={loadState}
      />

      <AccountModal
        isOpen={isAccountOpen}
        onClose={() => setIsAccountOpen(false)}
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

export default App;
