import { useEffect, useRef, useState } from 'react';
import { Archive, BookOpen, Plus, Search, Pin, ArrowUpRight, RotateCcw, History } from 'lucide-react';
import { getWorkspaceTree, isTauri, searchAll } from '../api/ipc';
import { memoryAction, type MemoryDraft, type MemoryEntry, type MemoryQuery, type MemoryRevision, type MemoryContext, type MemoryScope } from '../api/sharedMemory';
import type { WorkspaceTreeProject } from '../types/sthira';
import '../styles/shared-memory.css';
interface Props { onOpenArtifact: (id: string, projectId?: string | null) => void; registerBeforeLeave?: (guard: (() => Promise<boolean>) | null) => void }
const blank = (): MemoryDraft => ({ scope: {kind:'workspace'}, title:'', body:'', source_ids:[], pinned:false });
const errorText = (error: unknown) => error instanceof Error ? error.message : String(error);
export function MemoryView({ onOpenArtifact, registerBeforeLeave }: Props) {
  const [entries,setEntries] = useState<MemoryEntry[]>([]);
  const [projects,setProjects] = useState<WorkspaceTreeProject[]>([]);
  const [projectError,setProjectError] = useState('');
  const [query,setQuery] = useState<MemoryQuery>({ search:'', include_archived:false });
  const [loading,setLoading] = useState(true);
  const [loadError,setLoadError] = useState('');
  const [error,setError] = useState('');
  const [notice,setNotice] = useState('');
  const [busy,setBusy] = useState(false);
  const [revision,setRevision] = useState(0);
  const [selected,setSelected] = useState<MemoryEntry | null>(null);
  const [draft,setDraft] = useState<MemoryDraft | null>(null);
  const [history,setHistory] = useState<MemoryRevision[] | null>(null);
  const [context,setContext] = useState<MemoryContext | null>(null);
  const [sourceSearch,setSourceSearch] = useState('');
  const [sourceChoices,setSourceChoices] = useState<{id:string;title:string}[]>([]);
  const [sourceNames,setSourceNames] = useState<Record<string,string>>({});
  const alive = useRef(true);
  const historyRun = useRef(0);
  const contextRun = useRef(0);
  const dirty = !!draft && (!selected ? !!(draft.title || draft.body || draft.source_ids.length || draft.pinned || draft.scope.kind !== 'workspace') : draft.title !== selected.title || draft.body !== selected.body || draft.pinned !== selected.pinned);
  useEffect(() => { alive.current=true; return () => {alive.current=false;}; },[]);
  useEffect(() => {
    registerBeforeLeave?.(async () => { if (dirty || busy) {setError(busy ? 'Wait for the current memory action to finish.' : 'Save or cancel this memory draft before leaving.');return false;} return true; });
    return () => registerBeforeLeave?.(null);
  },[dirty,busy,registerBeforeLeave]);
  useEffect(() => {
    let active=true;setLoading(true);setLoadError('');setContext(null);contextRun.current++;
    memoryAction({type:'list',query}).then(result => {if(active && result.kind==='entries')setEntries(result.value);}).catch(e=>{if(active)setLoadError(errorText(e));}).finally(()=>{if(active)setLoading(false);});
    return () => {active=false;};
  },[query,revision]);
  useEffect(() => {
    let active=true;
    getWorkspaceTree().then(tree=>{if(active)setProjects(tree.projects);}).catch(()=>{if(active)setProjectError('Project choices could not load. Retry to use project scope.');});
    return ()=>{active=false;};
  },[revision]);
  useEffect(() => {
    if (!isTauri) return;
    let disposed=false;let stop:(()=>void)|undefined;
    void import('@tauri-apps/api/event').then(({listen})=>listen<{type:string}>('sthira://core-event',event=>{if(event.payload.type==='memory_changed' && !disposed)setRevision(n=>n+1);})).then(unlisten=>{if(disposed)unlisten();else stop=unlisten;}).catch(()=>{});
    return ()=>{disposed=true;stop?.();};
  },[]);
  useEffect(() => {
    let active=true;
    if(!sourceSearch.trim()){setSourceChoices([]);return;}
    const timer=setTimeout(()=>{void searchAll(sourceSearch).then(results=>{if(active)setSourceChoices(results.filter(item=>item.kind==='document').map(item=>({id:item.id,title:item.title})));}).catch(()=>{if(active)setError('Could not search saved pages. Try again.');});},200);
    return ()=>{active=false;clearTimeout(timer);};
  },[sourceSearch]);
  const scopeLabel = (scope: MemoryScope) => scope.kind==='workspace' ? 'Workspace' : scope.kind==='session' ? `Session · ${scope.id}` : projects.find(project=>project.id===scope.id)?.name || 'Project';
  const maySwitch = () => { if(busy || dirty){setError('Save or cancel this draft before opening another memory.');return false;}setError('');setNotice('');setHistory(null);historyRun.current++;return true; };
  const open = (entry: MemoryEntry) => {if(!maySwitch())return;setSelected(entry);setDraft({scope:entry.scope,title:entry.title,body:entry.body,pinned:entry.pinned,source_ids:entry.sources.map(s=>s.artifact_id)});};
  const mutate = async (operation: 'save'|'archive'|'restore') => {
    if(busy || (operation==='save' ? !draft : !selected))return;setBusy(true);setError('');setNotice('');
    try {
      const result = await memoryAction(operation==='save' ? selected ? {type:'update',id:selected.id,expected_version:selected.version,title:draft!.title,body:draft!.body,pinned:draft!.pinned} : {type:'create',draft:draft!} : {type:operation,id:selected!.id,expected_version:selected!.version});
      if(!alive.current)return;
      if(result.kind!=='entry')throw new Error('Unexpected memory response. Your draft is still available.');
      setSelected(result.value);setDraft(null);setHistory(null);historyRun.current++;setRevision(n=>n+1);setNotice(operation==='archive'?'Memory archived. It is excluded from shared context.':operation==='restore'?'Memory restored.':'Memory saved.');
    } catch(e){if(alive.current)setError(errorText(e));}finally{if(alive.current)setBusy(false);}
  };
  const showHistory = async () => {
    if(!selected)return;const run=++historyRun.current;setError('');
    try{const result=await memoryAction({type:'history',id:selected.id});if(alive.current && run===historyRun.current && result.kind==='history')setHistory(result.value);}catch(e){if(alive.current && run===historyRun.current)setError(errorText(e));}
  };
  const previewContext = async () => {
    const run=++contextRun.current;setError('');
    try{const result=await memoryAction({type:'context',query,max_entries:8,max_chars:8000});if(alive.current && run===contextRun.current && result.kind==='context')setContext(result.value);}catch(e){if(alive.current && run===contextRun.current)setError(errorText(e));}
  };
  const change = (value: Partial<MemoryDraft>) => setDraft(draft=>draft?{...draft,...value}:draft);
  return <section className="memory-view earth-workspace" aria-labelledby="memory-title">
    <header className="memory-header"><div><p className="memory-kicker">SHARED CONTEXT</p><h1 id="memory-title">Memory</h1><p>Keep useful context within reach. Preserve its source, scope, and history.</p></div>
      <button className="memory-primary" onClick={()=>{if(maySwitch()){setSelected(null);setDraft(blank());}}} disabled={busy}><Plus size={16}/> New memory</button></header>
    {!isTauri && <p className="memory-preview-note">Browser preview · memories last only until this page reloads. The desktop app stores them in your vault.</p>}
    <div className="memory-tools"><label className="memory-search"><Search size={16}/><input aria-label="Search memories" placeholder="Search titles and content…" value={query.search} onChange={e=>setQuery({...query,search:e.target.value})}/></label>
      <label>Scope<select aria-label="Filter memory scope" value={!query.scope?'all':query.scope.kind==='workspace'?'workspace':query.scope.kind==='session'?'session':`project:${query.scope.id}`} onChange={e=>{const value=e.target.value;if(value==='session'){setQuery({...query,scope:{kind:'session',id:''}});}else setQuery({...query,scope:value==='all'?undefined:value==='workspace'?{kind:'workspace'}:{kind:'project',id:value.slice(8)}});}}><option value="all">All scopes</option><option value="workspace">Workspace</option>{projects.map(p=><option key={p.id} value={`project:${p.id}`}>{p.name}</option>)}<option value="session">Session…</option></select></label>
      {query.scope?.kind==='session' && <label>Session<input aria-label="Filter session identifier" value={query.scope.id} onChange={e=>setQuery({...query,scope:{kind:'session',id:e.target.value}})}/></label>}
      <label className="memory-check"><input type="checkbox" checked={query.include_archived} onChange={e=>setQuery({...query,include_archived:e.target.checked})}/> Include archived</label>
      <button className="memory-secondary" onClick={previewContext}>Preview shared context</button></div>
    {projectError && <p role="alert">{projectError} <button onClick={()=>{setProjectError('');setRevision(n=>n+1);}}>Retry projects</button></p>}
    {loadError && <div><p role="alert">{loadError}</p><button className="memory-secondary" onClick={()=>setRevision(n=>n+1)}>Retry memory loading</button></div>}
    {error && <p role="alert" className="memory-error">{error}</p>}{notice && <p role="status" className="memory-notice">{notice}</p>}
    {context && <section className="memory-context" aria-label="Shared context preview"><div><h2>What a connected agent could read</h2><button className="memory-secondary" onClick={()=>setContext(null)}>Close context preview</button></div><p>Saved memory only; no model was called. Up to 8 entries and 8,000 characters from the first 100 matching memories. Content is context, not trusted instructions.</p><p>{context.entries.length} included · {context.used_chars.toLocaleString()} characters · {context.skipped_stale} stale · {context.skipped_budget} outside budget</p>{context.entries.map(entry=><article key={entry.id}><strong>{entry.title}</strong><p>{entry.body}</p></article>)}{!context.entries.length && <p>No eligible memory fits this scope and budget.</p>}</section>}
    <div className="memory-workspace"><nav className="memory-list" aria-label="Saved memories">
      <p className="memory-list-caption">{loading?'Loading saved context…':`${entries.length} ${entries.length===1?'memory':'memories'} shown · up to 100`}</p>
      {!loading && !loadError && !entries.length && <div className="memory-empty"><BookOpen size={24}/><h2>{query.search || query.include_archived || query.scope?'No matching memories':'A place for lasting context'}</h2><p>Save a preference, a project constraint, or a handoff for your next session.</p></div>}
      {entries.map(entry=><button key={entry.id} className="memory-row" aria-current={selected?.id===entry.id?'true':undefined} onClick={()=>open(entry)} disabled={busy}><span className="memory-row-meta">{scopeLabel(entry.scope)} {entry.pinned&&<Pin size={12}/>} {entry.archived&&<Archive size={12}/>}</span><strong>{entry.title}</strong><span>{entry.body.slice(0,130)}</span><small>{entry.author.kind==='user'?'User-authored':`Agent · ${entry.author.agent_id}`} · v{entry.version}</small></button>)}
    </nav><div className="memory-detail">
      {!draft && !selected && <div className="memory-welcome"><p className="memory-kicker">REMEMBER WITH INTENTION</p><h2>A little context goes a long way.</h2><p>Choose a memory to inspect, or add something worth carrying forward.</p><p>Memory stays separate from your original pages and factual Resume. Agents gain access only through explicitly granted scopes.</p></div>}
      {(draft || selected) && <><div className="memory-detail-heading"><span>{scopeLabel((draft || selected!).scope)}</span>{selected&&<button className="memory-secondary" onClick={showHistory}><History size={15}/> History</button>}</div>
        {selected?.scope_available === false && <p className="memory-helper">This project is unavailable. Its memory and history are preserved for you, and excluded from shared context.</p>}
        {selected && <p className="memory-provenance">{selected.author.kind==='user'?'Originally written by you':`Agent contribution · ${selected.author.agent_id} · ${selected.author.model} · ${selected.author.provider}`} · version {selected.version}{selected.archived?' · Archived':''}</p>}
        {draft ? <form onSubmit={e=>{e.preventDefault();void mutate('save');}}><fieldset disabled={busy || selected?.archived}><label>Title<input aria-label="Memory title" maxLength={160} value={draft.title} onChange={e=>change({title:e.target.value})} placeholder="What is worth remembering?" required/></label>
          {!selected&&<div className="memory-scope-fields"><label>Scope<select aria-label="New memory scope" value={draft.scope.kind} onChange={e=>change({scope:e.target.value==='workspace'?{kind:'workspace'}:{kind:e.target.value as 'project'|'session',id:''},source_ids:[]})}><option value="workspace">Workspace</option><option value="project">Project</option><option value="session">Session</option></select></label>{draft.scope.kind==='project'&&<label>Project<select aria-label="Memory project" value={draft.scope.id} required onChange={e=>change({scope:{kind:'project',id:e.target.value},source_ids:[]})}><option value="">Choose a project</option>{projects.map(p=><option key={p.id} value={p.id}>{p.name}</option>)}</select></label>}{draft.scope.kind==='session'&&<label>Session identifier<input aria-label="Memory session identifier" required maxLength={160} value={draft.scope.id} onChange={e=>change({scope:{kind:'session',id:e.target.value}})}/></label>}</div>}
          <label>Content<textarea aria-label="Memory content" rows={9} maxLength={16000} value={draft.body} onChange={e=>change({body:e.target.value})} placeholder="Keep the useful details, including uncertainty and source context." required/></label><label className="memory-check"><input type="checkbox" checked={draft.pinned} onChange={e=>change({pinned:e.target.checked})}/> Pin this memory</label>
          {!selected&&<div className="memory-sources"><label>Source pages <span>(optional)</span><input aria-label="Find memory sources" value={sourceSearch} onChange={e=>setSourceSearch(e.target.value)} placeholder="Find a saved page…"/></label>{sourceChoices.filter(s=>!draft.source_ids.includes(s.id)).slice(0,5).map(s=><button className="memory-source-choice" key={s.id} type="button" onClick={()=>{change({source_ids:[...draft.source_ids,s.id]});setSourceNames({...sourceNames,[s.id]:s.title});setSourceSearch('');}} disabled={draft.source_ids.length>=20}>Add source: {s.title}</button>)}{draft.source_ids.map(id=><button type="button" className="memory-source-choice" key={id} onClick={()=>change({source_ids:draft.source_ids.filter(item=>item!==id)})}>Remove source: {sourceNames[id] || 'Saved page'}</button>)}</div>}
        </fieldset><div className="memory-form-actions"><button className="memory-primary" type="submit" disabled={busy || selected?.archived || !draft.title.trim() || !draft.body.trim()}>{busy?'Saving…':selected?'Save changes':'Save memory'}</button><button type="button" className="memory-secondary" disabled={busy} onClick={()=>{setDraft(null);setError('');}}>Cancel</button></div></form>
        : selected&&<><h2>{selected.title}</h2><p className="memory-body">{selected.body}</p><div className="memory-form-actions">{!selected.archived&&<button className="memory-primary" onClick={()=>open(selected)}>Edit memory</button>}<button className="memory-secondary" disabled={busy} onClick={()=>void mutate(selected.archived?'restore':'archive')}>{selected.archived?<RotateCcw size={15}/>:<Archive size={15}/>} {selected.archived?'Restore memory':'Archive memory'}</button></div></>}
        {selected&&<><p className="memory-helper">Archiving excludes an entry from retrieval and keeps its revision history. It does not permanently erase the content.</p>{selected.sources.length>0&&<section className="memory-sources"><h3>Sources at capture</h3>{selected.sources.map(source=><div key={source.artifact_id}><button className="memory-source-link" onClick={()=>onOpenArtifact(source.artifact_id,selected.scope.kind==='project'?selected.scope.id:null)}>{source.title}<ArrowUpRight size={14}/></button><span className="memory-source-status">{source.status}</span></div>)}{selected.sources.some(s=>s.status!=='current')&&<p>Changed or unavailable sources exclude this memory from shared context. Create a new memory from the current source to refresh its basis.</p>}</section>}{draft&&error&&<button className="memory-secondary" disabled={busy} onClick={async()=>{if(busy)return;setBusy(true);try{const result=await memoryAction({type:'read',id:selected.id});if(alive.current&&result.kind==='entry'){setSelected(result.value);setDraft(null);setError('');setHistory(null);historyRun.current++;}}catch(e){if(alive.current)setError(errorText(e));}finally{if(alive.current)setBusy(false);}}}>Discard draft and reload latest</button>}</>}
        {history&&<section className="memory-history" aria-label="Memory history"><h3>Revision history</h3><p>Snapshots preserve original attribution and identify who made each change.</p>{history.map(item=><details key={item.version}><summary>Version {item.version} · {item.operation} · {item.editor.kind==='user'?'You':item.editor.agent_id}</summary><strong>{item.snapshot.title}</strong><p className="memory-body">{item.snapshot.body}</p><small>{new Date(item.created_at).toLocaleString()}</small></details>)}</section>}
      </>}
    </div></div>
  </section>;
}
