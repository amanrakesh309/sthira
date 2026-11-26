import { useState } from 'react';
import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import type { AgentRun, AgentStep } from '../api/workspaceAgent';
import type { MemoryEntry, MemoryScope } from '../api/sharedMemory';
import { DocumentEditReview } from './DocumentEditReview';

interface Props {
  run:AgentRun; step:AgentStep; current:string; memory?:MemoryEntry; projectNames?:Record<string,string>;
  stale:boolean; pending:boolean; error:string|null;
  onClose:()=>void; onApply:(title?:string)=>void; onDismiss:()=>void;
  onOpenArtifact:(id:string)=>void;
}
const scopeName=(scope:MemoryScope|undefined,names:Record<string,string>)=>!scope?'Original scope':scope.kind==='workspace'?'Workspace':scope.kind==='session'?'This conversation':names[scope.id]||'Original project';

export const decodeSourceEntities=(value:string)=>value
  .replace(/&#(x[0-9a-f]+|[0-9]+);/gi,(_match,code:string)=>{
    const parsed=code[0].toLowerCase()==='x'?Number.parseInt(code.slice(1),16):Number.parseInt(code,10);
    if(!Number.isInteger(parsed)||parsed<=0||parsed>0x10ffff||(parsed>=0xd800&&parsed<=0xdfff))return _match;
    return String.fromCodePoint(parsed);
  })
  .replace(/&(amp|lt|gt|quot|apos|nbsp|hellip|mdash|ndash|lsquo|rsquo|ldquo|rdquo|copy|reg|trade);/g,(_match,name:string)=>({
    amp:'&',lt:'<',gt:'>',quot:'"',apos:"'",nbsp:' ',hellip:'…',mdash:'—',ndash:'–',lsquo:'‘',rsquo:'’',ldquo:'“',rdquo:'”',copy:'©',reg:'®',trade:'™',
  } as Record<string,string>)[name]||_match);

const sourceNumber=(source:Record<string,unknown>,index:number)=>{
  const value=Number(source.index);
  return Number.isInteger(value)&&value>0?value:index+1;
};

const parseCitationLabel=(label:string,numbers:Set<number>):number[]|null=>{
  if(!/^Sources?\s+/i.test(label))return null;
  const body=label.replace(/^Sources?\s+/i,'');
  const parsed:number[]=[];
  for(const part of body.split(/[,;]/)){
    const cleaned=part.trim().replace(/^Source\s+/i,'').trim();
    if(!cleaned)return null;
    const range=cleaned.split(/[-–]/).map(item=>item.trim());
    if(range.length>2||range.some(item=>!/^\d+$/.test(item)))return null;
    const first=Number(range[0]);const last=Number(range[1]??range[0]);
    if(!Number.isInteger(first)||!Number.isInteger(last)||first<1||last<first||last-first>50)return null;
    for(let number=first;number<=last;number+=1){
      if(!numbers.has(number))return null;
      parsed.push(number);
    }
  }
  return Array.from(new Set(parsed));
};

const citationLinks=(numbers:number[])=>numbers.map(number=>`[Source ${number}](#research-source-${number})`).join(', ');

interface MdastNode{
  type:string;
  value?:string;
  position?:{start?:{offset?:number};end?:{offset?:number}};
  children?:MdastNode[];
}

interface TextEdit{start:number;end:number;replacement:string;}

const citationMarkdownParser=unified().use(remarkParse).use(remarkGfm);

const linkPlainLabel=(node:MdastNode):string|null=>{
  if(!Array.isArray(node.children)||node.children.length===0)return null;
  let text='';
  for(const child of node.children){
    if(child.type!=='text'||typeof child.value!=='string')return null;
    text+=child.value;
  }
  return text;
};

const collectTextCitationEdits=(markdown:string,startOffset:number,endOffset:number,numbers:Set<number>,edits:TextEdit[])=>{
  const slice=markdown.slice(startOffset,endOffset);
  let index=0;
  while(index<slice.length){
    if(slice[index]==='\\'){index+=2;continue;}
    if(slice[index]==='['){
      const close=slice.indexOf(']',index+1);
      if(close>index+1){
        const label=slice.slice(index+1,close);
        const valid=parseCitationLabel(label,numbers);
        if(valid!==null){
          edits.push({start:startOffset+index,end:startOffset+close+1,replacement:citationLinks(valid)});
          index=close+1;
          continue;
        }
      }
    }
    index+=1;
  }
};

