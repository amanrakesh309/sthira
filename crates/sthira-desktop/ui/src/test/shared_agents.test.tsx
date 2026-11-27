import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SharedActivityFeed } from '../components/SharedPagesPanel';
import { SharedAgentsPanel, SharedAgentSettings } from '../components/SharedAgentsPanel';
import type { SharedAgentProposal } from '../api/sharedAgents';

const collab = vi.hoisted(() => ({listeners: new Set<(event: unknown) => void>()}));
vi.mock('../api/liveCollab', () => ({subscribeCollabEvents: async (listener: (event: unknown) => void) => {collab.listeners.add(listener);return () => collab.listeners.delete(listener);}}));
const api=vi.hoisted(()=>({sharedAgents:vi.fn(),listSharedProjects:vi.fn(),getSharedProject:vi.fn(),getProposalDetails:vi.fn(),listSharedRevisions:vi.fn(),listWorkspaceActivity:vi.fn(),listWorkspaces:vi.fn(),listAvailableModels:vi.fn()}));
vi.mock('../api/sharedAgents',()=>({sharedAgents:api.sharedAgents}));
vi.mock('../api/ipc',()=>({listAvailableModels:api.listAvailableModels}));
vi.mock('../api/workspaces',async original=>({...await original<typeof import('../api/workspaces')>(),listSharedProjects:api.listSharedProjects,getSharedProject:api.getSharedProject,getProposalDetails:api.getProposalDetails,listSharedRevisions:api.listSharedRevisions,listWorkspaceActivity:api.listWorkspaceActivity,listWorkspaces:api.listWorkspaces}));
vi.mock('../components/RichDocumentRenderer',()=>({RichDocumentRenderer:({content}:{content:string})=><article>{content}</article>}));
const ok=(data:unknown)=>({outcome:'ok',data});
const proposal:SharedAgentProposal={proposalId:'proposal-b',kind:'newPage',workspaceId:'ws',projectId:'project',projectName:'Cedar',targetPageId:null,targetTitle:'Proposed page',baseRevisionNumber:null,proposedTitle:'Proposed page',proposedContentMd:'# Saved agent draft',summary:'Review the scoped suggestion',status:'pending',submissionCount:1,authorUid:'b',authorDisplayName:'Account B',connectionId:'connection',connectionLabel:'B connection',agentLabel:'Sthira native',modelLabel:'fixture-model',providerLabel:'fixture-provider',modelTrust:'declared_unverified',sources:[{pageId:'source',pageTitle:'Source Cedar',revisionNumber:2,contentSha256:'abc123'}],isStale:false,staleReason:null,reviewEligible:true,revocationState:'active',appliedPageId:null,appliedRevisionNumber:null,reviewerDisplayName:null};
function mount(uid='a', initialProposalId?:string){return render(<SharedAgentsPanel workspaceId="ws" myRole="owner" currentUid={uid} initialProposalId={initialProposalId} />);}
describe('Shared agent desktop boundary',()=>{
 beforeEach(()=>{
  vi.resetAllMocks();
  collab.listeners.clear();
  api.listAvailableModels.mockResolvedValue([{id:'model',display_name:'Fixture',provider_name:'Loopback',is_default:true}]);
  api.listSharedProjects.mockResolvedValue(ok({items:[{projectId:'project',name:'Cedar'}],nextCursor:null}));
  api.getSharedProject.mockResolvedValue(ok({pages:[{page:{pageId:'source',title:'Source Cedar'}},{page:{pageId:'unselected',title:'Private from this connection'}}],pagesNextCursor:null}));
  api.getProposalDetails.mockResolvedValue(ok({baseContentMd:'Immutable base',currentPageContentMd:'Current human revision',currentPageRevisionNumber:3}));
  api.sharedAgents.mockImplementation(async (_ws,action)=>ok(action==='policy'?{enabled:true}:action==='proposal'?proposal:action==='create'?{config:[0,1,2].map(i=>({label:'Format '+i,instructions:'Merge configuration',json:{mcpServers:{sthira:{args:['private-path']}}}}))}:action==='review'?{...proposal,status:'applied'}:[]));
 });
 it('defaults to read and sends only checked pages; configuration has no bearer token',async()=>{
  mount();
  fireEvent.change(await screen.findByLabelText('Shared project'),{target:{value:'project'}});
  fireEvent.click(await screen.findByLabelText('Source Cedar'));
  fireEvent.change(screen.getByLabelText('Agent client'),{target:{value:'Codex'}});
  fireEvent.click(screen.getByRole('button',{name:'Create shared connection'}));
  await screen.findByLabelText('Shared MCP configuration');
  expect(api.sharedAgents).toHaveBeenCalledWith('ws','create',{projectId:'project',clientName:'Codex',label:'Codex',permission:'read',pageIds:['source']});
  expect(screen.getByLabelText('Shared MCP configuration')).not.toHaveValue(expect.stringContaining('stha_'));
 });
 it('shows attributed new-page review, snapshots and independent apply',async()=>{
  mount('a','proposal-b');
  await screen.findByRole('heading',{name:'Review the scoped suggestion'});
  expect(screen.getByText('Account B')).toBeVisible();
  expect(screen.getByText('fixture-model / fixture-provider')).toBeVisible();
  expect(screen.getByText('Reported by the initiating app; not verified by the service')).toBeVisible();
  expect(screen.getByRole('button',{name:'Source Cedar · rev 2'})).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button',{name:'Apply agent proposal'}));
  await screen.findByText('Applied after independent review.');
  expect(api.sharedAgents).toHaveBeenCalledWith('ws','review',expect.objectContaining({proposalId:'proposal-b',review:expect.objectContaining({action:'apply',expectedSubmissionNumber:1})}));
 });
 it('clears an open source snapshot and proposal after their audience is revoked',async()=>{
  api.listSharedRevisions.mockResolvedValue(ok({items:[{revisionNumber:2,title:'Source Cedar',contentMd:'PRIVATE_SOURCE_BODY',contentSha256:'abc123',authorDisplayName:'Author'}],nextCursor:null}));
  mount('a','proposal-b');
  fireEvent.click(await screen.findByRole('button',{name:'Source Cedar · rev 2'}));
  await screen.findByText('PRIVATE_SOURCE_BODY');
  api.sharedAgents.mockImplementation(async(_ws,action)=>action==='proposal'?{outcome:'error',code:'agent_proposal_not_found',message:'Unavailable'}:ok(action==='policy'?{enabled:true}:[]));
  await act(async()=>{for(const listener of collab.listeners)listener({type:'workspace_event',workspaceId:'ws',eventType:'resource_access_changed',payload:{}});});
  await waitFor(()=>expect(screen.queryByText('PRIVATE_SOURCE_BODY')).not.toBeInTheDocument());
  expect(screen.queryByText('# Saved agent draft')).not.toBeInTheDocument();
 });
 it('rechecks a saved review when administrative access is removed by a role change',async()=>{
  const view=mount('a','proposal-b');
  await screen.findByRole('heading',{name:'Review the scoped suggestion'});
  api.sharedAgents.mockImplementation(async(_ws,action)=>action==='proposal'?{outcome:'error',code:'agent_proposal_not_found',message:'Unavailable'}:ok(action==='policy'?{enabled:true}:[]));
  view.rerender(<SharedAgentsPanel workspaceId="ws" myRole="editor" currentUid="a" initialProposalId="proposal-b"/>);
  await waitFor(()=>expect(screen.queryByRole('heading',{name:'Review the scoped suggestion'})).not.toBeInTheDocument());
  expect(screen.queryByText('# Saved agent draft')).not.toBeInTheDocument();
 });
 it('blocks self apply and keeps revoked suggestions inspectable and dismissible',async()=>{
  api.sharedAgents.mockImplementation(async(_ws,action)=>ok(action==='proposal'?{...proposal,reviewEligible:false,isStale:true,staleReason:'Connection revoked',revocationState:'revoked'}:action==='policy'?{enabled:true}:[]));
  mount('b','proposal-b');
  expect(await screen.findByRole('button',{name:'Apply agent proposal'})).toBeDisabled();
  expect(screen.getByText('Cannot apply: Connection revoked')).toBeVisible();
  expect(screen.getByRole('button',{name:'Dismiss agent proposal'})).toBeDisabled();
 });
 it('compares immutable base and current content for an edit',async()=>{
  api.sharedAgents.mockImplementation(async(_ws,action)=>ok(action==='proposal'?{...proposal,kind:'edit',targetPageId:'source',baseRevisionNumber:2}:action==='policy'?{enabled:true}:[]));
  mount('a','proposal-b');
  await screen.findByText('Immutable base');
  fireEvent.click(screen.getByRole('button',{name:'Compare current revision'}));
  expect(screen.getByText('Current human revision')).toBeVisible();
  expect(screen.getByRole('heading',{name:'Current published revision 3'})).toBeVisible();
 });
 it('clears old account review while an earlier request completes late',async()=>{
  let complete:(v:unknown)=>void=()=>{};
  api.sharedAgents.mockImplementation(async(_ws,action)=>action==='proposal'?new Promise(r=>{complete=r;}):ok(action==='policy'?{enabled:true}:[]));
  const view=mount('a','proposal-b');
  await waitFor(()=>expect(api.sharedAgents).toHaveBeenCalledWith('ws','proposal',{proposalId:'proposal-b'}));
  view.rerender(<SharedAgentsPanel workspaceId="ws" myRole="viewer" currentUid="other" />);
  complete(ok(proposal));
  await screen.findByRole('heading',{name:'Shared agents'});
  expect(screen.queryByText('Review the scoped suggestion')).not.toBeInTheDocument();
 });
 it('shows persisted interrupted runs and sends explicit recovery using the same run ID',async()=>{
  api.sharedAgents.mockImplementation(async(_ws,action)=>ok(action==='runs'?[{id:'saved-run',goal:'Keep edits',stage:'Interrupted',status:'interrupted',sources:[],modelId:'model'}]:action==='policy'?{enabled:true}:[]));
  mount();
  fireEvent.click(await screen.findByRole('button',{name:'Open run saved-ru'}));
  fireEvent.click(screen.getByRole('button',{name:'Recover shared run'}));
  await waitFor(()=>expect(api.sharedAgents).toHaveBeenCalledWith('ws','retry',{runId:'saved-run'}));
 });
 it('routes a new-page Activity proposal with no page ID to its exact attributed review',async()=>{
  api.listWorkspaceActivity.mockResolvedValue(ok({items:[{activityId:'activity',actorDisplayName:'Account B',detailText:'Proposed Cedar',createdAtMs:1,agentProposal:true,projectId:'project-exact',pageId:null,proposalId:'proposal-exact'}],nextCursor:null}));
  const open=vi.fn(),page=vi.fn(),project=vi.fn();
  render(<SharedActivityFeed workspaceId="ws" refreshSignal={0} onOpenAgentProposal={open} onOpenPage={page} onOpenProject={project} onAccessLost={vi.fn()} onSessionChange={vi.fn()} />);
  fireEvent.click(await screen.findByRole('button',{name:'Open agent proposal'}));
  expect(open).toHaveBeenCalledExactlyOnceWith('proposal-exact');expect(page).not.toHaveBeenCalled();expect(project).not.toHaveBeenCalled();
 });
 it('discards a late Settings workspace list after the account changes',async()=>{
  let complete:(value:unknown)=>void=()=>{};
  api.listWorkspaces.mockImplementation(()=>new Promise(r=>{complete=r;}));
  render(<SharedAgentSettings />);
  window.dispatchEvent(new Event('sthira:account-context-changed'));
  complete(ok({workspaces:[{workspaceId:'old',name:'Old account workspace',myRole:'owner'}]}));
  await screen.findByText('Account changed. Reopen shared setup to refresh.');
  expect(screen.queryByRole('option',{name:'Old account workspace'})).not.toBeInTheDocument();
 });

 it('offers native runs only for verified local credentials, independent of a missing UI identity',async()=>{
  api.sharedAgents.mockImplementation(async(_ws,action)=>ok(action==='connections'?[{connectionId:'local',label:'Local connection',projectName:'Cedar',memberUid:'verified-b',permission:'read',pages:[],canRunOnDevice:true},{connectionId:'remote',label:'Other device',projectName:'Cedar',memberUid:'verified-b',permission:'read',pages:[],canRunOnDevice:false}]:action==='policy'?{enabled:true}:[]));
  mount('');
  expect(await screen.findByRole('option',{name:'Local connection · Cedar'})).toBeInTheDocument();
  expect(screen.queryByRole('option',{name:'Other device · Cedar'})).not.toBeInTheDocument();
 });

 it('opens the exact saved source revision and checks its hash instead of substituting current content',async()=>{
  api.listSharedRevisions.mockResolvedValue(ok({items:[{pageId:'source',revisionNumber:3,contentMd:'Later human changes',contentSha256:'later'},{pageId:'source',revisionNumber:2,title:'Source Cedar',contentMd:'Original source snapshot',contentSha256:'abc123',authorDisplayName:'Original author'}],nextCursor:null}));
  mount('a','proposal-b');
  fireEvent.click(await screen.findByRole('button',{name:'Source Cedar · rev 2'}));
  expect(await screen.findByText('Original source snapshot')).toBeVisible();
  expect(screen.queryByText('Later human changes')).not.toBeInTheDocument();
  expect(screen.getByText('Author: Original author')).toBeVisible();
 });
 it('keeps a mismatched source hash out of the review',async()=>{
  api.listSharedRevisions.mockResolvedValue(ok({items:[{revisionNumber:2,contentMd:'Wrong snapshot',contentSha256:'wrong'}],nextCursor:null}));
  mount('a','proposal-b');
  fireEvent.click(await screen.findByRole('button',{name:'Source Cedar · rev 2'}));
  expect(await screen.findByRole('alert')).toHaveTextContent('source snapshot hash does not match');
  expect(screen.queryByText('Wrong snapshot')).not.toBeInTheDocument();
 });
 it('does not reopen a consumed Activity review when parent callbacks change',async()=>{
  const props={workspaceId:'ws',myRole:'owner' as const,currentUid:'a',initialProposalId:'proposal-b'};
  const view=render(<SharedAgentsPanel {...props} onAccessLost={vi.fn()} />);
  fireEvent.click(await screen.findByRole('button',{name:'Back to agents'}));
  view.rerender(<SharedAgentsPanel {...props} onAccessLost={vi.fn()} />);
  await screen.findByRole('heading',{name:'Shared agents'});
  expect(screen.queryByRole('heading',{name:'Review the scoped suggestion'})).not.toBeInTheDocument();
 });

 it('disables review during refresh so an accepted click cannot be silently ignored',async()=>{
  let hold=false,release:(value:unknown)=>void=()=>{};
  api.sharedAgents.mockImplementation(async(_ws,action)=>action==='policy' && hold ? new Promise(r=>{release=r;}) : ok(action==='policy'?{enabled:true}:action==='proposals'?[proposal]:action==='proposal'?proposal:[]));
  mount();
  const review=await screen.findByRole('button',{name:'Review Proposed page'});
  hold=true;fireEvent.click(screen.getByRole('button',{name:'Refresh agents'}));
  expect(review).toBeDisabled();hold=false;release(ok({enabled:true}));
  await waitFor(()=>expect(review).toBeEnabled());
  fireEvent.click(review);await screen.findByRole('heading',{name:'Review the scoped suggestion'});
 });
 it('shows an honest loading error without inventing a disabled workspace policy',async()=>{
  api.sharedAgents.mockRejectedValue(new Error('Service unavailable'));
  mount();
  expect(await screen.findByRole('alert')).toHaveTextContent('Service unavailable');
  expect(screen.queryByText('Workspace agent access: Disabled')).not.toBeInTheDocument();
  expect(screen.getByRole('button',{name:'Retry loading shared agents'})).toBeEnabled();
 });

 it('shows durable reviewer feedback to the initiating member after changes are requested',async()=>{
  api.sharedAgents.mockImplementation(async(_ws,action)=>ok(action==='proposal'?{...proposal,status:'changes_requested',reviewEligible:false,reviewerDisplayName:'Account A',reviewerNote:'Keep the source boundary explicit.'}:action==='policy'?{enabled:true}:[]));
  mount('b','proposal-b');
  expect(await screen.findByLabelText('Reviewer note')).toHaveTextContent('Keep the source boundary explicit.');
  expect(screen.getByText('Reviewed by Account A')).toBeVisible();
 });

});
