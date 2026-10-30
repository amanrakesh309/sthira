import React, { useState } from 'react';
import {
  Sliders,
  FileText,
  Activity,
  ShieldCheck,
  AlertTriangle,
  RotateCcw,
  Check,
  Copy,
  Sparkles,
} from 'lucide-react';
import { MilestonePlanDagWidget } from './widgets/MilestonePlanDagWidget';
import { EvidenceComparisonMatrixWidget } from './widgets/EvidenceComparisonMatrixWidget';
import { InteractiveDecisionSelectorWidget } from './widgets/InteractiveDecisionSelectorWidget';
import { FormattedMarkdownView } from './FormattedMarkdownView';
import { revertChangeSet } from '../api/ipc';
import type {
  ComparisonRow,
  DecisionPoint,
  PlanPhase,
  StreamMetrics,
  StreamStatus,
} from '../types/sthira';

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
  rawText,
  phases = [],
  decisions = [],
  comparisonRows = [],
  status = 'completed',
  metrics,
  basisFingerprint,
  fallbackNotices = [],
  projectId,
  onUndoRevert,
  onProjectCreated,
  onOpenInEditor,
}) => {
  const [viewMode, setViewMode] = useState<'widgets' | 'markdown'>('widgets');
  const [copied, setCopied] = useState(false);
  const [activeUndoToast, setActiveUndoToast] = useState<{
    id: string;
    message: string;
  } | null>(null);

  const isStreaming = status === 'streaming';

  // Deterministic Markdown-to-Widget Auto-Hydration (fallback for small models or plain markdown)
  const effectivePhases = phases.length > 0 ? phases : extractMarkdownPhases(rawText);
  const effectiveComparisonRows = comparisonRows.length > 0 ? comparisonRows : extractMarkdownComparisonRows(rawText);
  const effectiveDecisions = decisions.length > 0 ? decisions : extractMarkdownDecisions(rawText);

  const hasStructuredWidgets =
    effectivePhases.length > 0 || effectiveDecisions.length > 0 || effectiveComparisonRows.length > 0;

  const handleCopyMarkdown = () => {
    navigator.clipboard.writeText(rawText);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleLocalProjectCreated = (projId: string, changeSetId: string) => {
    setActiveUndoToast({
      id: changeSetId,
      message: `Project created from Phase DAG. 1-click rollback available.`,
    });
    onProjectCreated?.(projId, changeSetId);
  };

  const handleRevertUndo = async (csId: string) => {
    try {
      await revertChangeSet(csId);
      setActiveUndoToast(null);
      onUndoRevert?.(csId);
    } catch (err) {
      console.error('Failed to revert change set', err);
    }
  };

  // Extract clean text outside structured tags for the widget view commentary
  let cleanCommentaryText = rawText
    .replace(/```widget:(plan|decision|comparison)[\s\S]*?```/g, '')
    .replace(/<widget:(plan|decision|comparison)>[\s\S]*?<\/widget:\1>/g, '');

  if (phases.length === 0 && effectivePhases.length > 0) {
    // Strip phase markdown sections if they were auto-hydrated into widgets
    cleanCommentaryText = cleanCommentaryText.replace(/#{1,4}\s*(Phase|Step|Milestone)[\s\S]*?(?=(#{1,4}\s|\n\n\n|$))/gi, '');
  }
  if (comparisonRows.length === 0 && effectiveComparisonRows.length > 0) {
    // Strip markdown tables if they were auto-hydrated into matrix widgets
    cleanCommentaryText = cleanCommentaryText.replace(/\|[^\n]+\|\n\|[-:| ]+\|\n(?:\|[^\n]+\|\n?)*/g, '');
  }
  if (decisions.length === 0 && effectiveDecisions.length > 0) {
    // Strip decisions section if auto-hydrated
    cleanCommentaryText = cleanCommentaryText.replace(/#{1,4}\s*(Decisions?|Trade-?offs?|Choices?)[\s\S]*?(?=(#{1,4}\s|\n\n\n|$))/gi, '');
  }

  cleanCommentaryText = cleanCommentaryText.trim();

  return (
    <div className="w-full rounded-2xl border border-[#E8E5DC] dark:border-[#2C2E27] bg-[#FAF9F6] dark:bg-[#181915] p-4.5 text-[#1E201C] dark:text-[#EDEAE2] font-sans shadow-xs space-y-4">
      {/* Top Control Bar & Provenance Banner */}
      <div className="flex flex-wrap items-center justify-between gap-3 pb-3 border-b border-[#E8E5DC] dark:border-[#2C2E27]">
        {/* Left: Epistemic Provenance Badge */}
        <div className="flex items-center gap-2">
          {isStreaming ? (
            <span
              id="streaming-badge"
              className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-semibold bg-[#FFF7E8] dark:bg-[#2A200B] text-[#8C5D00] dark:text-[#F3BA59] border border-[#F2E0BA] dark:border-[#4D3A1B] animate-pulse"
            >
              <Activity className="w-3.5 h-3.5 text-[#E58C00]" />
              <span>[Streaming / Unverified]</span>
            </span>
          ) : (
            <div className="flex items-center gap-2">
              <span
                id="verified-badge"
                className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-semibold bg-[#EBF2EA] dark:bg-[#182618] text-[#2F5933] dark:text-[#88D080] border border-[#D1E2D0] dark:border-[#274026]"
              >
                <ShieldCheck className="w-3.5 h-3.5 text-[#3E7444] dark:text-[#88D080]" />
                <span>
                  [Verified Basis
                  {basisFingerprint ? `: ${basisFingerprint.slice(0, 8)}` : ''}]
                </span>
              </span>

              {metrics && (
                <span className="text-[11px] text-[#7A7870] dark:text-[#9E9C93] font-mono flex items-center gap-1.5">
                  <span>{metrics.total_tokens} tokens</span>
                  <span>•</span>
                  <span>{metrics.duration_ms}ms</span>
                </span>
              )}
            </div>
          )}

          {fallbackNotices.length > 0 && (
            <span
              id="fallback-notice-badge"
              className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium bg-[#FFF1E8] dark:bg-[#2C1810] text-[#9E3E0E] dark:text-[#E88C64] border border-[#F5D4C2] dark:border-[#4D281B]"
              title={fallbackNotices.join('\n')}
            >
              <AlertTriangle className="w-3 h-3" />
              <span>Degraded to Markdown</span>
            </span>
          )}
        </div>

        {/* Right: Dual-Mode Switcher & Copy Action */}
        <div className="flex items-center gap-2">
          {hasStructuredWidgets && (
            <div className="inline-flex p-0.5 rounded-lg bg-[#EFECE4] dark:bg-[#22241F] border border-[#E0DCD3] dark:border-[#2C2E27]">
              <button
                type="button"
                id="toggle-widgets-mode"
                onClick={() => setViewMode('widgets')}
                className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium transition-all cursor-pointer ${
                  viewMode === 'widgets'
                    ? 'bg-white dark:bg-[#1C1D19] text-[#1E201C] dark:text-[#EDEAE2] shadow-2xs font-semibold'
                    : 'text-[#66645D] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2]'
                }`}
                title="Render progressive interactive generative UI widgets"
              >
                <Sliders className="w-3.5 h-3.5" />
                <span>🎛️ Interactive Widgets</span>
              </button>

              <button
                type="button"
                id="toggle-markdown-mode"
                onClick={() => setViewMode('markdown')}
                className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium transition-all cursor-pointer ${
                  viewMode === 'markdown'
                    ? 'bg-white dark:bg-[#1C1D19] text-[#1E201C] dark:text-[#EDEAE2] shadow-2xs font-semibold'
                    : 'text-[#66645D] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2]'
                }`}
                title="Render formatted plain markdown text"
              >
                <FileText className="w-3.5 h-3.5" />
                <span>📄 Plain Markdown</span>
              </button>
            </div>
          )}

          {onOpenInEditor && (
            <button
              type="button"
              onClick={() => onOpenInEditor(rawText)}
              className="inline-flex items-center gap-1.5 px-3 py-1 rounded-lg bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] text-xs font-semibold hover:bg-[#3D4238] dark:hover:bg-[#D4D1C7] transition-colors shadow-2xs cursor-pointer"
              title="Open synthesized report in document workspace"
            >
              <FileText className="w-3.5 h-3.5" />
              <span>Open in Editor</span>
            </button>
          )}

          <button
            type="button"
            onClick={handleCopyMarkdown}
            aria-label="Copy markdown"
            className="p-1.5 rounded-lg bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-[#5C5B54] dark:text-[#9E9C93] transition-colors cursor-pointer"
            title="Copy response markdown"
          >
            {copied ? (
              <Check className="w-3.5 h-3.5 text-[#3E7444] dark:text-[#88D080]" />
            ) : (
              <Copy className="w-3.5 h-3.5" />
            )}
          </button>
        </div>
      </div>

      {/* Undo Toast Notification */}
      {activeUndoToast && (
        <div
          id="undo-toast"
          className="p-3 rounded-xl bg-[#282C25] dark:bg-[#161714] border border-transparent dark:border-[#2C2E27] text-white dark:text-[#EDEAE2] text-xs flex items-center justify-between shadow-lg animate-fadeIn"
        >
          <div className="flex items-center gap-2">
            <Sparkles className="w-4 h-4 text-[#C1DB99]" />
            <span>{activeUndoToast.message}</span>
          </div>
          <button
            type="button"
            onClick={() => handleRevertUndo(activeUndoToast.id)}
            className="inline-flex items-center gap-1 px-2.5 py-1 rounded bg-[#3D4238] dark:bg-[#252820] hover:bg-[#52594C] text-[#E0E8D7] dark:text-[#EDEAE2] font-semibold text-xs transition-colors cursor-pointer"
          >
            <RotateCcw className="w-3 h-3" />
            <span>Undo</span>
          </button>
        </div>
      )}

      {/* Body Presentation: Widgets vs Plain Markdown */}
      {viewMode === 'widgets' && hasStructuredWidgets ? (
        <div id="widgets-container" className="space-y-4">
          {/* Introductory commentary text */}
          {cleanCommentaryText && (
            <FormattedMarkdownView
              content={cleanCommentaryText}
              className="text-xs leading-relaxed text-[#363A31] dark:text-[#D4D1C7]"
            />
          )}

          {/* Interactive DAG Widget */}
          {effectivePhases.length > 0 && (
            <MilestonePlanDagWidget
              phases={effectivePhases}
              isStreaming={isStreaming}
              onProjectCreated={handleLocalProjectCreated}
            />
          )}

          {/* Evidence Comparison Matrix */}
          {effectiveComparisonRows.length > 0 && (
            <EvidenceComparisonMatrixWidget
              rows={effectiveComparisonRows}
              isStreaming={isStreaming}
            />
          )}

          {/* Interactive Decisions Selector */}
          {effectiveDecisions.length > 0 && (
            <InteractiveDecisionSelectorWidget
              decisions={effectiveDecisions}
              isStreaming={isStreaming}
              projectId={projectId}
            />
          )}
        </div>
      ) : (
        /* Rich Formatted Markdown View */
        <div
          id="markdown-container"
          className="p-4 rounded-xl bg-white dark:bg-[#161714] border border-[#E8E5DC] dark:border-[#2C2E27] shadow-2xs"
        >
          {rawText ? (
            <FormattedMarkdownView
              content={rawText}
              className="text-xs leading-relaxed"
            />
          ) : (
            <span className="text-[#8C8A81] dark:text-[#6E6D66] italic text-xs">
              {isStreaming ? 'Receiving token stream...' : 'No response text available.'}
            </span>
          )}
        </div>
      )}
    </div>
  );
};
