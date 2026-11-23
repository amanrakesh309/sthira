import { afterEach,beforeEach,expect,it,vi } from 'vitest';
import { cleanup,render,screen,within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { AgentProjectReview } from '../components/AgentProjectReview';
import type { AgentRun,AgentStep } from '../api/workspaceAgent';
const run={model_name:'Chosen model',provider_name:'Configured connection'} as AgentRun;
const project={id:'project-a',name:'Release planning',purpose:{id:'purpose-a',purpose:'Owner’s original direction.',origin:'user_recorded'}};
const step=(action:AgentStep['action'],result:Record<string,unknown>={}):AgentStep=>({id:'step-a',sequence:1,action,result,state:'pending',artifact_id:null,turn_id:null,change_set_id:null,basis_content_md:null});
const base={run,pending:false,error:null,onClose:vi.fn(),onDismiss:vi.fn(),onOpenProject:vi.fn(),onOpenArtifact:vi.fn()};
beforeEach(()=>vi.clearAllMocks());afterEach(cleanup);
it('reviews project creation with AI direction and warns about an existing same-name project',async()=>{
  const apply=vi.fn(),user=userEvent.setup();render(<AgentProjectReview {...base} onApply={apply} step={step({type:'propose_project_create',name:'Release planning',purpose:'Investigate timing before choosing a date.',reason:'Keep release planning together.'},{same_name_projects:1})}/>);
  expect(screen.getByRole('region',{name:'Captured state'})).toHaveTextContent('No project has been created');expect(screen.getByRole('region',{name:'Proposed state'})).toHaveTextContent('AI-inferred direction');expect(screen.getByRole('dialog')).toHaveTextContent('creates a separate project');expect(screen.getByRole('dialog')).toHaveTextContent('Continue with a follow-up to add pages');expect(apply).not.toHaveBeenCalled();await user.click(screen.getByRole('button',{name:'Create project'}));expect(apply).toHaveBeenCalledOnce();
});
it('keeps original user direction and proposed AI direction clearly attributed',()=>{
  render(<AgentProjectReview {...base} onApply={vi.fn()} step={step({type:'propose_project_purpose',project_id:'project-a',purpose:'Clarify uncertainty before scheduling.',reason:'Review a clearer direction.'},{project_basis:project})}/>);
  expect(screen.getByRole('region',{name:'Captured state'})).toHaveTextContent('Owner’s original direction');expect(screen.getByRole('region',{name:'Captured state'})).toHaveTextContent('Originally recorded by you');expect(screen.getByRole('region',{name:'Proposed state'})).toHaveTextContent('AI-inferred direction');expect(screen.getByRole('dialog')).toHaveTextContent('Chosen model · Configured connection');
});
it('moves an existing page without implying duplication or loss of other memberships',async()=>{
  const apply=vi.fn(),user=userEvent.setup();render(<AgentProjectReview {...base} onApply={apply} step={step({type:'propose_move',artifact_id:'page-a',from_project_id:'project-a',to_project_id:'project-b',reason:'Move the selected association.'},{pages:[{id:'page-a',title:'Original research'}],source_project:project,destination_project:{...project,id:'project-b',name:'Delivery planning'}})}/>);
  expect(screen.getByRole('region',{name:'Captured state'})).toHaveTextContent('From: Release planning');expect(screen.getByRole('region',{name:'Proposed state'})).toHaveTextContent('To: Delivery planning');expect(screen.getByRole('dialog')).toHaveTextContent('other memberships, references and lineage are preserved');await user.click(screen.getByRole('button',{name:'Move page'}));expect(apply).toHaveBeenCalledOnce();
});
it('distinguishes same-name source and destination projects by explicit identity navigation',async()=>{
  const user=userEvent.setup();render(<AgentProjectReview {...base} onApply={vi.fn()} step={step({type:'propose_move',artifact_id:'page-a',from_project_id:'project-a',to_project_id:'project-b',reason:'Move this saved page.'},{pages:[{id:'page-a',title:'Saved page'}],source_project:{...project,name:'Same name'},destination_project:{...project,id:'project-b',name:'Same name'}})}/>);
  await user.click(screen.getByRole('button',{name:'Open destination project: Same name'}));expect(base.onClose).toHaveBeenCalledOnce();expect(base.onOpenProject).toHaveBeenCalledWith('project-b');expect(base.onOpenProject).not.toHaveBeenCalledWith('project-a');
});
it('describes a standalone source explicitly and disables controls while applying',()=>{
  render(<AgentProjectReview {...base} pending onApply={vi.fn()} step={step({type:'propose_move',artifact_id:'page-a',from_project_id:null,to_project_id:'project-a',reason:'Organize the standalone page.'},{pages:[{id:'page-a',title:'Saved page'}],source_project:null,destination_project:project})}/>);
  expect(screen.getByRole('region',{name:'Captured state'})).toHaveTextContent('Standalone');expect(screen.getByRole('region',{name:'Proposed state'})).toHaveTextContent('first project membership');expect(screen.getByRole('button',{name:'Applying…'})).toBeDisabled();expect(screen.getByRole('button',{name:'Open destination project: Release planning'})).toBeDisabled();
});
it('retains a failed review and renders saved names literally rather than as links',()=>{
  render(<AgentProjectReview {...base} error="The project changed. Ask for a fresh proposal." onApply={vi.fn()} step={step({type:'propose_project_purpose',project_id:'project-a',purpose:'Unconfirmed direction.',reason:'Review before applying.'},{project_basis:{...project,name:'[Project](https://example.invalid)'}})}/>);
  const dialog=screen.getByRole('dialog');expect(within(dialog).getByRole('alert')).toHaveTextContent('project changed');expect(screen.getByRole('region',{name:'Captured state'})).toHaveTextContent('[Project](https://example.invalid)');expect(screen.getByRole('region',{name:'Captured state'}).querySelector('a')).toBeNull();expect(screen.getByRole('region',{name:'Proposed state'})).toHaveTextContent('Unconfirmed direction.');
});