const SKIP_CITATION_NODES=new Set(['code','inlineCode','html','yaml','toml','definition']);

const visitCitationNodes=(node:MdastNode,markdown:string,numbers:Set<number>,edits:TextEdit[])=>{
  if(SKIP_CITATION_NODES.has(node.type))return;
  const start=node.position?.start?.offset;
  const end=node.position?.end?.offset;
  if(node.type==='link'||node.type==='linkReference'){
    if(typeof start==='number'&&typeof end==='number'&&start>=0&&end<=markdown.length){
      const label=linkPlainLabel(node);
      if(label!==null){
        const valid=parseCitationLabel(label,numbers);
        if(valid!==null){
          edits.push({start,end,replacement:citationLinks(valid)});
        }else if(/^Sources?\s+/i.test(label)){
          edits.push({start,end,replacement:`[${label}]`});
        }
      }
    }
    return;
  }
  if(node.type==='text'){
    if(typeof start==='number'&&typeof end==='number'&&start>=0&&end<=markdown.length&&end>start){
      collectTextCitationEdits(markdown,start,end,numbers,edits);
    }
    return;
  }
  if(Array.isArray(node.children)){
    for(const child of node.children)visitCitationNodes(child,markdown,numbers,edits);
  }
};

/** Links only validated citation-shaped labels via a parsed Markdown AST. Code spans
 * (inline and multiline), fenced and indented code blocks, ordinary links, and
 * malformed/mixed groups keep their text and established behavior. */
export const linkResearchCitations=(markdown:string,numbers:Set<number>)=>{
  const tree=citationMarkdownParser.parse(markdown) as unknown as MdastNode;
  const edits:TextEdit[]=[];
  visitCitationNodes(tree,markdown,numbers,edits);
  if(edits.length===0)return markdown;
  edits.sort((a,b)=>a.start-b.start);
  let output='';
  let cursor=0;
  for(const edit of edits){
    if(edit.start<cursor)continue;
    output+=markdown.slice(cursor,edit.start)+edit.replacement;
    cursor=edit.end;
  }
  return output+markdown.slice(cursor);
};

