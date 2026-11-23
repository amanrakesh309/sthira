import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { AgentView } from '../views/AgentView';
import * as api from '../api/workspaceAgent';
import { defaultWorkflowSettings } from '../api/workflowSettings';
import type { AgentRun } from '../api/workspaceAgent';
vi.mock('../api/workspaceAgent',async original=>({...await original<typeof import('../api/workspaceAgent')>(),listAgentRuns:vi.fn(),getAgentRun:vi.fn(),createAgentRun:vi.fn(),executeAgentRun:vi.fn(),cancelAgentRun:vi.fn(),reviewAgentStep:vi.fn(),undoAgentEdit:vi.fn(),followUpAgentRun:vi.fn()}));
vi.mock('../api/ipc',()=>({isTauri:true,getWorkspaceTree:vi.fn(async()=>({projects:[{id:'project-a',name:'Reading journal'}]})),getProviderCatalog:vi.fn(async()=>[{id:'p',display_name:'Configured provider',is_enabled:true,models:[{id:'m',provider_id:'p',display_name:'Chosen model',is_enabled:true,is_default:true}]}]),getArtifactContext:vi.fn(async()=>({artifact:{content_md:'Original prose'}}))}));
const state=vi.hoisted(()=>({enabled:true}));
vi.mock('../hooks/useWorkflowSettings',()=>({useWorkflowSettings:()=>({settings:{...defaultWorkflowSettings(),agent:{enabled:state.enabled,model_id:'m',templates_enabled:false}},loading:false,error:null})}));
const saved:AgentRun={id:'run-a',goal:'Review my reading',project_id:'project-a',workspace_memory:false,model_id:'m',model_name:'Chosen model',provider_id:'p',provider_name:'Configured provider',status:'completed',summary:'No changes applied.',handoff:'Check the unresolved question.',error:null,created_at:1000,updated_at:1000,steps:[]};
beforeEach(()=>{vi.clearAllMocks();state.enabled=true;vi.mocked(api.listAgentRuns).mockResolvedValue([]);vi.mocked(api.createAgentRun).mockResolvedValue({...saved,status:'queued'});vi.mocked(api.executeAgentRun).mockResolvedValue(saved);vi.mocked(api.getAgentRun).mockResolvedValue(saved);});
afterEach(cleanup);
it('keeps the goal editable with honest setup when the agent workflow is disabled',async()=>{
  state.enabled=false;const user=userEvent.setup();render(<AgentView onOpenArtifact={vi.fn()}/>);
  await user.type(screen.getByRole('textbox',{name:'Agent goal'}),'Keep this unsent goal');
  expect(await screen.findByText(/Enable Workspace agent in Workflows/)).toBeInTheDocument();expect(screen.getByRole('button',{name:'Start working'})).toBeDisabled();expect(screen.getByRole('textbox',{name:'Agent goal'})).toHaveValue('Keep this unsent goal');expect(api.createAgentRun).not.toHaveBeenCalled();
});
it('dispatches the chosen scope and model, then shows the saved result and handoff',async()=>{
  const user=userEvent.setup();render(<AgentView onOpenArtifact={vi.fn()}/>);
  await screen.findByRole('option',{name:'Reading journal'});await user.type(screen.getByRole('textbox',{name:'Agent goal'}),'Review my reading');await user.selectOptions(screen.getByRole('combobox',{name:'Agent scope'}),'project-a');await user.click(screen.getByRole('button',{name:'Start working'}));
  await waitFor(()=>expect(api.createAgentRun).toHaveBeenCalledWith({goal:'Review my reading',project_id:'project-a',workspace_memory:false,model_id:'m'}));
  expect(await screen.findByRole('region',{name:'Agent result'})).toHaveTextContent('No changes applied.');expect(screen.getByRole('region',{name:'Saved handoff'})).toHaveTextContent('Check the unresolved question.');
});
it('retains a failed start and guards the unsent goal against navigation',async()=>{
  vi.mocked(api.createAgentRun).mockRejectedValue(new Error('Selected model unavailable'));
  const user=userEvent.setup();let guard:(()=>Promise<boolean>)|null=null;render(<AgentView onOpenArtifact={vi.fn()} registerBeforeLeave={value=>{guard=value;}}/>);
  await screen.findByRole('option',{name:/Chosen model/});await user.type(screen.getByRole('textbox',{name:'Agent goal'}),'A recoverable goal');await user.click(screen.getByRole('button',{name:'Start working'}));
  expect(await screen.findByRole('alert')).toHaveTextContent('Selected model unavailable');expect(screen.getByRole('textbox',{name:'Agent goal'})).toHaveValue('A recoverable goal');expect(await guard!()).toBe(false);
});
it('stops a running request and keeps its persisted activity available',async()=>{
  let resolve!:(run:AgentRun)=>void;vi.mocked(api.executeAgentRun).mockImplementation(()=>new Promise(done=>{resolve=done;}));vi.mocked(api.cancelAgentRun).mockResolvedValue({...saved,status:'cancelled',summary:null,error:'Stopped by you.'});
  const user=userEvent.setup();render(<AgentView onOpenArtifact={vi.fn()}/>);await screen.findByRole('option',{name:/Chosen model/});await user.type(screen.getByRole('textbox',{name:'Agent goal'}),'Stop this work');await user.click(screen.getByRole('button',{name:'Start working'}));
  await user.click(await screen.findByRole('button',{name:'Stop run'}));expect(api.cancelAgentRun).toHaveBeenCalledWith('run-a');expect(await screen.findByRole('alert')).toHaveTextContent('Stopped by you.');resolve(saved);await waitFor(()=>expect(screen.queryByRole('region',{name:'Agent result'})).not.toBeInTheDocument());expect(screen.getByRole('alert')).toHaveTextContent('Stopped by you.');
});
it('opens a saved proposal for rich review before applying and exposes undo',async()=>{
  const proposal={id:'step-a',sequence:1,action:{type:'propose_edit' as const,artifact_id:'page-a',content_md:'# Proposed\n\nNew prose'},state:'pending',result:{},artifact_id:'page-a',turn_id:'turn-a',change_set_id:null,basis_content_md:'Original prose'};
  const run={...saved,status:'review',steps:[proposal]};vi.mocked(api.listAgentRuns).mockResolvedValue([run]);vi.mocked(api.getAgentRun).mockResolvedValue(run);vi.mocked(api.reviewAgentStep).mockResolvedValue({...saved,steps:[{...proposal,state:'applied',change_set_id:'change-a'}]});
  const user=userEvent.setup();render(<AgentView onOpenArtifact={vi.fn()}/>);await user.click(await screen.findByRole('button',{name:/Review my reading/}));await user.click(screen.getByRole('button',{name:'Review proposal'}));
  const dialog=await screen.findByRole('dialog',{name:'Review changes'});expect(dialog).toHaveTextContent('Original prose');expect(dialog).toHaveTextContent('New prose');expect(api.reviewAgentStep).not.toHaveBeenCalled();await user.click(screen.getByRole('button',{name:'Apply edit'}));await waitFor(()=>expect(api.reviewAgentStep).toHaveBeenCalledWith('run-a','step-a',true));expect(await screen.findByRole('button',{name:'Undo applied edit'})).toBeInTheDocument();
});

