import React from 'react';
import {
  ChevronUp,
  FileText,
  Clock,
  Sparkles,
  Maximize2,
  Minimize2,
} from 'lucide-react';
import { EpistemicBadge } from './EpistemicBadge';
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
        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-[#EFECE4] hover:bg-[#E5E1D6] text-[#3A3B36] text-xs font-semibold shadow-sm transition-colors"
      >
        <Sparkles className="w-3.5 h-3.5 text-[#282C25]" />
        Brief
      </button>
    );
  }

  return (
    <div className="bg-[#FFFFFF] border border-[#E8E5DC] rounded-2xl shadow-sm p-5 w-full">
      {/* Header */}
      <div className="flex items-center justify-between pb-3 border-b border-[#ECE9E0]">
        <div className="flex items-center gap-2">
          <div className="w-6 h-6 rounded-lg bg-[#EFECE4] text-[#282C25] flex items-center justify-center">
            <Sparkles className="w-3.5 h-3.5" />
          </div>
          <div>
            <h3 className="text-sm font-bold text-[#1E201C]">
              {isExpandedReview ? 'Expanded Review (Level 2)' : 'Contextual Resume Brief'}
            </h3>
            <p className="text-[11px] text-[#6E6D66]">
              {brief.active_working_breadcrumb}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-1.5">
          {onExpandReview && (
            <button
              type="button"
              onClick={onExpandReview}
              className="inline-flex items-center gap-1 px-2.5 py-1 rounded-lg bg-[#EFECE4] hover:bg-[#E5E1D6] text-xs font-medium text-[#3A3B36] transition-colors"
            >
              {isExpandedReview ? (
                <>
                  <Minimize2 className="w-3 h-3" />
                  Compact
                </>
              ) : (
                <>
                  <Maximize2 className="w-3 h-3" />
                  Expand Review
                </>
              )}
            </button>
          )}

          <button
            type="button"
            onClick={onToggleOpen}
            aria-label="Collapse Brief"
            className="text-[#6E6D66] hover:text-[#1E201C] p-1 rounded-lg hover:bg-[#EFECE4] transition-colors"
          >
            <ChevronUp className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Purpose */}
      <div className="mt-3.5">
        <span className="text-[11px] font-semibold text-[#6E6D66] uppercase tracking-wider">
          Purpose
        </span>
        <p className="text-sm text-[#1E201C] mt-1 leading-relaxed font-normal">
          {brief.purpose}
        </p>
      </div>

      {/* Recent Progress / Claims */}
      <div className="mt-4">
        <span className="text-[11px] font-semibold text-[#6E6D66] uppercase tracking-wider">
          Recent Grounded Findings
        </span>
        <div className="mt-2 space-y-2.5">
          {(brief.recent_progress || []).map((claim, idx) => (
            <div
              key={idx}
              className="bg-[#FAF8F3] border border-[#E8E5DC] rounded-xl p-3 text-xs leading-relaxed"
            >
              <div className="flex items-center justify-between mb-1.5">
                <EpistemicBadge origin={claim.origin} />
                {claim.artifact_ids.length > 0 && onSelectArtifact && (
                  <button
                    type="button"
                    onClick={() => onSelectArtifact(claim.artifact_ids[0])}
                    className="inline-flex items-center gap-1 text-[11px] text-[#6E6D66] hover:text-[#1E201C] font-medium"
                  >
                    <FileText className="w-3 h-3" />
                    Source citation
                  </button>
                )}
              </div>
              <p className="text-[#1E201C] font-medium italic">
                &ldquo;{claim.text}&rdquo;
              </p>
            </div>
          ))}
        </div>
      </div>

      {/* Continuation Point Breadcrumb */}
      {brief.continuation_point && (
        <div className="mt-4 pt-3 border-t border-[#ECE9E0] flex items-center justify-between">
          <div className="flex items-center gap-2 text-xs text-[#6E6D66]">
            <Clock className="w-3.5 h-3.5 text-[#282C25]" />
            <span>Next step: <strong className="text-[#1E201C] font-semibold">{brief.continuation_point}</strong></span>
          </div>
          {brief.primary_continuation_artifact_id && onSelectArtifact && (
            <button
              type="button"
              onClick={() => onSelectArtifact(brief.primary_continuation_artifact_id!)}
              className="text-xs font-semibold text-[#282C25] hover:underline"
            >
              Resume →
            </button>
          )}
        </div>
      )}
    </div>
  );
};
