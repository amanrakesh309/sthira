import React, { useState } from 'react';
import { HelpCircle, CheckCircle, Sparkles, BookOpen, Check } from 'lucide-react';
import { createPage, saveArtifact } from '../../api/ipc';
import type { DecisionPoint } from '../../types/sthira';

interface InteractiveDecisionSelectorWidgetProps {
  decisions: DecisionPoint[];
  isStreaming?: boolean;
  projectId?: string | null;
  onDecisionRecorded?: (decisionId: string, chosenOption: string, artifactId?: string) => void;
}

export const InteractiveDecisionSelectorWidget: React.FC<InteractiveDecisionSelectorWidgetProps> = ({
  decisions,
  isStreaming = false,
  projectId,
  onDecisionRecorded,
}) => {
  // Store selected option per decision id
  const [selectedChoices, setSelectedChoices] = useState<Record<string, string>>({});
  const [recordedNotices, setRecordedNotices] = useState<Record<string, string>>({});
  const [recordingId, setRecordingId] = useState<string | null>(null);

  const handleSelectOption = async (decision: DecisionPoint, option: string) => {
    setSelectedChoices((prev) => ({ ...prev, [decision.id]: option }));
    setRecordingId(decision.id);

    try {
      // Create a rationale note for the recorded decision
      const noteTitle = `Decision: ${decision.question.slice(0, 50)}...`;
      const noteContent = `### Recorded Architectural Decision\n\n**Question:** ${decision.question}\n\n**Selected Option:** ${option}\n\n${
        decision.recommended
          ? `**Model Recommendation:** ${decision.recommended}\n`
          : ''
      }\n**Epistemic Provenance:** [Recorded] (User-directed choice during adaptive synthesis)`;

      const art = await createPage(
        projectId || 'default_project',
        noteTitle
      );
      await saveArtifact(art.id, noteContent, noteTitle, projectId || 'default_project');

      const artId = art.id;
      setRecordedNotices((prev) => ({
        ...prev,
        [decision.id]: `Choice recorded as note "${noteTitle}" [Recorded]`,
      }));

      onDecisionRecorded?.(decision.id, option, artId);
    } catch (err) {
      console.error('Failed to record decision note', err);
      setRecordedNotices((prev) => ({
        ...prev,
        [decision.id]: `Selection noted locally: ${option}`,
      }));
      onDecisionRecorded?.(decision.id, option);
    } finally {
      setRecordingId(null);
    }
  };

  if (!decisions || decisions.length === 0) {
    return null;
  }

  return (
    <div className="w-full space-y-3.5 my-4 font-sans">
      {/* Header */}
      <div className="flex items-center justify-between pb-1 border-b border-[#E8E5DC]">
        <div className="flex items-center gap-2">
          <HelpCircle className="w-4 h-4 text-[#7A7870]" />
          <span className="text-xs font-semibold uppercase tracking-wider text-[#7A7870]">
            Interactive Decisions & Trade-offs ({decisions.length})
          </span>
        </div>
        {isStreaming && (
          <span className="text-[11px] font-medium px-2 py-0.5 rounded-md bg-[#FFF7E8] text-[#8C5D00] border border-[#F2E0BA] flex items-center gap-1.5 animate-pulse">
            <span className="w-1.5 h-1.5 rounded-full bg-[#E58C00]" />
            Identifying Decision Points...
          </span>
        )}
      </div>

      {/* Decision Cards */}
      <div className="space-y-3">
        {decisions.map((decision) => {
          const selected = selectedChoices[decision.id];
          const notice = recordedNotices[decision.id];
          const isRecording = recordingId === decision.id;

          return (
            <div
              key={decision.id}
              className="rounded-xl border border-[#E8E5DC] bg-white p-4 shadow-xs"
            >
              {/* Question */}
              <div className="flex items-start gap-2.5 mb-3">
                <span className="inline-flex items-center justify-center w-5 h-5 rounded-md bg-[#F2EFEB] text-[#363A31] text-[11px] font-bold shrink-0 mt-0.5">
                  ?
                </span>
                <div>
                  <h4 className="text-xs font-semibold text-[#1E201C] leading-snug">
                    {decision.question}
                  </h4>
                  {notice && (
                    <div className="mt-1 flex items-center gap-1.5 text-[11px] text-[#2F5933]">
                      <CheckCircle className="w-3.5 h-3.5 text-[#3E7444] shrink-0" />
                      <span>{notice}</span>
                      <span className="px-1.5 py-0.2 rounded bg-[#EBF2EA] text-[#2F5933] border border-[#D1E2D0] font-mono text-[9px] flex items-center gap-0.5">
                        <BookOpen className="w-2.5 h-2.5" />
                        Recorded
                      </span>
                    </div>
                  )}
                </div>
              </div>

              {/* Options Pills */}
              <div className="flex flex-wrap gap-2 pt-1">
                {decision.options.map((option, optIdx) => {
                  const isSelected = selected === option;
                  const isRecommended = decision.recommended === option;

                  return (
                    <button
                      key={optIdx}
                      type="button"
                      onClick={() => handleSelectOption(decision, option)}
                      disabled={isRecording}
                      className={`group relative text-left px-3 py-2 rounded-lg text-xs transition-all duration-150 flex items-center gap-2 border cursor-pointer ${
                        isSelected
                          ? 'bg-[#EBF2EA] text-[#1E3B21] border-[#B9D5B7] shadow-xs'
                          : 'bg-[#FAF9F6] text-[#363A31] border-[#E8E5DC] hover:border-[#D5D0C3] hover:bg-[#F4F1EB]'
                      }`}
                    >
                      <div className="flex items-center gap-1.5">
                        {isSelected ? (
                          <Check className="w-3.5 h-3.5 text-[#3E7444] shrink-0" />
                        ) : (
                          <div className="w-3 h-3 rounded-full border border-[#BCBAAF] group-hover:border-[#7A7870] shrink-0" />
                        )}
                        <span className="font-medium">{option}</span>
                      </div>

                      {isRecommended && (
                        <span className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded text-[10px] font-medium bg-[#FFF7E8] text-[#8C5D00] border border-[#F2E0BA] ml-1">
                          <Sparkles className="w-2.5 h-2.5 text-[#E58C00]" />
                          Recommended
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};
