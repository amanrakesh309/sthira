import { useCallback, useEffect, useRef, useState } from 'react';
import { workspaceBackups, type BackupSummary, type RestorePreview, type Restoration, type RecoveryHistory } from '../api/workspaceBackups';
import type { WorkspaceListResponse, WorkspaceResponse, WorkspaceRole } from '../api/workspaces';
import type { GoogleSession } from './AccountModal';
import { HelpTopicButton } from './HelpTopicButton';
import { ContextInfo } from './ContextInfo';
import { RichDocumentRenderer } from './RichDocumentRenderer';
const button = 'min-h-[40px] px-4 py-2 rounded-lg border border-sthira-border text-sm hover:bg-sthira-surface-muted disabled:opacity-50 disabled:cursor-not-allowed';
const card = 'rounded-xl border border-sthira-border bg-sthira-surface p-4 space-y-3';
interface Props {
  workspaceId: string; workspaceName: string; currentUid: string | null; myRole: WorkspaceRole;
  onSessionChange: (session: GoogleSession) => void;
  onAccessLost: (message: string, workspaces?: WorkspaceListResponse | null) => void;
}
function Counts({counts}: {counts: Record<string, number>}) {
  const labels: Record<string, string> = {pages:'Pages',projects:'Projects',page_revisions:'Revisions',project_pages:'Project links',comment_threads:'Page threads',comment_messages:'Page messages',project_threads:'Project threads',project_messages:'Project messages',historical_records:'History records',recoverable_drafts:'Saved drafts'};
  return <dl className="grid grid-cols-2 sm:grid-cols-3 gap-3 text-sm">{Object.entries(counts).filter(([,n])=>n>0).map(([key,n])=><div key={key}><dt className="text-sthira-text-muted">{labels[key] ?? key}</dt><dd className="font-medium tabular-nums">{n}</dd></div>)}</dl>;
}
export function WorkspaceRecoveryPanel(props:Props) {
  // A fresh component identity prevents recovery state crossing accounts or workspaces.
  return <Recovery key={`${props.workspaceId}:${props.currentUid}:${props.myRole}`} {...props} />;
}
function Recovery(props:Props) {
  const {workspaceId,workspaceName,currentUid,myRole}=props;
  const callbacks=useRef(props);callbacks.current=props;
  const alive=useRef(false);const acting=useRef(false);
  const [backups,setBackups]=useState<BackupSummary[]>([]);
  const [restoration,setRestoration]=useState<Restoration|null>(null);
  const [preview,setPreview]=useState<RestorePreview|null>(null);
  const [confirmed,setConfirmed]=useState(false);
  const [remove,setRemove]=useState<string|null>(null);
  const [busy,setBusy]=useState(false);const [loading,setLoading]=useState(true);
  const [error,setError]=useState('');const [notice,setNotice]=useState('');
  const createKey=useRef<string|null>(null);
  const canManage=myRole==='owner'||myRole==='admin';
  const unwrap=useCallback(<T,>(result:WorkspaceResponse<T>):T=>{
    if (!alive.current) throw new Error('Account or workspace changed.');
    if(result.outcome==='ok') return result.data;
    if(result.outcome==='sessionUpdated') callbacks.current.onSessionChange(result.session);
    if(result.outcome==='accessLost') callbacks.current.onAccessLost(result.message,result.workspaces ?? null);
    throw new Error(result.outcome==='error'||result.outcome==='accessLost'?result.message:'Verify your account and retry.');
  },[]);
  const call=useCallback(async<T,>(action:string,input:unknown={})=>{
    if(!currentUid)throw new Error('Verify your account before using workspace recovery.');
    return unwrap(await workspaceBackups<T>(workspaceId,currentUid,action,input));
  },[workspaceId,currentUid,unwrap]);
  const load=useCallback(async()=>{
    const [receipt,saved]=await Promise.all([call<Restoration|null>('restoration'),canManage?call<BackupSummary[]>('list'):Promise.resolve([])]);
    if(alive.current){setRestoration(receipt);setBackups(saved);}
  },[call,canManage]);
  useEffect(()=>{
    alive.current=true;setLoading(true);
    void load().catch(e=>{if(alive.current)setError(String(e.message ?? e));}).finally(()=>{if(alive.current)setLoading(false);});
    return()=>{alive.current=false;};
  },[load]);
  async function act(work:()=>Promise<void>){
    if(acting.current)return;acting.current=true;setBusy(true);setError('');setNotice('');
    try{await work();}catch(e){if(alive.current)setError(e instanceof Error?e.message:String(e));}
    finally{acting.current=false;if(alive.current)setBusy(false);}
  }
  const disabled=busy||loading||!currentUid;
  return <section aria-label="Workspace recovery" className="space-y-5 text-sthira-text">
    <header className="flex flex-wrap items-center justify-between gap-3"><div className="flex items-center gap-2"><h2 className="text-xl font-semibold">Backup & recovery</h2><HelpTopicButton article="teamRecovery" label="Recovery help"/><ContextInfo title="workspace recovery"><p>Saved backups share this service’s storage. Export a file and keep a separate copy for recovery if the service is lost. Files contain workspace content and are not encrypted.</p><p>Restore preserves the target’s current members and roles. Invitations, credentials, agent access, Personal data, presence, and unacknowledged edits are excluded. History is retained as source records, never as active approvals or live sessions. Inline links keep their original text.</p></ContextInfo></div><button className={button} disabled={disabled} onClick={()=>void act(load)}>Refresh recovery</button></header>
    {loading&&<p role="status">Loading workspace recovery…</p>}
    {error&&<p role="alert" className="break-words">{error}</p>}{notice&&<p role="status">{notice}</p>}
    {!canManage&&<p>Only Owners and Admins can create, export, or restore backups. Restored history is available to current workspace members.</p>}
    {canManage&&<>
      <section className={card}><div className="flex flex-wrap items-center justify-between gap-3"><h3 className="font-semibold">Saved backups</h3><button className={button} disabled={disabled} onClick={()=>void act(async()=>{
        createKey.current ??= crypto.randomUUID();await call<BackupSummary>('create',{idempotencyKey:createKey.current});
        createKey.current=null;await load();if(alive.current)setNotice('Backup saved. Export a separate file for independent recovery.');
      })}>Create backup</button></div>
      {!loading&&backups.length===0&&<p className="text-sm text-sthira-text-muted">No saved backups yet.</p>}
      <ul className="space-y-3">{backups.map(b=><li className="border-t border-sthira-border pt-3 space-y-3" key={b.backupId}>
        <p className="font-medium">{b.sourceName} · {new Date(b.createdAtMs).toLocaleString()}</p><p className="text-sm text-sthira-text-muted">Saved by {b.createdByDisplayName} · {(b.bytes/1024).toFixed(1)} KiB</p><Counts counts={b.counts}/>
        <div className="flex flex-wrap gap-2"><button className={button} disabled={disabled} onClick={()=>void act(async()=>{const r=await call<{cancelled?:boolean}>('export',{backupId:b.backupId});if(!r.cancelled&&alive.current)setNotice('Backup exported. Keep the file in a separate safe location.');})}>Export backup</button><button className={button} disabled={disabled} onClick={()=>setRemove(b.backupId)}>Remove backup</button></div>
        {remove===b.backupId&&<div className="space-y-2" role="group" aria-label="Confirm backup removal"><p>Remove this saved backup? Exported files and workspace content are kept.</p><div className="flex gap-2"><button className={button} disabled={disabled} onClick={()=>void act(async()=>{await call('remove',{backupId:b.backupId});setRemove(null);await load();setNotice('Saved backup removed.');})}>Confirm remove backup</button><button className={button} disabled={disabled} onClick={()=>setRemove(null)}>Keep backup</button></div></div>}
      </li>)}</ul></section>
      <section className={card}><h3 className="font-semibold">Restore from a file</h3><p className="text-sm">Create or select an empty workspace first. Previewing a file does not change published content.</p><p className="text-sm">Current target: <strong>{workspaceName}</strong></p>
        <button className={button} disabled={disabled} onClick={()=>void act(async()=>{setPreview(null);setConfirmed(false);const p=await call<RestorePreview & {cancelled?:boolean}>('previewFile');if(!p.cancelled&&alive.current){if(p.workspaceId!==workspaceId)throw new Error('Recovery target changed. Preview again.');setPreview(p);}})}>Choose backup file</button>
        {preview&&<section className="rounded-lg border border-sthira-border p-4 space-y-3" aria-label="Restore preview"><h4 className="font-semibold">Restore {preview.sourceName} into {preview.targetName}</h4><Counts counts={preview.counts}/><p className="text-sm">{preview.memberCount} current member(s) keep their roles. Source history and attribution are unverified records. Saved drafts require a fresh proposal and review.</p><p className="text-sm">Preview expires {new Date(preview.expiresAtMs).toLocaleTimeString()}.</p><details><summary className="cursor-pointer text-sm">Backup identity</summary><p className="text-xs break-all">SHA-256: {preview.checksumSha256}</p><p className="text-xs break-all">Target workspace: {preview.workspaceId}</p></details>
          <label className="flex items-start gap-2 text-sm"><input type="checkbox" checked={confirmed} disabled={disabled} onChange={e=>setConfirmed(e.target.checked)}/>I reviewed the target and will restore content into this empty workspace.</label>
          <div className="flex flex-wrap gap-2"><button className="sthira-button-primary" disabled={disabled||!confirmed} onClick={()=>void act(async()=>{await call('commit',{ticketId:preview.ticketId});setPreview(null);setConfirmed(false);await load();if(alive.current)setNotice('Workspace restored. Open Pages or Projects to inspect the recovered content.');})}>Restore workspace</button><button className={button} disabled={disabled} onClick={()=>{setPreview(null);setConfirmed(false);}}>Cancel restore</button></div>
        </section>}
      </section>
    </>}
    {restoration&&<section className={card} aria-label="Restored workspace history"><h3 className="font-semibold">Restored from {restoration.receipt.sourceName}</h3><p className="text-sm">Restored by {restoration.receipt.restoredByDisplayName} · {new Date(restoration.receipt.restoredAtMs).toLocaleString()}. Historical attribution and decisions are unverified source records.</p><Counts counts={restoration.receipt.counts}/>
      <button className={button} disabled={disabled} onClick={()=>void act(async()=>{const value=await call<Restoration>('history');if(alive.current)setRestoration(value);})}>Inspect recovery history</button>
      {restoration.history?.map((h,i)=><section key={i} className="space-y-3"><h4 className="font-semibold">{h.sourceName} · source history</h4>{h.records.filter(g=>g.rows.length>0).map(group=><RecoveryGroup key={group.table} group={group} disabled={disabled} copy={text=>void act(async()=>{await navigator.clipboard.writeText(text);if(alive.current)setNotice('Saved draft copied. Create a fresh proposal to review these edits.');})} />)}</section>)}
      {restoration.history?.length===0&&<p className="text-sm">No proposal or draft history was included.</p>}
    </section>}
  </section>;
}
// Keep large recovery archives inspectable without mounting every source record at once.
function RecoveryGroup({group,disabled,copy}:{group:RecoveryHistory['records'][number];disabled:boolean;copy:(text:string)=>void}) {
  const [open,setOpen]=useState(false);
  const [limit,setLimit]=useState(50);
  const labels:Record<string,string>={shared_live_drafts:'Acknowledged drafts',shared_change_proposals:'Change proposals',shared_proposal_decisions:'Review decisions',shared_live_draft_contributors:'Draft contributors',shared_agent_page_proposals:'Agent proposals',shared_workspace_activity:'Activity'};
  return <details onToggle={e=>setOpen(e.currentTarget.open)}>
    <summary className="cursor-pointer py-2 text-sm">{labels[group.table] ?? group.table} ({group.rows.length})</summary>
    {open&&<div className="space-y-3">{group.rows.slice(0,limit).map((row,j)=><article className="rounded-lg border border-sthira-border p-3 space-y-2" key={j}>
      <p className="font-medium break-words">{String(row.recovery_title ?? row.proposed_title ?? row.page_title ?? row.summary ?? row.action ?? row.display_name ?? 'Source record')}</p>
      {typeof row.recovery_content_md==='string'&&<><RichDocumentRenderer content={row.recovery_content_md}/><button className={button} disabled={disabled} onClick={()=>copy(String(row.recovery_content_md))}>Copy saved draft</button></>}
      {typeof row.proposed_content_md==='string'&&<RichDocumentRenderer content={row.proposed_content_md}/>}
      <details><summary className="text-sm cursor-pointer">Source record details</summary><dl className="text-xs space-y-2">{Object.entries(row).filter(([key])=>!['recovery_content_md','proposed_content_md'].includes(key)).map(([key,v])=><div key={key}><dt className="font-medium">{key.replace(/_/g,' ')}</dt><dd className="break-words whitespace-pre-wrap">{v===null?'Not recorded':String(v)}</dd></div>)}</dl></details>
    </article>)}{group.rows.length>limit&&<button className={button} disabled={disabled} onClick={()=>setLimit(n=>n+50)}>Show more source records ({limit} of {group.rows.length})</button>}</div>}
  </details>;
}
export function RestoredWorkspaceNotice({workspaceId,currentUid,activeTab,onInspect}:{workspaceId:string;currentUid:string|null;activeTab:string;onInspect:()=>void}) {
  const [receipt,setReceipt]=useState<Restoration|null>(null);
  useEffect(()=>{let active=true;setReceipt(null);if(currentUid)void workspaceBackups<Restoration|null>(workspaceId,currentUid,'restoration').then(r=>{if(active&&r.outcome==='ok')setReceipt(r.data);}).catch(()=>{});return()=>{active=false;};},[workspaceId,currentUid,activeTab]);
  return receipt?<div className="rounded-lg border border-sthira-border bg-sthira-surface-muted p-3 text-sm flex flex-wrap items-center justify-between gap-2"><p>Restored content · historical attribution and decisions are unverified source records.</p><button className={button} onClick={onInspect}>View recovery history</button></div>:null;
}
