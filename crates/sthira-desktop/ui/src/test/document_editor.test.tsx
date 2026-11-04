import { createRef } from 'react';
import { act, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { DocumentEditor, type DocumentEditorHandle } from '../components/DocumentEditor';
import { richEditor } from './editor-helpers';

const props = (content = '# Test\n\nPlain text') => ({ content, title: 'Test', objectTypes: [], onChange: vi.fn(), onBlur: vi.fn(), onOpenEntity: vi.fn(), onSlash: vi.fn() });

describe('Single-page rich document editor', () => {
  it('renders formatted content immediately without rewriting a note on open', () => {
    const p = props('# Test\n\n## Section\n\nA **bold** statement.\n\n- One\n- Two');
    render(<DocumentEditor {...p} />);
    expect(screen.getByRole('textbox', { name: 'Document content' })).toHaveAttribute('contenteditable', 'true');
    expect(screen.getByRole('heading', { name: 'Section' })).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Test' })).toBeNull();
    expect(screen.getByText('bold').tagName).toBe('STRONG');
    expect(screen.getAllByRole('listitem')).toHaveLength(2);
    expect(p.onChange).not.toHaveBeenCalled();
  });

  it('formats the real selection, persists Markdown and supports undo', () => {
    const p = props(); const ref = createRef<DocumentEditorHandle>();
    render(<DocumentEditor ref={ref} {...p} />);
    const editor = richEditor(screen.getByRole('textbox'));
    act(() => { editor.commands.setTextSelection({ from: 1, to: 6 }); ref.current?.format('bold'); });
    expect(screen.getByText('Plain').tagName).toBe('STRONG');
    expect(p.onChange).toHaveBeenLastCalledWith('# Test\n\n**Plain** text');
    act(() => { editor.commands.undo(); });
    expect(editor.getMarkdown()).toBe('Plain text');
  });

  it('edits checklists on the same page', () => {
    const p = props('# Test\n\n- [ ] Verify connections\n- [x] Save draft');
    render(<DocumentEditor {...p} />);
    const checkboxes = screen.getAllByRole('checkbox');
    fireEvent.click(checkboxes[0]);
    expect(checkboxes[0]).toBeChecked();
    expect(p.onChange).toHaveBeenLastCalledWith(expect.stringContaining('- [x] Verify connections'));
  });

  it('inserts and follows inline references without creating another paragraph', async () => {
    const user = userEvent.setup(); const p = props(); const ref = createRef<DocumentEditorHandle>();
    render(<DocumentEditor ref={ref} {...p} />);
    const editor = richEditor(screen.getByRole('textbox'));
    act(() => { editor.commands.setTextSelection(7); ref.current?.insertLink('Source', '#/sthira/artifact/source'); });
    expect(editor.getJSON().content).toHaveLength(1);
    expect(editor.getMarkdown()).toContain('Plain [Source](#/sthira/artifact/source)text');
    await user.click(screen.getByRole('link', { name: 'Source' }));
    expect(p.onOpenEntity).toHaveBeenCalledWith({ kind: 'artifact', id: 'source' });
  });

  it('retains embedded object directives, unknown code fences and raw HTML during surrounding edits', async () => {
    const directive = JSON.stringify({ artifact_id: 'object-1', object_type_id: 'idea', title: 'An idea', properties: {} });
    const p = props(`# Test\n\nPlain text\n\n\`\`\`sthira-object\n${directive}\n\`\`\`\n\n\`\`\`custom-language\nkeep this intact\n\`\`\`\n\n<div>Retain HTML</div>`);
    render(<DocumentEditor {...p} />);
    expect(await screen.findByTestId('embedded-object-object-1')).toBeInTheDocument();
    const editor = richEditor(screen.getByRole('textbox'));
    act(() => { editor.commands.insertContentAt(6, ' edited'); });
    const saved = p.onChange.mock.lastCall?.[0] || '';
    expect(saved).toContain(`\`\`\`sthira-object\n${directive}\n\`\`\``);
    expect(saved).toContain('```custom-language\nkeep this intact\n```');
    expect(saved).toContain('<div>Retain HTML</div>');
  });

  it('keeps selection stable when a parent echoes an autosaved edit', () => {
    const p = props(); const { rerender } = render(<DocumentEditor {...p} />);
    const editor = richEditor(screen.getByRole('textbox'));
    act(() => { editor.commands.insertContentAt(6, ' changed'); });
    const position = editor.state.selection.from;
    rerender(<DocumentEditor {...p} content={p.onChange.mock.lastCall![0]} />);
    expect(editor.state.selection.from).toBe(position);
    expect(screen.getByRole('textbox')).toHaveTextContent('Plain changed text');
  });

  it('keeps a title-only note separated from newly typed body content', () => {
    const p = props('# Test'); render(<DocumentEditor {...p} />);
    act(() => { richEditor(screen.getByRole('textbox')).commands.insertContent('Body'); });
    expect(p.onChange).toHaveBeenLastCalledWith('# Test\n\nBody');
  });

  it('replaces an inline AI stream at its captured location without losing surrounding content', () => {
    const p = props('# Test\n\nBefore after'); const ref = createRef<DocumentEditorHandle>();
    render(<DocumentEditor ref={ref} {...p} />);
    const editor = richEditor(screen.getByRole('textbox'));
    act(() => { editor.commands.setTextSelection(8); });
    const insertion = ref.current!.startInsertion();
    act(() => { insertion.replace('First draft'); insertion.replace('Final answer'); });
    expect(editor.getText()).toContain('Before'); expect(editor.getText()).toContain('after');
    expect(editor.getText()).toContain('Final answer'); expect(editor.getText()).not.toContain('First draft');
  });

  it('round-trips editable GFM tables', () => {
    const p = props('# Test\n\n| Name | State |\n| --- | --- |\n| Notes | Linked |');
    render(<DocumentEditor {...p} />);
    const editor = richEditor(screen.getByRole('textbox'));
    expect(screen.getByRole('table')).toBeInTheDocument();
    act(() => { editor.commands.insertContentAt(4, 'New '); });
    expect(p.onChange).toHaveBeenLastCalledWith(expect.stringContaining('Linked'));
    expect(editor.getMarkdown()).toContain('|');
  });

  it('pastes Markdown as editable formatted blocks while preserving paste inside code blocks', () => {
    const p = props('# Test\n\n'); render(<DocumentEditor {...p} />);
    const element = screen.getByRole('textbox');
    fireEvent.paste(element, { clipboardData: { getData: (type: string) => type === 'text/plain' ? '## Pasted heading\n\n- [ ] Pasted task' : '' } });
    expect(screen.getByRole('heading', { name: 'Pasted heading' })).toBeInTheDocument();
    expect(screen.getByRole('checkbox')).not.toBeChecked();
    expect(p.onChange).toHaveBeenLastCalledWith(expect.stringContaining('## Pasted heading'));
  });

  it('preserves inline HTML without turning it into an invalid block inside a paragraph', () => {
    const p = props('# Test\n\nBefore <span>inline text</span> after');
    render(<DocumentEditor {...p} />);
    const editor = richEditor(screen.getByRole('textbox'));
    act(() => { editor.commands.insertContentAt(1, 'Edited '); });
    expect(p.onChange).toHaveBeenLastCalledWith(expect.stringContaining('Before <span>inline text</span> after'));
    expect(editor.getJSON().content?.[0].type).toBe('paragraph');
  });
});
