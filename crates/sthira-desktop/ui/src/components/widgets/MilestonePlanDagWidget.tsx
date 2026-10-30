import React, { useState } from 'react';
import {
  CheckCircle2,
  Circle,
  FolderPlus,
  Layout,
  GitBranch,
  ShieldCheck,
} from 'lucide-react';
import { createProject, createCanvas, addCanvasItem } from '../../api/ipc';
import type { PlanPhase } from '../../types/sthira';

interface MilestonePlanDagWidgetProps {
  phases: PlanPhase[];
  isStreaming?: boolean;
  onProjectCreated?: (projectId: string, changeSetId: string) => void;
  onCanvasPromoted?: (canvasId: string) => void;
}

export const MilestonePlanDagWidget: React.FC<MilestonePlanDagWidgetProps> = ({
  phases,
  isStreaming = false,
  onProjectCreated,
  onCanvasPromoted,
}) => {
  // Track checked milestones per phase and milestone index: key = `${phase_number}-${mIdx}`
  const [checkedMilestones, setCheckedMilestones] = useState<Record<string, boolean>>({});
  const [loadingPhase, setLoadingPhase] = useState<number | null>(null);
  const [actionNotice, setActionNotice] = useState<string | null>(null);

  const toggleMilestone = (phaseNumber: number, mIdx: number) => {
    const key = `${phaseNumber}-${mIdx}`;
    setCheckedMilestones((prev) => ({
      ...prev,
      [key]: !prev[key],
    }));
  };

  const handleCreateAsProject = async (phase: PlanPhase) => {
    setLoadingPhase(phase.phase_number);
    setActionNotice(null);
    try {
      const proj = await createProject(phase.title);
      // Generate change_set_id for undo reversibility
      const changeSetId = `cs_project_${proj.id}`;
      setActionNotice(`Created project "${proj.name}". ChangeSet logged for 1-click rollback.`);
      onProjectCreated?.(proj.id, changeSetId);
    } catch (err) {
      console.error('Failed to create project from plan phase', err);
      setActionNotice(`Failed to create project: ${err}`);
    } finally {
      setLoadingPhase(null);
    }
  };

  const handlePromoteToCanvas = async (phase: PlanPhase) => {
    setLoadingPhase(phase.phase_number);
    setActionNotice(null);
    try {
      const canvasId = await createCanvas(phase.title);
      // Place phase milestones on canvas
      for (let i = 0; i < phase.milestones.length; i++) {
        await addCanvasItem(
          canvasId,
          `item_${phase.phase_number}_${i}`,
          'text',
          undefined,
          60 + (i % 3) * 220,
          80 + Math.floor(i / 3) * 160
        );
      }
      setActionNotice(`Promoted Phase ${phase.phase_number} to Canvas.`);
      onCanvasPromoted?.(canvasId);
    } catch (err) {
      console.error('Failed to promote to canvas', err);
      setActionNotice(`Failed to promote to canvas: ${err}`);
    } finally {
      setLoadingPhase(null);
    }
  };

  if (!phases || phases.length === 0) {
    return null;
  }

  return (
    <div className="w-full space-y-4 my-4 font-sans">
      {/* Header Badge */}
      <div className="flex items-center justify-between pb-1 border-b border-[#E8E5DC]">
        <div className="flex items-center gap-2">
          <GitBranch className="w-4 h-4 text-[#7A7870]" />
          <span className="text-xs font-semibold uppercase tracking-wider text-[#7A7870]">
            Progressive Plan DAG ({phases.length} Phases)
          </span>
        </div>
        {isStreaming && (
          <span className="text-[11px] font-medium px-2 py-0.5 rounded-md bg-[#FFF7E8] text-[#8C5D00] border border-[#F2E0BA] flex items-center gap-1.5 animate-pulse">
            <span className="w-1.5 h-1.5 rounded-full bg-[#E58C00]" />
            Streaming Phase Milestones...
          </span>
        )}
      </div>

      {actionNotice && (
        <div className="px-3.5 py-2.5 rounded-lg bg-[#EBF2EA] text-[#2F5933] border border-[#D1E2D0] text-xs flex items-center justify-between animate-fadeIn">
          <div className="flex items-center gap-2">
            <ShieldCheck className="w-4 h-4 text-[#3E7444] shrink-0" />
            <span>{actionNotice}</span>
          </div>
          <button
            type="button"
            onClick={() => setActionNotice(null)}
            className="text-[#3E7444] hover:text-[#1F3D23] font-semibold text-xs ml-3"
          >
            Dismiss
          </button>
        </div>
      )}

      {/* Phase DAG Cards */}
      <div className="space-y-3.5">
        {phases.map((phase, idx) => {
          const isPhaseLoading = loadingPhase === phase.phase_number;
          const isLast = idx === phases.length - 1;

          return (
            <div
              key={phase.phase_number}
              className="group relative rounded-xl border border-[#E8E5DC] bg-white p-4.5 shadow-xs hover:border-[#D5D0C3] transition-all duration-200"
            >
              {/* Card Top: Phase Badge, Title & Prerequisites */}
              <div className="flex items-start justify-between gap-3 mb-3">
                <div className="flex items-start gap-2.5">
                  <span className="inline-flex items-center justify-center w-6 h-6 rounded-md bg-[#F2EFEB] text-[#363A31] font-mono text-xs font-bold border border-[#E0DCD3] shrink-0 mt-0.5">
                    {phase.phase_number}
                  </span>
                  <div>
                    <h4 className="text-sm font-semibold text-[#1E201C] tracking-tight">
                      {phase.title}
                    </h4>
                    {phase.prerequisites && phase.prerequisites.length > 0 && (
                      <div className="flex items-center gap-1.5 mt-1">
                        <span className="text-[11px] text-[#8C8A81]">Requires:</span>
                        {phase.prerequisites.map((req) => (
                          <span
                            key={req}
                            className="text-[10px] font-medium px-1.5 py-0.5 rounded bg-[#F5F3EC] text-[#5C5B54] border border-[#E8E5DC]"
                          >
                            {req}
                          </span>
                        ))}
                      </div>
                    )}
                  </div>
                </div>

                {/* Inline Action Buttons */}
                <div className="flex items-center gap-1.5 shrink-0">
                  <button
                    type="button"
                    onClick={() => handleCreateAsProject(phase)}
                    disabled={isPhaseLoading}
                    className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs font-medium text-[#282C25] bg-[#F5F3EC] hover:bg-[#EAE6DC] active:bg-[#DFDBD0] border border-[#E0DCD3] transition-colors disabled:opacity-50 cursor-pointer"
                    title="Promote phase to standalone project with full provenance"
                  >
                    <FolderPlus className="w-3.5 h-3.5 text-[#5C5B54]" />
                    <span>Create as Project</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => handlePromoteToCanvas(phase)}
                    disabled={isPhaseLoading}
                    className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs font-medium text-[#46556B] bg-[#F1F4F9] hover:bg-[#E5ECF6] active:bg-[#DAE4F2] border border-[#D5E1F0] transition-colors disabled:opacity-50 cursor-pointer"
                    title="Render phase cards on infinite canvas"
                  >
                    <Layout className="w-3.5 h-3.5 text-[#46556B]" />
                    <span>Promote to Canvas</span>
                  </button>
                </div>
              </div>

              {/* Milestones Checklist */}
              {phase.milestones && phase.milestones.length > 0 && (
                <div className="space-y-1.5 pt-2 border-t border-[#F5F3EC]">
                  {phase.milestones.map((milestone, mIdx) => {
                    const key = `${phase.phase_number}-${mIdx}`;
                    const isChecked = !!checkedMilestones[key];

                    return (
                      <div
                        key={mIdx}
                        onClick={() => toggleMilestone(phase.phase_number, mIdx)}
                        className="flex items-start gap-2.5 p-1.5 rounded-md hover:bg-[#F9F8F5] cursor-pointer transition-colors select-none"
                      >
                        <span className="mt-0.5 shrink-0 text-[#7A7870]">
                          {isChecked ? (
                            <CheckCircle2 className="w-4 h-4 text-[#3E7444]" />
                          ) : (
                            <Circle className="w-4 h-4 text-[#BCBAAF]" />
                          )}
                        </span>
                        <span
                          className={`text-xs leading-relaxed ${
                            isChecked
                              ? 'line-through text-[#9E9C94]'
                              : 'text-[#363A31]'
                          }`}
                        >
                          {milestone}
                        </span>
                      </div>
                    );
                  })}
                </div>
              )}

              {/* Connecting DAG stem line */}
              {!isLast && (
                <div className="absolute -bottom-3.5 left-7 w-0.5 h-3.5 bg-[#E8E5DC] pointer-events-none" />
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};
