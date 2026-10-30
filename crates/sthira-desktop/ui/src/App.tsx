import React, { useState, useEffect } from 'react';
import { HomeView } from './components/HomeView';
import { FocusView } from './components/FocusView';
import { CanvasView } from './components/CanvasView';
import { LeftSidebar } from './components/LeftSidebar';
import { RightModeRail } from './components/RightModeRail';
import { SearchModal } from './components/SearchModal';
import { ModelSettingsModal } from './components/ModelSettingsModal';
import { AccountModal } from './components/AccountModal';
import { InteractiveFlowModal, type FlowMode } from './components/InteractiveFlowModal';
import { ObjectCollectionContainer } from './components/objects/ObjectCollectionContainer';
import { ObjectTypeBuilderModal } from './components/objects/ObjectTypeBuilderModal';
import { GraphView } from './views/GraphView';
import { WorkspaceIndexView } from './views/WorkspaceIndexView';
import { getHomeState, promoteToProject, listObjectTypes, isTauri } from './api/ipc';
import type { HomeStateView, ObjectType, WorkspaceIndexTab } from './types/sthira';

export const App: React.FC = () => {
  const [currentView, setCurrentView] = useState<'home' | 'focus' | 'canvas' | 'objects' | 'graph' | 'workspace_index'>('home');
  const [workspaceIndexTab, setWorkspaceIndexTab] = useState<WorkspaceIndexTab>('all');
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null);
  const [activeArtifactId, setActiveArtifactId] = useState<string | null>(null);
  const [activeCanvasId, setActiveCanvasId] = useState<string | null>(null);
  const [activeObjectTypeId, setActiveObjectTypeId] = useState<string | null>(null);
  const [activeDocumentContent, setActiveDocumentContent] = useState<string>('');
  const [isLeftSidebarCollapsed, setIsLeftSidebarCollapsed] = useState(false);
  const [treeRefreshTrigger, setTreeRefreshTrigger] = useState(0);
  const [realtimeTitleUpdate, setRealtimeTitleUpdate] = useState<{ id: string; title: string } | null>(null);

  // Modals state
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isAccountOpen, setIsAccountOpen] = useState(false);
  const [isObjectTypeBuilderOpen, setIsObjectTypeBuilderOpen] = useState(false);
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

  const handleTypeCreated = async (newType: ObjectType) => {
    await loadState();
    setActiveObjectTypeId(newType.id);
    setCurrentView('objects');
  };

  const handleNavigateWorkspaceIndex = (tab?: WorkspaceIndexTab) => {
    setWorkspaceIndexTab(tab || 'all');
    setCurrentView('workspace_index');
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
        onOpenProject={handleOpenProject}
        onOpenArtifact={handleOpenArtifact}
        onOpenCanvas={handleOpenCanvas}
        onOpenObjectCollection={handleOpenObjectCollection}
        onOpenCreateObjectType={handleOpenCreateObjectType}
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

      {/* Main Content Area */}
      <main className={`flex-1 h-screen ${currentView === 'focus' ? 'overflow-hidden' : 'overflow-y-auto'} bg-[#F5F3EC] dark:bg-[#121310] flex flex-col relative`}>
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
          />
        )}

        {currentView === 'workspace_index' && (
          <WorkspaceIndexView
            initialTab={workspaceIndexTab}
            onOpenProject={handleOpenProject}
            onOpenArtifact={handleOpenArtifact}
            onOpenObjectCollection={handleOpenObjectCollection}
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
            onContentUpdate={(c) => setActiveDocumentContent(c)}
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

      {/* Step 3: Right Mode Rail (Capture, Research, Plan, Act) */}
      <RightModeRail
        activeProjectId={activeProjectId}
        activeArtifactId={activeArtifactId}
        activeDocumentContent={activeDocumentContent}
        onApplyContentUpdate={(newContent) => {
          setActiveDocumentContent(newContent);
          loadState();
        }}
        onWorkspaceMutated={loadState}
        onOpenArtifact={handleOpenArtifact}
      />

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
    </div>
  );
};

export default App;
