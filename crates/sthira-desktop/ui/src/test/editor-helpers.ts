import { act } from '@testing-library/react';
import type { Editor } from '@tiptap/core';

/** Tiptap attaches its editor instance to the contenteditable DOM element. */
export function richEditor(element: HTMLElement): Editor {
  return (element as HTMLElement & { editor: Editor }).editor;
}
export function editorMarkdown(element: HTMLElement): string { return richEditor(element).getMarkdown(); }
export function setEditorMarkdown(element: HTMLElement, markdown: string) {
  const title = element.closest('.focus-document-sheet')?.querySelector<HTMLInputElement>('[aria-label="Document title"]')?.value;
  const first = markdown.match(/^# ([^\n]+)\n*/);
  const body = first && first[1] === title ? markdown.slice(first[0].length) : markdown;
  act(() => {
    const editor = richEditor(element);
    editor.commands.setContent(body, { contentType: 'markdown' });
    editor.commands.setTextSelection(Math.max(1, editor.state.doc.content.size - 1));
  });
}
