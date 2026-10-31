import React, { useState, useEffect, useMemo, useCallback } from 'react';
import {
  Search,
  ClipboardList,
  Plus,
  Play,
  Pause,
  Trash2,
  Clock,
  Folder,
  Sparkles,
  Globe,
  Loader2,
  X,
  RefreshCw,
  ChevronRight,
  Filter,
} from 'lucide-react';
import {
  getWorkspaceIndex,
  getWorkspaceTree,
  getProviderCatalog,
  listScheduledTopics,
  createScheduledTopic,
  triggerScheduledTopicRun,
  toggleScheduledTopicActive,
  deleteScheduledTopic,
  onScheduledTopicReady,
  createResearchPage,
  createPage,
  saveArtifact,
} from '../api/ipc';
import { PageActionMenu } from '../components/PageActionMenu';
import type {
  ExplorePlanTab,
  ScheduledTopicView,
  ScheduledTopicCadence,
  ScheduledTopicType,
  ProviderModelEntry,
  WebSearchMode,
  WorkspaceIndexItem,
} from '../types/sthira';

export interface ExplorePlanViewProps {
  initialTab?: ExplorePlanTab;
  initialDigestFilter?: 'all' | 'research' | 'plan';
  onOpenArtifact?: (artifactId: string, projectId?: string | null) => void;
  onStartResearch?: (
    query: string,
    mode: WebSearchMode,
    projectId?: string | null
  ) => Promise<any>;
  onStartPlan?: (goal: string, projectId?: string | null) => Promise<any>;
  onBack?: () => void;
  onWorkspaceMutated?: () => void;
}

export interface DigestItem {
  id: string;
  title: string;
  artifact_type: 'research' | 'plan';
  project_id?: string | null;
  project_name?: string | null;
  snippet?: string;
  created_at: number;
  updated_at: number;
}

const formatRelativeTime = (timestamp?: number | null): string => {
  if (!timestamp) return 'Never run';
  const ms = timestamp > 1e11 ? timestamp : timestamp * 1000;
  const diff = Date.now() - ms;
  if (diff < 60_000) return 'Just now';
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  return `${Math.floor(diff / 86_400_000)}d ago`;
};

const formatUntil = (timestamp?: number | null): string => {
  if (!timestamp) return '';
  const ms = timestamp > 1e11 ? timestamp : timestamp * 1000;
  const diff = ms - Date.now();
  if (diff <= 0) return 'due now';
  if (diff < 60_000) return 'in <1m';
  if (diff < 3_600_000) return `in ${Math.floor(diff / 60_000)}m`;
  if (diff < 86_400_000) return `in ${Math.floor(diff / 3_600_000)}h`;
  return `in ${Math.floor(diff / 86_400_000)}d`;
};

const formatDate = (timestamp: number) => {
  if (!timestamp) return 'Just now';
  const date = new Date(timestamp > 1e11 ? timestamp : timestamp * 1000);
  return date.toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
    year: date.getFullYear() !== new Date().getFullYear() ? 'numeric' : undefined,
  });
};

const WEEKDAY_NAMES = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

