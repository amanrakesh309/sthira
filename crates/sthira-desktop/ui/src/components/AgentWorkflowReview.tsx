import { useState } from 'react';
import type { AgentRun, AgentStep } from '../api/workspaceAgent';
import type { MemoryEntry, MemoryScope } from '../api/sharedMemory';
import { DocumentEditReview } from './DocumentEditReview';

interface Props {
  run:AgentRun; step:AgentStep; current:string; memory?:MemoryEntry; projectNames?:Record<string,string>;
  stale:boolean; pending:boolean; error:string|null;
  onClose:()=>void; onApply:(title?:string)=>void; onDismiss:()=>void;
  onOpenArtifact:(id:string)=>void;
}
const scopeName=(scope:MemoryScope|undefined,names:Record<string,string>)=>!scope?'Original scope':scope.kind==='workspace'?'Workspace':scope.kind==='session'?'This conversation':names[scope.id]||'Original project';
export function AgentWorkflowReview({run,step,current,memory,projectNames={},stale,pending,error,onClose,onApply,onDismiss,onOpenArtifact}:Props) {
  const [title,setTitle]=useState('');
  const action=step.action;
  const naming=action.type==='propose_titles';
  const deriving=action.type==='propose_derive';
  const memoryChange=action.type==='propose_memory_create'||action.type==='propose_memory_update';
  const basis=step.result.memory_basis as MemoryEntry|undefined;
  const scope=(step.result.scope as MemoryScope|undefined)||basis?.scope;
  const sourceEntries=memoryChange?(basis?.sources || (step.result.sources as {id:string;title:string|null}[]|undefined)?.map(s=>({artifact_id:s.id,title:s.title||'Source page'})) || []):[];
  let proposed='content_md' in action?action.content_md:'';
  let summary='Review this refinement. Applying saves a restore snapshot; Undo refuses to overwrite later work.';
  let heading='Refine document',applyLabel='Apply refinement',footnote='The saved page changes only when you apply this refinement.';
  let currentLabel='Current document',proposedLabel='Proposed refinement';
  if(naming) {
    heading='Title suggestions';applyLabel='Use title';
    current=`Title: ${(step.result.basis as {title?:string})?.title || 'Untitled'}`;
    proposed=title?`Title: ${title}`:'Choose a suggested title above.';
    summary='Choose one saved suggestion. A matching first-level heading is renamed with the title; other text stays intact.';
    footnote='Your title changes only after you choose Use title. You can rename it again in the editor.';
    currentLabel='Current title';proposedLabel='Selected title';
  } else if(deriving) {
    heading='Derive a new page';applyLabel='Create derived page';
    summary=`Create **${action.title}** as an AI-authored page with source lineage. Method: ${({plan_from_research:'Execution plan',extract_ideas:'Ideas',extract_tasks:'Task checklist'}[action.derivation_type]||'Derived page')}. Destination: ${step.result.target_project_id ? `project ${projectNames[String(step.result.target_project_id)]||'selected during the request'}`:'standalone page'}.`;
    currentLabel='Saved source document';proposedLabel='Proposed derived page';
    footnote='The source stays intact. The new page is created only when you accept; you can delete it through the normal page actions.';
  } else if(memoryChange) {
    heading=action.type==='propose_memory_create'?'New shared memory':'Shared memory revision';applyLabel=action.type==='propose_memory_create'?'Save memory':'Apply memory revision';
    proposed=`## ${action.title}\n\n${action.body}`;
    current=memory?`## ${memory.title}\n\n${memory.body}`:'No memory has been created.';
    const author=basis?.author.kind==='user'?'Original author: you.':basis?.author.kind==='agent'?`Original author: ${basis.author.model} · ${basis.author.provider}.`:'New memory is attributed to Sthira’s agent and this model.';
    summary=`Scope: ${scopeName(scope,projectNames)}. ${author} Revision editor: ${run.model_name} · ${run.provider_name}. Existing source evidence, scope and pinned state are preserved.`;
    currentLabel=memory?'Current shared memory':'Before saving';proposedLabel='Proposed shared memory';
    footnote='Memory is context, not verified evidence. Acceptance is saved with revision history; edit or archive it in Shared memory later.';
  }
  return <DocumentEditReview operation={deriving?'create':'edit'} heading={heading} current={current} proposed={proposed} summary={summary} currentLabel={currentLabel} proposedLabel={proposedLabel} applyLabel={applyLabel} footnote={footnote} stale={stale} pending={pending} busy={naming&&!title} error={error||(stale?'The saved source or memory changed. Ask for a fresh proposal before applying.':null)} onClose={onClose} onApply={()=>onApply(naming?title:undefined)} onDismiss={onDismiss}>
    <p className="agent-helper">Prepared by {run.model_name} · {run.provider_name}</p>
    {naming&&<fieldset className="agent-title-choices" disabled={pending}><legend>Choose a title</legend>{action.titles.map(value=><label key={value}><input type="radio" name={`agent-title-${step.id}`} checked={title===value} value={value} onChange={()=>setTitle(value)}/><span>{value}</span></label>)}</fieldset>}
    {memoryChange&&<div><p className="agent-helper">{sourceEntries.length?'Captured source pages':'No source pages attached; review this as an unsupported memory contribution.'}</p>{sourceEntries.map(source=><button type="button" className="agent-link" key={source.artifact_id} disabled={pending} onClick={()=>{onClose();onOpenArtifact(source.artifact_id);}}>{source.title}</button>)}</div>}
  </DocumentEditReview>;
}
