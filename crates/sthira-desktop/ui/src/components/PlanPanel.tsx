import { useState } from 'react';
import { ClipboardList, Sparkles, Loader2 } from 'lucide-react';
import { useAdaptiveStream } from '../hooks/useAdaptiveStream';
import { AdaptiveResponseRenderer } from './AdaptiveResponseRenderer';

interface PlanPanelProps {
  projectId?: string | null;
  onRefresh?: () => void;
  onProjectCreated?: (projectId: string, changeSetId: string) => void;
}

export const PlanPanel: React.FC<PlanPanelProps> = ({
  projectId,
  onRefresh,
  onProjectCreated,
}) => {
  const [goalInput, setGoalInput] = useState('');
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
  const hasContent = rawText.length > 0 || phases.length > 0;

  const handleRunAdaptivePlan = async () => {
    if (!goalInput.trim() || isStreaming) return;
    await startStream(goalInput.trim(), 'plan');
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      handleRunAdaptivePlan();
    }
  };

  return (
    <div className="w-full space-y-4 font-sans text-xs">
      {/* Intro Description */}
      <div className="flex items-center justify-between text-[#66645D] dark:text-[#9E9C93]">
        <div className="flex items-center gap-1.5">
          <ClipboardList className="w-4 h-4 text-[#7A7870] dark:text-[#9E9C93]" />
          <span className="font-semibold text-xs text-[#282C25] dark:text-[#EDEAE2]">
            Adaptive Execution Planning
          </span>
        </div>
        <span className="text-[11px] text-[#8C8A81] dark:text-[#6E6D66]">Progressive DAG & Milestones</span>
      </div>

      {/* Goal Input Area */}
      <div className="space-y-2">
        <textarea
          value={goalInput}
          onChange={(e) => setGoalInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Enter objective or engineering initiative to plan (e.g. 'Build distributed event pipeline with SQLite')..."
          rows={3}
          className="w-full p-3 rounded-xl border border-[#E8E5DC] dark:border-[#2C2E27] bg-white dark:bg-[#141512] text-xs text-[#1E201C] dark:text-[#EDEAE2] placeholder-[#A09D94] dark:placeholder-[#6E6D66] focus:outline-none focus:border-[#7A7870] dark:focus:border-[#9E9C93] transition-colors resize-none leading-relaxed shadow-2xs"
        />

        <div className="flex items-center justify-between">
          <span className="text-[11px] text-[#8C8A81] dark:text-[#6E6D66]">
            Press ⌘+Enter to plan
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
              onClick={handleRunAdaptivePlan}
              disabled={isStreaming || !goalInput.trim()}
              className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-lg text-xs font-semibold text-white dark:text-[#121310] bg-[#282C25] dark:bg-[#EDEAE2] hover:bg-[#3D4238] dark:hover:bg-[#D4D1C7] active:bg-[#1E201C] disabled:opacity-40 transition-colors shadow-xs cursor-pointer"
            >
              {isStreaming ? (
                <>
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  <span>Streaming DAG...</span>
                </>
              ) : (
                <>
                  <Sparkles className="w-3.5 h-3.5" />
                  <span>Generate Adaptive Plan</span>
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
            onProjectCreated={(id, csId) => {
              onProjectCreated?.(id, csId);
              onRefresh?.();
            }}
          />
        </div>
      )}
    </div>
  );
};
