import React from 'react';
import {
  ChevronUp,
  Clock,
  Sparkles,
  Maximize2,
  Minimize2,
  CheckCircle2,
} from 'lucide-react';
import type { ResumeBrief } from '../types/sthira';

interface ContextualResumePaneProps {
  brief: ResumeBrief;
  onExpandReview?: () => void;
  isExpandedReview?: boolean;
  onSelectArtifact?: (artifactId: string) => void;
  isOpen: boolean;
  onToggleOpen: () => void;
}

export const ContextualResumePane: React.FC<ContextualResumePaneProps> = ({
  brief,
  onExpandReview,
  isExpandedReview = false,
  onSelectArtifact,
  isOpen,
  onToggleOpen,
}) => {
  if (!isOpen) {
    return (
      <button
        type="button"
        onClick={onToggleOpen}
        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-[#F1EEE6] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2A2C25] text-[#1E201C] dark:text-[#EDEAE2] text-xs font-semibold shadow-2xs transition-colors cursor-pointer"
      >
        <Sparkles className="w-3.5 h-3.5 text-[#49583D] dark:text-[#879E78]" />
        <span>Resume Brief</span>
      </button>
    );
  }

  return (
    <div className="bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl shadow-xs p-5 w-full relative overflow-hidden transition-colors">
      {/* Decorative corner curve */}
      <div
        className="absolute -bottom-8 -right-8 w-28 h-28 rounded-full bg-[#FAF8F3] dark:bg-[#23251F] opacity-60 pointer-events-none"
        aria-hidden="true"
      />

      {/* Header */}
      <div className="flex items-center justify-between pb-3.5 border-b border-[#E8E5DC]/80 dark:border-[#2C2E27]">
        <div className="flex items-center gap-2.5">
          <div className="w-7 h-7 rounded-lg bg-[#EEF1E8] dark:bg-[#283427] text-[#49583D] dark:text-[#879E78] flex items-center justify-center">
            <Sparkles className="w-4 h-4" />
          </div>
          <div>
            <h3 className="text-sm font-bold font-editorial text-[#1E201C] dark:text-[#EDEAE2]">
              {isExpandedReview ? 'Expanded Review (Level 2)' : 'Contextual Resume Brief'}
            </h3>
            <p className="text-[11px] text-[#73766D] dark:text-[#8C8F85]">
              {brief.active_working_breadcrumb || 'Active Thread'}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          {onExpandReview && (
            <button
              type="button"
              onClick={onExpandReview}
              className="inline-flex items-center gap-1 px-3 py-1 rounded-xl bg-[#F1EEE6] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-xs font-medium text-[#1E201C] dark:text-[#EDEAE2] transition-colors cursor-pointer"
            >
              {isExpandedReview ? (
                <>
                  <Minimize2 className="w-3 h-3" />
                  <span>Compact</span>
                </>
              ) : (
                <>
                  <Maximize2 className="w-3 h-3" />
                  <span>Expand Review</span>
                </>
              )}
            </button>
          )}

          <button
            type="button"
            onClick={onToggleOpen}
            aria-label="Collapse Brief"
            className="text-[#73766D] dark:text-[#8C8F85] hover:text-[#1E201C] dark:hover:text-white p-1 rounded-lg hover:bg-[#F1EEE6] dark:hover:bg-[#22241F] transition-colors cursor-pointer"
          >
            <ChevronUp className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Content: 2-Column Responsive Layout */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mt-4">
        {/* Purpose */}
        <div>
          <span className="text-[10px] font-bold text-[#73766D] dark:text-[#8C8F85] uppercase tracking-wider block mb-1">
            Purpose
          </span>
          <p className="text-xs sm:text-sm text-[#1E201C] dark:text-[#EDEAE2] leading-relaxed font-normal">
            {brief.purpose || 'Working document for active investigation and research.'}
          </p>
        </div>

        {/* Recent Grounded Findings */}
        <div>
          <span className="text-[10px] font-bold text-[#73766D] dark:text-[#8C8F85] uppercase tracking-wider block mb-1.5">
            Recent Grounded Findings
          </span>
          {(!brief.recent_progress || brief.recent_progress.length === 0) ? (
            <p className="text-xs text-[#73766D] dark:text-[#8C8F85] italic">
              No previous claims recorded yet.
            </p>
          ) : (
            <div className="space-y-2">
              {brief.recent_progress.slice(0, 3).map((claim, idx) => (
                <div key={idx} className="flex items-start gap-2 text-xs leading-relaxed">
                  <CheckCircle2 className="w-3.5 h-3.5 text-[#49583D] dark:text-[#879E78] shrink-0 mt-0.5" />
                  <div className="min-w-0 flex-1">
                    <span className="text-[#1E201C] dark:text-[#EDEAE2]">
                      {claim.text}
                    </span>
                    {claim.artifact_ids.length > 0 && onSelectArtifact && (
                      <button
                        type="button"
                        onClick={() => onSelectArtifact(claim.artifact_ids[0])}
                        className="ml-1.5 inline-flex items-center gap-0.5 text-[10px] text-[#49583D] dark:text-[#879E78] hover:underline"
                      >
                        [source]
                      </button>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Continuation Point Bottom Row */}
      {brief.continuation_point && (
        <div className="mt-4 pt-3.5 border-t border-[#E8E5DC]/80 dark:border-[#2C2E27] flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2.5">
          <div className="flex items-center gap-2 text-xs text-[#62665D] dark:text-[#B1B5AB]">
            <Clock className="w-3.5 h-3.5 text-[#49583D] dark:text-[#879E78] shrink-0" />
            <span>
              Next step: <strong className="text-[#1E201C] dark:text-[#EDEAE2] font-semibold">{brief.continuation_point}</strong>
            </span>
          </div>
          {brief.primary_continuation_artifact_id && onSelectArtifact && (
            <button
              type="button"
              onClick={() => onSelectArtifact(brief.primary_continuation_artifact_id!)}
              className="px-3 py-1 rounded-xl bg-[#49583D] dark:bg-[#2F3E28] hover:bg-[#39472F] text-white text-xs font-semibold inline-flex items-center gap-1 transition-colors self-start sm:self-auto cursor-pointer shadow-2xs"
            >
              <span>Resume</span>
              <span>→</span>
            </button>
          )}
        </div>
      )}
    </div>
  );
};
