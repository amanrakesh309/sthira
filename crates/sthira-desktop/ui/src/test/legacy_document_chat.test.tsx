import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { FocusView } from '../components/FocusView';
import { getDocumentConversation } from '../api/conversation';
vi.mock('../api/conversation',()=>({getDocumentConversation:vi.fn().mockResolvedValue({mode:'document',initial_prompt:'',turns:[]}),undoDocumentEdit:vi.fn()}));
vi.mock('../api/ipc',async(importOriginal)=>({
  ...await importOriginal<typeof import('../api/ipc')>(),
  getArtifactContext:vi.fn().mockImplementation(async(id:string)=>({artifact:{id,title:'Pre-existing notes',content_md:'# Pre-existing notes\n\nSaved before chat existed.',artifact_type:'note',authorship:'user',version:1,created_at:0,updated_at:0},epistemic_nodes:[],pending_proposals:[],project_id:null,is_unfinished_draft:false})),
  getProviderCatalog:vi.fn().mockResolvedValue([]),listObjectTypes:vi.fn().mockResolvedValue([]),getObjectInstance:vi.fn().mockResolvedValue(null),
}));
describe('Conversation on existing files',()=>{
  it('offers chat without a new run or inferred Plan/Research classification',async()=>{
    const user=userEvent.setup();render(<FocusView artifactId="legacy-note" onBack={()=>{}} onOpenCanvas={()=>{}} />);
    const button=await screen.findByRole('button',{name:'Conversation'});
    await waitFor(()=>expect(button).toBeEnabled());
    expect(getDocumentConversation).toHaveBeenCalledWith('legacy-note','document',undefined);
    await user.click(button);
    expect(screen.getByRole('complementary',{name:'Document conversation'})).toBeVisible();
    expect(screen.getByText('Think it through.')).toBeInTheDocument();
    expect(screen.getByTestId('document-editor')).toHaveTextContent('Saved before chat existed.');
  });
});
it('shows a truthful conversation error and retries for an existing document',async()=>{
  const user=userEvent.setup();
  vi.mocked(getDocumentConversation).mockRejectedValueOnce(new Error('Database unavailable'));
  render(<FocusView artifactId="legacy-retry" onBack={()=>{}} onOpenCanvas={()=>{}} />);
  const retry=await screen.findByRole('button',{name:'Retry conversation'});
  await user.click(retry);
  await waitFor(()=>expect(screen.queryByText('Couldn’t load this conversation. Try again.')).not.toBeInTheDocument());
  expect(screen.getByRole('button',{name:'Conversation'})).toBeEnabled();
});
