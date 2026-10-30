import React, { useState, useEffect } from 'react';
import {
  Folder,
  FolderOpen,
  FileText,
  LayoutGrid,
  Plus,
  ChevronRight,
  ChevronDown,
  Home,
  Search,
  SlidersHorizontal,
  User,
  PanelLeftClose,
  PanelLeft,
  Share2,
  Trash2,
  MoreHorizontal,
  Pin,
  Copy,
  Edit3,
  FolderPlus,
  Settings2,
} from 'lucide-react';
import {
  getWorkspaceTree,
  createPage,
  createProject,
  listObjectTypes,
  getObjectTypeCounts,
  deleteArtifact,
  duplicateArtifact,
  duplicateProject,
  renameProject,
  renameArtifact,
  getSidebarItems,
  togglePin,
} from '../api/ipc';
import { ProjectDeletionModal } from './ProjectDeletionModal';
import { AddToProjectModal } from './AddToProjectModal';
import { ThemeToggle } from './ThemeToggle';
import type {
  WorkspaceTree,
  ObjectType,
  SidebarItemsResponse,
  WorkspaceIndexTab,
} from '../types/sthira';

interface LeftSidebarProps {
  activeProjectId?: string | null;
  activeArtifactId?: string | null;
  activeCanvasId?: string | null;
  activeObjectTypeId?: string | null;
  currentView: 'home' | 'focus' | 'canvas' | 'objects' | 'graph' | 'workspace_index';
  onNavigateHome: () => void;
  onNavigateGraph: () => void;
  onNavigateWorkspaceIndex?: (tab?: WorkspaceIndexTab) => void;
  onOpenProject: (projectId: string) => void;
  onOpenArtifact: (artifactId: string, projectId?: string | null) => void;
  onOpenCanvas: (canvasId?: string, projectId?: string) => void;
  onOpenObjectCollection?: (typeId: string) => void;
  onOpenCreateObjectType?: () => void;
  /** Phase 16: Opens the Object Type & Template Manager modal. */
  onOpenObjectTypeManager?: () => void;
  onOpenSearch: () => void;
  onOpenSettings: () => void;
  onOpenAccount: () => void;
  isCollapsed: boolean;
  onToggleCollapse: () => void;
  refreshTrigger?: number;
  realtimeTitleUpdate?: { id: string; title: string } | null;
  onArtifactRenamed?: (id: string, newTitle: string) => void;
}

