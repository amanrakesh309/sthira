import { useCallback, useEffect, useRef, useState } from 'react';
import { sharedAgents, type SharedAgentConnection, type SharedAgentProposal, type SharedConfig, type SharedNativeRun } from '../api/sharedAgents';
import { getProposalDetails, getSharedProject, listSharedRevisions, listSharedProjects, listWorkspaces, makeRequestIdempotencyKey, type PagedList, type SharedProjectDetails, type SharedPageRevisionView, type SharedPageSummary, type SharedProjectSummary, type WorkspaceListResponse, type WorkspaceResponse, type WorkspaceRole, type WorkspaceSummary } from '../api/workspaces';
import { listAvailableModels } from '../api/ipc';
import { subscribeCollabEvents } from '../api/liveCollab';
import type { ModelView } from '../types/sthira';
import type { GoogleSession } from './AccountModal';
import { RichDocumentRenderer } from './RichDocumentRenderer';
import { ContextInfo } from './ContextInfo';
import '../styles/shared-agents.css';

interface Props {
  workspaceId: string; myRole: WorkspaceRole; currentUid: string | null;
  onSessionChange?: (session: GoogleSession) => void;
  onAccessLost?: (message: string, workspaces?: WorkspaceListResponse | null) => void;
  initialProposalId?: string | null;
  onClose?: () => void;
  onOpenPage?: (pageId: string, projectId?: string | null) => void;
}
const button = 'sthira-button-secondary';
const section = 'rounded-xl border border-sthira-border bg-sthira-surface p-4 space-y-3';

