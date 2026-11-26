import { HelpTopicButton } from '../components/HelpTopicButton';
import { ContextInfo } from '../components/ContextInfo';
import { useWorkflowSettings } from '../hooks/useWorkflowSettings';
import {
  openWorkflowSettings,
  isWorkflowReady,
  resolveWorkflowModel,
  getEnabledModels,
} from '../api/workflowSettings';
import React, { useState, useEffect, useMemo, useCallback, useRef } from 'react';
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
  Info,
} from 'lucide-react';
import { ModelSelector } from '../components/ModelSelector';
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
import {
  INHERIT_DEFAULT, NO_TEMPLATE, SYNTHESIS_TEMPLATES_CHANGED, listSynthesisTemplates, openTemplateManager,
  type SynthesisTemplate,
} from '../api/synthesisTemplates';
import {
  generateSmartGoalTitle,
  type ExplorePlanTab,
  type ScheduledTopicView,
  type ScheduledTopicCadence,
  type ScheduledTopicType,
  type ProviderModelEntry,
  type ProviderEntry,
  type WebSearchMode,
  type WorkspaceIndexItem,
} from '../types/sthira';

export interface ExplorePlanViewProps {
  initialTab?: ExplorePlanTab;
  initialDigestFilter?: 'all' | 'research' | 'plan';
  onOpenArtifact?: (artifactId: string, projectId?: string | null) => void;
  onStartResearch?: (
    query: string,
    mode: WebSearchMode,
    projectId?: string | null,
    templateId?: string | null,
    modelId?: string | null
  ) => Promise<any>;
  onStartPlan?: (
    goal: string,
    projectId?: string | null,
    templateId?: string | null,
    modelId?: string | null
  ) => Promise<any>;
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
  const workflows = useWorkflowSettings();
  const researchModelChosen = useRef(false);
  const planModelChosen = useRef(false);
  const [selectedResearchModel, setSelectedResearchModel] = useState<string>('');
  const [selectedResearchProject, setSelectedResearchProject] = useState<string | null>(null);
  const [selectedResearchTemplate, setSelectedResearchTemplate] = useState<string>(INHERIT_DEFAULT);
  const [isSubmittingResearch, setIsSubmittingResearch] = useState(false);

  // Tab 2: Plan state
  const [planGoal, setPlanGoal] = useState('');
  const [selectedPlanModel, setSelectedPlanModel] = useState<string>('');
  const [selectedPlanProject, setSelectedPlanProject] = useState<string | null>(null);
  const [selectedPlanTemplate, setSelectedPlanTemplate] = useState<string>(INHERIT_DEFAULT);
  const [isSubmittingPlan, setIsSubmittingPlan] = useState(false);

  // Synthesis Templates state
  const [templates, setTemplates] = useState<SynthesisTemplate[]>([]);

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
  const [catalog, setCatalog] = useState<ProviderEntry[]>([]);
  const [catalogLoaded, setCatalogLoaded] = useState(false);
  const [catalogError, setCatalogError] = useState<unknown>(null);
  const [models, setModels] = useState<ProviderModelEntry[]>([]);
  const [notes, setNotes] = useState<WorkspaceIndexItem[]>([]);

  const isConfigLoading = workflows.loading || !catalogLoaded;
  const configError = workflows.error || catalogError;
  const researchReady = isWorkflowReady(
    workflows.settings.research,
    catalog,
    isConfigLoading,
    configError
  );
  const planReady = isWorkflowReady(
    workflows.settings.plan,
    catalog,
    isConfigLoading,
    configError
  );

  useEffect(() => {
    if (!catalog.length || workflows.loading) return;
    const resolvedResearch = resolveWorkflowModel(workflows.settings.research, catalog);
    const resolvedPlan = resolveWorkflowModel(workflows.settings.plan, catalog);
    if (!researchModelChosen.current) {
      setSelectedResearchModel(resolvedResearch ? resolvedResearch.id : '');
    }
    if (!planModelChosen.current) {
      setSelectedPlanModel(resolvedPlan ? resolvedPlan.id : '');
    }
  }, [catalog, workflows.settings, workflows.loading]);

  useEffect(() => {
    let active = true;
    const refresh = () => {
      void getProviderCatalog()
        .then((cat) => {
          if (!active) return;
          setCatalog(cat);
          setCatalogLoaded(true);
          setCatalogError(null);
          setModels(getEnabledModels(cat));
        })
        .catch((err) => {
          if (active) setCatalogError(err);
        });
    };
    window.addEventListener('sthira:catalog-changed', refresh);
    return () => {
      active = false;
      window.removeEventListener('sthira:catalog-changed', refresh);
    };
  }, []);

  // Web search capability for selected research model
  const activeResearchModel = useMemo(() => {
    return (
      models.find(
        (m) => m.model_identifier === selectedResearchModel || m.id === selectedResearchModel
      ) || null
    );
  }, [models, selectedResearchModel]);

  const researchCapability = activeResearchModel?.web_search_capability || 'agentic_tool_call';
  const isResearchUnsupported = researchCapability === 'unsupported';
  const isResearchEngineAugmented = researchCapability === 'engine_augmented_only';

  useEffect(() => {
    if (isResearchUnsupported && webSearchMode !== 'local_vault_only') {
      setWebSearchMode('local_vault_only');
    }
  }, [isResearchUnsupported, webSearchMode]);

  // Sync initial tab when changed from props or when readiness changes
  useEffect(() => {
    if (initialTab) {
      if (initialTab === 'research' && !researchReady && planReady) {
        setActiveTab('plan');
      } else if (initialTab === 'plan' && !planReady && researchReady) {
        setActiveTab('research');
      } else {
        setActiveTab(initialTab);
      }
    }
  }, [initialTab, researchReady, planReady]);