export const ExplorePlanView: React.FC<ExplorePlanViewProps> = ({
  initialTab = 'research',
  initialDigestFilter = 'all',
  onOpenArtifact,
  onStartResearch,
  onStartPlan,
  onBack,
  onWorkspaceMutated,
}) => {
  const [activeTab, setActiveTab] = useState<ExplorePlanTab>(initialTab);

  // Tab 1: Research state
  const [researchQuery, setResearchQuery] = useState('');
  const [webSearchMode, setWebSearchMode] = useState<WebSearchMode>('auto');
  const [selectedResearchModel, setSelectedResearchModel] = useState<string>('');
  const [selectedResearchProject, setSelectedResearchProject] = useState<string | null>(null);
  const [isSubmittingResearch, setIsSubmittingResearch] = useState(false);

  // Tab 2: Plan state
  const [planGoal, setPlanGoal] = useState('');
  const [selectedPlanModel, setSelectedPlanModel] = useState<string>('');
  const [selectedPlanProject, setSelectedPlanProject] = useState<string | null>(null);
  const [isSubmittingPlan, setIsSubmittingPlan] = useState(false);

  // Tab 3: Trackers state
  const [topics, setTopics] = useState<ScheduledTopicView[]>([]);
  const [isNewTrackerOpen, setIsNewTrackerOpen] = useState(false);
  const [newTrackerType, setNewTrackerType] = useState<ScheduledTopicType>('research');
  const [newTrackerTitle, setNewTrackerTitle] = useState('');
  const [newTrackerPrompt, setNewTrackerPrompt] = useState('');
  const [newTrackerCadence, setNewTrackerCadence] = useState<ScheduledTopicCadence>('daily');
  const [newTrackerTime, setNewTrackerTime] = useState('06:00');
  const [newTrackerWeekday, setNewTrackerWeekday] = useState(0);
  const [newTrackerProject, setNewTrackerProject] = useState('');
  const [isSavingTracker, setIsSavingTracker] = useState(false);
  const [runningTopicId, setRunningTopicId] = useState<string | null>(null);

  // Tab 4: All Digests state
  const [digestSearch, setDigestSearch] = useState('');
  const [digestTypeFilter, setDigestTypeFilter] = useState<'all' | 'research' | 'plan'>(
    initialDigestFilter
  );

  // Shared data
  const [projects, setProjects] = useState<{ id: string; name: string }[]>([]);
  const [models, setModels] = useState<ProviderModelEntry[]>([]);
  const [notes, setNotes] = useState<WorkspaceIndexItem[]>([]);

  // Sync initial tab when changed from props
  useEffect(() => {
    if (initialTab) {
      setActiveTab(initialTab);
    }
  }, [initialTab]);

  // Load workspace data, topics, and models
  const loadData = useCallback(async () => {
    try {
      const [treeRes, catalogRes, indexRes, researchTopics, planTopics] = await Promise.all([
        getWorkspaceTree(),
        getProviderCatalog().catch(() => []),
        getWorkspaceIndex('notes').catch(() => ({ counts: {}, items: [] })),
        listScheduledTopics('research').catch(() => []),
        listScheduledTopics('plan').catch(() => []),
      ]);

      setProjects(treeRes.projects.map((p) => ({ id: p.id, name: p.name })));
      setNotes(indexRes.items || []);

      const availableModels: ProviderModelEntry[] = [];
      for (const provider of catalogRes) {
        if (provider.is_enabled && provider.models) {
          for (const m of provider.models) {
            if (m.is_enabled) {
              availableModels.push(m);
            }
          }
        }
      }
      setModels(availableModels);
      if (availableModels.length > 0) {
        const def = availableModels.find((m) => m.is_default) || availableModels[0];
        if (!selectedResearchModel) setSelectedResearchModel(def.model_identifier);
        if (!selectedPlanModel) setSelectedPlanModel(def.model_identifier);
      }

      // Combine topics
      const allTopics = [...researchTopics, ...planTopics].sort(
        (a, b) => b.created_at - a.created_at
      );
      setTopics(allTopics);
    } catch (err) {
      console.error('Failed to load explore & plan workspace data', err);
    }
  }, [selectedResearchModel, selectedPlanModel]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  // Subscribe to background scheduled topic ready events
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    onScheduledTopicReady(() => {
      loadData();
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [loadData]);

  // Derive all generated digests where artifact_type IN ('research', 'plan')
  const allDigests = useMemo<DigestItem[]>(() => {
    const digestsMap = new Map<string, DigestItem>();

    // 1. Ingest runs from scheduled topics
    for (const topic of topics) {
      for (const run of topic.runs || []) {
        if (!digestsMap.has(run.artifact_id)) {
          digestsMap.set(run.artifact_id, {
            id: run.artifact_id,
            title: run.artifact_title,
            artifact_type: topic.topic_type,
            project_id: topic.project_id,
            project_name: topic.project_name,
            snippet: `${topic.title} · ${topic.cadence} digest`,
            created_at: run.executed_at,
            updated_at: run.executed_at,
          });
        }
      }
    }

    // 2. Ingest notes from workspace index that match research or plan heuristics
    for (const item of notes) {
      const lowerTitle = (item.title || '').toLowerCase().trim();
      const lowerDesc = (item.description || '').toLowerCase();

      let detectedType: 'research' | 'plan' | null = null;
      if (
        lowerTitle.startsWith('research:') ||
        lowerTitle.startsWith('digest:') ||
        lowerDesc.includes('status: draft — researching') ||
        lowerDesc.includes('research inquiry')
      ) {
        detectedType = 'research';
      } else if (
        lowerTitle.startsWith('plan:') ||
        lowerTitle.startsWith('plan review:') ||
        lowerDesc.includes('status: drafting — planning') ||
        lowerDesc.includes('## goal')
      ) {
        detectedType = 'plan';
      }

      if (detectedType) {
        if (!digestsMap.has(item.id)) {
          digestsMap.set(item.id, {
            id: item.id,
            title: item.title,
            artifact_type: detectedType,
            project_id: item.project_id,
            project_name: item.project_name,
            snippet: item.description || undefined,
            created_at: item.created_at,
            updated_at: item.updated_at,
          });
        }
      }
    }

    return Array.from(digestsMap.values()).sort(
      (a, b) => (b.updated_at || b.created_at) - (a.updated_at || a.created_at)
    );
  }, [topics, notes]);

  // Filtered digests for Tab 4
  const filteredDigests = useMemo(() => {
    let list = allDigests;
    if (digestTypeFilter !== 'all') {
      list = list.filter((d) => d.artifact_type === digestTypeFilter);
    }
    const q = digestSearch.trim().toLowerCase();
    if (q) {
      list = list.filter(
        (d) =>
          d.title.toLowerCase().includes(q) ||
          (d.snippet && d.snippet.toLowerCase().includes(q)) ||
          (d.project_name && d.project_name.toLowerCase().includes(q))
      );
    }
    return list;
  }, [allDigests, digestTypeFilter, digestSearch]);

  // Recent 3 Research and Recent 3 Plans
  const recentResearch = useMemo(() => {
    return allDigests.filter((d) => d.artifact_type === 'research').slice(0, 3);
  }, [allDigests]);

  const recentPlans = useMemo(() => {
    return allDigests.filter((d) => d.artifact_type === 'plan').slice(0, 3);
  }, [allDigests]);

  // Active recurring topics count for badge
  const activeTrackersCount = useMemo(() => {
    return topics.filter((t) => t.is_active && t.cadence !== 'manual').length;
  }, [topics]);

  // Submit Research
  const handleSubmitResearch = async () => {
    const q = researchQuery.trim();
    if (!q || isSubmittingResearch) return;
    setIsSubmittingResearch(true);
    try {
      if (onStartResearch) {
        await onStartResearch(q, webSearchMode, selectedResearchProject);
      } else {
        const artifactId = await createResearchPage(q, selectedResearchProject);
        onOpenArtifact?.(artifactId, selectedResearchProject);
      }
    } catch (err) {
      console.error('Failed to start research', err);
    } finally {
      setIsSubmittingResearch(false);
    }
  };

  // Submit Plan
  const handleSubmitPlan = async () => {
    const g = planGoal.trim();
    if (!g || isSubmittingPlan) return;
    setIsSubmittingPlan(true);
    try {
      if (onStartPlan) {
        await onStartPlan(g, selectedPlanProject);
      } else {
        const title = `Plan: ${g.length > 60 ? `${g.slice(0, 60)}...` : g}`;
        const draft = await createPage(selectedPlanProject, title);
        await saveArtifact(draft.id, `# ${title}\n\n## Goal\n${g}\n`, title, selectedPlanProject);
        onOpenArtifact?.(draft.id, selectedPlanProject);
      }
    } catch (err) {
      console.error('Failed to start plan', err);
    } finally {
      setIsSubmittingPlan(false);
    }
  };

  // Run Tracker Now
  const handleRunTracker = async (topic: ScheduledTopicView) => {
    try {
      setRunningTopicId(topic.id);
      const artifactId = await triggerScheduledTopicRun(topic.id);
      await loadData();
      onWorkspaceMutated?.();
      if (artifactId && onOpenArtifact) {
        onOpenArtifact(artifactId, topic.project_id);
      }
    } catch (err) {
      console.error('Failed to trigger tracker run', err);
    } finally {
      setRunningTopicId(null);
    }
  };

  // Toggle Tracker Active
  const handleToggleTracker = async (topic: ScheduledTopicView) => {
    try {
      await toggleScheduledTopicActive(topic.id, !topic.is_active);
      await loadData();
      onWorkspaceMutated?.();
    } catch (err) {
      console.error('Failed to toggle tracker active', err);
    }
  };

  // Delete Tracker
  const handleDeleteTracker = async (topicId: string) => {
    try {
      await deleteScheduledTopic(topicId);
      await loadData();
      onWorkspaceMutated?.();
    } catch (err) {
      console.error('Failed to delete tracker', err);
    }
  };

  // Create New Tracker
  const handleSaveNewTracker = async (e: React.FormEvent) => {
    e.preventDefault();
    const t = newTrackerTitle.trim();
    const p = newTrackerPrompt.trim();
    if (!t || !p || isSavingTracker) return;

    try {
      setIsSavingTracker(true);
      await createScheduledTopic({
        title: t,
        prompt_query: p,
        topic_type: newTrackerType,
        project_id: newTrackerProject || null,
        cadence: newTrackerCadence,
        schedule_time: newTrackerCadence !== 'manual' ? newTrackerTime : null,
        schedule_weekday: newTrackerCadence === 'weekly' ? newTrackerWeekday : null,
      });

      setNewTrackerTitle('');
      setNewTrackerPrompt('');
      setIsNewTrackerOpen(false);
      await loadData();
      onWorkspaceMutated?.();
    } catch (err) {
      console.error('Failed to create tracker', err);
    } finally {
      setIsSavingTracker(false);
    }
  };

  const TABS = [
    { id: 'research' as ExplorePlanTab, label: 'Research', icon: '🔍' },
    { id: 'plan' as ExplorePlanTab, label: 'Plan', icon: '📋' },
    {
      id: 'trackers' as ExplorePlanTab,
      label: 'Trackers',
      icon: '⏱️',
      count: activeTrackersCount,
      countTestId: 'badge-active-trackers',
    },
    {
      id: 'digests' as ExplorePlanTab,
      label: 'All Digests',
      icon: '📚',
      count: allDigests.length,
      countTestId: 'badge-all-digests',
    },
  ];

  return (
    <div
      data-testid="explore-plan-view"
      className="flex-1 min-h-screen bg-[#FAF8F3] dark:bg-[#121310] text-[#1E201C] dark:text-[#EDEAE2] flex flex-col transition-colors overflow-y-auto"
    >
      <div className="max-w-6xl w-full mx-auto px-6 py-8 flex flex-col space-y-6">
        {/* Header & Breadcrumbs */}
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 border-b border-[#E8E5DC] dark:border-[#2C2E27] pb-6">
          <div className="space-y-1">
            <div className="flex items-center gap-2 text-xs font-semibold text-[#8C8A82] dark:text-[#A8A59C] uppercase tracking-wider">
              {onBack ? (
                <button
                  type="button"
                  data-testid="explore-plan-back"
                  onClick={onBack}
                  className="hover:text-[#1E201C] dark:hover:text-white transition-colors cursor-pointer"
                >
                  Workspace
                </button>
              ) : (
                <span>Workspace</span>
              )}
              <span>/</span>
              <span className="text-[#1E201C] dark:text-[#EDEAE2]">Explore &amp; Plan</span>
            </div>
            <h1 className="text-2xl font-bold tracking-tight text-[#1E201C] dark:text-[#EDEAE2] flex items-center gap-2.5">
              <span>🧭 Explore &amp; Plan</span>
            </h1>
            <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93]">
              Unified workspace for deep research, milestone planning, continuous background
              trackers, and generated digests.
            </p>
          </div>
        </div>

        {/* Top Segmented Tabs (matching All Files design) */}
        <div className="flex items-center gap-1.5 overflow-x-auto pb-1 select-none border-b border-[#E8E5DC]/60 dark:border-[#2C2E27]/60">
          {TABS.map((tab) => {
            const isActive = activeTab === tab.id;
            return (
              <button
                key={tab.id}
                type="button"
                data-testid={`tab-${tab.id}`}
                onClick={() => setActiveTab(tab.id)}
                className={`flex items-center gap-2 px-3.5 py-2 rounded-xl text-xs font-semibold transition-all cursor-pointer whitespace-nowrap ${
                  isActive
                    ? 'bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] shadow-2xs'
                    : 'bg-white/70 dark:bg-[#1C1D19]/70 text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white hover:bg-white dark:hover:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27]'
                }`}
              >
                <span>
                  {tab.icon} {tab.label}
                </span>
                {tab.count !== undefined && (
                  <span
                    data-testid={tab.countTestId}
                    className={`text-[10px] px-1.5 py-0.2 rounded-full font-mono ${
                      isActive
                        ? 'bg-white/20 dark:bg-black/10 text-white dark:text-[#121310]'
                        : 'bg-[#EFECE4] dark:bg-[#252820] text-[#7A7870] dark:text-[#B5B2A8]'
                    }`}
                  >
                    {tab.count}
                  </span>
                )}
              </button>
            );
          })}
        </div>

        {/* Tab 1: RESEARCH */}
        {activeTab === 'research' && (
          <div data-testid="tab-content-research" className="space-y-8 animate-fadeIn">
            {/* Input Card */}
            <div
              data-testid="research-input-card"
              className="rounded-2xl bg-white dark:bg-[#181915] border border-[#E8E5DC] dark:border-[#262822] p-6 shadow-2xs space-y-4"
            >
              <div className="space-y-1">
                <label
                  htmlFor="research-query-textarea"
                  className="text-xs font-bold uppercase tracking-wider text-[#7A7870] dark:text-[#9E9C93]"
                >
                  Autonomous Research Inquiry
                </label>
                <textarea
                  id="research-query-textarea"
                  data-testid="input-research-query"
                  rows={3}
                  value={researchQuery}
                  onChange={(e) => setResearchQuery(e.target.value)}
                  onKeyDown={(e) => {
                    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
                      e.preventDefault();
                      handleSubmitResearch();
                    }
                  }}
                  placeholder="What would you like to research?"
                  className="w-full bg-[#FAF8F3]/50 dark:bg-[#121310]/50 border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl p-3.5 text-sm text-[#1E201C] dark:text-[#EDEAE2] placeholder:text-[#8C8A82] dark:placeholder:text-[#6E6D66] outline-none focus:border-[#282C25] dark:focus:border-[#9E9C93] transition-colors resize-none"
                />
              </div>

              {/* Controls Row */}
              <div className="flex flex-wrap items-center justify-between gap-3 pt-1">
                <div className="flex flex-wrap items-center gap-2.5">
                  {/* Web Search Mode Toggle */}
                  <div
                    data-testid="toggle-web-search"
                    className="flex items-center gap-1 bg-[#FAF8F3] dark:bg-[#121310] p-1 rounded-xl border border-[#E8E5DC] dark:border-[#2C2E27]"
                  >
                    <span className="text-[11px] font-semibold text-[#8C8A82] px-1.5 flex items-center gap-1">
                      <Globe className="w-3 h-3" />
                      <span>Web Search:</span>
                    </span>
                    <button
                      type="button"
                      data-testid="search-mode-auto"
                      onClick={() => setWebSearchMode('auto')}
                      className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
                        webSearchMode === 'auto'
                          ? 'bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] font-semibold shadow-2xs'
                          : 'text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white'
                      }`}
                    >
                      Auto
                    </button>
                    <button
                      type="button"
                      data-testid="search-mode-always"
                      onClick={() => setWebSearchMode('always_search')}
                      className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
                        webSearchMode === 'always_search'
                          ? 'bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] font-semibold shadow-2xs'
                          : 'text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white'
                      }`}
                    >
                      Enabled
                    </button>
                    <button
                      type="button"
                      data-testid="search-mode-vault"
                      onClick={() => setWebSearchMode('local_vault_only')}
                      className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-colors cursor-pointer ${
                        webSearchMode === 'local_vault_only'
                          ? 'bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] font-semibold shadow-2xs'
                          : 'text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#1E201C] dark:hover:text-white'
                      }`}
                    >
                      Local Vault Only
                    </button>
                  </div>

                  {/* Model Selector Pill */}
                  <div className="flex items-center gap-1.5">
                    <Sparkles className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#7A786F]" />
                    <select
                      data-testid="select-research-model"
                      value={selectedResearchModel}
                      onChange={(e) => setSelectedResearchModel(e.target.value)}
                      className="px-2.5 py-1.5 bg-[#FAF8F3] dark:bg-[#121310] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none cursor-pointer"
                    >
                      {models.length > 0 ? (
                        models.map((m) => (
                          <option key={m.id} value={m.model_identifier}>
                            {m.display_name || m.model_identifier}
                          </option>
                        ))
                      ) : (
                        <option value="default">Default Model</option>
                      )}
                    </select>
                  </div>

                  {/* Project Selector */}
                  <select
                    data-testid="select-research-project"
                    value={selectedResearchProject || ''}
                    onChange={(e) => setSelectedResearchProject(e.target.value || null)}
                    className="px-2.5 py-1.5 bg-[#FAF8F3] dark:bg-[#121310] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none cursor-pointer"
                  >
                    <option value="">No Project (Standalone)</option>
                    {projects.map((p) => (
                      <option key={p.id} value={p.id}>
                        📁 {p.name}
                      </option>
                    ))}
                  </select>
                </div>

                {/* Submit Action */}
                <button
                  type="button"
                  data-testid="btn-start-research"
                  disabled={!researchQuery.trim() || isSubmittingResearch}
                  onClick={handleSubmitResearch}
                  className="px-5 py-2 bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] rounded-xl text-xs font-semibold hover:opacity-90 disabled:opacity-40 transition-opacity cursor-pointer flex items-center gap-2 shadow-2xs"
                >
                  {isSubmittingResearch ? (
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  ) : (
                    <Search className="w-3.5 h-3.5" />
                  )}
                  <span>Start Research</span>
                  <span className="text-[10px] opacity-60 font-mono">⌘↵</span>
                </button>
              </div>
            </div>

            {/* Recent Research Section */}
            <div data-testid="recent-research-section" className="space-y-4">
              <div className="flex items-center justify-between border-b border-[#E8E5DC]/60 dark:border-[#2C2E27]/60 pb-2">
                <h2 className="text-sm font-bold text-[#1E201C] dark:text-[#EDEAE2] flex items-center gap-2">
                  <span>Recent Research</span>
                  <span className="text-xs text-[#8C8A81] font-normal">
                    (last {recentResearch.length})
                  </span>
                </h2>
                <button
                  type="button"
                  data-testid="btn-view-all-research"
                  onClick={() => {
                    setActiveTab('digests');
                    setDigestTypeFilter('research');
                  }}
                  className="text-xs font-semibold text-[#282C25] dark:text-[#EDEAE2] hover:underline flex items-center gap-1 cursor-pointer"
                >
                  <span>View all research notes →</span>
                </button>
              </div>

              {recentResearch.length > 0 ? (
                <div
                  data-testid="recent-research-list"
                  className="grid grid-cols-1 md:grid-cols-3 gap-3.5"
                >
                  {recentResearch.map((item) => (
                    <div
                      key={item.id}
                      data-testid={`recent-research-item-${item.id}`}
                      onClick={() => onOpenArtifact?.(item.id, item.project_id)}
                      className="group p-4 bg-white dark:bg-[#181915] border border-[#E8E5DC] dark:border-[#262822] rounded-xl hover:border-[#282C25] dark:hover:border-[#9E9C93] transition-all cursor-pointer shadow-2xs flex flex-col justify-between space-y-3"
                    >
                      <div className="space-y-1">
                        <div className="flex items-center justify-between gap-2">
                          <span className="text-[10px] font-semibold px-2 py-0.5 rounded-md bg-[#E8EEF5] dark:bg-[#1F2633] text-[#2B4C6F] dark:text-[#A2C2E8]">
                            🔍 Research
                          </span>
                          <span className="text-[10px] text-[#8C8A82]">
                            {formatRelativeTime(item.updated_at || item.created_at)}
                          </span>
                        </div>
                        <h3 className="text-xs font-bold text-[#1E201C] dark:text-[#EDEAE2] line-clamp-2 group-hover:underline">
                          {item.title}
                        </h3>
                        {item.snippet && (
                          <p className="text-[11px] text-[#6E6D66] dark:text-[#9E9C93] line-clamp-2">
                            {item.snippet}
                          </p>
                        )}
                      </div>
                      <div className="flex items-center justify-between text-[11px] text-[#7A7870] pt-1 border-t border-[#E8E5DC]/50 dark:border-[#262822]/50">
                        <span className="flex items-center gap-1 truncate max-w-[140px]">
                          <Folder className="w-3 h-3 flex-shrink-0" />
                          <span>{item.project_name || 'Standalone'}</span>
                        </span>
                        <ChevronRight className="w-3.5 h-3.5 group-hover:translate-x-0.5 transition-transform" />
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="p-8 text-center rounded-xl bg-white/40 dark:bg-[#181915]/40 border border-dashed border-[#E8E5DC] dark:border-[#2C2E27] text-xs text-[#8C8A82]">
                  No research notes recorded yet. Enter a topic above to conduct your first
                  investigation.
                </div>
              )}
            </div>
          </div>
        )}

        {/* Tab 2: PLAN */}
        {activeTab === 'plan' && (
          <div data-testid="tab-content-plan" className="space-y-8 animate-fadeIn">
            {/* Input Card */}
            <div
              data-testid="plan-input-card"
              className="rounded-2xl bg-white dark:bg-[#181915] border border-[#E8E5DC] dark:border-[#262822] p-6 shadow-2xs space-y-4"
            >
              <div className="space-y-1">
                <label
                  htmlFor="plan-goal-textarea"
                  className="text-xs font-bold uppercase tracking-wider text-[#7A7870] dark:text-[#9E9C93]"
                >
                  Milestone &amp; Architecture Planning
                </label>
                <textarea
                  id="plan-goal-textarea"
                  data-testid="input-plan-goal"
                  rows={3}
                  value={planGoal}
                  onChange={(e) => setPlanGoal(e.target.value)}
                  onKeyDown={(e) => {
                    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
                      e.preventDefault();
                      handleSubmitPlan();
                    }
                  }}
                  placeholder="What project, feature, or milestone set are we planning?"
                  className="w-full bg-[#FAF8F3]/50 dark:bg-[#121310]/50 border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl p-3.5 text-sm text-[#1E201C] dark:text-[#EDEAE2] placeholder:text-[#8C8A82] dark:placeholder:text-[#6E6D66] outline-none focus:border-[#282C25] dark:focus:border-[#9E9C93] transition-colors resize-none"
                />
              </div>

              {/* Controls Row */}
              <div className="flex flex-wrap items-center justify-between gap-3 pt-1">
                <div className="flex flex-wrap items-center gap-2.5">
                  {/* Model Selector Pill */}
                  <div className="flex items-center gap-1.5">
                    <Sparkles className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#7A786F]" />
                    <select
                      data-testid="select-plan-model"
                      value={selectedPlanModel}
                      onChange={(e) => setSelectedPlanModel(e.target.value)}
                      className="px-2.5 py-1.5 bg-[#FAF8F3] dark:bg-[#121310] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none cursor-pointer"
                    >
                      {models.length > 0 ? (
                        models.map((m) => (
                          <option key={m.id} value={m.model_identifier}>
                            {m.display_name || m.model_identifier}
                          </option>
                        ))
                      ) : (
                        <option value="default">Default Model</option>
                      )}
                    </select>
                  </div>

                  {/* Project Selector */}
                  <select
                    data-testid="select-plan-project"
                    value={selectedPlanProject || ''}
                    onChange={(e) => setSelectedPlanProject(e.target.value || null)}
                    className="px-2.5 py-1.5 bg-[#FAF8F3] dark:bg-[#121310] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none cursor-pointer"
                  >
                    <option value="">No Project (Standalone)</option>
                    {projects.map((p) => (
                      <option key={p.id} value={p.id}>
                        📁 {p.name}
                      </option>
                    ))}
                  </select>
                </div>

                {/* Submit Action */}
                <button
                  type="button"
                  data-testid="btn-start-plan"
                  disabled={!planGoal.trim() || isSubmittingPlan}
                  onClick={handleSubmitPlan}
                  className="px-5 py-2 bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] rounded-xl text-xs font-semibold hover:opacity-90 disabled:opacity-40 transition-opacity cursor-pointer flex items-center gap-2 shadow-2xs"
                >
                  {isSubmittingPlan ? (
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  ) : (
                    <ClipboardList className="w-3.5 h-3.5" />
                  )}
                  <span>Create Plan</span>
                  <span className="text-[10px] opacity-60 font-mono">⌘↵</span>
                </button>
              </div>
            </div>

            {/* Recent Plans Section */}
            <div data-testid="recent-plans-section" className="space-y-4">
              <div className="flex items-center justify-between border-b border-[#E8E5DC]/60 dark:border-[#2C2E27]/60 pb-2">
                <h2 className="text-sm font-bold text-[#1E201C] dark:text-[#EDEAE2] flex items-center gap-2">
                  <span>Recent Plans</span>
                  <span className="text-xs text-[#8C8A81] font-normal">
                    (last {recentPlans.length})
                  </span>
                </h2>
                <button
                  type="button"
                  data-testid="btn-view-all-plans"
                  onClick={() => {
                    setActiveTab('digests');
                    setDigestTypeFilter('plan');
                  }}
                  className="text-xs font-semibold text-[#282C25] dark:text-[#EDEAE2] hover:underline flex items-center gap-1 cursor-pointer"
                >
                  <span>View all plans →</span>
                </button>
              </div>

              {recentPlans.length > 0 ? (
                <div
                  data-testid="recent-plans-list"
                  className="grid grid-cols-1 md:grid-cols-3 gap-3.5"
                >
                  {recentPlans.map((item) => (
                    <div
                      key={item.id}
                      data-testid={`recent-plan-item-${item.id}`}
                      onClick={() => onOpenArtifact?.(item.id, item.project_id)}
                      className="group p-4 bg-white dark:bg-[#181915] border border-[#E8E5DC] dark:border-[#262822] rounded-xl hover:border-[#282C25] dark:hover:border-[#9E9C93] transition-all cursor-pointer shadow-2xs flex flex-col justify-between space-y-3"
                    >
                      <div className="space-y-1">
                        <div className="flex items-center justify-between gap-2">
                          <span className="text-[10px] font-semibold px-2 py-0.5 rounded-md bg-[#F2ECE1] dark:bg-[#2A261D] text-[#875F1A] dark:text-[#E8C582]">
                            📋 Plan
                          </span>
                          <span className="text-[10px] text-[#8C8A82]">
                            {formatRelativeTime(item.updated_at || item.created_at)}
                          </span>
                        </div>
                        <h3 className="text-xs font-bold text-[#1E201C] dark:text-[#EDEAE2] line-clamp-2 group-hover:underline">
                          {item.title}
                        </h3>
                        {item.snippet && (
                          <p className="text-[11px] text-[#6E6D66] dark:text-[#9E9C93] line-clamp-2">
                            {item.snippet}
                          </p>
                        )}
                      </div>
                      <div className="flex items-center justify-between text-[11px] text-[#7A7870] pt-1 border-t border-[#E8E5DC]/50 dark:border-[#262822]/50">
                        <span className="flex items-center gap-1 truncate max-w-[140px]">
                          <Folder className="w-3 h-3 flex-shrink-0" />
                          <span>{item.project_name || 'Standalone'}</span>
                        </span>
                        <ChevronRight className="w-3.5 h-3.5 group-hover:translate-x-0.5 transition-transform" />
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="p-8 text-center rounded-xl bg-white/40 dark:bg-[#181915]/40 border border-dashed border-[#E8E5DC] dark:border-[#2C2E27] text-xs text-[#8C8A82]">
                  No project plans synthesized yet. Enter a goal above to draft milestones.
                </div>
              )}
            </div>
          </div>
        )}

        {/* Tab 3: TRACKERS (Continuous / Scheduled Topics) */}
        {activeTab === 'trackers' && (
          <div data-testid="tab-content-trackers" className="space-y-6 animate-fadeIn">
            {/* Action Bar */}
            <div className="flex items-center justify-between gap-4">
              <div>
                <h2 className="text-sm font-bold text-[#1E201C] dark:text-[#EDEAE2] flex items-center gap-2">
                  <span>Active Background Trackers</span>
                  <span className="text-xs text-[#8C8A81] font-normal">({topics.length} total)</span>
                </h2>
                <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93]">
                  Recurring investigations that autonomously gather new developments and generate
                  digests.
                </p>
              </div>
              <button
                type="button"
                data-testid="btn-new-tracker"
                onClick={() => setIsNewTrackerOpen(true)}
                className="px-3.5 py-2 bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] rounded-xl text-xs font-semibold hover:opacity-90 transition-opacity cursor-pointer flex items-center gap-1.5 shadow-2xs"
              >
                <Plus className="w-3.5 h-3.5" />
                <span>+ New Tracker</span>
              </button>
            </div>

            {/* New Tracker Modal / Inline Drawer */}
            {isNewTrackerOpen && (
              <div
                data-testid="modal-new-tracker"
                className="rounded-2xl bg-white dark:bg-[#181915] border-2 border-[#282C25] dark:border-[#EDEAE2] p-6 shadow-md space-y-4 animate-fadeIn"
              >
                <div className="flex items-center justify-between border-b border-[#E8E5DC] dark:border-[#2C2E27] pb-3">
                  <h3 className="text-sm font-bold text-[#1E201C] dark:text-[#EDEAE2] flex items-center gap-2">
                    <Clock className="w-4 h-4" />
                    <span>Configure Recurring Tracker</span>
                  </h3>
                  <button
                    type="button"
                    data-testid="btn-cancel-scheduled-topic"
                    onClick={() => setIsNewTrackerOpen(false)}
                    className="p-1 rounded-lg hover:bg-[#EFECE4] dark:hover:bg-[#2C2E27] text-[#8C8A82] cursor-pointer"
                  >
                    <X className="w-4 h-4" />
                  </button>
                </div>

                <form onSubmit={handleSaveNewTracker} className="space-y-4">
                  {/* Topic Type Selector */}
                  <div className="space-y-1">
                    <label className="text-[11px] font-bold text-[#7A7870] uppercase">
                      Tracker Type
                    </label>
                    <div className="flex items-center gap-2">
                      <button
                        type="button"
                        data-testid="select-topic-type-research"
                        onClick={() => setNewTrackerType('research')}
                        className={`px-3 py-1.5 rounded-lg text-xs font-semibold cursor-pointer ${
                          newTrackerType === 'research'
                            ? 'bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310]'
                            : 'bg-[#FAF8F3] dark:bg-[#121310] border border-[#E8E5DC] dark:border-[#2C2E27] text-[#6E6D66]'
                        }`}
                      >
                        🔍 Research Investigation
                      </button>
                      <button
                        type="button"
                        data-testid="select-topic-type-plan"
                        onClick={() => setNewTrackerType('plan')}
                        className={`px-3 py-1.5 rounded-lg text-xs font-semibold cursor-pointer ${
                          newTrackerType === 'plan'
                            ? 'bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310]'
                            : 'bg-[#FAF8F3] dark:bg-[#121310] border border-[#E8E5DC] dark:border-[#2C2E27] text-[#6E6D66]'
                        }`}
                      >
                        📋 Plan Review
                      </button>
                    </div>
                  </div>

                  {/* Title */}
                  <div className="space-y-1">
                    <label className="text-[11px] font-bold text-[#7A7870] uppercase">
                      Tracker Title
                    </label>
                    <input
                      type="text"
                      data-testid="input-topic-title"
                      value={newTrackerTitle}
                      onChange={(e) => setNewTrackerTitle(e.target.value)}
                      placeholder="e.g. SSD NVMe Streaming Benchmarks"
                      className="w-full px-3 py-2 bg-[#FAF8F3] dark:bg-[#121310] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none"
                    />
                  </div>

                  {/* Prompt / Objective */}
                  <div className="space-y-1">
                    <label className="text-[11px] font-bold text-[#7A7870] uppercase">
                      Inquiry / Objective Prompt
                    </label>
                    <textarea
                      rows={2}
                      data-testid="input-topic-objective"
                      value={newTrackerPrompt}
                      onChange={(e) => setNewTrackerPrompt(e.target.value)}
                      placeholder="Inquiry query executed during each automated run..."
                      className="w-full px-3 py-2 bg-[#FAF8F3] dark:bg-[#121310] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none resize-none"
                    />
                  </div>

                  {/* Cadence & Frequency */}
                  <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                    <div className="space-y-1">
                      <label className="text-[11px] font-bold text-[#7A7870] uppercase">
                        Cadence
                      </label>
                      <select
                        data-testid="select-topic-cadence"
                        value={newTrackerCadence}
                        onChange={(e) =>
                          setNewTrackerCadence(e.target.value as ScheduledTopicCadence)
                        }
                        className="w-full px-2.5 py-1.5 bg-[#FAF8F3] dark:bg-[#121310] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none cursor-pointer"
                      >
                        <option value="daily">Daily (Autonomous)</option>
                        <option value="weekly">Weekly</option>
                        <option value="manual">Manual Only</option>
                      </select>
                    </div>

                    {newTrackerCadence !== 'manual' && (
                      <div className="space-y-1">
                        <label className="text-[11px] font-bold text-[#7A7870] uppercase">
                          Run at (UTC)
                        </label>
                        <input
                          type="time"
                          data-testid="input-topic-time"
                          value={newTrackerTime}
                          onChange={(e) => setNewTrackerTime(e.target.value)}
                          className="w-full px-2.5 py-1.5 bg-[#FAF8F3] dark:bg-[#121310] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none cursor-pointer"
                        />
                      </div>
                    )}

                    {newTrackerCadence === 'weekly' && (
                      <div className="space-y-1">
                        <label className="text-[11px] font-bold text-[#7A7870] uppercase">
                          Weekday
                        </label>
                        <select
                          data-testid="select-topic-weekday"
                          value={newTrackerWeekday}
                          onChange={(e) => setNewTrackerWeekday(parseInt(e.target.value, 10))}
                          className="w-full px-2.5 py-1.5 bg-[#FAF8F3] dark:bg-[#121310] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none cursor-pointer"
                        >
                          {WEEKDAY_NAMES.map((name, idx) => (
                            <option key={name} value={idx}>
                              {name}
                            </option>
                          ))}
                        </select>
                      </div>
                    )}

                    <div className="space-y-1">
                      <label className="text-[11px] font-bold text-[#7A7870] uppercase">
                        Attach to Project
                      </label>
                      <select
                        data-testid="select-topic-project"
                        value={newTrackerProject}
                        onChange={(e) => setNewTrackerProject(e.target.value)}
                        className="w-full px-2.5 py-1.5 bg-[#FAF8F3] dark:bg-[#121310] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none cursor-pointer"
                      >
                        <option value="">Standalone (Workspace)</option>
                        {projects.map((p) => (
                          <option key={p.id} value={p.id}>
                            📁 {p.name}
                          </option>
                        ))}
                      </select>
                    </div>
                  </div>

                  {/* Actions */}
                  <div className="flex items-center justify-end gap-2 pt-2">
                    <button
                      type="button"
                      onClick={() => setIsNewTrackerOpen(false)}
                      className="px-4 py-1.5 rounded-xl text-xs text-[#6E6D66] hover:bg-[#EFECE4] dark:hover:bg-[#2C2E27] cursor-pointer"
                    >
                      Cancel
                    </button>
                    <button
                      type="submit"
                      data-testid="btn-save-scheduled-topic"
                      disabled={!newTrackerTitle.trim() || !newTrackerPrompt.trim() || isSavingTracker}
                      className="px-4 py-1.5 bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] rounded-xl text-xs font-semibold hover:opacity-90 disabled:opacity-40 cursor-pointer"
                    >
                      {isSavingTracker ? 'Saving...' : 'Save Tracker'}
                    </button>
                  </div>
                </form>
              </div>
            )}

            {/* Trackers Sections (with backward-compatibility testids) */}
            <div data-testid="scheduled-section-research" className="space-y-3">
              <div className="flex items-center gap-2">
                <Search className="w-4 h-4 text-[#7A7870]" />
                <h3 className="text-xs font-bold uppercase tracking-wider text-[#7A7870] dark:text-[#9E9C93]">
                  Research Trackers
                </h3>
              </div>
              <div
                data-testid="trackers-grid"
                className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4"
              >
                {topics
                  .filter((t) => t.topic_type === 'research')
                  .map((topic) => (
                    <TrackerCard
                      key={topic.id}
                      topic={topic}
                      isRunning={runningTopicId === topic.id}
                      onRun={() => handleRunTracker(topic)}
                      onToggle={() => handleToggleTracker(topic)}
                      onDelete={() => handleDeleteTracker(topic.id)}
                      onOpenArtifact={onOpenArtifact}
                    />
                  ))}
                {topics.filter((t) => t.topic_type === 'research').length === 0 && (
                  <div className="col-span-full p-6 text-center rounded-xl bg-white/40 dark:bg-[#181915]/40 border border-dashed border-[#E8E5DC] dark:border-[#2C2E27] text-xs text-[#8C8A82]">
                    No research trackers configured. Click &ldquo;+ New Tracker&rdquo; to set up an
                    autonomous query.
                  </div>
                )}
              </div>
            </div>

            <div data-testid="scheduled-section-plan" className="space-y-3 pt-4">
              <div className="flex items-center gap-2">
                <ClipboardList className="w-4 h-4 text-[#7A7870]" />
                <h3 className="text-xs font-bold uppercase tracking-wider text-[#7A7870] dark:text-[#9E9C93]">
                  Plan Review Trackers
                </h3>
              </div>
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {topics
                  .filter((t) => t.topic_type === 'plan')
                  .map((topic) => (
                    <TrackerCard
                      key={topic.id}
                      topic={topic}
                      isRunning={runningTopicId === topic.id}
                      onRun={() => handleRunTracker(topic)}
                      onToggle={() => handleToggleTracker(topic)}
                      onDelete={() => handleDeleteTracker(topic.id)}
                      onOpenArtifact={onOpenArtifact}
                    />
                  ))}
                {topics.filter((t) => t.topic_type === 'plan').length === 0 && (
                  <div className="col-span-full p-6 text-center rounded-xl bg-white/40 dark:bg-[#181915]/40 border border-dashed border-[#E8E5DC] dark:border-[#2C2E27] text-xs text-[#8C8A82]">
                    No plan review trackers configured.
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {/* Tab 4: ALL DIGESTS (The "View All" List) */}
        {activeTab === 'digests' && (
          <div data-testid="tab-content-digests" className="space-y-6 animate-fadeIn">
            {/* Filter Bar */}
            <div className="flex flex-col md:flex-row md:items-center justify-between gap-3">
              {/* Search filter */}
              <div className="relative flex-1 max-w-md">
                <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-[#8C8A82]" />
                <input
                  type="text"
                  data-testid="input-search-digests"
                  value={digestSearch}
                  onChange={(e) => setDigestSearch(e.target.value)}
                  placeholder="Filter past research & plan notes..."
                  className="w-full pl-9 pr-8 py-2 bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl text-xs text-[#1E201C] dark:text-[#EDEAE2] placeholder:text-[#8C8A82] outline-none focus:border-[#282C25] dark:focus:border-[#9E9C93] transition-colors shadow-2xs"
                />
                {digestSearch && (
                  <button
                    type="button"
                    onClick={() => setDigestSearch('')}
                    className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[10px] text-[#8C8A82] hover:text-[#1E201C] dark:hover:text-white"
                  >
                    Clear
                  </button>
                )}
              </div>

              {/* Type Filter Pills */}
              <div className="flex items-center gap-1.5 overflow-x-auto select-none">
                <span className="text-[11px] font-semibold text-[#8C8A82] mr-1 flex items-center gap-1">
                  <Filter className="w-3 h-3" />
                  <span>Type:</span>
                </span>
                <button
                  type="button"
                  data-testid="filter-type-all"
                  onClick={() => setDigestTypeFilter('all')}
                  className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors cursor-pointer ${
                    digestTypeFilter === 'all'
                      ? 'bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] shadow-2xs'
                      : 'bg-white/70 dark:bg-[#1C1D19]/70 text-[#6E6D66] border border-[#E8E5DC] dark:border-[#2C2E27]'
                  }`}
                >
                  All ({allDigests.length})
                </button>
                <button
                  type="button"
                  data-testid="filter-type-research"
                  onClick={() => setDigestTypeFilter('research')}
                  className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors cursor-pointer ${
                    digestTypeFilter === 'research'
                      ? 'bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] shadow-2xs'
                      : 'bg-white/70 dark:bg-[#1C1D19]/70 text-[#6E6D66] border border-[#E8E5DC] dark:border-[#2C2E27]'
                  }`}
                >
                  🔍 Research ({allDigests.filter((d) => d.artifact_type === 'research').length})
                </button>
                <button
                  type="button"
                  data-testid="filter-type-plan"
                  onClick={() => setDigestTypeFilter('plan')}
                  className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors cursor-pointer ${
                    digestTypeFilter === 'plan'
                      ? 'bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] shadow-2xs'
                      : 'bg-white/70 dark:bg-[#1C1D19]/70 text-[#6E6D66] border border-[#E8E5DC] dark:border-[#2C2E27]'
                  }`}
                >
                  📋 Plan ({allDigests.filter((d) => d.artifact_type === 'plan').length})
                </button>
              </div>
            </div>

            {/* Filterable Table */}
            <div
              data-testid="digests-table"
              className="rounded-2xl bg-white dark:bg-[#181915] border border-[#E8E5DC] dark:border-[#262822] shadow-2xs overflow-hidden"
            >
              <table className="w-full text-left border-collapse text-xs">
                <thead>
                  <tr className="border-b border-[#E8E5DC] dark:border-[#262822] bg-[#FAF8F3]/60 dark:bg-[#141511]/60 text-[11px] font-bold text-[#7A7870] dark:text-[#9E9C93] uppercase">
                    <th className="py-3 px-4">Title</th>
                    <th className="py-3 px-3 w-28">Type</th>
                    <th className="py-3 px-3 w-36">Project</th>
                    <th className="py-3 px-3 w-32">Date Generated</th>
                    <th className="py-3 px-3 w-12 text-center">•••</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-[#E8E5DC]/60 dark:divide-[#262822]/60">
                  {filteredDigests.map((item) => (
                    <tr
                      key={item.id}
                      data-testid={`digest-row-${item.id}`}
                      onClick={() => onOpenArtifact?.(item.id, item.project_id)}
                      className="group hover:bg-[#FAF8F3] dark:hover:bg-[#1F201B] transition-colors cursor-pointer"
                    >
                      {/* Title */}
                      <td className="py-3.5 px-4">
                        <div className="font-semibold text-[#1E201C] dark:text-[#EDEAE2] group-hover:underline flex items-center gap-2">
                          <span>{item.title}</span>
                        </div>
                        {item.snippet && (
                          <div className="text-[11px] text-[#7A7870] dark:text-[#8C8A81] line-clamp-1 mt-0.5">
                            {item.snippet}
                          </div>
                        )}
                      </td>

                      {/* Type Badge */}
                      <td className="py-3.5 px-3">
                        {item.artifact_type === 'research' ? (
                          <span
                            data-testid={`badge-type-${item.id}`}
                            className="inline-flex items-center gap-1 text-[10px] font-semibold px-2 py-0.5 rounded-md bg-[#E8EEF5] dark:bg-[#1F2633] text-[#2B4C6F] dark:text-[#A2C2E8]"
                          >
                            🔍 Research
                          </span>
                        ) : (
                          <span
                            data-testid={`badge-type-${item.id}`}
                            className="inline-flex items-center gap-1 text-[10px] font-semibold px-2 py-0.5 rounded-md bg-[#F2ECE1] dark:bg-[#2A261D] text-[#875F1A] dark:text-[#E8C582]"
                          >
                            📋 Plan
                          </span>
                        )}
                      </td>

                      {/* Project Chip */}
                      <td className="py-3.5 px-3 text-[#7A7870] dark:text-[#9E9C93]">
                        {item.project_name ? (
                          <span className="inline-flex items-center gap-1 text-[11px] px-2 py-0.5 rounded-md bg-[#EFECE4] dark:bg-[#252820] text-[#1E201C] dark:text-[#EDEAE2] truncate max-w-[130px]">
                            <Folder className="w-3 h-3 flex-shrink-0" />
                            <span className="truncate">{item.project_name}</span>
                          </span>
                        ) : (
                          <span className="text-[11px] text-[#8C8A82]">Standalone</span>
                        )}
                      </td>

                      {/* Date Generated */}
                      <td className="py-3.5 px-3 text-[#7A7870] dark:text-[#8C8A81]">
                        {formatDate(item.created_at || item.updated_at)}
                      </td>

                      {/* Actions */}
                      <td
                        className="py-3.5 px-3 text-center"
                        onClick={(e) => e.stopPropagation()}
                      >
                        <PageActionMenu
                          artifactId={item.id}
                          artifactTitle={item.title}
                          projectId={item.project_id}
                          variant="row"
                          align="right"
                          triggerTestId={`btn-page-actions-${item.id}`}
                          onChanged={loadData}
                          onOpenArtifact={onOpenArtifact}
                        />
                      </td>
                    </tr>
                  ))}
                  {filteredDigests.length === 0 && (
                    <tr>
                      <td
                        colSpan={5}
                        className="py-12 text-center text-xs text-[#8C8A82] dark:text-[#6E6D66]"
                      >
                        No generated research or plan notes found matching your criteria.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

interface TrackerCardProps {
  topic: ScheduledTopicView;
  isRunning: boolean;
  onRun: () => void;
  onToggle: () => void;
  onDelete: () => void;
  onOpenArtifact?: (artifactId: string, projectId?: string | null) => void;
}

const TrackerCard: React.FC<TrackerCardProps> = ({
  topic,
  isRunning,
  onRun,
  onToggle,
  onDelete,
  onOpenArtifact,
}) => {
  return (
    <div
      data-testid={`topic-card-${topic.id}`}
      className="p-5 rounded-2xl bg-white dark:bg-[#181915] border border-[#E8E5DC] dark:border-[#262822] shadow-2xs hover:shadow-xs transition-shadow flex flex-col justify-between space-y-4"
    >
      <div className="space-y-2.5">
        {/* Top Badges */}
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-1.5 flex-wrap">
            <span
              data-testid={`cadence-badge-${topic.id}`}
              className="text-[10px] font-semibold px-2 py-0.5 rounded-md uppercase font-mono bg-[#EFECE4] dark:bg-[#252820] text-[#6E6D66] dark:text-[#A8A69E]"
            >
              {topic.cadence}
            </span>
            <span
              data-testid={`status-badge-${topic.id}`}
              className={`text-[10px] font-semibold px-2 py-0.5 rounded-md ${
                topic.is_active
                  ? 'bg-[#E3EBE3] dark:bg-[#1E2B1E] text-[#2D5A27] dark:text-[#7BC274]'
                  : 'bg-[#F2ECE1] dark:bg-[#28251E] text-[#8C7A58] dark:text-[#D1B886]'
              }`}
            >
              {topic.is_active ? 'Active' : 'Paused'}
            </span>
          </div>
          <span
            data-testid={`schedule-label-${topic.id}`}
            className="text-[10px] text-[#8C8A82] font-mono"
          >
            {topic.schedule_label || topic.cadence}
          </span>
        </div>

        {/* Title & Prompt */}
        <div>
          <h4 className="text-sm font-bold text-[#1E201C] dark:text-[#EDEAE2]">
            {topic.title}
          </h4>
          <p className="text-[11px] text-[#6E6D66] dark:text-[#9E9C93] line-clamp-2 mt-1">
            {topic.prompt_query}
          </p>
        </div>

        {/* Meta / Links */}
        <div className="space-y-1 text-[11px] text-[#7A7870] dark:text-[#8C8A81] pt-1 border-t border-[#E8E5DC]/50 dark:border-[#262822]/50">
          <div className="flex items-center justify-between">
            <span className="flex items-center gap-1">
              <Folder className="w-3 h-3 flex-shrink-0" />
              <span>{topic.project_name || 'Standalone'}</span>
            </span>
            <span data-testid={`next-run-${topic.id}`} className="text-[10px]">
              {topic.next_run_at ? `Next in ${formatUntil(topic.next_run_at)}` : ''}
            </span>
          </div>
          <div className="text-[10px] text-[#8C8A82]">
            Last run: {formatRelativeTime(topic.last_run_at)}
          </div>
        </div>

        {/* Previous Runs (if any) */}
        {topic.runs && topic.runs.length > 0 && (
          <div className="pt-2 border-t border-[#E8E5DC]/50 dark:border-[#262822]/50 space-y-1">
            <span className="text-[10px] font-bold text-[#8C8A82] uppercase">
              Latest Digest:
            </span>
            {topic.runs.slice(0, 1).map((run) => (
              <button
                key={run.id}
                type="button"
                data-testid={`digest-link-${run.artifact_id}`}
                onClick={() => onOpenArtifact?.(run.artifact_id, topic.project_id)}
                className="w-full text-left text-[11px] font-medium text-[#282C25] dark:text-[#EDEAE2] hover:underline flex items-center justify-between gap-1 p-1.5 rounded-lg bg-[#FAF8F3] dark:bg-[#121310] border border-[#E8E5DC] dark:border-[#262822] cursor-pointer"
              >
                <span className="truncate">{run.artifact_title}</span>
                <ChevronRight className="w-3 h-3 flex-shrink-0" />
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Action Buttons Row */}
      <div className="flex items-center justify-between gap-2 pt-2 border-t border-[#E8E5DC] dark:border-[#262822]">
        <button
          type="button"
          data-testid={`btn-run-now-${topic.id}`}
          disabled={isRunning}
          onClick={onRun}
          className="px-2.5 py-1.5 rounded-xl bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] text-[11px] font-semibold hover:opacity-90 disabled:opacity-50 transition-opacity cursor-pointer flex items-center gap-1 shadow-2xs"
        >
          {isRunning ? (
            <Loader2 className="w-3 h-3 animate-spin" />
          ) : (
            <RefreshCw className="w-3 h-3" />
          )}
          <span>Run Now</span>
        </button>

        <div className="flex items-center gap-1">
          <button
            type="button"
            data-testid={`btn-toggle-active-${topic.id}`}
            onClick={onToggle}
            className="p-1.5 rounded-xl bg-[#EFECE4] dark:bg-[#252820] hover:bg-[#E5E1D6] dark:hover:bg-[#2C3027] text-[#6E6D66] dark:text-[#A8A69E] transition-colors cursor-pointer text-xs"
            title={topic.is_active ? 'Pause Tracker' : 'Resume Tracker'}
          >
            {topic.is_active ? <Pause className="w-3.5 h-3.5" /> : <Play className="w-3.5 h-3.5" />}
          </button>
          <button
            type="button"
            data-testid={`btn-delete-tracker-${topic.id}`}
            onClick={onDelete}
            className="p-1.5 rounded-xl bg-[#EFECE4] dark:bg-[#252820] hover:bg-[#FBEBEB] dark:hover:bg-[#3D1E1E] text-[#6E6D66] dark:text-[#A8A69E] hover:text-[#C53030] dark:hover:text-[#F87171] transition-colors cursor-pointer text-xs"
            title="Delete Tracker"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>
    </div>
  );
};

export default ExplorePlanView;
