import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useState } from 'react';
import { AgentTasksProvider, useAgentTasks } from '../context/AgentTasksContext';
import { AgentTaskCenter } from '../components/AgentTaskCenter';
import { AgentView } from '../views/AgentView';
import * as api from '../api/workspaceAgent';
import type { AgentRun, AgentTask } from '../api/workspaceAgent';
vi.mock('../api/workspaceAgent',async original=>({...await original<typeof import('../api/workspaceAgent')>(),listAgentTasks:vi.fn(),listAgentRuns:vi.fn(),getAgentRun:vi.fn(),createAgentRun:vi.fn(),executeAgentRun:vi.fn(),cancelAgentRun:vi.fn()}));
vi.mock('../api/ipc',()=>({isTauri:true,getWorkspaceTree:vi.fn(async()=>({projects:[]})),getProviderCatalog:vi.fn(async()=>[{id:'provider-a',display_name:'Configured connection',is_enabled:true,models:[{id:'model-a',provider_id:'provider-a',display_name:'Configured model',is_enabled:true,is_default:true}]}]),getArtifactContext:vi.fn()}));
vi.mock('@tauri-apps/api/event',()=>({listen:vi.fn(async()=>()=>{})}));
const active:AgentTask={id:'task-a',thread_id:'task-a',parent_run_id:null,goal:'Review current note',project_id:null,document_id:'doc-a',model_name:'Configured model',provider_name:'Configured connection',status:'running',error:null,updated_at:1000,revision:2};
const run:AgentRun={...active,goal:'Review current note',workspace_memory:false,model_id:'model-a',provider_id:'provider-a',summary:null,handoff:null,created_at:1000,steps:[]};
let current:AgentTask[]=[];
beforeEach(()=>{vi.clearAllMocks();current=[];vi.mocked(api.listAgentTasks).mockReset().mockImplementation(async()=>current);vi.mocked(api.listAgentRuns).mockResolvedValue([]);vi.mocked(api.getAgentRun).mockResolvedValue(run);vi.mocked(api.createAgentRun).mockResolvedValue({...run,status:'queued'});vi.mocked(api.executeAgentRun).mockImplementation(async()=>{current=[active];api.notifyAgentTasksChanged();return run;});vi.mocked(api.cancelAgentRun).mockReset().mockImplementation(async()=>{current=[{...active,status:'cancelled',revision:3}];return {...run,status:'cancelled'};});});
afterEach(cleanup);
function Probe(){const {tasks}=useAgentTasks();return <div><output data-testid="task-count">{tasks.length}</output>{tasks.map(task=><p key={task.id}>{task.goal} · {task.status}</p>)}</div>;}
it('retains task progress after the requesting workspace view unmounts and shows its later result',async()=>{
  function Workspace(){const [work,setWork]=useState(true),[open,setOpen]=useState(false);return <><button onClick={()=>setWork(false)}>Leave work view</button><button onClick={()=>setOpen(true)}>Open tasks</button><Probe/>{work&&<AgentView onOpenArtifact={vi.fn()}/>}<AgentTaskCenter open={open} onClose={()=>setOpen(false)} onOpenRun={vi.fn()} onOpenDocument={vi.fn()}/></>;}
  const user=userEvent.setup();render(<AgentTasksProvider><Workspace/></AgentTasksProvider>);await screen.findByRole('option',{name:/Configured model/});await user.type(screen.getByRole('textbox',{name:'Agent goal'}),'A background goal');await user.click(screen.getByRole('button',{name:'Start working'}));await screen.findByText('Review current note · running');await user.click(screen.getByRole('button',{name:'Leave work view'}));expect(screen.queryByRole('textbox',{name:'Agent goal'})).not.toBeInTheDocument();expect(screen.getByTestId('task-count')).toHaveTextContent('1');expect(api.cancelAgentRun).not.toHaveBeenCalled();current=[{...active,status:'completed',revision:4}];act(()=>api.notifyAgentTasksChanged());await screen.findByText('Review current note · completed');await user.click(screen.getByRole('button',{name:'Open tasks'}));expect(screen.getByRole('dialog',{name:'Agent tasks'})).toHaveTextContent('Review current note');expect(screen.queryByRole('button',{name:'Stop task'})).not.toBeInTheDocument();
});
it('stops a task globally and opens its document or saved run by identity',async()=>{
  current=[active];const openRun=vi.fn(),openDocument=vi.fn(),user=userEvent.setup();render(<AgentTasksProvider><AgentTaskCenter open onClose={vi.fn()} onOpenRun={openRun} onOpenDocument={openDocument}/></AgentTasksProvider>);await screen.findByText('Review current note');await user.click(screen.getByRole('button',{name:'View task'}));expect(openRun).toHaveBeenCalledWith('task-a');await user.click(screen.getByRole('button',{name:'Open document'}));expect(openDocument).toHaveBeenCalledWith('doc-a');await user.click(screen.getByRole('button',{name:'Stop task'}));expect(api.cancelAgentRun).toHaveBeenCalledWith('task-a');await screen.findByText(/Stopped by you/);
});
it('rejects a pre-reset task response and keeps the reset workspace empty',async()=>{
  let resolve!:(tasks:AgentTask[])=>void;vi.mocked(api.listAgentTasks).mockImplementationOnce(()=>new Promise(done=>{resolve=done;}));render(<AgentTasksProvider><Probe/></AgentTasksProvider>);act(()=>window.dispatchEvent(new Event('sthira:vault-reset')));await waitFor(()=>expect(screen.getByTestId('task-count')).toHaveTextContent('0'));await act(async()=>resolve([active]));expect(screen.getByTestId('task-count')).toHaveTextContent('0');expect(screen.queryByText(/Review current note/)).not.toBeInTheDocument();
});
it('ignores a late cancellation error from the vault that was reset',async()=>{
  current=[active];let reject!:(error:Error)=>void;vi.mocked(api.cancelAgentRun).mockImplementationOnce(()=>new Promise((_,fail)=>{reject=fail;}));const user=userEvent.setup();render(<AgentTasksProvider><AgentTaskCenter open onClose={vi.fn()} onOpenRun={vi.fn()} onOpenDocument={vi.fn()}/></AgentTasksProvider>);await user.click(await screen.findByRole('button',{name:'Stop task'}));current=[];act(()=>window.dispatchEvent(new Event('sthira:vault-reset')));await act(async()=>reject(new Error('Old vault failure')));expect(screen.queryByText(/Old vault failure/)).not.toBeInTheDocument();expect(await screen.findByText('Room to keep working.')).toBeInTheDocument();
});
it('loads saved tasks without dispatching them and provides keyboard dismissal',async()=>{
  current=[{...active,status:'interrupted'}];const close=vi.fn(),user=userEvent.setup();render(<AgentTasksProvider><AgentTaskCenter open onClose={close} onOpenRun={vi.fn()} onOpenDocument={vi.fn()}/></AgentTasksProvider>);await screen.findByText(/Interrupted/);expect(api.executeAgentRun).not.toHaveBeenCalled();await user.keyboard('{Escape}');expect(close).toHaveBeenCalled();
});