export const LeftSidebar: React.FC<LeftSidebarProps> = ({
  activeProjectId,
  activeArtifactId,
  activeCanvasId,
  activeObjectTypeId,
  currentView,
  onNavigateHome,
  onNavigateGraph,
  onNavigateWorkspaceIndex,
  onOpenProject,
  onOpenArtifact,
  onOpenCanvas,
  onOpenObjectCollection,
  onOpenCreateObjectType,
  onOpenObjectTypeManager,
  onOpenSearch,
  onOpenSettings,
  onOpenAccount,
  isCollapsed,
  onToggleCollapse,
  refreshTrigger,
  realtimeTitleUpdate,
  onArtifactRenamed,
}) => {
  const [tree, setTree] = useState<WorkspaceTree>({
    projects: [],
    unassociated_pages: [],
  });
  const [sidebarItems, setSidebarItems] = useState<SidebarItemsResponse>({
    projects: [],
    notes: [],
    objects: [],
  });
  const [objectTypes, setObjectTypes] = useState<ObjectType[]>([]);
  const [objectCounts, setObjectCounts] = useState<Record<string, number>>({});
  const [expandedProjects, setExpandedProjects] = useState<Record<string, boolean>>({});
  const [projectToDelete, setProjectToDelete] = useState<string | null>(null);

  // Settings Menu, Pinning, Renaming, Add to Project states
  const [pinnedIds, setPinnedIds] = useState<Record<string, boolean>>(() => {
    try {
      return JSON.parse(localStorage.getItem('sthira_pinned_items') || '{}');
    } catch {
      return {};
    }
  });
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renamingText, setRenamingText] = useState('');
  const [addToProjectTarget, setAddToProjectTarget] = useState<{ id: string; title: string } | null>(null);

  const [isCreatingProject, setIsCreatingProject] = useState(false);
  const [newProjectName, setNewProjectName] = useState('');
  const [newProjectPurpose, setNewProjectPurpose] = useState('');
  const [isSubmittingProject, setIsSubmittingProject] = useState(false);

  // Resizable sidebar width with localStorage persistence
  const [sidebarWidth, setSidebarWidth] = useState<number>(() => {
    try {
      const saved = localStorage.getItem('sthira_sidebar_width');
      if (saved) {
        const parsed = parseInt(saved, 10);
        if (!isNaN(parsed) && parsed >= 200 && parsed <= 700) {
          return parsed;
        }
      }
    } catch {}
    return 280;
  });
  const [isDragging, setIsDragging] = useState(false);

  const handleMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    setIsDragging(true);
  };

  const handleResetWidth = () => {
    setSidebarWidth(280);
    try {
      localStorage.setItem('sthira_sidebar_width', '280');
    } catch {}
  };

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      // Dynamic max width: min 200px, max 45% of viewport width (up to 700px)
      const maxW = Math.max(400, Math.min(700, window.innerWidth * 0.45));
      const newWidth = Math.min(Math.max(e.clientX, 200), maxW);
      setSidebarWidth(newWidth);
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
  }, [isDragging]);

  useEffect(() => {
    if (!isDragging) {
      try {
        localStorage.setItem('sthira_sidebar_width', sidebarWidth.toString());
      } catch {}
    }
  }, [sidebarWidth, isDragging]);

  // Close menus on outside click
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!target.closest('[data-menu-container]')) {
        setOpenMenuId(null);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  // Real-time optimistic title update from active FocusView typing
  useEffect(() => {
    if (realtimeTitleUpdate && realtimeTitleUpdate.id) {
      setTree((prev) => ({
        projects: prev.projects.map((proj) => ({
          ...proj,
          pages: proj.pages.map((p) =>
            p.id === realtimeTitleUpdate.id
              ? { ...p, title: realtimeTitleUpdate.title }
              : p
          ),
        })),
        unassociated_pages: prev.unassociated_pages.map((p) =>
          p.id === realtimeTitleUpdate.id
            ? { ...p, title: realtimeTitleUpdate.title }
            : p
        ),
      }));
    }
  }, [realtimeTitleUpdate]);

  const handleTogglePin = async (id: string, itemType: string, currentPinned: boolean, e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    try {
      await togglePin(id, itemType, !currentPinned);
      setPinnedIds((prev) => {
        const next = { ...prev, [id]: !currentPinned };
        try {
          localStorage.setItem('sthira_pinned_items', JSON.stringify(next));
        } catch {}
        return next;
      });
      await loadTree();
    } catch (err) {
      console.error('Failed to toggle pin', err);
    }
    setOpenMenuId(null);
  };

  const startInlineRename = (id: string, currentText: string, _kind: 'project' | 'artifact', e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    setRenamingId(id);
    setRenamingText(currentText);
    setOpenMenuId(null);
  };

  const commitRename = async (id: string, kind: 'project' | 'artifact') => {
    if (!renamingText.trim()) {
      setRenamingId(null);
      return;
    }
    try {
      if (kind === 'project') {
        await renameProject(id, renamingText.trim());
      } else {
        await renameArtifact(id, renamingText.trim());
        if (onArtifactRenamed) {
          onArtifactRenamed(id, renamingText.trim());
        }
      }
      await loadTree();
    } catch (err) {
      console.error('Rename failed', err);
    } finally {
      setRenamingId(null);
    }
  };

  const handleDuplicateProjectAction = async (projId: string, e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    setOpenMenuId(null);
    try {
      const copy = await duplicateProject(projId);
      await loadTree();
      setExpandedProjects((prev) => ({ ...prev, [copy.id]: true }));
      onOpenProject(copy.id);
    } catch (err) {
      console.error('Failed to duplicate project', err);
    }
  };

  const handleDuplicateNoteAction = async (artId: string, projId?: string, e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    setOpenMenuId(null);
    try {
      const copy = await duplicateArtifact(artId, projId || null);
      await loadTree();
      onOpenArtifact(copy.id, projId);
    } catch (err) {
      console.error('Failed to duplicate note', err);
    }
  };

  const handleCreateProjectSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newProjectName.trim() || isSubmittingProject) return;
    try {
      setIsSubmittingProject(true);
      const proj = await createProject(newProjectName.trim(), newProjectPurpose.trim() || undefined);
      setExpandedProjects((prev) => ({ ...prev, [proj.id]: true }));
      setIsCreatingProject(false);
      setNewProjectName('');
      setNewProjectPurpose('');
      await loadTree();
      onOpenProject(proj.id);
    } catch (err) {
      console.error('Failed to create project', err);
    } finally {
      setIsSubmittingProject(false);
    }
  };

  const loadTree = async () => {
    try {
      const [data, types, counts, sidebarData] = await Promise.all([
        getWorkspaceTree().catch(() => ({ projects: [], unassociated_pages: [] })),
        listObjectTypes().catch(() => []),
        getObjectTypeCounts().catch(() => ({})),
        getSidebarItems().catch(() => ({ projects: [], notes: [], objects: [] })),
      ]);
      setTree(data || { projects: [], unassociated_pages: [] });
      setObjectTypes(types || []);
      setObjectCounts(counts || {});
      setSidebarItems(sidebarData || { projects: [], notes: [], objects: [] });

      // Auto-expand active project if present
      if (activeProjectId) {
        setExpandedProjects((prev) => ({
          ...prev,
          [activeProjectId]: true,
        }));
      }
    } catch (err) {
      console.error('Failed to load workspace tree', err);
    }
  };

  useEffect(() => {
    loadTree();
  }, [refreshTrigger, activeProjectId]);

  useEffect(() => {
    const handleRefresh = () => {
      loadTree();
    };
    window.addEventListener('sthira:refresh_objects', handleRefresh);
    return () => window.removeEventListener('sthira:refresh_objects', handleRefresh);
  }, []);

  const toggleProjectExpand = (projectId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setExpandedProjects((prev) => ({
      ...prev,
      [projectId]: !prev[projectId],
    }));
  };

  const handleCreatePageInProject = async (projectId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      const art = await createPage(projectId, 'Untitled Page');
      // Ensure project is expanded
      setExpandedProjects((prev) => ({ ...prev, [projectId]: true }));
      await loadTree();
      onOpenArtifact(art.id, projectId);
    } catch (err) {
      console.error('Failed to create page in project', err);
    }
  };

  const handleCreateLooseNote = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      const art = await createPage(null, 'Untitled Note');
      await loadTree();
      onOpenArtifact(art.id, null);
    } catch (err) {
      console.error('Failed to create loose note', err);
    }
  };

  if (isCollapsed) {
    return (
      <aside className="w-14 bg-[#F5F3EC] dark:bg-[#161714] border-r border-[#E8E5DC] dark:border-[#2C2E27] flex flex-col items-center py-3.5 h-screen shrink-0 transition-all select-none z-30 justify-between">
        <div className="flex flex-col items-center gap-3">
          {/* Logo / Home */}
          <button
            type="button"
            onClick={onNavigateHome}
            title="Sthira Home"
            className="w-9 h-9 rounded-xl bg-[#282C25] dark:bg-[#20221C] border border-transparent dark:border-[#383B32] flex items-center justify-center text-white shadow-sm hover:opacity-90 transition-opacity cursor-pointer"
          >
            <svg
              className="w-4 h-4 fill-none stroke-current stroke-2"
              viewBox="0 0 24 24"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <rect x="3" y="3" width="18" height="18" rx="4" />
              <line x1="3" y1="9" x2="21" y2="9" />
              <line x1="9" y1="21" x2="9" y2="9" />
            </svg>
          </button>

          <button
            type="button"
            onClick={onToggleCollapse}
            title="Expand Sidebar"
            className="w-8 h-8 rounded-lg bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2F28] text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white flex items-center justify-center transition-colors cursor-pointer"
          >
            <PanelLeft className="w-4 h-4" />
          </button>

          <div className="w-6 h-px bg-[#E8E5DC] dark:bg-[#2C2E27] my-1" />

          {/* Search Trigger */}
          <button
            type="button"
            onClick={onOpenSearch}
            title="Search (Cmd+K)"
            className="w-8 h-8 rounded-lg text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white hover:bg-[#EFECE4] dark:hover:bg-[#22241F] flex items-center justify-center transition-colors cursor-pointer"
          >
            <Search className="w-4 h-4" />
          </button>

          {/* Graph Trigger */}
          <button
            type="button"
            data-testid="nav-knowledge-graph-collapsed"
            onClick={onNavigateGraph}
            title="Knowledge Graph"
            className={`w-8 h-8 rounded-lg flex items-center justify-center transition-colors cursor-pointer ${
              currentView === 'graph'
                ? 'bg-[#282C25] dark:bg-[#2C3026] text-white font-semibold'
                : 'text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white hover:bg-[#EFECE4] dark:hover:bg-[#22241F]'
            }`}
          >
            <Share2 className="w-4 h-4" />
          </button>

          {/* All Files Trigger */}
          <button
            type="button"
            data-testid="nav-workspace-index-collapsed"
            onClick={() => onNavigateWorkspaceIndex?.('all')}
            title="All Files"
            className={`w-8 h-8 rounded-lg flex items-center justify-center transition-colors cursor-pointer ${
              currentView === 'workspace_index'
                ? 'bg-[#282C25] dark:bg-[#2C3026] text-white font-semibold'
                : 'text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white hover:bg-[#EFECE4] dark:hover:bg-[#22241F]'
            }`}
          >
            <span className="text-sm">📁</span>
          </button>
        </div>

        {/* Bottom controls */}
        <div className="flex flex-col items-center gap-2">
          <ThemeToggle />
          <button
            type="button"
            onClick={onOpenSettings}
            title="Model Settings"
            className="w-8 h-8 rounded-lg text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white hover:bg-[#EFECE4] dark:hover:bg-[#22241F] flex items-center justify-center transition-colors cursor-pointer"
          >
            <SlidersHorizontal className="w-4 h-4" />
          </button>
          <button
            type="button"
            onClick={onOpenAccount}
            title="Account"
            className="w-8 h-8 rounded-full bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2F28] text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white flex items-center justify-center transition-colors border border-[#E8E5DC] dark:border-[#2C2E27] cursor-pointer"
          >
            <User className="w-3.5 h-3.5" />
          </button>
        </div>
      </aside>
    );
  }

  return (
    <aside
      style={{ width: `${sidebarWidth}px` }}
      className={`relative bg-[#F5F3EC] dark:bg-[#141512] border-r border-[#E8E5DC] dark:border-[#262822] flex flex-col h-screen shrink-0 select-none z-30 ${
        isDragging ? '' : 'transition-[width] duration-150 ease-out'
      }`}
    >
      {/* Top Header */}
      <div className="px-4 py-3.5 flex items-center justify-between border-b border-[#E8E5DC]/70 dark:border-[#262822]">
        <button
          type="button"
          onClick={onNavigateHome}
          className="flex items-center gap-2.5 hover:opacity-80 transition-opacity cursor-pointer group"
        >
          <div className="w-7 h-7 rounded-lg bg-[#282C25] dark:bg-[#22241F] border border-transparent dark:border-[#383B32] flex items-center justify-center text-white shadow-xs">
            <svg
              className="w-3.5 h-3.5 fill-none stroke-current stroke-2"
              viewBox="0 0 24 24"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <rect x="3" y="3" width="18" height="18" rx="4" />
              <line x1="3" y1="9" x2="21" y2="9" />
              <line x1="9" y1="21" x2="9" y2="9" />
            </svg>
          </div>
          <span className="text-base font-bold text-[#1E201C] dark:text-[#F3F1EA] tracking-tight">sthira</span>
        </button>

        <div className="flex items-center gap-1">
          <ThemeToggle />
          <button
            type="button"
            onClick={onNavigateHome}
            title="Home"
            className={`p-1.5 rounded-lg transition-colors cursor-pointer ${
              currentView === 'home'
                ? 'bg-[#EFECE4] dark:bg-[#22241F] text-[#1E201C] dark:text-[#EDEAE2]'
                : 'text-[#6E6D66] dark:text-[#A8A69E] hover:bg-[#EFECE4] dark:hover:bg-[#22241F] hover:text-[#1E201C] dark:hover:text-white'
            }`}
          >
            <Home className="w-3.5 h-3.5" />
          </button>

          <button
            type="button"
            onClick={onToggleCollapse}
            title="Collapse Sidebar"
            className="p-1.5 rounded-lg text-[#6E6D66] dark:text-[#A8A69E] hover:bg-[#EFECE4] dark:hover:bg-[#22241F] hover:text-[#1E201C] dark:hover:text-white transition-colors cursor-pointer"
          >
            <PanelLeftClose className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Quick Search & Graph Navigation */}
      <div className="px-3 pt-3 pb-1 space-y-1.5">
        <button
          type="button"
          onClick={onOpenSearch}
          className="w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl bg-[#EFECE4] dark:bg-[#1E201B] hover:bg-[#E5E1D6] dark:hover:bg-[#242721] text-xs text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white transition-colors cursor-pointer border border-[#E8E5DC]/80 dark:border-[#2C2E27] shadow-2xs"
        >
          <span className="flex items-center gap-2">
            <Search className="w-3.5 h-3.5" />
            <span>Search workspace...</span>
          </span>
          <kbd className="text-[10px] bg-white/70 dark:bg-[#262822] px-1.5 py-0.5 rounded text-[#3A3B36] dark:text-[#D4D1C7] font-mono border border-[#E8E5DC] dark:border-[#383B32]">
            ⌘K
          </kbd>
        </button>

        <button
          type="button"
          data-testid="nav-knowledge-graph"
          onClick={onNavigateGraph}
          className={`w-full flex items-center gap-2 px-2.5 py-1.5 rounded-xl text-xs transition-colors cursor-pointer border ${
            currentView === 'graph'
              ? 'bg-[#282C25] dark:bg-[#2C3026] text-white border-[#282C25] dark:border-[#3E4334] font-semibold shadow-xs'
              : 'bg-[#EFECE4] dark:bg-[#1E201B] hover:bg-[#E5E1D6] dark:hover:bg-[#242721] text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white border-[#E8E5DC]/80 dark:border-[#2C2E27]'
          }`}
        >
          <Share2 className="w-3.5 h-3.5" />
          <span>🌐 Knowledge Graph</span>
        </button>

        <button
          type="button"
          data-testid="nav-workspace-index"
          onClick={() => onNavigateWorkspaceIndex?.('all')}
          title="All Files — every project, note, and typed object"
          className={`w-full flex items-center gap-2 px-2.5 py-1.5 rounded-xl text-xs transition-colors cursor-pointer border ${
            currentView === 'workspace_index'
              ? 'bg-[#282C25] dark:bg-[#2C3026] text-white border-[#282C25] dark:border-[#3E4334] font-semibold shadow-xs'
              : 'bg-[#EFECE4] dark:bg-[#1E201B] hover:bg-[#E5E1D6] dark:hover:bg-[#242721] text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white border-[#E8E5DC]/80 dark:border-[#2C2E27]'
          }`}
        >
          <span className="text-xs">📁</span>
          <span>All Files</span>
        </button>
      </div>

      {/* Tree Navigation Scroll Container */}
      <div className="flex-1 overflow-y-auto px-2 py-3 space-y-5 text-xs">
        {/* 1. PROJECTS SECTION */}
        <div>
          <div className="flex items-center justify-between px-2 pb-1.5">
            <span className="text-[11px] font-bold text-[#8C8A82] dark:text-[#A8A59C] uppercase tracking-wider">
              Projects
            </span>
            <div className="flex items-center gap-1.5">
              <button
                type="button"
                data-testid="btn-view-all-projects"
                onClick={() => onNavigateWorkspaceIndex?.('projects')}
                className="text-[10px] text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white hover:underline cursor-pointer font-medium"
              >
                View all →
              </button>
              <button
                type="button"
                data-testid="btn-new-project"
                onClick={() => setIsCreatingProject(true)}
                title="New Project"
                className="p-0.5 rounded hover:bg-[#EFECE4] dark:hover:bg-[#22241F] text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white transition-colors cursor-pointer"
              >
                <Plus className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>

          {isCreatingProject && (
            <form
              onSubmit={handleCreateProjectSubmit}
              data-testid="form-new-project"
              className="p-2 mb-2 bg-white dark:bg-[#1C1D19] rounded-xl border border-[#E8E5DC] dark:border-[#2C2E27] shadow-xs space-y-1.5"
            >
              <input
                type="text"
                autoFocus
                required
                data-testid="input-new-project-name"
                value={newProjectName}
                onChange={(e) => setNewProjectName(e.target.value)}
                placeholder="Project Name..."
                className="w-full px-2 py-1 text-xs font-medium rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-[#FAF9F5] dark:bg-[#161714] text-[#1E201C] dark:text-[#EDEAE2] placeholder:text-[#8C8A82] dark:placeholder:text-[#6E6D66] outline-none focus:border-[#282C25] dark:focus:border-[#9E9C93]"
              />
              <input
                type="text"
                data-testid="input-new-project-purpose"
                value={newProjectPurpose}
                onChange={(e) => setNewProjectPurpose(e.target.value)}
                placeholder="Purpose (optional)..."
                className="w-full px-2 py-1 text-[11px] rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-[#FAF9F5] dark:bg-[#161714] text-[#1E201C] dark:text-[#EDEAE2] placeholder:text-[#8C8A82] dark:placeholder:text-[#6E6D66] outline-none focus:border-[#282C25] dark:focus:border-[#9E9C93]"
              />
              <div className="flex items-center justify-end gap-1.5 pt-1">
                <button
                  type="button"
                  onClick={() => setIsCreatingProject(false)}
                  className="px-2 py-0.5 rounded text-[11px] text-[#6E6D66] dark:text-[#A8A69E] hover:bg-[#EFECE4] dark:hover:bg-[#22241F] cursor-pointer"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={isSubmittingProject || !newProjectName.trim()}
                  data-testid="btn-submit-new-project"
                  className="px-2.5 py-0.5 rounded text-[11px] font-semibold bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] hover:opacity-90 disabled:opacity-50 cursor-pointer"
                >
                  {isSubmittingProject ? 'Creating...' : 'Create'}
                </button>
              </div>
            </form>
          )}

          {((sidebarItems?.projects?.length ?? 0) === 0 && (tree?.projects?.length ?? 0) === 0 && !isCreatingProject) ? (
            <div className="px-2 py-2 text-[11px] text-[#8C8A82] dark:text-[#8E8C82] italic">
              No projects yet
            </div>
          ) : (
            <div className="space-y-0.5">
              {(sidebarItems?.projects?.length ? sidebarItems.projects : (tree?.projects || []).slice(0, 7).map(p => ({
                id: p.id,
                title: p.name,
                item_type: 'project',
                is_pinned: !!pinnedIds[p.id],
                last_accessed_at: p.updated_at,
                updated_at: p.updated_at,
              }))).slice(0, 7).map((proj) => {
                const isExpanded = !!expandedProjects[proj.id];
                const isActiveProj =
                  currentView === 'focus' && activeProjectId === proj.id && !activeArtifactId;
                const isPinned = proj.is_pinned ?? !!pinnedIds[proj.id];
                const treeProj = (tree?.projects || []).find((p) => p.id === proj.id);

                return (
                  <div key={proj.id} className="rounded-lg">
                    {/* Project Header Row */}
                    <div
                      onClick={() => onOpenProject(proj.id)}
                      className={`group flex items-center justify-between px-2 py-1.5 rounded-lg cursor-pointer transition-colors ${
                        isActiveProj
                          ? 'bg-[#EFECE4] dark:bg-[#252821] text-[#1E201C] dark:text-white font-semibold shadow-2xs border border-[#E0DCD3] dark:border-[#383B32]'
                          : 'text-[#2E2F2B] dark:text-[#EDEAE2] hover:bg-[#EFECE4]/70 dark:hover:bg-[#22241F] hover:text-[#1E201C] dark:hover:text-white'
                      }`}
                    >
                      <div className="flex items-center gap-1.5 min-w-0 flex-1">
                        <button
                          type="button"
                          onClick={(e) => toggleProjectExpand(proj.id, e)}
                          className="p-0.5 rounded hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white transition-colors"
                        >
                          {isExpanded ? (
                            <ChevronDown className="w-3.5 h-3.5" />
                          ) : (
                            <ChevronRight className="w-3.5 h-3.5" />
                          )}
                        </button>
                        {isExpanded ? (
                          <FolderOpen className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400 shrink-0" />
                        ) : (
                          <Folder className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400 shrink-0" />
                        )}
                        {renamingId === proj.id ? (
                          <input
                            type="text"
                            autoFocus
                            data-testid={`input-rename-project-${proj.id}`}
                            value={renamingText}
                            onClick={(e) => e.stopPropagation()}
                            onChange={(e) => setRenamingText(e.target.value)}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') commitRename(proj.id, 'project');
                              else if (e.key === 'Escape') setRenamingId(null);
                            }}
                            onBlur={() => commitRename(proj.id, 'project')}
                            className="px-1.5 py-0.5 text-xs rounded border border-[#282C25] dark:border-[#9E9C93] bg-white dark:bg-[#1C1D19] text-[#1E201C] dark:text-[#EDEAE2] outline-none w-32"
                          />
                        ) : (
                          <span className="truncate text-xs font-medium flex items-center gap-1">
                            <span>{proj.title}</span>
                            {isPinned && (
                              <span
                                data-testid={`pinned-chip-${proj.id}`}
                                className="text-[10px] shrink-0"
                                title="Pinned"
                              >
                                📌
                              </span>
                            )}
                          </span>
                        )}
                      </div>

                      {/* Right hover actions: Pin toggle, + New Page, Settings 3-Dots */}
                      <div
                        className="flex items-center gap-1 shrink-0 relative"
                        data-menu-container
                      >
                        <button
                          type="button"
                          data-testid={`pin-btn-${proj.id}`}
                          onClick={(e) => handleTogglePin(proj.id, 'project', isPinned, e)}
                          title={isPinned ? 'Unpin project' : 'Pin project'}
                          className={`p-0.5 rounded hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] cursor-pointer transition-colors ${
                            isPinned
                              ? 'opacity-100 text-amber-600 dark:text-amber-400'
                              : 'opacity-0 group-hover:opacity-100 text-[#8C8A82] hover:text-[#1E201C] dark:hover:text-white'
                          }`}
                        >
                          <Pin className={`w-3 h-3 ${isPinned ? 'rotate-45' : ''}`} />
                        </button>

                        <button
                          type="button"
                          onClick={(e) => handleCreatePageInProject(proj.id, e)}
                          title="New page in this project"
                          className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white cursor-pointer transition-opacity"
                        >
                          <Plus className="w-3.5 h-3.5" />
                        </button>

                        <button
                          type="button"
                          data-testid={`btn-project-menu-${proj.id}`}
                          onClick={(e) => {
                            e.stopPropagation();
                            setOpenMenuId(openMenuId === `proj-${proj.id}` ? null : `proj-${proj.id}`);
                          }}
                          title="Project settings"
                          className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white cursor-pointer transition-opacity"
                        >
                          <MoreHorizontal className="w-3.5 h-3.5" />
                        </button>

                        {/* Project 3-Dots Popup Menu */}
                        {openMenuId === `proj-${proj.id}` && (
                          <div className="absolute right-0 top-6 z-50 w-44 bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl shadow-xl py-1 text-xs animate-in fade-in duration-100">
                            <button
                              type="button"
                              data-testid={`btn-pin-project-${proj.id}`}
                              onClick={(e) => handleTogglePin(proj.id, 'project', isPinned, e)}
                              className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-[#FAF9F5] dark:hover:bg-[#252820] text-left text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white cursor-pointer"
                            >
                              <Pin className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E]" />
                              <span>{isPinned ? 'Unpin from Top' : 'Pin to Top'}</span>
                            </button>
                            <button
                              type="button"
                              data-testid={`btn-duplicate-project-${proj.id}`}
                              onClick={(e) => handleDuplicateProjectAction(proj.id, e)}
                              className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-[#FAF9F5] dark:hover:bg-[#252820] text-left text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white cursor-pointer"
                            >
                              <Copy className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E]" />
                              <span>Duplicate</span>
                            </button>
                            <button
                              type="button"
                              onClick={(e) => startInlineRename(proj.id, proj.title, 'project', e)}
                              className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-[#FAF9F5] dark:hover:bg-[#252820] text-left text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white cursor-pointer"
                            >
                              <Edit3 className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E]" />
                              <span>Rename</span>
                            </button>
                            <div className="h-px bg-[#E8E5DC] dark:bg-[#2C2E27] my-1" />
                            <button
                              type="button"
                              data-testid={`btn-delete-project-${proj.id}`}
                              onClick={(e) => {
                                e.stopPropagation();
                                setOpenMenuId(null);
                                setProjectToDelete(proj.id);
                              }}
                              className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-red-50 dark:hover:bg-red-950/30 text-red-600 dark:text-red-400 text-left cursor-pointer"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                              <span>Delete Project</span>
                            </button>
                          </div>
                        )}
                      </div>
                    </div>

                    {/* Expanded Project Children: Canvases & Pages */}
                    {isExpanded && treeProj && (
                      <div className="ml-5 pl-1.5 border-l border-[#E8E5DC] dark:border-[#2C2E27] py-0.5 space-y-0.5">
                        {treeProj.canvases.map((canvas) => {
                          const isActiveCanvas =
                            currentView === 'canvas' && activeCanvasId === canvas.id;
                          return (
                            <div
                              key={canvas.id}
                              onClick={() => onOpenCanvas(canvas.id, proj.id)}
                              className={`flex items-center gap-2 px-2 py-1 rounded-md cursor-pointer transition-colors ${
                                isActiveCanvas
                                  ? 'bg-[#282C25] dark:bg-[#282B24] text-white font-medium shadow-xs border border-transparent dark:border-[#3A3D33]'
                                  : 'text-[#4A4B45] dark:text-[#D4D1C7] hover:bg-[#EFECE4] dark:hover:bg-[#22241F] hover:text-[#1E201C] dark:hover:text-white'
                              }`}
                            >
                              <LayoutGrid className="w-3 h-3 shrink-0" />
                              <span className="truncate text-[11px]">{canvas.name}</span>
                            </div>
                          );
                        })}

                        {treeProj.pages.map((page) => {
                          const isActivePage =
                            currentView === 'focus' && activeArtifactId === page.id;
                          const isPagePinned = !!pinnedIds[page.id];
                          return (
                            <div
                              key={page.id}
                              onClick={() => onOpenArtifact(page.id, proj.id)}
                              className={`group/page flex items-center justify-between px-2 py-1 rounded-md cursor-pointer transition-colors relative ${
                                isActivePage
                                  ? 'bg-[#282C25] dark:bg-[#282B24] text-white font-semibold shadow-xs border border-transparent dark:border-[#3A3D33]'
                                  : 'text-[#4A4B45] dark:text-[#D4D1C7] hover:bg-[#EFECE4] dark:hover:bg-[#22241F] hover:text-[#1E201C] dark:hover:text-white'
                              }`}
                            >
                              <div className="flex items-center gap-2 min-w-0 flex-1">
                                <FileText className="w-3 h-3 shrink-0" />
                                {renamingId === page.id ? (
                                  <input
                                    type="text"
                                    autoFocus
                                    data-testid={`input-rename-page-${page.id}`}
                                    value={renamingText}
                                    onClick={(e) => e.stopPropagation()}
                                    onChange={(e) => setRenamingText(e.target.value)}
                                    onKeyDown={(e) => {
                                      if (e.key === 'Enter') commitRename(page.id, 'artifact');
                                      else if (e.key === 'Escape') setRenamingId(null);
                                    }}
                                    onBlur={() => commitRename(page.id, 'artifact')}
                                    className="bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#383B32] text-[#1E201C] dark:text-white px-1 py-0.5 rounded text-[11px] w-full outline-hidden"
                                  />
                                ) : (
                                  <span className="truncate text-[11px] flex items-center gap-1">
                                    <span>{page.title || 'Untitled note'}</span>
                                    {isPagePinned && (
                                      <span className="text-[10px] shrink-0">📌</span>
                                    )}
                                  </span>
                                )}
                              </div>

                              <div className="flex items-center gap-1 shrink-0 relative" data-menu-container>
                                {/* Quiet pin hover button */}
                                <button
                                  type="button"
                                  data-testid={`pin-btn-${page.id}`}
                                  onClick={(e) => handleTogglePin(page.id, 'note', isPagePinned, e)}
                                  title={isPagePinned ? 'Unpin note' : 'Pin note'}
                                  className={`p-0.5 rounded hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] cursor-pointer transition-colors ${
                                    isPagePinned
                                      ? 'opacity-100 text-amber-600 dark:text-amber-400'
                                      : 'opacity-0 group-hover/page:opacity-100 text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2]'
                                  }`}
                                >
                                  <Pin className={`w-3 h-3 ${isPagePinned ? 'rotate-45' : ''}`} />
                                </button>

                                {/* 3-dots page menu button */}
                                <button
                                  type="button"
                                  data-testid={`btn-page-menu-${page.id}`}
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    setOpenMenuId(openMenuId === `page-${page.id}` ? null : `page-${page.id}`);
                                  }}
                                  title="Page settings"
                                  className="opacity-0 group-hover/page:opacity-100 p-0.5 rounded hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] transition-opacity cursor-pointer"
                                >
                                  <MoreHorizontal className="w-3 h-3" />
                                </button>

                                {/* Page 3-Dots Popup Menu */}
                                {openMenuId === `page-${page.id}` && (
                                  <div className="absolute right-0 top-5 z-50 w-44 bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl shadow-xl py-1 text-xs animate-in fade-in duration-100">
                                    <button
                                      type="button"
                                      data-testid={`btn-pin-page-${page.id}`}
                                      onClick={(e) => handleTogglePin(page.id, 'note', isPagePinned, e)}
                                      className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-[#FAF9F5] dark:hover:bg-[#22241F] text-left text-[#3A3B36] dark:text-[#D4D1C7] cursor-pointer"
                                    >
                                      <Pin className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#7A786F]" />
                                      <span>{isPagePinned ? 'Unpin Note' : 'Pin Note'}</span>
                                    </button>
                                    <button
                                      type="button"
                                      data-testid={`btn-duplicate-page-${page.id}`}
                                      onClick={(e) => handleDuplicateNoteAction(page.id, proj.id, e)}
                                      className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-[#FAF9F5] dark:hover:bg-[#252820] text-left text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white cursor-pointer"
                                    >
                                      <Copy className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E]" />
                                      <span>Duplicate</span>
                                    </button>
                                    <button
                                      type="button"
                                      data-testid={`btn-add-to-project-page-${page.id}`}
                                      onClick={(e) => {
                                        e.stopPropagation();
                                        setOpenMenuId(null);
                                        setAddToProjectTarget({ id: page.id, title: page.title || 'Untitled Note' });
                                      }}
                                      className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-[#FAF9F5] dark:hover:bg-[#252820] text-left text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white cursor-pointer"
                                    >
                                      <FolderPlus className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E]" />
                                      <span>Add to Project...</span>
                                    </button>
                                    <button
                                      type="button"
                                      data-testid={`btn-rename-page-${page.id}`}
                                      onClick={(e) => startInlineRename(page.id, page.title || 'Untitled note', 'artifact', e)}
                                      className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-[#FAF9F5] dark:hover:bg-[#252820] text-left text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white cursor-pointer"
                                    >
                                      <Edit3 className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E]" />
                                      <span>Rename</span>
                                    </button>
                                    <div className="h-px bg-[#E8E5DC] dark:bg-[#2C2E27] my-1" />
                                    <button
                                      type="button"
                                      data-testid={`btn-delete-page-${page.id}`}
                                      onClick={async (e) => {
                                        e.stopPropagation();
                                        setOpenMenuId(null);
                                        try {
                                          await deleteArtifact(page.id, proj.id);
                                          await loadTree();
                                          if (activeArtifactId === page.id) onNavigateHome();
                                        } catch (err) {
                                          console.error('Failed to delete note', err);
                                        }
                                      }}
                                      className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-red-50 dark:hover:bg-red-950/30 text-red-600 dark:text-red-400 text-left cursor-pointer"
                                    >
                                      <Trash2 className="w-3.5 h-3.5" />
                                      <span>Delete Note</span>
                                    </button>
                                  </div>
                                )}
                              </div>
                            </div>
                          );
                        })}

                        <button
                          type="button"
                          data-testid="btn-new-page-in-project"
                          onClick={(e) => handleCreatePageInProject(proj.id, e)}
                          className="flex items-center gap-1.5 px-2 py-1 w-full text-[11px] text-[#8C8A82] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white hover:bg-[#EFECE4] dark:hover:bg-[#22241F] rounded-md transition-colors cursor-pointer"
                        >
                          <Plus className="w-3 h-3" />
                          <span>New Page</span>
                        </button>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* 2. NOTES & PAGES SECTION */}
        <div>
          <div className="flex items-center justify-between px-2 pb-1.5">
            <span className="text-[11px] font-bold text-[#8C8A82] dark:text-[#A8A59C] uppercase tracking-wider">
              Notes &amp; Pages
            </span>
            <div className="flex items-center gap-1.5">
              <button
                type="button"
                data-testid="btn-view-all-notes"
                onClick={() => onNavigateWorkspaceIndex?.('notes')}
                className="text-[10px] text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white hover:underline cursor-pointer font-medium"
              >
                View all →
              </button>
              <button
                type="button"
                data-testid="btn-new-loose-note"
                onClick={handleCreateLooseNote}
                title="New Note"
                className="p-0.5 rounded hover:bg-[#EFECE4] dark:hover:bg-[#22241F] text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white transition-colors cursor-pointer"
              >
                <Plus className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>

          {((sidebarItems?.notes?.length ?? 0) === 0 && (tree?.unassociated_pages?.length ?? 0) === 0) ? (
            <div className="px-2 py-2 text-[11px] text-[#8C8A82] dark:text-[#8E8C82] italic">
              No notes yet
            </div>
          ) : (
            <div className="space-y-0.5">
              {(sidebarItems?.notes?.length ? sidebarItems.notes : (tree?.unassociated_pages || []).slice(0, 7).map(n => ({
                id: n.id,
                title: n.title,
                item_type: 'note',
                is_pinned: !!pinnedIds[n.id],
                last_accessed_at: n.updated_at,
                updated_at: n.updated_at,
                project_id: null,
                project_name: null,
              }))).slice(0, 7).map((note) => {
                const isActive = currentView === 'focus' && activeArtifactId === note.id;
                const isPinned = note.is_pinned ?? !!pinnedIds[note.id];

                return (
                  <div
                    key={note.id}
                    onClick={() => onOpenArtifact(note.id, note.project_id)}
                    className={`group/note flex items-center justify-between px-2 py-1.5 rounded-lg cursor-pointer transition-colors ${
                      isActive
                        ? 'bg-[#282C25] dark:bg-[#282B24] text-white font-semibold shadow-xs border border-transparent dark:border-[#3A3D33]'
                        : 'text-[#2E2F2B] dark:text-[#EDEAE2] hover:bg-[#EFECE4] dark:hover:bg-[#22241F] hover:text-[#1E201C] dark:hover:text-white'
                    }`}
                  >
                    <div className="flex items-center gap-2 min-w-0 flex-1">
                      <FileText className="w-3.5 h-3.5 shrink-0 text-[#6E6D66] dark:text-[#A8A69E]" />
                      {renamingId === note.id ? (
                        <input
                          type="text"
                          autoFocus
                          data-testid={`input-rename-loose-page-${note.id}`}
                          value={renamingText}
                          onClick={(e) => e.stopPropagation()}
                          onChange={(e) => setRenamingText(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter') commitRename(note.id, 'artifact');
                            else if (e.key === 'Escape') setRenamingId(null);
                          }}
                          onBlur={() => commitRename(note.id, 'artifact')}
                          className="px-1.5 py-0.5 text-xs rounded border border-[#282C25] dark:border-[#9E9C93] bg-white dark:bg-[#1C1D19] text-[#1E201C] dark:text-[#EDEAE2] outline-none w-28"
                        />
                      ) : (
                        <span className="truncate text-xs flex items-center gap-1 font-medium">
                          <span>{note.title || 'Untitled Note'}</span>
                          {isPinned && (
                            <span
                              data-testid={`pinned-chip-${note.id}`}
                              className="text-[10px] shrink-0"
                              title="Pinned"
                            >
                              📌
                            </span>
                          )}
                        </span>
                      )}
                    </div>

                    <div
                      className="flex items-center gap-1 shrink-0 relative"
                      data-menu-container
                    >
                      {note.project_name && (
                        <span className="text-[9px] px-1 py-0.2 rounded bg-[#E8E5DC]/60 dark:bg-[#252820] text-[#6E6D66] dark:text-[#A8A69E] truncate max-w-[70px]">
                          {note.project_name}
                        </span>
                      )}

                      {/* Quiet pin hover button */}
                      <button
                        type="button"
                        data-testid={`pin-btn-${note.id}`}
                        onClick={(e) => handleTogglePin(note.id, 'note', isPinned, e)}
                        title={isPinned ? 'Unpin note' : 'Pin note'}
                        className={`p-0.5 rounded hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] cursor-pointer transition-colors ${
                          isPinned
                            ? 'opacity-100 text-amber-600 dark:text-amber-400'
                            : 'opacity-0 group-hover/note:opacity-100 text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2]'
                        }`}
                      >
                        <Pin className={`w-3 h-3 ${isPinned ? 'rotate-45' : ''}`} />
                      </button>

                      <button
                        type="button"
                        data-testid={`btn-loose-page-menu-${note.id}`}
                        onClick={(e) => {
                          e.stopPropagation();
                          setOpenMenuId(openMenuId === `note-${note.id}` ? null : `note-${note.id}`);
                        }}
                        title="Note settings"
                        className="opacity-0 group-hover/note:opacity-100 p-0.5 rounded hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] transition-opacity cursor-pointer"
                      >
                        <MoreHorizontal className="w-3 h-3" />
                      </button>

                      {/* Note 3-Dots Popup Menu */}
                      {openMenuId === `note-${note.id}` && (
                        <div className="absolute right-0 top-5 z-50 w-44 bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl shadow-xl py-1 text-xs animate-in fade-in duration-100">
                          <button
                            type="button"
                            data-testid={`btn-pin-loose-page-${note.id}`}
                            onClick={(e) => handleTogglePin(note.id, 'note', isPinned, e)}
                            className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-[#FAF9F5] dark:hover:bg-[#22241F] text-left text-[#3A3B36] dark:text-[#D4D1C7] cursor-pointer"
                          >
                            <Pin className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#7A786F]" />
                            <span>{isPinned ? 'Unpin Note' : 'Pin Note'}</span>
                          </button>
                          <button
                            type="button"
                            data-testid={`btn-duplicate-loose-page-${note.id}`}
                            onClick={(e) => handleDuplicateNoteAction(note.id, note.project_id || undefined, e)}
                            className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-[#FAF9F5] dark:hover:bg-[#252820] text-left text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white cursor-pointer"
                          >
                            <Copy className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E]" />
                            <span>Duplicate</span>
                          </button>
                          <button
                            type="button"
                            data-testid={`btn-add-to-project-loose-page-${note.id}`}
                            onClick={(e) => {
                              e.stopPropagation();
                              setOpenMenuId(null);
                              setAddToProjectTarget({ id: note.id, title: note.title || 'Untitled Note' });
                            }}
                            className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-[#FAF9F5] dark:hover:bg-[#252820] text-left text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white cursor-pointer"
                          >
                            <FolderPlus className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E]" />
                            <span>Add to Project...</span>
                          </button>
                          <button
                            type="button"
                            onClick={(e) => startInlineRename(note.id, note.title || 'Untitled Note', 'artifact', e)}
                            className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-[#FAF9F5] dark:hover:bg-[#252820] text-left text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white cursor-pointer"
                          >
                            <Edit3 className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E]" />
                            <span>Rename</span>
                          </button>
                          <div className="h-px bg-[#E8E5DC] dark:bg-[#2C2E27] my-1" />
                          <button
                            type="button"
                            data-testid={`btn-delete-loose-page-${note.id}`}
                            onClick={async (e) => {
                              e.stopPropagation();
                              setOpenMenuId(null);
                              try {
                                await deleteArtifact(note.id, note.project_id || undefined);
                                await loadTree();
                                if (activeArtifactId === note.id) onNavigateHome();
                              } catch (err) {
                                console.error('Failed to delete note', err);
                              }
                            }}
                            className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-red-50 dark:hover:bg-red-950/30 text-red-600 dark:text-red-400 text-left cursor-pointer"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                            <span>Delete Note</span>
                          </button>
                        </div>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* 3. CUSTOM OBJECTS SECTION */}
        <div>
          <div className="flex items-center justify-between px-2 pb-1.5">
            <span className="text-[11px] font-bold text-[#8C8A82] dark:text-[#A8A59C] uppercase tracking-wider">
              Custom Objects
            </span>
            <div className="flex items-center gap-1.5">
              <button
                type="button"
                data-testid="btn-view-all-objects"
                onClick={() => onNavigateWorkspaceIndex?.('objects')}
                className="text-[10px] text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white hover:underline cursor-pointer font-medium"
              >
                View all →
              </button>
              <button
                type="button"
                data-testid="btn-new-custom-object-type"
                onClick={onOpenCreateObjectType}
                title="New Object Type"
                className="p-0.5 rounded hover:bg-[#EFECE4] dark:hover:bg-[#22241F] text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white transition-colors cursor-pointer"
              >
                <Plus className="w-3.5 h-3.5" />
              </button>
              <button
                type="button"
                data-testid="btn-manage-object-types"
                onClick={onOpenObjectTypeManager}
                title="Manage Object Types & Templates"
                className="p-0.5 rounded hover:bg-[#EFECE4] dark:hover:bg-[#22241F] text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white transition-colors cursor-pointer"
              >
                <Settings2 className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>

          <div className="space-y-0.5">
            {(sidebarItems?.objects?.length ?? 0) > 0 ? (
              (sidebarItems?.objects || []).slice(0, 7).map((obj) => {
                const isActive = currentView === 'focus' && activeArtifactId === obj.id;
                const isPinned = obj.is_pinned ?? !!pinnedIds[obj.id];
                return (
                  <div
                    key={obj.id}
                    onClick={() => onOpenArtifact(obj.id, obj.project_id)}
                    className={`group/obj flex items-center justify-between px-2 py-1.5 rounded-lg cursor-pointer transition-colors ${
                      isActive
                        ? 'bg-[#282C25] dark:bg-[#282B24] text-white font-semibold shadow-xs border border-transparent dark:border-[#3A3D33]'
                        : 'text-[#2E2F2B] dark:text-[#EDEAE2] hover:bg-[#EFECE4] dark:hover:bg-[#22241F] hover:text-[#1E201C] dark:hover:text-white'
                    }`}
                  >
                    <div className="flex items-center gap-2 min-w-0 flex-1">
                      <span className="text-sm shrink-0">{obj.icon || '📦'}</span>
                      <span className="truncate text-xs font-medium flex items-center gap-1">
                        <span>{obj.title}</span>
                        {isPinned && (
                          <span
                            data-testid={`pinned-chip-${obj.id}`}
                            className="text-[10px] shrink-0"
                            title="Pinned"
                          >
                            📌
                          </span>
                        )}
                      </span>
                    </div>

                    <div className="flex items-center gap-1 shrink-0">
                      <span className="text-[9px] uppercase px-1.5 py-0.2 rounded bg-[#E8E5DC]/60 dark:bg-[#252820] text-[#6E6D66] dark:text-[#A8A69E]">
                        {obj.object_type_name || 'Object'}
                      </span>
                      {/* Quiet pin button on hover */}
                      <button
                        type="button"
                        data-testid={`pin-btn-${obj.id}`}
                        onClick={(e) => handleTogglePin(obj.id, 'object', isPinned, e)}
                        title={isPinned ? 'Unpin' : 'Pin'}
                        className={`p-0.5 rounded hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] cursor-pointer transition-colors ${
                          isPinned
                            ? 'opacity-100 text-amber-600 dark:text-amber-400'
                            : 'opacity-0 group-hover/obj:opacity-100 text-[#8C8A82] hover:text-[#1E201C] dark:hover:text-white'
                        }`}
                      >
                        <Pin className={`w-3 h-3 ${isPinned ? 'rotate-45' : ''}`} />
                      </button>
                    </div>
                  </div>
                );
              })
            ) : (!objectTypes || objectTypes.length === 0) ? (
              <div className="px-2 py-1.5 text-[11px] text-[#8C8A82] dark:text-[#8E8C82] italic">
                No custom objects yet
              </div>
            ) : (
              (objectTypes || []).slice(0, 7).map((ot) => {
                const isActive = currentView === 'objects' && activeObjectTypeId === ot.id;
                const count = objectCounts[ot.id] ?? 0;
                const isPinned = !!pinnedIds[ot.id];
                return (
                  <div
                    key={ot.id}
                    onClick={() => onOpenObjectCollection?.(ot.id)}
                    className={`group/ot flex items-center justify-between px-2 py-1.5 rounded-lg cursor-pointer transition-colors ${
                      isActive
                        ? 'bg-[#282C25] dark:bg-[#282B24] text-white font-semibold shadow-xs border border-transparent dark:border-[#3A3D33]'
                        : 'text-[#2E2F2B] dark:text-[#EDEAE2] hover:bg-[#EFECE4] dark:hover:bg-[#22241F] hover:text-[#1E201C] dark:hover:text-white'
                    }`}
                  >
                    <div className="flex items-center gap-2 min-w-0">
                      <span className="text-sm shrink-0">{ot.icon}</span>
                      <span className="truncate text-xs flex items-center gap-1 font-medium">
                        <span>{ot.display_name}s</span>
                        {isPinned && (
                          <span
                            data-testid={`pinned-chip-${ot.id}`}
                            className="text-[10px] shrink-0"
                            title="Pinned"
                          >
                            📌
                          </span>
                        )}
                      </span>
                    </div>
                    <div className="flex items-center gap-1 shrink-0 relative" data-menu-container>
                      <span
                        className={`text-[10px] font-semibold px-1.5 py-0.5 rounded-md ${
                          isActive
                            ? 'bg-white/20 text-white'
                            : 'bg-[#EFECE4] dark:bg-[#22241F] text-[#6E6D66] dark:text-[#B5B2A8]'
                        }`}
                      >
                        {count}
                      </span>
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </div>
      </div>

      {/* Bottom User / Settings Bar */}
      <div className="p-3 border-t border-[#E8E5DC] dark:border-[#262822] flex items-center justify-between bg-[#EFECE4]/50 dark:bg-[#161714]/90">
        <div className="flex items-center gap-1">
          <button
            type="button"
            onClick={onOpenSettings}
            className="flex items-center gap-2 px-2 py-1.5 rounded-lg text-xs text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white hover:bg-[#E5E1D6] dark:hover:bg-[#232520] transition-colors cursor-pointer"
          >
            <SlidersHorizontal className="w-3.5 h-3.5 text-[#6E6D66] dark:text-[#A8A69E]" />
            <span>Settings</span>
          </button>

          <button
            type="button"
            onClick={onOpenAccount}
            className="flex items-center gap-2 px-2 py-1.5 rounded-lg text-xs text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white hover:bg-[#E5E1D6] dark:hover:bg-[#232520] transition-colors cursor-pointer"
          >
            <User className="w-3.5 h-3.5 text-[#6E6D66] dark:text-[#A8A69E]" />
            <span>Account</span>
          </button>
        </div>

        <ThemeToggle />
      </div>
      {/* Add To Project Modal */}
      <AddToProjectModal
        isOpen={!!addToProjectTarget}
        artifactId={addToProjectTarget?.id || null}
        artifactTitle={addToProjectTarget?.title}
        projects={tree.projects}
        onClose={() => setAddToProjectTarget(null)}
        onAdded={async () => {
          setAddToProjectTarget(null);
          await loadTree();
        }}
      />

      {/* Project Deletion Modal */}
      <ProjectDeletionModal
        isOpen={!!projectToDelete}
        projectId={projectToDelete}
        onClose={() => setProjectToDelete(null)}
        onDeleted={async () => {
          const deletedId = projectToDelete;
          setProjectToDelete(null);
          await loadTree();
          if (activeProjectId === deletedId) {
            onNavigateHome();
          }
        }}
      />

      {/* Resizable drag handle */}
      <div
        data-testid="sidebar-resize-handle"
        role="separator"
        aria-orientation="vertical"
        onMouseDown={handleMouseDown}
        onDoubleClick={handleResetWidth}
        title="Drag to resize sidebar (Double-click to reset)"
        className={`absolute top-0 right-0 w-2 h-full cursor-col-resize z-40 transition-colors group ${
          isDragging ? 'bg-[#282C25]/20 dark:bg-[#EDEAE2]/10' : 'hover:bg-[#E8E5DC]/80 dark:hover:bg-[#2C2E27]/80'
        }`}
      >
        <div
          className={`w-0.5 h-full mx-auto transition-colors ${
            isDragging ? 'bg-[#282C25]/50 dark:bg-[#EDEAE2]/30' : 'group-hover:bg-[#8C8A82]/60 dark:group-hover:bg-[#7A786F]/60'
          }`}
        />
      </div>
    </aside>
  );
};

export default LeftSidebar;
