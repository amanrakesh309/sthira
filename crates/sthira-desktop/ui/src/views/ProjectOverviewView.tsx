import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ArrowLeft, ArrowUpRight, CalendarDays, Check, ChevronLeft, ChevronRight, FileText, Folder, Layers, LayoutGrid, Link2, List, Loader2, Plus, RefreshCw, Search, X } from 'lucide-react';
import { getProjectOverview } from '../api/projects';
import { createPage, linkArtifactToProject } from '../api/ipc';
import { searchConnectionTargets } from '../api/connections';
import { ConnectionsPanel } from '../components/ConnectionsPanel';
import { PageActionMenu } from '../components/PageActionMenu';
import { ObjectIdentity } from '../components/objects/ObjectIdentity';
import { useDialogFocus } from '../hooks/useDialogFocus';
import { directoryDate, directoryExcerpt } from '../utils/directory';
import type { ProjectContentItem, ProjectItemRole, ProjectOverviewData, WorkspaceEntity } from '../types/sthira';

export interface ProjectBrowseState {
  query: string;
  type: string;
  relationship: 'all' | ProjectItemRole;
  sort: 'updated' | 'oldest' | 'created' | 'title' | 'title_desc';
  layout: 'list' | 'grid';
  page: number;
  pageSize: number;
}
export const DEFAULT_PROJECT_BROWSE: ProjectBrowseState = { query: '', type: 'all', relationship: 'all', sort: 'updated', layout: 'list', page: 1, pageSize: 25 };
interface Props {
  projectId: string;
  onBack: () => void;
  onOpenArtifact: (id: string, projectId?: string | null) => void;
  onOpenCanvas: (id: string, projectId: string) => void;
  onOpenProject: (id: string) => void;
  onWorkspaceMutated?: () => void;
  refreshTrigger?: number;
  initialBrowse?: ProjectBrowseState;
  onBrowseChange?: (state: ProjectBrowseState) => void;
}

const roleNames: Record<ProjectItemRole, string> = { member: 'In project', reference: 'Reference', backlink: 'Linked from' };
const itemLabel = (item: ProjectContentItem) => item.object_type_name || ({ note: 'Page', object: 'Object', source: 'Source', snippet: 'Snippet', canvas: 'Canvas', project: 'Project' }[item.item_type]);

