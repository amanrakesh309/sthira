import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DocumentChatPanel } from '../components/DocumentChatPanel';
import { decideDocumentEdit, discussDocument, getDocumentConversation, selectDocumentConversationModel, type Conversation } from '../api/conversation';
vi.mock('../api/conversation', () => ({ decideDocumentEdit: vi.fn(), discussDocument: vi.fn(), getDocumentConversation: vi.fn(), selectDocumentConversationModel: vi.fn() }));
import { getProviderCatalog } from '../api/ipc';
vi.mock('../api/ipc', async original => ({ ...await original<typeof import('../api/ipc')>(), getProviderCatalog: vi.fn() }));
const models = ['a','b'].map(id => ({id:`model-${id}`,provider_id:'local',model_identifier:`identifier-${id}`,display_name:`Model ${id.toUpperCase()}`,context_window:8192,supports_structured_output:true,is_enabled:true,is_default:id==='a'}));
const catalog = [{id:'local',display_name:'Local provider',provider_type:'ollama',base_url:'http://unused',has_api_key:false,is_enabled:true,models}];
const chat: Conversation = { mode:'plan', initial_prompt:'A quiet plan', turns:[] };
const suggestion: Conversation = { ...chat, turns:[{ id:'turn-a', user_text:'Shorten it', reply:'Here is a shorter version.', error:null, basis_content_md:'Original', proposed_content_md:'Shorter', proposal_state:'pending', change_set_id:null }] };
const props = () => ({ artifactId:'doc-a', initialConversation:chat, open:true, busy:false, modelName:'Test model', modelId:'model-a', currentContent:'Original', onClose:vi.fn(), beforeRequest:vi.fn().mockResolvedValue(true), onApplied:vi.fn(), onApplyingChange:vi.fn() });
beforeEach(() => { vi.clearAllMocks(); vi.mocked(getProviderCatalog).mockResolvedValue(catalog); vi.mocked(getDocumentConversation).mockResolvedValue(chat); });
describe('Document discussion and reviewed changes', () => {
  it('saves first, sends model and edit intent, then requires review and explicit Apply', async () => {
    const user=userEvent.setup(); const p=props();
    vi.mocked(discussDocument).mockResolvedValue(suggestion);
    render(<DocumentChatPanel {...p} />);
    await user.type(screen.getByRole('textbox'), 'Shorten it');
    await user.click(screen.getByRole('checkbox', { name:'Suggest a document edit' }));
    await user.click(screen.getByRole('button', {name:'Send message'}));
    expect(p.beforeRequest).toHaveBeenCalled();
    expect(discussDocument).toHaveBeenCalledWith('doc-a','Shorten it','model-a',true);
    expect(decideDocumentEdit).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button',{name:'Review suggested edit'}));
    expect(screen.getByText('Current document')).toBeInTheDocument();
    expect(screen.getByText('Suggested document')).toBeInTheDocument();
    vi.mocked(decideDocumentEdit).mockResolvedValue({id:'doc-a',content_md:'Shorter'} as never);
    vi.mocked(getDocumentConversation).mockResolvedValue({...suggestion,turns:[{...suggestion.turns[0],proposal_state:'applied',change_set_id:'cs-a'}]});
    await user.click(screen.getByRole('button',{name:'Apply edit'}));
    await waitFor(() => expect(p.onApplied).toHaveBeenCalledWith(expect.objectContaining({content_md:'Shorter'}),'cs-a'));
    expect(decideDocumentEdit).toHaveBeenCalledWith('doc-a','turn-a',true);
  });
  it('blocks stale suggestions and supports dismissal without saving the document', async () => {
    const user=userEvent.setup(); const p={...props(), initialConversation:suggestion,currentContent:'New draft'};
    vi.mocked(decideDocumentEdit).mockResolvedValue({id:'doc-a',content_md:'New draft'} as never);
    vi.mocked(getDocumentConversation).mockResolvedValue({...suggestion,turns:[{...suggestion.turns[0],proposal_state:'dismissed'}]});
    vi.mocked(getDocumentConversation).mockResolvedValue(p.initialConversation);
    render(<DocumentChatPanel {...p} />);
    await user.click(screen.getByRole('button',{name:'Review suggested edit'}));
    expect(screen.getByRole('button',{name:'Apply edit'})).toBeDisabled();
    await user.click(screen.getByRole('button',{name:'Dismiss'}));
    await waitFor(() => expect(decideDocumentEdit).toHaveBeenCalledWith('doc-a','turn-a',false));
    expect(p.beforeRequest).not.toHaveBeenCalled(); expect(p.onApplied).not.toHaveBeenCalled();
  });
  it('retains input on a failed save and does not call the model', async () => {
    const user=userEvent.setup();const p=props();p.beforeRequest.mockResolvedValue(false);
    render(<DocumentChatPanel {...p} />);
    await user.type(screen.getByRole('textbox'),'Keep this question');await user.keyboard('{Enter}');
    expect(discussDocument).not.toHaveBeenCalled();expect(screen.getByRole('textbox')).toHaveValue('Keep this question');
  });
  it('does not deliver a late reply or edit to a different document after unmount', async () => {
    const user=userEvent.setup(); const p=props(); let resolve!:(value:Conversation)=>void;
    vi.mocked(discussDocument).mockReturnValue(new Promise(r=>{resolve=r}));
    const view=render(<DocumentChatPanel {...p} />);
    await user.type(screen.getByRole('textbox'),'Question');await user.keyboard('{Enter}');
    await waitFor(()=>expect(discussDocument).toHaveBeenCalled());view.unmount();resolve(suggestion);
    expect(p.onApplied).not.toHaveBeenCalled();
  });
  it('blocks send during generation and closes with Escape', async () => {
    const user=userEvent.setup(); const p={...props(),busy:true};render(<DocumentChatPanel {...p} />);
    await user.type(screen.getByRole('textbox'),'Question');expect(screen.getByRole('button',{name:'Send message'})).toBeDisabled();
    await user.keyboard('{Escape}');expect(p.onClose).toHaveBeenCalled();
  });
});

