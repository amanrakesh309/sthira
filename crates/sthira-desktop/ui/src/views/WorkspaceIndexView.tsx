import React, { useState, useEffect, useMemo } from 'react';
import {
  Search,
  Folder,
  FileText,
  Pin,
  ArrowRight,
  ExternalLink,
  ChevronDown,
  Layers,
  Filter,
} from 'lucide-react';
import {
  getWorkspaceIndex,
  togglePin,
  addArtifactToProject,
  getWorkspaceTree,
  listObjectTypes,
} from '../api/ipc';
import type {
  WorkspaceIndexData,
  WorkspaceIndexItem,
  WorkspaceIndexTab,
  ObjectType,
} from '../types/sthira';

interface WorkspaceIndexViewProps {
  initialTab?: WorkspaceIndexTab;
  onOpenProject: (projectId: string) => void;
  onOpenArtifact: (artifactId: string, projectId?: string | null) => void;
  onOpenObjectCollection?: (typeId: string) => void;
  onBack?: () => void;
  onWorkspaceMutated?: () => void;
}

export const WorkspaceIndexView: React.FC<WorkspaceIndexViewProps> = ({
  initialTab = 'all',
  onOpenProject,
  onOpenArtifact,
  onOpenObjectCollection,
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
  const [availableObjectTypes, setAvailableObjectTypes] = useState<ObjectType[]>([]);
  const [projectsList, setProjectsList] = useState<{ id: string; name: string }[]>([]);
  const [attachDropdownOpenId, setAttachDropdownOpenId] = useState<string | null>(null);

  // Sync tab if initialTab prop changes
  useEffect(() => {
    if (initialTab) {
      setActiveTab(initialTab);
    }
  }, [initialTab]);

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
    if (activeTab === 'objects' && selectedTypeFilter) {
      return data.items.filter((item) => item.object_type_id === selectedTypeFilter);
    }
    return data.items;
  }, [data.items, activeTab, selectedTypeFilter]);

  const formatDate = (timestamp: number) => {
    if (!timestamp) return 'Just now';
    const date = new Date(timestamp > 1e11 ? timestamp : timestamp * 1000);
    return date.toLocaleDateString(undefined, {
      month: 'short',
      day: 'numeric',
      year: date.getFullYear() !== new Date().getFullYear() ? 'numeric' : undefined,
    });
  };

  return (
    <div
      data-testid="workspace-index-view"
      className="flex-1 min-h-screen bg-[#FAF8F3] dark:bg-[#121310] text-[#1E201C] dark:text-[#EDEAE2] flex flex-col transition-colors overflow-y-auto"
    >
      <div className="max-w-6xl w-full mx-auto px-6 py-8 flex flex-col space-y-6">
        {/* Top Header & Breadcrumbs */}
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 border-b border-[#E8E5DC] dark:border-[#2C2E27] pb-6">
          <div className="space-y-1">
            <div className="flex items-center gap-2 text-xs font-semibold text-[#8C8A82] dark:text-[#A8A59C] uppercase tracking-wider">
              {onBack && (
                <button
                  type="button"
                  onClick={onBack}
                  className="hover:text-[#1E201C] dark:hover:text-white transition-colors cursor-pointer"
                >
                  Workspace
                </button>
              )}
              {!onBack && <span>Workspace</span>}
              <span>/</span>
              <span className="text-[#1E201C] dark:text-[#EDEAE2]">Index</span>
            </div>
            <h1 className="text-2xl font-bold tracking-tight text-[#1E201C] dark:text-[#EDEAE2] flex items-center gap-2.5">
              <span>📊 Workspace Index</span>
            </h1>
            <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93]">
              Unified directory of all projects, notes, standalone pages, and structured objects.
            </p>
          </div>

          {/* Search Box */}
          <div className="relative w-full md:w-80">
            <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-[#8C8A82] dark:text-[#7A786F]" />
            <input
              type="text"
              data-testid="search-input"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Filter by title, content or type..."
              className="w-full pl-9 pr-4 py-2 bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl text-xs text-[#1E201C] dark:text-[#EDEAE2] placeholder:text-[#8C8A82] dark:placeholder:text-[#6E6D66] outline-none focus:border-[#282C25] dark:focus:border-[#9E9C93] transition-colors shadow-2xs"
            />
            {searchQuery && (
              <button
                type="button"
                onClick={() => setSearchQuery('')}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[10px] text-[#8C8A82] hover:text-[#1E201C] dark:hover:text-white"
              >
                Clear
              </button>
            )}
          </div>
        </div>

        {/* Tab Switcher */}
        <div className="flex items-center gap-1.5 overflow-x-auto pb-1 select-none border-b border-[#E8E5DC]/60 dark:border-[#2C2E27]/60">
          {[
            { id: 'all', label: 'All Items', count: data.counts.all },
            { id: 'projects', label: 'Projects', count: data.counts.projects },
            { id: 'notes', label: 'Notes & Pages', count: data.counts.notes },
            { id: 'objects', label: 'Custom Objects', count: data.counts.objects },
            { id: 'standalone', label: 'Standalone Notes', count: data.counts.standalone },
          ].map((tab) => {
            const isActive = activeTab === tab.id;
            return (
              <button
                key={tab.id}
                type="button"
                data-testid={`tab-${tab.id}`}
                onClick={() => {
                  setActiveTab(tab.id as WorkspaceIndexTab);
                  setSelectedTypeFilter(null);
                }}
                className={`flex items-center gap-2 px-3.5 py-2 rounded-xl text-xs font-semibold transition-all cursor-pointer whitespace-nowrap ${
                  isActive
                    ? 'bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] shadow-2xs'
                    : 'bg-white/70 dark:bg-[#1C1D19]/70 text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white hover:bg-white dark:hover:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27]'
                }`}
              >
                <span>{tab.label}</span>
                <span
                  className={`text-[10px] px-1.5 py-0.2 rounded-full font-mono ${
                    isActive
                      ? 'bg-white/20 dark:bg-black/10 text-white dark:text-[#121310]'
                      : 'bg-[#EFECE4] dark:bg-[#252820] text-[#7A7870] dark:text-[#B5B2A8]'
                  }`}
                >
                  {tab.count}
                </span>
              </button>
            );
          })}
        </div>

        {/* Custom Objects Type Filter Bar */}
        {activeTab === 'objects' && (
          <div className="flex items-center gap-1.5 overflow-x-auto py-1">
            <span className="text-[11px] font-semibold text-[#8C8A82] dark:text-[#7A786F] flex items-center gap-1 mr-1">
              <Filter className="w-3 h-3" />
              <span>Type:</span>
            </span>
            <button
              type="button"
              onClick={() => setSelectedTypeFilter(null)}
              className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
                selectedTypeFilter === null
                  ? 'bg-[#EFECE4] dark:bg-[#2A2D24] text-[#1E201C] dark:text-white font-semibold'
                  : 'text-[#6E6D66] dark:text-[#A8A69E] hover:bg-[#EFECE4]/50 dark:hover:bg-[#22241F]'
              }`}
            >
              All Types
            </button>
            {availableObjectTypes.map((ot) => {
              const isSelected = selectedTypeFilter === ot.id;
              return (
                <button
                  key={ot.id}
                  type="button"
                  onClick={() => setSelectedTypeFilter(isSelected ? null : ot.id)}
                  className={`flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-xs font-medium transition-colors cursor-pointer border ${
                    isSelected
                      ? 'bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] border-transparent font-semibold'
                      : 'bg-white dark:bg-[#1C1D19] border-[#E8E5DC] dark:border-[#2C2E27] text-[#3A3B36] dark:text-[#EDEAE2] hover:border-[#D4D1C7]'
                  }`}
                >
                  <span>{ot.icon}</span>
                  <span>{ot.display_name}</span>
                </button>
              );
            })}
          </div>
        )}

        {/* Content Section */}
        {isLoading ? (
          <div className="py-16 text-center text-xs text-[#8C8A82] dark:text-[#7A786F]">
            Loading workspace directory...
          </div>
        ) : filteredItems.length === 0 ? (
          <div className="py-16 bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl text-center p-8 space-y-2">
            <div className="w-10 h-10 rounded-xl bg-[#FAF8F3] dark:bg-[#22241F] text-[#8C8A82] flex items-center justify-center mx-auto">
              <Layers className="w-5 h-5" />
            </div>
            <h3 className="text-sm font-bold text-[#1E201C] dark:text-[#EDEAE2]">
              No items found
            </h3>
            <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93] max-w-sm mx-auto">
              {searchQuery
                ? `No matches for "${searchQuery}". Try clearing search.`
                : 'There are no items in this category yet.'}
            </p>
          </div>
        ) : (
          /* Render based on active tab */
          <div className="space-y-4">
            {/* PROJECTS TAB (Card Grid Layout) */}
            {activeTab === 'projects' && (
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {filteredItems.map((item) => (
                  <div
                    key={item.id}
                    data-testid={`index-item-${item.id}`}
                    onClick={() => onOpenProject(item.id)}
                    className="bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl p-4.5 hover:border-[#D4D1C7] dark:hover:border-[#383B32] hover:shadow-md transition-all cursor-pointer flex flex-col justify-between group relative"
                  >
                    <div className="space-y-2.5">
                      <div className="flex items-center justify-between gap-2">
                        <div className="flex items-center gap-2">
                          <div className="w-8 h-8 rounded-lg bg-[#FAF8F3] dark:bg-[#22241F] flex items-center justify-center text-amber-600 dark:text-amber-400">
                            <Folder className="w-4 h-4" />
                          </div>
                          <div>
                            <h3 className="font-bold text-sm text-[#1E201C] dark:text-[#EDEAE2] group-hover:text-amber-700 dark:group-hover:text-amber-300 transition-colors line-clamp-1">
                              {item.title}
                            </h3>
                            <span className="text-[10px] text-[#8C8A82] dark:text-[#7A786F]">
                              {item.item_count ?? 0} {item.item_count === 1 ? 'item' : 'items'}
                            </span>
                          </div>
                        </div>

                        {/* Pin Button */}
                        <button
                          type="button"
                          data-testid={`pin-btn-${item.id}`}
                          onClick={(e) => handleTogglePinAction(item, e)}
                          title={item.is_pinned ? 'Unpin project' : 'Pin project'}
                          className={`p-1.5 rounded-lg transition-colors cursor-pointer ${
                            item.is_pinned
                              ? 'text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-950/30'
                              : 'text-[#8C8A82] hover:text-[#1E201C] dark:hover:text-white hover:bg-[#FAF8F3] dark:hover:bg-[#22241F]'
                          }`}
                        >
                          <Pin className={`w-3.5 h-3.5 ${item.is_pinned ? 'rotate-45' : ''}`} />
                        </button>
                      </div>

                      {item.description && (
                        <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93] line-clamp-2 leading-relaxed">
                          {item.description}
                        </p>
                      )}
                    </div>

                    <div className="pt-4 mt-2 border-t border-[#E8E5DC]/60 dark:border-[#2C2E27]/60 flex items-center justify-between text-[11px] text-[#8C8A82] dark:text-[#7A786F]">
                      <span>Updated {formatDate(item.updated_at)}</span>
                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          onOpenProject(item.id);
                        }}
                        className="inline-flex items-center gap-1 font-semibold text-[#1E201C] dark:text-[#EDEAE2] hover:underline"
                      >
                        <span>Open</span>
                        <ArrowRight className="w-3 h-3 group-hover:translate-x-0.5 transition-transform" />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}

            {/* NOTES & PAGES TAB (Table / List Layout) */}
            {activeTab === 'notes' && (
              <div className="bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl overflow-hidden shadow-2xs divide-y divide-[#E8E5DC]/60 dark:divide-[#2C2E27]/60">
                {filteredItems.map((item) => (
                  <div
                    key={item.id}
                    data-testid={`index-item-${item.id}`}
                    onClick={() => onOpenArtifact(item.id, item.project_id)}
                    className="p-4 hover:bg-[#FAF8F3] dark:hover:bg-[#181916] transition-colors cursor-pointer flex items-center justify-between gap-4 group"
                  >
                    <div className="flex items-center gap-3 min-w-0 flex-1">
                      <div className="w-8 h-8 rounded-lg bg-[#FAF8F3] dark:bg-[#22241F] flex items-center justify-center text-[#6E6D66] dark:text-[#A8A69E] shrink-0">
                        <FileText className="w-4 h-4" />
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <h3 className="font-semibold text-xs text-[#1E201C] dark:text-[#EDEAE2] truncate group-hover:underline">
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
                          <p className="text-[11px] text-[#8C8A82] dark:text-[#7A786F] truncate mt-0.5 max-w-xl">
                            {item.description}
                          </p>
                        )}
                      </div>
                    </div>

                    <div className="flex items-center gap-3 shrink-0">
                      {/* Parent Project badge */}
                      {item.project_name ? (
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            if (item.project_id) onOpenProject(item.project_id);
                          }}
                          className="text-[11px] px-2 py-0.5 rounded-lg bg-[#EFECE4] dark:bg-[#22241F] text-[#4A4B45] dark:text-[#D4D1C7] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] font-medium flex items-center gap-1 transition-colors cursor-pointer"
                        >
                          <Folder className="w-3 h-3 text-amber-600 dark:text-amber-400" />
                          <span className="truncate max-w-[120px]">{item.project_name}</span>
                        </button>
                      ) : (
                        <span className="text-[11px] px-2 py-0.5 rounded-lg bg-[#FAF8F3] dark:bg-[#1E201A] text-[#8C8A82] dark:text-[#7A786F] border border-[#E8E5DC] dark:border-[#2C2E27]">
                          Standalone
                        </span>
                      )}

                      <span className="text-[11px] text-[#8C8A82] dark:text-[#7A786F] w-20 text-right">
                        {formatDate(item.updated_at)}
                      </span>

                      {/* Pin Button */}
                      <button
                        type="button"
                        data-testid={`pin-btn-${item.id}`}
                        onClick={(e) => handleTogglePinAction(item, e)}
                        title={item.is_pinned ? 'Unpin note' : 'Pin note'}
                        className={`p-1.5 rounded-lg transition-colors cursor-pointer ${
                          item.is_pinned
                            ? 'text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-950/30'
                            : 'text-[#8C8A82] hover:text-[#1E201C] dark:hover:text-white hover:bg-[#EFECE4] dark:hover:bg-[#22241F]'
                        }`}
                      >
                        <Pin className={`w-3.5 h-3.5 ${item.is_pinned ? 'rotate-45' : ''}`} />
                      </button>

                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          onOpenArtifact(item.id, item.project_id);
                        }}
                        className="p-1 rounded-md text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white"
                      >
                        <ExternalLink className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}

            {/* CUSTOM OBJECTS TAB */}
            {activeTab === 'objects' && (
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3.5">
                {filteredItems.map((item) => (
                  <div
                    key={item.id}
                    data-testid={`index-item-${item.id}`}
                    onClick={() => onOpenArtifact(item.id, item.project_id)}
                    className="bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl p-4 hover:border-[#D4D1C7] dark:hover:border-[#383B32] hover:shadow-xs transition-all cursor-pointer flex flex-col justify-between group space-y-3"
                  >
                    <div className="space-y-2">
                      <div className="flex items-center justify-between gap-2">
                        <div className="flex items-center gap-2">
                          <span className="text-lg shrink-0">{item.icon || '📦'}</span>
                          <span
                            onClick={(e) => {
                              if (item.object_type_id && onOpenObjectCollection) {
                                e.stopPropagation();
                                onOpenObjectCollection(item.object_type_id);
                              }
                            }}
                            style={{ borderColor: item.color || '#8C8A82' }}
                            className={`text-[10px] uppercase font-bold tracking-wider px-2 py-0.5 rounded-full bg-[#FAF8F3] dark:bg-[#22241F] text-[#4A4B45] dark:text-[#D4D1C7] border ${
                              onOpenObjectCollection ? 'hover:underline cursor-pointer' : ''
                            }`}
                          >
                            {item.object_type_name || 'Object'}
                          </span>
                        </div>

                        <button
                          type="button"
                          data-testid={`pin-btn-${item.id}`}
                          onClick={(e) => handleTogglePinAction(item, e)}
                          title={item.is_pinned ? 'Unpin' : 'Pin'}
                          className={`p-1.5 rounded-lg transition-colors cursor-pointer ${
                            item.is_pinned
                              ? 'text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-950/30'
                              : 'text-[#8C8A82] hover:text-[#1E201C] dark:hover:text-white hover:bg-[#FAF8F3] dark:hover:bg-[#22241F]'
                          }`}
                        >
                          <Pin className={`w-3.5 h-3.5 ${item.is_pinned ? 'rotate-45' : ''}`} />
                        </button>
                      </div>

                      <h3 className="font-bold text-xs text-[#1E201C] dark:text-[#EDEAE2] group-hover:underline line-clamp-2">
                        {item.title}
                      </h3>

                      {item.description && (
                        <p className="text-[11px] text-[#8C8A82] dark:text-[#7A786F] line-clamp-2 leading-relaxed">
                          {item.description}
                        </p>
                      )}
                    </div>

                    <div className="pt-2 border-t border-[#E8E5DC]/60 dark:border-[#2C2E27]/60 flex items-center justify-between text-[11px] text-[#8C8A82] dark:text-[#7A786F]">
                      <span className="truncate max-w-[140px]">
                        {item.project_name ? `📁 ${item.project_name}` : 'Standalone'}
                      </span>
                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          onOpenArtifact(item.id, item.project_id);
                        }}
                        className="inline-flex items-center gap-1 font-semibold text-[#1E201C] dark:text-[#EDEAE2] hover:underline"
                      >
                        <span>Open</span>
                        <ArrowRight className="w-3 h-3 group-hover:translate-x-0.5 transition-transform" />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}

            {/* STANDALONE NOTES TAB (Dedicated unaffiliated list with [Attach to Project ▾] dropdown) */}
            {activeTab === 'standalone' && (
              <div className="bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl overflow-hidden shadow-2xs divide-y divide-[#E8E5DC]/60 dark:divide-[#2C2E27]/60">
                {filteredItems.map((item) => (
                  <div
                    key={item.id}
                    data-testid={`index-item-${item.id}`}
                    onClick={() => onOpenArtifact(item.id, null)}
                    className="p-4 hover:bg-[#FAF8F3] dark:hover:bg-[#181916] transition-colors cursor-pointer flex items-center justify-between gap-4 group relative"
                  >
                    <div className="flex items-center gap-3 min-w-0 flex-1">
                      <div className="w-8 h-8 rounded-lg bg-[#FAF8F3] dark:bg-[#22241F] flex items-center justify-center text-[#6E6D66] dark:text-[#A8A69E] shrink-0">
                        <FileText className="w-4 h-4" />
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <h3 className="font-semibold text-xs text-[#1E201C] dark:text-[#EDEAE2] truncate group-hover:underline">
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
                          <p className="text-[11px] text-[#8C8A82] dark:text-[#7A786F] truncate mt-0.5 max-w-xl">
                            {item.description}
                          </p>
                        )}
                      </div>
                    </div>

                    <div className="flex items-center gap-3 shrink-0">
                      <span className="text-[11px] text-[#8C8A82] dark:text-[#7A786F]">
                        {formatDate(item.updated_at)}
                      </span>

                      {/* Attach to Project Dropdown */}
                      <div className="relative" onClick={(e) => e.stopPropagation()}>
                        <button
                          type="button"
                          data-testid={`attach-project-btn-${item.id}`}
                          onClick={() =>
                            setAttachDropdownOpenId(
                              attachDropdownOpenId === item.id ? null : item.id
                            )
                          }
                          className="flex items-center gap-1.5 px-2.5 py-1 rounded-xl text-xs font-semibold bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-[#1E201C] dark:text-[#EDEAE2] transition-colors cursor-pointer border border-[#E8E5DC] dark:border-[#2C2E27]"
                        >
                          <span>Attach to Project</span>
                          <ChevronDown className="w-3 h-3 text-[#8C8A82]" />
                        </button>

                        {attachDropdownOpenId === item.id && (
                          <div className="absolute right-0 top-8 z-50 w-52 bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl shadow-xl py-1 text-xs animate-in fade-in duration-100 max-h-48 overflow-y-auto">
                            <div className="px-3 py-1 text-[10px] font-bold text-[#8C8A82] dark:text-[#7A786F] uppercase">
                              Select Target Project
                            </div>
                            {projectsList.length === 0 ? (
                              <div className="px-3 py-2 text-[11px] text-[#8C8A82] italic">
                                No projects exist yet
                              </div>
                            ) : (
                              projectsList.map((p) => (
                                <button
                                  key={p.id}
                                  type="button"
                                  data-testid={`attach-target-${p.id}`}
                                  onClick={() => handleAttachNote(item.id, p.id)}
                                  className="w-full flex items-center gap-2 px-3 py-1.5 hover:bg-[#FAF8F3] dark:hover:bg-[#22241F] text-left text-[#3A3B36] dark:text-[#EDEAE2] hover:text-[#1E201C] dark:hover:text-white cursor-pointer"
                                >
                                  <Folder className="w-3 h-3 text-amber-600 dark:text-amber-400 shrink-0" />
                                  <span className="truncate">{p.name}</span>
                                </button>
                              ))
                            )}
                          </div>
                        )}
                      </div>

                      {/* Pin Button */}
                      <button
                        type="button"
                        data-testid={`pin-btn-${item.id}`}
                        onClick={(e) => handleTogglePinAction(item, e)}
                        title={item.is_pinned ? 'Unpin note' : 'Pin note'}
                        className={`p-1.5 rounded-lg transition-colors cursor-pointer ${
                          item.is_pinned
                            ? 'text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-950/30'
                            : 'text-[#8C8A82] hover:text-[#1E201C] dark:hover:text-white hover:bg-[#EFECE4] dark:hover:bg-[#22241F]'
                        }`}
                      >
                        <Pin className={`w-3.5 h-3.5 ${item.is_pinned ? 'rotate-45' : ''}`} />
                      </button>

                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          onOpenArtifact(item.id, null);
                        }}
                        className="p-1 rounded-md text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white"
                      >
                        <ExternalLink className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}

            {/* ALL ITEMS TAB (Unified Table) */}
            {activeTab === 'all' && (
              <div className="bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl overflow-hidden shadow-2xs divide-y divide-[#E8E5DC]/60 dark:divide-[#2C2E27]/60">
                {filteredItems.map((item) => {
                  const isProj = item.entity_type === 'project';
                  const isObj = item.entity_type === 'object';
                  return (
                    <div
                      key={item.id}
                      data-testid={`index-item-${item.id}`}
                      onClick={() => {
                        if (isProj) {
                          onOpenProject(item.id);
                        } else {
                          onOpenArtifact(item.id, item.project_id);
                        }
                      }}
                      className="p-4 hover:bg-[#FAF8F3] dark:hover:bg-[#181916] transition-colors cursor-pointer flex items-center justify-between gap-4 group"
                    >
                      <div className="flex items-center gap-3 min-w-0 flex-1">
                        <div className="w-8 h-8 rounded-lg bg-[#FAF8F3] dark:bg-[#22241F] flex items-center justify-center shrink-0">
                          {isProj ? (
                            <Folder className="w-4 h-4 text-amber-600 dark:text-amber-400" />
                          ) : isObj ? (
                            <span className="text-sm">{item.icon || '📦'}</span>
                          ) : (
                            <FileText className="w-4 h-4 text-[#6E6D66] dark:text-[#A8A69E]" />
                          )}
                        </div>
                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-2">
                            <h3 className="font-semibold text-xs text-[#1E201C] dark:text-[#EDEAE2] truncate group-hover:underline">
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
                            <p className="text-[11px] text-[#8C8A82] dark:text-[#7A786F] truncate mt-0.5 max-w-xl">
                              {item.description}
                            </p>
                          )}
                        </div>
                      </div>

                      <div className="flex items-center gap-3 shrink-0">
                        {/* Type badge */}
                        <span className="text-[10px] uppercase font-bold tracking-wider px-2 py-0.5 rounded-md bg-[#EFECE4] dark:bg-[#22241F] text-[#6E6D66] dark:text-[#A8A69E]">
                          {isProj
                            ? 'Project'
                            : isObj
                            ? item.object_type_name || 'Object'
                            : 'Note'}
                        </span>

                        {/* Project / Standalone chip */}
                        {item.project_name ? (
                          <span className="text-[11px] text-[#4A4B45] dark:text-[#D4D1C7] truncate max-w-[120px]">
                            📁 {item.project_name}
                          </span>
                        ) : isProj ? (
                          <span className="text-[11px] text-[#8C8A82] dark:text-[#7A786F]">
                            {item.item_count ?? 0} items
                          </span>
                        ) : (
                          <span className="text-[11px] text-[#8C8A82] dark:text-[#7A786F]">
                            Standalone
                          </span>
                        )}

                        <span className="text-[11px] text-[#8C8A82] dark:text-[#7A786F] w-20 text-right">
                          {formatDate(item.updated_at)}
                        </span>

                        {/* Pin Button */}
                        <button
                          type="button"
                          data-testid={`pin-btn-${item.id}`}
                          onClick={(e) => handleTogglePinAction(item, e)}
                          title={item.is_pinned ? 'Unpin' : 'Pin'}
                          className={`p-1.5 rounded-lg transition-colors cursor-pointer ${
                            item.is_pinned
                              ? 'text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-950/30'
                              : 'text-[#8C8A82] hover:text-[#1E201C] dark:hover:text-white hover:bg-[#EFECE4] dark:hover:bg-[#22241F]'
                          }`}
                        >
                          <Pin className={`w-3.5 h-3.5 ${item.is_pinned ? 'rotate-45' : ''}`} />
                        </button>

                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            if (isProj) {
                              onOpenProject(item.id);
                            } else {
                              onOpenArtifact(item.id, item.project_id);
                            }
                          }}
                          className="p-1 rounded-md text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white"
                        >
                          <ExternalLink className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
};

export default WorkspaceIndexView;
