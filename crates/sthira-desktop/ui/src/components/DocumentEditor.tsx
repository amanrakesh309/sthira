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
import Collaboration from '@tiptap/extension-collaboration';
import CollaborationCaret from '@tiptap/extension-collaboration-caret';
import type * as Y from 'yjs';
import type { Awareness } from 'y-protocols/awareness';
import { RichDocumentRenderer } from './RichDocumentRenderer';
import { InternalReference } from '../editor/internalReferences';
import { parseEntityHref } from '../api/connections';
import { plainPageMarkdown } from '../editor/plainPageMarkdown';
import { SLASH_QUERY_PATTERN } from '../editor/slashQuery';
import { CollaborativeCodeBlock, CollaborativeImage, CollaborativeLink } from '../editor/yjsCollab';
import type { EntityRef, ObjectType } from '../types/sthira';
import type { EmbeddedObjectData } from './objects/EmbeddedObjectCard';

const documentLinkOptions = {
  openOnClick: false,
  protocols: ['sthira'],
  HTMLAttributes: {
    target: '_blank',
    rel: 'noopener noreferrer nofollow',
    class: 'document-inline-link',
    tabindex: '0',
  },
};

export type DocumentFormat = 'bold' | 'italic' | 'h1' | 'h2' | 'todo' | 'code' | 'table' | 'slash';
export interface DocumentEditorHandle {
  focus: () => void;
  format: (format: DocumentFormat) => void;
  insertMarkdown: (markdown: string, replaceSlash?: boolean) => void;
  insertLink: (label: string, href: string) => void;
  startInsertion: () => { replace: (markdown: string) => void };
  undo: () => boolean;
  redo: () => boolean;
  getEditor: () => Editor | null;
}
export interface DocumentCollabConfig {
  ydoc: Y.Doc;
  awareness: Awareness;
  user: { name: string; color: string };
  field?: string;
}
interface Props {
  content: string;
  title: string;
  onChange: (markdown: string) => void;
  onBlur: () => void;
  onOpenEntity: (entity: EntityRef) => void;
  onSlash: (query: string | null, position: { top: number; left: number }) => void;
  slashOpen?: boolean;
  objectTypes: ObjectType[];
  onRecordDecision?: (text: string) => void;
  disabled?: boolean;
  collab?: DocumentCollabConfig | null;
  onSelectionRange?: (from: number, to: number) => void;
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
  const match = $from.parent.textBetween(0, $from.parentOffset).match(SLASH_QUERY_PATTERN);
  if (match) editor.commands.deleteRange({ from: from - match[1].length - 1, to: from });
}