it('opens saved internal citations by identity and reviews new page creation separately from edits',async()=>{
  const proposal={id:'create-a',sequence:1,action:{type:'propose_create' as const,title:'Open questions',content_md:'# Open questions\n\nCheck the evidence.'},state:'pending',result:{},artifact_id:null,turn_id:null,change_set_id:null,basis_content_md:null};
  const run={...saved,status:'review',summary:'Read [the source](sthira://artifact/source-a).',steps:[proposal]};vi.mocked(api.listAgentRuns).mockResolvedValue([run]);vi.mocked(api.getAgentRun).mockResolvedValue(run);vi.mocked(api.reviewAgentStep).mockResolvedValue({...run,status:'completed',steps:[{...proposal,state:'applied',artifact_id:'created-a'}]});
  const open=vi.fn(),user=userEvent.setup();render(<AgentView onOpenArtifact={open}/>);await user.click(await screen.findByRole('button',{name:/Review my reading/}));await user.click(screen.getByRole('button',{name:'the source'}));expect(open).toHaveBeenCalledWith('source-a','project-a');await user.click(screen.getByRole('button',{name:'Review proposal'}));expect(await screen.findByRole('dialog',{name:'Review changes'})).toHaveTextContent('Suggested new page');await user.click(screen.getByRole('button',{name:'Create page'}));await waitFor(()=>expect(api.reviewAgentStep).toHaveBeenCalledWith('run-a','create-a',true));
});
it('allows leaving a working task and opening another goal without cancelling it',async()=>{
  const pending={...saved,status:'running',summary:null};vi.mocked(api.createAgentRun).mockResolvedValue({...pending,status:'queued'});vi.mocked(api.executeAgentRun).mockResolvedValue(pending);
  let guard:(()=>Promise<boolean>)|null=null;const user=userEvent.setup();render(<AgentView onOpenArtifact={vi.fn()} registerBeforeLeave={value=>{guard=value;}}/>);await screen.findByRole('option',{name:/Chosen model/});await user.type(screen.getByRole('textbox',{name:'Agent goal'}),'Work while I write');await user.click(screen.getByRole('button',{name:'Start working'}));await screen.findByText(/You can leave this view/);expect(await guard!()).toBe(true);await user.click(screen.getByRole('button',{name:'New goal'}));expect(screen.getByRole('textbox',{name:'Agent goal'})).toBeInTheDocument();expect(api.cancelAgentRun).not.toHaveBeenCalled();
});
it('sends a follow-up using the selected parent and preserves its draft on failure',async()=>{
  vi.mocked(api.listAgentRuns).mockResolvedValue([saved]);vi.mocked(api.getAgentRun).mockResolvedValue(saved);vi.mocked(api.followUpAgentRun).mockRejectedValueOnce(new Error('Model unavailable'));
  const user=userEvent.setup();render(<AgentView onOpenArtifact={vi.fn()}/>);await user.click(await screen.findByRole('button',{name:/Review my reading/}));await user.type(screen.getByRole('textbox',{name:'Follow-up message'}),'Use the second approach');await user.click(screen.getByRole('button',{name:'Send follow-up'}));expect(api.followUpAgentRun).toHaveBeenCalledWith('run-a','Use the second approach','m');expect(await screen.findByRole('alert')).toHaveTextContent('Model unavailable');expect(screen.getByRole('textbox',{name:'Follow-up message'})).toHaveValue('Use the second approach');
});
it('opens a task by identity without dispatching inference again',async()=>{
  vi.mocked(api.getAgentRun).mockResolvedValue(saved);render(<AgentView initialRunId="run-a" onOpenArtifact={vi.fn()}/>);expect(await screen.findByRole('region',{name:'Agent result'})).toHaveTextContent('No changes applied.');expect(api.getAgentRun).toHaveBeenCalledWith('run-a');expect(api.executeAgentRun).not.toHaveBeenCalled();
});
