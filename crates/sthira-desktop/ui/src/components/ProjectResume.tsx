import { useEffect, useRef, useState } from 'react';
import { RefreshCw, Sparkles } from 'lucide-react';
import { getResumeBrief } from '../api/ipc';
import { defaultResumeSettings, generateResumeBrief, getResumeSettings, RESUME_SETTINGS_CHANGED } from '../api/resume';
import { openWorkflowSettings } from '../api/workflowSettings';
import { ResumeModelControl } from './ResumeModelControl';
import { ContextualResumePane } from './ContextualResumePane';
import type { ResumeBrief } from '../types/sthira';

export function ProjectResume({ projectId, onSelectArtifact }: { projectId:string; onSelectArtifact:(id:string)=>void }) {
  const [brief,setBrief]=useState<ResumeBrief|null>(null);
  const [settings,setSettings]=useState(defaultResumeSettings);
  const [loading,setLoading]=useState(true);
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  const [expanded,setExpanded]=useState(false);
  const [open,setOpen]=useState(true);
  const request=useRef(0);
  const load=async()=>{
    const current=++request.current;setLoading(true);setBusy(false);setError('');
    const [value,config]=await Promise.allSettled([getResumeBrief(projectId,1),getResumeSettings()]);
    if(current!==request.current)return;
    if(value.status==='fulfilled'){setBrief(value.value);setExpanded(false);}else setError('Could not read Resume. Your saved work is kept.');
    if(config.status==='fulfilled')setSettings(config.value);
    setLoading(false);
  };
  useEffect(()=>{setBrief(null);setOpen(true);setExpanded(false);setBusy(false);void load();window.addEventListener(RESUME_SETTINGS_CHANGED,load);return()=>{request.current++;window.removeEventListener(RESUME_SETTINGS_CHANGED,load);};},[projectId]);
  const generate=async()=>{
    const current=++request.current;setBusy(true);setError('');
    try{const value=await generateResumeBrief(projectId);if(current===request.current){setBrief(value);setExpanded(false);setOpen(true);}}
    catch{if(current===request.current)setError('Could not refresh the summary. The available Resume is kept.');}
    finally{if(current===request.current)setBusy(false);}
  };
  const review=async()=>{
    const current=++request.current;setBusy(true);setError('');
    try{const value=await getResumeBrief(projectId,expanded?1:2);if(current===request.current){setBrief(value);setExpanded(!expanded);}}
    catch{if(current===request.current)setError('Could not load the expanded review. Try again.');}
    finally{if(current===request.current)setBusy(false);}
  };
  const generated=brief?.generation?.mode==='generated'||brief?.generation?.mode==='cached';
  const modelLabel=(brief?.generation?.model_name||'Model').trim().split(/\s+/).slice(0,4).join(' ');
  return <div className="project-resume" aria-label="Project Resume" aria-busy={loading||busy}>
    <ResumeModelControl settings={settings}/>
    <div className="resume-toolbar"><span className="resume-helper" title={brief?.generation?.model_name||undefined}>{generated?`${brief?.generation?.mode==='cached'?'Saved excerpts':'Selected excerpts'} · ${modelLabel}${brief?.generation?.provider_name ? ` · ${brief.generation.provider_name}` : ''}`:'Saved facts'}</span><div>
      <button type="button" className="settings-btn-secondary" disabled={busy||loading} onClick={load}><RefreshCw size={14}/>Refresh facts</button>
      {settings.enabled&&settings.preferences.model_mode!=='factual'?<button type="button" className="settings-btn-secondary" disabled={busy||loading} onClick={generate}><Sparkles size={14}/>{busy?'Preparing…':generated?'Refresh summary':'Summarize'}</button>:<button type="button" className="settings-btn-secondary" onClick={openWorkflowSettings}>Set up model summaries</button>}
    </div></div>
    {loading&&!brief&&<p className="resume-helper" role="status">Reading saved working state…</p>}
    {error&&<p className="focus-error" role="alert">{error}<button type="button" onClick={load}>Retry</button></p>}
    {brief?.generation?.notice&&<p className="resume-helper" role="status">{brief.generation.notice}</p>}
    {brief&&<ContextualResumePane brief={brief} isOpen={open} onToggleOpen={()=>setOpen(value=>!value)} onExpandReview={review} isExpandedReview={expanded} isReviewLoading={busy} onSelectArtifact={onSelectArtifact} wordBudget={55}/>}
    {!!brief?.generation?.factual_progress?.length && <details className="resume-pack"><summary>Saved working state</summary><ul>{brief.generation.factual_progress.map((claim,index)=><li key={index}><p>{claim.text}</p>{claim.artifact_ids.map(id=><button key={id} type="button" className="focus-brief-source" onClick={()=>onSelectArtifact(id)}>Open source</button>)}</li>)}</ul></details>}
  </div>;
}
