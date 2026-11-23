import { render, screen, waitFor, fireEvent, cleanup } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import { MemoryView } from '../views/MemoryView';
import { memoryAction, type MemoryEntry, type MemoryAction, type MemoryResult } from '../api/sharedMemory';
vi.mock('../api/sharedMemory',()=>({memoryAction:vi.fn()}));
vi.mock('../api/ipc',()=>({isTauri:false,getWorkspaceTree:vi.fn(async()=>({projects:[{id:'project-a',name:'Reading journal'}],unassociated_pages:[]})),searchAll:vi.fn(async()=>[{id:'source-a',title:'Source page',kind:'document'}])}));
const original:MemoryEntry={id:'memory-a',scope:{kind:'workspace'},title:'Keep uncertainty',body:'Distinguish proposals from settled choices.',author:{kind:'user'},sources:[],version:1,pinned:false,archived:false,created_at:1000,updated_at:1000};
let stored:MemoryEntry[];
const api=vi.mocked(memoryAction);
beforeEach(()=>{
  stored=[structuredClone(original)];api.mockReset();
  api.mockImplementation(async(action:MemoryAction):Promise<MemoryResult>=>{
    if(action.type==='list')return {kind:'entries',value:structuredClone(stored.filter(e=>action.query.include_archived||!e.archived))};
    if(action.type==='history')return {kind:'history',value:[{version:1,operation:'create',editor:{kind:'user'},snapshot:original,created_at:1000}]};
    if(action.type==='context')return {kind:'context',value:{entries:stored,used_chars:300,skipped_stale:0,skipped_budget:0,candidate_limit:100,content_is_untrusted:true}};
    if(action.type==='read')return {kind:'entry',value:stored.find(e=>e.id===action.id)!};
    if(action.type==='create'){const entry={...original,id:'memory-new',...action.draft,sources:[]};stored.push(entry);return {kind:'entry',value:entry};}
    const index=stored.findIndex(e=>e.id===action.id);const entry={...stored[index],version:stored[index].version+1};
    if(action.type==='update'){entry.title=action.title;entry.body=action.body;entry.pinned=action.pinned;}
    else entry.archived=action.type==='archive';
    stored[index]=entry;return {kind:'entry',value:entry};
  });
});
afterEach(cleanup);
it('creates a scoped memory through the shared action and shows its saved attribution',async()=>{
  const user=userEvent.setup();render(<MemoryView onOpenArtifact={vi.fn()}/>);
  await screen.findByRole('button',{name:/Keep uncertainty/});
  await user.click(screen.getByRole('button',{name:'New memory'}));
  await user.type(screen.getByRole('textbox',{name:'Memory title'}),'Reading handoff');
  await user.type(screen.getByRole('textbox',{name:'Memory content'}),'Compare the remaining sources.');
  await user.selectOptions(screen.getByLabelText('New memory scope'),'project');
  await user.selectOptions(screen.getByLabelText('Memory project'),'project-a');
  await user.click(screen.getByRole('button',{name:'Save memory'}));
  expect(api).toHaveBeenCalledWith({type:'create',draft:{scope:{kind:'project',id:'project-a'},title:'Reading handoff',body:'Compare the remaining sources.',source_ids:[],pinned:false}});
  expect(await screen.findByText('Memory saved.')).toBeInTheDocument();expect(screen.getByText(/Originally written by you/)).toBeInTheDocument();
});
it('retains a failed save and blocks leaving or switching the unsaved draft',async()=>{
  const user=userEvent.setup();let guard:(()=>Promise<boolean>)|null=null;
  render(<MemoryView onOpenArtifact={vi.fn()} registerBeforeLeave={value=>{guard=value;}}/>);
  await user.click(await screen.findByRole('button',{name:/Keep uncertainty/}));
  await user.type(screen.getByRole('textbox',{name:'Memory content'}),' More context.');
  api.mockImplementationOnce(async()=>{throw new Error('Memory changed in another session. Reload before saving.');});
  await user.click(screen.getByRole('button',{name:'Save changes'}));
  expect(await screen.findByRole('alert')).toHaveTextContent('Memory changed');
  expect(screen.getByRole('textbox',{name:'Memory content'})).toHaveValue('Distinguish proposals from settled choices. More context.');
  expect(await guard!()).toBe(false);
  await user.click(screen.getByRole('button',{name:'New memory'}));expect(screen.getByRole('textbox',{name:'Memory title'})).toHaveValue('Keep uncertainty');
});
it('archives with the saved version and restores without rewriting its body',async()=>{
  const user=userEvent.setup();render(<MemoryView onOpenArtifact={vi.fn()}/>);
  await user.click(await screen.findByRole('button',{name:/Keep uncertainty/}));await user.click(screen.getByRole('button',{name:'Cancel'}));
  await user.click(screen.getByRole('button',{name:'Archive memory'}));
  await waitFor(()=>expect(api).toHaveBeenCalledWith({type:'archive',id:'memory-a',expected_version:1}));
  await user.click(await screen.findByRole('button',{name:'Restore memory'}));
  await waitFor(()=>expect(api).toHaveBeenCalledWith({type:'restore',id:'memory-a',expected_version:2}));
  expect(stored[0].body).toBe(original.body);expect(stored[0].archived).toBe(false);
});
it('ignores an older history response after selecting a different memory',async()=>{
  const user=userEvent.setup();stored.push({...original,id:'memory-b',title:'Another context'});
  render(<MemoryView onOpenArtifact={vi.fn()}/>);
  await user.click(await screen.findByRole('button',{name:/Keep uncertainty/}));
  let resolveHistory!:(result:MemoryResult)=>void;
  api.mockImplementationOnce(()=>new Promise(resolve=>{resolveHistory=resolve;}));
  await user.click(screen.getByRole('button',{name:'History'}));
  await user.click(screen.getByRole('button',{name:/Another context/}));
  resolveHistory({kind:'history',value:[{version:99,operation:'old-result',editor:{kind:'user'},snapshot:original,created_at:1000}]});
  await waitFor(()=>expect(screen.queryByText(/Version 99/)).not.toBeInTheDocument());
  expect(screen.getByRole('textbox',{name:'Memory title'})).toHaveValue('Another context');
});
it('requests bounded context with the selected scope and shows stale exclusions',async()=>{
  const user=userEvent.setup();render(<MemoryView onOpenArtifact={vi.fn()}/>);
  await screen.findByRole('button',{name:/Keep uncertainty/});
  await user.selectOptions(screen.getByLabelText('Filter memory scope'),'project:project-a');
  await waitFor(()=>expect(api).toHaveBeenCalledWith({type:'list',query:{search:'',include_archived:false,scope:{kind:'project',id:'project-a'}}}));
  await user.click(screen.getByRole('button',{name:'Preview shared context'}));
  expect(api).toHaveBeenCalledWith({type:'context',query:{search:'',include_archived:false,scope:{kind:'project',id:'project-a'}},max_entries:8,max_chars:8000});
  expect(await screen.findByRole('region',{name:'Shared context preview'})).toHaveTextContent('no model was called');
});
it('adds selected source pages to creation and never sends authorship in tool arguments',async()=>{
  const user=userEvent.setup();render(<MemoryView onOpenArtifact={vi.fn()}/>);
  await user.click(screen.getByRole('button',{name:'New memory'}));
  await user.type(screen.getByRole('textbox',{name:'Memory title'}),'Sourced memory');await user.type(screen.getByRole('textbox',{name:'Memory content'}),'Keep this excerpt available.');
  fireEvent.change(screen.getByRole('textbox',{name:'Find memory sources'}),{target:{value:'Source'}});
  await user.click(await screen.findByRole('button',{name:'Add source: Source page'}));
  await user.click(screen.getByRole('button',{name:'Save memory'}));
  expect(api).toHaveBeenCalledWith({type:'create',draft:{title:'Sourced memory',body:'Keep this excerpt available.',scope:{kind:'workspace'},source_ids:['source-a'],pinned:false}});
});
