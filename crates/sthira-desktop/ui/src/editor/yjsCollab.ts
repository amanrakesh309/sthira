import { Editor, mergeAttributes } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import { CodeBlock } from '@tiptap/extension-code-block';
import { Markdown } from '@tiptap/markdown';
import { TableKit } from '@tiptap/extension-table';
import TaskList from '@tiptap/extension-task-list';
import TaskItem from '@tiptap/extension-task-item';
import Image from '@tiptap/extension-image';
import Link from '@tiptap/extension-link';
import {
  absolutePositionToRelativePosition,
  prosemirrorToYXmlFragment,
  ySyncPluginKey,
} from '@tiptap/y-tiptap';
import * as Y from 'yjs';
import type { Awareness } from 'y-protocols/awareness';
import type { CollaboratorPresence } from '../api/liveCollab';
import { plainPageMarkdown } from './plainPageMarkdown';

// Yjs stores attributes on XML elements and marks on text. A link wrapped
// around an image therefore needs to travel with the image element.
export const CollaborativeImage = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      linkHref: {
        default: null,
        rendered: false,
        parseHTML: (element: HTMLElement) => element.closest('a')?.getAttribute('href'),
      },
      linkTitle: {
        default: null,
        rendered: false,
        parseHTML: (element: HTMLElement) => element.closest('a')?.getAttribute('title'),
      },
    };
  },
  renderHTML({ node, HTMLAttributes }) {
    const attributes = mergeAttributes(this.options.HTMLAttributes, HTMLAttributes);
    const { linkHref, linkTitle } = node.attrs;
    if (typeof linkHref !== 'string' || !/^(?:https?:\/\/|mailto:|#(?!\/sthira))/i.test(linkHref)) {
      return ['img', attributes];
    }
    return ['a', { href: linkHref, title: linkTitle, tabindex: '0' }, ['img', attributes]];
  },
  renderMarkdown(node) {
    const { src = '', alt = '', title, linkHref, linkTitle } = node.attrs ?? {};
    const escape = (value: string, pattern: RegExp) => value.replace(pattern, '\\$&');
    const target = (url: string, label?: string) => {
      const destination = /[()<>\\\s]/.test(url) ? `<${escape(url, /[<>\\]/g)}>` : url;
      return label ? `${destination} "${escape(label, /["\\]/g)}"` : destination;
    };
    const image = `![${escape(alt ?? '', /[[\]\\]/g)}](${target(src ?? '', title)})`;
    return linkHref ? `[${image}](${target(linkHref, linkTitle)})` : image;
  },
}).configure({ inline: true });

export const CollaborativeLink = Link.extend({
  parseMarkdown(token, helpers) {
    const attrs = { href: token.href, title: token.title || null };
    const content = helpers.parseInline(token.tokens ?? []).map((node) =>
      node.type === 'image'
        ? { ...node, attrs: { ...node.attrs, linkHref: attrs.href, linkTitle: attrs.title } }
        : node,
    );
    return helpers.applyMark('link', content, attrs);
  },
});

