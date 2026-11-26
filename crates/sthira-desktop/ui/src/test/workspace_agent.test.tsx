import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { AgentView } from '../views/AgentView';
import * as api from '../api/workspaceAgent';
import { defaultWorkflowSettings } from '../api/workflowSettings';
import { getWorkspaceTree } from '../api/ipc';
import { memoryAction } from '../api/sharedMemory';
import type { AgentRun } from '../api/workspaceAgent';
vi.mock('../api/workspaceAgent',async original=>({...await original<typeof import('../api/workspaceAgent')>(),listAgentRuns:vi.fn(),getAgentRun:vi.fn(),createAgentRun:vi.fn(),executeAgentRun:vi.fn(),cancelAgentRun:vi.fn(),reviewAgentStep:vi.fn(),undoAgentEdit:vi.fn(),followUpAgentRun:vi.fn()}));
vi.mock('../api/ipc',()=>({isTauri:true,getWorkspaceTree:vi.fn(async()=>({projects:[{id:'project-a',name:'Reading journal'}]})),getProviderCatalog:vi.fn(async()=>[{id:'p',display_name:'Configured provider',is_enabled:true,models:[{id:'m',provider_id:'p',display_name:'Chosen model',is_enabled:true,is_default:true}]}]),getArtifactContext:vi.fn(async()=>({artifact:{content_md:'Original prose',title:null,version:1,content_hash:'hash'}})),getSearchProviders:vi.fn(async()=>[])}));
vi.mock('../api/agentHandoffs',()=>({listAgentHandoffs:vi.fn(async()=>[]),listHandoffConnections:vi.fn(async()=>[]),getAgentHandoff:vi.fn(),saveAgentHandoff:vi.fn(),closeAgentHandoff:vi.fn(),continueAgentHandoff:vi.fn()}));
vi.mock('../api/sharedMemory',()=>({memoryAction:vi.fn()}));
const state=vi.hoisted(()=>({enabled:true,taskRevision:0}));
vi.mock('../context/AgentTasksContext',()=>({useAgentTasks:()=>({tasks:[],loading:false,error:null,refresh:vi.fn(),revision:state.taskRevision})}));
vi.mock('../hooks/useWorkflowSettings',()=>({useWorkflowSettings:()=>({settings:{...defaultWorkflowSettings(),agent:{enabled:state.enabled,model_id:'m',templates_enabled:false}},loading:false,error:null})}));
const saved:AgentRun={id:'run-a',goal:'Review my reading',project_id:'project-a',workspace_memory:false,model_id:'m',model_name:'Chosen model',provider_id:'p',provider_name:'Configured provider',status:'completed',summary:'No changes applied.',handoff:'Check the unresolved question.',error:null,created_at:1000,updated_at:1000,steps:[]};
beforeEach(()=>{vi.clearAllMocks();state.enabled=true;state.taskRevision=0;vi.mocked(api.listAgentRuns).mockResolvedValue([]);vi.mocked(api.createAgentRun).mockResolvedValue({...saved,status:'queued'});vi.mocked(api.executeAgentRun).mockResolvedValue(saved);vi.mocked(api.getAgentRun).mockResolvedValue(saved);});
afterEach(cleanup);
it('paginates saved goals and opens the activity details with the mouse and keyboard', async () => {
  const run: AgentRun = { ...saved, steps: [{
    id: 'read-a', sequence: 1, action: { type: 'read_page', artifact_id: 'source-a' }, state: 'done',
    result: { title: 'Experiment log', content_md: '**Saved excerpt** from the experiment.' }, artifact_id: 'source-a',
    turn_id: null, change_set_id: null, basis_content_md: null,
  }] };
  const recent = Array.from({ length: 8 }, (_, index) => ({ ...saved, id: `run-${index}`, goal: `Recent goal ${index}`, updated_at: 2000 - index }));
  vi.mocked(api.listAgentRuns).mockResolvedValue([...recent, run]);
  vi.mocked(api.getAgentRun).mockResolvedValue(run);
  const user = userEvent.setup();
  render(<AgentView onOpenArtifact={vi.fn()} />);

  const goals = await screen.findByRole('table', { name: 'Recent goals' });
  expect(within(goals).getAllByRole('row')).toHaveLength(9);
  await user.click(screen.getByRole('button', { name: 'Next' }));
  await user.click(within(goals).getByRole('cell', { name: 'Finished' }));
  expect(api.getAgentRun).toHaveBeenCalledWith(run.id);
  const activity = await screen.findByRole('table', { name: 'Recorded actions' });
  const detail = within(activity).getByRole('button', { name: 'Read saved page' });
  expect(detail).toHaveAttribute('aria-expanded', 'false');
  expect(screen.queryByText('Saved excerpt')).not.toBeInTheDocument();
  detail.focus();
  await user.keyboard('{Enter}');
  expect(detail).toHaveAttribute('aria-expanded', 'true');
  expect(screen.getByText('Saved excerpt')).toBeVisible();
  await user.click(within(activity).getByRole('cell', { name: 'Completed' }));
  expect(detail).toHaveFocus();
  expect(detail).toHaveAttribute('aria-expanded', 'false');
  expect(screen.queryByText('Saved excerpt')).not.toBeInTheDocument();
});
it('keeps the goal editable with honest setup when the agent workflow is disabled',async()=>{
  state.enabled=false;const user=userEvent.setup();render(<AgentView onOpenArtifact={vi.fn()}/>);
  await user.type(screen.getByRole('textbox',{name:'Agent goal'}),'Keep this unsent goal');
  expect(await screen.findByText(/Enable Workspace agent in Workflows/)).toBeInTheDocument();expect(screen.getByRole('button',{name:'Start working'})).toBeDisabled();expect(screen.getByRole('textbox',{name:'Agent goal'})).toHaveValue('Keep this unsent goal');expect(api.createAgentRun).not.toHaveBeenCalled();
});
it('dispatches the chosen scope and model, then shows the saved result and handoff',async()=>{
  const user=userEvent.setup();render(<AgentView onOpenArtifact={vi.fn()}/>);
  await screen.findByRole('option',{name:'Reading journal'});await user.type(screen.getByRole('textbox',{name:'Agent goal'}),'Review my reading');await user.selectOptions(screen.getByRole('combobox',{name:'Agent scope'}),'project-a');await user.click(screen.getByRole('button',{name:'Start working'}));
  await waitFor(()=>expect(api.createAgentRun).toHaveBeenCalledWith({goal:'Review my reading',project_id:'project-a',workspace_memory:false,model_id:'m',allow_web:false,research_template_mode:'inherit',research_template_id:null,plan_template_mode:'inherit',plan_template_id:null}));
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

it('submits the exact reviewed title choice through the native review boundary',async()=>{
  const proposal={id:'title-a',sequence:1,action:{type:'propose_titles' as const,artifact_id:'page-a',titles:['Open research questions','Uncertain launch timing']},state:'pending',result:{basis:{version:1,title:null,content_hash:'hash'}},artifact_id:'page-a',turn_id:null,change_set_id:null,basis_content_md:null};
  const run={...saved,status:'review',steps:[proposal]};vi.mocked(api.listAgentRuns).mockResolvedValue([run]);vi.mocked(api.getAgentRun).mockResolvedValue(run);vi.mocked(api.reviewAgentStep).mockResolvedValue({...run,status:'completed',steps:[{...proposal,state:'applied'}]});
  const user=userEvent.setup();render(<AgentView initialRunId="run-a" onOpenArtifact={vi.fn()}/>);await user.click(await screen.findByRole('button',{name:'Review proposal'}));
  expect(screen.getByRole('button',{name:'Use title'})).toBeDisabled();await user.click(screen.getByRole('radio',{name:'Uncertain launch timing'}));await user.click(screen.getByRole('button',{name:'Use title'}));
  await waitFor(()=>expect(api.reviewAgentStep).toHaveBeenCalledWith('run-a','title-a',true,'Uncertain launch timing'));expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
});
it('reloads current memory before review and blocks acceptance of a changed version',async()=>{
  const memory={id:'memory-a',scope:{kind:'project' as const,id:'project-a'},title:'Original question',body:'Original uncertainty',version:3,pinned:false,archived:false,author:{kind:'user' as const},sources:[],created_at:1,updated_at:2};
  vi.mocked(memoryAction).mockResolvedValue({kind:'entry',value:memory});
  const proposal={id:'memory-step',sequence:1,action:{type:'propose_memory_update' as const,id:'memory-a',expected_version:2,title:'Revised question',body:'Keep uncertainty.'},state:'pending',result:{memory_basis:{...memory,version:2}},artifact_id:null,turn_id:null,change_set_id:null,basis_content_md:null};
  const run={...saved,status:'review',steps:[proposal]};vi.mocked(api.listAgentRuns).mockResolvedValue([run]);vi.mocked(api.getAgentRun).mockResolvedValue(run);
  const user=userEvent.setup();render(<AgentView initialRunId="run-a" onOpenArtifact={vi.fn()}/>);await user.click(await screen.findByRole('button',{name:'Review proposal'}));
  expect(memoryAction).toHaveBeenCalledWith({type:'read',id:'memory-a'});expect(screen.getByRole('button',{name:'Apply memory revision'})).toBeDisabled();expect(screen.getByRole('alert')).toHaveTextContent('changed');expect(api.reviewAgentStep).not.toHaveBeenCalled();
});
it('reviews project membership through its own native receipt and exposes guarded organization undo',async()=>{
  const proposal={id:'organization-a',sequence:1,action:{type:'propose_membership' as const,artifact_id:'page-a',project_id:'project-a',attached:true,reason:'Keep the research together.'},state:'pending',result:{pages:[{id:'page-a',title:'Research page'}],project:{id:'project-a',name:'Reading journal'}},artifact_id:'page-a',turn_id:null,change_set_id:null,basis_content_md:null};
  const run={...saved,status:'review',steps:[proposal]};vi.mocked(api.getAgentRun).mockResolvedValue(run);vi.mocked(api.reviewAgentStep).mockResolvedValue({...run,status:'completed',steps:[{...proposal,state:'applied',change_set_id:'change-a'}]});vi.mocked(api.undoAgentEdit).mockRejectedValue(new Error('This relationship changed after acceptance.'));
  const user=userEvent.setup();render(<AgentView initialRunId="run-a" onOpenArtifact={vi.fn()}/>);await user.click(await screen.findByRole('button',{name:'Review proposal'}));expect(screen.getByRole('region',{name:'Proposed relationship'})).toHaveTextContent('Reading journal');expect(api.reviewAgentStep).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button',{name:'Add to project'}));await waitFor(()=>expect(api.reviewAgentStep).toHaveBeenCalledWith('run-a','organization-a',true));await user.click(await screen.findByRole('button',{name:'Undo organization change'}));expect(api.undoAgentEdit).toHaveBeenCalledWith('run-a','organization-a');expect(await screen.findByRole('alert')).toHaveTextContent('changed after acceptance');
});
it('reviews project creation, exposes its actual project identity and uses guarded project undo',async()=>{
  const proposal={id:'project-step',sequence:1,action:{type:'propose_project_create' as const,name:'Release planning',purpose:'Review release uncertainty.',reason:'Keep planning separate.'},state:'pending',result:{same_name_projects:0},artifact_id:null,turn_id:null,change_set_id:null,basis_content_md:null};
  const run={...saved,project_id:null,status:'review',steps:[proposal]};vi.mocked(api.getAgentRun).mockResolvedValue(run);vi.mocked(api.reviewAgentStep).mockResolvedValue({...run,status:'completed',steps:[{...proposal,state:'applied',change_set_id:'cs-project',result:{...proposal.result,project_id:'created-project'}}]});vi.mocked(api.undoAgentEdit).mockRejectedValue(new Error('This project has been used since creation.'));
  const open=vi.fn(),user=userEvent.setup();render(<AgentView initialRunId="run-a" onOpenArtifact={vi.fn()} onOpenProject={open}/>);await user.click(await screen.findByRole('button',{name:'Review proposal'}));await user.click(screen.getByRole('button',{name:'Create project'}));await waitFor(()=>expect(api.reviewAgentStep).toHaveBeenCalledWith('run-a','project-step',true));await user.click(await screen.findByRole('button',{name:'Open project'}));expect(open).toHaveBeenCalledWith('created-project');await user.click(screen.getByRole('button',{name:'Undo project change'}));expect(api.undoAgentEdit).toHaveBeenCalledWith('run-a','project-step');expect(await screen.findByRole('alert')).toHaveTextContent('used since creation');
});
it('opens saved project citations by identity without dispatching a model',async()=>{
  const run={...saved,summary:'Continue in [Release planning](sthira://project/project-b).'};vi.mocked(api.getAgentRun).mockResolvedValue(run);const open=vi.fn(),user=userEvent.setup();render(<AgentView initialRunId="run-a" onOpenArtifact={vi.fn()} onOpenProject={open}/>);await user.click(await screen.findByRole('button',{name:'Release planning'}));expect(open).toHaveBeenCalledWith('project-b');expect(api.executeAgentRun).not.toHaveBeenCalled();
});

it('refreshes the project chooser after project creation and successful undo',async()=>{
  const proposal={id:'create-refresh',sequence:1,action:{type:'propose_project_create' as const,name:'Newly created project',purpose:'Investigate the open questions.',reason:'Create a project for this context.'},state:'pending',result:{same_name_projects:0},artifact_id:null,turn_id:null,change_set_id:null,basis_content_md:null};
  const run={...saved,project_id:null,status:'review',steps:[proposal]};vi.mocked(api.getAgentRun).mockResolvedValue(run);vi.mocked(api.reviewAgentStep).mockResolvedValue({...run,status:'completed',steps:[{...proposal,state:'applied',change_set_id:'new-cs',result:{...proposal.result,project_id:'new-project'}}]});
  const user=userEvent.setup();render(<AgentView initialRunId="run-a" onOpenArtifact={vi.fn()}/>);await user.click(await screen.findByRole('button',{name:'Review proposal'}));
  vi.mocked(getWorkspaceTree).mockResolvedValue({projects:[{id:'new-project',name:'Newly created project'}],loose_artifacts:[]} as unknown as Awaited<ReturnType<typeof getWorkspaceTree>>);
  await user.click(screen.getByRole('button',{name:'Create project'}));await screen.findByRole('button',{name:'Undo project change'});await user.click(screen.getByRole('button',{name:'New goal'}));expect(await screen.findByRole('option',{name:'Newly created project'})).toBeInTheDocument();
  const applied={...run,status:'completed',steps:[{...proposal,state:'applied',change_set_id:'new-cs',result:{...proposal.result,project_id:'new-project'}}]};
  vi.mocked(api.getAgentRun).mockResolvedValue(applied);vi.mocked(api.undoAgentEdit).mockResolvedValue({...run,status:'completed',steps:[{...proposal,state:'reverted'}]});vi.mocked(getWorkspaceTree).mockResolvedValue({projects:[],loose_artifacts:[]} as unknown as Awaited<ReturnType<typeof getWorkspaceTree>>);
  await user.click(screen.getByRole('button',{name:/Review my reading/}));await user.click(await screen.findByRole('button',{name:'Undo project change'}));await waitFor(()=>expect(screen.queryByRole('button',{name:'Undo project change'})).not.toBeInTheDocument());await user.click(screen.getByRole('button',{name:'New goal'}));expect(screen.queryByRole('option',{name:'Newly created project'})).not.toBeInTheDocument();
});

it('discovers an external invocation on task refresh without replacing an unsent goal',async()=>{
  const view=render(<AgentView onOpenArtifact={vi.fn()}/>);const user=userEvent.setup();await screen.findByRole('textbox',{name:'Agent goal'});await user.type(screen.getByRole('textbox',{name:'Agent goal'}),'Preserve my draft');
  const external={...saved,id:'external-run',project_id:null,goal:'External review request',model_name:'External: editor · declared model'};vi.mocked(api.listAgentRuns).mockResolvedValue([external]);state.taskRevision++;view.rerender(<AgentView onOpenArtifact={vi.fn()}/>);
  expect(await screen.findByRole('button',{name:/External review request/})).toBeInTheDocument();expect(screen.getByRole('textbox',{name:'Agent goal'})).toHaveValue('Preserve my draft');expect(api.executeAgentRun).not.toHaveBeenCalled();
});
it('guards a handoff draft against switching goals and leaving the Agent view',async()=>{
 vi.mocked(api.getAgentRun).mockResolvedValue(saved);let guard:(()=>Promise<boolean>)|null=null;const user=userEvent.setup();render(<AgentView initialRunId="run-a" onOpenArtifact={vi.fn()} registerBeforeLeave={value=>{guard=value;}}/>);await user.click(await screen.findByText('Save a new handoff'));await user.type(screen.getByRole('textbox',{name:'Handoff title'}),'Preserve this handoff');await waitFor(()=>expect(screen.getByRole('button',{name:'Discard handoff draft'})).toBeInTheDocument());expect(await guard!()).toBe(false);await user.click(screen.getByRole('button',{name:'New goal'}));expect(screen.getByRole('textbox',{name:'Handoff title'})).toHaveValue('Preserve this handoff');await user.click(screen.getByRole('button',{name:'Discard handoff draft'}));await waitFor(async()=>expect(await guard!()).toBe(true));
});
