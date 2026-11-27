import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { WorkspaceRecoveryPanel } from '../components/WorkspaceRecoveryPanel';
const api=vi.hoisted(()=>vi.fn());vi.mock('../api/workspaceBackups',()=>({workspaceBackups:api}));
vi.mock('../components/RichDocumentRenderer',()=>({RichDocumentRenderer:({content}:{content:string})=><article>{content}</article>}));
const ok=(data:unknown)=>({outcome:'ok',data});
const preview={ticketId:'ticket',workspaceId:'target',targetName:'Empty team',sourceName:'Source team',checksumSha256:'checksum',counts:{pages:2,projects:1,historical_records:3,recoverable_drafts:1},expiresAtMs:Date.now()+900000,memberCount:2};
const receipt={restoreId:'r',workspaceId:'target',sourceName:'Source team',restoredAtMs:1,restoredByDisplayName:'Account A',checksumSha256:'checksum',counts:preview.counts};
const props={workspaceId:'target',workspaceName:'Empty team',currentUid:'a',myRole:'owner' as const,onSessionChange:vi.fn(),onAccessLost:vi.fn()};
describe('Shared workspace recovery UI (mocked native bridge)',()=>{
 beforeEach(()=>{vi.resetAllMocks();let restored=false;api.mockImplementation(async(_ws,_uid,action)=>ok(action==='list'?[]:action==='previewFile'?preview:action==='commit'?(restored=true,receipt):action==='restoration'&&restored?{receipt,history:null}:null));});
 it('requires a preview and explicit target confirmation; retries the same ticket after a lost response',async()=>{
  let attempts=0;const implementation=api.getMockImplementation()!;
  api.mockImplementation(async(...args)=>{if(args[2]==='commit'&&attempts++===0)throw new Error('Response lost. Retry safely.');return implementation(...args);});
  render(<WorkspaceRecoveryPanel {...props}/>);
  fireEvent.click(await screen.findByRole('button',{name:'Choose backup file'}));
  await screen.findByRole('heading',{name:'Restore Source team into Empty team'});
  expect(api.mock.calls.some(c=>c[2]==='commit')).toBe(false);
  const restore=screen.getByRole('button',{name:'Restore workspace'});expect(restore).toBeDisabled();
  fireEvent.click(screen.getByRole('checkbox'));fireEvent.click(restore);
  expect(await screen.findByRole('alert')).toHaveTextContent('Response lost');
  await waitFor(()=>expect(restore).toBeEnabled());fireEvent.click(restore);
  await screen.findByText('Workspace restored. Open Pages or Projects to inspect the recovered content.');
  expect(api.mock.calls.filter(c=>c[2]==='commit').map(c=>c[3])).toEqual([{ticketId:'ticket'},{ticketId:'ticket'}]);
 });
 it('keeps backup export and restore controls out of Viewer access',async()=>{
  render(<WorkspaceRecoveryPanel {...props} myRole="viewer"/>);
  await screen.findByText(/Only Owners and Admins/);expect(screen.queryByRole('button',{name:'Choose backup file'})).not.toBeInTheDocument();
  expect(api.mock.calls.some(c=>c[2]==='list')).toBe(false);
 });
 it('does not show a preview for a cancelled picker or validation failure',async()=>{
  api.mockImplementation(async(_ws,_uid,action)=>action==='previewFile'?Promise.reject(new Error('Backup checksum does not match its content.')):ok(action==='list'?[]:null));
  render(<WorkspaceRecoveryPanel {...props}/>);const choose=await screen.findByRole('button',{name:'Choose backup file'});await waitFor(()=>expect(choose).toBeEnabled());fireEvent.click(choose);
  await screen.findByRole('alert');expect(screen.queryByRole('button',{name:'Restore workspace'})).not.toBeInTheDocument();
  api.mockResolvedValue(ok({cancelled:true}));fireEvent.click(choose);await waitFor(()=>expect(choose).toBeEnabled());expect(screen.queryByLabelText('Restore preview')).not.toBeInTheDocument();
 });
 it('discards late preview results after same-profile account switching',async()=>{
  let release:(value:unknown)=>void=()=>{};
  const implementation=api.getMockImplementation()!;api.mockImplementation(async(...args)=>args[2]==='previewFile'?new Promise(r=>{release=r;}):implementation(...args));
  const view=render(<WorkspaceRecoveryPanel {...props}/>);const choose=await screen.findByRole('button',{name:'Choose backup file'});await waitFor(()=>expect(choose).toBeEnabled());fireEvent.click(choose);
  view.rerender(<WorkspaceRecoveryPanel {...props} currentUid="b" myRole="viewer"/>);
  await act(async()=>release(ok(preview)));expect(screen.queryByRole('heading',{name:'Restore Source team into Empty team'})).not.toBeInTheDocument();expect(api.mock.calls.some(c=>c[2]==='commit')).toBe(false);
 });
 it('drops an old preview across a role downgrade and later promotion',async()=>{
  const view=render(<WorkspaceRecoveryPanel {...props}/>);
  const choose=await screen.findByRole('button',{name:'Choose backup file'});await waitFor(()=>expect(choose).toBeEnabled());fireEvent.click(choose);
  await screen.findByRole('heading',{name:'Restore Source team into Empty team'});
  fireEvent.click(screen.getByRole('checkbox'));
  view.rerender(<WorkspaceRecoveryPanel {...props} myRole="viewer"/>);await screen.findByText(/Only Owners and Admins/);
  view.rerender(<WorkspaceRecoveryPanel {...props} myRole="admin"/>);
  await waitFor(()=>expect(screen.getByRole('button',{name:'Choose backup file'})).toBeEnabled());
  expect(screen.queryByRole('button',{name:'Restore workspace'})).not.toBeInTheDocument();expect(api.mock.calls.some(c=>c[2]==='commit')).toBe(false);
 });
 it('renders proposals and acknowledged drafts as read-only source history',async()=>{
  api.mockImplementation(async(_ws,_uid,action)=>ok(action==='list'?[]:{receipt,history:action==='history'?[{sourceName:'Source team',capturedAtMs:1,originChecksum:'checksum',pageMap:{},projectMap:{},records:[{table:'shared_change_proposals',rows:[{proposed_title:'Old proposal',proposed_content_md:'Unverified proposed text',status:'pending'}]},{table:'shared_live_drafts',rows:[{recovery_title:'Saved draft',recovery_content_md:'Recovered acknowledged text'}]}]}]:null}));
  render(<WorkspaceRecoveryPanel {...props} myRole="viewer"/>);fireEvent.click(await screen.findByRole('button',{name:'Inspect recovery history'}));
  for(const label of ['Change proposals (1)','Acknowledged drafts (1)']) {const details=(await screen.findByText(label)).closest('details')!;details.open=true;fireEvent(details,new Event('toggle'));}
  await screen.findByText('Unverified proposed text');await screen.findByText('Recovered acknowledged text');expect(screen.queryByRole('button',{name:/apply|approve|publish/i})).not.toBeInTheDocument();expect(screen.getByRole('button',{name:'Copy saved draft'})).toBeEnabled();
 });
 it('requires confirmation before removing a saved backup',async()=>{
  api.mockImplementation(async(_ws,_uid,action)=>ok(action==='list'?[{backupId:'backup',sourceName:'Source team',createdAtMs:1,createdByDisplayName:'Account A',bytes:2000,checksumSha256:'sum',counts:{pages:2}}]:null));
  render(<WorkspaceRecoveryPanel {...props}/>);fireEvent.click(await screen.findByRole('button',{name:'Remove backup'}));expect(api.mock.calls.some(c=>c[2]==='remove')).toBe(false);
  fireEvent.click(screen.getByRole('button',{name:'Keep backup'}));expect(api.mock.calls.some(c=>c[2]==='remove')).toBe(false);
  fireEvent.click(screen.getByRole('button',{name:'Remove backup'}));fireEvent.click(screen.getByRole('button',{name:'Confirm remove backup'}));await waitFor(()=>expect(api.mock.calls.some(c=>c[2]==='remove')).toBe(true));
 });
 it('chooses a legacy audience before requesting the file preview',async()=>{
  render(<WorkspaceRecoveryPanel {...props}/>);
  const choose=await screen.findByRole('button',{name:'Choose backup file'});
  await waitFor(()=>expect(choose).toBeEnabled());
  fireEvent.click(screen.getByRole('radio',{name:/Workspace-visible/}));
  fireEvent.click(choose);
  await screen.findByRole('heading',{name:'Restore Source team into Empty team'});
  expect(api).toHaveBeenCalledWith('target','a','previewFile',{legacyVisibility:'workspace'});
  expect(screen.getByRole('radio',{name:/Workspace-visible/})).toBeDisabled();
 });

});