function AddExistingDialog({ projectId, memberIds, onClose, onAdded }: { projectId: string; memberIds: Set<string>; onClose: () => void; onAdded: () => void }) {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<WorkspaceEntity[]>([]);
  const [loading, setLoading] = useState(true);
  const [pending, setPending] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const panel = useRef<HTMLDivElement>(null);
  useDialogFocus(true, panel, () => { if (!pending) onClose(); });
  useEffect(() => {
    let cancelled = false;
    setLoading(true); setError(null);
    const timer = window.setTimeout(() => {
      searchConnectionTargets(query).then((items) => {
        if (!cancelled) { setResults(items.filter((item) => item.kind === 'artifact')); setLoading(false); }
      }).catch(() => { if (!cancelled) { setError('Couldn’t search your saved items. Try again.'); setLoading(false); } });
    }, 180);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [query, retry]);
  const add = async (item: WorkspaceEntity) => {
    if (pending) return;
    setPending(item.id); setError(null);
    try { await linkArtifactToProject(item.id, projectId); onAdded(); onClose(); }
    catch { setError('Couldn’t add this item to the project. Try again.'); }
    finally { setPending(null); }
  };
  return <div className="project-dialog-layer">
    <div className="project-dialog-backdrop" onClick={() => { if (!pending) onClose(); }} />
    <div ref={panel} role="dialog" aria-modal="true" aria-labelledby="project-add-title" className="project-add-dialog">
      <header><div><span className="project-eyebrow">REUSE YOUR KNOWLEDGE</span><h2 id="project-add-title">Add an existing item</h2></div><button type="button" className="project-icon-button" aria-label="Close add existing" disabled={!!pending} onClick={onClose}><X size={18} /></button></header>
      <p>Bring a saved page, source, or object into this project. Its content and history stay together.</p>
      <label className="project-search"><Search size={17} /><input autoFocus aria-label="Search existing items" placeholder="Search by name or object type…" value={query} onChange={(event) => setQuery(event.target.value)} /></label>
      {error && <div role="alert" className="project-error"><p>{error}</p><button type="button" disabled={!!pending} onClick={() => setRetry((n) => n + 1)}>Retry</button></div>}
      <div className="project-add-results" aria-busy={loading}>
        {loading ? <p role="status" className="project-muted">Searching your workspace…</p> : results.length === 0 ? <p className="project-muted">No saved items match this search.</p> : results.map((item) => {
          const added = memberIds.has(item.id);
          return <button type="button" key={item.id} disabled={!!pending || added} aria-label={added ? `${item.title} already in project` : `Add ${item.title} to project`} onClick={() => add(item)}>
            <FileText size={18} /><span><strong>{item.title}</strong><small>{item.type_label}</small></span>{pending === item.id ? <Loader2 size={16} className="animate-spin" /> : added ? <Check size={16} /> : <Plus size={16} />}
          </button>;
        })}
      </div>
      <footer>Showing up to 60 matching items. Refine the search to find more.</footer>
    </div>
  </div>;
}

export function ProjectOverviewView({ projectId, onBack, onOpenArtifact, onOpenCanvas, onOpenProject, onWorkspaceMutated, refreshTrigger, initialBrowse, onBrowseChange }: Props) {
  const [data, setData] = useState<ProjectOverviewData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [briefExpanded, setBriefExpanded] = useState(false);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const [adding, setAdding] = useState(false);
  const [showConnections, setShowConnections] = useState(false);
  const [preview, setPreview] = useState(false);
  const [connectionRevision, setConnectionRevision] = useState(0);
  const [browse, setBrowse] = useState<ProjectBrowseState>(initialBrowse || DEFAULT_PROJECT_BROWSE);
  const request = useRef(0);
  const search = useRef<HTMLInputElement>(null);
  const connectionsButton = useRef<HTMLButtonElement>(null);
  const addButton = useRef<HTMLButtonElement>(null);
  useEffect(() => { onBrowseChange?.(browse); }, [browse, onBrowseChange]);
  const change = (next: Partial<ProjectBrowseState>) => setBrowse((prev) => ({ ...prev, ...next, page: 'page' in next ? next.page! : 1 }));
  const load = useCallback(async () => {
    const sequence = ++request.current;
    setLoading(true); setError(null);
    try { const value = await getProjectOverview(projectId); if (sequence === request.current) setData(value); }
    catch { if (sequence === request.current) setError('Couldn’t load this project. It may have been removed, or the vault may be unavailable.'); }
    finally { if (sequence === request.current) setLoading(false); }
  }, [projectId]);
  useEffect(() => { void load(); return () => { request.current += 1; }; }, [load, refreshTrigger, connectionRevision]);
  useEffect(() => {
    const keys = (event: KeyboardEvent) => {
      const target = event.target;
      const editing = target instanceof HTMLElement && target.closest('input, textarea, [contenteditable="true"], [role="dialog"]');
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'f' && !editing) {
        event.preventDefault(); search.current?.focus(); search.current?.select();
      }
    };
    window.addEventListener('keydown', keys); return () => window.removeEventListener('keydown', keys);
  }, []);
  const types = useMemo(() => [...new Map((data?.items || []).filter((item) => item.object_type_id).map((item) => [item.object_type_id!, item.object_type_name || 'Object'])).entries()].sort((a, b) => a[1].localeCompare(b[1])), [data]);
  const items = useMemo(() => {
    const query = browse.query.trim().toLocaleLowerCase();
    return (data?.items || []).filter((item) => {
      if (browse.type !== 'all' && (browse.type.startsWith('object:') ? item.object_type_id !== browse.type.slice(7) : item.item_type !== browse.type)) return false;
      if (browse.relationship !== 'all' && !item.roles.includes(browse.relationship)) return false;
      return !query || `${item.title} ${directoryExcerpt(item.description, item.title)} ${itemLabel(item)}`.toLocaleLowerCase().includes(query);
    }).sort((a, b) => {
      const name = a.title.localeCompare(b.title, undefined, { sensitivity: 'base', numeric: true });
      if (browse.sort === 'title') return name || a.id.localeCompare(b.id);
      if (browse.sort === 'title_desc') return -name || a.id.localeCompare(b.id);
      const time = browse.sort === 'created' ? b.created_at - a.created_at : browse.sort === 'oldest' ? a.updated_at - b.updated_at : b.updated_at - a.updated_at;
      return time || name || a.id.localeCompare(b.id);
    });
  }, [data, browse.query, browse.type, browse.relationship, browse.sort]);
  const pageCount = Math.max(1, Math.ceil(items.length / browse.pageSize));
  const page = Math.min(browse.page, pageCount);
  useEffect(() => { if (data && !loading) setBrowse((prev) => prev.page > pageCount ? { ...prev, page: pageCount } : prev); }, [pageCount, data, loading]);
  const visible = items.slice((page - 1) * browse.pageSize, page * browse.pageSize);
  const members = (data?.items || []).filter((item) => item.roles.includes('member'));
  const connected = (data?.items || []).filter((item) => item.roles.some((role) => role !== 'member'));
  const memberIds = new Set(members.filter((item) => item.kind === 'artifact').map((item) => item.id));
  const filtered = !!browse.query || browse.type !== 'all' || browse.relationship !== 'all';
  const open = (item: ProjectContentItem) => item.kind === 'canvas' ? onOpenCanvas(item.id, projectId) : item.kind === 'project' ? onOpenProject(item.id) : onOpenArtifact(item.id, item.project_id);
  const refreshed = () => { void load(); onWorkspaceMutated?.(); };
  const newPage = async () => {
    if (creating) return;
    setCreating(true); setActionError(null);
    try { const artifact = await createPage(projectId, 'Untitled page'); onWorkspaceMutated?.(); if (alive.current) onOpenArtifact(artifact.id, projectId); }
    catch { setActionError('Couldn’t create a page in this project. Try again.'); }
    finally { setCreating(false); }
  };
  const icon = (item: ProjectContentItem) => <span className={`project-item-icon ${item.kind === 'project' ? 'project-folder-icon' : ''}`} aria-hidden="true">{item.kind === 'project' ? <Folder size={21} /> : item.kind === 'canvas' ? <LayoutGrid size={20} /> : item.icon || <FileText size={20} />}</span>;
  const type = (item: ProjectContentItem) => item.item_type === 'object' ? <ObjectIdentity name={itemLabel(item)} icon={item.icon || undefined} color={item.color || undefined} /> : <span className="project-type">{itemLabel(item)}</span>;
  const relationships = (item: ProjectContentItem) => <div className="project-item-roles">{item.roles.map((role) => <span key={role} data-role={role}>{roleNames[role]}</span>)}</div>;
  const description = (item: ProjectContentItem) => directoryExcerpt(item.description, item.title) || (item.kind === 'canvas' ? `${item.item_count || 0} canvas items` : item.kind === 'project' ? `${item.item_count || 0} pages and objects` : 'No content preview yet.');
  const actions = (item: ProjectContentItem) => <div className="project-item-actions">
    {item.kind === 'artifact' && item.roles.includes('member') && <PageActionMenu variant="row" artifactId={item.id} artifactTitle={item.title} projectId={projectId} onOpenArtifact={onOpenArtifact} onChanged={refreshed} onDeleted={refreshed} />}
    <button type="button" className="project-icon-button" aria-label={`Open ${item.title} in workspace`} onClick={() => open(item)}><ArrowUpRight size={17} /></button>
  </div>;
  const closeConnections = () => { setShowConnections(false); setPreview(false); setConnectionRevision((n) => n + 1); connectionsButton.current?.focus(); };

  return <section className="project-overview" data-testid="project-overview" data-connections-open={showConnections} data-connection-preview={preview} aria-labelledby="project-title">
    <header className="project-topbar"><nav aria-label="Project location"><button type="button" onClick={onBack}><ArrowLeft size={16} /><span>Projects</span></button><span aria-hidden="true">/</span><span>{data?.project.name || 'Project overview'}</span></nav>
      <div><button type="button" className="project-icon-button" aria-label="Refresh project" disabled={loading} onClick={() => { void load(); }}><RefreshCw size={16} className={loading ? 'animate-spin' : ''} /></button><button type="button" ref={connectionsButton} className="project-secondary-button" aria-label="Project connections" disabled={!data} aria-expanded={showConnections} onClick={() => showConnections ? closeConnections() : setShowConnections(true)}><Link2 size={15} /><span>Connections</span></button></div>
    </header>
    <div className="project-overview-body"><div className="project-overview-scroll"><div className="project-overview-container">
      {!data && loading ? <div className="project-loading" role="status"><Loader2 size={22} className="animate-spin" />Opening project…</div> : !data && error ? <div className="project-empty" role="alert"><Folder size={28} /><h1 id="project-title">Project unavailable</h1><p>{error}</p><button type="button" className="project-secondary-button" onClick={() => { void load(); }}>Try again</button></div> : data && <>
        <div className="project-hero">
          <div className="project-hero-copy"><div className="project-kicker"><span className="project-eyebrow">PROJECT WORKSPACE</span><span className="project-status" data-status={data.project.status}>{data.project.status}</span></div><h1 id="project-title">{data.project.name}</h1><p>Browse saved work, shared items, and project references.</p></div>
          <div className="project-hero-actions"><button type="button" ref={addButton} className="project-secondary-button" onClick={() => setAdding(true)}><Link2 size={16} />Add existing</button><button type="button" className="project-primary-button" disabled={creating} onClick={() => { void newPage(); }}>{creating ? <Loader2 size={16} className="animate-spin" /> : <Plus size={17} />}New page</button></div>
        </div>
        <div className="project-summary"><section className="project-brief" aria-label="Project brief" data-expanded={briefExpanded}><span className="project-summary-icon"><Folder size={21} /></span><div><h2>Project brief</h2><p>{data.project.purpose || 'No project brief has been recorded yet.'}</p>{(data.project.purpose.length > 240 || data.project.purpose.split('\n').length > 3) && <button type="button" aria-expanded={briefExpanded} onClick={() => setBriefExpanded((value) => !value)}>{briefExpanded ? 'Show less' : 'Read full brief'}</button>}</div></section>
          <dl className="project-facts"><div><dt>In this project</dt><dd>{members.length}<span>{members.length === 1 ? 'item' : 'items'}</span></dd></div><div><dt>Connected</dt><dd>{connected.length}<span>{connected.length === 1 ? 'linked item' : 'linked items'}</span></dd></div><div><dt>Last worked on</dt><dd className="project-fact-date">{directoryDate(data.project.last_substantive_at)}</dd></div></dl>
        </div>
        {error && <div className="project-error" role="alert"><span>{error}</span><button type="button" onClick={() => { void load(); }}>Try again</button></div>}
        {actionError && <div className="project-error" role="alert"><span>{actionError}</span><button type="button" onClick={() => setActionError(null)}>Dismiss</button></div>}
        <section className="project-contents" aria-labelledby="project-contents-title">
          <div className="project-contents-heading"><div><h2 id="project-contents-title">Project contents <span>{data.items.length}</span></h2><p>Pages, objects, canvases, and directly linked items.</p></div><div className="project-layout-control" role="group" aria-label="Project contents layout"><button type="button" aria-label="List view" aria-pressed={browse.layout === 'list'} onClick={() => change({ layout: 'list', page })}><List size={17} /></button><button type="button" aria-label="Grid view" aria-pressed={browse.layout === 'grid'} onClick={() => change({ layout: 'grid', page })}><LayoutGrid size={17} /></button></div></div>
          <div className="project-toolbar">
            <div className="project-toolbar-search">
              <label className="project-search">
                <Search size={16} aria-hidden="true" />
                <input ref={search} aria-label="Search project contents" placeholder="Search names, types, and summaries…" value={browse.query} onChange={(event) => change({ query: event.target.value })} />
                {browse.query && <button type="button" aria-label="Clear project search" onClick={() => { change({ query: '' }); search.current?.focus(); }}><X size={15} /></button>}
              </label>
            </div>
            <div className="project-toolbar-filters">
              <label className="project-select">
                <span>Type</span>
                <div className="project-select-field">
                  <select aria-label="Filter project type" value={browse.type} onChange={(event) => change({ type: event.target.value })}>
                    <option value="all">All types</option>
                    <option value="note">Pages</option>
                    <option value="object">All objects</option>
                    {types.map(([id, name]) => <option key={id} value={`object:${id}`}>Object · {name}</option>)}
                    <option value="source">Sources</option>
                    <option value="snippet">Snippets</option>
                    <option value="canvas">Canvases</option>
                    <option value="project">Projects</option>
                  </select>
                </div>
              </label>
              <label className="project-select">
                <span>Relationship</span>
                <div className="project-select-field">
                  <select aria-label="Filter project relationship" value={browse.relationship} onChange={(event) => change({ relationship: event.target.value as ProjectBrowseState['relationship'] })}>
                    <option value="all">All relationships</option>
                    <option value="member">In project</option>
                    <option value="reference">References</option>
                    <option value="backlink">Linked from</option>
                  </select>
                </div>
              </label>
              <label className="project-select project-sort">
                <span>Sort by</span>
                <div className="project-select-field">
                  <select aria-label="Sort project contents" value={browse.sort} onChange={(event) => change({ sort: event.target.value as ProjectBrowseState['sort'] })}>
                    <option value="updated">Recently edited</option>
                    <option value="oldest">Oldest edited</option>
                    <option value="created">Recently created</option>
                    <option value="title">Name A–Z</option>
                    <option value="title_desc">Name Z–A</option>
                  </select>
                </div>
              </label>
            </div>
          </div>
          {filtered && <div className="project-filter-summary"><span>{items.length} matching {items.length === 1 ? 'item' : 'items'}</span><button type="button" onClick={() => change({ query: '', type: 'all', relationship: 'all' })}>Clear filters</button></div>}
          {items.length === 0 ? <div className="project-empty"><Layers size={28} /><h3>{filtered ? 'No items match these filters' : 'No items in this project yet'}</h3><p>{filtered ? 'Try another search or clear the filters to see everything.' : 'Create a page, add a saved item, or connect another part of your workspace.'}</p><button type="button" className="project-secondary-button" onClick={() => filtered ? change({ query: '', type: 'all', relationship: 'all' }) : setAdding(true)}>{filtered ? 'Clear search & filters' : 'Add an existing item'}</button></div> : browse.layout === 'list' ? <div className="project-table-scroll"><table className="project-table"><thead><tr><th scope="col">Name & summary</th><th scope="col">Type</th><th scope="col">Relationship</th><th scope="col">Edited</th><th scope="col"><span className="sr-only">Actions</span></th></tr></thead><tbody>{visible.map((item) => <tr key={`${item.kind}-${item.id}`}><td><button type="button" className="project-item-title" aria-label={`Open ${item.title}`} onClick={() => open(item)}>{icon(item)}<span><strong>{item.title}{item.is_draft && <small className="project-draft">Draft</small>}</strong><small>{description(item)}</small></span></button><div className="project-item-mobile-meta">{type(item)}{relationships(item)}<span className="project-modified">{directoryDate(item.updated_at)}</span></div></td><td>{type(item)}</td><td>{relationships(item)}</td><td className="project-modified" title={new Date(item.updated_at).toLocaleString()}>{directoryDate(item.updated_at)}</td><td>{actions(item)}</td></tr>)}</tbody></table></div> : <ul className="project-card-grid">{visible.map((item) => <li key={`${item.kind}-${item.id}`}><div className="project-card-top">{icon(item)}{actions(item)}</div><button type="button" className="project-card-title" aria-label={`Open ${item.title}`} onClick={() => open(item)}><strong>{item.title}</strong><span>{description(item)}</span></button><div className="project-card-meta">{type(item)}{item.is_draft && <span className="project-draft">Draft</span>}<span>{directoryDate(item.updated_at)}</span></div>{relationships(item)}</li>)}</ul>}
          <footer className="project-contents-footer"><span role="status">{items.length ? `${(page - 1) * browse.pageSize + 1}–${Math.min(page * browse.pageSize, items.length)} of ${items.length} items` : '0 items'}</span><div><label>Rows <select aria-label="Project rows per page" value={browse.pageSize} onChange={(event) => change({ pageSize: Number(event.target.value) })}>{[10, 25, 50].map((n) => <option key={n} value={n}>{n}</option>)}</select></label><nav aria-label="Project contents pages"><button type="button" className="project-icon-button" aria-label="Previous project page" disabled={page <= 1} onClick={() => change({ page: page - 1 })}><ChevronLeft size={16} /></button><span>{page} / {pageCount}</span><button type="button" className="project-icon-button" aria-label="Next project page" disabled={page >= pageCount} onClick={() => change({ page: page + 1 })}><ChevronRight size={16} /></button></nav></div></footer>
        </section>
        <div className="project-page-footer"><span><CalendarDays size={14} />Created {directoryDate(data.project.created_at)}</span><span>Shared items keep one content history across projects.</span></div>
      </>}
    </div></div>
    {showConnections && data && <ConnectionsPanel entity={{ kind: 'project', id: projectId }} title={data.project.name} scope="project" onScopeChange={() => {}} onClose={closeConnections} onPreviewChange={setPreview} onConnectionsChange={refreshed} onOpenFull={(item) => item.kind === 'project' ? onOpenProject(item.id) : onOpenArtifact(item.id, item.project_id)} />}
    </div>
    {adding && <AddExistingDialog projectId={projectId} memberIds={memberIds} onClose={() => { setAdding(false); addButton.current?.focus(); }} onAdded={refreshed} />}
  </section>;
}
