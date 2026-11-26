import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import type { ArtifactContext } from '../types/sthira';
import type { PageOriginKind, ShareableProvenance } from './workspaces';

export const MAX_SHARED_PAGE_TITLE_LEN = 160;
export const MAX_SHARED_PAGE_CONTENT_BYTES = 48 * 1024;
export const MAX_SHARED_COMMENT_BYTES = 8 * 1024;
export const MAX_SHARED_PROPOSAL_SUMMARY_LEN = 280;

export interface SanitizedPersonalCopyPreview {
  sourceContentHash: string;
  sourceVersion: number;
  title: string;
  sanitizedMarkdown: string;
  byteLength: number;
  maxBytes: number;
  exceedsLimit: boolean;
  provenance: ShareableProvenance;
  omissions: string[];
  hasOmissions: boolean;
}

type DestinationClass =
  | 'allowed'
  | 'personalVault'
  | 'localAttachment'
  | 'unsafeScheme';

interface MdastPositionPoint {
  offset?: number;
}

interface MdastNode {
  type: string;
  value?: string;
  url?: string;
  alt?: string | null;
  identifier?: string;
  label?: string;
  children?: MdastNode[];
  position?: {
    start?: MdastPositionPoint;
    end?: MdastPositionPoint;
  };
}

interface TextReplacement {
  start: number;
  end: number;
  replacement: string;
}

const markdownParser = unified().use(remarkParse).use(remarkGfm);

export function utf8ByteLength(text: string): number {
  return new TextEncoder().encode(text).length;
}

