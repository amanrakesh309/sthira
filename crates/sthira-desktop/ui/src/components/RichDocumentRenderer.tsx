import React, { useState, useEffect, useMemo, useRef } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import {
  Copy,
  Check,
  Search,
  BookmarkPlus,
  GitCommit,
  Layers,
  Code as CodeIcon,
  Table as TableIcon,
} from 'lucide-react';
import { EpistemicBadge } from './EpistemicBadge';
import { EmbeddedObjectCard, type EmbeddedObjectData } from './objects/EmbeddedObjectCard';
import type { ObjectType } from '../types/sthira';

export interface RichDocumentRendererProps {
  content: string;
  projectId?: string | null;
  objectTypes?: ObjectType[];
  onUpdateContent?: (newContent: string) => void;
  onUpdateObject?: (updated: EmbeddedObjectData) => void;
  onRecordDecision?: (decisionText: string) => void;
  className?: string;
}

// Helper to recursively extract plain text from nested React nodes
function extractText(node: React.ReactNode): string {
  if (node == null) return '';
  if (typeof node === 'string' || typeof node === 'number') return String(node);
  if (Array.isArray(node)) return node.map(extractText).join(' ');
  if (React.isValidElement(node)) {
    return extractText((node.props as { children?: React.ReactNode })?.children);
  }
  return '';
}

// ---------------------------------------------------------------------------
// 1. Interactive Comparison Table Component
// ---------------------------------------------------------------------------
interface InteractiveTableProps {
  children?: React.ReactNode;
}