export const CollaborativeCodeBlock = CodeBlock.extend({
  addAttributes() {
    return { ...this.parent?.(), fenceMeta: { default: null, rendered: false } };
  },
  parseMarkdown(token, helpers) {
    const [, language, metadata] = /^([\S]+)(?:\s+(.*))?$/.exec(token.lang?.trim() ?? '') ?? [];
    return helpers.createNode(
      'codeBlock',
      { language: language || null, fenceMeta: metadata || null },
      token.text ? [helpers.createTextNode(token.text)] : [],
    );
  },
  renderMarkdown(node) {
    const info = [node.attrs?.language, node.attrs?.fenceMeta].filter(Boolean).join(' ');
    const body = node.content?.map((child) => child.text ?? '').join('') ?? '';
    const delimiter = info.includes('`') ? '~' : '`';
    const runs = body.match(delimiter === '~' ? /~+/g : /`+/g) ?? [];
    const fence = delimiter.repeat(Math.max(2, ...runs.map((run) => run.length)) + 1);
    return `${fence}${info}\n${body}\n${fence}`;
  },
});

export function uint8ToBase64(bytes: Uint8Array): string {
  if (bytes.length === 0) return '';
  let binary = '';
  const chunkSize = 0x8000;
  for (let i = 0; i < bytes.length; i += chunkSize) {
    const slice = bytes.subarray(i, Math.min(i + chunkSize, bytes.length));
    binary += String.fromCharCode(...slice);
  }
  return btoa(binary);
}

export function base64ToUint8(b64: string): Uint8Array {
  const cleaned = b64.trim();
  if (!cleaned) return new Uint8Array(0);
  const binary = atob(cleaned);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

/**
 * Produces an initial Yjs v1 update containing `Y.Text("title")` and
 * TipTap-compatible `Y.XmlFragment("default")` from published Markdown.
 */
export function seedInitialYjsUpdateB64(title: string, contentMd: string): string {
  const ydoc = new Y.Doc();
  const host = document.createElement('div');
  let tempEditor: Editor | null = null;
  try {
    tempEditor = new Editor({
      element: host,
      extensions: [
        StarterKit.configure({
          codeBlock: false,
          link: false,
          trailingNode: false,
        }),
        CollaborativeCodeBlock,
        Markdown,
        TableKit,
        TaskList,
        TaskItem.configure({ nested: true }),
        CollaborativeImage,
        CollaborativeLink,
      ],
      content: plainPageMarkdown(contentMd || ''),
      contentType: 'markdown',
    });
    const titleText = ydoc.getText('title');
    if (title) {
      titleText.insert(0, title);
    }
    const frag = ydoc.getXmlFragment('default');
    prosemirrorToYXmlFragment(tempEditor.state.doc, frag);
    return uint8ToBase64(Y.encodeStateAsUpdate(ydoc));
  } finally {
    tempEditor?.destroy();
    ydoc.destroy();
  }
}

/** Applies a minimal prefix/suffix splice so concurrent title edits preserve peer characters. */
export function syncStringToYText(yText: Y.Text, nextValue: string, origin: string): void {
  const doc = yText.doc;
  if (!doc) return;
  const current = yText.toString();
  if (current === nextValue) return;

  let start = 0;
  const minLen = Math.min(current.length, nextValue.length);
  while (start < minLen && current.charCodeAt(start) === nextValue.charCodeAt(start)) {
    start += 1;
  }

  let curEnd = current.length;
  let nextEnd = nextValue.length;
  while (
    curEnd > start &&
    nextEnd > start &&
    current.charCodeAt(curEnd - 1) === nextValue.charCodeAt(nextEnd - 1)
  ) {
    curEnd -= 1;
    nextEnd -= 1;
  }

  const deleteCount = curEnd - start;
  const insertText = nextValue.slice(start, nextEnd);
  doc.transact(() => {
    if (deleteCount > 0) {
      yText.delete(start, deleteCount);
    }
    if (insertText.length > 0) {
      yText.insert(start, insertText);
    }
  }, origin);
}

function numericClientId(connectionId: string, selfClientId: number): number {
  let h = 2166136261;
  for (let i = 0; i < connectionId.length; i += 1) {
    h ^= connectionId.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  const id = (h & 0x7fffffff) || 1;
  return id === selfClientId ? ((id + 1) & 0x7fffffff) || 2 : id;
}

export function encodeSelectionRelativePositions(
  editor: Editor | null,
  from: number,
  to: number,
): { relAnchor: string | null; relHead: string | null } {
  if (!editor || editor.isDestroyed) {
    return { relAnchor: null, relHead: null };
  }
  const ystate = ySyncPluginKey.getState(editor.state);
  if (!ystate || !ystate.binding || ystate.binding.mapping.size === 0) {
    return { relAnchor: null, relHead: null };
  }
  const maxPos = editor.state.doc.content.size;
  const clampedAnchor = Math.max(0, Math.min(from, maxPos));
  const clampedHead = Math.max(0, Math.min(to, maxPos));
  try {
    const relA = absolutePositionToRelativePosition(
      clampedAnchor,
      ystate.type,
      ystate.binding.mapping,
    );
    const relH = absolutePositionToRelativePosition(
      clampedHead,
      ystate.type,
      ystate.binding.mapping,
    );
    return {
      relAnchor: JSON.stringify(Y.relativePositionToJSON(relA)),
      relHead: JSON.stringify(Y.relativePositionToJSON(relH)),
    };
  } catch {
    return { relAnchor: null, relHead: null };
  }
}

function tryDecodeRelativePositionJson(raw: string | null | undefined): ReturnType<typeof Y.relativePositionToJSON> | null {
  if (!raw || !raw.trim()) return null;
  try {
    const parsed = JSON.parse(raw) as unknown;
    Y.createRelativePositionFromJSON(parsed);
    return parsed as ReturnType<typeof Y.relativePositionToJSON>;
  } catch {
    return null;
  }
}

/**
 * Mirrors server-broadcast per-connection presence into TipTap's `Awareness` instance
 * so `CollaborationCaret` renders collaborator carets + display-name labels cleanly.
 */
export function syncRemoteAwarenessPositions(
  editor: Editor | null,
  awareness: Awareness,
  myConnectionId: string | null,
  participants: CollaboratorPresence[],
): void {
  const states = awareness.states;
  const keepIds = new Set<number>([awareness.clientID]);
  const added: number[] = [];
  const updated: number[] = [];
  const removed: number[] = [];

  const ystate = editor && !editor.isDestroyed ? ySyncPluginKey.getState(editor.state) : null;
  const maxPos = editor && !editor.isDestroyed ? editor.state.doc.content.size : 0;

  for (const p of participants) {
    if (myConnectionId && p.connectionId === myConnectionId) {
      continue;
    }
    const cid = numericClientId(p.connectionId, awareness.clientID);
    keepIds.add(cid);
    const prevState = states.get(cid);
    const existed = prevState !== undefined;
    let cursor: { anchor: ReturnType<typeof Y.relativePositionToJSON>; head: ReturnType<typeof Y.relativePositionToJSON> } | null = null;
    const relAnchorJson = tryDecodeRelativePositionJson(p.relAnchor);
    const relHeadJson = tryDecodeRelativePositionJson(p.relHead);
    if (relAnchorJson && relHeadJson) {
      cursor = {
        anchor: relAnchorJson,
        head: relHeadJson,
      };
    } else if (
      ystate &&
      ystate.binding &&
      ystate.binding.mapping.size > 0 &&
      typeof p.anchor === 'number' &&
      typeof p.head === 'number'
    ) {
      const clampedAnchor = Math.max(0, Math.min(p.anchor, maxPos));
      const clampedHead = Math.max(0, Math.min(p.head, maxPos));
      try {
        const relAnchor = absolutePositionToRelativePosition(
          clampedAnchor,
          ystate.type,
          ystate.binding.mapping,
        );
        const relHead = absolutePositionToRelativePosition(
          clampedHead,
          ystate.type,
          ystate.binding.mapping,
        );
        cursor = {
          anchor: Y.relativePositionToJSON(relAnchor),
          head: Y.relativePositionToJSON(relHead),
        };
      } catch {
        cursor = null;
      }
    }
    const nextState = {
      user: { name: p.displayName, color: p.color, connectionId: p.connectionId },
      ...(cursor ? { cursor } : {}),
    };
    const changed = !existed || JSON.stringify(prevState) !== JSON.stringify(nextState);
    if (changed) {
      states.set(cid, nextState);
      if (existed) updated.push(cid);
      else added.push(cid);
    }
  }

  for (const existingId of Array.from(states.keys())) {
    if (!keepIds.has(existingId)) {
      states.delete(existingId);
      removed.push(existingId);
    }
  }

  if (added.length > 0 || updated.length > 0 || removed.length > 0) {
    awareness.emit('update', [{ added, updated, removed }, 'remote']);
    awareness.emit('change', [{ added, updated, removed }, 'remote']);
  }
}