export function classifySharedDestination(
  rawUrl: string | undefined | null,
  kind: 'link' | 'image',
): DestinationClass {
  const decoded = (rawUrl ?? '').trim();
  if (!decoded) return 'localAttachment';

  // Strip ASCII control/whitespace before checking scheme (matches browser URL normalization).
  const schemeCompact = decoded.replace(/[\u0000-\u0020\u007f]+/g, '');
  const lower = schemeCompact.toLowerCase();

  if (lower.startsWith('sthira:') || lower.startsWith('#/sthira')) {
    return 'personalVault';
  }
  if (decoded.startsWith('#')) {
    return kind === 'link' ? 'allowed' : 'localAttachment';
  }

  const colonIdx = schemeCompact.indexOf(':');
  const firstDelim = schemeCompact.search(/[/?#]/);
  const hasScheme = colonIdx > 0 && (firstDelim < 0 || colonIdx < firstDelim);
  if (hasScheme) {
    const scheme = lower.slice(0, colonIdx);
    if (/^[a-z][a-z0-9+.-]*$/.test(scheme)) {
      if (scheme === 'https' || scheme === 'http') {
        try {
          const parsed = new URL(decoded);
          return parsed.hostname ? 'allowed' : 'unsafeScheme';
        } catch {
          return 'unsafeScheme';
        }
      }
      if (scheme === 'mailto' && kind === 'link') {
        return schemeCompact.slice(colonIdx + 1).length > 0 ? 'allowed' : 'localAttachment';
      }
      if (scheme === 'file') return 'localAttachment';
      return 'unsafeScheme';
    }
  }

  // Relative or absolute attachment path (`/foo`, `./bar`, `attachments/a.png`, `C:\...`, `//host`).
  return 'localAttachment';
}

function plainTextOfNode(node: MdastNode): string {
  if (typeof node.value === 'string') return node.value;
  if (!Array.isArray(node.children)) return '';
  return node.children.map(plainTextOfNode).join('');
}

function collectDefinitions(root: MdastNode): {
  destinations: Map<string, string>;
  defNodes: MdastNode[];
} {
  const destinations = new Map<string, string>();
  const defNodes: MdastNode[] = [];
  const walk = (node: MdastNode) => {
    if (node.type === 'definition' && node.identifier) {
      const id = node.identifier.toLowerCase();
      if (!destinations.has(id)) destinations.set(id, node.url ?? '');
      defNodes.push(node);
      return;
    }
    if (node.type === 'code' || node.type === 'inlineCode') return;
    for (const child of node.children ?? []) walk(child);
  };
  walk(root);
  return { destinations, defNodes };
}

function applyReplacements(markdown: string, replacements: TextReplacement[]): string {
  if (replacements.length === 0) return markdown;
  const sorted = [...replacements].sort((a, b) => a.start - b.start || a.end - b.end);
  const nonOverlapping: TextReplacement[] = [];
  let lastEnd = -1;
  for (const item of sorted) {
    if (item.start >= lastEnd && item.start >= 0 && item.end <= markdown.length && item.start <= item.end) {
      nonOverlapping.push(item);
      lastEnd = item.end;
    }
  }
  let out = '';
  let cursor = 0;
  for (const rep of nonOverlapping) {
    out += markdown.slice(cursor, rep.start) + rep.replacement;
    cursor = rep.end;
  }
  return out + markdown.slice(cursor);
}

export function validateSharedPageInput(title: string, contentMd: string): string | null {
  const trimmedTitle = title.trim();
  if (!trimmedTitle) return 'Page title is required.';
  if (trimmedTitle.length > MAX_SHARED_PAGE_TITLE_LEN) {
    return `Page title must be at most ${MAX_SHARED_PAGE_TITLE_LEN} characters.`;
  }
  const normalized = contentMd.replace(/\r\n/g, '\n');
  if (!normalized.trim()) return 'Page content cannot be empty.';
  const bytes = utf8ByteLength(normalized);
  if (bytes > MAX_SHARED_PAGE_CONTENT_BYTES) {
    return `Shared content is ${bytes.toLocaleString()} bytes, exceeding the ${MAX_SHARED_PAGE_CONTENT_BYTES / 1024} KiB limit. Shorten the content and try again; your draft stays intact.`;
  }

  const tree = markdownParser.parse(normalized) as unknown as MdastNode;
  const { destinations } = collectDefinitions(tree);
  let violation: string | null = null;

  const visit = (node: MdastNode) => {
    if (violation) return;
    if (node.type === 'code' || node.type === 'inlineCode') return;

    if (node.type === 'definition') {
      const cls = classifySharedDestination(node.url, 'link');
      if (cls === 'personalVault') {
        violation =
          'Personal vault links (sthira:// or #/sthira/) cannot be published directly. Remove or convert them to plain text first.';
        return;
      }
      if (cls === 'localAttachment') {
        violation =
          'Local file links, attachment paths, and private references are not supported in shared pages.';
        return;
      }
      if (cls === 'unsafeScheme') {
        violation = 'Unsupported or unsafe URL schemes cannot be published to shared pages.';
        return;
      }
      return;
    }

    if (node.type === 'link') {
      const cls = classifySharedDestination(node.url, 'link');
      if (cls === 'personalVault') {
        violation =
          'Personal vault links (sthira:// or #/sthira/) cannot be published directly. Remove or convert them to plain text first.';
        return;
      }
      if (cls === 'localAttachment') {
        violation =
          'Local file links, attachment paths, and script links are not supported in shared pages.';
        return;
      }
      if (cls === 'unsafeScheme') {
        violation = 'Unsupported or unsafe URL schemes cannot be published to shared pages.';
        return;
      }
    }

    if (node.type === 'image') {
      const cls = classifySharedDestination(node.url, 'image');
      if (cls !== 'allowed') {
        violation =
          cls === 'personalVault'
            ? 'Personal vault image links cannot be published directly.'
            : 'Images in shared pages must use public https:// or http:// URLs.';
        return;
      }
    }

    if (node.type === 'linkReference' || node.type === 'imageReference') {
      const destination = node.identifier && destinations.get(node.identifier.toLowerCase());
      const defCls = destination !== undefined ? classifySharedDestination(destination, node.type === 'imageReference' ? 'image' : 'link') : undefined;
      if (defCls && defCls !== 'allowed') {
        violation =
          defCls === 'personalVault'
            ? 'Personal vault reference links must be omitted before publishing.'
            : 'Reference-style local file links and unsupported URLs are not supported in shared pages.';
        return;
      }
    }

    if (node.type === 'html') {
      violation = 'Raw HTML is unsupported in shared pages. Use Markdown formatting.';
      return;
    }

    if (node.type === 'text' && typeof node.value === 'string') {
      if (/(?:sthira:\/\/|#\/sthira\/)/i.test(node.value)) {
        violation =
          'Private vault references (sthira:// or #/sthira/) cannot be published directly. Remove or convert them to plain text first.';
        return;
      }
    }

    for (const child of node.children ?? []) {
      visit(child);
    }
  };

  visit(tree);
  return violation;
}

export function sanitizePersonalPageForShare(
  ctx: ArtifactContext,
  authorLabel?: string,
): SanitizedPersonalCopyPreview {
  const { artifact } = ctx;
  const rawTitle = (artifact.title || 'Untitled shared page').replace(/[\r\n]+/g, ' ').trim();
  const title = rawTitle.slice(0, MAX_SHARED_PAGE_TITLE_LEN) || 'Untitled shared page';

  let internalLinksCount = 0;
  let attachmentOrLocalCount = 0;
  let unsafeSchemeCount = 0;
  let unsupportedHtmlCount = 0;

  let md = (artifact.content_md || '').replace(/\r\n/g, '\n');

  // Strip a matching leading H1 because shared pages store title separately.
  const firstHeading = md.match(/^# ([^\n]+)\n+/);
  if (firstHeading && firstHeading[1].trim() === rawTitle) {
    md = md.slice(firstHeading[0].length);
  }

  const tree = markdownParser.parse(md) as unknown as MdastNode;
  const { destinations, defNodes } = collectDefinitions(tree);
  const replacements: TextReplacement[] = [];

  const bumpClass = (cls: DestinationClass) => {
    if (cls === 'personalVault') internalLinksCount += 1;
    else if (cls === 'localAttachment') attachmentOrLocalCount += 1;
    else if (cls === 'unsafeScheme') unsafeSchemeCount += 1;
  };

  // Remove disallowed reference link definitions `[id]: file:///...` without touching valid `[pub]: https://...`.
  for (const def of defNodes) {
    const cls = classifySharedDestination(def.url, 'link');
    if (cls !== 'allowed') {
      const start = def.position?.start?.offset;
      const end = def.position?.end?.offset;
      if (typeof start === 'number' && typeof end === 'number') {
        bumpClass(cls);
        replacements.push({ start, end, replacement: '' });
      }
    }
  }

  const visit = (node: MdastNode, insideLink = false) => {
    if (node.type === 'code' || node.type === 'inlineCode' || node.type === 'definition') return;

    const start = node.position?.start?.offset;
    const end = node.position?.end?.offset;

    if (node.type === 'link') {
      const cls = classifySharedDestination(node.url, 'link');
      if (cls !== 'allowed' && typeof start === 'number' && typeof end === 'number') {
        bumpClass(cls);
        const label = plainTextOfNode(node).trim();
        const isAutolink =
          label === (node.url ?? '').trim() ||
          md.slice(start, end).startsWith('<');
        const safeText = isAutolink || !label
          ? cls === 'personalVault'
            ? '[omitted private reference]'
            : '[omitted unsafe link]'
          : cls === 'localAttachment'
            ? `${label} (private link omitted)`
            : label;
        replacements.push({ start, end, replacement: safeText });
        return;
      }
      for (const child of node.children ?? []) visit(child, true);
      return;
    }

    if (node.type === 'image') {
      const cls = classifySharedDestination(node.url, 'image');
      if (cls !== 'allowed' && typeof start === 'number' && typeof end === 'number') {
        bumpClass(cls);
        const label = (node.alt || '').trim() || 'image';
        replacements.push({ start, end, replacement: `[omitted attachment: ${label}]` });
        return;
      }
      return;
    }

    if (node.type === 'linkReference') {
      const destination = node.identifier && destinations.get(node.identifier.toLowerCase());
      const defCls = destination !== undefined ? classifySharedDestination(destination, 'link') : undefined;
      if (defCls && defCls !== 'allowed' && typeof start === 'number' && typeof end === 'number') {
        bumpClass(defCls);
        const label = plainTextOfNode(node).trim() || node.label || 'reference';
        const replacement =
          defCls === 'localAttachment' ? `${label} (private link omitted)` : label;
        replacements.push({ start, end, replacement });
        return;
      }
    }

    if (node.type === 'imageReference') {
      const destination = node.identifier && destinations.get(node.identifier.toLowerCase());
      const defCls = destination !== undefined ? classifySharedDestination(destination, 'image') : undefined;
      if (defCls && defCls !== 'allowed' && typeof start === 'number' && typeof end === 'number') {
        bumpClass(defCls);
        const label = (node.alt || '').trim() || 'image';
        replacements.push({ start, end, replacement: `[omitted attachment: ${label}]` });
        return;
      }
    }

    if (node.type === 'html' && typeof start === 'number' && typeof end === 'number') {
      unsupportedHtmlCount += 1;
      replacements.push({ start, end, replacement: '' });
      return;
    }

    if (
      node.type === 'text' &&
      !insideLink &&
      typeof node.value === 'string' &&
      typeof start === 'number' &&
      typeof end === 'number'
    ) {
      if (/(?:sthira:\/\/|#\/sthira\/)/i.test(node.value)) {
        const replaced = node.value.replace(/(?:sthira:\/\/|#\/sthira\/)[^\s)>\]"']+/gi, () => {
          internalLinksCount += 1;
          return '[omitted private reference]';
        });
        if (replaced !== node.value) {
          replacements.push({ start, end, replacement: replaced });
        }
      }
      return;
    }

    for (const child of node.children ?? []) visit(child, insideLink);
  };

  visit(tree, false);
  md = applyReplacements(md, replacements);

  const omissions: string[] = [];
  if (internalLinksCount > 0) {
    omissions.push(`Personal vault links converted to plain text (${internalLinksCount})`);
  }
  if (attachmentOrLocalCount > 0) {
    omissions.push(`Local file links or embedded attachments removed (${attachmentOrLocalCount})`);
  }
  if (unsafeSchemeCount > 0) {
    omissions.push(`Unsupported URL schemes removed (${unsafeSchemeCount})`);
  }
  if (unsupportedHtmlCount > 0) {
    omissions.push(`Unsupported embedded HTML blocks removed (${unsupportedHtmlCount})`);
  }
  if (artifact.file_path || artifact.last_synced_hash) {
    omissions.push('Personal vault file path and local sync metadata omitted');
  }
  if ((ctx.project_ids && ctx.project_ids.length > 0) || ctx.project_id || ctx.epistemic_nodes.length > 0) {
    omissions.push('Personal project memberships and internal epistemic IDs omitted');
  }
  if (artifact.derived_from_artifact_id) {
    omissions.push('Private source derivation ID omitted');
  }

  let safeSourceNote: string | undefined;
  if (artifact.source_url) {
    try {
      const parsed = new URL(artifact.source_url);
      if (parsed.protocol === 'https:' || parsed.protocol === 'http:') {
        safeSourceNote = `${parsed.hostname}`;
      } else {
        omissions.push('Private source location omitted from provenance');
      }
    } catch {
      omissions.push('Non-URL source metadata omitted from provenance');
    }
  }

  const originKind: PageOriginKind =
    artifact.authorship === 'ai_generated'
      ? 'aiAssisted'
      : artifact.authorship === 'external'
        ? 'externalSource'
        : 'userAuthored';

  const defaultAuthorLabel =
    originKind === 'aiAssisted'
      ? 'AI-assisted personal page'
      : originKind === 'externalSource'
        ? 'Imported external source'
        : authorLabel?.trim() || 'Original personal vault author';

  const sanitizedMarkdown = md.replace(/^(?:[ \t]*\n)+|(?:\n[ \t]*)+$/g, '');
  const byteLength = utf8ByteLength(sanitizedMarkdown);

  return {
    sourceContentHash: artifact.content_hash,
    sourceVersion: artifact.version,
    title,
    sanitizedMarkdown,
    byteLength,
    maxBytes: MAX_SHARED_PAGE_CONTENT_BYTES,
    exceedsLimit: byteLength > MAX_SHARED_PAGE_CONTENT_BYTES,
    provenance: {
      originKind,
      originalAuthorLabel: defaultAuthorLabel,
      sourceNote: safeSourceNote,
    },
    omissions,
    hasOmissions: omissions.length > 0,
  };
}
