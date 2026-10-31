import React, { useState, useEffect, useCallback } from 'react';
import {
  Plus,
  Loader2,
  Play,
  Pause,
  Trash2,
  ChevronDown,
  ChevronRight,
  Clock,
  CalendarClock,
  FileText,
  Folder,
  AlertCircle,
  RefreshCw,
} from 'lucide-react';
import {
  listScheduledTopics,
  createScheduledTopic,
  triggerScheduledTopicRun,
  toggleScheduledTopicActive,
  deleteScheduledTopic,
  getWorkspaceTree,
  getProviderCatalog,
  onScheduledTopicReady,
} from '../api/ipc';
import type {
  ScheduledTopicView,
  ScheduledTopicCadence,
  ScheduledTopicType,
  ProviderModelEntry,
} from '../types/sthira';

interface ScheduledTopicsListProps {
  topicType: ScheduledTopicType;
  projectId?: string | null;
  onOpenArtifact?: (artifactId: string, projectId?: string | null) => void;
  onChanged?: () => void;
}

const CADENCE_OPTIONS: { value: ScheduledTopicCadence; label: string }[] = [
  { value: 'daily', label: 'Daily (Autonomous)' },
  { value: 'weekly', label: 'Weekly' },
  { value: 'manual', label: 'Manual Only' },
];

const cadenceBadgeClass = (cadence: ScheduledTopicCadence): string => {
  switch (cadence) {
    case 'daily':
      return 'bg-[#EAF3E7] text-[#2D5A27] border-[#CCDCCC]';
    case 'weekly':
      return 'bg-[#E7EEF7] text-[#2B4C6F] border-[#C9D8EA]';
    default:
      return 'bg-[#F1EFE9] text-[#6E6D66] border-[#E0DCD3]';
  }
};

export const formatRelativeTime = (timestamp?: number | null): string => {
  if (!timestamp) return 'Never run';
  const ms = timestamp > 1e11 ? timestamp : timestamp * 1000;
  const diff = Date.now() - ms;
  if (diff < 60_000) return 'Just now';
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  return `${Math.floor(diff / 86_400_000)}d ago`;
};

/** Phase 18.1: compact "time until" label for the next scheduled firing. */
export const formatUntil = (timestamp?: number | null): string => {
  if (!timestamp) return '';
  const ms = timestamp > 1e11 ? timestamp : timestamp * 1000;
  const diff = ms - Date.now();
  if (diff <= 0) return 'due now';
  if (diff < 60_000) return 'in <1m';
  if (diff < 3_600_000) return `in ${Math.floor(diff / 60_000)}m`;
  if (diff < 86_400_000) return `in ${Math.floor(diff / 3_600_000)}h`;
  return `in ${Math.floor(diff / 86_400_000)}d`;
};

