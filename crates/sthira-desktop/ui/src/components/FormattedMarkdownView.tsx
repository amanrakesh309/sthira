import React from 'react';
import {
  CheckCircle2,
  HelpCircle,
  Info,
} from 'lucide-react';
import { EmbeddedObjectCard, type EmbeddedObjectData } from './objects/EmbeddedObjectCard';
import type { ObjectType } from '../types/sthira';

interface FormattedMarkdownViewProps {
  content: string;
  className?: string;
  onToggleCheckbox?: (lineIdx: number) => void;
  objectTypes?: ObjectType[];
  onUpdateObject?: (updated: EmbeddedObjectData) => void;
}

// Inline Markdown parser (bold, italic, code, links, math symbols)
export function renderInlineMarkdown(text: string): React.ReactNode {
  if (!text) return null;

  // Normalize common LaTeX inline math expressions often produced by LLMs
  const normalized = text
    .replace(/\$\\ge\s*([^$]+)\$/g, '≥ $1')
    .replace(/\$\\le\s*([^$]+)\$/g, '≤ $1')
    .replace(/\$\\approx\s*([^$]+)\$/g, '≈ $1')
    .replace(/\$\\pm\s*([^$]+)\$/g, '± $1')
    .replace(/\$\\times\s*([^$]+)\$/g, '× $1')
    .replace(/\$\\ge\$/g, '≥')
    .replace(/\$\\le\$/g, '≤')
    .replace(/\$\\approx\$/g, '≈');

  // Tokenize bold (**text**), code (`code`), italic (*text*), link ([text](url))
  const parts: React.ReactNode[] = [];
  const regex = /(\*\*[^*]+\*\*|`[^`]+`|\*[^*]+\*|\[[^\]]+\]\([^)]+\))/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = regex.exec(normalized)) !== null) {
    if (match.index > lastIndex) {
      parts.push(normalized.slice(lastIndex, match.index));
    }

    const token = match[0];
    if (token.startsWith('**') && token.endsWith('**')) {
      parts.push(
        <strong key={match.index} className="font-bold text-[#1E201C] dark:text-[#EDEAE2]">
          {token.slice(2, -2)}
        </strong>
      );
    } else if (token.startsWith('`') && token.endsWith('`')) {
      parts.push(
        <code
          key={match.index}
          className="px-1.5 py-0.5 rounded bg-[#EFECE4] dark:bg-[#252820] text-[#7A5A2B] dark:text-[#E8C280] font-mono text-[11px]"
        >
          {token.slice(1, -1)}
        </code>
      );
    } else if (token.startsWith('*') && token.endsWith('*')) {
      parts.push(
        <em key={match.index} className="italic text-[#3A3B36] dark:text-[#D4D1C7]">
          {token.slice(1, -1)}
        </em>
      );
    } else if (token.startsWith('[') && token.includes('](')) {
      const linkMatch = token.match(/\[([^\]]+)\]\(([^)]+)\)/);
      if (linkMatch) {
        parts.push(
          <a
            key={match.index}
            href={linkMatch[2]}
            target="_blank"
            rel="noopener noreferrer"
            className="text-[#2F5933] dark:text-[#88D080] underline underline-offset-2 hover:opacity-80 transition-opacity"
          >
            {linkMatch[1]}
          </a>
        );
      } else {
        parts.push(token);
      }
    }
    lastIndex = regex.lastIndex;
  }

  if (lastIndex < normalized.length) {
    parts.push(normalized.slice(lastIndex));
  }

  return <>{parts}</>;
}

export const FormattedMarkdownView: React.FC<FormattedMarkdownViewProps> = ({
  content,
  className = '',
  onToggleCheckbox,
  objectTypes = [],
  onUpdateObject,
}) => {
  if (!content || !content.trim()) {
    return (
      <div className="text-[#8C8A82] dark:text-[#6E6D66] italic text-xs py-4 text-center">
        No content available.
      </div>
    );
  }

  // Pre-process: Clean orphaned bullets and tab-separated tables
  const rawLines = content.split('\n');
  const normalizedLines: string[] = [];

  for (let idx = 0; idx < rawLines.length; idx++) {
    const current = rawLines[idx];
    const trimmed = current.trim();

    // Check for orphaned bullet e.g. "•" or "-" alone on a line
    if ((trimmed === '•' || trimmed === '-' || trimmed === '*') && idx + 1 < rawLines.length) {
      const nextLine = rawLines[idx + 1].trim();
      if (nextLine && !nextLine.startsWith('•') && !nextLine.startsWith('-') && !nextLine.startsWith('#')) {
        normalizedLines.push(`• ${nextLine}`);
        idx++; // skip next line as it's merged
        continue;
      }
    }

    // Check for tab-separated table row
    if (current.includes('\t') && current.split('\t').filter(Boolean).length >= 2) {
      const cells = current.split('\t').map((c) => c.trim()).filter(Boolean);
      normalizedLines.push(`| ${cells.join(' | ')} |`);
      // If previous line wasn't table and next line is table, insert header divider
      if (normalizedLines.length === 1 || !normalizedLines[normalizedLines.length - 2].startsWith('|')) {
        const divider = `| ${cells.map(() => ':---').join(' | ')} |`;
        normalizedLines.push(divider);
      }
      continue;
    }

    normalizedLines.push(current);
  }

  const elements: React.ReactNode[] = [];
  let inCodeBlock = false;
  let inObjectBlock = false;
  let codeBuffer: string[] = [];
  let inTable = false;
  let tableRows: string[][] = [];

  const flushCode = (key: string) => {
    if (codeBuffer.length > 0) {
      if (inObjectBlock) {
        try {
          const jsonStr = codeBuffer.join('\n');
          const objData: EmbeddedObjectData = JSON.parse(jsonStr);
          const matchedType = objectTypes.find((t) => t.id === objData.object_type_id);
          elements.push(
            <EmbeddedObjectCard
              key={key}
              data={objData}
              objectType={matchedType}
              onUpdate={onUpdateObject}
            />
          );
          codeBuffer = [];
          inObjectBlock = false;
          return;
        } catch {
          // Fall through to regular code block
        }
      }

      elements.push(
        <pre
          key={key}
          className="p-3.5 rounded-xl bg-[#282C25] dark:bg-[#121310] text-[#ECE9E0] dark:text-[#D4D1C7] border border-transparent dark:border-[#2C2E27] font-mono text-xs overflow-x-auto my-3 shadow-2xs leading-relaxed"
        >
          <code>{codeBuffer.join('\n')}</code>
        </pre>
      );
      codeBuffer = [];
      inObjectBlock = false;
    }
  };

  const flushTable = (key: string) => {
    if (tableRows.length > 0) {
      const filtered = tableRows.filter((r) => !r.every((cell) => /^:?-+:?$/.test(cell.trim())));
      if (filtered.length > 0) {
        const [headerRow, ...bodyRows] = filtered;
        elements.push(
          <div key={key} className="my-3 border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl overflow-hidden shadow-2xs">
            <table className="w-full text-left text-xs border-collapse">
              {headerRow && (
                <thead className="bg-[#FAF8F3] dark:bg-[#161714] border-b border-[#E8E5DC] dark:border-[#2C2E27]">
                  <tr>
                    {headerRow.map((cell, cIdx) => (
                      <th key={cIdx} className="px-3.5 py-2 font-bold text-[#1E201C] dark:text-[#EDEAE2]">
                        {renderInlineMarkdown(cell)}
                      </th>
                    ))}
                  </tr>
                </thead>
              )}
              <tbody className="divide-y divide-[#ECE9E0] dark:divide-[#262822] bg-white dark:bg-[#1C1D19]">
                {bodyRows.map((r, rIdx) => (
                  <tr key={rIdx} className="hover:bg-[#FAF9F6] dark:hover:bg-[#22241F] transition-colors">
                    {r.map((cell, cIdx) => (
                      <td key={cIdx} className="px-3.5 py-2 text-[#3A3B36] dark:text-[#D4D1C7] leading-relaxed">
                        {renderInlineMarkdown(cell)}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        );
      }
      tableRows = [];
    }
  };

  for (let i = 0; i < normalizedLines.length; i++) {
    const line = normalizedLines[i];
    const trimmed = line.trim();

    // Code fence
    if (trimmed.startsWith('```')) {
      if (inCodeBlock) {
        flushCode(`code-${i}`);
        inCodeBlock = false;
      } else {
        if (inTable) {
          flushTable(`table-${i}`);
          inTable = false;
        }
        inCodeBlock = true;
        inObjectBlock = trimmed.startsWith('```sthira-object');
      }
      continue;
    }

    if (inCodeBlock) {
      codeBuffer.push(line);
      continue;
    }

    // Markdown Table lines
    if (trimmed.startsWith('|') && trimmed.endsWith('|') && trimmed.length > 2) {
      inTable = true;
      const cells = trimmed
        .slice(1, -1)
        .split('|')
        .map((c) => c.trim());
      tableRows.push(cells);
      continue;
    } else if (inTable) {
      flushTable(`table-${i}`);
      inTable = false;
    }

    // Horizontal Rule
    if (trimmed === '---' || trimmed === '***' || trimmed === '___') {
      elements.push(
        <hr key={i} className="my-4 border-t border-[#E8E5DC] dark:border-[#2C2E27]" />
      );
      continue;
    }

    // Headings (H1, H2, H3, H4) or unhashed Phase/Step headings
    if (line.startsWith('# ')) {
      elements.push(
        <h1 key={i} className="text-xl sm:text-2xl font-bold tracking-tight text-[#1E201C] dark:text-[#EDEAE2] mt-4 mb-2">
          {renderInlineMarkdown(line.replace(/^#\s*/, ''))}
        </h1>
      );
      continue;
    }

    if (line.startsWith('## ') || /^Phase\s+\d+:/i.test(trimmed) || /^Step\s+\d+:/i.test(trimmed)) {
      const headingText = line.startsWith('## ') ? line.replace(/^##\s*/, '') : trimmed;
      elements.push(
        <h2 key={i} className="text-base sm:text-lg font-bold tracking-tight text-[#1E201C] dark:text-[#EDEAE2] mt-3.5 mb-1.5 flex items-center gap-2">
          <span className="w-1.5 h-3.5 rounded-full bg-[#282C25] dark:bg-[#EDEAE2]" />
          <span>{renderInlineMarkdown(headingText)}</span>
        </h2>
      );
      continue;
    }

    if (line.startsWith('### ')) {
      elements.push(
        <h3 key={i} className="text-sm sm:text-base font-semibold text-[#1E201C] dark:text-[#EDEAE2] mt-3 mb-1">
          {renderInlineMarkdown(line.replace(/^###\s*/, ''))}
        </h3>
      );
      continue;
    }

    if (line.startsWith('#### ')) {
      elements.push(
        <h4 key={i} className="text-xs sm:text-sm font-semibold uppercase tracking-wider text-[#66645D] dark:text-[#9E9C93] mt-2 mb-1">
          {renderInlineMarkdown(line.replace(/^####\s*/, ''))}
        </h4>
      );
      continue;
    }

    // Interactive Checklists
    if (trimmed.startsWith('- [ ] ') || trimmed.startsWith('- [x] ')) {
      const isChecked = trimmed.startsWith('- [x] ');
      const text = trimmed.replace(/^-\s*\[[ x]\]\s*/, '');

      elements.push(
        <div key={i} className="flex items-start gap-2.5 py-0.5 ml-1">
          <input
            type="checkbox"
            checked={isChecked}
            onChange={() => onToggleCheckbox?.(i)}
            className="mt-0.5 w-3.5 h-3.5 rounded border-[#CCDCCC] dark:border-[#3E4237] text-[#282C25] dark:text-[#EDEAE2] focus:ring-0 cursor-pointer"
          />
          <span
            onClick={() => onToggleCheckbox?.(i)}
            className={`text-xs cursor-pointer select-none leading-relaxed ${
              isChecked ? 'line-through text-[#8C8A82] dark:text-[#6E6D66]' : 'text-[#1E201C] dark:text-[#EDEAE2]'
            }`}
          >
            {renderInlineMarkdown(text)}
          </span>
        </div>
      );
      continue;
    }

    // Numbered List Items
    const numMatch = trimmed.match(/^(\d+)\.\s+(.*)$/);
    if (numMatch) {
      elements.push(
        <div key={i} className="flex items-start gap-2 py-0.5 ml-2">
          <span className="inline-flex items-center justify-center w-4 h-4 rounded-full bg-[#EFECE4] dark:bg-[#252820] text-[10px] font-bold text-[#66645D] dark:text-[#9E9C93] shrink-0 mt-0.5">
            {numMatch[1]}
          </span>
          <span className="text-xs text-[#1E201C] dark:text-[#EDEAE2] leading-relaxed">
            {renderInlineMarkdown(numMatch[2])}
          </span>
        </div>
      );
      continue;
    }

    // Bullet Items (including •, -, *)
    if (trimmed.startsWith('- ') || trimmed.startsWith('* ') || trimmed.startsWith('• ') || trimmed.startsWith('•')) {
      const text = trimmed.replace(/^[-*•]\s*/, '');
      elements.push(
        <div key={i} className="flex items-start gap-2 py-0.5 ml-2">
          <span className="text-[#7A7870] dark:text-[#9E9C93] font-bold">•</span>
          <span className="text-xs text-[#1E201C] dark:text-[#EDEAE2] leading-relaxed">
            {renderInlineMarkdown(text)}
          </span>
        </div>
      );
      continue;
    }

    // Epistemic Decision Block
    if (trimmed.includes('**Decision:**') || trimmed.includes('**Trade-offs & Decisions:**')) {
      elements.push(
        <div key={i} className="my-2 p-3 rounded-xl bg-[#EAF2E8] dark:bg-[#182618] border border-[#CCDCCC] dark:border-[#274026] text-xs space-y-1">
          <div className="flex items-center gap-1.5 font-bold text-[#2D5A27] dark:text-[#88D080] uppercase tracking-wider text-[10px]">
            <CheckCircle2 className="w-3.5 h-3.5 text-emerald-700 dark:text-emerald-400" />
            <span>Decision / Trade-off</span>
          </div>
          <div className="font-semibold text-[#1E201C] dark:text-[#EDEAE2] text-xs leading-relaxed">
            {renderInlineMarkdown(trimmed.replace(/^>\s*\*\*Decision:\*\*\s*/i, ''))}
          </div>
        </div>
      );
      continue;
    }

    // Epistemic Question Block
    if (trimmed.includes('**Open Question:**')) {
      elements.push(
        <div key={i} className="my-2 p-3 rounded-xl bg-[#FFF8E6] dark:bg-[#292211] border border-[#F2E0BA] dark:border-[#4D3A1B] text-xs space-y-1">
          <div className="flex items-center gap-1.5 font-bold text-[#8C5D00] dark:text-[#F3BA59] uppercase tracking-wider text-[10px]">
            <HelpCircle className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400" />
            <span>Open Question</span>
          </div>
          <div className="font-medium text-[#1E201C] dark:text-[#EDEAE2] text-xs">
            {renderInlineMarkdown(trimmed.replace(/^>\s*\*\*Open Question:\*\*\s*/, ''))}
          </div>
        </div>
      );
      continue;
    }

    // Callout Note
    if (trimmed.startsWith('> [!NOTE]')) {
      elements.push(
        <div key={i} className="my-2.5 p-2.5 rounded-xl bg-[#FAF8F3] dark:bg-[#1C1D19] border-l-4 border-[#282C25] dark:border-[#EDEAE2] text-xs text-[#282C25] dark:text-[#EDEAE2]">
          <div className="font-bold text-[10px] uppercase tracking-wider mb-1 flex items-center gap-1">
            <Info className="w-3.5 h-3.5" />
            <span>Note</span>
          </div>
        </div>
      );
      continue;
    }

    // Blockquote
    if (trimmed.startsWith('> ')) {
      elements.push(
        <blockquote key={i} className="border-l-2 border-[#D8D4C8] dark:border-[#3A3D34] pl-3 py-1 my-1 text-xs text-[#6E6D66] dark:text-[#9E9C93] italic">
          {renderInlineMarkdown(trimmed.replace(/^>\s*/, ''))}
        </blockquote>
      );
      continue;
    }

    // Empty space
    if (!trimmed) {
      elements.push(<div key={i} className="h-1.5" />);
      continue;
    }

    // Standard paragraph with rich inline formatting
    elements.push(
      <p key={i} className="text-xs leading-relaxed text-[#1E201C] dark:text-[#EDEAE2] my-1">
        {renderInlineMarkdown(line)}
      </p>
    );
  }

  if (inCodeBlock) flushCode('code-eof');
  if (inTable) flushTable('table-eof');

  return <div className={`space-y-1 font-sans ${className}`}>{elements}</div>;
};
