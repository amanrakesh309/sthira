import React from 'react';
import { ArrowLeft, CalendarClock, Search, ClipboardList } from 'lucide-react';
import { ScheduledTopicsList } from '../components/ScheduledTopicsList';

interface ScheduledTopicsViewProps {
  onOpenArtifact?: (artifactId: string, projectId?: string | null) => void;
  onBack?: () => void;
  onWorkspaceMutated?: () => void;
}

/**
 * Phase 20: Scheduled & Continuous Research/Planning dashboard.
 *
 * Relocated out of the (now removed) right-hand mode rail into a dedicated full-width
 * view reachable from the Left Sidebar, alongside `All Files` and `Knowledge Graph`.
 *
 * Each section is a `ScheduledTopicsList` instance, which owns the creation form
 * (cadence + `Run at (UTC)` time + weekday), the active/paused toggles, and the
 * previous-run digests that link straight into `FocusView`.
 */
export const ScheduledTopicsView: React.FC<ScheduledTopicsViewProps> = ({
  onOpenArtifact,
  onBack,
  onWorkspaceMutated,
}) => {
  return (
    <div className="w-full h-full flex flex-col bg-[#FAF8F3] dark:bg-[#121310]">
      {/* Header */}
      <header className="w-full px-8 pt-7 pb-5 border-b border-[#E8E5DC] dark:border-[#262822]">
        <div className="max-w-5xl mx-auto flex items-start justify-between gap-4">
          <div className="flex items-start gap-3">
            {onBack && (
              <button
                type="button"
                data-testid="scheduled-topics-back"
                onClick={onBack}
                className="mt-1 p-1.5 rounded-xl bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-[#6E6D66] dark:text-[#9E9C93] transition-colors cursor-pointer"
                title="Back to Home"
              >
                <ArrowLeft className="w-3.5 h-3.5" />
              </button>
            )}
            <div>
              <h1 className="text-2xl font-bold text-[#1E201C] dark:text-[#EDEAE2] tracking-tight flex items-center gap-2">
                <CalendarClock className="w-5 h-5 text-[#282C25] dark:text-[#D4D1C7]" />
                <span>⏱️ Scheduled Feeds</span>
              </h1>
              <p className="text-sm text-[#6E6D66] dark:text-[#9E9C93] mt-1">
                Continuous research and planning that runs on its own schedule and files
                each digest back into your workspace.
              </p>
            </div>
          </div>
        </div>
      </header>

      {/* Body */}
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-5xl mx-auto px-8 py-8 space-y-10">
          {/* Research Feeds */}
          <section data-testid="scheduled-section-research">
            <div className="flex items-center gap-2 mb-3">
              <Search className="w-4 h-4 text-[#7A7870] dark:text-[#9E9C93]" />
              <h2 className="text-base font-bold text-[#1E201C] dark:text-[#EDEAE2]">
                Research Feeds
              </h2>
              <span className="text-[11px] text-[#8C8A81] dark:text-[#6E6D66]">
                Autonomous web &amp; vault investigations
              </span>
            </div>
            <div className="rounded-2xl bg-white dark:bg-[#181915] border border-[#E8E5DC] dark:border-[#262822] p-5 shadow-2xs">
              <ScheduledTopicsList
                topicType="research"
                projectId={null}
                onOpenArtifact={onOpenArtifact}
                onChanged={onWorkspaceMutated}
              />
            </div>
          </section>

          {/* Plan Feeds */}
          <section data-testid="scheduled-section-plan">
            <div className="flex items-center gap-2 mb-3">
              <ClipboardList className="w-4 h-4 text-[#7A7870] dark:text-[#9E9C93]" />
              <h2 className="text-base font-bold text-[#1E201C] dark:text-[#EDEAE2]">
                Plan Feeds
              </h2>
              <span className="text-[11px] text-[#8C8A81] dark:text-[#6E6D66]">
                Recurring milestone &amp; prerequisite reviews
              </span>
            </div>
            <div className="rounded-2xl bg-white dark:bg-[#181915] border border-[#E8E5DC] dark:border-[#262822] p-5 shadow-2xs">
              <ScheduledTopicsList
                topicType="plan"
                projectId={null}
                onOpenArtifact={onOpenArtifact}
                onChanged={onWorkspaceMutated}
              />
            </div>
          </section>
        </div>
      </div>
    </div>
  );
};

export default ScheduledTopicsView;
