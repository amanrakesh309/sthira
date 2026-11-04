import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { KeyboardEvent } from 'react';
import { Search, Folder, FileText, Pin, ArrowUpDown, ExternalLink, Layers, Filter,
  SlidersHorizontal, MoreHorizontal, Trash2, ChevronDown, ChevronLeft, ChevronRight, X } from 'lucide-react';
import { getWorkspaceIndex, togglePin, addArtifactToProject, getWorkspaceTree, listObjectTypes } from '../api/ipc';
import { PageActionMenu } from '../components/PageActionMenu';
import { ProjectDeletionModal } from '../components/ProjectDeletionModal';
import { DirectoryMenu } from '../components/DirectoryMenu';
import { ObjectIdentity } from '../components/objects/ObjectIdentity';
import { directoryDate, directoryExcerpt } from '../utils/directory';
import type { WorkspaceIndexData, WorkspaceIndexItem, WorkspaceIndexTab, NoteScopeFilter, NoteSortKey, ObjectType } from '../types/sthira';

interface WorkspaceIndexViewProps {
  initialTab?: WorkspaceIndexTab;
  onOpenProject: (projectId: string) => void;
  onOpenArtifact: (artifactId: string, projectId?: string | null) => void;
  onOpenObjectCollection?: (typeId: string) => void;
  onOpenObjectTypeManager?: () => void;
  onBack?: () => void;
  onWorkspaceMutated?: () => void;
}

const categories = [
  { id: 'all', label: 'All Items' }, { id: 'projects', label: 'Projects' },
  { id: 'notes', label: 'Notes & Pages' }, { id: 'objects', label: 'Custom Objects' },
] as const;

function Pagination({ count, page, size, setPage, setSize }: {
  count: number; page: number; size: number; setPage: (page: number) => void; setSize: (size: number) => void;
}) {
  const pages = Math.max(1, Math.ceil(count / size));
  const numbers = Array.from({ length: pages }, (_, i) => i + 1).filter(n => n === 1 || n === pages || Math.abs(n - page) <= 1);
  return <footer className="all-files-footer">
    <span role="status">Showing {count ? (page - 1) * size + 1 : 0}–{Math.min(count, page * size)} of {count} items</span>
    <div className="all-files-footer-controls">
      <label>Rows per page: <select aria-label="Rows per page" value={size} onChange={event => { setSize(Number(event.target.value)); setPage(1); }}>
        {[10, 25, 50].map(n => <option key={n} value={n}>{n}</option>)}
      </select></label>
      <nav className="all-files-pagination" aria-label="Directory pages">
        <button type="button" aria-label="Previous page" disabled={page <= 1} onClick={() => setPage(Math.max(1, page - 1))}><ChevronLeft size={16} /></button>
        {numbers.map((number, index) => <span key={number}>
          {index > 0 && number - numbers[index - 1] > 1 && <span className="directory-page-gap">…</span>}
          <button type="button" aria-current={number === page ? 'page' : undefined} onClick={() => setPage(number)}>{number}</button>
        </span>)}
        <button type="button" aria-label="Next page" disabled={page >= pages} onClick={() => setPage(Math.min(pages, page + 1))}><ChevronRight size={16} /></button>
      </nav>
    </div>
  </footer>;
}

