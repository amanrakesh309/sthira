import React, { useEffect, useRef, useState } from 'react';
import { Check, Copy, FileText } from 'lucide-react';
import { RichDocumentRenderer } from './RichDocumentRenderer';
import { plainPageMarkdown } from '../editor/plainPageMarkdown';
import type { ComparisonRow, DecisionPoint, PlanPhase, StreamMetrics, StreamStatus } from '../types/sthira';

interface AdaptiveResponseRendererProps {
  rawText: string;
  phases?: PlanPhase[];
  decisions?: DecisionPoint[];
  comparisonRows?: ComparisonRow[];
  status?: StreamStatus;
  metrics?: StreamMetrics | null;
  basisFingerprint?: string | null;
  fallbackNotices?: string[];
  projectId?: string | null;
  onUndoRevert?: (changeSetId: string) => void;
  onProjectCreated?: (projectId: string, changeSetId: string) => void;
  onOpenInEditor?: (text: string, title?: string) => void;
}

// Kept for compatibility with older clients; ordinary Markdown is never auto-hydrated.
export function extractMarkdownPhases(text: string): PlanPhase[] {
  if (!text) return [];
  const lines = text.split('\n');
  const phases: PlanPhase[] = [];
  let current: { phase_number: number; title: string; milestones: string[]; prerequisites: string[] } | null = null;
  let autoNum = 1;

  const flush = () => {
    if (current) {
      if (current.milestones.length === 0) {
        current.milestones.push(current.title);
      }
      phases.push({ ...current });
      current = null;
    }
  };

  for (let i = 0; i < lines.length; i++) {
    const trimmed = lines[i].trim();
    const lower = trimmed.toLowerCase();

    const isPhaseHeading =
      (trimmed.startsWith('#') && (lower.includes('phase') || lower.includes('step') || lower.includes('milestone'))) ||
      (trimmed.startsWith('- [ ]') && (lower.includes('phase') || lower.includes('step'))) ||
      (trimmed.startsWith('**phase') || trimmed.startsWith('**step')) ||
      lower.startsWith('phase ') ||
      lower.startsWith('step ') ||
      lower.startsWith('milestone ');

    if (isPhaseHeading) {
      flush();
      const cleanTitle = trimmed
        .replace(/^#+\s*/, '')
        .replace(/^-\s*\[[ x]\]\s*/, '')
        .replace(/^\*\*|\*\*$/g, '')
        .trim();

      const phaseMatch = lower.match(/(?:phase|step|milestone)\s+(\d+)/);
      const num = phaseMatch ? parseInt(phaseMatch[1], 10) : autoNum;
      autoNum = num + 1;

      current = {
        phase_number: num,
        title: cleanTitle,
        milestones: [],
        prerequisites: num > 1 ? [`Phase ${num - 1}`] : [],
      };
    } else if (current) {
      if (trimmed.startsWith('---')) {
        // section divider
      } else if (
        (trimmed.startsWith('- [ ]') ||
          trimmed.startsWith('- [x]') ||
          trimmed.startsWith('- ') ||
          trimmed.startsWith('* ') ||
          trimmed.startsWith('• ') ||
          trimmed.startsWith('•')) &&
        trimmed.length > 1
      ) {
        const item = trimmed
          .replace(/^-\s*\[[ x]\]\s*/, '')
          .replace(/^[-*•]\s*/, '')
          .trim();
        if (item) {
          current.milestones.push(item);
        }
      } else if (/^\d+\.\s+/.test(trimmed)) {
        // Numbered item: e.g. "1. **Greek Yogurt/Paneer:** ..."
        const item = trimmed.replace(/^\d+\.\s*/, '').trim();
        if (item) {
          current.milestones.push(item);
        }
      } else if (
        (trimmed.startsWith('**architectural invariant:') ||
          trimmed.startsWith('**invariant:') ||
          trimmed.startsWith('**the "protein stack"') ||
          trimmed.startsWith('**blueprint:')) &&
        trimmed.length > 0
      ) {
        current.milestones.push(trimmed);
      }
    }
  }
  flush();
  return phases;
}

export function extractMarkdownComparisonRows(text: string): ComparisonRow[] {
  if (!text) return [];
  const lines = text.split('\n');
  const rows: ComparisonRow[] = [];
  const tableRows: string[][] = [];

  const flushTable = () => {
    if (tableRows.length >= 2) {
      for (let i = 1; i < tableRows.length; i++) {
        const dataRow = tableRows[i];
        if (dataRow.every((c) => /^:?-+:?$/.test(c.trim()))) continue;
        if (!dataRow[0] || !dataRow[0].trim()) continue;

        rows.push({
          criteria: dataRow[0].trim(),
          aspect_a: dataRow[1]?.trim() || '',
          aspect_b: dataRow[2]?.trim() || '',
          verdict: dataRow[3]?.trim() || undefined,
        });
      }
    }
    tableRows.length = 0;
  };

  for (let i = 0; i < lines.length; i++) {
    const trimmed = lines[i].trim();
    let cells: string[] = [];
    if (trimmed.includes('\t')) {
      cells = trimmed.split('\t').map((c) => c.trim()).filter(Boolean);
    } else if (trimmed.startsWith('|') && trimmed.endsWith('|') && trimmed.length > 2) {
      cells = trimmed.slice(1, -1).split('|').map((c) => c.trim());
    } else if (trimmed.includes('|') && trimmed.length > 4) {
      cells = trimmed.split('|').map((c) => c.trim());
    }

    if (cells.length >= 2) {
      tableRows.push(cells);
    } else if (tableRows.length > 0) {
      flushTable();
    }
  }
  flushTable();
  return rows;
}

export function extractMarkdownDecisions(text: string): DecisionPoint[] {
  if (!text) return [];
  const lines = text.split('\n');
  const decisions: DecisionPoint[] = [];
  let inDecisionSection = false;
  let currentQuestion: string | null = null;
  let currentOptions: string[] = [];
  let currentRecommended: string | undefined = undefined;

  const flush = () => {
    if (currentQuestion && currentOptions.length > 0) {
      const id = currentQuestion.replace(/[^a-zA-Z0-9_]/g, '').slice(0, 24).toLowerCase() || 'decision_point';
      decisions.push({
        id,
        question: currentQuestion,
        options: currentOptions,
        recommended: currentRecommended || currentOptions[0],
      });
      currentQuestion = null;
      currentOptions = [];
      currentRecommended = undefined;
    }
  };

  for (let i = 0; i < lines.length; i++) {
    const trimmed = lines[i].trim();
    const lower = trimmed.toLowerCase();

    const isDecisionHeading =
      (trimmed.startsWith('#') && (lower.includes('decision') || lower.includes('trade-off') || lower.includes('tradeoff'))) ||
      lower.startsWith('decision:') ||
      lower.startsWith('> **decision:**') ||
      lower.startsWith('**decision') ||
      lower.startsWith('**trade-off') ||
      lower.startsWith('**tradeoff') ||
      lower.startsWith('trade-offs & decisions') ||
      lower.startsWith('trade-offs:') ||
      lower.startsWith('trade-off:') ||
      lower.startsWith('decisions:');

    if (isDecisionHeading) {
      flush();
      inDecisionSection = true;
      const q = trimmed
        .replace(/^#+\s*/, '')
        .replace(/^>\s*/, '')
        .replace(/^\*\*decision:\*\*\s*/i, '')
        .replace(/^\*\*trade-offs\s*&?\s*decisions:\*\*\s*/i, '')
        .replace(/^\*\*trade-offs:\*\*\s*/i, '')
        .replace(/^decision:\s*/i, '')
        .replace(/^\*\*|\*\*$/g, '')
        .trim();
      currentQuestion = q || 'Key Trade-offs & Decisions';
    } else if (inDecisionSection) {
      if (trimmed.startsWith('#')) {
        flush();
        inDecisionSection = false;
      } else if (!trimmed) {
        // empty line
      } else if (trimmed.startsWith('- ') || trimmed.startsWith('* ') || trimmed.startsWith('• ') || trimmed.startsWith('•')) {
        const opt = trimmed.replace(/^[-*•]\s*/, '').trim();
        if (opt) {
          if (opt.toLowerCase().includes('(recommended)') || opt.toLowerCase().includes('(chosen)')) {
            currentRecommended = opt;
          }
          currentOptions.push(opt);
        }
      } else if (trimmed.startsWith('**') && (lower.includes('trade-off') || lower.includes('tradeoff') || lower.includes('choice'))) {
        flush();
        if (trimmed.includes(':')) {
          const colonPos = trimmed.indexOf(':');
          const q = trimmed.slice(0, colonPos).replace(/^\*\*|\*\*$/g, '').trim();
          const opt = trimmed.slice(colonPos + 1).trim();
          const opts = [opt];
          if (opt.toLowerCase().includes('trade')) {
            opts.push('Alternative approach (higher variance)');
          }
          decisions.push({
            id: q.replace(/[^a-zA-Z0-9_]/g, '').slice(0, 24).toLowerCase() || 'decision_point',
            question: q,
            options: opts,
            recommended: opt,
          });
        } else {
          currentQuestion = trimmed;
          currentOptions.push(trimmed);
        }
      } else if (trimmed && currentOptions.length === 0) {
        const cql = (currentQuestion || '').toLowerCase();
        if (cql === 'decisions' || cql === 'trade-offs' || cql === 'tradeoffs' || cql === 'key architecture decision' || cql === 'key trade-offs & decisions' || trimmed.endsWith('?')) {
          currentQuestion = trimmed;
        }
      }
    }
  }
  flush();
  return decisions;
}

export const AdaptiveResponseRenderer: React.FC<AdaptiveResponseRendererProps> = ({
  rawText, phases = [], decisions = [], comparisonRows = [], status = 'completed', basisFingerprint, onOpenInEditor,
}) => {
  const [copied, setCopied] = useState(false);
  const mountedRef = useRef(true);
  const copyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      if (copyTimerRef.current) {
        clearTimeout(copyTimerRef.current);
        copyTimerRef.current = null;
      }
    };
  }, []);
  let markdown = plainPageMarkdown(rawText);
  // Older streaming protocols may deliver content separately from their text tokens.
  for (const [kind, data, label] of [
    ...phases.map(data => ['plan', data, data.title] as const),
    ...comparisonRows.map(data => ['comparison', data, data.criteria] as const),
    ...decisions.map(data => ['decision', data, data.question] as const),
  ]) {
    if (!markdown.includes(label)) markdown += `\n\n${plainPageMarkdown(`\`\`\`widget:${kind}\n${JSON.stringify(data)}\n\`\`\``)}`;
  }
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(markdown);
      if (!mountedRef.current) return;
      setCopied(true);
      if (copyTimerRef.current) clearTimeout(copyTimerRef.current);
      copyTimerRef.current = setTimeout(() => {
        copyTimerRef.current = null;
        if (mountedRef.current) setCopied(false);
      }, 2000);
    }
    catch { if (mountedRef.current) setCopied(false); }
  };
  return <section className="adaptive-response space-y-3">
    <div className="flex items-center justify-between gap-3 text-xs text-sthira-text-muted">
      <span>{status === 'streaming' ? '[Streaming / Unverified]' : basisFingerprint ? `[Verified Basis: ${basisFingerprint.slice(0, 8)}]` : 'Response'}</span>
      <div className="flex items-center gap-2">
        {onOpenInEditor && <button type="button" onClick={() => onOpenInEditor(markdown)} className="sthira-button-secondary"><FileText size={14}/> Open in Editor</button>}
        <button type="button" onClick={copy} aria-label="Copy markdown" title="Copy response markdown">{copied ? <Check size={16}/> : <Copy size={16}/>}</button>
      </div>
    </div>
    <RichDocumentRenderer content={markdown} />
  </section>;
};