export function AgentWorkflowReview({run,step,current,memory,projectNames={},stale,pending,error,onClose,onApply,onDismiss,onOpenArtifact}:Props) {
  const [title,setTitle]=useState('');
  const action=step.action;
  const naming=action.type==='propose_titles';
  const deriving=action.type==='propose_derive';
  const researchGeneration=action.type==='propose_research';
  const planGeneration=action.type==='propose_plan';
  const generation=researchGeneration||planGeneration;
  const packet=(step.result.packet as Record<string, unknown>|undefined)||{};
  const researchSourceList=Array.isArray(packet.sources)?packet.sources.map((entry,index)=>{
    const source=(entry as Record<string,unknown>)||{};
    return {number:sourceNumber(source,index),title:decodeSourceEntities(String(source.title||source.url||`Source ${index+1}`)),url:String(source.url||'')};
  }):[];
  const validSourceNumbers=new Set(researchSourceList.map(source=>source.number));
  const articleDocuments=(Array.isArray(packet.documents)?packet.documents as Record<string,unknown>[]:[]).filter(doc=>/^https?:\/\//i.test(String(doc.url||'')));
  const provenanceKnown=articleDocuments.some(doc=>typeof doc.from_cache==='boolean'||doc.fetched_at!=null);
  const cachedArticles=articleDocuments.filter(doc=>doc.from_cache===true).length;
  const freshArticles=articleDocuments.length-cachedArticles;
  const memoryChange=action.type==='propose_memory_create'||action.type==='propose_memory_update';
  const basis=step.result.memory_basis as MemoryEntry|undefined;
  const scope=(step.result.scope as MemoryScope|undefined)||basis?.scope;
  const sourceEntries=memoryChange?(basis?.sources || (step.result.sources as {id:string;title:string|null}[]|undefined)?.map(s=>({artifact_id:s.id,title:s.title||'Source page'})) || []):[];
  let proposed='content_md' in action?action.content_md:'';
  if(generation) proposed=String(packet.content_md||'');
  if(researchGeneration) proposed=linkResearchCitations(proposed,validSourceNumbers);
  let summary='Review this refinement. Applying saves a restore snapshot; Undo refuses to overwrite later work.';
  let heading='Refine document',applyLabel='Apply refinement',footnote='The saved page changes only when you apply this refinement.';
  let currentLabel='Current document',proposedLabel='Proposed refinement';
  if(generation) {
    const template=packet.template as {name?:string;version?:number;id?:string}|null|undefined;
    const destination=packet.destination as {project_id?:string|null}|undefined;
    const projectId=destination?.project_id ?? (researchGeneration && !destination ? run.project_id : null);
    const location=projectId ? `in ${projectNames[projectId] || 'the selected project'}` : 'as a standalone page';
    const source=packet.source as {artifact_id?:string;title?:string|null;version?:number}|null|undefined;
    const searchStatus=String(packet.search_status||packet.evidence_mode||'not used');
    const provider=packet.search_provider?` · ${String(packet.search_provider)}`:'';
    const model=packet.model_name?`${String(packet.model_name)} · ${String(packet.provider_name||'')}`:`${run.model_name} · ${run.provider_name}`;
    const templateText=template?.name?`${template.name} (revision ${template.version})`:'No template';
    if(researchGeneration) {
      const explicitCount=Number(packet.external_sources_count);
      const inferredCount=Number.isFinite(explicitCount)
        ? explicitCount
        : packet.evidence_mode==='always_search' && packet.web_used && Array.isArray(packet.sources) ? researchSourceList.filter(source=>source.url.startsWith('http')).length : 0;
      const webBacked=inferredCount>0;
      const attempted=packet.search_attempted===true || (packet.search_requested===true && packet.web_grant===true);
      const webBasis=webBacked
        ? `Web research contributed ${inferredCount} external source${inferredCount===1?'':'s'}.`
        : packet.web_grant
          ? attempted
            ? 'Web research was allowed and attempted, but this proposal used zero external sources.'
            : 'Web research was allowed but not used.'
          : 'Web research was off for this run.';
      heading='Research proposal';
      applyLabel='Create research page';
      summary=`Create an AI-authored research page ${location}. Grounding: ${searchStatus}${provider}. Producing model: ${model}. Template: ${templateText}. ${webBasis}`;
      current='No page, source record, or project has been created. This proposal is stored only as run activity until you accept it.';
      currentLabel='Before accepting';proposedLabel='Proposed research page';
      footnote='Accepting creates the page once and records its sources, model, and template snapshot. To turn accepted research into a plan, accept first, then ask for a follow-up plan from that saved page.';
    } else {
      heading='Plan proposal';
      applyLabel='Create plan page';
      summary=`Create an AI-authored plan ${location}. Producing model: ${model}. Template: ${templateText}. ${source?.artifact_id?`Derived from the saved page “${source.title||'Untitled page'}” (version ${source.version}).`:'No saved source page was linked; review its assumptions carefully.'}`;
      current=source?.artifact_id?`Saved source: ${source.title||'Untitled page'} (version ${source.version}). The source stays intact.`:'No accepted source page. This plan is not lineage-linked to research.';
      currentLabel='Source and lineage';proposedLabel='Proposed plan page';
      footnote='Accepting creates the plan page and records derivation lineage when a source was supplied. Nothing is applied until you accept.';
    }
  } else if(naming) {
    heading='Title suggestions';applyLabel='Use title';
    current=`Title: ${(step.result.basis as {title?:string})?.title || 'Untitled'}`;
    proposed=title?`Title: ${title}`:'Choose a suggested title above.';
    summary='Choose one saved suggestion. A matching first-level heading is renamed with the title; other text stays intact.';
    footnote='Your title changes only after you choose Use title. You can rename it again in the editor.';
    currentLabel='Current title';proposedLabel='Selected title';
  } else if(deriving) {
    heading='Derive a new page';applyLabel='Create derived page';
    summary=`Create **${action.title}** as an AI-authored page with source lineage. Method: ${({plan_from_research:'Execution plan',extract_ideas:'Ideas',extract_tasks:'Task checklist'}[action.derivation_type]||'Derived page')}. Destination: ${step.result.target_project_id ? `project ${projectNames[String(step.result.target_project_id)]||'selected during the request'}`:'standalone page'}.`;
    currentLabel='Saved source document';proposedLabel='Proposed derived page';
    footnote='The source stays intact. The new page is created only when you accept; you can delete it through the normal page actions.';
  } else if(memoryChange) {
    heading=action.type==='propose_memory_create'?'New shared memory':'Shared memory revision';applyLabel=action.type==='propose_memory_create'?'Save memory':'Apply memory revision';
    proposed=`## ${action.title}\n\n${action.body}`;
    current=memory?`## ${memory.title}\n\n${memory.body}`:'No memory has been created.';
    const author=basis?.author.kind==='user'?'Original author: you.':basis?.author.kind==='agent'?`Original author: ${basis.author.model} · ${basis.author.provider}.`:'New memory is attributed to Sthira’s agent and this model.';
    summary=`Scope: ${scopeName(scope,projectNames)}. ${author} Revision editor: ${run.model_name} · ${run.provider_name}. Existing source evidence, scope and pinned state are preserved.`;
    currentLabel=memory?'Current shared memory':'Before saving';proposedLabel='Proposed shared memory';
    footnote='Memory is context, not verified evidence. Acceptance is saved with revision history; edit or archive it in Shared memory later.';
  }
  return <DocumentEditReview operation={deriving?'create':'edit'} researchCitations={researchGeneration} heading={heading} current={current} proposed={proposed} summary={summary} currentLabel={currentLabel} proposedLabel={proposedLabel} applyLabel={applyLabel} footnote={footnote} stale={stale} pending={pending} busy={naming&&!title} error={error||(stale?'The saved source or memory changed. Ask for a fresh proposal before applying.':null)} onClose={onClose} onApply={()=>onApply(naming?title:undefined)} onDismiss={onDismiss} onOpenArtifact={onOpenArtifact}>
    <p className="agent-helper">Prepared by {run.model_name} · {run.provider_name}</p>
    {generation&&<div className="agent-generation-evidence">
      {Array.isArray(packet.warnings)&&packet.warnings.length>0&&<div><p className="agent-helper">Warnings</p><ul>{packet.warnings.map((warning,index)=><li key={index}>{String(warning)}</li>)}</ul></div>}
      {provenanceKnown&&articleDocuments.length>0&&<p className="agent-helper">Article provenance: {freshArticles} fetched this run, {cachedArticles} from cache.</p>}
      {researchSourceList.length>0&&<div><p className="agent-helper">Retrieved sources</p><ol className="agent-source-list">{researchSourceList.map(source=><li key={source.number} value={source.number} id={`research-source-${source.number}`} tabIndex={0} aria-label={`Source ${source.number}: ${source.title}`}><span className="agent-source-number">{source.number}.</span>{source.url.startsWith('sthira://artifact/')?<button type="button" className="agent-link" disabled={pending} onClick={()=>{onClose();onOpenArtifact(source.url.slice('sthira://artifact/'.length));}}>{source.title}</button>:/^https?:\/\//i.test(source.url)?<a href={source.url} target="_blank" rel="noreferrer">{source.title}</a>:source.title}</li>)}</ol></div>}
      {Array.isArray(packet.stages)&&packet.stages.length>0&&<details><summary>Run stages and grounding</summary><ol>{packet.stages.map((stage,index)=>{const entry=stage as {message?:string};return <li key={index}>{String(entry.message||'')}</li>;})}</ol></details>}
      {planGeneration&&(packet.source as {artifact_id?:string}|null)?.artifact_id&&<button type="button" className="agent-link" disabled={pending} onClick={()=>{onClose();onOpenArtifact(String((packet.source as {artifact_id:string}).artifact_id));}}>Open saved source page</button>}
    </div>}
    {naming&&<fieldset className="agent-title-choices" disabled={pending}><legend>Choose a title</legend>{action.titles.map(value=><label key={value}><input type="radio" name={`agent-title-${step.id}`} checked={title===value} value={value} onChange={()=>setTitle(value)}/><span>{value}</span></label>)}</fieldset>}
    {memoryChange&&<div><p className="agent-helper">{sourceEntries.length?'Captured source pages':'No source pages attached; review this as an unsupported memory contribution.'}</p>{sourceEntries.map(source=><button type="button" className="agent-link" key={source.artifact_id} disabled={pending} onClick={()=>{onClose();onOpenArtifact(source.artifact_id);}}>{source.title}</button>)}</div>}
  </DocumentEditReview>;
}