export function SharedAgentsPanel(props: Props) {
  // Identity is a mount boundary: pending promises from another account never populate this panel.
  return <SharedAgentsBody key={`${props.workspaceId}:${props.currentUid}`} {...props} />;
}
function SharedAgentsBody({ workspaceId, myRole, currentUid, onSessionChange, onAccessLost, initialProposalId, onOpenPage, onClose }: Props) {
  const [loaded, setLoaded] = useState(false);
  const [accessRevision,setAccessRevision]=useState(0);
  const refreshSeq=useRef(0);
  const [snapshot, setSnapshot] = useState<SharedPageRevisionView | null>(null);
  const sourceTrigger = useRef<HTMLButtonElement | null>(null);
  const [policy, setPolicy] = useState(false);
  const [connections, setConnections] = useState<SharedAgentConnection[]>([]);
  const [proposals, setProposals] = useState<SharedAgentProposal[]>([]);
  const [projects, setProjects] = useState<SharedProjectSummary[]>([]);
  const [project, setProject] = useState('');
  const [pages, setPages] = useState<SharedPageSummary[]>([]);
  const [selectedPages, setSelectedPages] = useState<string[]>([]);
  const [permission, setPermission] = useState<'read' | 'suggest'>('read');
  const [clientName, setClientName] = useState('Sthira native');
  const [label, setLabel] = useState('');
  const [editing, setEditing] = useState<SharedAgentConnection | null>(null);
  const [config, setConfig] = useState<SharedConfig[] | null>(null);
  const [format, setFormat] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [review, setReview] = useState<SharedAgentProposal | null>(null);
  const [baseContent, setBaseContent] = useState('');
  const [compareCurrent, setCompareCurrent] = useState(false);
  const [currentRevision, setCurrentRevision] = useState<number | null>(null);
  const reviewSeq = useRef(0);
  const reviewId = useRef<string | null>(null);
  const [currentContent, setCurrentContent] = useState('');
  const [note, setNote] = useState('');
  const [models, setModels] = useState<ModelView[]>([]);
  const [modelId, setModelId] = useState('');
  const [runConnection, setRunConnection] = useState('');
  const [kind, setKind] = useState<'answer' | 'edit' | 'newPage'>('answer');
  const [target, setTarget] = useState('');
  const [goal, setGoal] = useState('');
  const [runs, setRuns] = useState<SharedNativeRun[]>([]);
  const [openRun, setOpenRun] = useState<string | null>(null);
  const callbacks = useRef({onSessionChange,onAccessLost});
  callbacks.current={onSessionChange,onAccessLost};
  const initialReview = useRef<string | null>(null);
  const previousRole = useRef(myRole);
  const alive = useRef(true);
  const reviewKey = useRef<{sig:string;key:string} | null>(null);
  const canManage = myRole === 'owner' || myRole === 'admin';
  const canSuggest = myRole !== 'viewer';
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const unwrap = useCallback(<T,>(response: WorkspaceResponse<T>): T => {
    if (!alive.current) throw new Error('This view has closed.');
    if (response.outcome === 'ok') return response.data;
    if (response.outcome === 'sessionUpdated') { setConnections([]); setProposals([]); setRuns([]); setReview(null); callbacks.current.onSessionChange?.(response.session); throw new Error('Verify your account to continue.'); }
    if (response.outcome === 'accessLost') { setConnections([]); setProposals([]); setRuns([]); setReview(null); callbacks.current.onAccessLost?.(response.message, response.workspaces); throw new Error(response.message); }
    if (['agent_proposal_not_found','proposal_not_found','page_not_found','project_not_found'].includes(response.code)) {
      ++reviewSeq.current;reviewId.current=null;reviewKey.current=null;
      setSnapshot(null);setReview(null);setBaseContent('');setCurrentContent('');setCurrentRevision(null);
      setConnections([]);setProposals([]);setPages([]);setSelectedPages([]);setConfig(null);
    }
    throw new Error(response.message);
  }, []);
  const call = useCallback(async <T,>(action: string, input: unknown = {}) => unwrap(await sharedAgents<T>(workspaceId, action, input)), [workspaceId, unwrap]);
  const refresh = useCallback(async () => {
    const seq=++refreshSeq.current;
    const nextPolicy = await call<{ enabled:boolean }>('policy');
    const nextConnections = await call<SharedAgentConnection[]>('connections', {includeWorkspace:canManage});
    const nextProposals = await call<SharedAgentProposal[]>('proposals');
    const nextRuns = await call<SharedNativeRun[]>('runs');
    if (!alive.current || seq!==refreshSeq.current) return;
    setPolicy(nextPolicy.enabled); setConnections(nextConnections); setProposals(nextProposals); setRuns(nextRuns); setLoaded(true);
  }, [call, canManage]);
  useEffect(() => {
    let current=true;
    void refresh().catch(e => { if(current && alive.current) setError(String(e)); });
    void (async () => {let cursor:string|null=null;const all:SharedProjectSummary[]=[];do {const p: PagedList<SharedProjectSummary>=unwrap(await listSharedProjects(workspaceId,false,null,cursor,100));all.push(...p.items);cursor=p.nextCursor??null;}while(cursor && alive.current);return all;})().then(p => { if(current && alive.current) setProjects(p); }).catch(e => { if(current && alive.current) setError(String(e)); });
    void listAvailableModels().then(m => { if(current && alive.current) { setModels(m); setModelId(m.find(v => v.is_default)?.id ?? m[0]?.id ?? ''); } }).catch(() => {});
    return()=>{current=false;};
  }, [workspaceId, refresh, unwrap, accessRevision]);
  useEffect(() => {
    if (!runs.some(r => r.status === 'running')) return;
    const timer = window.setInterval(() => { void call<SharedNativeRun[]>('runs').then(r => { if(alive.current) setRuns(r); }).catch(e => { if(alive.current) setError(String(e)); }); }, 1000);
    return () => window.clearInterval(timer);
  }, [runs, call]);
  useEffect(() => {
    if (!project) { setPages([]); return; }
    let current = true;
    void (async () => {let cursor:string|null=null;const all:SharedPageSummary[]=[];do {const p: SharedProjectDetails=unwrap(await getSharedProject(workspaceId,project,null,cursor,100));all.push(...p.pages.map(v=>v.page));cursor=p.pagesNextCursor??null;}while(cursor && current && alive.current);return all;})().then(p => { if(current && alive.current) setPages(p); }).catch(e => { if(current && alive.current) setError(String(e)); });
    return () => { current = false; };
  }, [workspaceId, project, unwrap]);
  const openReview = useCallback(async (proposalId: string) => {
    const seq=++reviewSeq.current;
    setSnapshot(null);setCurrentContent('');setBaseContent('');setCurrentRevision(null);setCompareCurrent(false);if(reviewId.current!==proposalId){setNote('');reviewKey.current=null;}reviewId.current=proposalId;
    const response=await sharedAgents<SharedAgentProposal>(workspaceId,'proposal',{proposalId});
    if(!alive.current || seq!==reviewSeq.current) return;
    const next=unwrap(response);
    setReview(next);
    if(next.targetPageId) {
      const response=await getProposalDetails(workspaceId,next.proposalId);
      if(!alive.current || seq!==reviewSeq.current)return;
      const details=unwrap(response);
      if(alive.current && seq===reviewSeq.current) {setCurrentContent(details.currentPageContentMd);setBaseContent(details.baseContentMd);setCurrentRevision(details.currentPageRevisionNumber);setCompareCurrent(next.isStale);}
    }
  }, [call, workspaceId, unwrap]);
  useEffect(() => {
    if(previousRole.current===myRole)return;
    previousRole.current=myRole;
    const id=reviewId.current;
    ++reviewSeq.current;++refreshSeq.current;
    setSnapshot(null);setReview(null);setBaseContent('');setCurrentContent('');setCurrentRevision(null);
    setConnections([]);setProposals([]);setRuns([]);setOpenRun(null);setConfig(null);setEditing(null);
    setProjects([]);setProject('');setPages([]);setSelectedPages([]);setAccessRevision(value=>value+1);
    if(id)void openReview(id).catch(error=>{if(alive.current)setError(String(error));});
  },[myRole,openReview]);
  useEffect(() => {
    let active=true;let unlisten:(()=>void)|null=null;
    void subscribeCollabEvents(event => {
      if(!active || event.type!=='workspace_event' || event.workspaceId!==workspaceId || event.eventType!=='resource_access_changed') return;
      if(event.accountUid && event.accountUid!==currentUid) return;
      // Invalidate source-inspector promises before checking the current audience.
      const id=reviewId.current;++reviewSeq.current;++refreshSeq.current;setSnapshot(null);
      setProjects([]);setProject('');setPages([]);setSelectedPages([]);setAccessRevision(value=>value+1);
      if(id) void openReview(id).catch(error=>{if(active&&alive.current)setError(String(error));});
      void refresh().catch(error=>{if(active&&alive.current)setError(String(error));});
    }).then(stop=>{if(active)unlisten=stop;else stop();});
    return()=>{active=false;unlisten?.();};
  },[workspaceId,currentUid,openReview,refresh]);
  useEffect(() => { if(initialProposalId && initialReview.current!==initialProposalId) {initialReview.current=initialProposalId;void openReview(initialProposalId).catch(e => { if(alive.current) setError(String(e)); }); } }, [initialProposalId, openReview]);
  async function act(task: () => Promise<void>) {
    if(busy) return;
    setBusy(true); setError(''); setNotice('');
    try { await task(); if(alive.current) await refresh(); }
    catch(e) { if(alive.current) setError(e instanceof Error ? e.message : String(e)); }
    finally { if(alive.current) setBusy(false); }
  }
  async function decide(action: string) {
    if(!review) return;
    const sig = `${review.proposalId}:${review.submissionCount}:${action}:${note}`;
    if(reviewKey.current?.sig !== sig) reviewKey.current = {sig,key:makeRequestIdempotencyKey('agent-review')};
    const result = await call<SharedAgentProposal>('review', {proposalId:review.proposalId,review:{action,expectedSubmissionNumber:review.submissionCount,note:note.trim()||null,idempotencyKey:reviewKey.current.key}});
    setReview(result); if(action==='apply' && result.appliedRevisionNumber) {setCurrentRevision(result.appliedRevisionNumber);setCurrentContent(result.proposedContentMd);} setNote(''); reviewKey.current=null; setNotice(action === 'apply' ? 'Applied after independent review.' : 'Review saved. Published content is unchanged.');
  }
  async function inspectSource(source: SharedAgentProposal['sources'][number]) {
    const seq=reviewSeq.current;
    setSnapshot(null);
    let cursor: string | null = null;
    do {
      const response=await listSharedRevisions(workspaceId,source.pageId,cursor,100);
      if(!alive.current || seq!==reviewSeq.current)return;
      const revisions: PagedList<SharedPageRevisionView> = unwrap(response);
      const exact=revisions.items.find(r=>r.revisionNumber===source.revisionNumber);
      if(exact) {
        if(exact.contentSha256!==source.contentSha256) throw new Error('The source snapshot hash does not match. Refresh this review before deciding.');
        setSnapshot(exact);return;
      }
      cursor=revisions.nextCursor??null;
    } while(cursor && alive.current);
    throw new Error('The saved source revision is unavailable. No current page was substituted.');
  }
  const active = connections.filter(c => !c.revokedAtMs);
  const selectedConnection = active.find(c => c.connectionId === runConnection);
  const detailRun = runs.find(r => r.id === openRun);
  const snippet = config?.[format];
  if(review) return <section className="shared-agents space-y-4" aria-label="Shared agent proposal review">
    <div className="flex flex-wrap gap-2"><button className={button} disabled={busy} onClick={() => {if(onClose) onClose(); else {setReview(null);setError('');}}}>{onClose ? 'Back to page' : 'Back to agents'}</button><button className={button} disabled={busy} onClick={() => void act(() => openReview(review.proposalId))}>Refresh agent review</button></div>
    {error && <p role="alert">{error}</p>}{notice && <p role="status">{notice}</p>}
    <div className={section}><h2 className="text-lg font-semibold">{review.summary}</h2><p>{review.kind === 'newPage' ? 'New page' : 'Edit page'} · {review.projectName} · {review.status} · Submission {review.submissionCount}</p>
      <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-2 break-words"><dt>Initiated by</dt><dd>{review.authorDisplayName}</dd><dt>Agent</dt><dd>{review.agentLabel}</dd><dt>Connection</dt><dd>{review.connectionLabel ?? 'Unavailable'}</dd><dt>Model / provider</dt><dd>{review.modelLabel} / {review.providerLabel}</dd><dt>Attribution</dt><dd>Reported by the initiating app; not verified by the service</dd><dt>Connection state</dt><dd>{review.revocationState}</dd></dl>
      {review.isStale && review.status === 'pending' && <p role="alert">Cannot apply: {review.staleReason ?? 'The saved source basis has changed.'}</p>}
      {!review.reviewEligible && review.status === 'pending' && <p role="status">An independent Owner or Admin must apply this proposal. The initiating member cannot approve it.</p>}
      {review.reviewerNote && <p aria-label="Reviewer note" className="whitespace-pre-wrap">{review.reviewerNote}</p>}
      {review.reviewerDisplayName && <p>Reviewed by {review.reviewerDisplayName}{review.appliedRevisionNumber ? ` · revision ${review.appliedRevisionNumber}` : ''}</p>}
    </div>
    <div className="grid gap-4 md:grid-cols-2">{review.kind === 'edit' && <section className={section}><h3 className="font-semibold">{compareCurrent ? `Current published revision ${currentRevision ?? "…"}` : `Saved base revision ${review.baseRevisionNumber}`}</h3><button className={button} onClick={()=>setCompareCurrent(v=>!v)}>{compareCurrent ? "Compare saved base" : "Compare current revision"}</button><RichDocumentRenderer content={compareCurrent ? currentContent : baseContent} /></section>}<section className={section}><h3 className="font-semibold">Proposed: {review.proposedTitle}</h3><RichDocumentRenderer content={review.proposedContentMd} /></section></div>
    <section className={section}><h3 className="font-semibold">Source snapshots</h3><ul className="space-y-2">{review.sources.map(s => <li key={s.pageId}><button className={button} disabled={busy} onClick={e => {sourceTrigger.current=e.currentTarget;void act(()=>inspectSource(s));}}>{s.pageTitle} · rev {s.revisionNumber}</button><details><summary>Snapshot identity</summary><p className="break-all">Page {s.pageId} · SHA-256 {s.contentSha256}</p></details></li>)}</ul></section>
    {snapshot && <section className={section} aria-label="Saved source snapshot"><h3 className="font-semibold">{snapshot.title} · saved revision {snapshot.revisionNumber}</h3><p>Author: {snapshot.authorDisplayName}{snapshot.reviewerDisplayName && ` · Reviewer: ${snapshot.reviewerDisplayName}`}</p><div className="max-h-80 overflow-auto"><RichDocumentRenderer content={snapshot.contentMd} /></div><button className={button} disabled={busy} onClick={()=>{setSnapshot(null);sourceTrigger.current?.focus();}}>Close source snapshot</button>{onOpenPage && <button className={button} onClick={()=>onOpenPage(snapshot.pageId,review.projectId)}>Open current source page</button>}</section>}
    {review.status === 'pending' && canManage && <section className={section}><label className="block">Review note<textarea aria-label="Agent review note" value={note} onChange={e => setNote(e.target.value)} maxLength={2000} className="w-full" /></label><div className="flex flex-wrap gap-2"><button className="sthira-button-primary" disabled={busy || !review.reviewEligible || review.isStale} onClick={() => void act(() => decide('apply'))}>Apply agent proposal</button><button className={button} disabled={busy || (review.initiatedByCurrentUser ?? review.authorUid === currentUid)} onClick={() => void act(() => decide('request_changes'))}>Request changes</button><button className={button} disabled={busy || (review.initiatedByCurrentUser ?? review.authorUid === currentUid)} onClick={() => void act(() => decide('reject'))}>Dismiss agent proposal</button></div></section>}
    {review.status === 'applied' && (review.appliedPageId || review.targetPageId) && onOpenPage && <button className={button} onClick={() => onOpenPage((review.appliedPageId ?? review.targetPageId)!,review.projectId)}>Open applied page</button>}
  </section>;
  if(!loaded) return <section className="shared-agents space-y-4"><h2 className="text-lg font-semibold">Shared agents</h2>{error ? <><p role="alert">{error}</p><button className={button} disabled={busy} onClick={()=>void act(refresh)}>Retry loading shared agents</button></> : <p role="status">Loading shared agents…</p>}</section>;
  return <div className="shared-agents space-y-5" aria-label="Shared agents">
    <header className="flex flex-wrap justify-between gap-2"><div className="flex items-center gap-2"><h2 className="text-lg font-semibold">Shared agents</h2><ContextInfo title="shared agent access">Only selected project pages are shared. Personal pages and provider keys are never exposed to external clients. Suggestions require independent human review. Native runs send the selected pages to your chosen model. Web access is disabled.</ContextInfo></div><button className={button} disabled={busy} onClick={() => void act(refresh)}>Refresh agents</button></header>
    {error && <p role="alert" className="text-sthira-danger">{error}</p>}{notice && <p role="status">{notice}</p>}
    <section className={section}><p>Workspace agent access: {policy ? 'Enabled' : 'Disabled'}</p>{canManage && <button className={button} disabled={busy} onClick={() => void act(async () => {await call('setPolicy',{enabled:!policy});})}>{policy ? 'Disable workspace agents' : 'Enable workspace agents'}</button>}</section>
    {policy && <section className={section}><h3 className="font-semibold">{editing ? 'Update connection scope' : 'Connect a shared agent'}</h3><form className="space-y-3" onSubmit={e => {e.preventDefault();void act(async () => {
      if(editing) await call('update',{connectionId:editing.connectionId,grant:{expectedGeneration:editing.generation,permission,pageIds:selectedPages}});
      else {const result=await call<{config:SharedConfig[]}>('create',{projectId:project,clientName,label:label.trim()||clientName,permission,pageIds:selectedPages});setConfig(clientName==='Sthira native' ? null : result.config);setFormat(clientName==='Codex' ? 2 : clientName==='OpenCode' ? 1 : 0);}
      setEditing(null);setSelectedPages([]);setPermission('read');setNotice('Connection saved. Only the selected pages are shared.');
    });}}>
      <div className="grid gap-3 sm:grid-cols-2"><label>Shared project<select aria-label="Shared project" className="block w-full" disabled={busy || !!editing} value={project} onChange={e => {setProject(e.target.value);setSelectedPages([]);}}><option value="">Choose a project</option>{projects.map(p => <option key={p.projectId} value={p.projectId}>{p.name}</option>)}</select></label><label>Agent client<select aria-label="Agent client" className="block w-full" value={clientName} disabled={busy || !!editing} onChange={e => setClientName(e.target.value)}>{['Sthira native','Codex','Antigravity','Claude Code','Cline','Kilo Code','OpenCode','Custom MCP client'].map(v => <option key={v}>{v}</option>)}</select></label></div>
      <label className="block">Connection label<input aria-label="Connection label" className="block w-full" value={label} disabled={busy || !!editing} maxLength={96} onChange={e => setLabel(e.target.value)} /></label>
      <fieldset disabled={busy}><legend>Pages to share</legend>{pages.length === 0 && <p>Choose a project with attached pages.</p>}<div className="max-h-52 overflow-auto space-y-2">{pages.map(p => <label key={p.pageId} className="flex gap-2 items-center"><input type="checkbox" checked={selectedPages.includes(p.pageId)} onChange={e => setSelectedPages(v => e.target.checked ? [...v,p.pageId] : v.filter(id => id!==p.pageId))} />{p.title}</label>)}</div></fieldset>
      <label className="block">Permission<select aria-label="Permission" className="block" value={permission} disabled={busy} onChange={e => setPermission(e.target.value as 'read'|'suggest')}><option value="read">Can read</option><option value="suggest" disabled={!canSuggest}>Can read and suggest changes</option></select></label>
      <div className="flex gap-2"><button className="sthira-button-primary" disabled={busy || !project || !selectedPages.length || selectedPages.length>100}>{editing ? 'Save shared scope' : 'Create shared connection'}</button>{editing && <button type="button" className={button} onClick={() => {setEditing(null);setSelectedPages([]);setPermission('read');}}>Cancel scope change</button>}</div>
    </form></section>}
    {snippet && <section className={section} aria-label="Shared MCP setup"><div className="flex justify-between"><h3 className="font-semibold">MCP configuration</h3><button className={button} onClick={() => setConfig(null)}>Close configuration</button></div><label>Client format<select value={format} onChange={e => setFormat(Number(e.target.value))}>{config!.map((s,i) => <option value={i} key={s.label}>{s.label}</option>)}</select></label><p>{snippet.instructions}</p><textarea aria-label="Shared MCP configuration" readOnly value={snippet.text ?? JSON.stringify(snippet.json,null,2)} rows={7} className="w-full font-mono text-xs" /><button className={button} onClick={() => void navigator.clipboard.writeText(snippet.text ?? JSON.stringify(snippet.json,null,2)).then(() => setNotice('Configuration copied.')).catch(() => setError('Copy failed. Select and copy the configuration text.'))}>Copy shared configuration</button></section>}
    <section className={section}><h3 className="font-semibold">Connections</h3>{connections.length===0 ? <p>No shared connections yet.</p> : <ul className="space-y-3">{connections.map(c => <li key={c.connectionId} className="border-b border-sthira-border pb-3"><p className="font-medium">{c.label} · {c.projectName}</p><p>{c.memberDisplayName} · {c.deviceLabel} · {c.permission === 'read' ? 'Can read' : 'Can read and suggest'} · {c.pages.length} selected pages · {c.revokedAtMs ? 'Disconnected' : policy ? 'Active' : 'Paused by workspace policy'}</p><p className="text-sthira-text-muted">{c.canRunOnDevice ? "Available on this device" : c.localStatus === "anotherAccount" ? "Another member’s connection" : c.localStatus === "configurationMismatch" ? "Connection configuration needs repair" : "Set up on the issuing device"}</p><details><summary>Shared pages</summary><ul>{c.pages.map(p => <li key={p.pageId}>{p.title} · rev {p.currentRevisionNumber}{!p.available && ' · unavailable'}</li>)}</ul></details>{!c.revokedAtMs && <div className="flex flex-wrap gap-2 mt-2"><button className={button} disabled={busy || !c.canRunOnDevice} onClick={() => void act(async () => {setConfig(await call<SharedConfig[]>('config',{connectionId:c.connectionId}));setFormat(0);})}>Setup {c.label}</button><button className={button} disabled={busy || !c.initiatedByCurrentUser} onClick={() => {setEditing(c);setProject(c.projectId);setLabel(c.label);setClientName(c.clientName);setPermission(c.permission);setSelectedPages(c.pages.filter(p=>p.available).map(p=>p.pageId));}}>Change scope {c.label}</button><button className={button} disabled={busy} onClick={() => void act(async () => {await call('disconnect',{connectionId:c.connectionId});setConfig(null);setNotice(`Disconnected ${c.label}. Pending suggestions can no longer be applied.`);})}>Disconnect {c.label}</button></div>}</li>)}</ul>}</section>
    {policy && <section className={section}><h3 className="font-semibold">Run Sthira on shared pages</h3><form className="space-y-3" onSubmit={e => {e.preventDefault();void act(async () => {const run=await call<SharedNativeRun>('run',{connectionId:runConnection,kind,pageId:target||null,goal,modelId});setOpenRun(run.id);setNotice('Run queued. Published pages remain unchanged until review.');});}}>
      <div className="grid gap-3 sm:grid-cols-2"><label>Run connection<select aria-label="Run connection" className="block w-full" value={runConnection} onChange={e=>{setRunConnection(e.target.value);setTarget('');setKind('answer');}}><option value="">Choose a connection</option>{active.filter(c=>c.canRunOnDevice).map(c=><option key={c.connectionId} value={c.connectionId}>{c.label} · {c.projectName}</option>)}</select></label><label>Shared run model<select aria-label="Shared run model" className="block w-full" value={modelId} onChange={e=>setModelId(e.target.value)}><option value="">Choose a model</option>{models.map(m=><option key={m.id} value={m.id}>{m.display_name} · {m.provider_name}</option>)}</select></label></div>
      <label>Operation<select aria-label="Operation" className="block" value={kind} onChange={e=>setKind(e.target.value as typeof kind)}><option value="answer">Answer from selected pages</option><option value="edit" disabled={selectedConnection?.permission!=='suggest'}>Suggest an edit</option><option value="newPage" disabled={selectedConnection?.permission!=='suggest'}>Suggest a new page</option></select></label>
      {kind==='edit' && <label>Page to edit<select aria-label="Page to edit" className="block w-full" value={target} onChange={e=>setTarget(e.target.value)}><option value="">Choose a selected page</option>{selectedConnection?.pages.filter(p=>p.available).map(p=><option key={p.pageId} value={p.pageId}>{p.title}</option>)}</select></label>}
      <label className="block">Shared agent goal<textarea aria-label="Shared agent goal" className="block w-full" value={goal} maxLength={8000} onChange={e=>setGoal(e.target.value)} rows={3} /></label><p className="text-sthira-text-muted">Only this connection’s selected pages go to the chosen model. Web access is off.</p><button className="sthira-button-primary" disabled={busy || !selectedConnection || !runConnection || (kind!=="answer" && selectedConnection.permission!=="suggest") || !modelId || !goal.trim() || (kind==='edit' && !target)}>Run shared agent</button>
    </form></section>}
    <section className={section}><h3 className="font-semibold">Shared agent runs</h3>{!runs.length ? <p>No runs for this account on this device.</p> : <div className="overflow-x-auto"><table className="w-full text-left"><thead><tr><th>Goal</th><th>Status</th><th>Model</th><th>Details</th></tr></thead><tbody>{runs.map(r=><tr key={r.id}><td className="max-w-64 truncate">{r.goal}</td><td>{r.stage}</td><td>{r.modelLabel || r.modelId}</td><td><button className={button} onClick={()=>setOpenRun(r.id)}>Open run {r.id.slice(0,8)}</button></td></tr>)}</tbody></table></div>}
      {detailRun && <div className="space-y-2 border-t border-sthira-border pt-3"><h4 className="font-semibold">{detailRun.goal}</h4><p role="status">{detailRun.stage}</p>{detailRun.error && <p role="alert">{detailRun.error}</p>}{detailRun.answer && <RichDocumentRenderer content={detailRun.answer} />}<p>Model: {detailRun.modelLabel || detailRun.modelId} · Provider: {detailRun.providerLabel || 'Not dispatched'}</p><ul>{detailRun.sources.map(s=><li key={s.pageId}>{s.title} · rev {s.revisionNumber}</li>)}</ul>{detailRun.status==='running' && <button className={button} disabled={busy} onClick={()=>void act(async()=>{const r=await call<{outcome:string}>('cancel',{runId:detailRun.id});setNotice(r.outcome==='alreadyCommitting' ? 'Submission has started. Check the proposal receipt.' : r.outcome==='stopped' || r.outcome==='completed' ? 'This run has already stopped. Refresh its details.' : 'Cancellation requested.');})}>Cancel shared run</button>}{['failed','interrupted'].includes(detailRun.status) && <button className={button} disabled={busy} onClick={()=>void act(async()=>{await call('retry',{runId:detailRun.id});})}>Recover shared run</button>}{detailRun.proposalId && <button className={button} disabled={busy} onClick={()=>void act(()=>openReview(detailRun.proposalId!))}>Review run proposal</button>}<button className={button} onClick={()=>setOpenRun(null)}>Close run details</button></div>}
    </section>
    <section className={section}><h3 className="font-semibold">Agent proposals</h3>{!proposals.length ? <p>No agent proposals.</p> : <ul className="space-y-2">{proposals.map(p=><li key={p.proposalId} className="flex flex-wrap justify-between gap-2"><span>{p.proposedTitle} · {p.authorDisplayName} · {p.agentLabel} · {p.status}{p.isStale && ' · stale'}</span><button className={button} disabled={busy} onClick={()=>void act(()=>openReview(p.proposalId))}>Review {p.proposedTitle}</button></li>)}</ul>}</section>
  </div>;
}