export const ScheduledTopicsList: React.FC<ScheduledTopicsListProps> = ({
  topicType,
  projectId,
  onOpenArtifact,
  onChanged,
}) => {
  const [topics, setTopics] = useState<ScheduledTopicView[]>([]);
  const [projects, setProjects] = useState<{ id: string; name: string }[]>([]);
  const [models, setModels] = useState<ProviderModelEntry[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  // Creation form state
  const [title, setTitle] = useState('');
  const [objective, setObjective] = useState('');
  const [cadence, setCadence] = useState<ScheduledTopicCadence>('daily');
  const [scheduleTime, setScheduleTime] = useState<string>('06:00');
  const [scheduleWeekday, setScheduleWeekday] = useState<number>(0);
  const [targetProject, setTargetProject] = useState<string>(projectId || '');
  const [modelId, setModelId] = useState<string>('');
  const [isSaving, setIsSaving] = useState(false);

  // Per-topic transient UI state
  const [runningId, setRunningId] = useState<string | null>(null);
  const [expandedRuns, setExpandedRuns] = useState<Record<string, boolean>>({});
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  const loadTopics = useCallback(async () => {
    try {
      setIsLoading(true);
      const list = await listScheduledTopics(topicType);
      setTopics(list);
    } catch (err) {
      console.error('Failed to load scheduled topics', err);
      setError('Could not load scheduled topics.');
    } finally {
      setIsLoading(false);
    }
  }, [topicType]);

  useEffect(() => {
    loadTopics();
  }, [loadTopics]);

  useEffect(() => {
    (async () => {
      try {
        const [tree, catalog] = await Promise.all([getWorkspaceTree(), getProviderCatalog()]);
        setProjects(tree.projects.map((p) => ({ id: p.id, name: p.name })));
        setModels(
          catalog
            .filter((p) => p.is_enabled)
            .flatMap((p) => p.models.filter((m) => m.is_enabled))
        );
      } catch (err) {
        console.warn('ScheduledTopicsList: auxiliary load failed', err);
      }
    })();
  }, []);

  // Surface autonomous runs pushed by the desktop scheduler.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onScheduledTopicReady((evt) => {
      setToast(`${evt.topic_type === 'plan' ? 'Plan review' : 'Digest'} ready: ${evt.topic_title}`);
      loadTopics();
      onChanged?.();
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, [loadTopics, onChanged]);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim() || !objective.trim() || isSaving) return;
    setIsSaving(true);
    setError(null);
    try {
      await createScheduledTopic({
        title: title.trim(),
        prompt_query: objective.trim(),
        topic_type: topicType,
        project_id: targetProject || null,
        cadence,
        model_id: modelId || null,
        // Phase 18.1: a time is only meaningful for an autonomous cadence.
        schedule_time: cadence === 'manual' ? null : scheduleTime || null,
        schedule_weekday: cadence === 'weekly' ? scheduleWeekday : null,
      });
      setTitle('');
      setObjective('');
      setCadence('daily');
      setScheduleTime('06:00');
      setScheduleWeekday(0);
      setModelId('');
      setToast('Scheduled topic created');
      await loadTopics();
      onChanged?.();
    } catch (err) {
      console.error('Failed to create scheduled topic', err);
      setError('Failed to save the scheduled topic.');
    } finally {
      setIsSaving(false);
    }
  };

  const handleRunNow = async (topic: ScheduledTopicView) => {
    setRunningId(topic.id);
    setError(null);
    try {
      const artifactId = await triggerScheduledTopicRun(topic.id);
      await loadTopics();
      onChanged?.();
      setToast(
        topicType === 'plan' ? 'Plan review generated' : 'Digest generated'
      );
      if (artifactId && onOpenArtifact) {
        onOpenArtifact(artifactId, topic.project_id ?? null);
      }
    } catch (err) {
      console.error('Failed to run scheduled topic', err);
      setError('Run failed — check the search provider and model settings.');
    } finally {
      setRunningId(null);
    }
  };

  const handleToggleActive = async (topic: ScheduledTopicView) => {
    try {
      await toggleScheduledTopicActive(topic.id, !topic.is_active);
      setTopics((prev) =>
        prev.map((t) => (t.id === topic.id ? { ...t, is_active: !t.is_active } : t))
      );
      onChanged?.();
    } catch (err) {
      console.error('Failed to toggle topic', err);
      setError('Could not update the topic.');
    }
  };

  const handleDelete = async (topic: ScheduledTopicView) => {
    try {
      await deleteScheduledTopic(topic.id);
      setTopics((prev) => prev.filter((t) => t.id !== topic.id));
      setToast('Scheduled topic deleted');
      onChanged?.();
    } catch (err) {
      console.error('Failed to delete topic', err);
      setError('Could not delete the topic.');
    }
  };

  const runNoun = topicType === 'plan' ? 'Reviews' : 'Digests';


  return (
    <div className="w-full space-y-4 font-sans text-xs" data-testid="scheduled-topics-list">
      {/* ---------------------------------------------------------------- */}
      {/* Topic Creation Card                                              */}
      {/* ---------------------------------------------------------------- */}
      <form
        onSubmit={handleCreate}
        data-testid="scheduled-topic-create-card"
        className="space-y-2.5 p-3 rounded-xl bg-[#FAF8F3] dark:bg-[#161714] border border-[#E8E5DC] dark:border-[#2C2E27]"
      >
        <div className="flex items-center gap-1.5">
          <Plus className="w-3.5 h-3.5 text-[#7A7870] dark:text-[#9E9C93]" />
          <span className="font-semibold text-xs text-[#282C25] dark:text-[#EDEAE2]">
            New Scheduled {topicType === 'plan' ? 'Review' : 'Research'} Topic
          </span>
        </div>

        <input
          type="text"
          data-testid="input-topic-title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder={topicType === 'plan' ? 'e.g. Weekly Architecture Review' : 'e.g. SSD Streaming & NVMe'}
          className="w-full px-2.5 py-1.5 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-white dark:bg-[#141512] text-xs text-[#1E201C] dark:text-[#EDEAE2] placeholder-[#A09D94] dark:placeholder-[#6E6D66] outline-none focus:border-[#7A7870] dark:focus:border-[#9E9C93] transition-colors"
        />

        <textarea
          data-testid="input-topic-objective"
          value={objective}
          onChange={(e) => setObjective(e.target.value)}
          rows={3}
          placeholder={
            topicType === 'plan'
              ? 'What progress, blockers, or milestones should each review evaluate?'
              : 'What specific developments, benchmarks, or architectures should each run investigate?'
          }
          className="w-full px-2.5 py-1.5 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-white dark:bg-[#141512] text-xs text-[#1E201C] dark:text-[#EDEAE2] placeholder-[#A09D94] dark:placeholder-[#6E6D66] outline-none focus:border-[#7A7870] dark:focus:border-[#9E9C93] transition-colors resize-none leading-relaxed"
        />

        <div className="grid grid-cols-2 gap-2">
          <label className="space-y-1">
            <span className="block text-[10px] font-semibold uppercase tracking-wider text-[#8C8A81] dark:text-[#6E6D66]">
              Frequency
            </span>
            <select
              data-testid="select-topic-cadence"
              value={cadence}
              onChange={(e) => setCadence(e.target.value as ScheduledTopicCadence)}
              className="w-full px-2 py-1.5 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-white dark:bg-[#141512] text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none cursor-pointer focus:border-[#7A7870] transition-colors"
            >
              {CADENCE_OPTIONS.map((c) => (
                <option key={c.value} value={c.value}>
                  {c.label}
                </option>
              ))}
            </select>
          </label>

          <label className="space-y-1">
            <span className="block text-[10px] font-semibold uppercase tracking-wider text-[#8C8A81] dark:text-[#6E6D66]">
              Project (optional)
            </span>
            <select
              data-testid="select-topic-project"
              value={targetProject}
              onChange={(e) => setTargetProject(e.target.value)}
              className="w-full px-2 py-1.5 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-white dark:bg-[#141512] text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none cursor-pointer focus:border-[#7A7870] transition-colors"
            >
              <option value="">No project</option>
              {projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </label>
        </div>

        {/* Phase 18.1: Wall-clock scheduling */}
        {cadence !== 'manual' && (
          <div className="grid grid-cols-2 gap-2">
            <label className="space-y-1">
              <span className="block text-[10px] font-semibold uppercase tracking-wider text-[#8C8A81] dark:text-[#6E6D66]">
                Run at (UTC)
              </span>
              <input
                type="time"
                data-testid="input-topic-time"
                value={scheduleTime}
                onChange={(e) => setScheduleTime(e.target.value)}
                className="w-full px-2 py-1.5 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-white dark:bg-[#141512] text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none focus:border-[#7A7870] transition-colors cursor-pointer"
              />
            </label>

            {cadence === 'weekly' ? (
              <label className="space-y-1">
                <span className="block text-[10px] font-semibold uppercase tracking-wider text-[#8C8A81] dark:text-[#6E6D66]">
                  On day
                </span>
                <select
                  data-testid="select-topic-weekday"
                  value={scheduleWeekday}
                  onChange={(e) => setScheduleWeekday(Number(e.target.value))}
                  className="w-full px-2 py-1.5 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-white dark:bg-[#141512] text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none cursor-pointer focus:border-[#7A7870] transition-colors"
                >
                  {['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'].map(
                    (label, idx) => (
                      <option key={label} value={idx}>
                        {label}
                      </option>
                    )
                  )}
                </select>
              </label>
            ) : (
              <div className="space-y-1">
                <span className="block text-[10px] font-semibold uppercase tracking-wider text-[#8C8A81] dark:text-[#6E6D66]">
                  Fires
                </span>
                <p className="px-2 py-1.5 text-[11px] text-[#7A7870] dark:text-[#9E9C93]">
                  Every day at {scheduleTime || '--:--'}
                </p>
              </div>
            )}
          </div>
        )}

        <label className="space-y-1 block">
          <span className="block text-[10px] font-semibold uppercase tracking-wider text-[#8C8A81] dark:text-[#6E6D66]">
            Model Override
          </span>
          <select
            data-testid="select-topic-model"
            value={modelId}
            onChange={(e) => setModelId(e.target.value)}
            className="w-full px-2 py-1.5 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-white dark:bg-[#141512] text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none cursor-pointer focus:border-[#7A7870] transition-colors"
          >
            <option value="">Use workspace default</option>
            {models.map((m) => (
              <option key={m.id} value={m.id}>
                {m.display_name || m.model_identifier}
              </option>
            ))}
          </select>
        </label>

        <button
          type="submit"
          data-testid="btn-save-scheduled-topic"
          disabled={isSaving || !title.trim() || !objective.trim()}
          className="w-full inline-flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold text-white dark:text-[#121310] bg-[#282C25] dark:bg-[#EDEAE2] hover:bg-[#3D4238] dark:hover:bg-[#D4D1C7] disabled:opacity-40 transition-colors shadow-xs cursor-pointer"
        >
          {isSaving ? (
            <>
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
              <span>Saving...</span>
            </>
          ) : (
            <>
              <Plus className="w-3.5 h-3.5" />
              <span>Save Scheduled Topic</span>
            </>
          )}
        </button>
      </form>


      {/* Error / Toast Banners */}
      {error && (
        <div
          data-testid="scheduled-topics-error"
          className="p-2.5 rounded-xl bg-red-50 dark:bg-red-950/30 border border-red-200 dark:border-red-900/40 text-[11px] text-red-700 dark:text-red-400 flex items-start gap-2"
        >
          <AlertCircle className="w-3.5 h-3.5 shrink-0 mt-0.5" />
          <span>{error}</span>
        </div>
      )}
      {toast && !error && (
        <div
          data-testid="scheduled-topics-toast"
          className="p-2.5 rounded-xl bg-emerald-50 dark:bg-emerald-950/30 border border-emerald-200 dark:border-emerald-900/40 text-[11px] text-emerald-800 dark:text-emerald-300 flex items-center justify-between gap-2"
        >
          <span>{toast}</span>
          <button
            type="button"
            onClick={() => setToast(null)}
            className="text-emerald-700 dark:text-emerald-400 hover:opacity-70 cursor-pointer"
          >
            ✕
          </button>
        </div>
      )}

      {/* ---------------------------------------------------------------- */}
      {/* Active Topics List                                                */}
      {/* ---------------------------------------------------------------- */}
      <div className="flex items-center justify-between">
        <span className="font-semibold text-xs text-[#282C25] dark:text-[#EDEAE2]">
          Active Topics ({topics.length})
        </span>
        <button
          type="button"
          data-testid="btn-refresh-topics"
          onClick={loadTopics}
          className="inline-flex items-center gap-1 text-[11px] text-[#7A7870] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-white transition-colors cursor-pointer"
        >
          <RefreshCw className="w-3 h-3" />
          Refresh
        </button>
      </div>

      {isLoading ? (
        <div className="py-8 text-center text-[11px] text-[#8C8A81] dark:text-[#6E6D66]">
          Loading scheduled topics...
        </div>
      ) : topics.length === 0 ? (
        <div className="py-8 px-4 text-center rounded-xl bg-[#FAF8F3] dark:bg-[#161714] border border-dashed border-[#E8E5DC] dark:border-[#2C2E27] space-y-1">
          <Clock className="w-4 h-4 mx-auto text-[#8C8A81] dark:text-[#6E6D66]" />
          <p className="text-[11px] text-[#6E6D66] dark:text-[#9E9C93]">
            No scheduled topics yet. Define one above to run it on a cadence or on demand.
          </p>
        </div>
      ) : (
        <div className="space-y-2.5">
          {topics.map((topic) => {
            const isRunning = runningId === topic.id;
            const runsOpen = !!expandedRuns[topic.id];
            return (
              <div
                key={topic.id}
                data-testid={`topic-card-${topic.id}`}
                className="p-3 rounded-xl bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] space-y-2 shadow-2xs"
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0 space-y-0.5">
                    <div className="flex items-center gap-1.5 flex-wrap">
                      <span className="font-semibold text-xs text-[#1E201C] dark:text-[#EDEAE2] truncate">
                        {topic.title}
                      </span>
                      <span
                        data-testid={`cadence-badge-${topic.id}`}
                        className={`text-[9px] font-bold uppercase tracking-wider px-1.5 py-0.5 rounded border ${cadenceBadgeClass(
                          topic.cadence
                        )}`}
                      >
                        {topic.cadence}
                      </span>
                      {!topic.is_active && (
                        <span className="text-[9px] font-bold uppercase tracking-wider px-1.5 py-0.5 rounded border bg-[#F1EFE9] text-[#8C8A81] border-[#E0DCD3]">
                          paused
                        </span>
                      )}
                    </div>
                    <p className="text-[11px] text-[#7A7870] dark:text-[#9E9C93] line-clamp-2 leading-relaxed">
                      {topic.prompt_query}
                    </p>
                    <div className="flex items-center gap-2 text-[10px] text-[#8C8A81] dark:text-[#6E6D66]">
                      <span className="inline-flex items-center gap-1">
                        <Clock className="w-2.5 h-2.5" />
                        {formatRelativeTime(topic.last_run_at)}
                      </span>
                      {topic.project_name && (
                        <span className="inline-flex items-center gap-1">
                          <Folder className="w-2.5 h-2.5" />
                          {topic.project_name}
                        </span>
                      )}
                      {topic.schedule_label && (
                        <span
                          data-testid={`schedule-label-${topic.id}`}
                          className="inline-flex items-center gap-1"
                        >
                          <CalendarClock className="w-2.5 h-2.5" />
                          {topic.schedule_label}
                        </span>
                      )}
                      {topic.is_active && topic.next_run_at && (
                        <span
                          data-testid={`next-run-${topic.id}`}
                          className="text-[#8C8A81] dark:text-[#6E6D66]"
                        >
                          Next {formatUntil(topic.next_run_at)}
                        </span>
                      )}
                    </div>
                  </div>


                  <div className="flex items-center gap-1 shrink-0">
                    <button
                      type="button"
                      data-testid={`btn-run-now-${topic.id}`}
                      onClick={() => handleRunNow(topic)}
                      disabled={isRunning}
                      title="Run now"
                      className="inline-flex items-center gap-1 px-2 py-1 rounded-lg text-[11px] font-semibold text-white dark:text-[#121310] bg-[#282C25] dark:bg-[#EDEAE2] hover:bg-[#3D4238] dark:hover:bg-[#D4D1C7] disabled:opacity-50 transition-colors cursor-pointer"
                    >
                      {isRunning ? (
                        <>
                          <Loader2 className="w-3 h-3 animate-spin" />
                          <span>Running</span>
                        </>
                      ) : (
                        <>
                          <Play className="w-3 h-3" />
                          <span>Run Now</span>
                        </>
                      )}
                    </button>

                    <button
                      type="button"
                      data-testid={`btn-toggle-active-${topic.id}`}
                      onClick={() => handleToggleActive(topic)}
                      title={topic.is_active ? 'Pause topic' : 'Activate topic'}
                      className={`p-1 rounded-lg border transition-colors cursor-pointer ${
                        topic.is_active
                          ? 'bg-[#EAF3E7] border-[#CCDCCC] text-[#2D5A27]'
                          : 'bg-[#F1EFE9] border-[#E0DCD3] text-[#8C8A81]'
                      }`}
                    >
                      {topic.is_active ? (
                        <Pause className="w-3 h-3" />
                      ) : (
                        <Play className="w-3 h-3" />
                      )}
                    </button>

                    <button
                      type="button"
                      data-testid={`btn-delete-topic-${topic.id}`}
                      onClick={() => handleDelete(topic)}
                      title="Delete topic"
                      className="p-1 rounded-lg text-[#8C8A81] hover:text-red-600 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/30 transition-colors cursor-pointer"
                    >
                      <Trash2 className="w-3 h-3" />
                    </button>
                  </div>
                </div>

                {/* Collapsible run history */}
                {topic.runs.length > 0 && (
                  <div className="pt-1.5 border-t border-[#F0EDE6] dark:border-[#262822]">
                    <button
                      type="button"
                      data-testid={`btn-toggle-runs-${topic.id}`}
                      onClick={() =>
                        setExpandedRuns((prev) => ({ ...prev, [topic.id]: !prev[topic.id] }))
                      }
                      className="w-full inline-flex items-center gap-1 text-[10px] font-semibold uppercase tracking-wider text-[#8C8A81] dark:text-[#6E6D66] hover:text-[#1E201C] dark:hover:text-white transition-colors cursor-pointer"
                    >
                      {runsOpen ? (
                        <ChevronDown className="w-3 h-3" />
                      ) : (
                        <ChevronRight className="w-3 h-3" />
                      )}
                      <span>
                        Previous {runNoun} ({topic.runs.length})
                      </span>
                    </button>

                    {runsOpen && (
                      <div className="mt-1.5 space-y-1" data-testid={`runs-list-${topic.id}`}>
                        {topic.runs.map((run) => (
                          <button
                            key={run.id}
                            type="button"
                            data-testid={`run-item-${run.artifact_id}`}
                            onClick={() => onOpenArtifact?.(run.artifact_id, topic.project_id ?? null)}
                            className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-left hover:bg-[#FAF8F3] dark:hover:bg-[#22241F] transition-colors cursor-pointer"
                          >
                            <FileText className="w-3 h-3 shrink-0 text-[#7A7870] dark:text-[#9E9C93]" />
                            <span className="min-w-0 flex-1">
                              <span className="block truncate text-[11px] text-[#1E201C] dark:text-[#EDEAE2]">
                                {run.artifact_title}
                              </span>
                              <span className="block text-[9px] text-[#8C8A81] dark:text-[#6E6D66]">
                                {formatRelativeTime(run.executed_at)} ·{' '}
                                {run.sources_scraped_count} source
                                {run.sources_scraped_count === 1 ? '' : 's'}
                              </span>
                            </span>
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};

export default ScheduledTopicsList;

