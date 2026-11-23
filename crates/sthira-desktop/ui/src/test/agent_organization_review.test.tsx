import { afterEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { AgentOrganizationReview } from '../components/AgentOrganizationReview';
import type { AgentRun,AgentStep } from '../api/workspaceAgent';
const run={model_name:'Chosen model',provider_name:'Connected provider'} as AgentRun;
const result={pages:[{id:'source-a',title:'Source page'},{id:'target-a',title:'Target page'}],project:{id:'project-a',name:'Reading journal'}};
const step=(action:AgentStep['action']):AgentStep=>({id:'step-a',sequence:1,action,result,state:'pending',artifact_id:'source-a',turn_id:null,change_set_id:null,basis_content_md:null});
const base={run,pending:false,error:null,onClose:vi.fn(),onDismiss:vi.fn(),onOpenArtifact:vi.fn()};
afterEach(cleanup);
it('describes adding one project membership without duplicating the saved page',async()=>{
  const apply=vi.fn(),user=userEvent.setup();render(<AgentOrganizationReview {...base} onApply={apply} step={step({type:'propose_membership',artifact_id:'source-a',project_id:'project-a',attached:true,reason:'Keep this research in the project.'})}/>);
  expect(screen.getByRole('dialog')).toHaveTextContent('Reading journal');expect(screen.getByRole('region',{name:'Captured relationship'})).toHaveTextContent('Not a member');expect(screen.getByRole('region',{name:'Proposed relationship'})).toHaveTextContent('Becomes a member');expect(screen.getByRole('dialog')).toHaveTextContent('other memberships, references and lineage stay intact');expect(apply).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button',{name:'Add to project'}));expect(apply).toHaveBeenCalledOnce();
});
it('explains that removing project membership preserves the saved page',()=>{
  render(<AgentOrganizationReview {...base} onApply={vi.fn()} step={step({type:'propose_membership',artifact_id:'source-a',project_id:'project-a',attached:false,reason:'This page belongs outside the current project.'})}/>);
  expect(screen.getByRole('region',{name:'Proposed relationship'})).toHaveTextContent('The page remains saved');expect(screen.getByRole('button',{name:'Remove from project'})).toBeEnabled();
});
it('shows directional endpoints and keeps references separate from lineage and membership',async()=>{
  const apply=vi.fn(),user=userEvent.setup();render(<AgentOrganizationReview {...base} onApply={apply} step={step({type:'propose_connection',source_id:'source-a',target_id:'target-a',connected:true,reason:'Connect the supporting page.'})}/>);
  expect(screen.getByRole('region',{name:'Proposed relationship'})).toHaveTextContent('From: Source page');expect(screen.getByRole('region',{name:'Proposed relationship'})).toHaveTextContent('To: Target page');expect(screen.getByRole('dialog')).toHaveTextContent('reverse references and derivation lineage stay intact');
  await user.click(screen.getByRole('button',{name:'Create reference'}));expect(apply).toHaveBeenCalledOnce();
});
it('keeps a failed or stale review open and preserves both captured endpoints',()=>{
  render(<AgentOrganizationReview {...base} error="This relationship changed. Ask for a fresh proposal." onApply={vi.fn()} step={step({type:'propose_connection',source_id:'source-a',target_id:'target-a',connected:false,reason:'Remove only this directional reference.'})}/>);
  expect(screen.getByRole('alert')).toHaveTextContent('relationship changed');expect(screen.getByRole('region',{name:'Captured relationship'})).toHaveTextContent('Source page');expect(screen.getByRole('button',{name:'Remove reference'})).toBeInTheDocument();
});
it('disables mutation during acceptance and closes review before opening a source identity',async()=>{
  const open=vi.fn(),close=vi.fn(),user=userEvent.setup();const proposal=step({type:'propose_connection',source_id:'source-a',target_id:'target-a',connected:true,reason:'Read the supporting source.'});
  const view=render(<AgentOrganizationReview {...base} pending onApply={vi.fn()} onOpenArtifact={open} onClose={close} step={proposal}/>);
  expect(screen.getByRole('button',{name:'Applying…'})).toBeDisabled();expect(screen.getByRole('button',{name:'Open source page: Source page'})).toBeDisabled();
  view.rerender(<AgentOrganizationReview {...base} onApply={vi.fn()} onOpenArtifact={open} onClose={close} step={proposal}/>);
  await user.click(screen.getByRole('button',{name:'Open target page: Target page'}));expect(close).toHaveBeenCalledOnce();expect(open).toHaveBeenCalledWith('target-a');
});
it('renders source and project names literally without turning names into external links',()=>{
  const proposal=step({type:'propose_membership',artifact_id:'source-a',project_id:'project-a',attached:true,reason:'Review the named saved page.'});proposal.result={pages:[{id:'source-a',title:'Research [external](https://example.invalid) **draft**'}],project:{name:'Reading [journal](https://example.invalid)'}};
  render(<AgentOrganizationReview {...base} onApply={vi.fn()} step={proposal}/>);
  const captured=screen.getByRole('region',{name:'Captured relationship'});expect(captured).toHaveTextContent('Research [external](https://example.invalid) **draft**');expect(captured).toHaveTextContent('Reading [journal](https://example.invalid)');expect(captured.querySelector('a')).toBeNull();
});
