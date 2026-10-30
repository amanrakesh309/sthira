import React from 'react';
import { History, X, Clock, FileText } from 'lucide-react';
import { EpistemicBadge } from './EpistemicBadge';
import type { ProjectHistory, EpistemicOrigin } from '../types/sthira';

interface ProjectHistoryDrawerProps {
  history: ProjectHistory;
  isOpen: boolean;
  onClose: () => void;
  onSelectArtifact?: (artifactId: string) => void;
}

export const ProjectHistoryDrawer: React.FC<ProjectHistoryDrawerProps> = ({
  history,
  isOpen,
  onClose,
  onSelectArtifact,
}) => {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-y-0 right-0 z-50 w-full max-w-md bg-[#FFFFFF] border-l border-[#E8E5DC] shadow-2xl flex flex-col animate-in slide-in-from-right duration-200">
      {/* Header */}
      <div className="p-5 border-b border-[#ECE9E0] flex items-center justify-between">
        <div className="flex items-center gap-2.5">
          <div className="w-8 h-8 rounded-xl bg-[#EFECE4] text-[#282C25] flex items-center justify-center">
            <History className="w-4 h-4" />
          </div>
          <div>
            <h3 className="text-base font-bold text-[#1E201C]">Project History</h3>
            <p className="text-xs text-[#6E6D66]">Evolution narrative and audited milestones</p>
          </div>
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label="Close Drawer"
          className="p-1.5 rounded-lg text-[#6E6D66] hover:text-[#1E201C] hover:bg-[#EFECE4] transition-colors"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto p-5 space-y-6">
        {/* Segmented Evolution Narrative */}
        <div>
          <h4 className="text-xs font-bold text-[#6E6D66] uppercase tracking-wider mb-2.5">
            Evolution Narrative
          </h4>
          <div className="space-y-2">
            {history.narrative_segments.map((seg, idx) => (
              <div
                key={idx}
                className={`p-3.5 rounded-xl text-xs leading-relaxed ${
                  seg.is_ai_interpretation
                    ? 'bg-[#F9F5EC] border border-dashed border-[#E5D7B7] text-[#5C4813]'
                    : 'bg-[#FAF8F3] border border-[#E8E5DC] text-[#1E201C]'
                }`}
              >
                <div className="flex items-center justify-between mb-1">
                  <span className="text-[10px] font-bold uppercase tracking-wider text-[#6E6D66]">
                    {seg.is_ai_interpretation ? 'AI Interpretation' : 'Recorded Segment'}
                  </span>
                </div>
                <p>{seg.content}</p>
              </div>
            ))}
          </div>
        </div>

        {/* Milestone Timeline */}
        <div>
          <h4 className="text-xs font-bold text-[#6E6D66] uppercase tracking-wider mb-3">
            Milestone Timeline
          </h4>
          <div className="relative pl-6 space-y-4 before:absolute before:left-2 before:top-2 before:bottom-2 before:w-0.5 before:bg-[#E8E5DC]">
            {history.milestones.map((m) => {
              const origin: EpistemicOrigin =
                m.attribution === 'you_recorded'
                  ? 'user_recorded'
                  : m.attribution === 'supported_by_source'
                  ? 'source_backed'
                  : 'ai_inferred';

              return (
                <div key={m.seq} className="relative">
                  <div className="absolute -left-6 top-1.5 w-2 h-2 rounded-full bg-[#282C25] ring-4 ring-[#FFFFFF]" />
                  <div className="bg-[#FAF8F3] border border-[#E8E5DC] rounded-xl p-3 text-xs">
                    <div className="flex items-center justify-between mb-1.5">
                      <EpistemicBadge origin={origin} />
                      <span className="text-[10px] text-[#6E6D66] flex items-center gap-1">
                        <Clock className="w-3 h-3" />
                        Seq #{m.seq}
                      </span>
                    </div>
                    <div className="font-semibold text-[#1E201C] mb-1">
                      {m.title}
                    </div>
                    {m.artifact_id && onSelectArtifact && (
                      <button
                        type="button"
                        onClick={() => onSelectArtifact(m.artifact_id!)}
                        className="inline-flex items-center gap-1 text-[11px] text-[#6E6D66] hover:text-[#1E201C] font-medium mt-1"
                      >
                        <FileText className="w-3 h-3" />
                        View artifact
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
};
