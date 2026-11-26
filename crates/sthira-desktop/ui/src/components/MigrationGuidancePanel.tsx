import {useEffect,useRef,useState} from 'react';
import {getProviderCatalog} from '../api/ipc';
import {getEnabledModels,openWorkflowSettings} from '../api/workflowSettings';
import {useWorkflowSettings} from '../hooks/useWorkflowSettings';
import type {ProviderEntry} from '../types/sthira';
import {startMigrationGuidance,getMigrationGuidance,latestMigrationGuidance,cancelMigrationGuidance,type ImportRun,type ImportSelection,type GuidanceInput,type MigrationGuidance} from '../api/migration';
export function MigrationGuidancePanel({run,selection,onActive,onSelection,locked}:{locked:boolean;run:ImportRun;selection:ImportSelection|null;onActive:(active:boolean)=>void;onSelection:(paths:string[])=>void}) {
 const [providers,setProviders]=useState<ProviderEntry[]>([]),[model,setModel]=useState(''),[instructions,setInstructions]=useState(''),[excerpts,setExcerpts]=useState(false);
 const [job,setJob]=useState<MigrationGuidance|null>(null),[pending,setPending]=useState(false),[error,setError]=useState('');
 const workflows=useWorkflowSettings(),alive=useRef(true),request=useRef<GuidanceInput|null>(null),lookup=useRef(0);
 const models=getEnabledModels(providers),chosen=model||workflows.settings.agent?.model_id||models.find(m=>m.is_default)?.id||'';
 const working=!!job&&['queued','running'].includes(job.status),current=selection?.basis===job?.input.basis;
 const setup=async()=>{try{const value=await getProviderCatalog();if(alive.current)setProviders(value);}catch(e){if(alive.current)setError(String(e));}};
 useEffect(()=>{alive.current=true;let current=true;const ticket=++lookup.current;void setup();void latestMigrationGuidance(run.id).then(value=>{if(current&&ticket===lookup.current)setJob(value);}).catch(e=>{if(current&&ticket===lookup.current)setError(String(e));});return()=>{current=false;alive.current=false;};},[run.id]);
 useEffect(()=>{request.current=null;},[selection?.basis,chosen,instructions,excerpts]);
 useEffect(()=>{onActive(pending||working);return()=>onActive(false);},[pending,working,onActive]);
 useEffect(()=>{
  if(!job||!working)return;let current=true,inflight=false;const id=job.id;
  const timer=window.setInterval(async()=>{if(inflight)return;inflight=true;try{const value=await getMigrationGuidance(id);if(current)setJob(previous=>previous&&previous.id===value.id&&!['queued','running'].includes(previous.status)?previous:value);}catch(e){if(current)setError(String(e));}finally{inflight=false;}},1000);
  return()=>{current=false;window.clearInterval(timer);};
 },[job?.id,working]);
 const ask=async()=>{
  if(!selection||!selection.paths.length)return;lookup.current++;setPending(true);setError('');
  const input=request.current??{request_id:crypto.randomUUID(),import_id:run.id,revision:run.revision,paths:selection.paths,basis:selection.basis,model_id:chosen,instructions,include_excerpts:excerpts};request.current=input;
  try{const value=await startMigrationGuidance(input);if(alive.current){setJob(value);request.current=null;}}catch(e){if(alive.current)setError(String(e));}finally{if(alive.current)setPending(false);}
 };
 const stop=async()=>{if(!job)return;lookup.current++;setPending(true);setError('');try{const value=await cancelMigrationGuidance(job.id);if(alive.current)setJob(value);}catch(e){if(alive.current)setError(String(e));}finally{if(alive.current)setPending(false);}};
 return <section className="migration-guidance" aria-label="Migration-agent guidance">
  <h4>Plan this import with the agent</h4>
  {run.status==='preview'&&<>
   <p className="migration-helper">Optional guidance uses only selected file names, titles, warnings and your instructions. It can suggest page mappings and organization groups; it cannot import, create projects or read your workspace.</p>
   {!workflows.settings.agent?.enabled&&<p className="migration-helper">Enable Workspace agent for guidance. Manual import remains available. <button type="button" className="settings-btn-secondary" onClick={()=>openWorkflowSettings()}>Open workflow settings</button></p>}
   <label>Guidance model<select value={chosen} disabled={locked||pending||working} onChange={e=>setModel(e.target.value)}><option value="">Choose an enabled model</option>{models.map(m=><option key={m.id} value={m.id}>{m.display_name} · {providers.find(p=>p.id===m.provider_id)?.display_name||m.provider_id}</option>)}</select></label>
   <button type="button" className="settings-btn-secondary" disabled={locked||pending||working} onClick={()=>{void setup();workflows.retry();}}>Refresh guidance setup</button>
   <label>What should the agent consider?<textarea maxLength={2000} value={instructions} disabled={locked||pending||working} placeholder="Suggest related groups and flag unsupported mappings." onChange={e=>setInstructions(e.target.value)}/></label>
   <label className="migration-check"><input type="checkbox" checked={excerpts} disabled={locked||pending||working} onChange={e=>setExcerpts(e.target.checked)}/><span>Include up to 500 characters from each selected source</span></label>
   <p className="migration-helper">Context is sent to the chosen model provider. Source excerpts are off by default. Guidance supports up to 50 selected files. Suggestions never apply automatically.</p>
   <button type="button" className="settings-btn-secondary" disabled={locked||pending||working||!selection||!selection.paths.length||selection.paths.length>50||!chosen||workflows.loading||!workflows.settings.agent?.enabled} onClick={()=>void ask()}>Request migration guidance</button>
  </>}
  {(error||workflows.error)&&<p role="alert">{error||workflows.error}</p>}
  {job&&<section aria-label="Saved migration guidance"><h5>Agent guidance · {job.status}</h5><p className="migration-helper">{job.model_name} · {job.provider_name} · {job.input.paths.length} captured files. Suggestions remain AI-authored.</p>
   {working&&<><p role="status">Guidance continues while the app remains open.</p><button type="button" className="settings-btn-secondary" disabled={locked||pending} onClick={()=>void stop()}>Stop guidance</button></>}
   {job.error&&<p role="alert">{job.error}</p>}
   {run.status==='preview'&&!current&&<p className="migration-helper">The current selection differs from this guidance. Request fresh guidance before using its selection.</p>}
   {job.result&&<><p>{job.result.summary}</p><ul>{job.result.mappings.map(m=><li key={m.path}><strong>{m.path}</strong><p>{m.choice.replace(/_/g,' ')} · {m.reason}</p>{m.suggested_group&&<p className="migration-helper">Suggested group: {m.suggested_group} (not created)</p>}</li>)}</ul>{job.result.cautions.map((c,n)=><p className="migration-helper" key={n}>{c}</p>)}
    {run.status==='preview'&&<button type="button" className="settings-btn-secondary" disabled={locked||pending||!current||!job.result.mappings.length} onClick={()=>onSelection(selection!.paths.filter(path=>!job.result!.mappings.some(m=>m.path===path&&m.choice!=='import_as_page')))}>Use suggested file selection</button>}
   </>}
  </section>}
 </section>;
}
