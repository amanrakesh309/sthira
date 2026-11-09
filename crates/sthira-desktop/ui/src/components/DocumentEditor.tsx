import { forwardRef, useEffect, useImperativeHandle, useRef } from 'react';
import { EditorContent, NodeViewContent, NodeViewWrapper, ReactNodeViewRenderer, useEditor, type NodeViewProps } from '@tiptap/react';
import { Node, type Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import { CodeBlock } from '@tiptap/extension-code-block';
import { Markdown } from '@tiptap/markdown';
import { TableKit } from '@tiptap/extension-table';
import TaskList from '@tiptap/extension-task-list';
import TaskItem from '@tiptap/extension-task-item';
import Image from '@tiptap/extension-image';
import Blockquote from '@tiptap/extension-blockquote';
import { RichDocumentRenderer } from './RichDocumentRenderer';
import { InternalReference } from '../editor/internalReferences';
import { parseEntityHref } from '../api/connections';
import { plainPageMarkdown } from '../editor/plainPageMarkdown';
import type { EntityRef, ObjectType } from '../types/sthira';
import type { EmbeddedObjectData } from './objects/EmbeddedObjectCard';

export type DocumentFormat = 'bold' | 'italic' | 'h1' | 'h2' | 'todo' | 'code' | 'table' | 'slash';
export interface DocumentEditorHandle {
  focus: () => void;
  format: (format: DocumentFormat) => void;
  insertMarkdown: (markdown: string, replaceSlash?: boolean) => void;
  insertLink: (label: string, href: string) => void;
  startInsertion: () => { replace: (markdown: string) => void };
}
interface Props {
  content: string;
  title: string;
  onChange: (markdown: string) => void;
  onBlur: () => void;
  onOpenEntity: (entity: EntityRef) => void;
  onSlash: (query: string | null, position: { top: number; left: number }) => void;
  objectTypes: ObjectType[];
  onRecordDecision?: (text: string) => void;
  disabled?: boolean;
}

/** Keep non-Markdown HTML/comments intact when editing surrounding Markdown. */
const RawHtml = Node.create({
  name: 'rawHtml', group: 'block', atom: true,
  addAttributes: () => ({ raw: { default: '' } }),
  parseHTML: () => [{ tag: 'div[data-raw-html]' }],
  renderHTML: ({ node }) => ['div', { 'data-raw-html': '', class: 'document-raw-block' }, node.attrs.raw],
  markdownTokenName: 'html',
  parseMarkdown: (token, helpers) => token.block === false ? [] : helpers.createNode('rawHtml', { raw: token.raw || token.text || '' }),
  renderMarkdown: (node) => node.attrs?.raw || '',
});
const RawInlineHtml = Node.create({
  name: 'rawInlineHtml', group: 'inline', inline: true, atom: true,
  addAttributes: () => ({ raw: { default: '' } }),
  parseHTML: () => [{ tag: 'span[data-raw-html]' }],
  renderHTML: ({ node }) => ['span', { 'data-raw-html': '' }, node.attrs.raw],
  markdownTokenName: 'rawInlineHtml',
  markdownTokenizer: {
    name: 'rawInlineHtml', level: 'inline', start: '<',
    tokenize: (source) => {
      const match = source.match(/^<\/?[A-Za-z][\w-]*(?:\s[^<>]*?)?\/?>/);
      return match ? { type: 'rawInlineHtml', raw: match[0], text: match[0] } : undefined;
    },
  },
  parseMarkdown: (token, helpers) => helpers.createNode('rawInlineHtml', { raw: token.raw || token.text || '' }),
  renderMarkdown: (node) => node.attrs?.raw || '',
});

function removeSlash(editor: Editor) {
  const { $from, from } = editor.state.selection;
  const match = $from.parent.textBetween(0, $from.parentOffset).match(/(?:^|\s)(\/[\w-]*)$/);
  if (match) editor.commands.deleteRange({ from: from - match[1].length, to: from });
}

export const DocumentEditor = forwardRef<DocumentEditorHandle, Props>(function DocumentEditor(props, ref) {
  const latest = useRef(props); latest.current = props;
  const emitted = useRef(props.content);
  const titlePrefix = useRef('');
  const splitTitle = (content: string) => {
    const first = content.match(/^# ([^\n]+)\n*/);
    if (first && first[1].trim() === latest.current.title.trim()) {
      titlePrefix.current = first[0].endsWith('\n') ? first[0] : `${first[0]}\n\n`;
      return content.slice(first[0].length);
    }
    titlePrefix.current = ''; return content;
  };
  const notifySlash = (editor: Editor) => {
    const { $from, from, empty } = editor.state.selection;
    const match = empty && !$from.parent.type.spec.code && $from.parent.textBetween(0, $from.parentOffset).match(/(?:^|\s)\/([\w-]*)$/);
    const body = editor.view.dom.closest('.focus-editor-body')?.getBoundingClientRect();
    let position = { top: 120, left: 24 };
    if (match && body) {
      const caret = editor.view.coordsAtPos(from);
      position = { top: caret.bottom - body.top + 8, left: Math.max(0, caret.left - body.left) };
    }
    latest.current.onSlash(match ? match[1] : null, position);
  };
  const editor = useEditor({
    immediatelyRender: true,
    extensions: [
      StarterKit.configure({ codeBlock: false, blockquote: false, link: { openOnClick: false, protocols: ['sthira'] }, trailingNode: false }),
      Markdown, InternalReference, TableKit, TaskList, TaskItem.configure({ nested: true }), Image, RawHtml, RawInlineHtml,
      Blockquote.extend({
        addNodeView() {
          return ReactNodeViewRenderer(({ node }: NodeViewProps) => <NodeViewWrapper as="blockquote">
            <NodeViewContent />
            {/decision:|trade-off/i.test(node.textContent) && latest.current.onRecordDecision && <button type="button" contentEditable={false} onClick={() => latest.current.onRecordDecision?.(node.textContent)}>Add to Decisions</button>}
          </NodeViewWrapper>);
        },
      }),
      CodeBlock.extend({
        addNodeView() {
          const CodeView = ({ node, editor: viewEditor, getPos }: NodeViewProps) => {
            const language = String(node.attrs.language || '');
            if (language === 'sthira-object' || language === 'mermaid') {
              const updateObject = (data: EmbeddedObjectData) => {
                const pos = getPos(); if (pos === undefined) return;
                viewEditor.commands.insertContentAt({ from: pos, to: pos + node.nodeSize }, {
                  type: 'codeBlock', attrs: node.attrs, content: [{ type: 'text', text: JSON.stringify(data, null, 2) }],
                });
              };
              return <NodeViewWrapper className="document-embedded-block" contentEditable={false}>
                <RichDocumentRenderer content={`\`\`\`${language}\n${node.textContent}\n\`\`\``} objectTypes={latest.current.objectTypes} onUpdateObject={updateObject} onOpenEntity={latest.current.onOpenEntity} />
              </NodeViewWrapper>;
            }
            return <NodeViewWrapper>
              <pre><NodeViewContent<'code'> as="code" /></pre>
            </NodeViewWrapper>;
          };
          return ReactNodeViewRenderer(CodeView);
        },
      }),
    ],
    content: splitTitle(plainPageMarkdown(props.content)), contentType: 'markdown',
    editable: !props.disabled,
    editorProps: {
      attributes: { role: 'textbox', 'aria-label': 'Document content', 'aria-multiline': 'true', 'data-testid': 'document-editor', class: 'focus-prose document-rich-editor' },
      handlePaste: (view, event) => {
        if (view.state.selection.$from.parent.type.spec.code || !event.clipboardData) return false;
        const markdown = event.clipboardData.getData('text/markdown');
        const plain = markdown || event.clipboardData.getData('text/plain');
        if (!markdown && event.clipboardData.getData('text/html')) return false;
        if (!markdown && !plain.includes('sthira://') && !/^(?:#{1,6} |[-*+] |\d+\. |> |```|~~~|\|.+\|)/m.test(plain) && !/\[[^\]]+\]\([^)]+\)/.test(plain)) return false;
        event.preventDefault();
        editor?.commands.insertContent(plainPageMarkdown(plain), { contentType: 'markdown' });
        return true;
      },
      handleDOMEvents: { click: (_view, event) => {
        const link = (event.target as HTMLElement).closest('a');
        const entity = parseEntityHref(link?.getAttribute('href') || undefined);
        if (entity) { event.preventDefault(); latest.current.onOpenEntity(entity); return true; }
        const href = link?.getAttribute('href');
        if(href?.startsWith('sthira:')){event.preventDefault();return true;}
        if (href && /^(https?:|mailto:)/i.test(href)) { event.preventDefault(); window.open(href, '_blank', 'noopener,noreferrer'); return true; }
        return false;
      } },
      handleKeyDown: (view, event) => {
        if (event.isComposing) return false;
        if(['Enter',' '].includes(event.key)) {
          const anchor=(event.target as HTMLElement).closest('a[data-workspace-reference]');
          const entity=parseEntityHref(anchor?.getAttribute('href')||undefined);
          if(entity){event.preventDefault();latest.current.onOpenEntity(entity);return true;}
        }
        // The slash popover owns these keys while it is open.
        const { $from, empty } = view.state.selection;
        const slash = empty && !$from.parent.type.spec.code && /(?:^|\s)\/[\w-]*$/.test($from.parent.textBetween(0, $from.parentOffset));
        if (slash && ['ArrowUp', 'ArrowDown', 'Enter', 'Escape'].includes(event.key)) { event.preventDefault(); return true; }
        return false;
      },
    },
    onUpdate: ({ editor: active }) => {
      const markdown = titlePrefix.current + active.getMarkdown();
      emitted.current = markdown;
      latest.current.onChange(markdown);
      notifySlash(active);
    },
    onSelectionUpdate: ({ editor: active }) => notifySlash(active),
    onBlur: () => latest.current.onBlur(),
  });

  useEffect(() => {
    if (!editor || props.content === emitted.current) return;
    emitted.current = props.content;
    editor.commands.setContent(splitTitle(plainPageMarkdown(props.content)), { contentType: 'markdown', emitUpdate: false });
  }, [editor, props.content, props.title]);
  useEffect(() => { editor?.setEditable(!props.disabled, false); }, [editor, props.disabled]);

  useImperativeHandle(ref, () => ({
    focus: () => { editor?.commands.focus(); },
    insertMarkdown: (markdown, replaceSlash = false) => {
      if (!editor) return;
      if (replaceSlash) removeSlash(editor);
      editor.chain().focus().insertContent(plainPageMarkdown(markdown), { contentType: 'markdown' }).run();
    },
    insertLink: (label, href) => {
      editor?.chain().focus().insertContent({ type: 'text', text: label, marks: [{ type: 'link', attrs: { href } }] }).run();
    },
    startInsertion: () => {
      if (!editor) return { replace: () => {} };
      removeSlash(editor);
      const from = editor.state.selection.from;
      let to = editor.state.selection.to;
      return { replace: (markdown) => {
        if (editor.isDestroyed) return;
        const oldSize = editor.state.doc.content.size;
        editor.commands.insertContentAt({ from, to }, plainPageMarkdown(markdown), { contentType: 'markdown', updateSelection: false });
        to += editor.state.doc.content.size - oldSize;
      } };
    },
    format: (format) => {
      if (!editor) return;
      const chain = editor.chain().focus();
      switch (format) {
        case 'bold': chain.toggleBold().run(); break;
        case 'italic': chain.toggleItalic().run(); break;
        case 'h1': chain.toggleHeading({ level: 1 }).run(); break;
        case 'h2': chain.toggleHeading({ level: 2 }).run(); break;
        case 'todo': chain.toggleTaskList().run(); break;
        case 'code': chain.toggleCodeBlock().run(); break;
        case 'table': chain.insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run(); break;
        case 'slash': {
          if (editor.state.selection.$from.parent.type.spec.code) editor.chain().focus().exitCode().run();
          const { $from } = editor.state.selection;
          const before = $from.parent.textBetween(0, $from.parentOffset);
          editor.chain().focus().insertContent(before && !/\s$/.test(before) ? ' /' : '/').run();
          break;
        }
      }
    },
  }), [editor]);

  return <EditorContent editor={editor} />;
});
