import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { isTauri } from '../api/ipc';
import { AGENT_TASKS_CHANGED, isAgentActive, listAgentTasks, type AgentTask } from '../api/workspaceAgent';
interface Value {tasks:AgentTask[];loading:boolean;error:string|null;refresh:()=>void;revision:number}
const Context=createContext<Value>({tasks:[],loading:false,error:null,refresh:()=>{},revision:0});
export function useAgentTasks(){return useContext(Context);}
export function AgentTasksProvider({children}:{children:ReactNode}) {
  const [tasks,setTasks]=useState<AgentTask[]>([]),[loading,setLoading]=useState(true),[error,setError]=useState<string|null>(null),[revision,setRevision]=useState(0);
  const mounted=useRef(true),epoch=useRef(0),request=useRef(0),snapshot=useRef('');
  const refresh=useCallback(()=>{
    const currentEpoch=epoch.current,run=++request.current;
    void listAgentTasks().then(value=>{if(mounted.current&&epoch.current===currentEpoch&&request.current===run){const next=JSON.stringify(value);if(next!==snapshot.current){snapshot.current=next;setTasks(value);setRevision(n=>n+1);}setError(null);}}).catch(e=>{if(mounted.current&&epoch.current===currentEpoch&&request.current===run)setError(String(e));}).finally(()=>{if(mounted.current&&epoch.current===currentEpoch&&request.current===run)setLoading(false);});
  },[]);
  useEffect(()=>{
    mounted.current=true;refresh();
    const reset=()=>{epoch.current++;request.current++;snapshot.current='';setTasks([]);setError(null);setLoading(false);setRevision(n=>n+1);refresh();};
    window.addEventListener(AGENT_TASKS_CHANGED,refresh);window.addEventListener('sthira:vault-reset',reset);
    let disposed=false,stop:(()=>void)|undefined;
    if(isTauri)void import('@tauri-apps/api/event').then(({listen})=>listen<{type:string}>('sthira://core-event',e=>{if(!disposed&&e.payload.type==='agent_changed')refresh();})).then(unlisten=>{if(disposed)unlisten();else stop=unlisten;}).catch(()=>{});
    return()=>{mounted.current=false;epoch.current++;request.current++;disposed=true;stop?.();window.removeEventListener(AGENT_TASKS_CHANGED,refresh);window.removeEventListener('sthira:vault-reset',reset);};
  },[refresh]);
  const active=tasks.some(isAgentActive);
  useEffect(()=>{if(!active)return;const timer=window.setInterval(refresh,1500);return()=>window.clearInterval(timer);},[active,refresh]);
  return <Context.Provider value={{tasks,loading,error,refresh,revision}}>{children}</Context.Provider>;
}
