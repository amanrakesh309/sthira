import React, { useState, useEffect, useMemo, useRef } from 'react';
import {
  Search,
  Folder,
  FileText,
  Pin,
  ArrowRight,
  ArrowUpDown,
  ExternalLink,
  ChevronDown,
  Layers,
  Filter,
  SlidersHorizontal,
  Scale,
  Check,
  CheckSquare,
  Lightbulb,
  Link as LinkIcon,
  MoreHorizontal,
  Trash2,
} from 'lucide-react';
import {
  getWorkspaceIndex,
  togglePin,
  addArtifactToProject,
  getWorkspaceTree,
  listObjectTypes,
} from '../api/ipc';
import { PageActionMenu } from '../components/PageActionMenu';
import { ProjectDeletionModal } from '../components/ProjectDeletionModal';
import type {
  WorkspaceIndexData,
  WorkspaceIndexItem,
  WorkspaceIndexTab,
  NoteScopeFilter,
  NoteSortKey,
  ObjectType,
} from '../types/sthira';

interface WorkspaceIndexViewProps {
  initialTab?: WorkspaceIndexTab;
  onOpenProject: (projectId: string) => void;
  onOpenArtifact: (artifactId: string, projectId?: string | null) => void;
  onOpenObjectCollection?: (typeId: string) => void;
  /** Phase 16: Opens the Object Type & Template Manager modal. */
  onOpenObjectTypeManager?: () => void;
  onBack?: () => void;
  onWorkspaceMutated?: () => void;
}

