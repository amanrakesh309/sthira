import { act, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { DocumentEditor } from '../components/DocumentEditor';
import { RichDocumentRenderer } from '../components/RichDocumentRenderer';
import { plainPageMarkdown } from '../editor/plainPageMarkdown';
import { richEditor } from './editor-helpers';
import { SLASH_COMMANDS } from '../components/SlashCommandMenu';

const fence = (kind: string, data: unknown) => `\`\`\`widget:${kind}\n${JSON.stringify(data)}\n\`\`\``;
const plan = {title:'Investigate the approach',milestones:['Collect evidence','Review assumptions'],prerequisites:['Define the question'],completed:[0],source:'[Evidence](https://example.com)'};
const choice = {id:'approach',question:'Which direction?',options:['Explore locally','Use a service'],recommended:'Explore locally',selected:'Use a service'};
const body = `# Test\n\nBefore\n\n${fence('plan',plan)}\n\n${fence('decision',choice)}\n\nAfter`;
const props = (content=body) => ({content,title:'Test',objectTypes:[],onChange:vi.fn(),onBlur:vi.fn(),onOpenEntity:vi.fn(),onSlash:vi.fn()});

describe('Rich text without generative UI',()=>{
  it('shows saved blocks as ordinary editable text without writing on open; edits save readable Markdown',()=>{
    const p=props(); const view=render(<DocumentEditor {...p}/>);
    expect(screen.getByRole('heading',{name:plan.title})).toBeInTheDocument();
    expect(screen.queryByText('Block details')).not.toBeInTheDocument();
    expect(document.querySelector('[data-page-block]')).toBeNull();
    expect(p.onChange).not.toHaveBeenCalled();
    const editor=richEditor(screen.getByRole('textbox',{name:'Document content'}));
    act(()=>{editor.commands.insertContentAt(1,'Edited ');});
    const saved=p.onChange.mock.lastCall![0] as string;
    expect(saved).not.toContain('widget:');
    expect(saved).toContain('- [x] Collect evidence');
    expect(saved).toContain('Selected'); expect(saved).toContain('Use a service');
    expect(saved).toContain('https://example.com'); expect(saved).toContain('After');
    view.unmount(); render(<DocumentEditor {...props(saved)}/>);
    expect(screen.getByRole('heading',{name:plan.title})).toBeInTheDocument();
    expect(document.querySelector('[data-page-block]')).toBeNull();
  });
  it('renders previews as selectable normal prose with links and no choice controls',()=>{
    const view=render(<RichDocumentRenderer content={body}/>);
    expect(view.container.textContent).toContain('Selected: Use a service');
    expect(screen.queryByRole('button',{name:/Use a service/})).not.toBeInTheDocument();
    expect(screen.getByRole('link',{name:'Evidence'})).toHaveAttribute('href','https://example.com');
    expect(view.container.querySelector('[data-page-block]')).toBeNull();
  });
  it('preserves malformed, unsupported and future-version fences literally on surrounding edits',()=>{
    const raw='```widget:plan\n{bad}\n```\n\n'+fence('decision',{...choice,version:2})+'\n\n```widget:calculator\nunknown output\n```';
    const p=props('# Test\n\nBefore\n\n'+raw);render(<DocumentEditor {...p}/>);
    const editor=richEditor(screen.getByRole('textbox',{name:'Document content'}));
    act(()=>{editor.commands.insertContentAt(1,'Edited ');});
    expect(p.onChange.mock.lastCall![0]).toContain(raw);
  });
  it('does not convert example fences inside longer code fences or incomplete streamed fences',()=>{
    const example='````markdown\n'+fence('plan',plan)+'\n````';
    expect(plainPageMarkdown(example)).toBe(example);
    const partial='```widget:plan\n'+JSON.stringify(plan);
    expect(plainPageMarkdown(partial)).toBe(partial);
  });
  it('removes widget slash actions while keeping standard rich-text formatting',()=>{
    expect(SLASH_COMMANDS.some(item=>item.id.startsWith('interactive-'))).toBe(false);
    expect(SLASH_COMMANDS.map(item=>item.id)).toEqual(expect.arrayContaining(['h1','todo','table']));
  });
});
