import { HelpTopicButton } from '../components/HelpTopicButton';
import { ContextInfo } from '../components/ContextInfo';
import { Fragment, useCallback, useEffect, useId, useRef, useState } from 'react';
import { ArrowUpRight, ChevronRight, Compass, Plus, Square, Check, Search, FileText } from 'lucide-react';
import ReactMarkdown, { defaultUrlTransform } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { getArtifactContext, getProviderCatalog, getWorkspaceTree, getSearchProviders, isTauri } from '../api/ipc';
import { createAgentRun, followUpAgentRun, openAgentTasks, executeAgentRun, listAgentRuns, getAgentRun, cancelAgentRun, reviewAgentStep, undoAgentEdit, isAgentActive, isOrganizationProposal, isProjectProposal, type AgentRun, type AgentStep } from '../api/workspaceAgent';
import { listSynthesisTemplates, openTemplateManager, type SynthesisTemplate } from '../api/synthesisTemplates';
import { useAgentTasks } from '../context/AgentTasksContext';
import { getEnabledModels, openWorkflowSettings } from '../api/workflowSettings';
import { useWorkflowSettings } from '../hooks/useWorkflowSettings';
import { DocumentEditReview } from '../components/DocumentEditReview';
import { memoryAction, type MemoryEntry } from '../api/sharedMemory';
import { AgentHandoffs } from '../components/AgentHandoffs';
import { AgentProjectReview } from '../components/AgentProjectReview';
import { AgentOrganizationReview } from '../components/AgentOrganizationReview';
import { AgentWorkflowReview } from '../components/AgentWorkflowReview';
import type { ProviderEntry, WorkspaceTreeProject } from '../types/sthira';
import '../styles/workspace-agent.css';
interface Props {initialRunId?:string|null;onOpenArtifact:(id:string,projectId?:string|null)=>void;onOpenProject?:(id:string)=>void;registerBeforeLeave?:(guard:(()=>Promise<boolean>)|null)=>void}
const message=(value:unknown)=>value instanceof Error?value.message:String(value);
const statusLabel:Record<string,string>={queued:'Ready to start',running:'Working',review:'Ready for review',completed:'Finished',failed:'Stopped with an error',cancelled:'Stopped by you',interrupted:'Interrupted'};
const workflowLabels:Record<string,string>={propose_project_create:'New project proposal',propose_project_purpose:'Project direction proposal',propose_move:'Page move proposal',search_projects:'Search projects',read_project:'Read project overview',read_connections:'Read page references',propose_membership:'Project membership proposal',propose_connection:'Page reference proposal',propose_refine:'Refine document',propose_derive:'Derive a new page',propose_titles:'Title suggestions',list_templates:'List Research/Plan templates',read_template:'Read template structure',propose_research:'Research proposal',propose_plan:'Plan proposal',read_memory:'Read shared memory',propose_memory_create:'New shared memory',propose_memory_update:'Shared memory revision'};
const actionLabel=(step:AgentStep)=>workflowLabels[step.action.type] || (step.action.type==='search'?`Search · ${step.action.query}`:step.action.type==='read_page'?'Read saved page':step.action.type==='propose_create'?`New page · ${step.action.title}`:step.action.type==='propose_edit'?'Suggested document edit':'Summary and handoff');
function Prose({children,onOpen,onOpenProject}:{children:string;onOpen:(id:string)=>void;onOpenProject?:(id:string)=>void}) {return <div className="agent-prose"><ReactMarkdown remarkPlugins={[remarkGfm]} urlTransform={url=>url.startsWith('sthira://artifact/')||url.startsWith('sthira://project/')?url:defaultUrlTransform(url)} components={{a:({href,children})=>href?.startsWith('sthira://artifact/')?<button type="button" className="agent-link" onClick={()=>onOpen(href.slice('sthira://artifact/'.length))}>{children}</button>:href?.startsWith('sthira://project/')?<button type="button" className="agent-link" disabled={!onOpenProject} onClick={()=>onOpenProject?.(href.slice('sthira://project/'.length))}>{children}</button>:<a href={href} target="_blank" rel="noreferrer">{children}</a>}}>{children}</ReactMarkdown></div>;}

const goalsPerPage = 8;
const stepStatus: Record<string, string> = { done: 'Completed', pending: 'Pending review', applied: 'Applied', dismissed: 'Dismissed', reverted: 'Reverted' };