export function SharedAgentSettings() {
  const [workspaces,setWorkspaces]=useState<WorkspaceSummary[]>([]);
  const [selected,setSelected]=useState('');
  const [error,setError]=useState('');
  const [uid,setUid]=useState<string|null>(null);
  const accountEpoch=useRef(0);
  useEffect(()=>{const clear=()=>{accountEpoch.current+=1;setSelected('');setUid(null);setWorkspaces([]);setError('Account changed. Reopen shared setup to refresh.');};window.addEventListener('sthira:account-context-changed',clear);return()=>window.removeEventListener('sthira:account-context-changed',clear);},[]);
  useEffect(()=>{let active=true;const epoch=accountEpoch.current;void listWorkspaces().then(r=>{if(!active || epoch!==accountEpoch.current)return;if(r.outcome==='ok')setWorkspaces(r.data.workspaces);else setError('Sign in and verify your account to connect a shared workspace.');}).catch(()=>{if(active)setError('Could not load shared workspaces.');});return()=>{active=false;};},[]);
  const workspace=workspaces.find(w=>w.workspaceId===selected);
  // The member identity is loaded from the verified workspace, never supplied as authority.
  useEffect(()=>{let active=true;setUid(null);if(selected) void import('../api/workspaces').then(api=>api.getWorkspaceDetails(selected)).then(r=>{if(active && r.outcome==='ok')setUid(r.data.members.find(m=>m.isCurrentUser)?.uid??null);});return()=>{active=false;};},[selected]);
  return <section className="shared-agent-settings space-y-4"><label>Shared workspace<select aria-label="Shared workspace" className="block" value={selected} onChange={e=>setSelected(e.target.value)}><option value="">Choose a workspace</option>{workspaces.map(w=><option key={w.workspaceId} value={w.workspaceId}>{w.name}</option>)}</select></label>{error && <p role="status">{error}</p>}{workspace && uid && <SharedAgentsPanel workspaceId={workspace.workspaceId} myRole={workspace.myRole} currentUid={uid} onSessionChange={()=>{setSelected('');setUid(null);setWorkspaces([]);setError('Verify your account before reopening shared setup.');}} onAccessLost={()=>{setSelected('');setUid(null);setError('Workspace access changed. Reopen Settings to refresh.');}} />}</section>;
}