const InteractiveTable: React.FC<InteractiveTableProps> = ({ children }) => {
  const [filterQuery, setFilterQuery] = useState('');
  const [copied, setCopied] = useState(false);
  const tableRef = useRef<HTMLTableElement>(null);

  const handleCopyTable = () => {
    if (!tableRef.current) return;
    const rows = Array.from(tableRef.current.querySelectorAll('tr'));
    const tsv = rows
      .map((row) => {
        const cells = Array.from(row.querySelectorAll('th, td'));
        return cells.map((c) => (c.textContent || '').trim()).join('\t');
      })
      .join('\n');

    navigator.clipboard.writeText(tsv).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  return (
    <div
      data-testid="interactive-comparison-table-container"
      className="my-5 rounded-2xl border border-[#E8E5DC] dark:border-[#383530] bg-[#FAF8F3] dark:bg-[#181915] shadow-xs overflow-hidden transition-all"
    >
      {/* Table Header Bar */}
      <div className="flex items-center justify-between px-4 py-2.5 bg-[#EFECE4] dark:bg-[#252320] border-b border-[#E8E5DC] dark:border-[#383530] text-xs">
        <div className="flex items-center gap-2">
          <TableIcon className="w-4 h-4 text-[#7A7870] dark:text-[#A09D94]" />
          <span className="font-semibold text-[#1E201C] dark:text-[#E6E4DD] tracking-tight">
            Comparison Matrix
          </span>
        </div>

        <div className="flex items-center gap-2.5">
          {/* Real-time Search / Filter */}
          <div className="relative flex items-center">
            <Search className="w-3 h-3 absolute left-2 text-[#8C8A82] dark:text-[#7A7870]" />
            <input
              type="text"
              value={filterQuery}
              onChange={(e) => setFilterQuery(e.target.value)}
              placeholder="Filter rows..."
              aria-label="Filter rows"
              className="pl-6 pr-2 py-1 text-[11px] rounded-lg bg-white dark:bg-[#1E201C] border border-[#E0DCD3] dark:border-[#33302B] text-[#1E201C] dark:text-[#E6E4DD] placeholder-[#A09D94] dark:placeholder-[#6E6D66] focus:outline-none focus:ring-1 focus:ring-emerald-600/50 w-28 sm:w-36 transition-all"
            />
          </div>

          {/* Copy Table Button */}
          <button
            type="button"
            data-testid="btn-copy-table"
            onClick={handleCopyTable}
            className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-white dark:bg-[#1E201C] hover:bg-[#FAF9F6] dark:hover:bg-[#282622] border border-[#E0DCD3] dark:border-[#33302B] text-[11px] font-medium text-[#3A3B36] dark:text-[#D4D1C7] shadow-2xs transition-all cursor-pointer"
            title="Copy table as tab-delimited text"
          >
            {copied ? (
              <>
                <Check className="w-3 h-3 text-emerald-600 dark:text-emerald-400" />
                <span className="text-emerald-700 dark:text-emerald-300 font-semibold">Copied!</span>
              </>
            ) : (
              <>
                <Copy className="w-3 h-3 text-[#7A7870] dark:text-[#A09D94]" />
                <span>Copy Table</span>
              </>
            )}
          </button>
        </div>
      </div>

      {/* Styled Responsive Table Body */}
      <div className="overflow-x-auto">
        <table ref={tableRef} className="w-full text-left text-xs border-collapse font-sans">
          {React.Children.map(children, (child, idx) => {
            if (!React.isValidElement(child)) return child;
            const isTbody = idx === 1 || child.type === 'tbody';
            if (isTbody) {
              // Apply row filtering if filterQuery is provided
              const tbodyChildren = React.Children.toArray((child as React.ReactElement<{ children?: React.ReactNode }>).props.children);
              const filteredRows = filterQuery
                ? tbodyChildren.filter((row) => {
                    const rowText = extractText(row).toLowerCase();
                    return rowText.includes(filterQuery.toLowerCase());
                  })
                : tbodyChildren;

              return React.cloneElement(child as React.ReactElement<{ children: React.ReactNode }>, {
                children: filteredRows,
              });
            }
            return child;
          })}
        </table>
      </div>
    </div>
  );
};

// ---------------------------------------------------------------------------
// 2. Interactive Mermaid Diagram Renderer
// ---------------------------------------------------------------------------
interface MermaidBlockProps {
  code: string;
}

const MermaidBlock: React.FC<MermaidBlockProps> = ({ code }) => {
  const [svg, setSvg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isCopied, setIsCopied] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let isMounted = true;
    const renderMermaid = async () => {
      try {
        // Dynamic import wrapper that prevents Vite/Rollup from attempting build-time resolution
        let mermaid: {
          initialize: (config: Record<string, unknown>) => void;
          render: (id: string, text: string) => Promise<{ svg: string }>;
        } | null = null;

        if (typeof window !== 'undefined' && (window as unknown as { mermaid?: typeof mermaid }).mermaid) {
          mermaid = (window as unknown as { mermaid: typeof mermaid }).mermaid;
        } else {
          try {
            const dynamicImport = new Function('specifier', 'return import(specifier)');
            const mod = await dynamicImport('mermaid');
            mermaid = mod?.default || mod;
          } catch {
            mermaid = null;
          }
        }

        if (mermaid && typeof mermaid.render === 'function') {
          mermaid.initialize({
            startOnLoad: false,
            theme: 'neutral',
            themeVariables: {
              background: '#FAF8F3',
              primaryColor: '#EFECE4',
              primaryTextColor: '#1E201C',
              primaryBorderColor: '#8C8A82',
              lineColor: '#5C5953',
              fontFamily: 'inherit',
            },
          });
          const id = `mermaid-${Math.random().toString(36).substring(2, 9)}`;
          const { svg: renderedSvg } = await mermaid.render(id, code);
          if (isMounted) setSvg(renderedSvg);
          return;
        }

        // Graceful fallback when mermaid is not available locally
        if (isMounted) setSvg(null);
      } catch (err: unknown) {
        if (isMounted) setError(err instanceof Error ? err.message : 'Failed to parse Mermaid diagram');
      }
    };

    renderMermaid();
    return () => {
      isMounted = false;
    };
  }, [code]);

  const handleCopyCode = () => {
    navigator.clipboard.writeText(code).then(() => {
      setIsCopied(true);
      setTimeout(() => setIsCopied(false), 2000);
    });
  };

  return (
    <div
      data-testid="mermaid-diagram-card"
      className="my-5 rounded-2xl border border-[#E8E5DC] dark:border-[#383530] bg-[#FAF8F3] dark:bg-[#181915] shadow-xs overflow-hidden transition-all"
    >
      <div className="flex items-center justify-between px-4 py-2.5 bg-[#EFECE4] dark:bg-[#252320] border-b border-[#E8E5DC] dark:border-[#383530] text-xs">
        <div className="flex items-center gap-2">
          <GitCommit className="w-4 h-4 text-[#7A7870] dark:text-[#A09D94]" />
          <span className="font-semibold text-[#1E201C] dark:text-[#E6E4DD]">
            Visual Diagram (Mermaid)
          </span>
        </div>
        <button
          type="button"
          onClick={handleCopyCode}
          className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-white dark:bg-[#1E201C] hover:bg-[#FAF9F6] dark:hover:bg-[#282622] border border-[#E0DCD3] dark:border-[#33302B] text-[11px] font-medium text-[#3A3B36] dark:text-[#D4D1C7] transition-all cursor-pointer"
        >
          {isCopied ? <Check className="w-3 h-3 text-emerald-600" /> : <Copy className="w-3 h-3 text-[#7A7870]" />}
          <span>{isCopied ? 'Copied' : 'Copy Source'}</span>
        </button>
      </div>

      <div className="p-6 flex flex-col items-center justify-center min-h-[140px] overflow-x-auto">
        {svg ? (
          <div
            ref={containerRef}
            className="w-full flex justify-center [&>svg]:max-w-full [&>svg]:h-auto"
            dangerouslySetInnerHTML={{ __html: svg }}
          />
        ) : error ? (
          <div className="text-center p-4">
            <span className="text-xs text-amber-700 dark:text-amber-400 font-medium">
              Diagram syntax warning: {error}
            </span>
            <pre className="mt-2 text-[11px] font-mono text-left bg-black/5 dark:bg-white/5 p-3 rounded-lg overflow-x-auto text-[#1E201C] dark:text-[#EDEAE2]">
              {code}
            </pre>
          </div>
        ) : (
          /* Graceful Warm Stone Diagram View */
          <div className="w-full max-w-lg p-4 rounded-xl bg-white dark:bg-[#1E201C] border border-[#E8E5DC] dark:border-[#33302B] flex flex-col gap-3">
            <div className="flex items-center gap-2 pb-2 border-b border-[#ECE9E0] dark:border-[#2C2E27] text-xs font-semibold text-[#3A3B36] dark:text-[#D4D1C7]">
              <Layers className="w-3.5 h-3.5 text-purple-600 dark:text-purple-400" />
              <span>Diagram Flowchart</span>
            </div>
            <pre className="text-xs font-mono text-[#1E201C] dark:text-[#EDEAE2] leading-relaxed overflow-x-auto whitespace-pre-wrap">
              {code}
            </pre>
          </div>
        )}
      </div>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Main Component: RichDocumentRenderer
// ---------------------------------------------------------------------------
export const RichDocumentRenderer: React.FC<RichDocumentRendererProps> = ({
  content,
  projectId: _projectId,
  objectTypes = [],
  onUpdateContent,
  onUpdateObject,
  onRecordDecision,
  className = '',
}) => {
  // Pre-process: Clean orphaned bullets and tab-separated tables into clean GFM
  const processedContent = useMemo(() => {
    if (!content) return '';
    const rawLines = content.split('\n');
    const normalizedLines: string[] = [];

    for (let idx = 0; idx < rawLines.length; idx++) {
      const current = rawLines[idx];
      const trimmed = current.trim();

      // Merge orphaned bullets: •\nItem -> • Item
      if ((trimmed === '•' || trimmed === '-' || trimmed === '*') && idx + 1 < rawLines.length) {
        const nextLine = rawLines[idx + 1].trim();
        if (nextLine && !nextLine.startsWith('•') && !nextLine.startsWith('-') && !nextLine.startsWith('#')) {
          normalizedLines.push(`• ${nextLine}`);
          idx++;
          continue;
        }
      }

      // Convert tab-delimited tables into standard Markdown pipe tables
      if (current.includes('\t') && current.split('\t').filter(Boolean).length >= 2) {
        const cells = current.split('\t').map((c) => c.trim()).filter(Boolean);
        normalizedLines.push(`| ${cells.join(' | ')} |`);
        if (normalizedLines.length === 1 || !normalizedLines[normalizedLines.length - 2].startsWith('|')) {
          const divider = `| ${cells.map(() => ':---').join(' | ')} |`;
          normalizedLines.push(divider);
        }
        continue;
      }

      // Normalize inline LaTeX math formulas into standard unicode
      const cleanLine = current
        .replace(/\$\\ge\s*([^$]+)\$/g, '≥ $1')
        .replace(/\$\\le\s*([^$]+)\$/g, '≤ $1')
        .replace(/\$\\approx\s*([^$]+)\$/g, '≈ $1')
        .replace(/\$\\pm\s*([^$]+)\$/g, '± $1')
        .replace(/\$\\times\s*([^$]+)\$/g, '× $1')
        .replace(/\$\\ge\$/g, '≥')
        .replace(/\$\\le\$/g, '≤')
        .replace(/\$\\approx\$/g, '≈');

      normalizedLines.push(cleanLine);
    }

    return normalizedLines.join('\n');
  }, [content]);

  // Checkbox toggle handler in preview
  const handleToggleTask = (taskText: string, currentChecked: boolean) => {
    if (!onUpdateContent || !content) return;
    const lines = content.split('\n');
    const cleanTask = taskText.trim().replace(/^\[[ xX]\]\s*/, '');

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      if (
        (line.includes('- [ ]') || line.includes('- [x]') || line.includes('- [X]')) &&
        line.toLowerCase().includes(cleanTask.toLowerCase().slice(0, 24))
      ) {
        if (currentChecked) {
          lines[i] = line.replace(/- \[[xX]\]/, '- [ ]');
        } else {
          lines[i] = line.replace(/- \[ \]/, '- [x]');
        }
        onUpdateContent(lines.join('\n'));
        return;
      }
    }
  };

  return (
    <div
      data-testid="rich-document-renderer"
      className={`prose dark:prose-invert max-w-none text-sm text-[#1E201C] dark:text-[#EDEAE2] leading-relaxed ${className}`}
    >
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          // -----------------------------------------------------------------
          // 1. Table override -> Interactive Comparison Table
          // -----------------------------------------------------------------
          table: ({ children }) => <InteractiveTable>{children}</InteractiveTable>,
          thead: ({ children }) => (
            <thead className="bg-[#FAF8F3] dark:bg-[#1E201C] border-b border-[#E8E5DC] dark:border-[#383530]">
              {children}
            </thead>
          ),
          th: ({ children }) => (
            <th className="px-4 py-3 font-bold text-[#1E201C] dark:text-[#EDEAE2] text-xs tracking-tight border-r border-[#E8E5DC]/50 dark:border-[#383530]/50 last:border-r-0">
              {children}
            </th>
          ),
          tbody: ({ children }) => (
            <tbody className="divide-y divide-[#ECE9E0] dark:divide-[#2C2E27] bg-white dark:bg-[#181915]">
              {children}
            </tbody>
          ),
          tr: ({ children }) => (
            <tr className="hover:bg-[#FAF9F6] dark:hover:bg-[#20221D] transition-colors odd:bg-transparent even:bg-[#FAF8F3]/50 dark:even:bg-[#1C1E19]/50">
              {children}
            </tr>
          ),
          td: ({ children }) => (
            <td className="px-4 py-2.5 text-[#3A3B36] dark:text-[#D4D1C7] text-xs leading-relaxed border-r border-[#ECE9E0]/40 dark:border-[#2C2E27]/40 last:border-r-0">
              {children}
            </td>
          ),

          // -----------------------------------------------------------------
          // 2. Headings with refined Warm Stone typography
          // -----------------------------------------------------------------
          h1: ({ children }) => (
            <h1 className="text-2xl sm:text-3xl font-extrabold tracking-tight text-[#1E201C] dark:text-[#EDEAE2] mt-8 mb-4 pb-2 border-b border-[#ECE9E0] dark:border-[#2C2E27]">
              {children}
            </h1>
          ),
          h2: ({ children }) => (
            <h2 className="text-xl sm:text-2xl font-bold tracking-tight text-[#1E201C] dark:text-[#EDEAE2] mt-6 mb-3 flex items-center gap-2">
              <span className="w-1.5 h-5 rounded-full bg-emerald-600/70 dark:bg-emerald-400/70" />
              <span>{children}</span>
            </h2>
          ),
          h3: ({ children }) => (
            <h3 className="text-lg font-bold text-[#1E201C] dark:text-[#EDEAE2] mt-5 mb-2">
              {children}
            </h3>
          ),
          h4: ({ children }) => (
            <h4 className="text-sm font-bold text-[#3A3B36] dark:text-[#D4D1C7] mt-4 mb-2 uppercase tracking-wider">
              {children}
            </h4>
          ),

          // -----------------------------------------------------------------
          // 3. Lists & Interactive Task Checklists
          // -----------------------------------------------------------------
          ul: ({ children, className: ulClass }) => {
            const isTaskList = ulClass?.includes('contains-task-list');
            return (
              <ul
                className={`my-3 space-y-1.5 ${
                  isTaskList ? 'list-none pl-0' : 'list-disc pl-5 text-[#3A3B36] dark:text-[#D4D1C7]'
                }`}
              >
                {children}
              </ul>
            );
          },
          ol: ({ children }) => (
            <ol className="list-decimal pl-5 my-3 space-y-1.5 text-[#3A3B36] dark:text-[#D4D1C7]">
              {children}
            </ol>
          ),
          li: ({ children, className: liClass }) => {
            const isTaskItem = liClass?.includes('task-list-item');
            if (isTaskItem) {
              // Extract checked status and label
              const childArray = React.Children.toArray(children);
              let checked = false;
              let labelNode: React.ReactNode = children;

              if (React.isValidElement(childArray[0]) && (childArray[0].type === 'input' || (childArray[0].props as { type?: string })?.type === 'checkbox')) {
                checked = Boolean((childArray[0].props as { checked?: boolean })?.checked);
                labelNode = childArray.slice(1);
              }

              const stringContent = typeof labelNode === 'string'
                ? labelNode
                : React.Children.toArray(labelNode)
                    .map((c) => (React.isValidElement(c) ? (c as React.ReactElement<{ children?: React.ReactNode }>).props.children : String(c)))
                    .join(' ')
                    .trim();

              return (
                <li
                  data-testid="interactive-task-item"
                  className="flex items-start gap-2.5 py-1 px-2 -mx-2 rounded-lg hover:bg-[#FAF8F3] dark:hover:bg-[#20221D] transition-colors cursor-pointer group"
                >
                  <input
                    type="checkbox"
                    checked={checked}
                    onChange={() => handleToggleTask(stringContent, checked)}
                    className="mt-1 h-4 w-4 rounded border-[#C8C5BC] dark:border-[#423F3A] text-emerald-600 focus:ring-emerald-500 cursor-pointer shrink-0"
                  />
                  <div
                    onClick={() => handleToggleTask(stringContent, checked)}
                    className={`flex-1 text-xs sm:text-sm leading-relaxed ${
                      checked
                        ? 'line-through text-[#8C8A82] dark:text-[#7A7870]'
                        : 'text-[#1E201C] dark:text-[#EDEAE2]'
                    }`}
                  >
                    {labelNode}
                  </div>
                </li>
              );
            }

            return <li className="text-xs sm:text-sm leading-relaxed text-[#3A3B36] dark:text-[#D4D1C7]">{children}</li>;
          },

          // -----------------------------------------------------------------
          // 4. Blockquotes -> Interactive Epistemic Decision Callout
          // -----------------------------------------------------------------
          blockquote: ({ children }) => {
            // Check if blockquote content contains "Decision:" or "decision"
            const text = extractText(children);
            const isDecision =
              text.toLowerCase().includes('decision:') ||
              text.toLowerCase().includes('trade-off');

            if (isDecision) {
              return (
                <div
                  data-testid="interactive-epistemic-decision-callout"
                  className="my-5 p-4 rounded-2xl border-l-4 border-emerald-500 bg-[#EBF2EA] dark:bg-[#1A261D] shadow-2xs flex flex-col sm:flex-row items-start justify-between gap-4 transition-all"
                >
                  <div className="flex-1 space-y-1.5">
                    <div className="flex items-center gap-2">
                      <EpistemicBadge origin="user_recorded" label="Recorded" />
                      <span className="text-[11px] font-bold text-emerald-800 dark:text-emerald-300 uppercase tracking-wider">
                        Architectural Decision
                      </span>
                    </div>
                    <div className="text-xs sm:text-sm text-[#1E201C] dark:text-[#EDEAE2] font-medium leading-relaxed italic">
                      {children}
                    </div>
                  </div>

                  {onRecordDecision && (
                    <button
                      type="button"
                      data-testid="btn-add-to-project-decisions"
                      onClick={() => onRecordDecision(text)}
                      className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-emerald-700 hover:bg-emerald-800 text-white text-xs font-semibold shadow-2xs transition-colors shrink-0 cursor-pointer"
                      title="Pin and record this decision in Project Epistemic History"
                    >
                      <BookmarkPlus className="w-3.5 h-3.5" />
                      <span>Add to Decisions</span>
                    </button>
                  )}
                </div>
              );
            }

            return (
              <blockquote className="my-4 pl-4 border-l-3 border-[#8C8A82] dark:border-[#6E6D66] bg-[#FAF8F3]/60 dark:bg-[#1E201C]/60 py-2 pr-3 rounded-r-xl italic text-xs sm:text-sm text-[#3A3B36] dark:text-[#D4D1C7]">
                {children}
              </blockquote>
            );
          },

          // -----------------------------------------------------------------
          // 5. Code blocks -> Embedded Objects, Mermaid, or Styled Syntax
          // -----------------------------------------------------------------
          code: ({ node: _node, className: codeClass, children, ...props }) => {
            const match = /language-([a-zA-Z0-9_-]+)/.exec(codeClass || '');
            const lang = match ? match[1] : (codeClass || '').replace(/^language-/, '');
            const rawCode = String(children).replace(/\n$/, '');

            // Embedded Custom Object Card Directive: ```sthira-object ... ```
            if (lang === 'sthira-object' || codeClass?.includes('sthira-object')) {
              try {
                const parsed: EmbeddedObjectData = JSON.parse(rawCode);
                const matchedType = objectTypes.find((t) => t.id === parsed.object_type_id);
                return (
                  <div className="my-4 not-prose">
                    <EmbeddedObjectCard
                      data={parsed}
                      objectType={matchedType}
                      onUpdate={onUpdateObject}
                    />
                  </div>
                );
              } catch {
                // Fall back to standard code block on parse failure
              }
            }

            // Mermaid Diagrams Directive: ```mermaid ... ```
            if (lang === 'mermaid') {
              return <MermaidBlock code={rawCode} />;
            }

            // Inline code
            if (!codeClass && !rawCode.includes('\n')) {
              return (
                <code
                  className="px-1.5 py-0.5 rounded-md bg-[#EFECE4] dark:bg-[#252820] text-[#7A5A2B] dark:text-[#E8C280] font-mono text-[11px] font-semibold"
                  {...props}
                >
                  {children}
                </code>
              );
            }

            // Multi-line code block
            return (
              <div className="my-4 rounded-xl overflow-hidden border border-[#E8E5DC] dark:border-[#2C2E27] shadow-2xs not-prose">
                <div className="flex items-center justify-between px-3.5 py-1.5 bg-[#20221D] text-[#8C8A82] text-[10px] font-mono border-b border-[#2C2E27]">
                  <span>{lang || 'code'}</span>
                  <CodeIcon className="w-3 h-3" />
                </div>
                <pre className="p-4 bg-[#141512] text-[#EDEAE2] font-mono text-xs overflow-x-auto leading-relaxed m-0">
                  <code>{children}</code>
                </pre>
              </div>
            );
          },

          // -----------------------------------------------------------------
          // Horizontal Rules & Inline Formatting
          // -----------------------------------------------------------------
          hr: () => <hr className="my-6 border-t border-[#E8E5DC] dark:border-[#2C2E27]" />,
          a: ({ href, children }) => (
            <a
              href={href}
              target="_blank"
              rel="noopener noreferrer"
              className="text-emerald-700 dark:text-emerald-400 font-medium underline underline-offset-3 hover:opacity-80 transition-opacity"
            >
              {children}
            </a>
          ),
          strong: ({ children }) => (
            <strong className="font-bold text-[#1E201C] dark:text-[#EDEAE2]">{children}</strong>
          ),
          em: ({ children }) => (
            <em className="italic text-[#3A3B36] dark:text-[#D4D1C7]">{children}</em>
          ),
        }}
      >
        {processedContent}
      </ReactMarkdown>
    </div>
  );
};
