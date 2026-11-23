import type { ReactNode } from 'react';
import type { AgentRun,AgentStep } from '../api/workspaceAgent';
import { DocumentEditReview } from './DocumentEditReview';
interface Purpose {id:string;purpose:string;origin:string}
interface Project {id:string;name:string;purpose:Purpose|null}
interface Props {run:AgentRun;step:AgentStep;pending:boolean;error:string|null;onClose:()=>void;onApply:()=>void;onDismiss:()=>void;onOpenProject?:(id:string)=>void;onOpenArtifact:(id:string)=>void}
function Direction({purpose}:{purpose:Purpose|null}) {return <><p className="agent-project-purpose">{purpose?.purpose||'No project purpose recorded.'}</p><p className="agent-helper">{purpose?.origin==='user_recorded'?'Originally recorded by you':purpose?'AI-inferred direction':'No direction attribution'}</p></>;}
export function AgentProjectReview({run,step,pending,error,onClose,onApply,onDismiss,onOpenProject,onOpenArtifact}:Props) {
  const action=step.action;
  if(action.type!=='propose_project_create'&&action.type!=='propose_project_purpose'&&action.type!=='propose_move')return null;
  let heading:string,applyLabel:string,footnote:string,current:ReactNode,proposed:ReactNode;
  let relatedProjects:Project[]=[];
  let source:{id:string;title:string|null}|undefined;
  if(action.type==='propose_project_create') {
    heading='Create a project';applyLabel='Create project';current=<p>No project has been created by this proposal.</p>;
    proposed=<><p>Project: <strong>{action.name}</strong></p><p className="agent-project-purpose">{action.purpose.trim()||'No initial purpose.'}</p>{action.purpose.trim()&&<p className="agent-helper">AI-inferred direction</p>}</>;
    footnote='Creates an empty project after review. Continue with a follow-up to add pages. Undo is available only while the project remains unchanged and unused.';
  } else if(action.type==='propose_project_purpose') {
    const basis=step.result.project_basis as Project;relatedProjects=[basis];
    heading='Update project direction';applyLabel='Apply project direction';
    current=<><p>Project: <strong>{basis.name}</strong></p><Direction purpose={basis.purpose}/></>;
    proposed=<><p>Project: <strong>{basis.name}</strong></p><p className="agent-project-purpose">{action.purpose.trim()}</p><p className="agent-helper">AI-inferred direction</p></>;
    footnote='Saves a new purpose history entry with AI provenance. Previous direction remains in history. Guarded undo restores the prior direction without relabeling its authorship.';
  } else {
    const from=step.result.source_project as Project|null,to=step.result.destination_project as Project;
    source=(step.result.pages as {id:string;title:string|null}[])[0];relatedProjects=from?[from,to]:[to];
    heading='Move a saved page';applyLabel='Move page';
    current=<><p>Page: <strong>{source.title||'Untitled page'}</strong></p><p>From: <strong>{from?.name||'Standalone'}</strong></p></>;
    proposed=<><p>Page: <strong>{source.title||'Untitled page'}</strong></p><p>To: <strong>{to.name}</strong></p><p>{from?'Leaves the selected source project.':'Gains its first project membership.'}</p></>;
    footnote='Changes the source and destination memberships together. Page identity, content, authorship, other memberships, references and lineage are preserved. Undo rejects newer membership changes.';
  }
  return <DocumentEditReview heading={heading} currentLabel="Captured state" proposedLabel="Proposed state" current={current} proposed={proposed} summary={action.reason} applyLabel={applyLabel} footnote={footnote} stale={false} pending={pending} busy={false} error={error} onClose={onClose} onApply={onApply} onDismiss={onDismiss}>
    <p className="agent-helper">Prepared by {run.model_name} · {run.provider_name}. Every change requires your acceptance.</p>
    {action.type==='propose_project_create'&&Number(step.result.same_name_projects)>0&&<p className="agent-helper">A saved project already uses this name. Accepting creates a separate project.</p>}
    <div className="agent-proposal-actions">{relatedProjects.map((p,index)=><button key={p.id} className="agent-link" type="button" disabled={pending||!onOpenProject} onClick={()=>{onClose();onOpenProject?.(p.id);}}>Open {action.type==='propose_move'?(index===relatedProjects.length-1?'destination':'source'):'saved'} project: {p.name}</button>)}{source&&<button className="agent-link" type="button" disabled={pending} onClick={()=>{onClose();onOpenArtifact(source!.id);}}>Open saved page: {source.title||'Untitled page'}</button>}</div>
  </DocumentEditReview>;
}
