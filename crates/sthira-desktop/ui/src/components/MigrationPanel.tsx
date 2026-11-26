import {useCallback,useEffect,useRef,useState} from 'react';
import {getWorkspaceTree} from '../api/ipc';
import {MigrationGuidancePanel} from './MigrationGuidancePanel';
import {previewMigration,listMigrations,getMigration,reviewMigrationSelection,applyMigration,cancelMigration,rollbackMigration,type ImportRun,type ImportFile,type ImportDraft,type ImportHistory,type HistoryCursor,type ImportSelection} from '../api/migration';
import '../styles/migration.css';
export function MigrationPanel({onChanged}:{onChanged:()=>void}) {
 const [source,setSource]=useState('notion'),[collection,setCollection]=useState(''),[project,setProject]=useState(''),[files,setFiles]=useState<ImportFile[]>([]),[projects,setProjects]=useState<{id:string;name:string}[]>([]);
 const [history,setHistory]=useState<ImportHistory>({items:[],next_cursor:null}),[cursors,setCursors]=useState<(HistoryCursor|null)[]>([null]),[historyBusy,setHistoryBusy]=useState(false),[historyError,setHistoryError]=useState('');
 const [run,setRun]=useState<ImportRun|null>(null),[paths,setPaths]=useState<string[]>([]),[review,setReview]=useState<ImportSelection|null>(null),[reviewBusy,setReviewBusy]=useState(false),[reviewError,setReviewError]=useState('');
 const [busy,setBusy]=useState(false),[guidanceActive,setGuidanceActive]=useState(false),[error,setError]=useState(''),[notice,setNotice]=useState('');
 const alive=useRef(true),epoch=useRef(0),historyEpoch=useRef(0),draft=useRef<ImportDraft|null>(null),input=useRef<HTMLInputElement>(null);
 const page=cursors.length-1,cursor=cursors[page],selectedKey=JSON.stringify([...paths].sort());
 const currentReview=review&&review.id===run?.id&&review.revision===run?.revision&&JSON.stringify([...review.paths].sort())===selectedKey?review:null;
 const loadHistory=useCallback(async(before:HistoryCursor|null)=>{
  const ticket=++historyEpoch.current;setHistoryBusy(true);setHistoryError('');
  try{const value=await listMigrations(before);if(alive.current&&ticket===historyEpoch.current)setHistory(value);}catch(e){if(alive.current&&ticket===historyEpoch.current)setHistoryError(String(e));}finally{if(alive.current&&ticket===historyEpoch.current)setHistoryBusy(false);}
 },[]);
 useEffect(()=>{alive.current=true;void getWorkspaceTree().then(tree=>{if(alive.current)setProjects(tree.projects);}).catch(e=>{if(alive.current)setError(String(e));});return()=>{alive.current=false;epoch.current++;historyEpoch.current++;};},[]);
 useEffect(()=>{void loadHistory(cursor);},[cursor,loadHistory]);
 useEffect(()=>{
  let current=true;setReview(null);setReviewError('');
  if(run?.status!=='preview'){setReviewBusy(false);return;}
  setReviewBusy(true);void reviewMigrationSelection(run,JSON.parse(selectedKey) as string[]).then(value=>{if(current)setReview(value);}).catch(e=>{if(current)setReviewError(String(e));}).finally(()=>{if(current)setReviewBusy(false);});
  return()=>{current=false;};
 },[run?.id,run?.revision,run?.status,selectedKey]);
 const clearPreview=()=>{epoch.current++;setRun(null);draft.current=null;setNotice('');};
 const accept=(value:ImportRun)=>{epoch.current++;setRun(value);setReview(null);setPaths(value.receipt?.selected||value.plan.items.filter(i=>i.status==='new').map(i=>i.path));};
 const choose=async(selected:FileList|null)=>{
  if(!selected)return;clearPreview();setBusy(true);setError('');setFiles([]);const ticket=epoch.current;
  try{if(selected.length>200)throw new Error('Choose at most 200 files per batch.');let bytes=0;const entries:ImportFile[]=[];
   for(const file of Array.from(selected)){const path=file.webkitRelativePath||file.name;if(!/\.(md|markdown|txt)$/i.test(path)||file.size>262144){entries.push({path,content:null,problem:'Unsupported format or file above 256 KiB. Extract ZIP exports and choose Markdown/text; attachments, CSV, HTML and DOCX need a separate workflow.'});continue;}bytes+=file.size;if(bytes>4*1024*1024)throw new Error('Selection exceeds 4 MiB of text. Split the export into smaller batches.');try{entries.push({path,content:new TextDecoder('utf-8',{fatal:true}).decode(await file.arrayBuffer())});}catch{entries.push({path,content:null,problem:'File is not valid UTF-8 text.'});}}
   if(alive.current&&ticket===epoch.current)setFiles(entries);
  }catch(e){if(alive.current&&ticket===epoch.current)setError(String(e));}finally{if(alive.current)setBusy(false);}
 };
 const preview=async()=>{
  setBusy(true);setError('');setNotice('');const ticket=epoch.current,request=draft.current??{request_id:crypto.randomUUID(),source,collection,project_id:project||null,files};draft.current=request;
  try{const value=await previewMigration(request);if(alive.current&&ticket===epoch.current){accept(value);void loadHistory(cursor);}}catch(e){if(alive.current&&ticket===epoch.current)setError(String(e));}finally{if(alive.current)setBusy(false);}
 };
 const openBatch=async(id:string)=>{const ticket=++epoch.current;setBusy(true);setError('');setNotice('');try{const value=await getMigration(id);if(alive.current&&ticket===epoch.current)accept(value);}catch(e){if(alive.current&&ticket===epoch.current)setError(String(e));}finally{if(alive.current)setBusy(false);}};
 const decide=async(kind:'apply'|'cancel'|'rollback')=>{
  if(!run||(kind==='apply'&&(!currentReview||!paths.length)))return;const ticket=epoch.current;setBusy(true);setError('');setNotice('');
  try{const value=await(kind==='apply'?applyMigration(run,currentReview!):kind==='cancel'?cancelMigration(run):rollbackMigration(run));if(!alive.current||ticket!==epoch.current)return;setRun(value);setNotice(value.status==='applied'?'Selected documents are saved with external provenance.':value.status==='rolled_back'?'Unchanged imported documents were removed. Original exports remain intact.':'Preview cancelled. No documents imported.');if(kind!=='cancel')onChanged();void loadHistory(cursor);}catch(e){if(alive.current&&ticket===epoch.current)setError(String(e));}finally{if(alive.current)setBusy(false);}
 };
 return <section className="migration-panel" aria-label="Migration assistant"><header><p className="migration-eyebrow">BRING YOUR KNOWLEDGE WITH YOU</p><h3>Import existing knowledge</h3><p>Review local Notion, Obsidian, Google Docs or other Markdown/text exports. Manual import needs no model. Original files stay untouched.</p></header>
 <form onSubmit={e=>{e.preventDefault();void preview();}}><div className="migration-options"><label>Source preset<select value={source} disabled={busy} onChange={e=>{setSource(e.target.value);clearPreview();}}><option value="notion">Notion · Markdown export</option><option value="obsidian">Obsidian · Markdown notes</option><option value="google_docs">Google Docs · Markdown or plain text</option><option value="markdown">Other Markdown / text</option></select></label><label>Collection identity<input value={collection} maxLength={160} disabled={busy} onChange={e=>{setCollection(e.target.value);clearPreview();}} placeholder="e.g. personal-research-export"/></label><label>Destination<select value={project} disabled={busy} onChange={e=>{setProject(e.target.value);clearPreview();}}><option value="">Standalone pages</option>{projects.map(p=><option key={p.id} value={p.id}>{p.name}</option>)}</select></label></div>
 <p className="migration-helper">Keep the same collection identity and relative file paths for duplicate detection on later batches. Changed source files are reported as conflicts; they never overwrite existing pages.</p><label>Choose exported files<input ref={input} type="file" multiple disabled={busy} onChange={e=>void choose(e.target.files)}/></label><label>Or choose an export folder<input type="file" multiple {...{webkitdirectory:''}} disabled={busy} onChange={e=>void choose(e.target.files)}/></label><p className="migration-helper">UTF-8 .md, .markdown and .txt · up to 200 files, 256 KiB each, 4 MiB total. Extract ZIP exports first. Images, embeds, databases and rich document formats are reported as unsupported.</p>{files.length>0&&<p role="status">{files.length} files selected for inventory.</p>}<button type="submit" className="settings-btn-primary" disabled={busy||!collection.trim()||!files.length}>{busy?'Working…':'Prepare import preview'}</button></form>
 {error&&<p role="alert" className="migration-error">{error}</p>}{notice&&<p role="status" className="migration-notice">{notice}</p>}
 <section aria-label="Saved import history" className="migration-history"><h4>Saved import batches</h4>
  {historyBusy&&<p role="status">Loading batch summaries…</p>}{historyError&&<p role="alert">{historyError}</p>}
  {!historyBusy&&!historyError&&!history.items.length&&<p className="migration-helper">No saved batches on this page.</p>}
  <ul>{!historyBusy&&!historyError&&history.items.map(item=><li key={item.id}><button type="button" className="migration-history-row" aria-pressed={run?.id===item.id} disabled={busy||historyBusy} onClick={()=>void openBatch(item.id)}><span>{item.counts.collection}</span><span>{item.status} · {item.counts.total} files · {item.id.slice(0,8)}</span></button></li>)}</ul>
  <div className="migration-actions"><button type="button" className="settings-btn-secondary" disabled={busy||historyBusy||page===0} onClick={()=>setCursors(value=>value.slice(0,-1))}>Previous batches</button><span>Page {page+1}</span><button type="button" className="settings-btn-secondary" disabled={busy||historyBusy||!!historyError||!history.next_cursor} onClick={()=>setCursors(value=>[...value,history.next_cursor])}>Next batches</button><button type="button" className="settings-btn-secondary" disabled={busy||historyBusy} onClick={()=>void loadHistory(cursor)}>Refresh history</button></div>
 </section>
 {run&&<article aria-label="Import review"><header><h4>{run.plan.collection}</h4><p className="migration-helper">{run.status} · {run.plan.items.filter(i=>i.status==='new').length} new · {run.plan.items.filter(i=>i.status==='duplicate').length} already imported · {run.plan.items.filter(i=>i.status==='conflict').length} conflicts · {run.plan.items.filter(i=>i.status==='unsupported').length} unsupported</p></header>
  <p className="migration-helper">Destination: {projects.find(p=>p.id===run.plan.project_id)?.name||(run.plan.project_id?run.plan.project_id:'Standalone pages')}. Import preserves external authorship. No decisions, shared memories or object properties are inferred.</p>
  {run.status==='preview'&&<div className="migration-actions"><button type="button" className="settings-btn-secondary" disabled={busy} onClick={()=>setPaths(run.plan.items.filter(i=>i.status==='new').map(i=>i.path))}>Select all new files</button><button type="button" className="settings-btn-secondary" disabled={busy} onClick={()=>setPaths([])}>Clear selection</button><span>{paths.length} selected</span></div>}
  {reviewBusy&&<p role="status">Updating the exact selected preview…</p>}{reviewError&&<p role="alert">{reviewError}</p>}
  <ul className="migration-items">{run.plan.items.map(item=>{const proposed=currentReview?.items.find(i=>i.path===item.path)||item,included=paths.includes(item.path);return <li key={item.path}><div className="migration-file-row">
   {item.status==='new'&&<input type="checkbox" aria-label={`Import ${item.path}`} checked={included} disabled={busy||run.status!=='preview'} onChange={e=>setPaths(previous=>e.target.checked?[...previous,item.path]:previous.filter(path=>path!==item.path))}/>}
   <details><summary><span>{item.title}</span><span>{item.status==='new'&&!included?'not selected':item.status}</span></summary><p className="migration-helper">{item.path} · saved identity {item.artifact_id}</p>{proposed.warnings.map((w,n)=><p className="migration-helper" key={n}>{w}</p>)}
    {included&&(run.status!=='preview'||!!currentReview)&&proposed.preview_content!==null&&<><h5>{run.status==='preview'?'Selected document preview':'Imported document preview'}</h5><pre>{proposed.preview_content}</pre></>}{!included&&item.status==='new'&&<p className="migration-helper">This document is outside the current selection.</p>}
   </details></div></li>;})}</ul>
  <MigrationGuidancePanel key={run.id} run={run} locked={busy} selection={currentReview} onActive={setGuidanceActive} onSelection={setPaths}/>
  {run.receipt&&<><p role="status">{run.receipt.pages.length} documents in this import receipt.</p>{run.receipt.pages.flatMap(p=>p.warnings).map((w,n)=><p className="migration-helper" key={n}>{w}</p>)}{run.receipt.mirror_warnings.map(w=><p role="alert" key={w.artifact_id}>Saved in the database; Markdown mirror needs repair: {w.error}</p>)}</>}
  <div className="migration-actions">{run.status==='preview'&&<><button type="button" className="settings-btn-primary" disabled={busy||guidanceActive||reviewBusy||!currentReview||!paths.length} onClick={()=>void decide('apply')}>Import selected documents</button><button type="button" className="settings-btn-secondary" disabled={busy} onClick={()=>void decide('cancel')}>Cancel preview</button></>}{run.status==='applied'&&<button type="button" className="settings-btn-secondary" disabled={busy} onClick={()=>void decide('rollback')}>Roll back unchanged import</button>}</div>
  <p className="migration-helper">Links are recalculated for selected files; omitted or unavailable targets stay literal with warnings. Rollback cannot discard imported work that has been edited or reused by memory, agents or other features.</p>
 </article>}
 </section>;
}
