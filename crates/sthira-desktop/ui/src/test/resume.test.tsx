import { beforeEach,describe,expect,it,vi } from 'vitest';
import { act,fireEvent,render,screen,waitFor,within } from '@testing-library/react';
import { ProjectResume } from '../components/ProjectResume';
import { ResumeSettingsPanel } from '../components/ResumeSettingsPanel';
import type { ResumeBrief } from '../types/sthira';
import type { ResumeSettings } from '../api/resume';

const api=vi.hoisted(()=>({getResumeBrief:vi.fn(),getResumeSettings:vi.fn(),saveResumeSettings:vi.fn(),getResumePackStatus:vi.fn(),installResumePack:vi.fn(),removeResumePack:vi.fn(),generateResumeBrief:vi.fn()}));
vi.mock('../api/ipc',()=>({getResumeBrief:api.getResumeBrief,getProviderCatalog:async()=>[]}));
vi.mock('../api/resume',()=>({...api,getResumeModel:async()=>({available:true,name:'Workspace example',provider_name:'Example connection'}),RESUME_SETTINGS_CHANGED:'sthira:resume-settings-changed',defaultResumeSettings:()=>({enabled:false,preferences:{model_mode:'workspace',model_id:null,local_fallback:true,include_decisions:true,include_tasks:true,include_questions:true,include_recent_changes:true,include_excerpts:true,max_sources:8}})}));
vi.mock('../api/workflowSettings',()=>({openWorkflowSettings:()=>window.dispatchEvent(new CustomEvent('sthira:open-settings',{detail:'workflows'}))}));
const config=():ResumeSettings=>({enabled:true,preferences:{model_mode:'workspace',model_id:null,local_fallback:true,include_decisions:true,include_tasks:true,include_questions:true,include_recent_changes:true,include_excerpts:true,max_sources:8}});
const fact:ResumeBrief={project_id:'p',level:1,purpose:'Review restoration.',recent_progress:[{text:'Backup test has not been completed.',origin:'source_backed',rendering:'deterministic_template',artifact_ids:['note'],evidence_node_ids:[]}],conflicting_decisions:[],active_working_breadcrumb:'Restore review',pane_hint:{collapse:{reason:'active_session'}},primary_continuation_artifact_id:'note',continuation_point:'Continue in Release checklist',generation:{mode:'factual',model_id:null,model_name:null,generated_at:null,notice:null}};
const generated:ResumeBrief={...fact,recent_progress:[{...fact.recent_progress[0],origin:'ai_inferred',rendering:'ai_prose'}],generation:{mode:'generated',model_id:'workspace',model_name:'Selected workspace model',generated_at:1000,notice:null,factual_progress:fact.recent_progress}};
beforeEach(()=>{vi.clearAllMocks();api.getResumeBrief.mockResolvedValue(fact);api.getResumeSettings.mockResolvedValue(config());api.getResumePackStatus.mockResolvedValue({label:'SmolLM2-360M · local preview',ready:false,downloading:false,supported:true,download_bytes:409456950,notice:null});api.saveResumeSettings.mockResolvedValue(undefined);api.generateResumeBrief.mockResolvedValue(generated);});