function RecentGoals({ runs, selectedId, busy, onChoose }: {
  runs: AgentRun[]; selectedId?: string; busy: boolean; onChoose: (run: AgentRun) => void;
}) {
  const [page, setPage] = useState(0);
  const pageCount = Math.ceil(runs.length / goalsPerPage);
  const currentPage = Math.min(page, pageCount - 1);
  const start = currentPage * goalsPerPage;
  return <>
    <table className="agent-table agent-goals-table" aria-label="Recent goals">
      <thead><tr><th scope="col">Goal</th><th scope="col">Status</th></tr></thead>
      <tbody>{runs.slice(start, start + goalsPerPage).map(run => <tr key={run.id}
        className="agent-table-selectable" data-selected={run.id === selectedId}
        onClick={event => { if (!busy && !(event.target as Element).closest('button')) onChoose(run); }}>
        <th scope="row"><button type="button" className="agent-table-trigger" aria-current={run.id === selectedId ? 'true' : undefined}
          disabled={busy} onClick={() => onChoose(run)} title={run.goal}>{run.goal}</button></th>
        <td>{statusLabel[run.status] || run.status}</td>
      </tr>)}</tbody>
    </table>
    {pageCount > 1 && <nav className="agent-history-pagination" aria-label="Recent goals pages">
      <button type="button" className="agent-secondary" disabled={currentPage === 0} onClick={() => setPage(currentPage - 1)}>Previous</button>
      <span aria-live="polite">{currentPage + 1} / {pageCount}</span>
      <button type="button" className="agent-secondary" disabled={currentPage + 1 === pageCount} onClick={() => setPage(currentPage + 1)}>Next</button>
    </nav>}
  </>;
}

function RecordedActions({ run, busy, onOpenArtifact, onOpenProject, onReview, onDismiss, onUndo }: {
  run: AgentRun; busy: boolean; onOpenArtifact: Props['onOpenArtifact']; onOpenProject?: Props['onOpenProject'];
  onReview: (step: AgentStep) => void; onDismiss: (step: AgentStep) => void; onUndo: (step: AgentStep) => void;
}) {
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const tableId = useId();
  const openPage = (id: string) => onOpenArtifact(id, run.project_id);
  if (!run.steps.length) return <p>No tool activity recorded yet.</p>;
  return <div className="agent-actions-scroll">
    <table className="agent-table agent-actions-table" aria-label="Recorded actions">
      <thead><tr><th scope="col">Step</th><th scope="col">Activity</th><th scope="col">Status</th><th scope="col">Actions</th></tr></thead>
      <tbody>{run.steps.map(step => {
        const expanded = expandedId === step.id;
        const detailsId = `${tableId}-${step.id}`;
        const toggle = () => setExpandedId(expanded ? null : step.id);
        const openProjectId = step.state === 'applied' && isProjectProposal(step.action) && typeof step.result.project_id === 'string' ? step.result.project_id : null;
        const pageProject = step.action.type === 'propose_move' ? (step.state === 'applied' ? step.action.to_project_id : step.action.from_project_id) : run.project_id;
        return <Fragment key={step.id}>
          <tr className="agent-table-selectable" data-selected={expanded}
            onClick={event => {
              if ((event.target as Element).closest('button')) return;
              event.currentTarget.querySelector<HTMLButtonElement>('th button')?.focus();
              toggle();
            }}>
            <td>{step.sequence}</td>
            <th scope="row"><button type="button" className="agent-table-trigger" aria-expanded={expanded} aria-controls={detailsId} onClick={toggle}>
              <ChevronRight size={15} aria-hidden="true" /><span>{actionLabel(step)}</span>
            </button></th>
            <td>{step.result.error ? 'Tool error' : stepStatus[step.state] ?? step.state}</td>
            <td><div className="agent-row-actions">
              {step.state === 'pending' && <>
                <button className="agent-primary" disabled={busy || !['review', 'completed'].includes(run.status)} onClick={() => onReview(step)}>Review proposal</button>
                <button className="agent-secondary" disabled={busy} onClick={() => onDismiss(step)}>Dismiss proposal</button>
              </>}
              {step.artifact_id && <button className="agent-link" disabled={busy} onClick={() => onOpenArtifact(step.artifact_id!, pageProject)}>Open page <ArrowUpRight size={14}/></button>}
              {openProjectId && onOpenProject && <button className="agent-link" disabled={busy} onClick={() => onOpenProject(openProjectId)}>Open project <ArrowUpRight size={14}/></button>}
              {step.state === 'applied' && step.change_set_id && <button className="agent-secondary" disabled={busy} onClick={() => onUndo(step)}>
                {isProjectProposal(step.action) ? 'Undo project change' : isOrganizationProposal(step.action) ? 'Undo organization change' : 'Undo applied edit'}
              </button>}
            </div></td>
          </tr>
          <tr id={detailsId} className="agent-action-details" hidden={!expanded}><td colSpan={4}>
            {expanded && <ActivityDetails step={step} onOpen={openPage} onOpenProject={onOpenProject} />}
          </td></tr>
        </Fragment>;
      })}</tbody>
    </table>
  </div>;
}