export function WorkspaceIndexView({ initialTab = 'all', onOpenProject, onOpenArtifact, onOpenObjectCollection,
  onOpenObjectTypeManager, onBack, onWorkspaceMutated }: WorkspaceIndexViewProps) {
  const [activeTab, setActiveTab] = useState<WorkspaceIndexTab>(initialTab);
  const [searchQuery, setSearchQuery] = useState('');
  const [data, setData] = useState<WorkspaceIndexData>({ counts: { all: 0, projects: 0, notes: 0, objects: 0, standalone: 0 }, items: [] });
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [notice, setNotice] = useState('');
  const [pendingId, setPendingId] = useState<string | null>(null);
  const [selectedTypeFilter, setSelectedTypeFilter] = useState<string | null>(null);
  const [noteScope, setNoteScope] = useState<NoteScopeFilter>('all');
  const [noteSort, setNoteSort] = useState<NoteSortKey>('updated');
  const [objectTypes, setObjectTypes] = useState<ObjectType[]>([]);
  const [projects, setProjects] = useState<{ id: string; name: string }[]>([]);
  const [projectToDeleteId, setProjectToDeleteId] = useState<string | null>(null);
  const [currentPage, setCurrentPage] = useState(1);
  const [rowsPerPage, setRowsPerPage] = useState(25);
  const searchInput = useRef<HTMLInputElement>(null);
  const request = useRef(0);
  const retryAction = useRef<(() => void) | null>(null);

  useEffect(() => { setActiveTab(initialTab); setSelectedTypeFilter(null); setCurrentPage(1); }, [initialTab]);
  useEffect(() => {
    const shortcut = (event: globalThis.KeyboardEvent) => {
      const target = event.target as HTMLElement;
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'f' &&
        !target.closest('input, textarea, [contenteditable="true"], [role="dialog"]')) {
        event.preventDefault(); searchInput.current?.focus(); searchInput.current?.select();
      }
    };
    window.addEventListener('keydown', shortcut);
    return () => window.removeEventListener('keydown', shortcut);
  }, []);

  const loadData = useCallback(async () => {
    const id = ++request.current;
    setIsLoading(true); setLoadError(null);
    // Auxiliary metadata must not turn a readable directory into a failed load.
    const [index, tree, types] = await Promise.allSettled([
      getWorkspaceIndex(activeTab, searchQuery.trim() || undefined), getWorkspaceTree(), listObjectTypes(),
    ]);
    if (id !== request.current) return;
    if (index.status === 'fulfilled') setData(index.value);
    else setLoadError('Couldn’t load the workspace directory. Check your vault and try again.');
    if (tree.status === 'fulfilled') setProjects(tree.value.projects.map(p => ({ id: p.id, name: p.name })));
    if (types.status === 'fulfilled') setObjectTypes(types.value);
    if (tree.status === 'rejected' || types.status === 'rejected') setActionError('Some project or type controls are unavailable. Retry loading the directory.');
    setIsLoading(false);
  }, [activeTab, searchQuery]);
  const latestLoad = useRef(loadData);
  latestLoad.current = loadData;

  useEffect(() => { void loadData(); return () => { request.current += 1; }; }, [loadData]);

  const mutate = async (id: string, action: () => Promise<unknown>, success: string) => {
    setPendingId(id); setActionError(null); setNotice('');
    retryAction.current = () => { void mutate(id, action, success); };
    try {
      await action(); setNotice(success); retryAction.current = null;
      onWorkspaceMutated?.(); await latestLoad.current();
    } catch { setActionError('Couldn’t update this item. Your saved data has not been replaced. Try again.'); }
    finally { setPendingId(null); }
  };

  const categoryItems = useMemo(() => data.items.filter(item => activeTab === 'all' ||
    item.entity_type === (activeTab === 'projects' ? 'project' : activeTab === 'objects' ? 'object' : 'note')), [data.items, activeTab]);
  const scopeCounts = useMemo(() => ({ all: categoryItems.length,
    projects: categoryItems.filter(item => !item.standalone).length,
    standalone: categoryItems.filter(item => item.standalone).length }), [categoryItems]);
  const filteredItems = useMemo(() => {
    let items = categoryItems;
    if (activeTab === 'all' || activeTab === 'notes') {
      if (noteScope === 'standalone') items = items.filter(item => item.standalone);
      if (noteScope === 'projects') items = items.filter(item => !item.standalone);
    }
    if ((activeTab === 'all' || activeTab === 'objects') && selectedTypeFilter) items = items.filter(item => item.object_type_id === selectedTypeFilter);
    return [...items].sort((a, b) => {
      if (a.is_pinned !== b.is_pinned) return a.is_pinned ? -1 : 1;
      if (noteSort === 'created') return b.created_at - a.created_at;
      if (noteSort === 'title_asc') return a.title.localeCompare(b.title, undefined, { sensitivity: 'base' });
      if (noteSort === 'title_desc') return b.title.localeCompare(a.title, undefined, { sensitivity: 'base' });
      return b.updated_at - a.updated_at;
    });
  }, [categoryItems, activeTab, noteScope, selectedTypeFilter, noteSort]);
  const pageCount = Math.max(1, Math.ceil(filteredItems.length / rowsPerPage));
  const safePage = Math.min(currentPage, pageCount);
  useEffect(() => { setCurrentPage(page => Math.min(page, pageCount)); }, [pageCount]);
  const pageItems = filteredItems.slice((safePage - 1) * rowsPerPage, safePage * rowsPerPage);
  const sortedTypes = useMemo(() => [...objectTypes].sort((a, b) => a.display_name.localeCompare(b.display_name)), [objectTypes]);
  const isMac = /Mac|iPhone|iPad/i.test(navigator.platform || navigator.userAgent);
  const openItem = (item: WorkspaceIndexItem) => item.entity_type === 'project' ? onOpenProject(item.id) : onOpenArtifact(item.id, item.project_id);
  const rowKey = (event: KeyboardEvent<HTMLElement>, item: WorkspaceIndexItem) => {
    if (event.target === event.currentTarget && (event.key === 'Enter' || event.key === ' ')) { event.preventDefault(); openItem(item); }
  };
  const refreshed = () => { void latestLoad.current(); onWorkspaceMutated?.(); };
  const pin = (item: WorkspaceIndexItem) => { void mutate(item.id, () => togglePin(item.id, item.entity_type, !item.is_pinned), item.is_pinned ? 'Item unpinned.' : 'Item pinned.'); };
  const identity = (item: WorkspaceIndexItem) => {
    const type = objectTypes.find(type => type.id === item.object_type_id);
    return { name: type?.display_name || item.object_type_name || 'Object', icon: type?.icon || item.icon, color: type?.color || item.color };
  };
  const typeBadge = (item: WorkspaceIndexItem) => item.entity_type === 'object' ? (
    onOpenObjectCollection && item.object_type_id ? <button type="button" className="directory-type-link" aria-label={`Open ${identity(item).name} collection`}
      onClick={event => { event.stopPropagation(); onOpenObjectCollection(item.object_type_id!); }}><ObjectIdentity {...identity(item)} /></button>
      : <ObjectIdentity {...identity(item)} />
  ) : <span className="directory-type-badge">{item.entity_type === 'project' ? <Folder size={14} /> : <FileText size={14} />}{item.entity_type === 'project' ? 'Project' : 'Note'}</span>;
  const entityIcon = (item: WorkspaceIndexItem) => <span className={`directory-entity-icon ${item.entity_type === 'project' ? 'directory-folder' : ''}`} aria-hidden="true">
    {item.entity_type === 'project' ? <Folder size={22} /> : item.entity_type === 'object' ? identity(item).icon || '◇' : <FileText size={20} />}
  </span>;
  const actions = (item: WorkspaceIndexItem) => <div className="directory-row-actions" onClick={event => event.stopPropagation()}>
    {!item.project_id && item.entity_type === 'note' && <DirectoryMenu label="Attach to Project" icon={<ChevronDown size={16} />}
      testId={`attach-project-btn-${item.id}`} disabled={pendingId === item.id} items={projects.map(project => ({
        id: project.id, label: project.name, icon: <Folder size={16} />, testId: `attach-target-${project.id}`,
        onSelect: () => { void mutate(item.id, () => addArtifactToProject(item.id, project.id), 'Note attached to project.'); },
      }))} />}
    {item.entity_type === 'project' ? <DirectoryMenu label="Project actions" testId={`project-menu-btn-${item.id}`} icon={<MoreHorizontal size={18} />}
      items={[{ id: 'open', label: 'Open', icon: <ExternalLink size={16} />, onSelect: () => onOpenProject(item.id) },
        { id: 'delete', label: 'Delete', danger: true, icon: <Trash2 size={16} />, testId: `delete-project-${item.id}`, onSelect: () => setProjectToDeleteId(item.id) }]} />
      : <PageActionMenu variant="row" align="right" artifactId={item.id} artifactTitle={item.title} projectId={item.project_id ?? null}
        objectTypes={objectTypes} onChanged={refreshed} onOpenArtifact={onOpenArtifact}
        onProjectCreated={id => { refreshed(); onOpenProject(id); }} onDeleted={refreshed} />}
    <button type="button" className={`directory-icon-button directory-pin ${item.is_pinned ? 'is-pinned' : ''}`}
      data-testid={`pin-btn-${item.id}`} title={item.is_pinned ? 'Unpin' : 'Pin'} aria-label={item.is_pinned ? 'Unpin' : 'Pin'}
      disabled={pendingId === item.id} onClick={() => pin(item)}><Pin size={16} /></button>
    <button type="button" className="directory-icon-button" title="Open" aria-label={`Open ${item.title}`} onClick={() => openItem(item)}><ExternalLink size={16} /></button>
  </div>;
  const pagination = <Pagination count={filteredItems.length} page={safePage} size={rowsPerPage} setPage={setCurrentPage} setSize={setRowsPerPage} />;

  return <section data-testid="workspace-index-view" className="sthira-all-files" aria-labelledby="directory-title">
    <div className="all-files-container">
      <header className="all-files-header">
        <div className="all-files-heading-copy"><div className="all-files-eyebrow">{onBack ? <button type="button" onClick={onBack}>Workspace</button> : <span>Workspace</span>}<span aria-hidden="true">/</span></div>
          <h1 id="directory-title"><Folder aria-hidden="true" />All Files</h1>
          <p>Unified directory of every project, note, page, and structured object.</p>
        </div>
        <div className="all-files-header-actions"><div className="directory-search">
          <Search size={17} aria-hidden="true" /><input ref={searchInput} type="search" aria-label="Filter workspace items" data-testid="search-input"
            placeholder="Filter by title, content or type…" value={searchQuery} onChange={event => { setSearchQuery(event.target.value); setCurrentPage(1); }} />
          {searchQuery ? <button type="button" aria-label="Clear search" onClick={() => { setSearchQuery(''); setCurrentPage(1); searchInput.current?.focus(); }}><X size={16} /></button>
            : <kbd aria-hidden="true">{isMac ? '⌘F' : 'Ctrl F'}</kbd>}
        </div>{onOpenObjectTypeManager && <button type="button" className="directory-manage-button" data-testid="btn-manage-object-types-index" onClick={onOpenObjectTypeManager}><SlidersHorizontal size={17} />Manage Types</button>}</div>
      </header>
      <div className="all-files-category-tabs" role="tablist" aria-label="Directory Categories" onKeyDown={event => {
        if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
        event.preventDefault(); const index = categories.findIndex(category => category.id === activeTab);
        const next = event.key === 'Home' ? 0 : event.key === 'End' ? categories.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : -1) + categories.length) % categories.length;
        setActiveTab(categories[next].id); setSelectedTypeFilter(null); setCurrentPage(1);
        event.currentTarget.querySelectorAll<HTMLButtonElement>('button')[next]?.focus();
      }}>
        {categories.map(category => <button key={category.id} id={`directory-tab-${category.id}`} type="button" role="tab" tabIndex={activeTab === category.id ? 0 : -1}
          aria-selected={activeTab === category.id} aria-controls="directory-results" data-testid={`tab-${category.id}`}
          onClick={() => { setActiveTab(category.id); setSelectedTypeFilter(null); setCurrentPage(1); }}>
          {category.label}<span>{data.counts[category.id]}</span>
        </button>)}
      </div>
      <div className="all-files-filter-bar">
        <div className="directory-filter-groups">
          {(activeTab === 'all' || activeTab === 'notes') && <div className="directory-filter-group" data-testid="note-scope-filter" role="group" aria-label="Scope of matching items">
            <span className="directory-filter-label"><Filter size={15} />Show:</span>
            {([{ id: 'all', label: 'All' }, { id: 'projects', label: 'In Projects' }, { id: 'standalone', label: 'Standalone' }] as const).map(scope => <button
              key={scope.id} type="button" aria-pressed={noteScope === scope.id} data-testid={`note-scope-${scope.id}`}
              onClick={() => { setNoteScope(scope.id); setCurrentPage(1); }}>{scope.label}<span>{scopeCounts[scope.id]}</span></button>)}
          </div>}
          {(activeTab === 'all' || activeTab === 'objects') && sortedTypes.length > 0 && <div className="directory-filter-group directory-type-filters" role="group" aria-label="Object type">
            <span className="directory-filter-label"><Layers size={15} />Type:</span>
            {sortedTypes.map(type => <button key={type.id} type="button" data-testid={`type-filter-${type.id}`} aria-pressed={selectedTypeFilter === type.id}
              onClick={() => { setSelectedTypeFilter(selectedTypeFilter === type.id ? null : type.id); setCurrentPage(1); }}><span aria-hidden="true">{type.icon}</span>{type.display_name}</button>)}
            {selectedTypeFilter && onOpenObjectCollection && <button type="button" className="directory-collection-link"
              onClick={() => onOpenObjectCollection(selectedTypeFilter)}><ExternalLink size={15} />Open collection</button>}
          </div>}
        </div>
        <label className="directory-sort"><ArrowUpDown size={15} />Sort:<select data-testid="note-sort-select" value={noteSort} title="Pinned items always appear first"
          onChange={event => { setNoteSort(event.target.value as NoteSortKey); setCurrentPage(1); }}>
          <option value="updated">Last Modified</option><option value="created">Date Created</option><option value="title_asc">Title (A–Z)</option><option value="title_desc">Title (Z–A)</option>
        </select></label>
      </div>
      {actionError && <div className="directory-notice" role="alert"><p>{actionError}</p><button type="button" onClick={() => { if (retryAction.current) retryAction.current(); else { setActionError(null); void loadData(); } }}>Retry</button></div>}
      <p className="sr-only" role="status">{notice}</p>
      <div id="directory-results" role="tabpanel" aria-labelledby={`directory-tab-${activeTab}`} aria-busy={isLoading}>
        {isLoading ? <div className="directory-empty" role="status">Loading workspace directory…</div>
          : loadError ? <div className="directory-empty"><p role="alert">{loadError}</p><button className="sthira-button-secondary" type="button" onClick={() => { void loadData(); }}>Try again</button></div>
          : filteredItems.length === 0 ? <div className="directory-empty"><Layers size={24} /><h2>No items found</h2><p>{searchQuery ? `No matches for “${searchQuery}”. Try clearing search.` : selectedTypeFilter ? 'No items match this type.' : noteScope !== 'all' && activeTab !== 'objects' && activeTab !== 'projects' ? 'No items match this scope.' : 'There are no items in this category yet.'}</p>
            {(searchQuery || selectedTypeFilter || noteScope !== 'all') && <button type="button" className="sthira-button-secondary" onClick={() => { setSearchQuery(''); setSelectedTypeFilter(null); setNoteScope('all'); }}>Clear filters</button>}</div>
          : activeTab === 'projects' ? <><div className="directory-project-grid">{pageItems.map(item => <article key={item.id} data-testid={`index-item-${item.id}`} className="directory-project-card"
            onClick={() => openItem(item)} onKeyDown={event => rowKey(event, item)}>
            <div className="directory-project-heading">{entityIcon(item)}<div><button className="directory-title-button" type="button" onClick={event => { event.stopPropagation(); openItem(item); }}>{item.title}</button><p>{item.item_count ?? 0} {item.item_count === 1 ? 'item' : 'items'}</p></div></div>
            <p className="directory-project-purpose">{directoryExcerpt(item.description, item.title)}</p>
            <div className="directory-project-footer"><span>Updated {directoryDate(item.updated_at)}</span>{actions(item)}</div>
          </article>)}</div>{pagination}</>
          : <div className="all-files-card"><table className="all-files-table"><caption className="sr-only">Workspace items</caption>
            <thead><tr><th scope="col">Title &amp; Description</th><th scope="col" className="directory-type-column">Type</th><th scope="col" className="directory-project-column">Project</th><th scope="col" className="directory-date-column">Modified</th><th scope="col" className="directory-actions-column"><span className="sr-only">Actions</span></th></tr></thead>
            <tbody>{pageItems.map(item => <tr key={`${item.id}-${item.project_id || ''}`} data-testid={`index-item-${item.id}`} tabIndex={0}
              onKeyDown={event => rowKey(event, item)} onClick={() => openItem(item)}>
              <td><div className="directory-item-copy">{entityIcon(item)}<div><div className="directory-item-title"><button type="button" tabIndex={-1} className="directory-title-button" onClick={event => { event.stopPropagation(); openItem(item); }}>{item.title}</button>{item.is_pinned && <Pin size={13} aria-label="Pinned" />}</div><p>{directoryExcerpt(item.description, item.title)}</p></div></div></td>
              <td className="directory-type-column">{typeBadge(item)}</td><td className="directory-project-column">{item.project_name && item.project_id ? <button type="button" className="directory-project-link" title={item.project_name}
                onClick={event => { event.stopPropagation(); onOpenProject(item.project_id!); }}><Folder size={15} /><span>{item.project_name}</span></button> : <span className="directory-dash">—</span>}</td>
              <td className="directory-date-column"><time>{directoryDate(item.updated_at)}</time></td><td className="directory-actions-column">{actions(item)}</td>
            </tr>)}</tbody></table>{pagination}</div>}
      </div>
    </div>
    <ProjectDeletionModal isOpen={!!projectToDeleteId} projectId={projectToDeleteId} onClose={() => setProjectToDeleteId(null)} onDeleted={async () => { setProjectToDeleteId(null); refreshed(); }} />
  </section>;
}

export default WorkspaceIndexView;