describe('Resume reading and explicit generation',()=>{
 it('reads facts without a model request and only summarizes when explicitly clicked',async()=>{
  const open=vi.fn();render(<ProjectResume projectId="p" onSelectArtifact={open}/>);
  expect(await screen.findByText('Saved facts')).toBeInTheDocument();await screen.findByTestId('contextual-resume-pane');
  expect(api.generateResumeBrief).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button',{name:'Summarize'}));
  await screen.findByText(/Selected excerpts · Selected workspace model/);
  expect(api.generateResumeBrief).toHaveBeenCalledWith('p');expect(screen.getByText('Saved working state')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button',{name:/Open source 1 for/}));expect(open).toHaveBeenCalledWith('note');
 });
 it('shows the configured model and connection before summarizing',async()=>{render(<ProjectResume projectId="p" onSelectArtifact={vi.fn()}/>);expect(await screen.findByText('Workspace example · Example connection')).toBeInTheDocument();expect(screen.getByLabelText('Resume model')).toHaveValue('workspace');expect(api.generateResumeBrief).not.toHaveBeenCalled();});
 it('changes the Resume model without running a summary',async()=>{render(<ProjectResume projectId="p" onSelectArtifact={vi.fn()}/>);await screen.findByText('Workspace example · Example connection');fireEvent.change(screen.getByLabelText('Resume model'),{target:{value:'local'}});await waitFor(()=>expect(api.saveResumeSettings).toHaveBeenCalledWith(expect.objectContaining({preferences:expect.objectContaining({model_mode:'local',model_id:null})})));expect(api.generateResumeBrief).not.toHaveBeenCalled();});
 it('keeps factual Resume readable when model summaries are disabled',async()=>{
  api.getResumeSettings.mockResolvedValue({...config(),enabled:false});render(<ProjectResume projectId="p" onSelectArtifact={vi.fn()}/>);
  await screen.findByTestId('contextual-resume-pane');expect(screen.getByText('Backup test has not been completed.')).toBeInTheDocument();
  expect(screen.queryByRole('button',{name:'Summarize'})).not.toBeInTheDocument();expect(screen.getByRole('button',{name:'Set up model summaries'})).toBeInTheDocument();
 });
 it('uses a cached brief for expanded review without another inference call',async()=>{
  render(<ProjectResume projectId="p" onSelectArtifact={vi.fn()}/>);await screen.findByTestId('contextual-resume-pane');
  fireEvent.click(screen.getByRole('button',{name:'Expand Review'}));await waitFor(()=>expect(api.getResumeBrief).toHaveBeenCalledWith('p',2));expect(api.generateResumeBrief).not.toHaveBeenCalled();
 });
 it('keeps the current Resume on generation failure and allows retry',async()=>{
  api.generateResumeBrief.mockRejectedValueOnce(new Error('unavailable'));render(<ProjectResume projectId="p" onSelectArtifact={vi.fn()}/>);await screen.findByTestId('contextual-resume-pane');
  fireEvent.click(screen.getByRole('button',{name:'Summarize'}));expect(await screen.findByRole('alert')).toHaveTextContent('available Resume is kept');expect(screen.getByText('Backup test has not been completed.')).toBeInTheDocument();
 });
 it('ignores a late result after moving to another project',async()=>{
  let resolve!:(value:ResumeBrief)=>void;api.generateResumeBrief.mockReturnValueOnce(new Promise(done=>{resolve=done;}));
  const view=render(<ProjectResume projectId="p" onSelectArtifact={vi.fn()}/>);await screen.findByTestId('contextual-resume-pane');fireEvent.click(screen.getByRole('button',{name:'Summarize'}));
  api.getResumeBrief.mockResolvedValue({...fact,project_id:'q',purpose:'Other project'});view.rerender(<ProjectResume projectId="q" onSelectArtifact={vi.fn()}/>);await screen.findByText('Other project');
  await act(async()=>resolve(generated));expect(screen.queryByText(/Selected workspace model/)).not.toBeInTheDocument();expect(screen.getByText('Other project')).toBeInTheDocument();
 });
 it('releases busy controls when Resume settings change during generation',async()=>{
  api.generateResumeBrief.mockReturnValueOnce(new Promise(()=>{}));render(<ProjectResume projectId="p" onSelectArtifact={vi.fn()}/>);await screen.findByTestId('contextual-resume-pane');fireEvent.click(screen.getByRole('button',{name:'Summarize'}));
  await act(async()=>window.dispatchEvent(new Event('sthira:resume-settings-changed')));await waitFor(()=>expect(screen.getByRole('button',{name:'Summarize'})).toBeEnabled());
 });
});

describe('Dedicated Resume settings',()=>{
 it('saves model, source-detail and document limit choices',async()=>{
  render(<ResumeSettingsPanel providers={[]}/>);const select=await screen.findByLabelText('Resume model');
  fireEvent.change(select,{target:{value:'local'}});await waitFor(()=>expect(api.saveResumeSettings).toHaveBeenCalledWith(expect.objectContaining({preferences:expect.objectContaining({model_mode:'local',model_id:null})})));
  await waitFor(()=>expect(screen.getByLabelText('Unfinished checklist items')).toBeEnabled());fireEvent.click(screen.getByLabelText('Unfinished checklist items'));
  await waitFor(()=>expect(api.saveResumeSettings).toHaveBeenLastCalledWith(expect.objectContaining({preferences:expect.objectContaining({include_tasks:false})})));
  await waitFor(()=>expect(screen.getByLabelText('Maximum documents to read')).toBeEnabled());fireEvent.change(screen.getByLabelText('Maximum documents to read'),{target:{value:'4'}});
  await waitFor(()=>expect(api.saveResumeSettings).toHaveBeenLastCalledWith(expect.objectContaining({preferences:expect.objectContaining({max_sources:4})})));
 });
 it('offers an explicit optional download and keeps the factual fallback explained',async()=>{
  api.installResumePack.mockResolvedValue({label:'SmolLM2-360M · local preview',ready:true,downloading:false,supported:true,download_bytes:409456950,notice:null});render(<ResumeSettingsPanel providers={[]}/>);
  await screen.findByLabelText('Resume model');expect(api.installResumePack).not.toHaveBeenCalled();expect(screen.getByText(/Factual Resume is always available/)).toBeInTheDocument();
  fireEvent.click(screen.getByText(/Local Resume pack · Optional download/));fireEvent.click(screen.getByRole('button',{name:'Download local Resume pack'}));await screen.findByRole('button',{name:'Remove local pack'});expect(api.installResumePack).toHaveBeenCalledTimes(1);
 });
 it('retains saved choices when a save fails',async()=>{
  api.saveResumeSettings.mockRejectedValue(new Error('disk full'));render(<ResumeSettingsPanel providers={[]}/>);await screen.findByLabelText('Resume model');fireEvent.change(screen.getByLabelText('Resume model'),{target:{value:'local'}});
  expect(await screen.findByRole('alert')).toHaveTextContent('previous choices are kept');expect(screen.getByLabelText('Resume model')).toHaveValue('workspace');
 });
 it('lets users turn model summaries off without hiding factual Resume',async()=>{
  render(<ResumeSettingsPanel providers={[]}/>);const toggle=await screen.findByRole('checkbox',{name:/Enable model summaries/});fireEvent.click(toggle);
  await waitFor(()=>expect(api.saveResumeSettings).toHaveBeenCalledWith(expect.objectContaining({enabled:false})));expect(screen.queryByLabelText('Resume model')).not.toBeInTheDocument();
  expect(within(screen.getByRole('article')).getByText(/Factual Resume is always available/)).toBeInTheDocument();
  await waitFor(()=>expect(screen.getByLabelText('Unfinished checklist items')).toBeEnabled());
  fireEvent.click(screen.getByLabelText('Unfinished checklist items'));
  await waitFor(()=>expect(api.saveResumeSettings).toHaveBeenLastCalledWith(expect.objectContaining({enabled:false,preferences:expect.objectContaining({include_tasks:false})})));
 });
});
