import React, { useState, useEffect, useRef } from 'react';
import {
  ArrowUp,
  Plus,
  Sparkles,
  Check,
  ChevronDown,
  FolderPlus,
  FileText,
  Bookmark,
  Layers,
  HelpCircle,
  X,
  Globe,
  Info,
} from 'lucide-react';
import { WebSearchCapabilityBadge } from './ModelSelector';
import {
  getProviderCatalog,
  executePlanAction,
  createPage,
  saveArtifact,
} from '../api/ipc';
import { useAdaptiveStream } from '../hooks/useAdaptiveStream';
import { AdaptiveResponseRenderer } from './AdaptiveResponseRenderer';
import type {
  OperationalMode,
  ProviderEntry,
  ProviderModelEntry,
  StructuredPlanPreview,
  PlanProposedAction,
  RouterPlan,
  ProjectCardView,
  WebSearchMode,
  WebResearchResult,
} from '../types/sthira';

interface UniversalInputBarProps {
  onProjectSelect?: (projectId: string) => void;
  onOpenArtifact?: (artifactId: string, projectId?: string | null) => void;
  onRefresh?: () => void;
  onOpenFlow?: (mode: 'research' | 'plan' | 'capture', initialPrompt?: string) => void;
  onOpenSettings?: () => void;
  projects?: ProjectCardView[];
  /**
   * Phase 20: runs an autonomous research pipeline that streams directly into a new
   * page in the full-width center workspace. When provided, `Research` mode submits
   * through this instead of the legacy inline/flow path.
   */
  onStartResearch?: (
    query: string,
    mode: WebSearchMode,
    projectId?: string | null
  ) => Promise<WebResearchResult | null>;
  /**
   * Phase 20: creates a `Plan: <Goal>` draft and streams LLM tokens into the center
   * workspace document editor.
   */
  onStartPlan?: (goal: string, projectId?: string | null) => Promise<string | null>;
}