function ActivityDetails({ step, onOpen, onOpenProject }: {step: AgentStep; onOpen: (id: string) => void; onOpenProject?: Props['onOpenProject']}) {
  const { action, result } = step;
  const pages = action.type === 'search' && Array.isArray(result.saved_pages) ? result.saved_pages as { artifact_id: string; title: string }[] : null;
  const prose = typeof result.content_md === 'string' ? result.content_md
    : 'content_md' in action ? action.content_md
    : 'reason' in action ? action.reason
    : action.type === 'finish' ? action.summary : null;
  return <>
    {Boolean(result.error) && <p role="alert">{String(result.error)}</p>}
    {Boolean(result.mirror_warning) && <p role="alert">Page saved with a mirror warning: {String(result.mirror_warning)}</p>}
    {typeof result.title === 'string' && <p><strong>{result.title}</strong></p>}
    {result.truncated === true && <p>Showing the saved excerpt.</p>}
    {prose && <Prose onOpen={onOpen} onOpenProject={onOpenProject}>{prose}</Prose>}
    {pages && (pages.length ? <ul aria-label="Matching pages">{pages.map(page => <li key={page.artifact_id}>
      <button type="button" className="agent-link" onClick={() => onOpen(page.artifact_id)}>{page.title}</button>
    </li>)}</ul> : <p>No saved pages matched.</p>)}
    <details open={!prose && !pages && !result.error}><summary>Inspect saved action and result</summary>
      <pre>{JSON.stringify({ action, result }, null, 2)}</pre>
    </details>
  </>;
}