it('reviews rich text in a dialog outside the narrow rail and restores keyboard focus', async () => {
  const user=userEvent.setup();
  const rich={...suggestion,turns:[{...suggestion.turns[0], proposed_content_md:'# Revised plan\n\n**Focus** on one milestone.\n\n- [ ] Review evidence\n\n| Step | Owner |\n| --- | --- |\n| Review | You |'}]};
  vi.mocked(getDocumentConversation).mockResolvedValue(rich);
  render(<DocumentChatPanel {...props()} initialConversation={rich} />);
  const opener=screen.getByRole('button',{name:'Review suggested edit'});
  await user.click(opener);
  const dialog=screen.getByRole('dialog',{name:'Review changes'});
  expect(document.querySelector('.document-chat')).not.toContainElement(dialog);
  expect(screen.getByRole('heading',{name:'Revised plan'})).toBeInTheDocument();
  expect(dialog.querySelector('strong')).toHaveTextContent('Focus');
  expect(dialog.querySelector('table')).toBeInTheDocument();
  expect(dialog.querySelector('input[type="checkbox"]')).toBeDisabled();
  await user.keyboard('{Escape}');
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(opener).toHaveFocus();
});

it('persists the chosen model, keeps conversation and draft, and sends the next message to that model', async () => {
  const user=userEvent.setup();
  const prior={...suggestion,model_id:'model-a',model_name:'Model A',turns:[{...suggestion.turns[0],model_id:'model-a',model_name:'Model A',provider_name:'Local provider'}]};
  const switched={...prior,model_id:'model-b',model_name:'Model B'};
  vi.mocked(getDocumentConversation).mockResolvedValue(prior);
  vi.mocked(selectDocumentConversationModel).mockResolvedValue(switched);
  vi.mocked(discussDocument).mockResolvedValue({...switched,turns:[...prior.turns,{...prior.turns[0],id:'turn-b',reply:'Second reply',model_id:'model-b',model_name:'Model B'}]});
  const p=props();render(<DocumentChatPanel {...p} initialConversation={prior} />);
  const picker=screen.getByRole('combobox',{name:'Conversation model'});
  await waitFor(()=>expect(picker).toBeEnabled());
  await user.type(screen.getByRole('textbox'),'Keep discussing this');
  await user.selectOptions(picker,'model-b');
  await waitFor(()=>expect(picker).toHaveValue('model-b'));
  expect(selectDocumentConversationModel).toHaveBeenCalledWith('doc-a','model-b');
  expect(screen.getByRole('textbox')).toHaveValue('Keep discussing this');
  expect(screen.getByText('Here is a shorter version.')).toBeInTheDocument();
  expect(screen.getByText('Sthira · Model A · Local provider')).toBeInTheDocument();
  expect(p.beforeRequest).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button',{name:'Send message'}));
  await waitFor(()=>expect(discussDocument).toHaveBeenCalledWith('doc-a','Keep discussing this','model-b',false));
  expect(screen.getByText('Sthira · Model B · Local provider')).toBeInTheDocument();
});
it('keeps the previous choice and draft if saving a model selection fails', async () => {
  const user=userEvent.setup();vi.mocked(selectDocumentConversationModel).mockRejectedValue(new Error('Could not save model choice'));
  render(<DocumentChatPanel {...props()} />);
  const picker=screen.getByRole('combobox');await waitFor(()=>expect(picker).toBeEnabled());
  await user.type(screen.getByRole('textbox'),'Unsent question');await user.selectOptions(picker,'model-b');
  expect(await screen.findByRole('alert')).toHaveTextContent('Could not save model choice');
  expect(picker).toHaveValue('model-a');expect(screen.getByRole('textbox')).toHaveValue('Unsent question');
});
it('restores a saved model and shows older replies without invented model attribution', async () => {
  const saved={...suggestion,model_id:'model-b',model_name:'Model B'};
  vi.mocked(getDocumentConversation).mockResolvedValue(saved);
  render(<DocumentChatPanel {...props()} initialConversation={saved} />);
  await waitFor(()=>expect(screen.getByRole('combobox')).toHaveValue('model-b'));
  expect(screen.getByText('Sthira · Model not recorded')).toBeInTheDocument();
  expect(selectDocumentConversationModel).not.toHaveBeenCalled();
});
it('does not silently replace a removed model with the catalog default', async () => {
  const saved={...chat,model_id:'removed-model',model_name:'Removed model'};
  vi.mocked(getDocumentConversation).mockResolvedValue(saved);
  render(<DocumentChatPanel {...props()} initialConversation={saved} />);
  await waitFor(()=>expect(screen.getByRole('combobox')).toBeEnabled());
  expect(screen.getByRole('combobox')).toHaveValue('removed-model');
  expect(screen.getByText('This model is unavailable. Choose an enabled model to continue.')).toBeInTheDocument();
  expect(screen.getByRole('button',{name:'Send message'})).toBeDisabled();
});
