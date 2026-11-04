import React, { useEffect, useRef, useState } from 'react';
import { ArrowLeft, ArrowDownLeft, ArrowUpRight, Check, ChevronRight, FileText, Folder, Link2, Loader2, Plus, Search, Unlink, X } from 'lucide-react';
import type { EntityRef, WorkspaceConnection, WorkspaceEntity, WorkspaceEntityDetail } from '../types/sthira';
import { createWorkspaceConnection, deleteWorkspaceConnection, getWorkspaceConnections, getWorkspaceEntity, sameEntity, searchConnectionTargets } from '../api/connections';
import { RichDocumentRenderer } from './RichDocumentRenderer';

interface Props {
  entity: EntityRef;
  title: string;
  project?: { id: string; title: string } | null;
  scope: 'page' | 'project';
  onScopeChange: (scope: 'page' | 'project') => void;
  onClose: () => void;
  onOpenFull?: (entity: WorkspaceEntity) => void;
  onInsertLink?: (entity: WorkspaceEntity) => void;
  requestedPreview?: { entity: EntityRef; sequence: number } | null;
  onPreviewChange: (open: boolean) => void;
  onConnectionsChange?: () => void;
}

type Frame = { entity: EntityRef; title: string; scroll: number };

export const ConnectionsPanel: React.FC<Props> = ({ entity, title, project, scope, onScopeChange, onClose, onOpenFull, onInsertLink, requestedPreview, onPreviewChange, onConnectionsChange }) => {
  const [trail, setTrail] = useState<Frame[]>([]);
  const [detail, setDetail] = useState<WorkspaceEntityDetail | null>(null);
  const [links, setLinks] = useState<WorkspaceConnection[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const [picker, setPicker] = useState(false);
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<WorkspaceEntity[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [removed, setRemoved] = useState<WorkspaceConnection | null>(null);
  const scroll = useRef<HTMLDivElement>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  const backButton = useRef<HTMLButtonElement>(null);
  const addButton = useRef<HTMLButtonElement>(null);
  const rootScroll = useRef(0);
  const returningToRoot = useRef(false);
  const alive = useRef(true);
  const navigationKey = useRef('');
  const current = trail[trail.length - 1]?.entity || entity;
  const currentTitle = detail?.entity.title || trail[trail.length - 1]?.title || title;
  const preview = trail.length > 0;
  navigationKey.current = `${current.kind}:${current.id}`;
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);

  useEffect(() => {
    setTrail([]); setPicker(false); setRemoved(null); setActionError(null); rootScroll.current = 0;
  }, [entity.id, entity.kind]);

  useEffect(() => { onPreviewChange(preview); }, [preview, onPreviewChange]);

  const open = (target: EntityRef, targetTitle = 'Reference') => {
    setPicker(false); setActionError(null); setRemoved(null);
    if (sameEntity(target, current)) return;
    const position = scroll.current?.scrollTop || 0;
    if (trail.length === 0) rootScroll.current = position;
    setTrail((prev) => [...prev.map((frame, index) => index === prev.length - 1 ? { ...frame, scroll: position } : frame), { entity: target, title: targetTitle, scroll: 0 }]);
  };

  useEffect(() => {
    if (requestedPreview) open(requestedPreview.entity);
    // The sequence is an explicit navigation request, including repeated clicks.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [requestedPreview?.sequence]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true); setError(null); setDetail(null); setLinks([]);
    Promise.all([getWorkspaceEntity(current), getWorkspaceConnections(current)]).then(([item, connections]) => {
      if (cancelled) return;
      setDetail(item || null); setLinks(connections || []); setLoading(false);
      if (item) setTrail((prev) => prev.map((frame, index) => index === prev.length - 1 && sameEntity(frame.entity, item.entity) ? { ...frame, title: item.entity.title } : frame));
    }).catch(() => {
      if (!cancelled) { setError('Couldn’t load this reference. It may have been removed, or the vault may be unavailable.'); setLoading(false); }
    });
    return () => { cancelled = true; };
  }, [current.id, current.kind, refresh]);

  useEffect(() => {
    if (!loading && scroll.current) scroll.current.scrollTop = trail[trail.length - 1]?.scroll || (preview ? 0 : rootScroll.current);
    if (!loading && preview) backButton.current?.focus({ preventScroll: true });
    if (!loading && !preview && returningToRoot.current && detail && sameEntity(detail.entity, current)) {
      addButton.current?.focus({ preventScroll: true }); returningToRoot.current = false;
    }
    // Restore each reading position only when navigation/load settles.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loading, current.id, current.kind, detail?.entity.id]);

  useEffect(() => {
    if (!picker) return;
    let cancelled = false;
    setSearching(true); setSearchError(null);
    const timer = window.setTimeout(() => {
      searchConnectionTargets(query).then((items) => {
        if (!cancelled) { setResults(items || []); setSearching(false); }
      }).catch(() => {
        if (!cancelled) { setResults([]); setSearching(false); setSearchError('Couldn’t search the vault. Try again.'); }
      });
    }, 180);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [picker, query, refresh]);

  useEffect(() => { if (picker) searchInput.current?.focus(); }, [picker]);

  const back = () => { if (trail.length === 1) returningToRoot.current = true; setTrail((prev) => prev.slice(0, -1)); setPicker(false); setActionError(null); setRemoved(null); };
  const closePicker = () => { setPicker(false); addButton.current?.focus(); };
  const add = async (target: WorkspaceEntity, insert: boolean) => {
    if (busy) return;
    setBusy(true); setActionError(null);
    const sourceKey = navigationKey.current;
    try {
      await createWorkspaceConnection(current, target);
      if (!alive.current || navigationKey.current !== sourceKey) return;
      if (insert) onInsertLink?.(target);
      setPicker(false); setQuery(''); setRefresh((value) => value + 1); setRemoved(null);
      onConnectionsChange?.();
      addButton.current?.focus();
    } catch { setActionError('Couldn’t create the connection. Your page has not changed. Try again.'); }
    finally { setBusy(false); }
  };
  const remove = async (link: WorkspaceConnection) => {
    if (busy) return;
    setBusy(true); setActionError(null);
    try { await deleteWorkspaceConnection(link.id); setRemoved(link); setRefresh((value) => value + 1); onConnectionsChange?.(); }
    catch { setActionError('Couldn’t remove this connection. Try again.'); }
    finally { setBusy(false); }
  };
  const undo = async () => {
    if (!removed || busy) return;
    setBusy(true); setActionError(null);
    try { await createWorkspaceConnection(removed.source, removed.target); setRemoved(null); setRefresh((value) => value + 1); onConnectionsChange?.(); }
    catch { setActionError('Couldn’t restore the connection. Try again.'); }
    finally { setBusy(false); }
  };
  const outgoing = links.filter((link) => sameEntity(link.source, current));
  const incoming = links.filter((link) => sameEntity(link.target, current));
  const available = results.filter((result) => !sameEntity(result, current));

  const group = (items: WorkspaceConnection[], direction: 'outgoing' | 'incoming') => (
    <section className="connection-group" aria-label={direction === 'outgoing' ? 'Links from this item' : 'Backlinks to this item'}>
      <h3>{direction === 'outgoing' ? <ArrowUpRight size={14} /> : <ArrowDownLeft size={14} />}{direction === 'outgoing' ? 'Links from here' : 'Linked from'}<span>{items.length}</span></h3>
      {items.length === 0 ? <p className="connection-muted">{direction === 'outgoing' ? 'Connect a page, project, or object.' : 'Items that link here will appear here.'}</p> : (
        <ul className="connection-tree">{items.map((link) => {
          const target = direction === 'outgoing' ? link.target : link.source;
          return <li key={link.id}>
            <button type="button" className="connection-card" onClick={() => open(target, target.title)} aria-label={`Preview ${target.title}`}>
              <span className="connection-entity-icon">{target.kind === 'project' ? <Folder size={17} /> : <FileText size={17} />}</span>
              <span className="connection-card-copy"><small>{link.relationship === 'derived_from' ? direction === 'outgoing' ? 'Derived from' : 'Derived item' : target.type_label}</small><strong>{target.title}</strong></span><ChevronRight size={15} />
            </button>
            {link.relationship !== 'derived_from' && <button type="button" className="connection-unlink" aria-label={`Remove connection ${link.source.title} to ${link.target.title}`} title="Remove connection card; page content is preserved" disabled={busy} onClick={() => remove(link)}><Unlink size={14} /></button>}
          </li>;
        })}</ul>
      )}
    </section>
  );

  return <aside className={`connections-panel ${preview ? 'connections-preview' : ''}`} id="workspace-connections" aria-label="Connections" onKeyDown={(event) => {
    if (event.key !== 'Escape') return;
    event.preventDefault(); event.stopPropagation();
    if (busy) return;
    if (picker) closePicker(); else if (preview) back(); else onClose();
  }}>
    <header className="connections-heading">
      {preview ? <button ref={backButton} type="button" className="connection-back" onClick={back}><ArrowLeft size={16} />{trail.length > 1 ? 'Back' : 'Connections'}</button> : <span><Link2 size={16} />Connections</span>}
      <button type="button" className="focus-icon-button" aria-label="Close connections" onClick={onClose}><X size={17} /></button>
    </header>
    {!preview && project && <div className="connections-scope" role="group" aria-label="Connection scope">
      <button type="button" aria-pressed={scope === 'page'} onClick={() => onScopeChange('page')}>This page</button>
      <button type="button" aria-pressed={scope === 'project'} onClick={() => onScopeChange('project')}>Project</button>
    </div>}
    <div className="connections-scroll" ref={scroll}>
      {preview && <nav className="connection-trail" aria-label="Reference trail">
        <button type="button" onClick={() => { returningToRoot.current = true; setTrail([]); setPicker(false); }}>{title}</button>
        {trail.map((frame, index) => <React.Fragment key={`${frame.entity.kind}-${frame.entity.id}-${index}`}><ChevronRight size={12} /><button type="button" aria-current={index === trail.length - 1 ? 'page' : undefined} onClick={() => { setTrail((prev) => prev.slice(0, index + 1)); setPicker(false); }}>{index === trail.length - 1 ? currentTitle : frame.title}</button></React.Fragment>)}
      </nav>}
      <div className="connection-anchor">
        <span className="connection-eyebrow">{preview ? 'REFERENCE PREVIEW' : 'CONNECTED TO'}</span>
        <h2>{currentTitle}</h2>
        <p>{preview ? detail?.entity.type_label || 'Reference' : 'Follow a connection without leaving your work.'}</p>
        {preview && detail && onOpenFull && <button type="button" className="connection-open-full" onClick={() => onOpenFull(detail.entity)}>Open in workspace<ArrowUpRight size={14} /></button>}
      </div>
      {loading ? <div className="connection-state" role="status"><Loader2 size={18} className="animate-spin" />Loading connections…</div> : error ? <div className="connection-state" role="alert"><p>{error}</p><button type="button" onClick={() => setRefresh((value) => value + 1)}>Try again</button></div> : <>
        {preview && detail && <section className="connection-document" aria-label="Reference content">
          {detail.object_instance && <dl className="connection-properties" aria-label="Object properties">{Object.entries(detail.object_instance.properties).map(([key, value]) => <div key={key}><dt>{detail.object_type?.schema.find((field) => field.key === key)?.label || key}</dt><dd>{value == null || value === '' ? 'Empty' : typeof value === 'boolean' ? value ? 'Yes' : 'No' : Array.isArray(value) ? value.join(', ') : typeof value === 'object' ? JSON.stringify(value) : String(value)}</dd></div>)}</dl>}
          {detail.entity.kind === 'project' ? <>
            <p className="connection-project-purpose">{detail.description || 'No purpose recorded yet.'}</p>
            <h3>Pages & objects <span>{detail.members.length}</span></h3>
            {detail.members.length === 0 ? <p className="connection-muted">This project has no pages yet.</p> : <ul>{detail.members.map((member) => <li key={member.id}><button type="button" className="connection-member" onClick={() => open(member, member.title)}><FileText size={16} /><span>{member.title}<small>{member.type_label}</small></span><ChevronRight size={14} /></button></li>)}</ul>}
          </> : detail.content_md.trim() ? <RichDocumentRenderer content={detail.content_md} documentTitle={detail.entity.title} className="focus-prose connection-prose" onOpenEntity={(target) => open(target)} /> : <p className="connection-muted">This page is empty.</p>}
        </section>}
        {actionError && <p className="connection-error" role="alert">{actionError}</p>}
        {removed && <div className="connection-notice" role="status"><span>Connection removed.</span><button type="button" disabled={busy} onClick={undo}>Undo</button></div>}
        <button type="button" className="connection-add" ref={addButton} disabled={busy} onClick={() => { setPicker(true); setActionError(null); }} aria-expanded={picker}><Plus size={16} />Link an item</button>
        {picker && <section className="connection-picker" aria-label="Link an item">
          <div className="connection-picker-title"><h3>Find a connection</h3><button type="button" className="focus-icon-button" aria-label="Cancel linking" disabled={busy} onClick={closePicker}><X size={16} /></button></div>
          <label className="connection-search"><Search size={16} /><input ref={searchInput} value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Pages, projects, objects…" aria-label="Search connection targets" /></label>
          <p className="connection-muted">Search by title or object type. Showing up to 60 items.</p>
          {searching ? <p className="connection-muted" role="status">Searching…</p> : searchError ? <div role="alert"><p className="connection-error">{searchError}</p><button type="button" onClick={() => setRefresh((value) => value + 1)}>Try again</button></div> : available.length === 0 ? <p className="connection-muted">No matching items. Try another title.</p> : <ul className="connection-results">{available.map((target) => {
            const linked = outgoing.some((link) => sameEntity(link.target, target));
            return <li key={`${target.kind}-${target.id}`}><button type="button" disabled={busy || linked} onClick={() => add(target, false)} aria-label={linked ? `${target.title} already linked` : `Link ${target.title}`}>
              {target.kind === 'project' ? <Folder size={16} /> : <FileText size={16} />}<span><strong>{target.title}</strong><small>{target.type_label}</small></span>{linked ? <Check size={15} /> : <Plus size={15} />}
            </button>{!preview && entity.kind === 'artifact' && onInsertLink && <button type="button" className="connection-insert" disabled={busy} onClick={() => add(target, true)} aria-label={`Insert link to ${target.title}`}>Insert in page</button>}</li>;
          })}</ul>}
        </section>}
        {group(outgoing, 'outgoing')}
        {group(incoming, 'incoming')}
      </>}
    </div>
    <footer className="connections-footer"><Link2 size={13} /><span>{preview ? 'Your original page stays open.' : 'Links preserve each item’s own content.'}</span></footer>
  </aside>;
};
