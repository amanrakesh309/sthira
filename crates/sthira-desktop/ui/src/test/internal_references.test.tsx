import { act, fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { DocumentEditor } from '../components/DocumentEditor';
import { RichDocumentRenderer } from '../components/RichDocumentRenderer';
import { parseEntityHref, parseReferenceQuery } from '../api/connections';
import { richEditor } from './editor-helpers';
const id='9ea18c81-36ab-4111-ae5d-a6e0250011fd';
const uri=`sthira://artifact/${id}`;
const references=`## References\n\n1. Privacy Focused Distraction Blocking App – \`${uri}\`\n2. Executive Summary – \`sthira://artifact/1e6e33ee-987c-4854-b646-664b0d4fa3aa\`\n3. Learning System Design from Scratch – \`sthira://artifact/5f220542-f47b-4470-8195-94175b0b813f\``;
const props=(content=references)=>({content,title:'Notes',objectTypes:[],onChange:vi.fn(),onBlur:vi.fn(),onOpenEntity:vi.fn(),onSlash:vi.fn()});
describe('Saved internal references',()=>{
 it('renders the reported legacy reference list as named links without rewriting Markdown',()=>{
  const p=props();render(<DocumentEditor {...p}/>);
  const link=screen.getByRole('link',{name:'Privacy Focused Distraction Blocking App'});
  expect(link).toHaveAttribute('href',`#/sthira/artifact/${id}`);expect(screen.getAllByRole('link')).toHaveLength(3);expect(p.onChange).not.toHaveBeenCalled();
  fireEvent.click(link);expect(p.onOpenEntity).toHaveBeenCalledWith({kind:'artifact',id});
  const editor=richEditor(screen.getByRole('textbox'));act(()=>editor.commands.insertContentAt(editor.state.doc.content.size-1,' Additional note.'));
  expect(editor.getMarkdown()).toContain(`Privacy Focused Distraction Blocking App – \`${uri}\``);
  expect(editor.getMarkdown()).toContain('sthira://artifact/1e6e33ee-987c-4854-b646-664b0d4fa3aa');
 });
 it('supports ordinary Markdown links, raw URI references and keyboard activation',()=>{
  const p=props(`[Named source](${uri})\n\n\`${uri}\`\n\nsthira://project/plan`);render(<DocumentEditor {...p}/>);
  const named=screen.getByRole('link',{name:'Named source'});expect(named).toHaveAttribute('tabindex','0');
  fireEvent.keyDown(named,{key:' '});expect(p.onOpenEntity).toHaveBeenCalledWith({kind:'artifact',id});
  fireEvent.click(named);expect(p.onOpenEntity).toHaveBeenCalledTimes(2);
  const codeLink=screen.getByRole('link',{name:'Open saved page'});expect(codeLink).toHaveAttribute('tabindex','0');fireEvent.keyDown(codeLink,{key:'Enter'});expect(p.onOpenEntity).toHaveBeenCalledTimes(3);
  fireEvent.click(screen.getByRole('link',{name:'Open saved project'}));expect(p.onOpenEntity).toHaveBeenLastCalledWith({kind:'project',id:'plan'});
 });
 it('leaves fenced code, ordinary inline code and invalid reference examples intact',()=>{
  const content=`Example: \`ordinary code\`\n\n\`\`\`text\n${uri}\n\`\`\`\n\n\`sthira://artifact/%invalid\``;const p=props(content);render(<DocumentEditor {...p}/>);
  expect(screen.queryByRole('link')).not.toBeInTheDocument();expect(screen.getByText('ordinary code').tagName).toBe('CODE');expect(richEditor(screen.getByRole('textbox')).getMarkdown()).toContain(`\`\`\`text\n${uri}`);expect(p.onChange).not.toHaveBeenCalled();
 });
 it('makes old and new references work in read-only previews without enabling unsafe URL schemes',()=>{
  const open=vi.fn();render(<RichDocumentRenderer content={`${references}\n\n[Named source](${uri})\n\n[Unsafe](javascript:alert%281%29)\n\n[External](https://example.com)`} onOpenEntity={open}/>);
  fireEvent.click(screen.getByRole('button',{name:'Privacy Focused Distraction Blocking App'}));fireEvent.click(screen.getByRole('button',{name:'Named source'}));expect(open).toHaveBeenCalledTimes(2);expect(open).toHaveBeenCalledWith({kind:'artifact',id});expect(screen.getByRole('link',{name:'External'})).toHaveAttribute('href','https://example.com');expect(document.querySelector('[href^="javascript:"]')).toBeNull();
 });
 it('keeps literal reference URLs inside fenced read-only examples as code',()=>{render(<RichDocumentRenderer content={`\`\`\`text\n${uri}\n\`\`\``} onOpenEntity={vi.fn()}/>);expect(screen.queryByRole('button',{name:'Open saved page'})).not.toBeInTheDocument();});
 it('does not trust reference attributes pasted from arbitrary HTML',()=>{
  render(<DocumentEditor {...props('Plain text')}/>);const editor=richEditor(screen.getByRole('textbox'));
  act(()=>editor.commands.insertContent('<a data-workspace-reference data-reference-markdown="untrusted" href="javascript:alert(1)">Untrusted link</a>'));
  expect(document.querySelector('[href^="javascript:"]')).toBeNull();expect(document.querySelector('a[data-workspace-reference]')).toBeNull();
 });
 it('parses canonical, legacy and copied references and rejects malformed paths',()=>{
  expect(parseEntityHref(uri)).toEqual({kind:'artifact',id});expect(parseReferenceQuery(`\`${uri}\``)).toEqual({kind:'artifact',id});expect(parseReferenceQuery(`<${uri}>`)).toEqual({kind:'artifact',id});expect(parseEntityHref('#/sthira/project/hello%20world')).toEqual({kind:'project',id:'hello world'});
  for(const url of ['sthira://artifact/a/b','sthira://artifact/%00','sthira://artifact/%2Fetc','sthira://other/id','https://example.com/sthira/artifact/a','javascript:alert(1)'])expect(parseEntityHref(url)).toBeNull();
 });
});
