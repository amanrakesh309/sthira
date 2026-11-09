import { useRef, useState } from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { DocumentEditor, type DocumentEditorHandle } from '../components/DocumentEditor';
import { SlashCommandMenu, type SlashCommandItem } from '../components/SlashCommandMenu';
import { richEditor } from './editor-helpers';

function EditorWithMenu() {
  const [content,setContent]=useState('# Test\n\nSaved text');
  const [slash,setSlash]=useState<{query:string;position:{top:number;left:number}}|null>(null);
  const editor=useRef<DocumentEditorHandle>(null); const body=useRef<HTMLDivElement>(null);
  const [selected,setSelected]=useState('');
  return <><button onClick={()=>editor.current?.format('slash')}>Insert Block</button><output aria-label="Selected command">{selected}</output>
    <div className="focus-editor-body" ref={body}><DocumentEditor ref={editor} content={content} title="Test" objectTypes={[]} onChange={setContent} onBlur={()=>{}} onOpenEntity={()=>{}}
      onSlash={(query,position)=>setSlash(query===null?null:{query,position})} />
      {slash&&<SlashCommandMenu anchorElement={body.current} position={slash.position} filterQuery={slash.query} customObjectTypes={[{id:'idea',display_name:'Idea',icon:'💡',color:'',schema:[],is_system:true,created_at:0}]}
        onClose={()=>setSlash(null)} onSelect={(item:SlashCommandItem)=>{setSelected(item.id);setSlash(null);if(item.template)editor.current?.insertMarkdown(item.template,true)}} />}
    </div></>;
}
describe('Slash commands in the rich editor',()=>{
  it('opens every command category from the toolbar inside existing text and portals outside the scroll region',async()=>{
    const user=userEvent.setup();render(<EditorWithMenu />);
    const editor=richEditor(screen.getByRole('textbox'));
    act(()=>editor.commands.setTextSelection(editor.state.doc.content.size-1));
    await user.click(screen.getByRole('button',{name:'Insert Block'}));
    const menu=screen.getByRole('listbox',{name:'Insert block or command'});
    expect(document.querySelector('.focus-editor-body')).not.toContainElement(menu);
    for(const id of ['h1','h2','h3','todo','bullet','table','code','callout','decision','question','idea','plan','research','expand'])expect(screen.getByTestId(`slash-cmd-${id}`)).toBeInTheDocument();
    await user.keyboard('{Escape}');expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });
  it('filters typed slash commands and activates a real formatted block from the keyboard',async()=>{
    render(<EditorWithMenu />);
    const element=screen.getByRole('textbox');const editor=richEditor(element);
    act(()=>{editor.commands.focus('end');editor.commands.insertContent(' /h2')});
    expect(screen.getByTestId('slash-cmd-h2')).toBeInTheDocument();
    expect(screen.queryByTestId('slash-cmd-table')).not.toBeInTheDocument();
    fireEvent.keyDown(element,{key:'Enter'});
    expect(screen.getByLabelText('Selected command')).toHaveTextContent('h2');
    await waitFor(()=>expect(screen.queryByRole('listbox')).not.toBeInTheDocument());
    expect(editor.getText()).toContain('Saved text');expect(editor.getText()).not.toContain('/h2');
    expect(editor.getJSON().content?.some(node=>node.type==='heading'&&node.attrs?.level===2)).toBe(true);
  });
  it('keeps empty searches discoverable and closes when leaving the editor by scrolling',()=>{
    render(<EditorWithMenu />);const editor=richEditor(screen.getByRole('textbox'));
    act(()=>editor.commands.insertContent(' /nomatchingcommand'));
    expect(screen.getByText(/No matching commands/)).toBeVisible();
    fireEvent.scroll(window);expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });
});
