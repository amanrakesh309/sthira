import { useEffect, useState } from 'react';
import { getProviderCatalog } from '../api/ipc';
import { getResumeModel, saveResumeSettings, type ResumeModelView, type ResumeSettings } from '../api/resume';
import type { ProviderEntry } from '../types/sthira';
import { openWorkflowSettings } from '../api/workflowSettings';
export function ResumeModelControl({settings}:{settings:ResumeSettings}) {
  const [catalog,setCatalog]=useState<ProviderEntry[]>([]);
  const [resolved,setResolved]=useState<ResumeModelView|null>(null);
  const [pending,setPending]=useState(false);
  const [error,setError]=useState('');
  const [revision,setRevision]=useState(0);
  useEffect(()=>{const changed=()=>setRevision(x=>x+1);window.addEventListener('sthira:catalog-changed',changed);return()=>window.removeEventListener('sthira:catalog-changed',changed);},[]);
  useEffect(()=>{let alive=true;setError('');Promise.all([getProviderCatalog(),getResumeModel()]).then(([providers,model])=>{if(alive){setCatalog(providers);setResolved(model);}}).catch(()=>{if(alive)setError('Could not read the selected model.');});return()=>{alive=false;};},[settings,revision]);
  const value=settings.preferences.model_mode==='workspace'&&settings.preferences.model_id?`model:${settings.preferences.model_id}`:settings.preferences.model_mode;
  return <div className="resume-model-choice">
    <label>Resume model<select aria-label="Resume model" disabled={pending||!settings.enabled} value={value} onChange={async event=>{
      const next=event.target.value;setPending(true);setError('');
      try{await saveResumeSettings({...settings,preferences:{...settings.preferences,model_mode:next.startsWith('model:')?'workspace':next as 'workspace'|'local'|'factual',model_id:next.startsWith('model:')?next.slice(6):null}});}catch{setError('Could not change the Resume model. Your previous choice is kept.');}finally{setPending(false);}
    }}>
      <option value="workspace">Use workspace default</option><option value="local">Local Resume pack</option><option value="factual">Factual only</option>
      {settings.preferences.model_id&&!catalog.some(p=>p.models.some(m=>m.id===settings.preferences.model_id))&&<option value={value}>Saved model unavailable</option>}
      {catalog.filter(p=>p.is_enabled).map(p=><optgroup key={p.id} label={p.display_name}>{p.models.filter(m=>m.is_enabled).map(m=><option key={m.id} value={`model:${m.id}`}>{m.display_name}</option>)}</optgroup>)}
    </select></label>
    <span className="resume-helper" role="status">{!settings.enabled?'Model summaries are off.':resolved?.available?`${resolved.name} · ${resolved.provider_name}`:resolved?.notice||'Reading model choice…'}</span>
    <button type="button" className="focus-brief-source" onClick={openWorkflowSettings}>Model settings</button>
    {error&&<p role="alert" className="focus-error">{error}</p>}
  </div>;
}
