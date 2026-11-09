import { useEffect, useRef, useState } from 'react';
import { Sparkles, X } from 'lucide-react';
import { getProviderCatalog } from '../api/ipc';
import { getResumeModel, RESUME_SETTINGS_CHANGED, type ResumeModelView } from '../api/resume';
import { suggestDocumentTitles, type TitleSuggestions } from '../api/naming';
import { openWorkflowSettings } from '../api/workflowSettings';
import type { ProviderEntry } from '../types/sthira';
export interface TitleBasis { content:string; title:string; contentHash?:string }
interface Props { artifactId:string; current:TitleBasis; beforeRequest:()=>Promise<boolean>; snapshot:()=>TitleBasis; onApply:(title:string,basis:TitleBasis)=>Promise<boolean>; onClose:()=>void }
export function TitleSuggestionsPanel({artifactId,current,beforeRequest,snapshot,onApply,onClose}:Props) {
  const [catalog,setCatalog]=useState<ProviderEntry[]>([]);
  const [model,setModel]=useState<ResumeModelView|null>(null);
  const [choice,setChoice]=useState('resume');
  const [result,setResult]=useState<TitleSuggestions|null>(null);
  const [basis,setBasis]=useState<TitleBasis|null>(null);
  const [pending,setPending]=useState(false);
  const [error,setError]=useState('');
  const [notice,setNotice]=useState('');
  const serial=useRef(0);
  const [revision,setRevision]=useState(0);
  useEffect(()=>{const changed=()=>{serial.current++;setPending(false);setResult(null);setRevision(x=>x+1);};window.addEventListener(RESUME_SETTINGS_CHANGED,changed);window.addEventListener('sthira:catalog-changed',changed);return()=>{window.removeEventListener(RESUME_SETTINGS_CHANGED,changed);window.removeEventListener('sthira:catalog-changed',changed);};},[]);
  useEffect(()=>{let alive=true;Promise.all([getProviderCatalog(),getResumeModel(true)]).then(([providers,resolved])=>{if(alive){setCatalog(providers);setModel(resolved);}}).catch(()=>{if(alive)setError('Could not read available models.');});return()=>{alive=false;serial.current++;};},[artifactId,revision]);
  const chosenProvider=catalog.find(provider=>provider.models.some(model=>`model:${model.id}`===choice));
  const chosenModel=chosenProvider?.models.find(model=>`model:${model.id}`===choice);
  const resolvedLabel=choice==='resume' ? model?.name ? `${model.name} · ${model.provider_name}` : null : choice==='local' ? 'SmolLM2-360M · Sthira local preview pack' : chosenModel ? `${chosenModel.display_name} · ${chosenProvider?.display_name}` : null;
  const stale=!!basis&&(basis.title!==current.title||basis.content!==current.content);
  const generate=async()=>{
    const request=++serial.current;setPending(true);setError('');setNotice('');setResult(null);
    try {
      if(!(await beforeRequest()))return;
      const source=snapshot();
      const value=await suggestDocumentTitles(artifactId,{model_id:choice.startsWith('model:')?choice.slice(6):null,local:choice==='local'});
      if(request===serial.current){setResult(value);setBasis({...source,contentHash:value.basis_content_hash});}
    }catch(reason){if(request===serial.current)setError(String(reason));}
    finally{if(request===serial.current)setPending(false);}
  };
  return <section className="title-suggestions" aria-label="Title suggestions" aria-busy={pending}>
    <header><div><Sparkles size={16}/><strong>Suggest a title</strong></div><button type="button" className="focus-icon-button" aria-label="Close title suggestions" onClick={onClose}><X size={16}/></button></header>
    <p>Your title stays unchanged until you choose a suggestion. The selected model reads up to 5,000 characters of saved content.</p>
    <div className="title-suggestion-controls"><label>Model<select aria-label="Title suggestion model" value={choice} disabled={pending} onChange={e=>{serial.current++;setChoice(e.target.value);setResult(null);setError('');}}>
      <option value="resume">Use Resume model{model?.name?` · ${model.name} · ${model.provider_name}`:''}</option><option value="local">Local Resume pack</option>
      {catalog.filter(p=>p.is_enabled).map(p=><optgroup key={p.id} label={p.display_name}>{p.models.filter(m=>m.is_enabled).map(m=><option key={m.id} value={`model:${m.id}`}>{m.display_name}</option>)}</optgroup>)}
    </select></label><button type="button" className="settings-btn-secondary" disabled={pending} onClick={generate}>{pending?'Suggesting…':'Suggest titles'}</button></div>
    {resolvedLabel&&<p className="resume-helper">Using {resolvedLabel}</p>}
    {choice==='resume'&&model&&!model.available&&<p role="status">{model.notice} <button type="button" className="focus-brief-source" onClick={openWorkflowSettings}>Model settings</button></p>}
    {error&&<p className="focus-error" role="alert">{error}</p>}
    {result&&<><p className="resume-helper">Suggested by {result.model_name} · {result.provider_name}. Review the wording before applying.</p>{stale&&<p role="status">This page changed. Request new suggestions to use the latest text.</p>}<ul>{result.titles.map(title=><li key={title}><span>{title}</span><button type="button" className="settings-btn-secondary" disabled={pending||stale} onClick={async()=>{if(!basis)return;setPending(true);try{if(await onApply(title,basis)){setResult(null);setNotice('Title applied. You can edit it above.');}}finally{setPending(false);}}}>Use title<span className="sr-only">: {title}</span></button></li>)}</ul></>}
    {notice&&<p role="status">{notice}</p>}
  </section>;
}
