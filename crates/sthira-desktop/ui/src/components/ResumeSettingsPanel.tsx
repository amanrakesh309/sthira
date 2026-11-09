import { useEffect, useRef, useState } from 'react';
import { Download, RefreshCw, ShieldCheck } from 'lucide-react';
import { defaultResumeSettings, getResumeSettings, saveResumeSettings, getResumePackStatus, installResumePack, removeResumePack, type ResumeSettings, type ResumePackStatus } from '../api/resume';
import type { ProviderEntry } from '../types/sthira';

export function ResumeSettingsPanel({ providers }: { providers: ProviderEntry[] }) {
  const [settings,setSettings] = useState(defaultResumeSettings);
  const [pack,setPack] = useState<ResumePackStatus | null>(null);
  const [loading,setLoading] = useState(true);
  const [pending,setPending] = useState(false);
  const [downloading,setDownloading] = useState(false);
  const [error,setError] = useState('');
  const [notice,setNotice] = useState('');
  const alive = useRef(true);
  const load = async () => {
    setLoading(true); setError('');
    try { const [value,status] = await Promise.all([getResumeSettings(),getResumePackStatus()]); if (alive.current) {setSettings(value);setPack(status);} }
    catch { if(alive.current)setError('Could not load Resume settings.'); }
    finally { if(alive.current)setLoading(false); }
  };
  useEffect(() => { alive.current=true; void load(); return()=>{alive.current=false;}; },[]);
  useEffect(() => { if(!pack?.downloading)return; const timer=setInterval(()=>{getResumePackStatus().then(status=>{if(alive.current)setPack(status);}).catch(()=>{});},2000);return()=>clearInterval(timer); },[pack?.downloading]);
  const update = async (value: ResumeSettings) => {
    setPending(true);setError('');setNotice('');
    try {await saveResumeSettings(value);if(alive.current){setSettings(value);setNotice('Saved');}}
    catch {if(alive.current)setError('Could not save Resume settings. Your previous choices are kept.');}
    finally{if(alive.current)setPending(false);}
  };
  const preference = <K extends keyof ResumeSettings['preferences']>(key: K,value: ResumeSettings['preferences'][K]) => update({...settings,preferences:{...settings.preferences,[key]:value}});
  const download = async () => {
    setDownloading(true);setError('');
    try {const value=await installResumePack();if(alive.current){setPack(value);setNotice('Local Resume pack ready.');}}
    catch(reason){if(alive.current)setError(String(reason));}
    finally{if(alive.current)setDownloading(false);}
  };
  const models=providers.filter(provider=>provider.is_enabled).flatMap(provider=>provider.models.filter(model=>model.is_enabled));
  const selected=settings.preferences.model_mode==='workspace' && settings.preferences.model_id ? `model:${settings.preferences.model_id}` : settings.preferences.model_mode;
  return <article className="workflow-card resume-settings" aria-labelledby="resume-settings-title">
    <div className="workflow-card-heading"><div><h4 id="resume-settings-title">Resume</h4><p>Pick up saved work with factual context and optional model-selected excerpts.</p></div><span className="workflow-tag">On demand</span></div>
    <div className="workflow-capture"><ShieldCheck size={18}/><span>Factual Resume is always available. No model or download is required.</span></div>
    {loading ? <p role="status">Loading Resume settings…</p> : <fieldset disabled={pending || !!error && !pack}>
      <label className="resume-option"><input type="checkbox" checked={settings.enabled} onChange={event=>update({...settings,enabled:event.target.checked})}/><span>Enable model summaries<small>Run only when you choose Summarize or Refresh summary in Resume. No background generation.</small></span></label>
      <div className="resume-preferences">
        {settings.enabled && <>
        <label>Resume model<select value={selected} onChange={event=>{const value=event.target.value;void update({...settings,preferences:{...settings.preferences,model_mode:value.startsWith('model:')?'workspace':value as ResumeSettings['preferences']['model_mode'],model_id:value.startsWith('model:')?value.slice(6):null}});}}>
          <option value="workspace">Use workspace default{models.find(model=>model.is_default)?` · ${models.find(model=>model.is_default)!.display_name}`:' · not configured'}</option>
          <option value="local">Use local Resume pack</option><option value="factual">Factual Resume only</option>
          {settings.preferences.model_id && !models.some(model=>model.id===settings.preferences.model_id) && <option value={`model:${settings.preferences.model_id}`}>Saved model unavailable</option>}
          {models.map(model=><option key={model.id} value={`model:${model.id}`}>{model.display_name}</option>)}
        </select></label>
        {settings.preferences.model_mode==='workspace' && !settings.preferences.model_id && <label className="resume-option"><input type="checkbox" checked={settings.preferences.local_fallback} onChange={event=>preference('local_fallback',event.target.checked)}/><span>Use the downloaded local pack if no workspace default is configured</span></label>}
        <p className="resume-helper">An unavailable chosen model keeps factual Resume available. A cloud workspace model receives the selected saved excerpts when you request a summary.</p>
        </>}
        <fieldset><legend>Include in Resume</legend>{([['include_decisions','Recorded decisions'],['include_tasks','Unfinished checklist items'],['include_questions','Open questions'],['include_recent_changes','Saved document references'],['include_excerpts','Document excerpts for model summaries']] as const).map(([key,label])=><label key={key} className="resume-option"><input type="checkbox" checked={settings.preferences[key]} onChange={event=>preference(key,event.target.checked)}/><span>{label}</span></label>)}</fieldset>
        <label>Maximum documents to read<select value={settings.preferences.max_sources} onChange={event=>preference('max_sources',Number(event.target.value))}>{[1,2,4,8,12].map(value=><option key={value} value={value}>{value} documents</option>)}</select></label>
        <p className="resume-helper">Input excerpts are bounded. Decisions, tasks, and source links stay factual; the model does not edit notes or resolve questions.</p>
      </div>
    </fieldset>}
    {pack && <details className="resume-pack"><summary>Local Resume pack · {pack.ready?'Downloaded':downloading||pack.downloading?'Downloading…':'Optional download'}</summary><div>
      <strong>{pack.label}</strong><p>About {Math.ceil(pack.download_bytes/1_000_000)} MB including the CPU runtime. No Ollama or LM Studio installation needed. Works offline after download and unloads after each request.</p>
      <p className="resume-helper">This is an optional preview, selected after a small comparison with Qwen3-0.6B. It primarily supports English and can omit useful details; factual working state remains available.</p>
      {pack.notice && <p>{pack.notice}</p>}
      {pack.ready ? <button type="button" className="settings-btn-secondary" disabled={downloading||pack.downloading} onClick={async()=>{if(!confirm('Remove the local Resume pack? Your documents and provider connections will be kept.'))return;try{setPack(await removeResumePack());}catch{setError('Could not remove the local Resume pack.');}}}>Remove local pack</button> : <button type="button" className="settings-btn-secondary" disabled={!pack.supported||downloading||pack.downloading} onClick={download}>{downloading||pack.downloading?<RefreshCw size={15} className="animate-spin"/>:<Download size={15}/>} {downloading||pack.downloading?'Downloading…':'Download local Resume pack'}</button>}
    </div></details>}
    {error && <p className="focus-error" role="alert">{error} <button type="button" disabled={pending||downloading} onClick={load}>Retry</button></p>}
    {notice && <p className="resume-helper" role="status">{notice}</p>}
  </article>;
}
