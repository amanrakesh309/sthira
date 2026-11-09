import { expect,it,vi } from 'vitest';
import { fireEvent,render,screen } from '@testing-library/react';
import { DocumentChatPanel } from '../components/DocumentChatPanel';
vi.mock('../api/conversation',()=>({getDocumentConversation:async()=>null,selectDocumentConversationModel:vi.fn(),discussDocument:vi.fn(),decideDocumentEdit:vi.fn()}));
vi.mock('../api/ipc',()=>({getProviderCatalog:async()=>[]}));
it('keeps unsent conversation text with its document and workspace while switching tabs',async()=>{
 sessionStorage.clear();
 const props={artifactId:'a',initialConversation:{mode:'document' as const,initial_prompt:'',turns:[]},open:true,busy:false,modelName:'Example',currentContent:'Source text',onClose:vi.fn(),onApplyingChange:vi.fn(),beforeRequest:async()=>true,onApplied:vi.fn()};
 const first=render(<DocumentChatPanel {...props} sessionNamespace="vault-a"/>);fireEvent.change(screen.getByLabelText('Message about this document'),{target:{value:'Unsent question for A'}});first.unmount();
 const other=render(<DocumentChatPanel {...props} artifactId="b" sessionNamespace="vault-a"/>);expect(screen.getByLabelText('Message about this document')).toHaveValue('');other.unmount();
 const restored=render(<DocumentChatPanel {...props} sessionNamespace="vault-a"/>);expect(screen.getByLabelText('Message about this document')).toHaveValue('Unsent question for A');restored.unmount();
 render(<DocumentChatPanel {...props} sessionNamespace="vault-b"/>);expect(screen.getByLabelText('Message about this document')).toHaveValue('');
});
