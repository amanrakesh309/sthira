import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { AgentWorkflowReview } from '../components/AgentWorkflowReview';
import type { AgentRun, AgentStep } from '../api/workspaceAgent';
import type { MemoryEntry } from '../api/sharedMemory';
const run={model_name:'Chosen model',provider_name:'Connected provider'} as AgentRun;
const step=(action:AgentStep['action'],result:Record<string,unknown>={}):AgentStep=>({id:'step-a',sequence:1,action,result,state:'pending',artifact_id:'page-a',turn_id:null,change_set_id:null,basis_content_md:null});
const props={run,projectNames:{'project-a':'Reading journal'},current:'Saved original prose',stale:false,pending:false,error:null,onClose:vi.fn(),onDismiss:vi.fn(),onOpenArtifact:vi.fn()};
afterEach(cleanup);
it('requires an explicit title choice, submits only that title, and restores keyboard focus',async()=>{
  const user=userEvent.setup(),apply=vi.fn();
  const opener=document.createElement('button');document.body.append(opener);opener.focus();
  const view=render(<AgentWorkflowReview {...props} onApply={apply} step={step({type:'propose_titles',artifact_id:'page-a',titles:['Open research questions','Uncertain launch timing']},{basis:{title:'Saved title'}})}/>);
  expect(screen.getByRole('dialog')).toHaveTextContent('Chosen model · Connected provider');expect(screen.getByRole('button',{name:'Use title'})).toBeDisabled();
  await user.click(screen.getByRole('radio',{name:'Uncertain launch timing'}));
  expect(within(screen.getByRole('region',{name:'Selected title'})).getByText('Title: Uncertain launch timing')).toBeInTheDocument();
  await user.click(screen.getByRole('button',{name:'Use title'}));expect(apply).toHaveBeenCalledWith('Uncertain launch timing');
  view.unmount();expect(opener).toHaveFocus();opener.remove();
});
it('reviews Derive as a separate page with source, destination and lineage',async()=>{
  const apply=vi.fn(),user=userEvent.setup();render(<AgentWorkflowReview {...props} onApply={apply} step={step({type:'propose_derive',artifact_id:'page-a',title:'Research milestones',content_md:'# Milestones\n\nVerify evidence.',derivation_type:'plan_from_research'},{target_project_id:'project-a'})}/>);
  expect(screen.getByRole('region',{name:'Saved source document'})).toHaveTextContent('Saved original prose');expect(screen.getByRole('region',{name:'Proposed derived page'})).toHaveTextContent('Verify evidence.');
  expect(screen.getByRole('dialog')).toHaveTextContent('AI-authored page with source lineage');expect(screen.getByRole('dialog')).toHaveTextContent('Reading journal');expect(apply).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button',{name:'Create derived page'}));expect(apply).toHaveBeenCalledWith(undefined);
});
it('shows original memory author, agent editor and source evidence',()=>{
  const memory:MemoryEntry={id:'memory-a',scope:{kind:'project',id:'project-a'},title:'Open question',body:'Original uncertainty',version:2,pinned:true,archived:false,created_at:1,updated_at:2,author:{kind:'user'},sources:[{artifact_id:'source-a',title:'Evidence page',content_hash:'hash',status:'current'}]};
  const apply=vi.fn();render(<AgentWorkflowReview {...props} onApply={apply} memory={memory} step={step({type:'propose_memory_update',id:memory.id,expected_version:2,title:'Still unresolved',body:'Confirm before planning.'},{memory_basis:memory})}/>);
  expect(screen.getByRole('dialog')).toHaveTextContent('Original author: you.');expect(screen.getByRole('dialog')).toHaveTextContent('Revision editor: Chosen model · Connected provider');
  expect(screen.getByRole('region',{name:'Current shared memory'})).toHaveTextContent('Original uncertainty');expect(screen.getByRole('region',{name:'Proposed shared memory'})).toHaveTextContent('Confirm before planning.');
  expect(screen.getByRole('button',{name:'Evidence page'})).toBeInTheDocument();expect(screen.getByRole('dialog')).toHaveTextContent('Memory is context, not verified evidence.');expect(apply).not.toHaveBeenCalled();
});
it('blocks stale memory acceptance and retains failed review content',()=>{
  const apply=vi.fn();render(<AgentWorkflowReview {...props} onApply={apply} stale error="Memory changed in another session" step={step({type:'propose_memory_create',scope:'project',title:'Unresolved date',body:'Date is not confirmed.',source_ids:[]},{scope:{kind:'project',id:'project-a'}})}/>);
  expect(screen.getByRole('alert')).toHaveTextContent('Memory changed in another session');expect(screen.getByRole('button',{name:'Save memory'})).toBeDisabled();expect(screen.getByRole('region',{name:'Proposed shared memory'})).toHaveTextContent('Date is not confirmed.');expect(apply).not.toHaveBeenCalled();
});
it('describes Refine restore without conflating it with page creation',async()=>{
  const apply=vi.fn(),user=userEvent.setup();render(<AgentWorkflowReview {...props} onApply={apply} step={step({type:'propose_refine',artifact_id:'page-a',content_md:'Revised uncertainty.'})}/>);
  expect(screen.getByRole('dialog')).toHaveTextContent('Undo refuses to overwrite later work');expect(screen.getByRole('region',{name:'Current document'})).toHaveTextContent('Saved original prose');
  await user.click(screen.getByRole('button',{name:'Apply refinement'}));expect(apply).toHaveBeenCalledOnce();
});
