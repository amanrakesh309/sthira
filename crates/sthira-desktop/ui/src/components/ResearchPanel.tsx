import { useState } from 'react';
import { Search, Sparkles, Loader2 } from 'lucide-react';
import { useAdaptiveStream } from '../hooks/useAdaptiveStream';
import { AdaptiveResponseRenderer } from './AdaptiveResponseRenderer';

interface ResearchPanelProps {
  projectId?: string | null;
  onRefresh?: () => void;
}

export const ResearchPanel: React.FC<ResearchPanelProps> = ({
  projectId,
  onRefresh,
}) => {
  const [researchQuery, setResearchQuery] = useState('');
  const {
    rawText,
    phases,
    decisions,
    comparisonRows,
    status,
    metrics,
    basisFingerprint,
    fallbackNotices,
    startStream,
    resetStream,
  } = useAdaptiveStream();

  const isStreaming = status === 'streaming';
  const hasContent = rawText.length > 0 || comparisonRows.length > 0;

  const handleRunAdaptiveResearch = async () => {
    if (!researchQuery.trim() || isStreaming) return;
    await startStream(researchQuery.trim(), 'research');
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      handleRunAdaptiveResearch();
    }
  };

  return (
    <div className="w-full space-y-4 font-sans text-xs">
      {/* Intro Description */}
      <div className="flex items-center justify-between text-[#66645D] dark:text-[#9E9C93]">
        <div className="flex items-center gap-1.5">
          <Search className="w-4 h-4 text-[#7A7870] dark:text-[#9E9C93]" />
          <span className="font-semibold text-xs text-[#282C25] dark:text-[#EDEAE2]">
            Comparative Research & Synthesis
          </span>
        </div>
        <span className="text-[11px] text-[#8C8A81] dark:text-[#6E6D66]">Matrix & Grounded Evidence</span>
      </div>

      {/* Query Input Area */}
      <div className="space-y-2">
        <textarea
          value={researchQuery}
          onChange={(e) => setResearchQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Enter inquiry to research and compare (e.g. 'Tauri v2 Channel vs WebSocket loopback performance')..."
          rows={3}
          className="w-full p-3 rounded-xl border border-[#E8E5DC] dark:border-[#2C2E27] bg-white dark:bg-[#141512] text-xs text-[#1E201C] dark:text-[#EDEAE2] placeholder-[#A09D94] dark:placeholder-[#6E6D66] focus:outline-none focus:border-[#7A7870] dark:focus:border-[#9E9C93] transition-colors resize-none leading-relaxed shadow-2xs"
        />

        <div className="flex items-center justify-between">
          <span className="text-[11px] text-[#8C8A81] dark:text-[#6E6D66]">
            Press ⌘+Enter to research
          </span>
          <div className="flex items-center gap-2">
            {hasContent && (
              <button
                type="button"
                onClick={resetStream}
                className="px-2.5 py-1.5 rounded-lg text-xs font-medium text-[#7A7870] dark:text-[#9E9C93] hover:bg-[#EFECE4] dark:hover:bg-[#252820] transition-colors cursor-pointer"
              >
                Clear
              </button>
            )}
            <button
              type="button"
              onClick={handleRunAdaptiveResearch}
              disabled={isStreaming || !researchQuery.trim()}
              className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg text-xs font-semibold text-white dark:text-[#121310] bg-[#282C25] dark:bg-[#EDEAE2] hover:bg-[#3D4238] dark:hover:bg-[#D4D1C7] active:bg-[#1E201C] disabled:opacity-40 transition-colors shadow-xs cursor-pointer"
            >
              {isStreaming ? (
                <>
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  <span>Synthesizing Matrix...</span>
                </>
              ) : (
                <>
                  <Sparkles className="w-3.5 h-3.5" />
                  <span>Investigate & Compare</span>
                </>
              )}
            </button>
          </div>
        </div>
      </div>

      {/* Adaptive Stream Result Container */}
      {hasContent && (
        <div className="pt-2 animate-fadeIn">
          <AdaptiveResponseRenderer
            rawText={rawText}
            phases={phases}
            decisions={decisions}
            comparisonRows={comparisonRows}
            status={status}
            metrics={metrics}
            basisFingerprint={basisFingerprint}
            fallbackNotices={fallbackNotices}
            projectId={projectId}
            onUndoRevert={() => onRefresh?.()}
          />
        </div>
      )}
    </div>
  );
};
