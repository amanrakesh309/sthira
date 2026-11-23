import type { ReactNode } from 'react';
import type { AgentRun, AgentStep } from '../api/workspaceAgent';
import { DocumentEditReview } from './DocumentEditReview';
interface Props {
  run:AgentRun;step:AgentStep;pending:boolean;error:string|null;
  onClose:()=>void;onApply:()=>void;onDismiss:()=>void;onOpenArtifact:(id:string)=>void;
}
interface PageBasis {id:string;title:string|null}
/** Review captured relationships, not a misleading comparison of page bodies. */
export function AgentOrganizationReview({run,step,pending,error,onClose,onApply,onDismiss,onOpenArtifact}:Props) {
  const action=step.action;
  if(action.type!=='propose_membership'&&action.type!=='propose_connection')return null;
  const pages=(step.result.pages as PageBasis[]|undefined)||[];
  const page=(id:string)=>pages.find(p=>p.id===id)?.title||'Untitled page';
  let heading:string,applyLabel:string,footnote:string;let current:ReactNode,proposed:ReactNode;
  if(action.type==='propose_membership') {
    const project=(step.result.project as {name?:string}|undefined)?.name||'Original project';
    heading=action.attached?'Add page to project':'Remove page from project';applyLabel=action.attached?'Add to project':'Remove from project';
    const endpoints=<><p>Page: <strong>{page(action.artifact_id)}</strong></p><p>Project: <strong>{project}</strong></p></>;
    current=<>{endpoints}<p>{action.attached?'Not a member of this project.':'Currently a member of this project.'}</p></>;
    proposed=<>{endpoints}<p>{action.attached?'Becomes a member of this project.':'Leaves this project. The page remains saved.'}</p></>;
    footnote='This changes one project membership. Page content, identity, other memberships, references and lineage stay intact. Guarded undo is available after acceptance.';
  } else {
    heading=action.connected?'Connect two pages':'Remove page reference';applyLabel=action.connected?'Create reference':'Remove reference';
    const endpoints=<><p>From: <strong>{page(action.source_id)}</strong></p><p>To: <strong>{page(action.target_id)}</strong></p></>;
    current=<>{endpoints}<p>{action.connected?'No explicit reference in this direction.':'An explicit reference exists in this direction.'}</p></>;
    proposed=<>{endpoints}<p>{action.connected?'Creates a directional page reference.':'Removes this directional page reference.'}</p></>;
    footnote='This changes one explicit reference. Page content, project membership, reverse references and derivation lineage stay intact. Guarded undo is available after acceptance.';
  }
  return <DocumentEditReview heading={heading} current={current} proposed={proposed} summary={action.reason} currentLabel="Captured relationship" proposedLabel="Proposed relationship" applyLabel={applyLabel} footnote={footnote} stale={false} pending={pending} busy={false} error={error} onClose={onClose} onApply={onApply} onDismiss={onDismiss}>
    <p className="agent-helper">Suggested by {run.model_name} · {run.provider_name}. Acceptance checks the current relationship, saved pages and original scope.</p>
    <div className="agent-proposal-actions">{pages.map((p,index)=><button type="button" className="agent-link" key={p.id} disabled={pending} onClick={()=>{onClose();onOpenArtifact(p.id);}}>Open {action.type==='propose_connection'?(index===0?'source':'target'):'saved'} page: {p.title||'Untitled page'}</button>)}</div>
  </DocumentEditReview>;
}
