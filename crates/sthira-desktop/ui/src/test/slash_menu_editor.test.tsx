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
      slashOpen={!!slash}
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
    for(const id of ['h1','h2','h3','todo','bullet','table','code','callout','divider','page','project','object-type','decision','question','idea','plan','research','expand'])expect(screen.getByTestId(`slash-cmd-${id}`)).toBeInTheDocument();
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
  it.each([['new page','page'],['new project','project'],['new custom object','object-type'],['new idea','idea']])('keeps /%s open and activates the intended command', (query,id)=>{
    render(<EditorWithMenu />);
    const element=screen.getByRole('textbox'); const editor=richEditor(element);
    act(()=>{editor.commands.focus('end');editor.commands.insertContent(` /${query}`);});
    expect(screen.getAllByRole('option')).toHaveLength(1);
    fireEvent.keyDown(element,{key:'Enter'});
    expect(screen.getByLabelText('Selected command')).toHaveTextContent(id);
  });
  it('replaces the whole divider command with a durable rule and preserves surrounding text',()=>{
    render(<EditorWithMenu />); const element=screen.getByRole('textbox');const editor=richEditor(element);
    act(()=>{editor.commands.focus('end');editor.commands.insertContent(' /divider');});
    fireEvent.keyDown(element,{key:'Enter'});
    expect(element.querySelector('hr')).toBeInTheDocument();
    expect(editor.getMarkdown()).toContain('Saved text');
    expect(editor.getMarkdown()).toContain('---');
    expect(editor.getMarkdown()).not.toContain('/divider');
    const markdown=editor.getMarkdown();
    act(()=>editor.commands.setContent(markdown,{contentType:'markdown'}));
    expect(element.querySelector('hr')).toBeInTheDocument();
  });
  it('moves through categories in their visible order with the keyboard',()=>{
    render(<EditorWithMenu />); const element=screen.getByRole('textbox');const editor=richEditor(element);
    act(()=>{editor.commands.focus('end');editor.commands.insertContent(' /');});
    const options=screen.getAllByRole('option');
    for(const option of options){
      expect(option).toHaveAttribute('aria-selected','true');
      fireEvent.keyDown(element,{key:'ArrowDown'});
    }
    expect(options[0]).toHaveAttribute('aria-selected','true');
  });
  it('releases Escape, ArrowUp, ArrowDown, and Enter after Escape dismisses the slash menu and still reopens on a new slash',async()=>{
    const user=userEvent.setup();
    render(<EditorWithMenu />);
    const element=screen.getByRole('textbox');
    const editor=richEditor(element);
    act(()=>{editor.commands.focus('end');editor.commands.insertContent(' /todo');});
    expect(screen.getByRole('listbox',{name:'Insert block or command'})).toBeInTheDocument();

    await user.keyboard('{Escape}');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();

    // Navigation inside the dismissed token must not reopen the slash menu or be defaultPrevented
    const arrowLeftEv=new KeyboardEvent('keydown',{key:'ArrowLeft',bubbles:true,cancelable:true});
    element.dispatchEvent(arrowLeftEv);
    expect(arrowLeftEv.defaultPrevented).toBe(false);
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();

    const arrowDownEv=new KeyboardEvent('keydown',{key:'ArrowDown',bubbles:true,cancelable:true});
    element.dispatchEvent(arrowDownEv);
    expect(arrowDownEv.defaultPrevented).toBe(false);

    const arrowUpEv=new KeyboardEvent('keydown',{key:'ArrowUp',bubbles:true,cancelable:true});
    element.dispatchEvent(arrowUpEv);
    expect(arrowUpEv.defaultPrevented).toBe(false);

    const escapeEv=new KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true});
    element.dispatchEvent(escapeEv);
    expect(escapeEv.defaultPrevented).toBe(false);

    // Pressing Enter inserts a real paragraph break instead of being trapped
    const paragraphsBefore=editor.getJSON().content?.length||0;
    act(()=>{editor.commands.focus('end');});
    await user.keyboard('{Enter}');
    expect(editor.getJSON().content?.length||0).toBeGreaterThan(paragraphsBefore);
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();

    // Typing a new slash command on the new paragraph reopens the menu cleanly
    act(()=>editor.commands.insertContent('/h2'));
    expect(screen.getByTestId('slash-cmd-h2')).toBeInTheDocument();

    // Escape + toolbar "Insert Block" button also reopens cleanly
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button',{name:'Insert Block'}));
    expect(screen.getByRole('listbox',{name:'Insert block or command'})).toBeInTheDocument();
  });
});