  // Auto-switch away from an unavailable workflow tab
  useEffect(() => {
    if (activeTab === 'research' && !researchReady && planReady) {
      setActiveTab('plan');
    } else if (activeTab === 'plan' && !planReady && researchReady) {
      setActiveTab('research');
    }
  }, [activeTab, researchReady, planReady]);

  // Load workspace data, topics, and models
  const loadData = useCallback(async () => {
    try {
      const [treeRes, catalogRes, indexRes, researchTopics, planTopics, synthesisTemplates] = await Promise.all([
        getWorkspaceTree(),
        getProviderCatalog().catch(() => []),
        getWorkspaceIndex('notes').catch(() => ({ counts: {}, items: [] })),
        listScheduledTopics('research').catch(() => []),
        listScheduledTopics('plan').catch(() => []),
        listSynthesisTemplates().catch(() => []),
      ]);

      setProjects(treeRes.projects.map((p) => ({ id: p.id, name: p.name })));
      setNotes(indexRes.items || []);

      if (synthesisTemplates) setTemplates(synthesisTemplates);

      setCatalog(catalogRes);
      setCatalogLoaded(true);
      setCatalogError(null);
      const availableModels = getEnabledModels(catalogRes);
      setModels(availableModels);

      const resolvedResearch = resolveWorkflowModel(workflows.settings.research, catalogRes);
      const resolvedPlan = resolveWorkflowModel(workflows.settings.plan, catalogRes);
      if (!researchModelChosen.current) {
        setSelectedResearchModel(resolvedResearch ? resolvedResearch.id : '');
      }
      if (!planModelChosen.current) {
        setSelectedPlanModel(resolvedPlan ? resolvedPlan.id : '');
      }

      // Combine topics
      const allTopics = [...researchTopics, ...planTopics].sort(
        (a, b) => b.created_at - a.created_at
      );
      setTopics(allTopics);
    } catch (err) {
      console.error('Failed to load explore & plan workspace data', err);
    }
  }, [workflows.settings]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  // Refresh picker options after a template is created, edited, archived, or made default.
  useEffect(() => {
    const refresh = () => {
      void listSynthesisTemplates().then(rows => setTemplates(rows)).catch(() => undefined);
    };
    window.addEventListener(SYNTHESIS_TEMPLATES_CHANGED, refresh);
    return () => window.removeEventListener(SYNTHESIS_TEMPLATES_CHANGED, refresh);
  }, []);

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
    if (!q || isSubmittingResearch || !researchReady || !models.some(model => model.id === selectedResearchModel)) return;
    setIsSubmittingResearch(true);
    try {
      if (onStartResearch) {
        const researchTemplate = !workflows.settings.research.templates_enabled
          ? null
          : selectedResearchTemplate === INHERIT_DEFAULT
            ? null
            : selectedResearchTemplate;
        await onStartResearch(q, webSearchMode, selectedResearchProject, researchTemplate, selectedResearchModel || undefined);
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
    if (!g || isSubmittingPlan || !planReady || !models.some(model => model.id === selectedPlanModel)) return;
    setIsSubmittingPlan(true);
    try {
      if (onStartPlan) {
        const planTemplate = !workflows.settings.plan.templates_enabled
          ? null
          : selectedPlanTemplate === INHERIT_DEFAULT
            ? null
            : selectedPlanTemplate;
        await onStartPlan(g, selectedPlanProject, planTemplate, selectedPlanModel || undefined);
      } else {
        const title = generateSmartGoalTitle(g);
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
    {
      id: 'research' as ExplorePlanTab,
      label: 'Research',
      icon: '🔍',
      disabled: !researchReady,
    },
    {
      id: 'plan' as ExplorePlanTab,
      label: 'Plan',
      icon: '📋',
      disabled: !planReady,
    },
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
      className="earth-workspace flex-1 min-h-screen bg-[var(--sthira-canvas)] text-[var(--sthira-text)] flex flex-col transition-colors overflow-y-auto"
    >
      <div className="max-w-6xl w-full mx-auto px-6 py-8 flex flex-col space-y-6">
        {/* Header & Breadcrumbs */}
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 border-b border-[var(--sthira-border)] pb-6">
          <div className="space-y-1">
            <div className="flex items-center gap-2 text-xs font-semibold text-[var(--sthira-text-muted)] uppercase tracking-wider">
              {onBack ? (
                <button
                  type="button"
                  data-testid="explore-plan-back"
                  onClick={onBack}
                  className="hover:text-[var(--sthira-text)] transition-colors cursor-pointer"
                >
                  Workspace
                </button>
              ) : (
                <span>Workspace</span>
              )}
              <span>/</span>
              <span className="text-[var(--sthira-text)]">Explore &amp; Plan</span>
            </div>
            <div className="context-info-heading">
              <h1 className="text-2xl sm:text-3xl font-bold font-editorial tracking-tight text-[var(--sthira-text)]">Explore &amp; Plan</h1>
              <ContextInfo title="Explore and Plan">Use Research for questions, Plan for next steps, and Trackers for recurring research. Digests collect the results of tracker runs.</ContextInfo>
            </div>
          </div>
        </div>

        {/* Top Segmented Tabs: Warm Stone Pill Bar */}
        <div className="flex items-center gap-1.5 overflow-x-auto pb-1 select-none">
          <div className="p-1 rounded-2xl bg-[var(--sthira-surface-muted)] border border-[var(--sthira-border)] inline-flex items-center gap-1">
            {TABS.map((tab) => {
              const isActive = activeTab === tab.id;
              const isDisabled = Boolean(tab.disabled);
              return (
                <button
                  key={tab.id}
                  type="button"
                  data-testid={`tab-${tab.id}`}
                  disabled={isDisabled}
                  aria-disabled={isDisabled ? 'true' : undefined}
                  title={isDisabled ? 'Enable this workflow and select a model in Settings.' : undefined}
                  onClick={() => {
                    if (!isDisabled) setActiveTab(tab.id);
                  }}
                  className={`flex items-center gap-2 px-3.5 py-1.5 rounded-xl text-xs font-semibold transition-all whitespace-nowrap ${
                    isDisabled
                      ? 'opacity-40 cursor-not-allowed text-[var(--sthira-text-muted)] select-none'
                      : isActive
                      ? 'bg-[var(--sthira-primary)] text-white shadow-2xs cursor-pointer'
                      : 'text-[var(--sthira-text-muted)] hover:text-[var(--sthira-text)] hover:bg-white/60 dark:hover:bg-[#252820] cursor-pointer'
                  }`}
                >
                  <span>
                    {tab.icon} {tab.label}
                  </span>
                  {tab.count !== undefined && (
                    <span
                      data-testid={tab.countTestId}
                      className={`text-xs px-1.5 py-0.2 rounded-full font-mono ${
                        isActive
                          ? 'bg-white/20 text-white'
                          : 'bg-[#E5E1D6] dark:bg-[#282B23] text-[#7A7870] dark:text-[#B5B2A8]'
                      }`}
                    >
                      {tab.count}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        </div>

        {/* Tab 1: RESEARCH */}
        {activeTab === 'research' && (
          <div data-testid="tab-content-research" className="space-y-8 animate-fadeIn">
            {/* Input Card */}
            <div
              data-testid="research-input-card"
              className="rounded-2xl bg-[var(--sthira-surface)] border border-[var(--sthira-border)] p-6 shadow-2xs space-y-4"
            >
              <div className="space-y-1">
                <label
                  htmlFor="research-query-textarea"
                  className="text-xs font-bold uppercase tracking-wider text-[var(--sthira-text-muted)]"
                >
                  Autonomous Research Inquiry
                </label>
                <textarea
                  id="research-query-textarea"
                  data-testid="input-research-query"
                  rows={3}
                  value={researchQuery}
                  disabled={!researchReady}
                  onChange={(e) => setResearchQuery(e.target.value)}
                  onKeyDown={(e) => {
                    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
                      e.preventDefault();
                      handleSubmitResearch();
                    }
                  }}
                  placeholder={researchReady ? "What would you like to research?" : "Enable Research and select a model in Settings."}
                  className={`w-full bg-[var(--sthira-canvas)]/50 border border-[var(--sthira-border)] rounded-xl p-3.5 text-sm text-[var(--sthira-text)] placeholder:text-[var(--sthira-text-muted)] outline-none focus:border-[var(--sthira-primary)] transition-colors resize-none ${!researchReady ? 'opacity-50 cursor-not-allowed' : ''}`}
                />
              </div>

              {/* Controls Row */}
              <div className="flex flex-wrap items-center justify-between gap-3 pt-1">
                <div className="flex flex-wrap items-center gap-2.5">
                  {/* Web Search Mode Toggle */}
                  <div className="flex items-center gap-1.5">
                    <div
                      data-testid="toggle-web-search"
                      className={`flex items-center gap-1 bg-[var(--sthira-canvas)] p-1 rounded-xl border border-[var(--sthira-border)] ${
                        isResearchUnsupported ? 'opacity-50 cursor-not-allowed' : ''
                      }`}
                      title={
                        isResearchUnsupported
                          ? 'Web search is disabled because the selected model is offline/unsupported.'
                          : undefined
                      }
                    >
                      <span className="text-[11px] font-semibold text-[var(--sthira-text-muted)] px-1.5 flex items-center gap-1">
                        <Globe className="w-3 h-3" />
                        <span>Web Search:</span>
                      </span>
                      <button
                        type="button"
                        data-testid="search-mode-auto"
                        disabled={isResearchUnsupported}
                        onClick={() => setWebSearchMode('auto')}
                        className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-colors ${
                          isResearchUnsupported ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'
                        } ${
                          webSearchMode === 'auto'
                            ? 'bg-[var(--sthira-primary)] text-white hover:bg-[var(--sthira-primary-hover)] font-semibold shadow-2xs'
                            : 'text-[var(--sthira-text-muted)] hover:text-[var(--sthira-text)]'
                        }`}
                      >
                        Auto
                      </button>
                      <button
                        type="button"
                        data-testid="search-mode-always"
                        disabled={isResearchUnsupported}
                        onClick={() => setWebSearchMode('always_search')}
                        className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-colors ${
                          isResearchUnsupported ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'
                        } ${
                          webSearchMode === 'always_search'
                            ? 'bg-[var(--sthira-primary)] text-white hover:bg-[var(--sthira-primary-hover)] font-semibold shadow-2xs'
                            : 'text-[var(--sthira-text-muted)] hover:text-[var(--sthira-text)]'
                        }`}
                      >
                        Enabled
                      </button>
                      <button
                        type="button"
                        data-testid="search-mode-vault"
                        disabled={isResearchUnsupported}
                        onClick={() => setWebSearchMode('local_vault_only')}
                        className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-colors ${
                          isResearchUnsupported ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'
                        } ${
                          webSearchMode === 'local_vault_only'
                            ? 'bg-[var(--sthira-primary)] text-white hover:bg-[var(--sthira-primary-hover)] font-semibold shadow-2xs'
                            : 'text-[var(--sthira-text-muted)] hover:text-[var(--sthira-text)]'
                        }`}
                      >
                        Local Vault Only
                      </button>
                    </div>

                    {/* Quiet hint icon for EngineAugmentedOnly */}
                    {isResearchEngineAugmented && (
                      <span
                        data-testid="web-search-engine-augmented-hint"
                        title="Sthira will automatically query SearXNG/Brave and inject scraped web pages into this model's prompt."
                        className="inline-flex items-center text-[var(--sthira-text-muted)] hover:text-[var(--sthira-text)] cursor-help p-1"
                      >
                        <Info className="w-3.5 h-3.5" />
                      </span>
                    )}

                    {/* Explicit indicator when model is Unsupported */}
                    {isResearchUnsupported && (
                      <span
                        data-testid="web-search-unsupported-hint"
                        title="Web search is disabled because the selected model is offline/unsupported."
                        className="text-[11px] text-stone-500 italic pl-0.5"
                      >
                        (Offline only)
                      </span>
                    )}
                  </div>

                  {/* Model Selector */}
                  <div className="flex items-center gap-1.5">
                    <Sparkles className="w-3.5 h-3.5 text-[var(--sthira-text-muted)]" />
                    <ModelSelector
                      dataTestId="select-research-model"
                      models={models}
                      selectedModelId={selectedResearchModel}
                      onSelectModel={(m) => { researchModelChosen.current = true; setSelectedResearchModel(m.id); }}
                    />
                  </div>

                  {/* Project Selector */}
                  <select
                    data-testid="select-research-project"
                    value={selectedResearchProject || ''}
                    onChange={(e) => setSelectedResearchProject(e.target.value || null)}
                    className="px-2.5 py-1.5 bg-[var(--sthira-canvas)] border border-[var(--sthira-border)] rounded-xl text-xs text-[var(--sthira-text)] outline-none cursor-pointer"
                  >
                    <option value="">No Project (Standalone)</option>
                    {projects.map((p) => (
                      <option key={p.id} value={p.id}>
                        📁 {p.name}
                      </option>
                    ))}
                  </select>

                  {/* Synthesis Template Selector */}
                  <div className="flex items-center gap-1.5">
                    <select
                      data-testid="select-synthesis-template"
                      aria-label="Research template"
                      value={selectedResearchTemplate}
                      disabled={!researchReady || !workflows.settings.research.templates_enabled}
                      onChange={(e) => setSelectedResearchTemplate(e.target.value)}
                      className="max-w-[220px] px-2.5 py-1.5 bg-[var(--sthira-canvas)] border border-[var(--sthira-border)] rounded-xl text-xs text-[var(--sthira-text)] outline-none cursor-pointer"
                      title="Choose a Research template, inherit the workflow default, or use no template"
                    >
                      <option value={INHERIT_DEFAULT}>
                        Inherit default{templates.find((t) => t.target_mode === 'research' && t.is_default) ? ` · ${templates.find((t) => t.target_mode === 'research' && t.is_default)!.name}` : ' (none set)'}
                      </option>
                      <option value={NO_TEMPLATE}>No template</option>
                      {templates
                        .filter((t) => t.target_mode === 'research')
                        .map((t) => (
                          <option key={t.id} value={t.id}>
                            {t.is_builtin ? '📑' : '✳️'} {t.name} · r{t.version}
                          </option>
                        ))}
                    </select>
                    <button
                      type="button"
                      data-testid="btn-manage-research-templates"
                      onClick={() => openTemplateManager('research')}
                      className="text-[11px] text-[var(--sthira-text-muted)] hover:text-[var(--sthira-primary)] underline decoration-dotted whitespace-nowrap"
                    >
                      Manage
                    </button>
                  </div>
                </div>

                {/* Submit Action */}
                <button
                  type="button"
                  data-testid="btn-start-research"
                  disabled={!researchReady || !models.some(model => model.id === selectedResearchModel) || !researchQuery.trim() || isSubmittingResearch}
                  onClick={handleSubmitResearch}
                  className="px-5 py-2 bg-[var(--sthira-primary)] text-white hover:bg-[var(--sthira-primary-hover)] rounded-xl text-xs font-semibold hover:opacity-90 disabled:opacity-40 transition-opacity cursor-pointer flex items-center gap-2 shadow-2xs"
                >
                  {isSubmittingResearch ? (
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  ) : (
                    <Search className="w-3.5 h-3.5" />
                  )}
                  <span>Start Research</span>
                  <span className="text-xs opacity-60 font-mono">⌘↵</span>
                </button>
              </div>
            </div>

            {!researchReady && (
              <div
                data-testid="research-unavailable-banner"
                className="workflow-setup-notice p-4 rounded-xl border border-[var(--sthira-border)] bg-[var(--sthira-surface-muted)] flex items-center justify-between gap-4"
              >
                <p className="text-xs text-[var(--sthira-text-muted)]">
                  Enable this workflow and select a model in Settings.
                  <HelpTopicButton article="troubleshooting" label="Setup help" />
                </p>
                <button
                  type="button"
                  data-testid="btn-setup-research-ai"
                  onClick={openWorkflowSettings}
                  className="px-3 py-1.5 text-xs font-semibold rounded-lg bg-sthira-primary/10 !text-sthira-primary hover:bg-sthira-primary hover:!text-sthira-on-primary transition-colors cursor-pointer whitespace-nowrap !no-underline !min-h-0"
                >
                  Check settings
                </button>
              </div>
            )}
            {/* Recent Research Section */}
            <div data-testid="recent-research-section" className="space-y-4">
              <div className="flex items-center justify-between border-b border-[#E8E5DC]/60 dark:border-[#2C2E27]/60 pb-2">
                <h2 className="text-sm font-bold text-[var(--sthira-text)] flex items-center gap-2">
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
                  className="text-xs font-semibold text-[var(--sthira-text)] hover:underline flex items-center gap-1 cursor-pointer"
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
                      className="group p-4 bg-[var(--sthira-surface)] border border-[var(--sthira-border)] rounded-xl hover:border-[var(--sthira-primary)] transition-all cursor-pointer shadow-2xs flex flex-col justify-between space-y-3"
                    >
                      <div className="space-y-1">
                        <div className="flex items-center justify-between gap-2">
                          <span className="text-xs font-semibold px-2 py-0.5 rounded-md bg-[#E8EEF5] dark:bg-[#1F2633] text-[#2B4C6F] dark:text-[#A2C2E8]">
                            🔍 Research
                          </span>
                          <span className="text-xs text-[var(--sthira-text-muted)]">
                            {formatRelativeTime(item.updated_at || item.created_at)}
                          </span>
                        </div>
                        <h3 className="text-xs font-bold text-[var(--sthira-text)] line-clamp-2 group-hover:underline">
                          {item.title}
                        </h3>
                        {item.snippet && (
                          <p className="text-[11px] text-[var(--sthira-text-muted)] line-clamp-2">
                            {item.snippet}
                          </p>
                        )}
                      </div>
                      <div className="flex items-center justify-between text-[11px] text-[#7A7870] pt-1 border-t border-[var(--sthira-border)]/50">
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
                <div className="p-8 text-center rounded-xl bg-[var(--sthira-surface)]/40 border border-dashed border-[var(--sthira-border)] text-xs text-[var(--sthira-text-muted)]">
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
              className="rounded-2xl bg-[var(--sthira-surface)] border border-[var(--sthira-border)] p-6 shadow-2xs space-y-4"
            >
              <div className="space-y-1">
                <label
                  htmlFor="plan-goal-textarea"
                  className="text-xs font-bold uppercase tracking-wider text-[var(--sthira-text-muted)]"
                >
                  Milestone &amp; Architecture Planning
                </label>
                <textarea
                  id="plan-goal-textarea"
                  data-testid="input-plan-goal"
                  rows={3}
                  value={planGoal}
                  disabled={!planReady}
                  onChange={(e) => setPlanGoal(e.target.value)}
                  onKeyDown={(e) => {
                    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
                      e.preventDefault();
                      handleSubmitPlan();
                    }
                  }}
                  placeholder={planReady ? "What project, feature, or milestone set are we planning?" : "Enable Plan and select a model in Settings."}
                  className={`w-full bg-[var(--sthira-canvas)]/50 border border-[var(--sthira-border)] rounded-xl p-3.5 text-sm text-[var(--sthira-text)] placeholder:text-[var(--sthira-text-muted)] outline-none focus:border-[var(--sthira-primary)] transition-colors resize-none ${!planReady ? 'opacity-50 cursor-not-allowed' : ''}`}
                />
              </div>

              {/* Controls Row */}
              <div className="flex flex-wrap items-center justify-between gap-3 pt-1">
                <div className="flex flex-wrap items-center gap-2.5">
                  {/* Model Selector */}
                  <div className="flex items-center gap-1.5">
                    <Sparkles className="w-3.5 h-3.5 text-[var(--sthira-text-muted)]" />
                    <ModelSelector
                      dataTestId="select-plan-model"
                      models={models}
                      selectedModelId={selectedPlanModel}
                      onSelectModel={(m) => { planModelChosen.current = true; setSelectedPlanModel(m.id); }}
                    />
                  </div>

                  {/* Project Selector */}
                  <select
                    data-testid="select-plan-project"
                    value={selectedPlanProject || ''}
                    onChange={(e) => setSelectedPlanProject(e.target.value || null)}
                    className="px-2.5 py-1.5 bg-[var(--sthira-canvas)] border border-[var(--sthira-border)] rounded-xl text-xs text-[var(--sthira-text)] outline-none cursor-pointer"
                  >
                    <option value="">No Project (Standalone)</option>
                    {projects.map((p) => (
                      <option key={p.id} value={p.id}>
                        📁 {p.name}
                      </option>
                    ))}
                  </select>

                  {/* Synthesis Template Selector */}
                  <div className="flex items-center gap-1.5">
                    <select
                      data-testid="select-plan-synthesis-template"
                      aria-label="Plan template"
                      value={selectedPlanTemplate}
                      disabled={!planReady || !workflows.settings.plan.templates_enabled}
                      onChange={(e) => setSelectedPlanTemplate(e.target.value)}
                      className="max-w-[220px] px-2.5 py-1.5 bg-[var(--sthira-canvas)] border border-[var(--sthira-border)] rounded-xl text-xs text-[var(--sthira-text)] outline-none cursor-pointer"
                      title="Choose a Plan template, inherit the workflow default, or use no template"
                    >
                      <option value={INHERIT_DEFAULT}>
                        Inherit default{templates.find((t) => t.target_mode === 'plan' && t.is_default) ? ` · ${templates.find((t) => t.target_mode === 'plan' && t.is_default)!.name}` : ' (none set)'}
                      </option>
                      <option value={NO_TEMPLATE}>No template</option>
                      {templates
                        .filter((t) => t.target_mode === 'plan')
                        .map((t) => (
                          <option key={t.id} value={t.id}>
                            {t.is_builtin ? '📑' : '✳️'} {t.name} · r{t.version}
                          </option>
                        ))}
                    </select>
                    <button
                      type="button"
                      data-testid="btn-manage-plan-templates"
                      onClick={() => openTemplateManager('plan')}
                      className="text-[11px] text-[var(--sthira-text-muted)] hover:text-[var(--sthira-primary)] underline decoration-dotted whitespace-nowrap"
                    >
                      Manage
                    </button>
                  </div>
                </div>

                {/* Submit Action */}
                <button
                  type="button"
                  data-testid="btn-start-plan"
                  disabled={!planReady || !models.some(model => model.id === selectedPlanModel) || !planGoal.trim() || isSubmittingPlan}
                  onClick={handleSubmitPlan}
                  className="px-5 py-2 bg-[var(--sthira-primary)] text-white hover:bg-[var(--sthira-primary-hover)] rounded-xl text-xs font-semibold hover:opacity-90 disabled:opacity-40 transition-opacity cursor-pointer flex items-center gap-2 shadow-2xs"
                >
                  {isSubmittingPlan ? (
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  ) : (
                    <ClipboardList className="w-3.5 h-3.5" />
                  )}
                  <span>Create Plan</span>
                  <span className="text-xs opacity-60 font-mono">⌘↵</span>
                </button>
              </div>
            </div>

            {!planReady && (
              <div
                data-testid="plan-unavailable-banner"
                className="workflow-setup-notice p-4 rounded-xl border border-[var(--sthira-border)] bg-[var(--sthira-surface-muted)] flex items-center justify-between gap-4"
              >
                <p className="text-xs text-[var(--sthira-text-muted)]">
                  Enable this workflow and select a model in Settings.
                  <HelpTopicButton article="troubleshooting" label="Setup help" />
                </p>
                <button
                  type="button"
                  data-testid="btn-setup-plan-ai"
                  onClick={openWorkflowSettings}
                  className="px-3 py-1.5 text-xs font-semibold rounded-lg bg-sthira-primary/10 !text-sthira-primary hover:bg-sthira-primary hover:!text-sthira-on-primary transition-colors cursor-pointer whitespace-nowrap !no-underline !min-h-0"
                >
                  Check settings
                </button>
              </div>
            )}
            {/* Recent Plans Section */}
            <div data-testid="recent-plans-section" className="space-y-4">
              <div className="flex items-center justify-between border-b border-[#E8E5DC]/60 dark:border-[#2C2E27]/60 pb-2">
                <h2 className="text-sm font-bold text-[var(--sthira-text)] flex items-center gap-2">
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
                  className="text-xs font-semibold text-[var(--sthira-text)] hover:underline flex items-center gap-1 cursor-pointer"
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
                      className="group p-4 bg-[var(--sthira-surface)] border border-[var(--sthira-border)] rounded-xl hover:border-[var(--sthira-primary)] transition-all cursor-pointer shadow-2xs flex flex-col justify-between space-y-3"
                    >
                      <div className="space-y-1">
                        <div className="flex items-center justify-between gap-2">
                          <span className="text-xs font-semibold px-2 py-0.5 rounded-md bg-[#F2ECE1] dark:bg-[#2A261D] text-[#875F1A] dark:text-[#E8C582]">
                            📋 Plan
                          </span>
                          <span className="text-xs text-[var(--sthira-text-muted)]">
                            {formatRelativeTime(item.updated_at || item.created_at)}
                          </span>
                        </div>
                        <h3 className="text-xs font-bold text-[var(--sthira-text)] line-clamp-2 group-hover:underline">
                          {item.title}
                        </h3>
                        {item.snippet && (
                          <p className="text-[11px] text-[var(--sthira-text-muted)] line-clamp-2">
                            {item.snippet}
                          </p>
                        )}
                      </div>
                      <div className="flex items-center justify-between text-[11px] text-[#7A7870] pt-1 border-t border-[var(--sthira-border)]/50">
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
                <div className="p-8 text-center rounded-xl bg-[var(--sthira-surface)]/40 border border-dashed border-[var(--sthira-border)] text-xs text-[var(--sthira-text-muted)]">
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
                <h2 className="text-sm font-bold text-[var(--sthira-text)] flex items-center gap-2">
                  <span>Active Background Trackers</span>
                  <span className="text-xs text-[#8C8A81] font-normal">({topics.length} total)</span>
                </h2>
              </div>
              <button
                type="button"
                data-testid="btn-new-tracker"
                onClick={() => setIsNewTrackerOpen(true)}
                className="px-3.5 py-2 bg-[var(--sthira-primary)] text-white hover:bg-[var(--sthira-primary-hover)] rounded-xl text-xs font-semibold hover:opacity-90 transition-opacity cursor-pointer flex items-center gap-1.5 shadow-2xs"
              >
                <Plus className="w-3.5 h-3.5" />
                <span>+ New Tracker</span>
              </button>
            </div>

            {/* New Tracker Modal / Inline Drawer */}
            {isNewTrackerOpen && (
              <div
                data-testid="modal-new-tracker"
                className="rounded-2xl bg-[var(--sthira-surface)] border-2 border-[var(--sthira-primary)] p-6 shadow-md space-y-4 animate-fadeIn"
              >
                <div className="flex items-center justify-between border-b border-[var(--sthira-border)] pb-3">
                  <h3 className="text-sm font-bold text-[var(--sthira-text)] flex items-center gap-2">
                    <Clock className="w-4 h-4" />
                    <span>Configure Recurring Tracker</span>
                  </h3>
                  <button
                    type="button"
                    data-testid="btn-cancel-scheduled-topic"
                    onClick={() => setIsNewTrackerOpen(false)}
                    className="p-1 rounded-lg hover:bg-[var(--sthira-surface-muted)] text-[var(--sthira-text-muted)] cursor-pointer"
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
                            ? 'bg-[var(--sthira-primary)] text-white hover:bg-[var(--sthira-primary-hover)]'
                            : 'bg-[var(--sthira-canvas)] border border-[var(--sthira-border)] text-[#6E6D66]'
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
                            ? 'bg-[var(--sthira-primary)] text-white hover:bg-[var(--sthira-primary-hover)]'
                            : 'bg-[var(--sthira-canvas)] border border-[var(--sthira-border)] text-[#6E6D66]'
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
                      className="w-full px-3 py-2 bg-[var(--sthira-canvas)] border border-[var(--sthira-border)] rounded-xl text-xs text-[var(--sthira-text)] outline-none"
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
                      className="w-full px-3 py-2 bg-[var(--sthira-canvas)] border border-[var(--sthira-border)] rounded-xl text-xs text-[var(--sthira-text)] outline-none resize-none"
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
                        className="w-full px-2.5 py-1.5 bg-[var(--sthira-canvas)] border border-[var(--sthira-border)] rounded-xl text-xs text-[var(--sthira-text)] outline-none cursor-pointer"
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
                          className="w-full px-2.5 py-1.5 bg-[var(--sthira-canvas)] border border-[var(--sthira-border)] rounded-xl text-xs text-[var(--sthira-text)] outline-none cursor-pointer"
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
                          className="w-full px-2.5 py-1.5 bg-[var(--sthira-canvas)] border border-[var(--sthira-border)] rounded-xl text-xs text-[var(--sthira-text)] outline-none cursor-pointer"
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
                        className="w-full px-2.5 py-1.5 bg-[var(--sthira-canvas)] border border-[var(--sthira-border)] rounded-xl text-xs text-[var(--sthira-text)] outline-none cursor-pointer"
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
                      className="px-4 py-1.5 rounded-xl text-xs text-[#6E6D66] hover:bg-[var(--sthira-surface-muted)] cursor-pointer"
                    >
                      Cancel
                    </button>
                    <button
                      type="submit"
                      data-testid="btn-save-scheduled-topic"
                      disabled={!newTrackerTitle.trim() || !newTrackerPrompt.trim() || isSavingTracker}
                      className="px-4 py-1.5 bg-[var(--sthira-primary)] text-white hover:bg-[var(--sthira-primary-hover)] rounded-xl text-xs font-semibold hover:opacity-90 disabled:opacity-40 cursor-pointer"
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
                <h3 className="text-xs font-bold uppercase tracking-wider text-[var(--sthira-text-muted)]">
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
                  <div className="col-span-full p-6 text-center rounded-xl bg-[var(--sthira-surface)]/40 border border-dashed border-[var(--sthira-border)] text-xs text-[var(--sthira-text-muted)]">
                    No research trackers configured. Click &ldquo;+ New Tracker&rdquo; to set up an
                    autonomous query.
                  </div>
                )}
              </div>
            </div>

            <div data-testid="scheduled-section-plan" className="space-y-3 pt-4">
              <div className="flex items-center gap-2">
                <ClipboardList className="w-4 h-4 text-[#7A7870]" />
                <h3 className="text-xs font-bold uppercase tracking-wider text-[var(--sthira-text-muted)]">
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
                  <div className="col-span-full p-6 text-center rounded-xl bg-[var(--sthira-surface)]/40 border border-dashed border-[var(--sthira-border)] text-xs text-[var(--sthira-text-muted)]">
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
                <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-[var(--sthira-text-muted)]" />
                <input
                  type="text"
                  data-testid="input-search-digests"
                  value={digestSearch}
                  onChange={(e) => setDigestSearch(e.target.value)}
                  placeholder="Filter past research & plan notes..."
                  className="w-full pl-9 pr-8 py-2 bg-[var(--sthira-surface)] border border-[var(--sthira-border)] rounded-xl text-xs text-[var(--sthira-text)] placeholder:text-[var(--sthira-text-muted)] outline-none focus:border-[var(--sthira-primary)] transition-colors shadow-2xs"
                />
                {digestSearch && (
                  <button
                    type="button"
                    onClick={() => setDigestSearch('')}
                    className="absolute right-2.5 top-1/2 -translate-y-1/2 text-xs text-[var(--sthira-text-muted)] hover:text-[var(--sthira-text)]"
                  >
                    Clear
                  </button>
                )}
              </div>

              {/* Type Filter Pills */}
              <div className="flex items-center gap-1.5 overflow-x-auto select-none">
                <span className="text-[11px] font-semibold text-[var(--sthira-text-muted)] mr-1 flex items-center gap-1">
                  <Filter className="w-3 h-3" />
                  <span>Type:</span>
                </span>
                <button
                  type="button"
                  data-testid="filter-type-all"
                  onClick={() => setDigestTypeFilter('all')}
                  className={`px-3 py-1.5 rounded-lg text-xs font-semibold transition-colors cursor-pointer ${
                    digestTypeFilter === 'all'
                      ? 'bg-[var(--sthira-primary)] text-white hover:bg-[var(--sthira-primary-hover)] shadow-2xs'
                      : 'bg-[var(--sthira-surface)]/70 text-[#6E6D66] border border-[var(--sthira-border)]'
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
                      ? 'bg-[var(--sthira-primary)] text-white hover:bg-[var(--sthira-primary-hover)] shadow-2xs'
                      : 'bg-[var(--sthira-surface)]/70 text-[#6E6D66] border border-[var(--sthira-border)]'
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
                      ? 'bg-[var(--sthira-primary)] text-white hover:bg-[var(--sthira-primary-hover)] shadow-2xs'
                      : 'bg-[var(--sthira-surface)]/70 text-[#6E6D66] border border-[var(--sthira-border)]'
                  }`}
                >
                  📋 Plan ({allDigests.filter((d) => d.artifact_type === 'plan').length})
                </button>
              </div>
            </div>

            {/* Filterable Table */}
            <div
              data-testid="digests-table"
              className="rounded-2xl bg-[var(--sthira-surface)] border border-[var(--sthira-border)] shadow-2xs overflow-hidden"
            >
              <table className="w-full text-left border-collapse text-xs">
                <thead>
                  <tr className="border-b border-[var(--sthira-border)] bg-[var(--sthira-canvas)]/60 text-[11px] font-bold text-[var(--sthira-text-muted)] uppercase">
                    <th className="py-3 px-4">Title</th>
                    <th className="py-3 px-3 w-28">Type</th>
                    <th className="py-3 px-3 w-36">Project</th>
                    <th className="py-3 px-3 w-32">Date Generated</th>
                    <th className="py-3 px-3 w-12 text-center">•••</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-[var(--sthira-border)]/60">
                  {filteredDigests.map((item) => (
                    <tr
                      key={item.id}
                      data-testid={`digest-row-${item.id}`}
                      onClick={() => onOpenArtifact?.(item.id, item.project_id)}
                      className="group hover:bg-[var(--sthira-canvas)] transition-colors cursor-pointer"
                    >
                      {/* Title */}
                      <td className="py-3.5 px-4">
                        <div className="font-semibold text-[var(--sthira-text)] group-hover:underline flex items-center gap-2">
                          <span>{item.title}</span>
                        </div>
                        {item.snippet && (
                          <div className="text-[11px] text-[var(--sthira-text-muted)] line-clamp-1 mt-0.5">
                            {item.snippet}
                          </div>
                        )}
                      </td>

                      {/* Type Badge */}
                      <td className="py-3.5 px-3">
                        {item.artifact_type === 'research' ? (
                          <span
                            data-testid={`badge-type-${item.id}`}
                            className="inline-flex items-center gap-1 text-xs font-semibold px-2 py-0.5 rounded-md bg-[#E8EEF5] dark:bg-[#1F2633] text-[#2B4C6F] dark:text-[#A2C2E8]"
                          >
                            🔍 Research
                          </span>
                        ) : (
                          <span
                            data-testid={`badge-type-${item.id}`}
                            className="inline-flex items-center gap-1 text-xs font-semibold px-2 py-0.5 rounded-md bg-[#F2ECE1] dark:bg-[#2A261D] text-[#875F1A] dark:text-[#E8C582]"
                          >
                            📋 Plan
                          </span>
                        )}
                      </td>

                      {/* Project Chip */}
                      <td className="py-3.5 px-3 text-[var(--sthira-text-muted)]">
                        {item.project_name ? (
                          <span className="inline-flex items-center gap-1 text-[11px] px-2 py-0.5 rounded-md bg-[var(--sthira-surface-muted)] text-[var(--sthira-text)] truncate max-w-[130px]">
                            <Folder className="w-3 h-3 flex-shrink-0" />
                            <span className="truncate">{item.project_name}</span>
                          </span>
                        ) : (
                          <span className="text-[11px] text-[var(--sthira-text-muted)]">Standalone</span>
                        )}
                      </td>

                      {/* Date Generated */}
                      <td className="py-3.5 px-3 text-[var(--sthira-text-muted)]">
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
                        className="py-12 text-center text-xs text-[var(--sthira-text-muted)]"
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
      className="p-5 rounded-2xl bg-[var(--sthira-surface)] border border-[var(--sthira-border)] shadow-2xs hover:shadow-xs transition-shadow flex flex-col justify-between space-y-4"
    >
      <div className="space-y-2.5">
        {/* Top Badges */}
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-1.5 flex-wrap">
            <span
              data-testid={`cadence-badge-${topic.id}`}
              className="text-xs font-semibold px-2 py-0.5 rounded-md uppercase font-mono bg-[var(--sthira-surface-muted)] text-[var(--sthira-text-muted)]"
            >
              {topic.cadence}
            </span>
            <span
              data-testid={`status-badge-${topic.id}`}
              className={`text-xs font-semibold px-2 py-0.5 rounded-md ${
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
            className="text-xs text-[var(--sthira-text-muted)] font-mono"
          >
            {topic.schedule_label || topic.cadence}
          </span>
        </div>

        {/* Title & Prompt */}
        <div>
          <h4 className="text-sm font-bold text-[var(--sthira-text)]">
            {topic.title}
          </h4>
          <p className="text-[11px] text-[var(--sthira-text-muted)] line-clamp-2 mt-1">
            {topic.prompt_query}
          </p>
        </div>

        {/* Meta / Links */}
        <div className="space-y-1 text-[11px] text-[var(--sthira-text-muted)] pt-1 border-t border-[var(--sthira-border)]/50">
          <div className="flex items-center justify-between">
            <span className="flex items-center gap-1">
              <Folder className="w-3 h-3 flex-shrink-0" />
              <span>{topic.project_name || 'Standalone'}</span>
            </span>
            <span data-testid={`next-run-${topic.id}`} className="text-xs">
              {topic.next_run_at ? `Next in ${formatUntil(topic.next_run_at)}` : ''}
            </span>
          </div>
          <div className="text-xs text-[var(--sthira-text-muted)]">
            Last run: {formatRelativeTime(topic.last_run_at)}
          </div>
        </div>

        {/* Previous Runs (if any) */}
        {topic.runs && topic.runs.length > 0 && (
          <div className="pt-2 border-t border-[var(--sthira-border)]/50 space-y-1">
            <span className="text-xs font-bold text-[var(--sthira-text-muted)] uppercase">
              Latest Digest:
            </span>
            {topic.runs.slice(0, 1).map((run) => (
              <button
                key={run.id}
                type="button"
                data-testid={`digest-link-${run.artifact_id}`}
                onClick={() => onOpenArtifact?.(run.artifact_id, topic.project_id)}
                className="w-full text-left text-[11px] font-medium text-[var(--sthira-text)] hover:underline flex items-center justify-between gap-1 p-1.5 rounded-lg bg-[var(--sthira-canvas)] border border-[var(--sthira-border)] cursor-pointer"
              >
                <span className="truncate">{run.artifact_title}</span>
                <ChevronRight className="w-3 h-3 flex-shrink-0" />
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Action Buttons Row */}
      <div className="flex items-center justify-between gap-2 pt-2 border-t border-[var(--sthira-border)]">
        <button
          type="button"
          data-testid={`btn-run-now-${topic.id}`}
          disabled={isRunning}
          onClick={onRun}
          className="px-2.5 py-1.5 rounded-xl bg-[var(--sthira-primary)] text-white hover:bg-[var(--sthira-primary-hover)] text-[11px] font-semibold hover:opacity-90 disabled:opacity-50 transition-opacity cursor-pointer flex items-center gap-1 shadow-2xs"
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
            className="p-1.5 rounded-xl bg-[var(--sthira-surface-muted)] hover:bg-[#E5E1D6] dark:hover:bg-[#2C3027] text-[var(--sthira-text-muted)] transition-colors cursor-pointer text-xs"
            title={topic.is_active ? 'Pause Tracker' : 'Resume Tracker'}
          >
            {topic.is_active ? <Pause className="w-3.5 h-3.5" /> : <Play className="w-3.5 h-3.5" />}
          </button>
          <button
            type="button"
            data-testid={`btn-delete-tracker-${topic.id}`}
            onClick={onDelete}
            className="p-1.5 rounded-xl bg-[var(--sthira-surface-muted)] hover:bg-[#FBEBEB] dark:hover:bg-[#3D1E1E] text-[var(--sthira-text-muted)] hover:text-[#C53030] dark:hover:text-[#F87171] transition-colors cursor-pointer text-xs"
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
