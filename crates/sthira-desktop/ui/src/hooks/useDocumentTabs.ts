import { useEffect, useRef, useState } from 'react';
import { getWorkspaceSessionKey } from '../api/workspaceSession';
export interface DocumentTab { id:string; title:string; projectId:string|null; returnProjectId:string|null; scrollTop:number }
export interface DocumentTabsState { tabs:DocumentTab[]; activeId:string|null; resume:boolean }
export const emptyTabs=():DocumentTabsState=>({tabs:[],activeId:null,resume:false});
export function openDocumentTab(state:DocumentTabsState,tab:DocumentTab,inNewTab:boolean):DocumentTabsState {
  const existing=state.tabs.find(t=>t.id===tab.id);
  let tabs:DocumentTab[];
  if(existing)tabs=state.tabs.map(t=>t.id===tab.id?{...t,projectId:tab.projectId,returnProjectId:tab.returnProjectId}:t);
  else if(!inNewTab&&state.activeId&&state.tabs.some(t=>t.id===state.activeId))tabs=state.tabs.map(t=>t.id===state.activeId?tab:t);
  else tabs=[...state.tabs,tab];
  return {tabs,activeId:tab.id,resume:true};
}
export function closeDocumentTab(state:DocumentTabsState,id:string):DocumentTabsState {
  const index=state.tabs.findIndex(t=>t.id===id);
  const tabs=state.tabs.filter(t=>t.id!==id);
  return {...state,tabs,activeId:state.activeId===id?(tabs[Math.min(index,tabs.length-1)]?.id||null):state.activeId,resume:!!tabs.length&&state.resume};
}
export function readDocumentTabs(raw:string|null):DocumentTabsState {
  try {
    const value=JSON.parse(raw||'null');
    if(!Array.isArray(value?.tabs))return emptyTabs();
    const seen=new Set<string>();
    const tabs:DocumentTab[]=value.tabs.filter((t:DocumentTab)=>typeof t?.id==='string'&&t.id&&typeof t.title==='string'&&!seen.has(t.id)&&seen.add(t.id)).map((t:DocumentTab)=>({id:t.id,title:t.title.slice(0,500),projectId:typeof t.projectId==='string'?t.projectId:null,returnProjectId:typeof t.returnProjectId==='string'?t.returnProjectId:null,scrollTop:Number.isFinite(t.scrollTop)?Math.max(0,t.scrollTop):0}));
    return {tabs,activeId:tabs.some(t=>t.id===value.activeId)?value.activeId:tabs[0]?.id||null,resume:value.resume===true};
  }catch{return emptyTabs();}
}
export function useDocumentTabs(onRestore:(state:DocumentTabsState)=>void) {
  const [state,setState]=useState(emptyTabs);
  const [namespace,setNamespace]=useState('');
  const stateRef=useRef(state);
  const touched=useRef(false);
  const restore=useRef(onRestore);restore.current=onRestore;
  const update=(next:DocumentTabsState|((previous:DocumentTabsState)=>DocumentTabsState))=>{
    touched.current=true;
    const value=typeof next==='function'?next(stateRef.current):next;
    stateRef.current=value;setState(value);
  };
  useEffect(()=>{let alive=true;getWorkspaceSessionKey().then(key=>{
    if(!alive)return;setNamespace(key);
    if(!touched.current){let value=emptyTabs();try{value=readDocumentTabs(localStorage.getItem(`sthira.tabs.v1:${key}`));}catch{/* Browsing still works when storage is unavailable. */}stateRef.current=value;setState(value);restore.current(value);}
  }).catch(()=>{/* Tabs work in memory if workspace session storage is unavailable. */});return()=>{alive=false;};},[]);
  useEffect(()=>{if(namespace)try{localStorage.setItem(`sthira.tabs.v1:${namespace}`,JSON.stringify(state));}catch{/* Keep the current in-memory session. */}},[state,namespace]);
  return {state,stateRef,update,namespace};
}