const latest=(previous:AgentRun|undefined,incoming:AgentRun)=>previous&&((previous.revision!==undefined&&incoming.revision!==undefined&&previous.revision>incoming.revision)||(previous.status==='cancelled'&&incoming.status!=='cancelled')||(!isAgentActive(previous)&&isAgentActive(incoming))||(previous.status==='running'&&incoming.status==='queued')||(previous.status==='running'&&incoming.status==='running'&&previous.steps.length>incoming.steps.length))?previous:incoming;
export function AgentView({initialRunId,onOpenArtifact,onOpenProject,registerBeforeLeave}:Props) {
  const [runs,setRuns]=useState<AgentRun[]>([]),[selected,setSelected]=useState<AgentRun|null>(null);
  const [handoffDraft,setHandoffDraft]=useState({dirty:false,pending:false});
  const handoffDraftChanged=useCallback((dirty:boolean,pending:boolean)=>setHandoffDraft(previous=>previous.dirty===dirty&&previous.pending===pending?previous:{dirty,pending}),[]);
  const [followup,setFollowup]=useState(''),[followupModel,setFollowupModel]=useState('');
  const tasks=useAgentTasks();
  const [goal,setGoal]=useState(''),[project,setProject]=useState(''),[workspaceMemory,setWorkspaceMemory]=useState(false),[model,setModel]=useState('');
  const [allowWeb,setAllowWeb]=useState(false);
  const [researchTemplateMode,setResearchTemplateMode]=useState<'inherit'|'none'|'explicit'>('inherit');
  const [researchTemplateId,setResearchTemplateId]=useState('');
  const [planTemplateMode,setPlanTemplateMode]=useState<'inherit'|'none'|'explicit'>('inherit');
  const [planTemplateId,setPlanTemplateId]=useState('');
  const [templates,setTemplates]=useState<SynthesisTemplate[]>([]);
  const [searchProviders,setSearchProviders]=useState<{id:string;display_name:string;is_enabled:boolean;is_default:boolean}[]>([]);
  const [providers,setProviders]=useState<ProviderEntry[]>([]),[projects,setProjects]=useState<WorkspaceTreeProject[]>([]);
  const [loading,setLoading]=useState(true),[loadError,setLoadError]=useState(''),[error,setError]=useState(''),[busy,setBusy]=useState(false),[reload,setReload]=useState(0);
  const [review,setReview]=useState<{step:AgentStep;current:string;stale:boolean;memory?:MemoryEntry}|null>(null);
  const workflows=useWorkflowSettings(),alive=useRef(true),selection=useRef(0),selectedId=useRef<string|null>(null);
  const models=getEnabledModels(providers);
  const chosen=model || workflows.settings.agent?.model_id || models.find(m=>m.is_default)?.id || '';
  const active=isAgentActive(selected);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;selection.current++;};},[]);
  const accept=useCallback((run:AgentRun)=>{
    if(!alive.current)return;
    setRuns(values=>{
      const previous = values.find(value => value.id === run.id);
      const updated = previous ? values.map(value => value.id === run.id ? latest(value, run) : value) : [run, ...values];
      return updated.sort((a, b) => Number(isAgentActive(b)) - Number(isAgentActive(a)) || b.updated_at - a.updated_at).slice(0, 50);
    });
    if(selectedId.current===run.id)setSelected(previous=>latest(previous||undefined,run));
  },[]);
  useEffect(()=>{let current=true;setLoading(true);setLoadError('');Promise.all([listAgentRuns(),getProviderCatalog(),getWorkspaceTree(),listSynthesisTemplates().catch(()=>[] as SynthesisTemplate[]),getSearchProviders().catch(()=>[])]).then(([history,catalog,tree,synthesisTemplates,searches])=>{if(current){setRuns(history);setProviders(catalog);setProjects(tree.projects);setTemplates(synthesisTemplates);setSearchProviders(searches);}}).catch(e=>{if(current)setLoadError(message(e));}).finally(()=>{if(current)setLoading(false);});return()=>{current=false;};},[reload,workflows.settings.agent?.enabled,workflows.settings.agent?.model_id]);
  useEffect(()=>{
    if(!selected || !active || !isTauri)return;
    let current=true,inflight=false;const id=selected.id;
    const tick=async()=>{if(inflight)return;inflight=true;try{const result=await getAgentRun(id);if(current)accept(result);}catch(e){if(current)setError(message(e));}finally{inflight=false;}};
    const timer=window.setInterval(()=>{void tick();},1200);
    return()=>{current=false;window.clearInterval(timer);};
  },[selected?.id,active,accept]);
  useEffect(()=>{registerBeforeLeave?.(async()=>{if(busy || handoffDraft.pending || handoffDraft.dirty || (!selected&&goal.trim()) || followup.trim()){setError(busy||handoffDraft.pending?'Wait for the current action to finish.':handoffDraft.dirty?'Save or discard your handoff draft before leaving.':'Send or clear your unsent goal before leaving.');return false;}return true;});return()=>registerBeforeLeave?.(null);},[busy,goal,followup,selected,registerBeforeLeave,handoffDraft]);
  const maySwitch=()=>{if(busy||handoffDraft.pending||handoffDraft.dirty||(!selected&&goal.trim())||followup.trim()){setError(handoffDraft.dirty?'Save or discard your handoff draft before switching.':'Finish the current action or send or clear your unsent goal before switching.');return false;}selection.current++;setReview(null);setError('');return true;};
  const choose=async(run:AgentRun)=>{if(!maySwitch())return;const epoch=selection.current;selectedId.current=run.id;setSelected(run);setBusy(true);try{const current=await getAgentRun(run.id);if(alive.current&&epoch===selection.current)accept(current);}catch(e){if(alive.current&&epoch===selection.current)setError(message(e));}finally{if(alive.current)setBusy(false);}};
  useEffect(()=>{if(!initialRunId)return;let current=true;selection.current++;const epoch=selection.current;selectedId.current=initialRunId;setBusy(true);setError('');getAgentRun(initialRunId).then(run=>{if(current&&epoch===selection.current)accept(run);}).catch(e=>{if(current&&epoch===selection.current)setError(message(e));}).finally(()=>{if(current)setBusy(false);});return()=>{current=false;};},[initialRunId,accept]);
  useEffect(()=>{let current=true;void listAgentRuns().then(history=>{if(current)setRuns(previous=>history.map(run=>latest(previous.find(value=>value.id===run.id),run)));}).catch(()=>{});if(selectedId.current){const id=selectedId.current;void getAgentRun(id).then(run=>{if(current)accept(run);}).catch(()=>{});}return()=>{current=false;};},[tasks.revision,accept]);
  const openSavedRun=async(id:string)=>{if(!maySwitch())return;const epoch=selection.current;setBusy(true);try{const run=await getAgentRun(id);if(alive.current&&epoch===selection.current){selectedId.current=run.id;setSelected(run);accept(run);}}catch(e){if(alive.current&&epoch===selection.current)setError(message(e));}finally{if(alive.current)setBusy(false);}};
  const follow=async()=>{if(!selected||busy||active||!followup.trim())return;setBusy(true);setError('');try{const run=await followUpAgentRun(selected.id,followup.trim(),followupModel||selected.model_id);if(!alive.current)return;selectedId.current=run.id;setSelected(run);accept(run);setFollowup('');setFollowupModel('');}catch(e){if(alive.current)setError(message(e));}finally{if(alive.current)setBusy(false);}};
  const start=async()=>{
    if(busy||active||!goal.trim())return;
    const enabledSearch=searchProviders.filter(provider=>provider.is_enabled);
    if(allowWeb&&!enabledSearch.length){setError('Web research is allowed, but no enabled search provider is configured. Configure one in Settings → Web Search or turn web research off.');return;}
    if(researchTemplateMode==='explicit'&&!researchTemplateId){setError('Choose a Research template, or switch that tool to inherit or no template.');return;}
    if(planTemplateMode==='explicit'&&!planTemplateId){setError('Choose a Plan template, or switch that tool to inherit or no template.');return;}
    setBusy(true);setError('');const epoch=selection.current;
    try {
      const run=await createAgentRun({goal:goal.trim(),project_id:project||null,workspace_memory:workspaceMemory,model_id:chosen,allow_web:allowWeb,research_template_mode:researchTemplateMode,research_template_id:researchTemplateMode==='explicit'?researchTemplateId:null,plan_template_mode:planTemplateMode,plan_template_id:planTemplateMode==='explicit'?planTemplateId:null});
      if(!alive.current||epoch!==selection.current)return;
      selectedId.current=run.id;setSelected(run);setGoal('');accept(run);setBusy(false);
      try{accept(await executeAgentRun(run.id));}catch(e){if(alive.current&&selectedId.current===run.id){setError(message(e));try{accept(await getAgentRun(run.id));}catch{/* The saved run remains inspectable after reconnecting. */}}}
    }catch(e){if(alive.current)setError(message(e));}finally{if(alive.current)setBusy(false);}
  };
  const stop=async()=>{if(!selected||busy)return;setBusy(true);setError('');try{accept(await cancelAgentRun(selected.id));}catch(e){if(alive.current)setError(message(e));}finally{if(alive.current)setBusy(false);}};
  const refreshProjects=async()=>{try{const tree=await getWorkspaceTree();if(alive.current)setProjects(tree.projects);}catch{/* The committed result remains successful; Refresh setup can reconnect. */}};
  const decide=async(step:AgentStep,apply:boolean,title?:string)=>{if(!selected||busy)return;setBusy(true);setError('');try{accept(await (title?reviewAgentStep(selected.id,step.id,apply,title):reviewAgentStep(selected.id,step.id,apply)));setReview(null);if(apply&&isProjectProposal(step.action))await refreshProjects();}catch(e){if(alive.current)setError(message(e));}finally{if(alive.current)setBusy(false);}};
  const inspect=async(step:AgentStep)=>{
    if(!selected||busy)return;
    setBusy(true);setError('');const epoch=selection.current;
    try {
      let current='',stale=false,memory:MemoryEntry|undefined;
      if(step.action.type==='propose_memory_update') {
        const result=await memoryAction({type:'read',id:step.action.id});
        if(result.kind!=='entry')throw new Error('Memory unavailable.');
        memory=result.value;current=memory.body;
        stale=memory.version!==step.action.expected_version || memory.archived || memory.scope_available===false || memory.sources.some(s=>s.status!=='current');
      } else if(step.artifact_id&&!isOrganizationProposal(step.action)&&!isProjectProposal(step.action)) {
        const {artifact}=await getArtifactContext(step.artifact_id);current=artifact.content_md;
        if(step.action.type==='propose_edit')stale=current!==step.basis_content_md;
        else if(step.result.basis) {
          const basis=step.result.basis as {version:number;title:string|null;content_hash:string};
          stale=artifact.version!==basis.version || artifact.title!==basis.title || artifact.content_hash!==basis.content_hash;
        }
      }
      if(alive.current&&epoch===selection.current)setReview({step,current,stale,memory});
    }catch(e){if(alive.current&&epoch===selection.current)setError(message(e));}finally{if(alive.current)setBusy(false);}
  };
  const undo=async(step:AgentStep)=>{if(!selected||busy)return;setBusy(true);setError('');try{accept(await undoAgentEdit(selected.id,step.id));if(isProjectProposal(step.action))await refreshProjects();}catch(e){if(alive.current)setError(message(e));}finally{if(alive.current)setBusy(false);}};
  return <section className="agent-view earth-workspace" aria-labelledby="agent-title">
    <header className="agent-header"><div className="context-info-heading"><h1 id="agent-title">Workspace agent</h1><ContextInfo title="Workspace agent">Give the agent a goal and choose which pages it can use. Proposed changes wait for your review.</ContextInfo></div><button className="agent-secondary" disabled={busy} onClick={()=>{if(maySwitch()){selectedId.current=null;setSelected(null);}}}><Plus size={16}/> New goal</button></header>
    {!isTauri&&<p className="agent-banner">Agent runs require the desktop app and a connected model.</p>}
    {loading&&<p role="status">Loading workspace and saved runs…</p>}
    {loadError&&<p role="alert">{loadError} <button onClick={()=>setReload(n=>n+1)}>Retry loading</button></p>}
    {error&&<p role="alert" className="agent-error">{error}</p>}
    <div className="agent-layout"><aside className="agent-runs" aria-label="Saved agent runs"><h2>Recent goals</h2>{!loading&&!runs.length&&<div className="agent-history-empty"><Compass size={24}/><p>Your goals and their handoffs will stay here.</p></div>}{runs.length>0&&<RecentGoals runs={runs} selectedId={selected?.id} busy={busy} onChoose={run=>void choose(run)}/>}</aside>
      <div className="agent-main">{!selected?<>
        <form className="agent-goal-form" onSubmit={e=>{e.preventDefault();void start();}}><label>Your goal<textarea aria-label="Agent goal" rows={5} maxLength={8000} value={goal} onChange={e=>setGoal(e.target.value)} placeholder="Review my reading notes and propose a page outlining the unresolved questions…" disabled={busy}/></label>
          <div className="agent-options"><label>Work within<select aria-label="Agent scope" value={project} disabled={busy} onChange={e=>setProject(e.target.value)}><option value="">Entire workspace</option>{projects.map(p=><option key={p.id} value={p.id}>{p.name}</option>)}</select></label><label>Model<select aria-label="Agent model" value={chosen} disabled={busy} onChange={e=>setModel(e.target.value)}><option value="">Choose a configured model</option>{models.map(m=><option key={m.id} value={m.id}>{m.display_name} · {providers.find(p=>p.id===m.provider_id)?.display_name}</option>)}</select></label></div>
          <div className="context-info-heading"><label className="agent-check"><input type="checkbox" checked={workspaceMemory} disabled={busy} onChange={e=>setWorkspaceMemory(e.target.checked)}/> Include workspace-wide memory</label><ContextInfo title="agent scope and memory">Project scope includes the project's attached pages and memory. Entire workspace includes saved pages and project memories. Workspace-wide memory requires the checkbox. Follow-ups inherit this scope.</ContextInfo></div>
          <p className="agent-helper">Selected context is sent to your chosen model.</p>
          <details className="agent-tools">
            <summary>Research &amp; Plan tools (optional)</summary>
            <div className="agent-tools-body">
              <div className="context-info-heading"><label className="agent-check"><input type="checkbox" aria-label="Allow web research" data-testid="agent-allow-web" checked={allowWeb} disabled={busy} onChange={e=>setAllowWeb(e.target.checked)}/> Allow web research for this request</label><ContextInfo title="web research">Search providers receive search terms. The selected model receives the approved context. Follow-ups inherit this grant.</ContextInfo></div>
              {allowWeb && <p className="agent-helper">{searchProviders.find(provider=>provider.is_enabled)?.display_name ? `Search provider: ${searchProviders.find(provider=>provider.is_enabled)?.display_name}.` : 'No enabled search provider is configured yet.'}</p>}
              <div className="agent-options">
                <label>Research template
                  <select aria-label="Agent research template" data-testid="agent-research-template" value={researchTemplateMode==='explicit'?researchTemplateId:researchTemplateMode} disabled={busy}
                    onChange={e=>{const value=e.target.value;if(value==='inherit'||value==='none'){setResearchTemplateMode(value);setResearchTemplateId('');}else{setResearchTemplateMode('explicit');setResearchTemplateId(value);}}}>
                    <option value="inherit">Inherit workflow default</option>
                    <option value="none">No template</option>
                    {templates.filter(template=>template.target_mode==='research').map(template=><option key={template.id} value={template.id}>{template.name} · r{template.version}</option>)}
                  </select>
                  {!workflows.settings.research?.enabled && <small>Research is disabled; this tool will refuse until you enable it.</small>}
                  {workflows.settings.research?.enabled && !workflows.settings.research?.templates_enabled && <small>Templates are off for Research; runs use no template.</small>}
                </label>
                <label>Plan template
                  <select aria-label="Agent plan template" data-testid="agent-plan-template" value={planTemplateMode==='explicit'?planTemplateId:planTemplateMode} disabled={busy}
                    onChange={e=>{const value=e.target.value;if(value==='inherit'||value==='none'){setPlanTemplateMode(value);setPlanTemplateId('');}else{setPlanTemplateMode('explicit');setPlanTemplateId(value);}}}>
                    <option value="inherit">Inherit workflow default</option>
                    <option value="none">No template</option>
                    {templates.filter(template=>template.target_mode==='plan').map(template=><option key={template.id} value={template.id}>{template.name} · r{template.version}</option>)}
                  </select>
                  {!workflows.settings.plan?.enabled && <small>Plan is disabled; this tool will refuse until you enable it.</small>}
                  {workflows.settings.plan?.enabled && !workflows.settings.plan?.templates_enabled && <small>Templates are off for Plan; runs use no template.</small>}
                </label>
              </div>
              <div className="agent-form-actions">
                <button type="button" className="agent-secondary" onClick={()=>openTemplateManager('research')}>Manage templates</button>
                <button type="button" className="agent-secondary" onClick={()=>setReload(value=>value+1)} disabled={busy}>Refresh tools</button>
              </div>
            </div>
          </details>
          {(!workflows.settings.agent?.enabled||!chosen||workflows.error)&&<p className="agent-setup">{workflows.error||(!workflows.settings.agent?.enabled?'Enable Workspace agent in Workflows to begin.':'Connect and select an enabled model to begin.')} <button type="button" onClick={openWorkflowSettings}>Open workflow settings <ArrowUpRight size={13}/></button><HelpTopicButton article="troubleshooting" label="Setup help" /></p>}
          <div className="agent-form-actions"><button type="submit" className="agent-primary" disabled={!isTauri||busy||loading||!!loadError||workflows.loading||!!workflows.error||!workflows.settings.agent?.enabled||!models.some(m=>m.id===chosen)||!goal.trim()}>{busy?'Starting…':'Start working'}</button><button type="button" className="agent-secondary" disabled={busy||loading} onClick={()=>setReload(n=>n+1)}>Refresh setup</button>{goal&&<button type="button" className="agent-secondary" disabled={busy} onClick={()=>{setGoal('');setError('');}}>Clear goal</button>}</div>
        </form><div className="agent-boundaries"><span><Search size={16}/> Search and read saved context</span><span><FileText size={16}/> Refine, derive, name, and remember with review</span><span><Check size={16}/> You choose what gets applied</span></div>
      </>:<>
        <header className="agent-run-heading"><div><p className="agent-eyebrow">{statusLabel[selected.status]||selected.status}</p><h2>{selected.goal}</h2><p>{selected.model_name} · {selected.provider_name}</p><p>Web research {selected.allow_web?'allowed':'off'}{selected.allow_web&&selected.search_provider_name?` · ${selected.search_provider_name}`:''} · Research template {selected.research_template_mode==='explicit'?templates.find(t=>t.id===selected.research_template_id)?.name||'selected template':selected.research_template_mode==='none'?'none':'workflow default'} · Plan template {selected.plan_template_mode==='explicit'?templates.find(t=>t.id===selected.plan_template_id)?.name||'selected template':selected.plan_template_mode==='none'?'none':'workflow default'}</p><p>{selected.document_id?'This document':selected.project_id?projects.find(p=>p.id===selected.project_id)?.name||'Original project':'Entire workspace'} · Workspace memory {selected.workspace_memory?'included':'excluded'}</p>{active&&selected.progress_message&&<p className="agent-progress" role="status" data-testid="agent-progress">{selected.progress_message}</p>}</div>{active&&<button className="agent-secondary" disabled={busy} onClick={()=>void stop()}><Square size={14}/> Stop run</button>}</header>
        {selected.error&&<p role="alert" className="agent-error">{selected.error}</p>}{active&&<p role="status">Working with your selected context… {selected.steps.length} of 8 actions recorded.</p>}{active&&<p className="agent-background-note">You can leave this view and keep working. <button type="button" className="agent-link" onClick={openAgentTasks}>Open agent tasks</button></p>}{selected.parent_run_id&&<button type="button" className="agent-thread-link" disabled={busy} onClick={()=>void openSavedRun(selected.parent_run_id!)}>Previous conversation turn</button>}{selected.document_id&&<button type="button" className="agent-link" disabled={busy} onClick={()=>onOpenArtifact(selected.document_id!)}>Open original document</button>}
        {selected.summary&&<section className="agent-result" aria-label="Agent result"><h3>Result</h3><Prose onOpenProject={onOpenProject} onOpen={id=>onOpenArtifact(id,selected?.project_id)}>{selected.summary}</Prose></section>}
        <section className="agent-activity" data-guide-target="agent-activity" aria-label="Agent activity">
          <h3>Activity and proposals</h3>
          <RecordedActions key={selected.id} run={selected} busy={busy} onOpenArtifact={onOpenArtifact} onOpenProject={onOpenProject}
            onReview={step=>void inspect(step)} onDismiss={step=>void decide(step,false)} onUndo={step=>void undo(step)}/>
        </section>
        {selected.handoff&&<section className="agent-handoff" aria-label="Saved handoff"><h3>Carry forward</h3><Prose onOpenProject={onOpenProject} onOpen={id=>onOpenArtifact(id,selected?.project_id)}>{selected.handoff}</Prose><p className="agent-helper">Saved with this run. It does not automatically become shared memory.</p></section>}
        {!active&&!selected.document_id&&<AgentHandoffs key={selected.id} run={selected} revision={tasks.revision} models={models} defaultModel={chosen} enabled={!!workflows.settings.agent?.enabled} blocked={busy||!!followup.trim()} onDraftChange={handoffDraftChanged} onOpenArtifact={id=>onOpenArtifact(id,selected.project_id)} onOpenRun={id=>void openSavedRun(id)} onContinued={run=>{selectedId.current=run.id;setSelected(run);accept(run);setFollowup('');setHandoffDraft({dirty:false,pending:false});}}/>}
        {!active&&<form className="agent-followup" onSubmit={e=>{e.preventDefault();void follow();}}><label>Continue this conversation<textarea aria-label="Follow-up message" rows={3} maxLength={8000} value={followup} disabled={busy} onChange={e=>setFollowup(e.target.value)} placeholder="Use the second approach, with these changes…"/></label><label>Model<select aria-label="Follow-up model" value={followupModel||selected.model_id} disabled={busy} onChange={e=>setFollowupModel(e.target.value)}>{!models.some(m=>m.id===(followupModel||selected.model_id))&&<option value={followupModel||selected.model_id}>{selected.model_name} (unavailable)</option>}{models.map(m=><option key={m.id} value={m.id}>{m.display_name}</option>)}</select></label><ContextInfo title="follow-ups">A follow-up uses this thread's scope and recent context. Each earlier proposal keeps its own review state.</ContextInfo>{selected.document_id&&selected.allow_edits===false&&<p className="agent-helper">This document thread is read-only here. To request changes, open its conversation and enable Suggest a document edit.</p>}{!workflows.settings[selected.document_id?'chat':'agent']?.enabled&&<p className="agent-helper">Enable {selected.document_id?'Document chat':'Workspace agent'} to send another message. <button type="button" onClick={openWorkflowSettings}>Open workflow settings</button></p>}<div className="agent-form-actions"><button type="submit" className="agent-primary" disabled={busy||!isTauri||!followup.trim()||!workflows.settings[selected.document_id?'chat':'agent']?.enabled||!models.some(m=>m.id===(followupModel||selected.model_id))}>Send follow-up</button>{followup&&<button type="button" className="agent-secondary" disabled={busy} onClick={()=>setFollowup('')}>Clear follow-up</button>}</div></form>}
        {selected.status==='queued'&&<button className="agent-secondary" disabled={busy} onClick={()=>void stop()}>Stop queued run</button>}
      </>}</div>
    </div>
    {review&&selected&&['propose_create','propose_edit'].includes(review.step.action.type)&&<DocumentEditReview operation={review.step.action.type==='propose_create'?'create':'edit'} current={review.current} proposed={'content_md' in review.step.action?review.step.action.content_md:''} summary={review.step.action.type==='propose_create'?`Create **${review.step.action.title}** as an AI-authored page${selected.project_id?' in the selected project':''}.`:'Review this agent proposal against the current saved document.'} stale={review.stale} pending={busy} busy={false} error={error||null} onClose={()=>setReview(null)} onApply={()=>void decide(review.step,true)} onDismiss={()=>void decide(review.step,false)}/>}
    {review&&selected&&!['propose_create','propose_edit'].includes(review.step.action.type)&&!isOrganizationProposal(review.step.action)&&!isProjectProposal(review.step.action)&&<AgentWorkflowReview run={selected} step={review.step} current={review.current} memory={review.memory} projectNames={Object.fromEntries(projects.map(project=>[project.id,project.name]))} stale={review.stale} pending={busy} error={error||null} onClose={()=>setReview(null)} onApply={title=>void decide(review.step,true,title)} onDismiss={()=>void decide(review.step,false)} onOpenArtifact={onOpenArtifact}/>}
    {review&&selected&&isOrganizationProposal(review.step.action)&&<AgentOrganizationReview run={selected} step={review.step} pending={busy} error={error||null} onClose={()=>setReview(null)} onApply={()=>void decide(review.step,true)} onDismiss={()=>void decide(review.step,false)} onOpenArtifact={onOpenArtifact}/>}
    {review&&selected&&isProjectProposal(review.step.action)&&<AgentProjectReview run={selected} step={review.step} pending={busy} error={error||null} onClose={()=>setReview(null)} onApply={()=>void decide(review.step,true)} onDismiss={()=>void decide(review.step,false)} onOpenProject={onOpenProject} onOpenArtifact={onOpenArtifact}/>}
  </section>;
}
