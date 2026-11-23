import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { ListTodo, X, Square, ArrowUpRight } from 'lucide-react';
import { cancelAgentRun, isAgentActive, type AgentTask } from '../api/workspaceAgent';
import { useAgentTasks } from '../context/AgentTasksContext';
import { useDialogFocus } from '../hooks/useDialogFocus';
import { isTauri } from '../api/ipc';
import '../styles/agent-tasks.css';
interface Props {open:boolean;onClose:()=>void;onOpenRun:(id:string)=>void;onOpenDocument:(id:string)=>void}
const labels:Record<string,string>={queued:'Queued',running:'Working',review:'Ready for review',completed:'Finished',failed:'Stopped with an error',cancelled:'Stopped by you',interrupted:'Interrupted'};
export function AgentTaskCenter({open,onClose,onOpenRun,onOpenDocument}:Props) {
  const {tasks,loading,error,refresh}=useAgentTasks();
  const [stopping,setStopping]=useState<string|null>(null),[actionError,setActionError]=useState('');
  const epoch=useRef(0),alive=useRef(true);
  useEffect(()=>{alive.current=true;const reset=()=>{epoch.current++;setStopping(null);setActionError('');};window.addEventListener('sthira:vault-reset',reset);return()=>{alive.current=false;epoch.current++;window.removeEventListener('sthira:vault-reset',reset);};},[]);
  const panel=useRef<HTMLDivElement>(null),close=useRef<HTMLButtonElement>(null);
  useDialogFocus(open,panel,onClose,{initialFocus:close});
  const stop=async(task:AgentTask)=>{if(stopping)return;setStopping(task.id);setActionError('');const run=epoch.current;try{await cancelAgentRun(task.id);refresh();}catch(e){if(alive.current&&run===epoch.current){setActionError(String(e));refresh();}}finally{if(alive.current&&run===epoch.current)setStopping(null);}};
  if(!open)return null;
  return createPortal(<div className="agent-tasks-layer" onMouseDown={e=>{if(e.target===e.currentTarget)onClose();}}><div ref={panel} role="dialog" aria-modal="true" aria-labelledby="agent-tasks-title" className="agent-tasks-panel">
    <header><div><p>WORK IN PROGRESS</p><h2 id="agent-tasks-title">Agent tasks</h2></div><button ref={close} type="button" aria-label="Close agent tasks" onClick={onClose}><X size={20}/></button></header>
    <p className="agent-tasks-help">Keep writing while Sthira works. Tasks run while the desktop app is open; proposed edits wait for review.</p>
    {!isTauri&&<p className="agent-tasks-help">Browser preview · no background agent work is simulated.</p>}
    {loading&&<p role="status">Loading saved tasks…</p>}
    {(error||actionError)&&<p role="alert" className="agent-tasks-error">{actionError||error} <button type="button" onClick={refresh}>Retry task loading</button></p>}
    {!loading&&!error&&!tasks.length&&<div className="agent-tasks-empty"><ListTodo size={26}/><h3>Room to keep working.</h3><p>Start a goal in Agent or send a message from a document. Its progress and saved result will appear here.</p></div>}
    <ul>{tasks.map(task=><li key={task.id}><div className="agent-task-status">{labels[task.status]||task.status} · {task.document_id?'Document':'Workspace'}{task.parent_run_id?' · Follow-up':''}</div><h3>{task.goal}</h3><p>{task.model_name} · {task.provider_name}</p>{task.error&&<p className="agent-tasks-error">{task.error}</p>}<div className="agent-task-actions"><button type="button" onClick={()=>onOpenRun(task.id)}>View task <ArrowUpRight size={14}/></button>{task.document_id&&<button type="button" onClick={()=>onOpenDocument(task.document_id!)}>Open document</button>}{isAgentActive(task)&&<button type="button" disabled={!!stopping} onClick={()=>void stop(task)}><Square size={13}/>{stopping===task.id?'Stopping…':'Stop task'}</button>}</div></li>)}</ul>
    <footer>Recent tasks · up to 50 shown <button type="button" onClick={refresh}>Refresh tasks</button></footer>
  </div></div>,document.body);
}