export const WorkspaceIndexView: React.FC<WorkspaceIndexViewProps> = ({
  initialTab = 'all',
  onOpenProject,
  onOpenArtifact,
  onOpenObjectCollection,
  onOpenObjectTypeManager,
  onBack,
  onWorkspaceMutated,
}) => {
  const [activeTab, setActiveTab] = useState<WorkspaceIndexTab>(initialTab);
  const [searchQuery, setSearchQuery] = useState('');
  const [data, setData] = useState<WorkspaceIndexData>({
    counts: { all: 0, projects: 0, notes: 0, objects: 0, standalone: 0 },
    items: [],
  });
  const [isLoading, setIsLoading] = useState(true);
  const [selectedTypeFilter, setSelectedTypeFilter] = useState<string | null>(null);
  const [noteScope, setNoteScope] = useState<NoteScopeFilter>('all');
  const [noteSort, setNoteSort] = useState<NoteSortKey>('updated');
  const [availableObjectTypes, setAvailableObjectTypes] = useState<ObjectType[]>([]);
  const [projectsList, setProjectsList] = useState<{ id: string; name: string }[]>([]);
  const [attachDropdownOpenId, setAttachDropdownOpenId] = useState<string | null>(null);
  const [openProjectMenuId, setOpenProjectMenuId] = useState<string | null>(null);
  const [projectToDeleteId, setProjectToDeleteId] = useState<string | null>(null);

  // Pagination state
  const [currentPage, setCurrentPage] = useState(1);
  const [rowsPerPage, setRowsPerPage] = useState(25);

  const searchInputRef = useRef<HTMLInputElement>(null);

  // Sync tab if initialTab prop changes
  useEffect(() => {
    if (initialTab) {
      setActiveTab(initialTab);
      setCurrentPage(1);
    }
  }, [initialTab]);

  // Global shortcut: Cmd+F / Ctrl+F focuses local search
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'f') {
        const target = e.target as HTMLElement | null;
        if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') && target !== searchInputRef.current) {
          return;
        }
        e.preventDefault();
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  // Close dropdowns on outside click
  useEffect(() => {
    const handleDocumentClick = () => {
      setAttachDropdownOpenId(null);
      setOpenProjectMenuId(null);
    };
    window.addEventListener('click', handleDocumentClick);
    return () => window.removeEventListener('click', handleDocumentClick);
  }, []);

  const loadData = async () => {
    try {
      setIsLoading(true);
      const [indexRes, treeRes, typesRes] = await Promise.all([
        getWorkspaceIndex(activeTab, searchQuery.trim() || undefined),
        getWorkspaceTree(),
        listObjectTypes(),
      ]);
      setData(indexRes);
      setProjectsList(treeRes.projects.map((p) => ({ id: p.id, name: p.name })));
      setAvailableObjectTypes(typesRes);
    } catch (err) {
      console.error('Failed to load workspace index', err);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, [activeTab, searchQuery]);

  const handleTogglePinAction = async (item: WorkspaceIndexItem, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await togglePin(item.id, item.entity_type, !item.is_pinned);
      await loadData();
      onWorkspaceMutated?.();
    } catch (err) {
      console.error('Failed to toggle pin', err);
    }
  };

  const handleAttachNote = async (noteId: string, projectId: string) => {
    try {
      await addArtifactToProject(noteId, projectId);
      setAttachDropdownOpenId(null);
      await loadData();
      onWorkspaceMutated?.();
    } catch (err) {
      console.error('Failed to attach note to project', err);
    }
  };

  const filteredItems = useMemo(() => {
    let list = data.items;

    if (activeTab === 'projects') {
      list = list.filter((item) => item.entity_type === 'project');
    } else if (activeTab === 'notes') {
      list = list.filter((item) => item.entity_type === 'note');
      if (noteScope === 'standalone') {
        list = list.filter((item) => item.standalone);
      } else if (noteScope === 'projects') {
        list = list.filter((item) => !item.standalone);
      }
    } else if (activeTab === 'objects') {
      list = list.filter((item) => item.entity_type === 'object');
      if (selectedTypeFilter) {
        list = list.filter((item) => item.object_type_id === selectedTypeFilter);
      }
    } else if (activeTab === 'all') {
      if (noteScope === 'standalone') {
        list = list.filter((item) => item.standalone || (item.entity_type === 'note' && !item.project_id));
      } else if (noteScope === 'projects') {
        list = list.filter((item) => !item.standalone || item.entity_type === 'project');
      }
      if (selectedTypeFilter) {
        list = list.filter((item) => item.object_type_id === selectedTypeFilter);
      }
    }

    return [...list].sort((a, b) => {
      if (a.is_pinned !== b.is_pinned) return a.is_pinned ? -1 : 1;
      switch (noteSort) {
        case 'created':
          return b.created_at - a.created_at;
        case 'title_asc':
          return a.title.localeCompare(b.title, undefined, { sensitivity: 'base' });
        case 'title_desc':
          return b.title.localeCompare(a.title, undefined, { sensitivity: 'base' });
        case 'updated':
        default:
          return b.updated_at - a.updated_at;
      }
    });
  }, [data.items, activeTab, selectedTypeFilter, noteScope, noteSort]);

  // Pagination calculations
  const totalFiltered = filteredItems.length;
  const totalPages = Math.max(1, Math.ceil(totalFiltered / rowsPerPage));
  const safePage = Math.min(Math.max(1, currentPage), totalPages);

  const paginatedItems = useMemo(() => {
    const start = (safePage - 1) * rowsPerPage;
    return filteredItems.slice(start, start + rowsPerPage);
  }, [filteredItems, safePage, rowsPerPage]);

  const startItem = totalFiltered === 0 ? 0 : (safePage - 1) * rowsPerPage + 1;
  const endItem = Math.min(totalFiltered, safePage * rowsPerPage);

  // Scope counts
  const inProjectNotesCount = Math.max(0, data.counts.notes - data.counts.standalone);
  const inProjectAllCount = Math.max(0, (data.counts.all - data.counts.standalone));

  const isMac = useMemo(() => {
    if (typeof navigator === 'undefined') return true;
    return /Mac|iPhone|iPad/i.test(navigator.platform || navigator.userAgent);
  }, []);

  const sortedObjectTypes = useMemo(() => {
    const priority: Record<string, number> = {
      decision: 1,
      idea: 2,
      source: 3,
      todo: 4,
      task: 4,
    };
    return [...availableObjectTypes].sort((a, b) => {
      const pA = priority[a.id.toLowerCase()] ?? 99;
      const pB = priority[b.id.toLowerCase()] ?? 99;
      if (pA !== pB) return pA - pB;
      return a.display_name.localeCompare(b.display_name);
    });
  }, [availableObjectTypes]);

  const formatDate = (timestamp: number) => {
    if (!timestamp) return 'Just now';
    const date = new Date(timestamp > 1e11 ? timestamp : timestamp * 1000);
    const now = new Date();
    return date.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: date.getFullYear() !== now.getFullYear() ? 'numeric' : undefined,
    });
  };

  const renderEntityIcon = (item: WorkspaceIndexItem) => {
    if (item.entity_type === 'project') {
      return (
        <div className="all-files-entity-icon bg-[#FFF6E5] dark:bg-[#2C2415] text-[#BE8B41]">
          <Folder className="w-4 h-4 fill-current" />
        </div>
      );
    }
    if (item.entity_type === 'object') {
      const otId = item.object_type_id?.toLowerCase();
      if (otId === 'decision') {
        return (
          <div className="all-files-entity-icon bg-[#EEF2FF] dark:bg-[#1E2235] text-[#4F46E5] dark:text-[#818CF8]">
            <Scale className="w-4 h-4" />
          </div>
        );
      }
      if (otId === 'todo' || otId === 'task') {
        return (
          <div className="all-files-entity-icon bg-[#ECFDF5] dark:bg-[#152E22] text-[#059669] dark:text-[#34D399]">
            <CheckSquare className="w-4 h-4" />
          </div>
        );
      }
      if (otId === 'idea') {
        return (
          <div className="all-files-entity-icon bg-[#FFFBEB] dark:bg-[#2D2512] text-[#D97706] dark:text-[#FBBF24]">
            <Lightbulb className="w-4 h-4" />
          </div>
        );
      }
      if (otId === 'source') {
        return (
          <div className="all-files-entity-icon bg-[#EFF6FF] dark:bg-[#182338] text-[#2563EB] dark:text-[#60A5FA]">
            <LinkIcon className="w-4 h-4" />
          </div>
        );
      }
      return (
        <div
          style={{
            backgroundColor: item.color ? `${item.color}20` : undefined,
            color: item.color || undefined,
          }}
          className="all-files-entity-icon bg-sthira-surface-muted text-sthira-text"
        >
          {item.icon || '📦'}
        </div>
      );
    }
    return (
      <div className="all-files-entity-icon bg-[#F5F3EC] dark:bg-[#23251F] text-[#6E6D66] dark:text-[#A8A69E]">
        <FileText className="w-4 h-4" />
      </div>
    );
  };

  const renderTypeBadge = (item: WorkspaceIndexItem) => {
    if (item.entity_type === 'project') {
      return (
        <span className="all-files-badge bg-[#F3E8FF] dark:bg-[#2D1F3D] text-[#7E22CE] dark:text-[#C084FC]">
          <Folder className="w-3 h-3 text-[#7E22CE] dark:text-[#C084FC]" />
          <span>Project</span>
        </span>
      );
    }
    if (item.entity_type === 'object') {
      const otId = item.object_type_id?.toLowerCase();
      const handleClickCollection = (e: React.MouseEvent) => {
        if (item.object_type_id && onOpenObjectCollection) {
          e.stopPropagation();
          onOpenObjectCollection(item.object_type_id);
        }
      };

      if (otId === 'decision') {
        return (
          <span
            onClick={handleClickCollection}
            className={`all-files-badge bg-[#EEF2FF] dark:bg-[#1E2235] text-[#4338CA] dark:text-[#A5B4FC] ${
              onOpenObjectCollection ? 'cursor-pointer hover:underline' : ''
            }`}
          >
            <Scale className="w-3 h-3" />
            <span>Decision</span>
          </span>
        );
      }
      if (otId === 'todo' || otId === 'task') {
        return (
          <span
            onClick={handleClickCollection}
            className={`all-files-badge bg-[#ECFDF5] dark:bg-[#152E22] text-[#047857] dark:text-[#6EE7B7] ${
              onOpenObjectCollection ? 'cursor-pointer hover:underline' : ''
            }`}
          >
            <Check className="w-3 h-3" />
            <span>Task</span>
          </span>
        );
      }
      if (otId === 'idea') {
        return (
          <span
            onClick={handleClickCollection}
            className={`all-files-badge bg-[#FFFBEB] dark:bg-[#2D2512] text-[#B45309] dark:text-[#FCD34D] ${
              onOpenObjectCollection ? 'cursor-pointer hover:underline' : ''
            }`}
          >
            <span>💡</span>
            <span>Idea</span>
          </span>
        );
      }
      if (otId === 'source') {
        return (
          <span
            onClick={handleClickCollection}
            className={`all-files-badge bg-[#EFF6FF] dark:bg-[#182338] text-[#1D4ED8] dark:text-[#93C5FD] ${
              onOpenObjectCollection ? 'cursor-pointer hover:underline' : ''
            }`}
          >
            <span>🔗</span>
            <span>Source</span>
          </span>
        );
      }
      return (
        <span
          onClick={handleClickCollection}
          style={{
            borderColor: item.color || undefined,
            color: item.color || undefined,
          }}
          className={`all-files-badge bg-sthira-surface-muted text-sthira-text border ${
            onOpenObjectCollection ? 'cursor-pointer hover:underline' : ''
          }`}
        >
          <span>{item.icon || '📦'}</span>
          <span>{item.object_type_name || 'Object'}</span>
        </span>
      );
    }
    return (
      <span className="all-files-badge bg-[#F5F3EC] dark:bg-[#23251F] text-[#5C5F56] dark:text-[#C5C8BD]">
        <FileText className="w-3 h-3" />
        <span>Note</span>
      </span>
    );
  };

  return (
    <div
      data-testid="workspace-index-view"
      className="sthira-all-files flex-1 overflow-y-auto"
    >
      <div className="all-files-container">
        {/* Top Header */}
        <div className="all-files-header">
          <div>
            <div className="all-files-eyebrow">
              {onBack ? (
                <button
                  type="button"
                  onClick={onBack}
                  className="hover:text-sthira-text transition-colors cursor-pointer"
                >
                  WORKSPACE
                </button>
              ) : (
                <span>WORKSPACE</span>
              )}
              <span className="text-sthira-text-subtle">/</span>
            </div>

            <div className="all-files-title-row">
              <div className="all-files-folder-icon" aria-hidden="true">
                <Folder className="w-5 h-5 fill-current" />
              </div>
              <h1 className="all-files-heading">All Files</h1>
            </div>
            <p className="all-files-subtitle">
              Unified directory of every project, note, page, and structured object.
            </p>
          </div>

          {/* Local Search + Manage Types */}
          <div className="all-files-header-actions">
            <div className="all-files-search-box">
              <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-sthira-text-muted" />
              <input
                ref={searchInputRef}
                type="text"
                data-testid="search-input"
                value={searchQuery}
                onChange={(e) => {
                  setSearchQuery(e.target.value);
                  setCurrentPage(1);
                }}
                placeholder="Filter by title, content or type..."
                className="all-files-search-input"
              />
              {searchQuery ? (
                <button
                  type="button"
                  onClick={() => {
                    setSearchQuery('');
                    setCurrentPage(1);
                  }}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-xs text-sthira-text-muted hover:text-sthira-text cursor-pointer"
                >
                  Clear
                </button>
              ) : (
                <kbd className="absolute right-3 top-1/2 -translate-y-1/2 text-[10px] bg-sthira-surface-muted px-1.5 py-0.5 rounded text-sthira-text-muted font-mono border border-sthira-border pointer-events-none">
                  {isMac ? '⌘F' : 'Ctrl F'}
                </kbd>
              )}
            </div>

            {onOpenObjectTypeManager && (
              <button
                type="button"
                data-testid="btn-manage-object-types-index"
                onClick={onOpenObjectTypeManager}
                title="Manage Object Types & Templates"
                className="all-files-manage-btn"
              >
                <SlidersHorizontal className="w-4 h-4" />
                <span>Manage Types</span>
              </button>
            )}
          </div>
        </div>

        {/* Category Tabs: All Items / Projects / Notes & Pages / Custom Objects */}
        <div className="all-files-category-tabs" role="tablist" aria-label="Directory Categories">
          {[
            { id: 'all', label: 'All Items', count: data.counts.all },
            { id: 'projects', label: 'Projects', count: data.counts.projects },
            { id: 'notes', label: 'Notes & Pages', count: data.counts.notes },
            { id: 'objects', label: 'Custom Objects', count: data.counts.objects },
          ].map((tab) => {
            const isActive = activeTab === tab.id;
            return (
              <button
                key={tab.id}
                type="button"
                role="tab"
                data-testid={`tab-${tab.id}`}
                aria-selected={isActive}
                onClick={() => {
                  setActiveTab(tab.id as WorkspaceIndexTab);
                  setSelectedTypeFilter(null);
                  setCurrentPage(1);
                }}
                className="all-files-category-tab"
              >
                <span>{tab.label}</span>
                <span className="all-files-tab-badge">
                  {tab.count}
                </span>
              </button>
            );
          })}
        </div>

        {/* Secondary Filter & Sort Toolbar */}
        <div className="all-files-filter-bar">
          <div className="flex flex-wrap items-center gap-4">
            {/* Scope filter: Show: All / In Projects / Standalone */}
            {(activeTab === 'all' || activeTab === 'notes') && (
              <div className="all-files-filter-group" data-testid="note-scope-filter">
                <span className="all-files-filter-label">
                  <Filter className="w-3.5 h-3.5" />
                  <span>Show:</span>
                </span>
                {(
                  [
                    { id: 'all', label: 'All', count: activeTab === 'all' ? data.counts.all : data.counts.notes },
                    { id: 'projects', label: 'In Projects', count: activeTab === 'all' ? inProjectAllCount : inProjectNotesCount },
                    { id: 'standalone', label: 'Standalone', count: data.counts.standalone },
                  ] as { id: NoteScopeFilter; label: string; count: number }[]
                ).map((opt) => {
                  const isActive = noteScope === opt.id;
                  return (
                    <button
                      key={opt.id}
                      type="button"
                      data-testid={`note-scope-${opt.id}`}
                      aria-pressed={isActive}
                      onClick={() => {
                        setNoteScope(opt.id);
                        setCurrentPage(1);
                      }}
                      className="all-files-filter-pill"
                    >
                      <span>{opt.label}</span>
                      <span className="all-files-pill-count">
                        {opt.count}
                      </span>
                    </button>
                  );
                })}
              </div>
            )}

            {/* Registered Object Type filters */}
            {(activeTab === 'all' || activeTab === 'objects') && sortedObjectTypes.length > 0 && (
              <div className="all-files-filter-group">
                <span className="all-files-filter-label">
                  <Layers className="w-3.5 h-3.5" />
                  <span>Type:</span>
                </span>
                {sortedObjectTypes.map((ot) => {
                  const isSelected = selectedTypeFilter === ot.id;
                  return (
                    <button
                      key={ot.id}
                      type="button"
                      data-testid={`type-filter-${ot.id}`}
                      aria-pressed={isSelected}
                      onClick={() => {
                        setSelectedTypeFilter(isSelected ? null : ot.id);
                        setCurrentPage(1);
                      }}
                      className="all-files-filter-pill"
                    >
                      <span>{ot.icon}</span>
                      <span>{ot.display_name}</span>
                    </button>
                  );
                })}
              </div>
            )}
          </div>

          {/* Sort dropdown */}
          <div className="flex items-center gap-2">
            <label
              htmlFor="note-sort-select"
              className="all-files-filter-label"
            >
              <ArrowUpDown className="w-3.5 h-3.5" />
              <span>Sort:</span>
            </label>
            <select
              id="note-sort-select"
              data-testid="note-sort-select"
              value={noteSort}
              title="Pinned items always appear first"
              onChange={(e) => {
                setNoteSort(e.target.value as NoteSortKey);
                setCurrentPage(1);
              }}
              className="all-files-sort-select"
            >
              <option value="updated">Last Modified</option>
              <option value="created">Date Created</option>
              <option value="title_asc">Title (A–Z)</option>
              <option value="title_desc">Title (Z–A)</option>
            </select>
          </div>
        </div>

        {/* Content Section */}
        {isLoading ? (
          <div className="py-16 text-center text-xs text-sthira-text-muted">
            Loading workspace directory...
          </div>
        ) : filteredItems.length === 0 ? (
          <div className="py-16 all-files-card text-center p-8 space-y-2">
            <div className="w-10 h-10 rounded-xl bg-sthira-surface-muted text-sthira-text-muted flex items-center justify-center mx-auto">
              <Layers className="w-5 h-5" />
            </div>
            <h3 className="text-sm font-bold text-sthira-text">
              No items found
            </h3>
            <p className="text-xs text-sthira-text-muted max-w-sm mx-auto">
              {searchQuery
                ? `No matches for "${searchQuery}". Try clearing search.`
                : noteScope !== 'all'
                ? `No ${noteScope === 'standalone' ? 'standalone' : 'project-attached'} items found. Switch the "Show" filter to see the rest.`
                : selectedTypeFilter
                ? 'No items found matching the selected type filter.'
                : 'There are no items in this category yet.'}
            </p>
          </div>
        ) : (
          <div>
            {/* PROJECTS TAB (Card Grid Layout) */}
            {activeTab === 'projects' ? (
              <div className="space-y-4">
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                  {paginatedItems.map((item) => (
                    <div
                      key={item.id}
                      data-testid={`index-item-${item.id}`}
                      tabIndex={0}
                      role="button"
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' || e.key === ' ') {
                          e.preventDefault();
                          onOpenProject(item.id);
                        }
                      }}
                      onClick={() => onOpenProject(item.id)}
                      className="all-files-project-card group"
                    >
                      <div className="space-y-3">
                        <div className="flex items-center justify-between gap-2">
                          <div className="flex items-center gap-2.5 min-w-0">
                            <div className="all-files-entity-icon bg-[#FFF6E5] dark:bg-[#2C2415] text-[#BE8B41]">
                              <Folder className="w-4 h-4 fill-current" />
                            </div>
                            <div className="min-w-0">
                              <h3 className="font-bold text-sm text-sthira-text group-hover:text-amber-700 dark:group-hover:text-amber-300 transition-colors truncate">
                                {item.title}
                              </h3>
                              <span className="text-[11px] text-sthira-text-muted">
                                {item.item_count ?? 0} {item.item_count === 1 ? 'item' : 'items'}
                              </span>
                            </div>
                          </div>

                          <div className="flex items-center gap-1">
                            {/* Pin Button */}
                            <button
                              type="button"
                              data-testid={`pin-btn-${item.id}`}
                              onClick={(e) => handleTogglePinAction(item, e)}
                              title={item.is_pinned ? 'Unpin project' : 'Pin project'}
                              className={`p-1.5 rounded-lg transition-colors cursor-pointer ${
                                item.is_pinned
                                  ? 'text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-950/30'
                                  : 'text-sthira-text-muted hover:text-sthira-text hover:bg-sthira-surface-muted'
                              }`}
                            >
                              <Pin className={`w-3.5 h-3.5 ${item.is_pinned ? 'rotate-45' : ''}`} />
                            </button>

                            {/* Project Options Menu */}
                            <div className="relative">
                              <button
                                type="button"
                                data-testid={`project-menu-btn-${item.id}`}
                                onClick={(e) => {
                                  e.stopPropagation();
                                  setOpenProjectMenuId(openProjectMenuId === item.id ? null : item.id);
                                }}
                                title="Project options"
                                className="p-1.5 rounded-lg text-sthira-text-muted hover:text-sthira-text hover:bg-sthira-surface-muted transition-colors cursor-pointer"
                              >
                                <MoreHorizontal className="w-3.5 h-3.5" />
                              </button>
                              {openProjectMenuId === item.id && (
                                <div
                                  className="absolute right-0 top-7 z-50 w-36 bg-sthira-surface border border-sthira-border rounded-xl shadow-xl py-1 text-xs animate-in fade-in duration-100"
                                  onClick={(e) => e.stopPropagation()}
                                >
                                  <button
                                    type="button"
                                    onClick={() => {
                                      setOpenProjectMenuId(null);
                                      onOpenProject(item.id);
                                    }}
                                    className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-sthira-surface-muted text-left text-sthira-text cursor-pointer"
                                  >
                                    <ExternalLink className="w-3.5 h-3.5" />
                                    <span>Open</span>
                                  </button>
                                  <button
                                    type="button"
                                    data-testid={`delete-project-${item.id}`}
                                    onClick={() => {
                                      setOpenProjectMenuId(null);
                                      setProjectToDeleteId(item.id);
                                    }}
                                    className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-red-500/10 text-left text-red-600 dark:text-red-400 cursor-pointer"
                                  >
                                    <Trash2 className="w-3.5 h-3.5" />
                                    <span>Delete</span>
                                  </button>
                                </div>
                              )}
                            </div>
                          </div>
                        </div>

                        {item.description && (
                          <p className="text-xs text-sthira-text-muted line-clamp-2 leading-relaxed">
                            {item.description}
                          </p>
                        )}
                      </div>

                      <div className="pt-3 mt-3 border-t border-sthira-border flex items-center justify-between text-xs text-sthira-text-muted">
                        <span>Updated {formatDate(item.updated_at)}</span>
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            onOpenProject(item.id);
                          }}
                          className="inline-flex items-center gap-1 font-semibold text-sthira-text hover:underline cursor-pointer"
                        >
                          <span>Open</span>
                          <ArrowRight className="w-3.5 h-3.5 group-hover:translate-x-0.5 transition-transform" />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>

                {/* Footer / Pagination */}
                <div className="all-files-footer rounded-2xl border border-sthira-border">
                  <div>
                    Showing {startItem}–{endItem} of {totalFiltered} items
                  </div>
                  <div className="flex items-center gap-4">
                    <div className="flex items-center gap-1.5 text-xs text-sthira-text-muted">
                      <span>Rows per page:</span>
                      <select
                        value={rowsPerPage}
                        onChange={(e) => {
                          setRowsPerPage(Number(e.target.value));
                          setCurrentPage(1);
                        }}
                        className="px-2 py-0.5 rounded border border-sthira-border bg-sthira-surface text-sthira-text outline-none text-xs cursor-pointer"
                      >
                        <option value={10}>10</option>
                        <option value={25}>25</option>
                        <option value={50}>50</option>
                      </select>
                    </div>
                    <div className="all-files-pagination">
                      <button
                        type="button"
                        onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
                        disabled={safePage <= 1}
                        className="all-files-page-btn"
                        title="Previous Page"
                      >
                        &lt;
                      </button>
                      {Array.from({ length: totalPages }).map((_, i) => {
                        const pageNum = i + 1;
                        if (totalPages > 5 && Math.abs(pageNum - safePage) > 2 && pageNum !== 1 && pageNum !== totalPages) {
                          if (pageNum === 2 || pageNum === totalPages - 1) {
                            return <span key={pageNum} className="text-sthira-text-subtle px-1">…</span>;
                          }
                          return null;
                        }
                        return (
                          <button
                            key={pageNum}
                            type="button"
                            aria-current={safePage === pageNum ? 'page' : undefined}
                            onClick={() => setCurrentPage(pageNum)}
                            className="all-files-page-btn"
                          >
                            {pageNum}
                          </button>
                        );
                      })}
                      <button
                        type="button"
                        onClick={() => setCurrentPage((p) => Math.min(totalPages, p + 1))}
                        disabled={safePage >= totalPages}
                        className="all-files-page-btn"
                        title="Next Page"
                      >
                        &gt;
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            ) : (
              /* ALL ITEMS / NOTES / CUSTOM OBJECTS TABLE */
              <div className="all-files-card">
                {/* Table Column Headers */}
                <div className="all-files-table-header">
                  <span className="flex-1">Title &amp; Description</span>
                  <span className="w-24 text-left hidden sm:inline-block">Type</span>
                  <span className="w-36 text-left hidden lg:inline-block">Project</span>
                  <span className="w-20 text-left hidden sm:inline-block">Modified</span>
                  <span className="w-24 text-right"></span>
                </div>

                {/* Table Data Rows */}
                <div className="divide-y divide-sthira-border">
                  {paginatedItems.map((item) => {
                    const isProj = item.entity_type === 'project';

                    return (
                      <div
                        key={item.id}
                        data-testid={`index-item-${item.id}`}
                        tabIndex={0}
                        role="button"
                        onKeyDown={(e) => {
                          if (e.key === 'Enter' || e.key === ' ') {
                            e.preventDefault();
                            if (isProj) onOpenProject(item.id);
                            else onOpenArtifact(item.id, item.project_id);
                          }
                        }}
                        onClick={() => {
                          if (isProj) onOpenProject(item.id);
                          else onOpenArtifact(item.id, item.project_id);
                        }}
                        className="all-files-table-row group"
                      >
                        {/* Icon + Title + Description */}
                        <div className="flex items-center gap-3 min-w-0 flex-1 pr-4">
                          {renderEntityIcon(item)}
                          <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-2">
                              <h3 className="font-semibold text-sm text-sthira-text truncate group-hover:underline">
                                {item.title}
                              </h3>
                              {item.is_pinned && (
                                <span className="text-[10px] px-1.5 py-0.2 rounded bg-amber-50 dark:bg-amber-950/40 text-amber-600 dark:text-amber-400 font-medium inline-flex items-center gap-1 shrink-0">
                                  <Pin className="w-2.5 h-2.5 rotate-45" />
                                  <span>Pinned</span>
                                </span>
                              )}
                            </div>
                            {item.description && (
                              <p className="text-xs text-sthira-text-muted truncate mt-0.5 max-w-xl">
                                {item.description}
                              </p>
                            )}
                          </div>
                        </div>

                        {/* Type badge */}
                        <div className="w-24 shrink-0 hidden sm:block">
                          {renderTypeBadge(item)}
                        </div>

                        {/* Project */}
                        <div className="w-36 shrink-0 hidden lg:block">
                          {item.project_name ? (
                            <button
                              type="button"
                              onClick={(e) => {
                                e.stopPropagation();
                                if (item.project_id) onOpenProject(item.project_id);
                              }}
                              className="inline-flex items-center gap-1.5 text-xs font-medium text-sthira-text hover:underline max-w-[140px] truncate cursor-pointer"
                            >
                              <span className="truncate">📁 {item.project_name}</span>
                            </button>
                          ) : (
                            <span className="text-xs text-sthira-text-subtle font-mono">—</span>
                          )}
                        </div>

                        {/* Modified Date */}
                        <div className="w-20 shrink-0 text-xs text-sthira-text-muted hidden sm:block">
                          {formatDate(item.updated_at)}
                        </div>

                        {/* Actions */}
                        <div
                          className="w-24 shrink-0 flex items-center justify-end gap-1"
                          onClick={(e) => e.stopPropagation()}
                        >
                          {/* Standalone attach button if note has no project */}
                          {!item.project_name && item.entity_type === 'note' && (
                            <div className="relative">
                              <button
                                type="button"
                                data-testid={`attach-project-btn-${item.id}`}
                                onClick={() =>
                                  setAttachDropdownOpenId(
                                    attachDropdownOpenId === item.id ? null : item.id
                                  )
                                }
                                title="Attach to Project"
                                className="p-1.5 rounded-lg text-sthira-text-subtle hover:text-sthira-text hover:bg-sthira-surface-muted transition-colors cursor-pointer"
                              >
                                <ChevronDown className="w-3.5 h-3.5" />
                              </button>

                              {attachDropdownOpenId === item.id && (
                                <div className="absolute right-0 top-7 z-50 w-52 bg-sthira-surface border border-sthira-border rounded-xl shadow-xl py-1 text-xs animate-in fade-in duration-100 max-h-48 overflow-y-auto">
                                  <div className="px-3 py-1 text-[10px] font-bold text-sthira-text-subtle uppercase">
                                    Select Target Project
                                  </div>
                                  {projectsList.length === 0 ? (
                                    <div className="px-3 py-2 text-[11px] text-sthira-text-subtle italic">
                                      No projects exist yet
                                    </div>
                                  ) : (
                                    projectsList.map((p) => (
                                      <button
                                        key={p.id}
                                        type="button"
                                        data-testid={`attach-target-${p.id}`}
                                        onClick={() => handleAttachNote(item.id, p.id)}
                                        className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-sthira-surface-muted text-left text-sthira-text cursor-pointer"
                                      >
                                        <Folder className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400 shrink-0" />
                                        <span className="truncate">{p.name}</span>
                                      </button>
                                    ))
                                  )}
                                </div>
                              )}
                            </div>
                          )}

                          {isProj ? (
                            <div className="relative">
                              <button
                                type="button"
                                data-testid={`project-menu-btn-${item.id}`}
                                onClick={() =>
                                  setOpenProjectMenuId(
                                    openProjectMenuId === item.id ? null : item.id
                                  )
                                }
                                title="Project actions"
                                className="p-1.5 rounded-lg text-sthira-text-muted hover:text-sthira-text hover:bg-sthira-surface-muted transition-colors cursor-pointer"
                              >
                                <MoreHorizontal className="w-3.5 h-3.5" />
                              </button>
                              {openProjectMenuId === item.id && (
                                <div
                                  className="absolute right-0 top-7 z-50 w-36 bg-sthira-surface border border-sthira-border rounded-xl shadow-xl py-1 text-xs animate-in fade-in duration-100"
                                  onClick={(e) => e.stopPropagation()}
                                >
                                  <button
                                    type="button"
                                    onClick={() => {
                                      setOpenProjectMenuId(null);
                                      onOpenProject(item.id);
                                    }}
                                    className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-sthira-surface-muted text-left text-sthira-text cursor-pointer"
                                  >
                                    <ExternalLink className="w-3.5 h-3.5" />
                                    <span>Open</span>
                                  </button>
                                  <button
                                    type="button"
                                    data-testid={`delete-project-${item.id}`}
                                    onClick={() => {
                                      setOpenProjectMenuId(null);
                                      setProjectToDeleteId(item.id);
                                    }}
                                    className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-red-500/10 text-left text-red-600 dark:text-red-400 cursor-pointer"
                                  >
                                    <Trash2 className="w-3.5 h-3.5" />
                                    <span>Delete</span>
                                  </button>
                                </div>
                              )}
                            </div>
                          ) : (
                            <PageActionMenu
                              variant="row"
                              align="right"
                              artifactId={item.id}
                              artifactTitle={item.title}
                              projectId={item.project_id ?? null}
                              onChanged={() => loadData()}
                              onOpenArtifact={(id, pid) => onOpenArtifact(id, pid ?? null)}
                              onProjectCreated={(newProjectId) => {
                                loadData();
                                onOpenProject(newProjectId);
                              }}
                              onDeleted={() => {
                                loadData();
                                onWorkspaceMutated?.();
                              }}
                            />
                          )}

                          <button
                            type="button"
                            data-testid={`pin-btn-${item.id}`}
                            onClick={(e) => handleTogglePinAction(item, e)}
                            title={item.is_pinned ? 'Unpin' : 'Pin'}
                            className={`p-1.5 rounded-lg transition-colors cursor-pointer ${
                              item.is_pinned
                                ? 'text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-950/30'
                                : 'text-sthira-text-muted hover:text-sthira-text hover:bg-sthira-surface-muted opacity-0 group-hover:opacity-100 group-focus-within:opacity-100'
                            }`}
                          >
                            <Pin className={`w-3.5 h-3.5 ${item.is_pinned ? 'rotate-45' : ''}`} />
                          </button>

                          <button
                            type="button"
                            onClick={(e) => {
                              e.stopPropagation();
                              if (isProj) onOpenProject(item.id);
                              else onOpenArtifact(item.id, item.project_id);
                            }}
                            title="Open"
                            className="p-1.5 rounded-lg text-sthira-text-muted hover:text-sthira-text hover:bg-sthira-surface-muted transition-colors cursor-pointer"
                          >
                            <ExternalLink className="w-3.5 h-3.5" />
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>

                {/* Directory Footer & Pagination */}
                <div className="all-files-footer">
                  <div>
                    Showing {startItem}–{endItem} of {totalFiltered} items
                  </div>
                  <div className="flex items-center gap-4">
                    <div className="flex items-center gap-1.5 text-xs text-sthira-text-muted">
                      <span>Rows per page:</span>
                      <select
                        value={rowsPerPage}
                        onChange={(e) => {
                          setRowsPerPage(Number(e.target.value));
                          setCurrentPage(1);
                        }}
                        className="px-2 py-0.5 rounded border border-sthira-border bg-sthira-surface text-sthira-text outline-none text-xs cursor-pointer"
                      >
                        <option value={10}>10</option>
                        <option value={25}>25</option>
                        <option value={50}>50</option>
                      </select>
                    </div>
                    <div className="all-files-pagination">
                      <button
                        type="button"
                        onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
                        disabled={safePage <= 1}
                        className="all-files-page-btn"
                        title="Previous Page"
                      >
                        &lt;
                      </button>
                      {Array.from({ length: totalPages }).map((_, i) => {
                        const pageNum = i + 1;
                        if (totalPages > 5 && Math.abs(pageNum - safePage) > 2 && pageNum !== 1 && pageNum !== totalPages) {
                          if (pageNum === 2 || pageNum === totalPages - 1) {
                            return <span key={pageNum} className="text-sthira-text-subtle px-1">…</span>;
                          }
                          return null;
                        }
                        return (
                          <button
                            key={pageNum}
                            type="button"
                            aria-current={safePage === pageNum ? 'page' : undefined}
                            onClick={() => setCurrentPage(pageNum)}
                            className="all-files-page-btn"
                          >
                            {pageNum}
                          </button>
                        );
                      })}
                      <button
                        type="button"
                        onClick={() => setCurrentPage((p) => Math.min(totalPages, p + 1))}
                        disabled={safePage >= totalPages}
                        className="all-files-page-btn"
                        title="Next Page"
                      >
                        &gt;
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            )}
          </div>
        )}

        {/* Project Deletion Modal */}
        <ProjectDeletionModal
          isOpen={!!projectToDeleteId}
          projectId={projectToDeleteId}
          onClose={() => setProjectToDeleteId(null)}
          onDeleted={async () => {
            setProjectToDeleteId(null);
            await loadData();
            onWorkspaceMutated?.();
          }}
        />
      </div>
    </div>
  );
};

export default WorkspaceIndexView;