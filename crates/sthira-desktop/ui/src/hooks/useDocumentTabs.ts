import { useEffect, useRef, useState } from 'react';
import { getWorkspaceSessionKey } from '../api/workspaceSession';
import { filterExistingArtifacts } from '../api/ipc';
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
export function pruneMissingTabs(state:DocumentTabsState,existingIds:string[]):DocumentTabsState {
  const valid=new Set(existingIds);
  const tabs=state.tabs.filter(t=>valid.has(t.id));
  const activeId=tabs.some(t=>t.id===state.activeId)?state.activeId:(tabs[0]?.id||null);
  return {tabs,activeId,resume:tabs.length>0&&state.resume};
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
const TABS_KEY_PREFIX = 'sthira.tabs.v1:';
const CHAT_DRAFT_KEY_PREFIX = 'sthira.chat-draft:';

function clearNamespaceStorage(targetNamespace: string) {
  if (!targetNamespace) return;
  try {
    localStorage.removeItem(`${TABS_KEY_PREFIX}${targetNamespace}`);
  } catch { /* Ignore storage errors. */ }
  const chatPrefix = `${CHAT_DRAFT_KEY_PREFIX}${targetNamespace}:`;
  try {
    for (let i = sessionStorage.length - 1; i >= 0; i--) {
      const k = sessionStorage.key(i);
      if (k && k.startsWith(chatPrefix)) sessionStorage.removeItem(k);
    }
  } catch { /* Ignore storage errors. */ }
}
export function useDocumentTabs(onRestore:(state:DocumentTabsState)=>void) {
  const [state,setState]=useState(emptyTabs);
  const [namespace,setNamespace]=useState('');
  const stateRef=useRef(state);
  const namespaceRef=useRef('');
  const epochRef=useRef(0);
  const restoringRef=useRef(true);
  const postResetNamespacesRef=useRef<Set<string>>(new Set());
  const touched=useRef(false);
  const restore=useRef(onRestore);restore.current=onRestore;
  const update=(next:DocumentTabsState|((previous:DocumentTabsState)=>DocumentTabsState))=>{
    touched.current=true;
    restoringRef.current=false;
    const value=typeof next==='function'?next(stateRef.current):next;
    stateRef.current=value;setState(value);
  };
  useEffect(()=>{
    let alive=true;
    const initialEpoch=epochRef.current;
    restoringRef.current=true;
    (async()=>{
      try{
        const key=await getWorkspaceSessionKey();
        if(!alive)return;
        if(initialEpoch!==epochRef.current){
          if(key&&key!==namespaceRef.current&&!postResetNamespacesRef.current.has(key)){
            clearNamespaceStorage(key);
          }
          return;
        }
        if(!key){restoringRef.current=false;return;}
        namespaceRef.current=key;
        setNamespace(key);
        if(!touched.current){
          let value=emptyTabs();
          try{value=readDocumentTabs(localStorage.getItem(`${TABS_KEY_PREFIX}${key}`));}catch{/* Browsing still works when storage is unavailable. */}
          if(value.tabs.length>0){
            try{
              const existing=await filterExistingArtifacts(value.tabs.map(t=>t.id));
              if(!alive||initialEpoch!==epochRef.current||namespaceRef.current!==key)return;
              if(Array.isArray(existing))value=pruneMissingTabs(value,existing);
            }catch{/* Keep parsed tabs if existence validation fails transiently. */}
          }
          if(!alive||initialEpoch!==epochRef.current||namespaceRef.current!==key)return;
          restoringRef.current=false;
          if(touched.current)return;
          stateRef.current=value;setState(value);restore.current(value);
        } else {
          restoringRef.current=false;
        }
      }catch{
        if(alive&&initialEpoch===epochRef.current)restoringRef.current=false;
      }
    })();
    const handleVaultReset=()=>{
      const previousNamespace=namespaceRef.current;
      postResetNamespacesRef.current.delete(previousNamespace);
      const resetEpoch=++epochRef.current;
      touched.current=false;
      restoringRef.current=false;
      namespaceRef.current='';
      setNamespace('');
      clearNamespaceStorage(previousNamespace);
      const empty=emptyTabs();
      stateRef.current=empty;
      setState(empty);
      getWorkspaceSessionKey().then(nextKey=>{
        if(!alive)return;
        if(resetEpoch!==epochRef.current){
          if(nextKey&&nextKey!==namespaceRef.current&&!postResetNamespacesRef.current.has(nextKey)){
            clearNamespaceStorage(nextKey);
          }
          return;
        }
        if(nextKey){
          postResetNamespacesRef.current.add(nextKey);
          namespaceRef.current=nextKey;
          setNamespace(nextKey);
        }
      }).catch(()=>{/* Session key refresh best-effort. */});
    };
    window.addEventListener('sthira:vault-reset',handleVaultReset);
    return()=>{alive=false;epochRef.current+=1;window.removeEventListener('sthira:vault-reset',handleVaultReset);};
  },[]);
  useEffect(()=>{if(namespace&&namespace===namespaceRef.current&&(touched.current||!restoringRef.current))try{localStorage.setItem(`${TABS_KEY_PREFIX}${namespace}`,JSON.stringify(state));}catch{/* Keep the current in-memory session. */}},[state,namespace]);
  return {state,stateRef,update,namespace};
}