export const UniversalInputBar: React.FC<UniversalInputBarProps> = ({
  onProjectSelect,
  onOpenArtifact,
  onRefresh,
  onOpenFlow,
  onOpenSettings,
  projects = [],
  onStartResearch,
  onStartPlan,
}) => {
  const [inputVal, setInputVal] = useState('');
  const [currentMode, setCurrentMode] = useState<OperationalMode>('capture');
  const [catalog, setCatalog] = useState<ProviderEntry[]>([]);
  const [selectedModel, setSelectedModel] = useState<ProviderModelEntry | null>(null);
  const [isModelDropdownOpen, setIsModelDropdownOpen] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [statusMsg, setStatusMsg] = useState<string | null>(null);
  const [webSearchEnabled, setWebSearchEnabled] = useState(true);

  const modelSearchCapability = selectedModel?.web_search_capability || 'agentic_tool_call';
  const isUnsupported = modelSearchCapability === 'unsupported';
  const isEngineAugmented = modelSearchCapability === 'engine_augmented_only';

  useEffect(() => {
    if (isUnsupported) {
      setWebSearchEnabled(false);
    } else {
      setWebSearchEnabled(true);
    }
  }, [isUnsupported]);

  // Results state
  const [structuredPreview, setStructuredPreview] = useState<StructuredPlanPreview | null>(null);
  const [activePlan, setActivePlan] = useState<RouterPlan | null>(null);
  const [selectedProjectId, setSelectedProjectId] = useState<string>(projects[0]?.id || '');
  const [isAddPageDropdownOpen, setIsAddPageDropdownOpen] = useState(false);

  const {
    rawText: streamRawText,
    phases: streamPhases,
    decisions: streamDecisions,
    comparisonRows: streamComparisonRows,
    status: streamStatus,
    metrics: streamMetrics,
    basisFingerprint: streamBasisFingerprint,
    fallbackNotices: streamFallbackNotices,
    startStream,
    resetStream,
  } = useAdaptiveStream();

  const isAdaptiveStreaming = streamStatus === 'streaming';
  const hasStreamOutput =
    streamRawText.length > 0 ||
    streamPhases.length > 0 ||
    streamComparisonRows.length > 0 ||
    streamDecisions.length > 0;

  const handleLiveAdaptiveStream = async () => {
    if (!inputVal.trim() || isAdaptiveStreaming) return;
    setStructuredPreview(null);
    setStatusMsg(null);
    await startStream(inputVal.trim(), currentMode, selectedModel?.id);
  };

  const dropdownRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const modes: { mode: OperationalMode; label: string; icon: string }[] = [
    { mode: 'capture', label: 'Capture', icon: '📝' },
    { mode: 'research', label: 'Research', icon: '🔍' },
    { mode: 'plan', label: 'Plan', icon: '📋' },
    { mode: 'act', label: 'Act', icon: '⚡' },
  ];

  // Fetch catalog on mount or refresh
  const loadCatalog = async () => {
    try {
      const cat = (await getProviderCatalog()) || [];
      setCatalog(cat);
      // If no selected model or currently selected model is no longer present, pick default
      setSelectedModel((current) => {
        if (current) {
          const stillExists = cat.some((p) => p.models.some((m) => m.id === current.id && m.is_enabled));
          if (stillExists) return current;
        }
        for (const prov of cat) {
          if (!prov.is_enabled) continue;
          const def = prov.models.find((m) => m.is_default && m.is_enabled);
          if (def) return def;
        }
        for (const prov of cat) {
          if (!prov.is_enabled) continue;
          const first = prov.models.find((m) => m.is_enabled);
          if (first) return first;
        }
        return null;
      });
    } catch (err) {
      console.error('Failed to load model catalog', err);
    }
  };

  useEffect(() => {
    loadCatalog();
  }, []);

  const handleToggleModelDropdown = () => {
    if (!isModelDropdownOpen) {
      loadCatalog();
    }
    setIsModelDropdownOpen((prev) => !prev);
  };

  // Update default project selection when projects change
  useEffect(() => {
    if (projects.length > 0 && !selectedProjectId) {
      setSelectedProjectId(projects[0].id);
    }
  }, [projects, selectedProjectId]);

  // Close dropdown on outside click
  useEffect(() => {
    const handleOutsideClick = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setIsModelDropdownOpen(false);
      }
    };
    if (isModelDropdownOpen) {
      document.addEventListener('mousedown', handleOutsideClick);
    }
    return () => {
      document.removeEventListener('mousedown', handleOutsideClick);
    };
  }, [isModelDropdownOpen]);

  // Tab key cycles through modes, Enter key executes
  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Tab') {
      e.preventDefault();
      const currentIndex = modes.findIndex((m) => m.mode === currentMode);
      const nextIndex = (currentIndex + 1) % modes.length;
      setCurrentMode(modes[nextIndex].mode);
    } else if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleRoute();
    }
  };

  const handleRoute = async (e?: React.FormEvent, explicitMode?: OperationalMode) => {
    if (e) e.preventDefault();
    const trimmed = inputVal.trim();
    if (!trimmed) return;

    const modeToRun = explicitMode || currentMode;

    setIsLoading(true);
    setStatusMsg(null);
    setStructuredPreview(null);
    setActivePlan(null);

    try {
      if (modeToRun === 'capture') {
        const firstLine = trimmed.split('\n')[0].replace(/^#*\s*/, '').trim();
        const title = firstLine.length > 50 ? `${firstLine.slice(0, 50)}...` : (firstLine || 'Captured Thought');
        const content = trimmed.startsWith('#') ? trimmed : `# ${title}\n\n${trimmed}`;

        const newArt = await createPage(null, title);
        await saveArtifact(newArt.id, content, title, null);

        setInputVal('');
        if (onRefresh) onRefresh();
        if (onOpenArtifact) {
          onOpenArtifact(newArt.id, null);
        }
      } else if (modeToRun === 'research' && onStartResearch) {
        // Phase 20: Research streams the autonomous pipeline straight into a new page in
        // the full-width center workspace. Pressing Enter navigates immediately — there is
        // no intermediate "[Research in Workspace]" confirmation step.
        setStatusMsg('Researching...');
        setInputVal('');
        await onStartResearch(
          trimmed,
          webSearchEnabled && !isUnsupported ? 'auto' : 'local_vault_only',
          null
        );
        if (onRefresh) onRefresh();
      } else if (modeToRun === 'plan' && onStartPlan) {
        // Phase 20: Plan creates a `Plan: <Goal>` draft and streams tokens into it.
        setStatusMsg('Planning...');
        setInputVal('');
        await onStartPlan(trimmed, null);
        if (onRefresh) onRefresh();
      } else if (modeToRun === 'plan' || modeToRun === 'research' || modeToRun === 'act') {
        const title = trimmed.replace(/^#*\s*/, '').trim();
        const cleanTitle = title.length > 60 ? `${title.slice(0, 60)}...` : title;
        const modelName = selectedModel?.display_name || selectedModel?.model_identifier || 'Active Model';
        const actionLabel = modeToRun === 'plan' ? 'Planning' : modeToRun === 'research' ? 'Researching' : 'Executing';

        setStatusMsg(`${actionLabel} with ${modelName}...`);

        // Trigger real live adaptive LLM stream against configured/active model
        const streamedContent = await startStream(trimmed, modeToRun, selectedModel?.id);

        // Prepare rich document content using live streamed LLM tokens
        const hasStream = streamedContent && streamedContent.trim().length > 0;
        const finalContent = hasStream
          ? (streamedContent.trim().startsWith('#')
              ? streamedContent.trim()
              : `# ${cleanTitle}\n\n${streamedContent.trim()}`)
          : `# ${cleanTitle}\n\n## Vision & Scope\n${trimmed}\n\n## Synthesis & Working Notes\nGenerated with ${modelName}.\n`;

        // Create page with real LLM generated content and open in document editor
        const newArt = await createPage(null, cleanTitle);
        await saveArtifact(newArt.id, finalContent, cleanTitle, null);

        setInputVal('');
        if (onRefresh) onRefresh();
        if (onOpenArtifact) {
          onOpenArtifact(newArt.id, null);
        }
      }
    } catch (err: any) {
      console.error('Failed to execute mode action', err);
      setStatusMsg(`Error: ${err?.message || err}`);
    } finally {
      setIsLoading(false);
    }
  };

  // Execution handlers for Structured Plan actions
  const handleExecuteAction = async (action: PlanProposedAction) => {
    setIsLoading(true);
    try {
      const resId = await executePlanAction(action);
      setStatusMsg('Action executed successfully.');
      setStructuredPreview(null);
      setInputVal('');
      if (onRefresh) onRefresh();

      if (action.action_type === 'create_project' && onProjectSelect) {
        onProjectSelect(resId);
      }
    } catch (err: any) {
      setStatusMsg(`Failed to execute action: ${err?.message || err}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleCreateProjectAction = () => {
    if (!structuredPreview) return;
    const action = structuredPreview.proposed_actions.find(
      (a) => a.action_type === 'create_project'
    );
    if (action) {
      handleExecuteAction(action);
    }
  };

  const handleAddPageAction = (targetProjId?: string) => {
    if (!structuredPreview) return;
    const action = structuredPreview.proposed_actions.find(
      (a) => a.action_type === 'add_page_to_project'
    );
    if (action && action.action_type === 'add_page_to_project') {
      const projIdToUse = targetProjId || selectedProjectId || action.project_id;
      const targetProj = projects.find((p) => p.id === projIdToUse);
      const updatedAction: PlanProposedAction = {
        ...action,
        project_id: projIdToUse,
        project_name: targetProj ? targetProj.name : action.project_name,
      };
      handleExecuteAction(updatedAction);
    }
    setIsAddPageDropdownOpen(false);
  };

  const handleSaveStandaloneAction = () => {
    if (!structuredPreview) return;
    const action = structuredPreview.proposed_actions.find(
      (a) => a.action_type === 'save_standalone_note'
    );
    if (action) {
      handleExecuteAction(action);
    }
  };

  // Provider icon helper
  const getProviderIcon = (providerType?: string, providerId?: string) => {
    if (providerId === 'ollama' || providerType === 'ollama') return '🦙';
    if (providerId === 'anthropic' || providerType === 'anthropic') return '⚡';
    if (providerId === 'openrouter') return '🌐';
    if (providerId === 'groq') return '🚀';
    if (providerId === 'deepseek') return '🐋';
    if (providerId === 'openai') return '✨';
    return '☁️';
  };

  const modelChipLabel = selectedModel
    ? (() => {
        const icon = getProviderIcon(selectedModel.provider_id, selectedModel.provider_id);
        const name = selectedModel.display_name.trim();
        const ident = selectedModel.model_identifier.trim();
        if (!name || name.toLowerCase() === ident.toLowerCase()) {
          return `${icon} ${ident}`;
        }
        return `${icon} ${name}`;
      })()
    : '🦙 Ollama: llama3.2';

  return (
    <div className="bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl p-6 shadow-sm relative transition-colors">
      <div className="text-sm text-[#6E6D66] dark:text-[#9E9C93] font-medium mb-3">
        What would you like to explore or work on?
      </div>

      {/* Main Input Form */}
      <form onSubmit={handleRoute} className="flex items-center w-full relative">
        <button
          type="button"
          onClick={() => onOpenFlow && onOpenFlow('capture')}
          aria-label="Add item"
          className="text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] p-1.5 mr-2 rounded-lg hover:bg-[#EFECE4] dark:hover:bg-[#22241F] transition-colors cursor-pointer"
        >
          <Plus className="w-5 h-5" />
        </button>

        <input
          ref={inputRef}
          type="text"
          value={inputVal}
          onChange={(e) => setInputVal(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Ask, research, capture or plan... (Tab to switch modes)"
          className="w-full text-base text-[#1E201C] dark:text-[#EDEAE2] placeholder-[#6E6D66] dark:placeholder-[#73716A] bg-transparent border-none outline-none focus:ring-0 px-1 py-1"
        />

        {/* Live Adaptive Stream Button */}
        {(currentMode === 'plan' || currentMode === 'research') && (
          <button
            type="button"
            onClick={handleLiveAdaptiveStream}
            disabled={isAdaptiveStreaming || !inputVal.trim()}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 mr-2 rounded-lg text-xs font-semibold bg-[#282C25] dark:bg-[#383B32] text-white hover:bg-[#3D4238] disabled:opacity-40 transition-colors shadow-2xs shrink-0 cursor-pointer"
            title="Run adaptive token stream with live generative UI widgets"
          >
            <Sparkles className="w-3.5 h-3.5 text-[#C1DB99]" />
            <span>{isAdaptiveStreaming ? 'Streaming...' : 'Live Stream'}</span>
          </button>
        )}

        {/* Submit Button */}
        <button
          type="submit"
          disabled={isLoading || !inputVal.trim()}
          aria-label="Submit input"
          className="bg-[#282C25] dark:bg-[#383B32] text-white rounded-lg w-8 h-8 flex items-center justify-center hover:opacity-90 transition-opacity disabled:opacity-40 disabled:cursor-not-allowed shrink-0 cursor-pointer"
        >
          <ArrowUp className="w-4 h-4 stroke-[2.5]" />
        </button>
      </form>

      <div className="border-t border-[#ECE9E0] dark:border-[#2C2E27] my-4" />

      {/* Mode & Model Selection Controls */}
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
        {/* Left: Mode Selection with explicit guidance text */}
        <div className="flex items-center gap-2.5 flex-wrap">
          <span className="text-xs text-[#6E6D66] dark:text-[#9E9C93] font-medium whitespace-nowrap">
            Select a mode:
          </span>
          <div className="flex items-center gap-1.5 flex-wrap">
            {modes.map((item) => {
              const isActive = currentMode === item.mode;
              return (
                <button
                  key={item.mode}
                  type="button"
                  data-testid={`mode-btn-${item.mode}`}
                  onClick={() => {
                    setCurrentMode(item.mode);
                    inputRef.current?.focus();
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      if (inputVal.trim()) {
                        handleRoute(undefined, item.mode);
                      }
                    }
                  }}
                  className={`inline-flex items-center gap-1.5 px-3 py-1 rounded-xl text-xs font-medium transition-all cursor-pointer ${
                    isActive
                      ? 'bg-[#282C25] dark:bg-[#383B32] text-white shadow-xs'
                      : 'bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2A2C25] text-[#3A3B36] dark:text-[#D4D1C7]'
                  }`}
                >
                  <span>{item.icon}</span>
                  <span>{item.label}</span>
                </button>
              );
            })}
          </div>
        </div>

        {/* Right: Model Selection Chip & Dynamic Dropdown */}
        <div className="flex items-center gap-2 shrink-0">
          {/* Web Search Toggle & Badges */}
          <div className="flex items-center gap-1.5">
            <button
              type="button"
              data-testid="toggle-web-search"
              disabled={isUnsupported}
              onClick={() => {
                if (!isUnsupported) {
                  setWebSearchEnabled((prev) => !prev);
                }
              }}
              title={
                isUnsupported
                  ? 'Web search is disabled because the selected model is offline/unsupported.'
                  : webSearchEnabled
                  ? 'Web search is enabled'
                  : 'Web search is disabled'
              }
              className={`px-2.5 py-1 rounded-xl text-xs font-medium border flex items-center gap-1.5 transition-colors ${
                isUnsupported
                  ? 'opacity-40 cursor-not-allowed bg-stone-100 dark:bg-stone-800 border-stone-200 text-stone-400'
                  : webSearchEnabled
                  ? 'bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] border-transparent font-semibold shadow-2xs cursor-pointer'
                  : 'bg-[#EFECE4] dark:bg-[#22241F] text-[#3A3B36] dark:text-[#D4D1C7] border-[#E8E5DC] dark:border-[#2C2E27] cursor-pointer'
              }`}
            >
              <Globe className="w-3.5 h-3.5" />
              <span>Web Search</span>
            </button>

            {isEngineAugmented && (
              <span
                data-testid="web-search-engine-augmented-hint"
                title="Sthira will automatically query SearXNG/Brave and inject scraped web pages into this model's prompt."
                className="inline-flex items-center text-[#8C8A82] dark:text-[#A8A59B] hover:text-[#282C25] dark:hover:text-[#EDEAE2] cursor-help p-0.5"
              >
                <Info className="w-3.5 h-3.5" />
              </span>
            )}

            {isUnsupported && (
              <span
                data-testid="web-search-unsupported-hint"
                title="Web search is disabled because the selected model is offline/unsupported."
                className="text-[11px] text-stone-500 italic pl-0.5"
              >
                (Offline)
              </span>
            )}
          </div>

          <span className="text-xs text-[#6E6D66] dark:text-[#9E9C93] font-medium whitespace-nowrap">
            Model:
          </span>
          <div className="relative" ref={dropdownRef}>
            <button
              type="button"
              onClick={handleToggleModelDropdown}
              className="px-2.5 py-1 rounded-xl bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2A2C25] text-[#3A3B36] dark:text-[#D4D1C7] text-xs font-medium border border-[#E8E5DC] dark:border-[#2C2E27] flex items-center gap-1.5 transition-colors cursor-pointer whitespace-nowrap shadow-xs"
              title="Switch Compute Model & Provider"
            >
              <span>{modelChipLabel}</span>
              {selectedModel && (
                <WebSearchCapabilityBadge
                  capability={selectedModel.web_search_capability}
                  className="hidden sm:inline-flex"
                />
              )}
              <ChevronDown className="w-3 h-3 text-[#6E6D66] dark:text-[#9E9C93]" />
            </button>

            {/* Model Selection Dropdown Grouped by Provider */}
            {isModelDropdownOpen && (
              <div className="absolute right-0 bottom-full mb-1.5 w-80 bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl shadow-xl z-50 p-2 space-y-2 animate-in fade-in max-h-84 overflow-y-auto">
                <div className="flex items-center justify-between px-2 py-0.5">
                  <span className="text-[10px] font-bold text-[#8C8A82] uppercase tracking-wider">
                    Compute Registry
                  </span>
                  <span className="text-[10px] text-[#8C8A82]">
                    {catalog.reduce((acc, p) => acc + p.models.filter((m) => m.is_enabled).length, 0)} models ready
                  </span>
                </div>

                {catalog.map((provider) => {
                  const enabledModels = provider.models.filter((m) => m.is_enabled);
                  return (
                    <div key={provider.id} className="space-y-1">
                      <div className="text-[10px] font-bold text-[#6E6D66] uppercase px-2 pt-1 border-t border-[#ECE9E0] flex items-center justify-between">
                        <div className="flex items-center gap-1">
                          <span>{getProviderIcon(provider.provider_type, provider.id)}</span>
                          <span>{provider.display_name}</span>
                        </div>
                        <span className="text-[9px] text-[#A09D94] font-normal lowercase">
                          {enabledModels.length} active
                        </span>
                      </div>

                      <div className="space-y-0.5">
                        {enabledModels.length === 0 ? (
                          <div className="px-2.5 py-1.5 text-[11px] text-[#8C8A82] bg-[#FAF8F5] rounded-lg flex items-center justify-between">
                            <span>No models fetched</span>
                            {onOpenSettings && (
                              <button
                                type="button"
                                onClick={() => {
                                  setIsModelDropdownOpen(false);
                                  onOpenSettings();
                                }}
                                className="text-[10px] text-[#282C25] font-semibold underline hover:text-black cursor-pointer"
                              >
                                Fetch Models →
                              </button>
                            )}
                          </div>
                        ) : (
                          enabledModels.map((m) => {
                            const isSelected = selectedModel?.id === m.id;
                            return (
                              <button
                                key={m.id}
                                type="button"
                                onClick={() => {
                                  setSelectedModel(m);
                                  setIsModelDropdownOpen(false);
                                }}
                                className={`w-full text-left px-2.5 py-1.5 rounded-lg text-xs flex items-center justify-between cursor-pointer transition-colors ${
                                  isSelected
                                    ? 'bg-[#282C25] dark:bg-[#383B32] text-white font-medium'
                                    : 'hover:bg-[#EFECE4] dark:hover:bg-[#252820] text-[#1E201C] dark:text-[#EDEAE2]'
                                }`}
                              >
                                <div className="truncate pr-2">
                                  <div>{m.display_name}</div>
                                  <div
                                    className={`text-[10px] truncate font-mono ${
                                      isSelected ? 'text-zinc-300' : 'text-[#6E6D66] dark:text-[#9E9C93]'
                                    }`}
                                  >
                                    {m.model_identifier}
                                  </div>
                                </div>
                                <div className="flex items-center gap-1.5 shrink-0">
                                  <WebSearchCapabilityBadge
                                    capability={m.web_search_capability}
                                    className={isSelected ? '!bg-white/10 !border-white/20 !text-white' : ''}
                                  />
                                  {isSelected && <Check className="w-3.5 h-3.5 shrink-0 ml-1" />}
                                </div>
                              </button>
                            );
                          })
                        )}
                      </div>
                    </div>
                  );
                })}

                {/* Bottom Quick Action: Connect Providers & Fetch Models */}
                {onOpenSettings && (
                  <div className="pt-2 border-t border-[#ECE9E0] dark:border-[#2C2E27]">
                    <button
                      type="button"
                      onClick={() => {
                        setIsModelDropdownOpen(false);
                        onOpenSettings();
                      }}
                      className="w-full text-center px-2 py-1.5 rounded-lg text-xs font-medium text-[#282C25] dark:text-[#EDEAE2] bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2A2C25] flex items-center justify-center gap-1.5 transition-colors cursor-pointer"
                    >
                      <span>⚙️</span>
                      <span>Connect Providers & Fetch Models</span>
                    </button>
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Status Notice */}
      {statusMsg && (
        <div className="mt-3 text-xs text-[#2D5A27] dark:text-[#72C968] font-medium flex items-center gap-1.5 bg-[#E3EBE3] dark:bg-[#1A2E1A] border border-[#CCDCCC] dark:border-[#2B472A] p-2 rounded-xl">
          <Check className="w-3.5 h-3.5 shrink-0" />
          <span>{statusMsg}</span>
        </div>
      )}

      {/* Interactive Action Preview Card for Plan & Research Modes */}
      {structuredPreview && (
        <div className="mt-4 pt-4 border-t border-[#ECE9E0] dark:border-[#2C2E27] bg-[#FAF8F3] dark:bg-[#161714] -mx-6 -mb-6 p-6 rounded-b-2xl animate-in fade-in space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <span className="px-2 py-0.5 rounded bg-[#282C25] dark:bg-[#383B32] text-white text-[10px] font-bold uppercase tracking-wider">
                {structuredPreview.mode} Mode
              </span>
              <span className="text-xs font-semibold text-[#1E201C] dark:text-[#EDEAE2]">
                {structuredPreview.summary}
              </span>
            </div>
            <button
              type="button"
              onClick={() => setStructuredPreview(null)}
              className="text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] p-1 rounded-lg hover:bg-[#EFECE4] dark:hover:bg-[#22241F] transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          {/* Identified Phases */}
          {structuredPreview.identified_phases.length > 0 && (
            <div>
              <div className="text-[11px] font-bold uppercase tracking-wider text-[#6E6D66] dark:text-[#9E9C93] mb-1.5 flex items-center gap-1">
                <Layers className="w-3.5 h-3.5 text-[#282C25] dark:text-[#EDEAE2]" />
                Deconstructed Phases &amp; Milestones
              </div>
              <ul className="space-y-1 bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl p-3 text-xs text-[#1E201C] dark:text-[#EDEAE2]">
                {structuredPreview.identified_phases.map((phase, idx) => (
                  <li key={idx} className="flex items-start gap-2">
                    <span className="w-4 h-4 rounded-full bg-[#EFECE4] text-[#282C25] font-bold text-[10px] flex items-center justify-center shrink-0 mt-0.5">
                      {idx + 1}
                    </span>
                    <span>{phase}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {/* Open Questions */}
          {structuredPreview.open_questions.length > 0 && (
            <div>
              <div className="text-[11px] font-bold uppercase tracking-wider text-[#6E6D66] mb-1.5 flex items-center gap-1">
                <HelpCircle className="w-3.5 h-3.5 text-[#282C25]" />
                Unresolved Invariants &amp; Open Questions
              </div>
              <ul className="space-y-1 bg-[#FFFFFF] border border-[#E8E5DC] rounded-xl p-3 text-xs text-[#6E6D66]">
                {structuredPreview.open_questions.map((q, idx) => (
                  <li key={idx} className="flex items-start gap-1.5">
                    <span className="text-[#1E201C] font-semibold">•</span>
                    <span>{q}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {/* Three Concrete Action Buttons */}
          <div className="pt-2 flex flex-wrap items-center gap-2.5">
            {/* Action 1: Create as Project */}
            <button
              type="button"
              onClick={handleCreateProjectAction}
              disabled={isLoading}
              className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl bg-[#282C25] text-white text-xs font-semibold hover:opacity-90 transition-opacity cursor-pointer shadow-xs disabled:opacity-50"
            >
              <FolderPlus className="w-3.5 h-3.5" />
              <span>Create as Project</span>
            </button>

            {/* Action 2: Add Page to Project */}
            <div className="relative">
              <button
                type="button"
                onClick={() => setIsAddPageDropdownOpen((prev) => !prev)}
                disabled={isLoading}
                className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl bg-[#EFECE4] hover:bg-[#E5E1D6] text-[#1E201C] text-xs font-medium border border-[#E8E5DC] transition-colors cursor-pointer disabled:opacity-50"
              >
                <FileText className="w-3.5 h-3.5" />
                <span>
                  Add Page to Project{' '}
                  {selectedProjectId && projects.find((p) => p.id === selectedProjectId)
                    ? `(${projects.find((p) => p.id === selectedProjectId)?.name})`
                    : ''}
                </span>
                <ChevronDown className="w-3 h-3 text-[#6E6D66]" />
              </button>

              {isAddPageDropdownOpen && (
                <div className="absolute left-0 bottom-full mb-1.5 w-60 bg-[#FFFFFF] border border-[#E8E5DC] rounded-xl shadow-xl z-50 p-2 space-y-1 animate-in fade-in">
                  <div className="text-[10px] font-bold text-[#8C8A82] uppercase tracking-wider px-2 py-0.5">
                    Select Target Project
                  </div>
                  {projects.length === 0 ? (
                    <button
                      type="button"
                      onClick={() => handleAddPageAction('default-active-proj')}
                      className="w-full text-left px-2 py-1.5 rounded-lg text-xs hover:bg-[#EFECE4] text-[#1E201C]"
                    >
                      Default Active Workspace
                    </button>
                  ) : (
                    projects.map((p) => (
                      <button
                        key={p.id}
                        type="button"
                        onClick={() => {
                          setSelectedProjectId(p.id);
                          handleAddPageAction(p.id);
                        }}
                        className="w-full text-left px-2 py-1.5 rounded-lg text-xs hover:bg-[#EFECE4] text-[#1E201C] flex items-center justify-between"
                      >
                        <span className="truncate">{p.name}</span>
                        {selectedProjectId === p.id && <Check className="w-3 h-3 shrink-0" />}
                      </button>
                    ))
                  )}
                </div>
              )}
            </div>

            {/* Action 3: Save Standalone Note */}
            <button
              type="button"
              onClick={handleSaveStandaloneAction}
              disabled={isLoading}
              className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl bg-[#EFECE4] hover:bg-[#E5E1D6] text-[#3A3B36] text-xs font-medium transition-colors cursor-pointer disabled:opacity-50"
            >
              <Bookmark className="w-3.5 h-3.5" />
              <span>Save Standalone Note</span>
            </button>
          </div>
        </div>
      )}

      {/* Legacy/Act Standard Plan Display */}
      {activePlan && (
        <div className="mt-4 pt-4 border-t border-[#ECE9E0] bg-[#FAF8F3] -mx-6 -mb-6 p-6 rounded-b-2xl animate-in fade-in">
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs font-semibold uppercase tracking-wider text-[#6E6D66] flex items-center gap-1.5">
              <Sparkles className="w-3.5 h-3.5 text-[#282C25]" />
              Intent: {activePlan.intent_type.replace('_', ' ')}
            </span>
            <span className="text-xs text-[#6E6D66]">
              Confidence: {Math.round(activePlan.confidence * 100)}%
            </span>
          </div>

          <div className="text-sm text-[#1E201C] mb-3 leading-relaxed">
            {activePlan.payload?.answer || 'Synthesized plan ready.'}
          </div>

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => {
                setActivePlan(null);
                setInputVal('');
              }}
              className="px-3 py-1.5 rounded-lg bg-[#282C25] text-white text-xs font-medium hover:opacity-90 transition-opacity"
            >
              Done
            </button>
          </div>
        </div>
      )}

      {/* Live Adaptive Generative UI Stream Preview */}
      {hasStreamOutput && (
        <div className="mt-4 pt-4 border-t border-[#ECE9E0] bg-[#FAF8F3] -mx-6 -mb-6 p-6 rounded-b-2xl animate-in fade-in space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <span className="px-2 py-0.5 rounded bg-[#282C25] text-white text-[10px] font-bold uppercase tracking-wider">
                Adaptive Generative UI Stream
              </span>
              <span className="text-xs font-semibold text-[#1E201C]">
                {currentMode.toUpperCase()} STREAM
              </span>
            </div>
            <button
              type="button"
              onClick={resetStream}
              className="text-[#6E6D66] hover:text-[#1E201C] p-1 rounded-lg hover:bg-[#EFECE4] transition-colors cursor-pointer"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          <AdaptiveResponseRenderer
            rawText={streamRawText}
            phases={streamPhases}
            decisions={streamDecisions}
            comparisonRows={streamComparisonRows}
            status={streamStatus}
            metrics={streamMetrics}
            basisFingerprint={streamBasisFingerprint}
            fallbackNotices={streamFallbackNotices}
            projectId={selectedProjectId}
            onProjectCreated={(id) => {
              onRefresh?.();
              onProjectSelect?.(id);
            }}
            onOpenInEditor={async (content) => {
              const firstLine = content.split('\n')[0].replace(/^#*\s*/, '').trim();
              const pageTitle = firstLine.length > 50 ? `${firstLine.slice(0, 50)}...` : (firstLine || 'Synthesized Report');
              const newArt = await createPage(null, pageTitle);
              await saveArtifact(newArt.id, content, pageTitle, null);
              if (onRefresh) onRefresh();
              if (onOpenArtifact) onOpenArtifact(newArt.id, null);
            }}
          />
        </div>
      )}
    </div>
  );
};
