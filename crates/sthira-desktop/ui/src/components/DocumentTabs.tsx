import { useEffect } from 'react';
import { ChevronLeft, ChevronRight, X } from 'lucide-react';
import type { DocumentTab } from '../hooks/useDocumentTabs';
interface Props { tabs:DocumentTab[]; activeId:string|null; onSelect:(id:string)=>void; onClose:(id:string)=>void; onMove:(id:string,offset:number)=>void }
export function DocumentTabs({tabs,activeId,onSelect,onClose,onMove}:Props) {
  useEffect(()=>{if(activeId)document.getElementById(`document-tab-${activeId}`)?.scrollIntoView({block:'nearest',inline:'nearest'});},[activeId]);
  if(tabs.length<2)return null;
  return <nav className="document-tabs" aria-label="Open documents"><div className="document-tab-list" role="tablist" aria-label="Document tabs" onKeyDown={event=>{
    if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;
    const index=tabs.findIndex(t=>t.id===activeId);
    const next=event.key==='Home'?0:event.key==='End'?tabs.length-1:(index+(event.key==='ArrowRight'?1:-1)+tabs.length)%tabs.length;
    event.preventDefault();onSelect(tabs[next].id);
    document.getElementById(`document-tab-${tabs[next].id}`)?.focus();
  }}>{tabs.map((tab,index)=><div className={`document-tab ${tab.id===activeId?'is-active':''}`} key={tab.id}>
    <button type="button" role="tab" id={`document-tab-${tab.id}`} aria-selected={tab.id===activeId} tabIndex={tab.id===activeId||!activeId&&index===0?0:-1} title={tab.title} onClick={()=>onSelect(tab.id)}>{tab.title||'Untitled page'}</button>
    <div className="document-tab-actions"><button type="button" aria-label={`Move ${tab.title} left`} title="Move tab left" disabled={index===0} onClick={()=>onMove(tab.id,-1)}><ChevronLeft size={12}/></button><button type="button" aria-label={`Move ${tab.title} right`} title="Move tab right" disabled={index===tabs.length-1} onClick={()=>onMove(tab.id,1)}><ChevronRight size={12}/></button><button type="button" aria-label={`Close tab: ${tab.title}`} title="Close tab" onClick={()=>onClose(tab.id)}><X size={13}/></button></div>
  </div>)}</div></nav>;
}