export const DocumentEditor = forwardRef<DocumentEditorHandle, Props>(function DocumentEditor(props, ref) {
  const latest = useRef(props); latest.current = props;
  const emitted = useRef(props.content);
  const titlePrefix = useRef('');
  const prevSlashOpenRef = useRef(!!props.slashOpen);
  const dismissedSlashPosRef = useRef<number | null>(null);
  const splitTitle = (content: string) => {
    const first = content.match(/^# ([^\n]+)\n*/);
    if (first && first[1].trim() === latest.current.title.trim()) {
      titlePrefix.current = first[0].endsWith('\n') ? first[0] : `${first[0]}\n\n`;
      return content.slice(first[0].length);
    }
    titlePrefix.current = ''; return content;
  };
  const notifySlash = (editor: Editor, source: 'update' | 'selection') => {
    const { $from, from, empty } = editor.state.selection;
    const match = empty && !$from.parent.type.spec.code && $from.parent.textBetween(0, $from.parentOffset).match(SLASH_QUERY_PATTERN);
    if (!match) {
      if (source === 'update' && dismissedSlashPosRef.current !== null) {
        const docSize = editor.state.doc.content.size;
        const pos = dismissedSlashPosRef.current;
        if (pos >= docSize || editor.state.doc.textBetween(pos, Math.min(pos + 1, docSize)) !== '/') {
          dismissedSlashPosRef.current = null;
        }
      }
      latest.current.onSlash(null, { top: 120, left: 24 });
      return;
    }
    const slashPos = from - match[1].length - 1;
    if (dismissedSlashPosRef.current !== null) {
      if (source === 'update' && slashPos !== dismissedSlashPosRef.current) {
        dismissedSlashPosRef.current = null;
      } else if (slashPos === dismissedSlashPosRef.current) {
        latest.current.onSlash(null, { top: 120, left: 24 });
        return;
      }
    }
    const body = editor.view.dom.closest('.focus-editor-body')?.getBoundingClientRect();
    let position = { top: 120, left: 24 };
    if (body) {
      const caret = editor.view.coordsAtPos(from);
      position = { top: caret.bottom - body.top + 8, left: Math.max(0, caret.left - body.left) };
    }
    latest.current.onSlash(match[1], position);
  };
  const editor = useEditor(
    {
      immediatelyRender: true,
      extensions: [
        StarterKit.configure({
          codeBlock: false,
          blockquote: false,
          ...(props.collab ? { undoRedo: false } : {}),
          link: props.collab ? false : documentLinkOptions,
          trailingNode: false,
        }),
        Markdown, InternalReference, TableKit, TaskList, TaskItem.configure({ nested: true }),
        props.collab ? CollaborativeImage : Image, RawHtml, RawInlineHtml,
        ...(props.collab ? [CollaborativeLink.configure(documentLinkOptions)] : []),
        Blockquote.extend({
          addNodeView() {
            return ReactNodeViewRenderer(({ node }: NodeViewProps) => <NodeViewWrapper as="blockquote">
              <NodeViewContent />
              {/decision:|trade-off/i.test(node.textContent) && latest.current.onRecordDecision && <button type="button" contentEditable={false} onClick={() => latest.current.onRecordDecision?.(node.textContent)}>Add to Decisions</button>}
            </NodeViewWrapper>);
          },
        }),
        (props.collab ? CollaborativeCodeBlock : CodeBlock).extend({
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
        ...(props.collab
          ? [
              Collaboration.configure({
                document: props.collab.ydoc,
                field: props.collab.field ?? 'default',
              }),
              CollaborationCaret.configure({
                provider: { awareness: props.collab.awareness },
                user: props.collab.user,
                render: (user: Record<string, unknown>) => {
                  const color = typeof user.color === 'string' && user.color ? user.color : '#0B6E4F';
                  const displayName =
                    typeof user.name === 'string' && user.name ? user.name : 'Collaborator';
                  const cursor = document.createElement('span');
                  cursor.classList.add('collaboration-carets__caret');
                  cursor.style.borderColor = color;
                  const label = document.createElement('div');
                  label.classList.add('collaboration-carets__label');
                  label.style.backgroundColor = color;
                  label.setAttribute('data-collaborator-caret', displayName);
                  label.textContent = displayName;
                  cursor.appendChild(label);
                  return cursor;
                },
              }),
            ]
          : []),
      ],
      ...(props.collab
        ? {}
        : { content: splitTitle(plainPageMarkdown(props.content)), contentType: 'markdown' }),
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
          if (['Enter', ' '].includes(event.key) && !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey) {
            const anchor = (event.target as HTMLElement)?.closest?.('a');
            if (anchor && anchor !== view.dom && !anchor.closest('pre')) {
              const href = anchor.getAttribute('href') || undefined;
              const entity = parseEntityHref(href);
              if (entity) {
                event.preventDefault();
                latest.current.onOpenEntity(entity);
                return true;
              }
              if (href?.startsWith('sthira:')) {
                event.preventDefault();
                return true;
              }
              if (href && /^(https?:|mailto:)/i.test(href)) {
                event.preventDefault();
                window.open(href, '_blank', 'noopener,noreferrer');
                return true;
              }
            }
          }
          // The slash popover owns these keys only while it is actively open.
          if (latest.current.slashOpen && ['ArrowUp', 'ArrowDown', 'Enter', 'Escape'].includes(event.key)) {
            event.preventDefault();
            return true;
          }
          return false;
        },
      },
      onUpdate: ({ editor: active }) => {
        const markdown = latest.current.collab ? active.getMarkdown() : titlePrefix.current + active.getMarkdown();
        emitted.current = markdown;
        latest.current.onChange(markdown);
        notifySlash(active, 'update');
      },
      onSelectionUpdate: ({ editor: active }) => {
        notifySlash(active, 'selection');
        latest.current.onSelectionRange?.(active.state.selection.from, active.state.selection.to);
      },
      onBlur: () => latest.current.onBlur(),
    },
    [props.collab?.ydoc ?? null, props.collab?.awareness ?? null],
  );

  if (prevSlashOpenRef.current && !props.slashOpen && editor && !editor.isDestroyed) {
    const { $from, from, empty } = editor.state.selection;
    const match = empty && !$from.parent.type.spec.code && $from.parent.textBetween(0, $from.parentOffset).match(SLASH_QUERY_PATTERN);
    if (match) {
      dismissedSlashPosRef.current = from - match[1].length - 1;
    }
  }
  prevSlashOpenRef.current = !!props.slashOpen;

  useEffect(() => {
    if (!editor || latest.current.collab || props.content === emitted.current) return;
    emitted.current = props.content;
    dismissedSlashPosRef.current = null;
    editor.commands.setContent(splitTitle(plainPageMarkdown(props.content)), { contentType: 'markdown', emitUpdate: false });
  }, [editor, props.content, props.title]);
  useEffect(() => { editor?.setEditable(!props.disabled, false); }, [editor, props.disabled]);

  useImperativeHandle(ref, () => ({
    focus: () => { editor?.commands.focus(); },
    undo: () => {
      if (!editor) return false;
      return editor.commands.undo();
    },
    redo: () => {
      if (!editor) return false;
      return editor.commands.redo();
    },
    getEditor: () => editor ?? null,
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
          dismissedSlashPosRef.current = null;
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
