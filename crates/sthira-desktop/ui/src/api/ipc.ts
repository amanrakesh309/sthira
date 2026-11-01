import { invoke, Channel } from '@tauri-apps/api/core';
import type {
  ApplyOutcome,
  Artifact,
  ArtifactContext,
  CanvasOpsResponse,
  GuidedProjectResult,
  HomeStateView,
  OperationalMode,
  PlanProposedAction,
  ProjectHistory,
  ProviderEntry,
  ProviderModelEntry,
  ResumeBrief,
  ResumePaneHint,
  RouteOutcome,
  RouterPlan,
  SearchResultItem,
  WorkspaceTree,
  WorkspaceTreeProject,
  ObjectType,
  ObjectTemplate,
  ObjectInstance,
  ObjectItem,
  ProjectDeletionImpact,
  KnowledgeGraphData,
  SearchProviderEntry,
  WebSearchResultItem,
  WebResearchResult,
  ProgressiveStreamChunk,
  SidebarItemsResponse,
  WorkspaceIndexData,
  WorkspaceIndexTab,
  ScheduledTopicView,
  CreateScheduledTopicInput,
  WebSearchMode,
  ResearchStage,
  ModelView,
  WebSearchCapability,
  SynthesisTemplateView,
} from '../types/sthira';

// Check if running inside Tauri runtime
export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

// Fallback demo state for standalone web browser preview (empty on initial start)
const fallbackHomeState: HomeStateView = {
  projects: [],
  recent_items: [],
  loose_captures: [],
};

const mockArtifactStore = new Map<string, { artifact: Artifact; project_id?: string | null }>();

export async function routeInput(raw: string): Promise<RouterPlan> {
  if (isTauri) {
    return await invoke<RouterPlan>('cmd_route_input', { raw });
  }

  // Realistic client fallback
  const trimmed = raw.trim();
  const lower = trimmed.toLowerCase();
  if (lower.startsWith('save this thought:') || lower.startsWith('note:') || lower.startsWith('capture:')) {
    const content = trimmed.replace(/^[^:]+:\s*/, '');
    return {
      intent_type: 'capture_note',
      title: 'Captured Thought',
      confidence: 1.0,
      payload: { content },
    };
  }
  if (lower.includes('research') || lower.startsWith('start research')) {
    return {
      intent_type: 'start_research',
      title: trimmed.replace(/^start research:?\s*/i, '') || 'New Research',
      confidence: 0.95,
      payload: { topic: trimmed },
    };
  }
  if (lower.includes('plan') || lower.includes('project')) {
    return {
      intent_type: 'guided_project_init',
      title: trimmed.replace(/^plan:?\s*/i, '') || 'New Project Plan',
      confidence: 0.9,
      payload: { goal: trimmed },
    };
  }
  return {
    intent_type: 'transient_qa',
    title: null,
    confidence: 0.85,
    payload: { question: trimmed, answer: 'Local models process requests deterministically in under 15ms.' },
  };
}

export async function applyPlan(plan: RouterPlan, approved: boolean): Promise<ApplyOutcome> {
  if (isTauri) {
    return await invoke<ApplyOutcome>('cmd_apply_plan', { plan, approved });
  }
  return {
    success: true,
    created_id: 'mock-created-id',
    message: 'Plan applied in browser preview',
  };
}

export async function getHomeState(): Promise<HomeStateView> {
  if (isTauri) {
    return await invoke<HomeStateView>('cmd_get_home_state');
  }
  return fallbackHomeState;
}

export async function getResumeBrief(projectId: string, level: number = 1): Promise<ResumeBrief> {
  if (isTauri) {
    return await invoke<ResumeBrief>('cmd_get_resume_brief', { projectId, level });
  }
  return {
    project_id: projectId,
    level,
    purpose: 'Evaluate fast local inference engines vs cloud synthesis models for knowledge extraction.',
    recent_progress: [
      {
        text: 'The 3B local model processes routing requests in under 15ms with 100% intent classification accuracy.',
        origin: 'source_backed',
        rendering: 'verbatim_quote',
        evidence_node_ids: ['node-1'],
        artifact_ids: ['art-demo-1'],
      },
      {
        text: 'We decided to allocate 16GB VRAM for local execution to ensure deterministic responsiveness.',
        origin: 'user_recorded',
        rendering: 'verbatim_quote',
        evidence_node_ids: ['node-2'],
        artifact_ids: ['art-demo-1'],
      },
    ],
    continuation_point: 'Continue research in Model performance notes',
    primary_continuation_artifact_id: 'art-demo-1',
    conflicting_decisions: [],
    active_working_breadcrumb: 'Local AI research > Active Thread',
    pane_hint: { show: { reason: 'cold_return' } },
  };
}

export async function getResumePaneHint(projectId: string): Promise<ResumePaneHint> {
  if (isTauri) {
    return await invoke<ResumePaneHint>('cmd_get_resume_pane_hint', { projectId });
  }
  return { show: { reason: 'cold_return' } };
}

export async function getProjectHistory(projectId: string): Promise<ProjectHistory> {
  if (isTauri) {
    return await invoke<ProjectHistory>('cmd_get_project_history', { projectId });
  }
  return {
    project_id: projectId,
    narrative_segments: [
      {
        content: 'You started this project to benchmark fast local inference engines against cloud models.',
        is_ai_interpretation: false,
      },
      {
        content: 'Sthira observed that local 3.2B parameter models maintain sub-15ms router latency with zero cloud leakage.',
        is_ai_interpretation: true,
      },
    ],
    milestones: [
      {
        seq: 1,
        occurred_at: Date.now() - 7200000,
        title: 'Project initiated: Local AI research',
        attribution: 'you_recorded',
        milestone_type: 'project_created',
        artifact_id: null,
      },
      {
        seq: 2,
        occurred_at: Date.now() - 3600000,
        title: 'Decision recorded: Allocate 16GB VRAM for local execution',
        attribution: 'you_recorded',
        milestone_type: 'decision_recorded',
        artifact_id: 'art-demo-1',
      },
    ],
  };
}

export async function getArtifactContext(artifactId: string): Promise<ArtifactContext> {
  if (isTauri) {
    return await invoke<ArtifactContext>('cmd_get_artifact_context', { artifactId });
  }

  const stored = mockArtifactStore.get(artifactId);
  if (stored) {
    return {
      artifact: stored.artifact,
      epistemic_nodes: [
        {
          id: 'node-1',
          artifact_id: artifactId,
          node_type: 'finding',
          origin: 'source_backed',
          lifecycle: 'open',
          statement: stored.artifact.title || 'Core Finding',
          quote: stored.artifact.content_md.slice(0, 100),
          quote_start: 0,
          quote_end: 100,
        },
      ],
      pending_proposals: [],
      project_id: stored.project_id || null,
      is_unfinished_draft: false,
    };
  }

  return {
    artifact: {
      id: artifactId,
      artifact_type: 'note',
      authorship: 'user',
      title: 'Model performance notes',
      content_md: `# Model performance notes\n\nWe evaluated local 3B parameter models against larger cloud endpoints.\n\nThe 3B local model processes routing requests in under 15ms with 100% intent classification accuracy.\nWe decided to allocate 16GB VRAM for local execution to ensure deterministic responsiveness.\n`,
      source_url: null,
      content_hash: 'hash-demo-1',
      version: 2,
      is_unfinished_draft: false,
      created_at: Date.now() - 86400000,
      updated_at: Date.now() - 1800000,
    },
    epistemic_nodes: [
      {
        id: 'node-1',
        artifact_id: artifactId,
        node_type: 'finding',
        origin: 'source_backed',
        lifecycle: 'open',
        statement: '3B local model processes routing requests in under 15ms',
        quote: 'The 3B local model processes routing requests in under 15ms with 100% intent classification accuracy.',
        quote_start: 111,
        quote_end: 209,
      },
      {
        id: 'node-2',
        artifact_id: artifactId,
        node_type: 'decision',
        origin: 'user_recorded',
        lifecycle: 'open',
        statement: 'Allocate 16GB VRAM for local execution',
        quote: 'We decided to allocate 16GB VRAM for local execution to ensure deterministic responsiveness.',
        quote_start: 210,
        quote_end: 300,
      },
    ],
    pending_proposals: [
      {
        id: 'prop-1',
        kind: 'artifact_link',
        reason: 'Both documents discuss local inference memory bandwidth constraints.',
        source_artifact_id: artifactId,
        target_artifact_id: 'art-demo-2',
        target_title: 'Quantization benchmarks',
      },
    ],
    project_id: 'proj-demo-1',
    is_unfinished_draft: false,
  };
}

export async function saveArtifact(
  artifactId: string,
  contentMd: string,
  title?: string | null,
  projectId?: string | null,
): Promise<Artifact> {
  let resolvedTitle: string | null = title || null;
  let resolvedProjectId: string | null = projectId || null;
  if (resolvedProjectId === 'default_project') {
    resolvedProjectId = null;
  }
  if (title && !projectId && (title.startsWith('proj') || title.startsWith('mock-proj'))) {
    resolvedProjectId = title;
    resolvedTitle = null;
  }

  if (isTauri) {
    try {
      return await invoke<Artifact>('cmd_save_artifact', {
        artifactId,
        artifact_id: artifactId,
        contentMd,
        content_md: contentMd,
        title: resolvedTitle,
        projectId: resolvedProjectId,
        project_id: resolvedProjectId,
      });
    } catch (err) {
      console.error('cmd_save_artifact failed:', err);
      throw err;
    }
  }

  const aid = artifactId || 'mock-art-' + Date.now();
  const existing = mockArtifactStore.get(aid);
  const derivedTitle = resolvedTitle || contentMd.split('\n')[0].replace(/^#*\s*/, '').trim() || existing?.artifact.title || 'Updated Note';
  const updatedArt: Artifact = {
    id: aid,
    artifact_type: 'note',
    authorship: 'user',
    title: derivedTitle,
    content_md: contentMd,
    source_url: null,
    content_hash: 'updated-hash',
    version: (existing?.artifact.version || 0) + 1,
    is_unfinished_draft: false,
    created_at: existing?.artifact.created_at || Date.now(),
    updated_at: Date.now(),
  };
  mockArtifactStore.set(aid, { artifact: updatedArt, project_id: resolvedProjectId !== null ? resolvedProjectId : (existing?.project_id || null) });
  return updatedArt;
}

export async function decideProposal(proposalId: string, decision: 'accept' | 'dismiss'): Promise<string> {
  if (isTauri) {
    return await invoke<string>('cmd_decide_proposal', { proposalId, decision });
  }
  return proposalId;
}

export async function revertChangeSet(changeSetId: string): Promise<string> {
  if (isTauri) {
    return await invoke<string>('cmd_revert_change_set', { changeSetId });
  }
  return 'revert-done';
}

export async function promoteToProject(artifactIds: string[], name: string): Promise<string> {
  if (isTauri) {
    return await invoke<string>('cmd_promote_to_project', { artifactIds, name });
  }
  return 'new-promoted-proj-id';
}

export async function canvasOps(req: any): Promise<CanvasOpsResponse> {
  if (isTauri) {
    return await invoke<CanvasOpsResponse>('cmd_canvas_ops', { req });
  }
  return {
    canvas: {
      id: 'canvas-demo-1',
      name: 'Ideas to investigate',
      items: [
        {
          item_id: 'c-item-1',
          item_kind: 'artifact',
          artifact_id: 'art-demo-1',
          title: 'Model performance notes',
          excerpt: 'We evaluated local 3B parameter models against larger cloud endpoints...',
          x: 120,
          y: 140,
          w: 320,
          h: 220,
          z: 1,
        },
        {
          item_id: 'c-item-2',
          item_kind: 'text',
          artifact_id: null,
          title: 'Open Questions',
          excerpt: 'Verify KV cache quantization memory savings vs perplexity hit.',
          x: 480,
          y: 140,
          w: 280,
          h: 160,
          z: 2,
        },
      ],
      links: [
        {
          source_artifact_id: 'art-demo-1',
          target_artifact_id: 'c-item-2',
          relationship_type: 'references',
          origin: 'user_recorded',
        },
      ],
    },
    change_set_id: null,
    success: true,
  };
}

export async function createCanvas(name: string, projectId?: string): Promise<string> {
  const resp = await canvasOps({
    op: 'create',
    name,
    project_id: projectId || null,
  });
  return resp?.canvas?.id || 'canvas-demo-1';
}

export async function addCanvasItem(
  canvasId: string,
  itemId: string,
  itemKind: string,
  artifactId?: string,
  x: number = 0,
  y: number = 0
): Promise<void> {
  await canvasOps({
    op: 'add_item',
    canvas_id: canvasId,
    item_id: itemId,
    item_kind: itemKind,
    artifact_id: artifactId || null,
    x,
    y,
  });
}

export async function createGuidedProject(
  name: string,
  purpose: string,
  initialTitle: string,
  initialContent: string
): Promise<GuidedProjectResult> {
  if (isTauri) {
    return await invoke<GuidedProjectResult>('cmd_create_guided_project', {
      name,
      purpose,
      initialTitle,
      initialContent,
    });
  }
  const pid = 'mock-proj-' + Date.now();
  const aid = 'mock-art-' + Date.now();
  const art: Artifact = {
    id: aid,
    artifact_type: 'note',
    authorship: 'user',
    title: initialTitle || name,
    content_md: initialContent,
    source_url: null,
    content_hash: 'mock-hash',
    version: 1,
    is_unfinished_draft: false,
    created_at: Date.now(),
    updated_at: Date.now(),
  };
  mockArtifactStore.set(aid, { artifact: art, project_id: pid });
  fallbackHomeState.projects.unshift({
    id: pid,
    name,
    purpose,
    last_substantive_at: Date.now(),
    item_count: 1,
    category: 'Active',
    icon_type: 'brain',
  });
  fallbackHomeState.recent_items.unshift({
    id: aid,
    title: initialTitle || name,
    item_type: 'Document',
    subtitle: initialContent.slice(0, 80),
    project_id: pid,
    artifact_id: aid,
    timestamp: Date.now(),
  });
  return {
    project_id: pid,
    artifact_id: aid,
  };
}

export async function clearVaultData(): Promise<void> {
  if (isTauri) {
    await invoke('cmd_clear_vault_data');
  }
}

export async function seedDemoWorkspace(): Promise<void> {
  if (isTauri) {
    await invoke('cmd_seed_demo_workspace');
  }
}

export async function searchAll(query: string): Promise<SearchResultItem[]> {
  if (isTauri) {
    return await invoke<SearchResultItem[]>('cmd_search_all', { query });
  }
  return [];
}

export async function synthesizeAndCreateResearch(
  prompt: string,
  modelId?: string | null
): Promise<GuidedProjectResult> {
  if (isTauri) {
    return await invoke<GuidedProjectResult>('cmd_synthesize_and_create_research', {
      prompt,
      modelId: modelId || null,
      model_id: modelId || null,
    });
  }
  const pid = 'mock-proj-' + Date.now();
  const aid = 'mock-art-' + Date.now();
  const title = prompt.trim();
  const cleanTitle = title.length > 60 ? `${title.slice(0, 60)}...` : title;
  const content = `# ${title}\n\n## Research Inquiry\n${prompt}\n\n## Investigation & Problem Scope\n- **Objective**: Conduct systematic investigation and structured synthesis for "${cleanTitle}".\n- **Key Dimensions**: Analyze underlying mechanisms, context, domain requirements, and relevant precedents.\n- **Evaluation Criteria**: Establish clear qualitative or quantitative criteria to validate findings.\n\n## Epistemic Hypotheses\n- [ ] Structured analysis of this inquiry yields actionable decision criteria and clear tradeoffs.\n- [ ] Edge cases and primary constraints can be systematically characterized through focused exploration.\n\n## Open Questions & Next Steps\n- What are the core assumptions and boundaries governing this topic?\n- What empirical evidence or additional context is required to reach a definitive conclusion?\n`;
  const art: Artifact = {
    id: aid,
    artifact_type: 'note',
    authorship: 'user',
    title,
    content_md: content,
    source_url: null,
    content_hash: 'mock-hash',
    version: 1,
    is_unfinished_draft: false,
    created_at: Date.now(),
    updated_at: Date.now(),
  };
  mockArtifactStore.set(aid, { artifact: art, project_id: pid });
  fallbackHomeState.projects.unshift({
    id: pid,
    name: title.length > 50 ? `${title.slice(0, 50)}...` : title,
    purpose: `Research: ${prompt}`,
    last_substantive_at: Date.now(),
    item_count: 1,
    category: 'Active',
    icon_type: 'brain',
  });
  fallbackHomeState.recent_items.unshift({
    id: aid,
    title,
    item_type: 'Document',
    subtitle: content.slice(0, 80),
    project_id: pid,
    artifact_id: aid,
    timestamp: Date.now(),
  });
  return {
    project_id: pid,
    artifact_id: aid,
  };
}

export async function answerQuestion(question: string): Promise<string> {
  if (isTauri) {
    return await invoke<string>('cmd_answer_question', { question });
  }
  return `Answer for "${question.trim()}": Connect an inference model to receive real-time answers.`;
}

export async function getProviderCatalog(): Promise<ProviderEntry[]> {
  if (isTauri) {
    return await invoke<ProviderEntry[]>('cmd_get_provider_catalog');
  }
  return [
    {
      id: 'ollama',
      display_name: 'Ollama (Local)',
      provider_type: 'ollama',
      base_url: 'http://localhost:11434',
      has_api_key: false,
      is_enabled: true,
      models: [
        {
          id: 'ollama-llama3.2',
          provider_id: 'ollama',
          model_identifier: 'llama3.2:latest',
          display_name: 'Llama 3.2 (3.2B)',
          context_window: 131072,
          supports_structured_output: true,
          is_enabled: true,
          is_default: true,
          web_search_capability: 'engine_augmented_only',
        },
      ],
    },
    {
      id: 'groq',
      display_name: 'Groq Cloud',
      provider_type: 'openai_compatible',
      base_url: 'https://api.groq.com/openai/v1',
      has_api_key: false,
      is_enabled: true,
      models: [
        {
          id: 'groq-llama-3.3-70b',
          provider_id: 'groq',
          model_identifier: 'llama-3.3-70b-versatile',
          display_name: 'Llama 3.3 (70B Versatile)',
          context_window: 131072,
          supports_structured_output: true,
          is_enabled: true,
          is_default: false,
          web_search_capability: 'agentic_tool_call',
        },
      ],
    },
    {
      id: 'anthropic',
      display_name: 'Anthropic',
      provider_type: 'anthropic',
      base_url: 'https://api.anthropic.com/v1',
      has_api_key: false,
      is_enabled: true,
      models: [
        {
          id: 'anthropic-claude-3-7-sonnet',
          provider_id: 'anthropic',
          model_identifier: 'claude-3-7-sonnet',
          display_name: 'Claude 3.7 Sonnet',
          context_window: 200000,
          supports_structured_output: true,
          is_enabled: true,
          is_default: false,
          web_search_capability: 'agentic_tool_call',
        },
      ],
    },
  ];
}

export async function listAvailableModels(): Promise<ModelView[]> {
  if (isTauri) {
    return await invoke<ModelView[]>('cmd_list_available_models');
  }
  const catalog = await getProviderCatalog();
  const models: ModelView[] = [];
  for (const p of catalog) {
    if (p.is_enabled) {
      for (const m of p.models) {
        if (m.is_enabled) {
          models.push({
            id: m.id,
            provider_id: p.id,
            provider_name: p.display_name,
            provider_type: p.provider_type,
            model_identifier: m.model_identifier,
            display_name: m.display_name,
            context_window: m.context_window,
            supports_structured_output: m.supports_structured_output,
            is_enabled: m.is_enabled,
            is_default: m.is_default,
            web_search_capability: m.web_search_capability || 'engine_augmented_only',
          });
        }
      }
    }
  }
  return models;
}

export async function updateModelSearchCapability(
  modelId: string,
  capability: WebSearchCapability
): Promise<void> {
  if (isTauri) {
    await invoke('cmd_update_model_search_capability', {
      modelId,
      capability,
    });
  }
}

export async function saveProvider(entry: ProviderEntry, apiKey?: string): Promise<void> {
  if (isTauri) {
    await invoke('cmd_save_provider', { entry, apiKey: apiKey || null });
  }
}

export async function discoverOllamaModels(endpoint?: string): Promise<ProviderModelEntry[]> {
  if (isTauri) {
    return await invoke<ProviderModelEntry[]>('cmd_discover_ollama_models', { endpoint: endpoint || null });
  }
  return [
    {
      id: 'ollama:llama3.2:latest',
      provider_id: 'ollama',
      model_identifier: 'llama3.2:latest',
      display_name: 'Llama 3.2 (3.2B)',
      context_window: 131072,
      supports_structured_output: true,
      is_enabled: true,
      is_default: true,
    },
  ];
}

export async function discoverProviderModels(
  providerId: string,
  providerType: string,
  endpoint: string,
  apiKey?: string
): Promise<ProviderModelEntry[]> {
  if (isTauri) {
    return await invoke<ProviderModelEntry[]>('cmd_discover_provider_models', {
      providerId,
      providerType,
      endpoint,
      apiKey: apiKey || null,
    });
  }
  return [
    {
      id: `${providerId}:sample-model`,
      provider_id: providerId,
      model_identifier: 'sample-model',
      display_name: 'Sample Model',
      context_window: 128000,
      supports_structured_output: true,
      is_enabled: true,
      is_default: false,
    },
  ];
}

export async function fetchProviderModels(
  providerId: string,
  endpoint?: string,
  apiKey?: string
): Promise<ProviderModelEntry[]> {
  if (isTauri) {
    return await invoke<ProviderModelEntry[]>('cmd_fetch_provider_models', {
      providerId,
      endpoint: endpoint || null,
      apiKey: apiKey || null,
    });
  }
  return [
    {
      id: `${providerId}:sample-model`,
      provider_id: providerId,
      model_identifier: 'sample-model',
      display_name: 'Sample Model',
      context_window: 128000,
      supports_structured_output: true,
      is_enabled: true,
      is_default: false,
    },
  ];
}

export async function upsertModel(model: ProviderModelEntry): Promise<void> {
  if (isTauri) {
    await invoke('cmd_upsert_model', { model });
  }
}

export async function deleteModel(modelId: string): Promise<void> {
  if (isTauri) {
    await invoke('cmd_delete_model', { modelId });
  }
}

export async function deleteProvider(providerId: string): Promise<void> {
  if (isTauri) {
    await invoke('cmd_delete_provider', { providerId });
  }
}

export async function setDefaultModel(modelId: string): Promise<void> {
  if (isTauri) {
    await invoke('cmd_set_default_model', { modelId });
  }
}

export async function toggleModel(modelId: string, enabled: boolean): Promise<void> {
  if (isTauri) {
    await invoke('cmd_toggle_model', { modelId, enabled });
  }
}

export async function routeInputSteered(
  raw: string,
  mode?: OperationalMode,
  modelId?: string,
  templateId?: string | null
): Promise<RouteOutcome> {
  if (isTauri) {
    return await invoke<RouteOutcome>('cmd_route_input_steered', {
      raw,
      mode: mode || null,
      modelId: modelId || null,
      model_id: modelId || null,
      templateId: templateId || null,
      template_id: templateId || null,
    });
  }

  // Realistic browser fallback
  const trimmed = raw.trim();
  if (mode === 'capture' || !mode) {
    return {
      status: 'direct_captured',
      artifact_id: 'mock-captured-' + Date.now(),
    };
  }

  if (mode === 'plan' || mode === 'research') {
    return {
      status: 'structured_preview',
      mode,
      provider_id: 'ollama',
      model_id: 'llama3.2:latest',
      summary: `${mode === 'plan' ? 'Execution Plan' : 'Research Investigation'}: ${trimmed}`,
      identified_phases: [
        'Phase 1: Architecture, Invariants & Data Schema',
        'Phase 2: Core Engine Implementation & Safety Boundaries',
        'Phase 3: Integration Testing & Verification',
      ],
      open_questions: [
        'What are the latency budgets and concurrency bounds?',
        'Which failure modes require deterministic rollback?',
      ],
      proposed_actions: [
        {
          action_type: 'create_project',
          name: trimmed.length > 30 ? trimmed.substring(0, 30) + '...' : trimmed,
          purpose: `Execute work for: ${trimmed}`,
          initial_milestones: [
            'Phase 1: Architecture, Invariants & Data Schema',
            'Phase 2: Core Engine Implementation & Safety Boundaries',
            'Phase 3: Integration Testing & Verification',
          ],
        },
        {
          action_type: 'add_page_to_project',
          project_id: 'default-active-proj',
          project_name: 'Active Workspace',
          title: `${trimmed.length > 30 ? trimmed.substring(0, 30) + '...' : trimmed} — Notes`,
          content_md: `# ${trimmed}\n\n## Working Notes\nInitial draft notes.`,
        },
        {
          action_type: 'save_standalone_note',
          title: `${trimmed.length > 30 ? trimmed.substring(0, 30) + '...' : trimmed} — Notes`,
          content_md: `# ${trimmed}\n\n## Notes\nCaptured note content.`,
        },
      ],
    };
  }

  const standard = await routeInput(raw);
  return {
    status: 'standard_plan',
    plan: standard,
  };
}

export async function executePlanAction(action: PlanProposedAction): Promise<string> {
  if (isTauri) {
    return await invoke<string>('cmd_execute_plan_action', { action });
  }
  return 'mock-executed-id-' + Date.now();
}

export async function getWorkspaceTree(): Promise<WorkspaceTree> {
  if (isTauri) {
    return await invoke<WorkspaceTree>('cmd_get_workspace_tree');
  }
  return {
    projects: [],
    unassociated_pages: [],
  };
}

export async function createProject(
  name: string,
  purpose?: string
): Promise<WorkspaceTreeProject> {
  if (isTauri) {
    try {
      return await invoke<WorkspaceTreeProject>('cmd_create_project', {
        name,
        purpose: purpose || null,
      });
    } catch (err) {
      console.error('cmd_create_project failed:', err);
      throw err;
    }
  }
  return {
    id: 'mock-proj-' + Date.now(),
    name,
    purpose: purpose || `Project ${name}`,
    pages: [],
    canvases: [],
    updated_at: Date.now(),
  };
}

export async function createPage(projectId?: string | null, title?: string): Promise<Artifact> {
  if (isTauri) {
    try {
      return await invoke<Artifact>('cmd_create_page', {
        projectId: projectId || null,
        title: title || null,
      });
    } catch (err) {
      console.error('cmd_create_page failed:', err);
      throw err;
    }
  }
  const t = title || 'Untitled Note';
  const aid = 'mock-page-' + Date.now();
  const art: Artifact = {
    id: aid,
    artifact_type: 'note',
    authorship: 'user',
    title: t,
    content_md: `# ${t}\n\n`,
    content_hash: 'mock-hash',
    version: 1,
    is_unfinished_draft: false,
    created_at: Date.now(),
    updated_at: Date.now(),
  };
  mockArtifactStore.set(aid, { artifact: art, project_id: projectId });
  if (!projectId) {
    fallbackHomeState.loose_captures.unshift(art);
  }
  fallbackHomeState.recent_items.unshift({
    id: aid,
    title: t,
    item_type: 'Document',
    subtitle: `# ${t}`,
    project_id: projectId || undefined,
    artifact_id: aid,
    timestamp: Date.now(),
  });
  return art;
}

export async function listObjectTypes(): Promise<ObjectType[]> {
  if (isTauri) {
    try {
      return await invoke<ObjectType[]>('cmd_list_object_types');
    } catch (err) {
      console.error('cmd_list_object_types failed:', err);
      throw err;
    }
  }
  return [
    {
      id: 'idea',
      display_name: 'Idea',
      icon: '💡',
      color: '#F59E0B',
      schema: [
        { key: 'status', label: 'Status', property_type: 'select', options: ['spark', 'exploring', 'dormant', 'validated'], required: true },
        { key: 'tags', label: 'Tags', property_type: 'text' },
      ],
      is_system: true,
      created_at: Date.now(),
    },
    {
      id: 'todo',
      display_name: 'Task',
      icon: '☑️',
      color: '#10B981',
      schema: [
        { key: 'completed', label: 'Completed', property_type: 'boolean' },
        { key: 'due_date', label: 'Due Date', property_type: 'date' },
        { key: 'priority', label: 'Priority', property_type: 'select', options: ['low', 'medium', 'high'] },
      ],
      is_system: true,
      created_at: Date.now(),
    },
    {
      id: 'decision',
      display_name: 'Decision',
      icon: '⚖️',
      color: '#6366F1',
      schema: [
        { key: 'rationale', label: 'Rationale', property_type: 'text', required: true },
        { key: 'superseded_by', label: 'Superseded By', property_type: 'text' },
      ],
      is_system: true,
      created_at: Date.now(),
    },
    {
      id: 'source',
      display_name: 'Source',
      icon: '🔗',
      color: '#3B82F6',
      schema: [
        { key: 'url', label: 'URL', property_type: 'url', required: true },
        { key: 'read_status', label: 'Read Status', property_type: 'select', options: ['unread', 'reading', 'processed'] },
      ],
      is_system: true,
      created_at: Date.now(),
    },
  ];
}

export async function createObjectType(typeDef: ObjectType): Promise<string> {
  if (isTauri) {
    try {
      return await invoke<string>('cmd_create_object_type', {
        typeDef,
        type_def: typeDef,
      });
    } catch (err) {
      console.error('cmd_create_object_type failed:', err);
      throw err;
    }
  }
  return typeDef.id;
}

export async function listObjectsByType(typeId: string): Promise<ObjectItem[]> {
  if (isTauri) {
    try {
      return await invoke<ObjectItem[]>('cmd_list_objects_by_type', { typeId });
    } catch (err) {
      console.error('cmd_list_objects_by_type failed:', err);
      throw err;
    }
  }
  return [];
}

export async function saveObjectInstance(instance: ObjectInstance): Promise<void> {
  if (isTauri) {
    try {
      await invoke<void>('cmd_save_object_instance', { instance });
    } catch (err) {
      console.error('cmd_save_object_instance failed:', err);
      throw err;
    }
  }
}

export async function getObjectTypeCounts(): Promise<Record<string, number>> {
  if (isTauri) {
    try {
      return await invoke<Record<string, number>>('cmd_get_object_type_counts');
    } catch (err) {
      console.error('cmd_get_object_type_counts failed:', err);
      throw err;
    }
  }
  return {
    idea: 0,
    todo: 0,
    decision: 0,
    source: 0,
  };
}

export async function createTypedObject(
  objectTypeId: string,
  title: string,
  properties: Record<string, any>,
  projectId?: string | null,
): Promise<ObjectItem> {
  if (isTauri) {
    try {
      return await invoke<ObjectItem>('cmd_create_typed_object', {
        objectTypeId,
        title,
        properties,
        projectId: projectId || null,
      });
    } catch (err) {
      console.error('cmd_create_typed_object failed:', err);
      throw err;
    }
  }
  return {
    artifact_id: 'mock-obj-' + Date.now(),
    title,
    content_md: `# ${title}\n\n`,
    object_type_id: objectTypeId,
    properties,
    project_id: projectId || null,
    updated_at: Date.now(),
  };
}

// ---------------------------------------------------------------------------
// Phase 16: Object Type & Template Management IPC Bridge
// ---------------------------------------------------------------------------

export async function updateObjectType(typeDef: ObjectType): Promise<void> {
  if (isTauri) {
    try {
      await invoke<void>('cmd_update_object_type', {
        typeDef,
        type_def: typeDef,
      });
    } catch (err) {
      console.error('cmd_update_object_type failed:', err);
      throw err;
    }
  }
}

export async function deleteObjectType(typeId: string): Promise<void> {
  if (isTauri) {
    try {
      await invoke<void>('cmd_delete_object_type', { typeId, type_id: typeId });
    } catch (err) {
      console.error('cmd_delete_object_type failed:', err);
      throw err;
    }
  }
}

export async function getTypeTemplates(typeId: string): Promise<ObjectTemplate[]> {
  if (isTauri) {
    try {
      return await invoke<ObjectTemplate[]>('cmd_get_type_templates', {
        typeId,
        type_id: typeId,
      });
    } catch (err) {
      console.error('cmd_get_type_templates failed:', err);
      throw err;
    }
  }
  return [];
}

export async function saveObjectTemplate(template: ObjectTemplate): Promise<string> {
  if (isTauri) {
    try {
      return await invoke<string>('cmd_save_object_template', { template });
    } catch (err) {
      console.error('cmd_save_object_template failed:', err);
      throw err;
    }
  }
  return template.id || `mock-template-${Date.now()}`;
}

export async function getObjectInstance(artifactId: string): Promise<ObjectInstance | null> {
  if (isTauri) {
    try {
      return await invoke<ObjectInstance | null>('cmd_get_object_instance', {
        artifactId,
        artifact_id: artifactId,
      });
    } catch (err) {
      console.error('cmd_get_object_instance failed:', err);
      throw err;
    }
  }
  return null;
}

export async function instantiateObjectFromTemplate(
  typeId: string,
  templateId: string | null,
  title: string,
  projectId?: string | null,
): Promise<string> {
  if (isTauri) {
    try {
      return await invoke<string>('cmd_instantiate_object_from_template', {
        typeId,
        type_id: typeId,
        templateId: templateId || null,
        template_id: templateId || null,
        title,
        projectId: projectId || null,
        project_id: projectId || null,
      });
    } catch (err) {
      console.error('cmd_instantiate_object_from_template failed:', err);
      throw err;
    }
  }
  return `mock-obj-${Date.now()}`;
}

export async function inspectProjectDeletion(projectId: string): Promise<ProjectDeletionImpact> {
  if (isTauri) {
    try {
      return await invoke<ProjectDeletionImpact>('cmd_inspect_project_deletion', {
        projectId,
        project_id: projectId,
      });
    } catch (err) {
      console.error('cmd_inspect_project_deletion failed:', err);
      throw err;
    }
  }
  return {
    project_id: projectId,
    project_name: 'Mock Project',
    exclusive_artifacts: [],
    shared_artifacts: [],
  };
}

export async function deleteProject(projectId: string, cascadeExclusive: boolean): Promise<void> {
  if (isTauri) {
    try {
      await invoke('cmd_delete_project', {
        projectId,
        project_id: projectId,
        cascadeExclusive,
        cascade_exclusive: cascadeExclusive,
      });
      return;
    } catch (err) {
      console.error('cmd_delete_project failed:', err);
      throw err;
    }
  }
}

export async function deleteArtifact(artifactId: string, fromProjectId?: string | null): Promise<void> {
  if (isTauri) {
    try {
      await invoke('cmd_delete_artifact', {
        artifactId,
        artifact_id: artifactId,
        fromProjectId: fromProjectId || null,
        from_project_id: fromProjectId || null,
      });
      return;
    } catch (err) {
      console.error('cmd_delete_artifact failed:', err);
      throw err;
    }
  }
}

export async function getKnowledgeGraph(): Promise<KnowledgeGraphData> {
  if (isTauri) {
    try {
      return await invoke<KnowledgeGraphData>('cmd_get_knowledge_graph');
    } catch (err) {
      console.error('cmd_get_knowledge_graph failed:', err);
      throw err;
    }
  }
  return {
    nodes: [],
    edges: [],
  };
}

/**
 * Deep-copies a note. Returns the **new artifact id**.
 *
 * @param newTitle Optional explicit title; defaults server-side to `"{title} (Copy)"`.
 * @param targetProjectId Optional project to attach the copy to; when omitted the copy
 *   inherits the source's active project association.
 */
export async function duplicateArtifact(
  artifactId: string,
  newTitle?: string | null,
  targetProjectId?: string | null
): Promise<string> {
  if (isTauri) {
    return await invoke<string>('cmd_duplicate_artifact', {
      artifactId,
      artifact_id: artifactId,
      newTitle: newTitle || null,
      new_title: newTitle || null,
      targetProjectId: targetProjectId || null,
      target_project_id: targetProjectId || null,
    });
  }
  return `art-dup-${Date.now()}`;
}

// ---------------------------------------------------------------------------
// Phase 17: Page Action Menu IPC Bridge
// ---------------------------------------------------------------------------

/** Links a note to a project (join-table only — zero content duplication). */
export async function linkArtifactToProject(
  artifactId: string,
  projectId: string
): Promise<void> {
  if (isTauri) {
    await invoke('cmd_link_artifact_to_project', {
      artifactId,
      artifact_id: artifactId,
      projectId,
      project_id: projectId,
    });
  }
}

/** Reassigns a note from one project to another in a single transaction. */
export async function moveArtifactToProject(
  artifactId: string,
  fromProjectId: string | null,
  toProjectId: string
): Promise<void> {
  if (isTauri) {
    await invoke('cmd_move_artifact_to_project', {
      artifactId,
      artifact_id: artifactId,
      fromProjectId: fromProjectId || null,
      from_project_id: fromProjectId || null,
      toProjectId,
      to_project_id: toProjectId,
    });
  }
}

/** Creates a new project seeded with this note. Returns the new project id. */
export async function createProjectWithArtifact(
  projectName: string,
  artifactId: string
): Promise<string> {
  if (isTauri) {
    return await invoke<string>('cmd_create_project_with_artifact', {
      projectName,
      project_name: projectName,
      artifactId,
      artifact_id: artifactId,
    });
  }
  return `proj-${Date.now()}`;
}

/**
 * Registers an existing note as an instance of an object type. The note's Markdown body
 * is preserved byte-for-byte; only an `object_instances` row is written.
 */
export async function convertArtifactToObject(
  artifactId: string,
  objectTypeId: string,
  initialProperties?: Record<string, any> | null
): Promise<void> {
  if (isTauri) {
    await invoke('cmd_convert_artifact_to_object', {
      artifactId,
      artifact_id: artifactId,
      objectTypeId,
      object_type_id: objectTypeId,
      initialProperties: initialProperties || null,
      initial_properties: initialProperties || null,
    });
  }
}

/** Detaches a note from a project, returning it to the standalone pool. */
export async function detachArtifactFromProject(
  artifactId: string,
  projectId: string
): Promise<void> {
  if (isTauri) {
    await invoke('cmd_detach_artifact_from_project', {
      artifactId,
      artifact_id: artifactId,
      projectId,
      project_id: projectId,
    });
  }
}

export async function addArtifactToProject(
  artifactId: string,
  projectId: string
): Promise<void> {
  if (isTauri) {
    await invoke('cmd_add_artifact_to_project', {
      artifactId,
      artifact_id: artifactId,
      projectId,
      project_id: projectId,
    });
  }
}

export async function duplicateProject(
  projectId: string
): Promise<WorkspaceTreeProject> {
  if (isTauri) {
    return await invoke<WorkspaceTreeProject>('cmd_duplicate_project', {
      projectId,
      project_id: projectId,
    });
  }
  return {
    id: `proj-dup-${Date.now()}`,
    name: 'Copy of Project',
    purpose: '',
    pages: [],
    canvases: [],
    updated_at: Date.now(),
  };
}

export async function renameProject(
  projectId: string,
  newName: string
): Promise<void> {
  if (isTauri) {
    await invoke('cmd_rename_project', {
      projectId,
      project_id: projectId,
      newName,
      new_name: newName,
    });
  }
}

export async function renameArtifact(
  artifactId: string,
  newTitle: string
): Promise<void> {
  if (isTauri) {
    await invoke('cmd_rename_artifact', {
      artifactId,
      artifact_id: artifactId,
      newTitle,
      new_title: newTitle,
    });
  }
}

// ============================================================================
// Phase 10: Pluggable Web Search Providers & Evidence Extraction
// ============================================================================

let mockSearchProviders: SearchProviderEntry[] = [
  {
    id: 'duckduckgo',
    display_name: 'DuckDuckGo (Headless HTML)',
    provider_type: 'duckduckgo_scraper',
    endpoint_url: 'https://html.duckduckgo.com/html/',
    api_key: null,
    is_enabled: true,
    is_default: true,
  },
  {
    id: 'searxng',
    display_name: 'SearXNG (Self-Hosted / Local)',
    provider_type: 'searxng',
    endpoint_url: 'http://localhost:8080',
    api_key: null,
    is_enabled: false,
    is_default: false,
  },
  {
    id: 'brave',
    display_name: 'Brave Search API',
    provider_type: 'brave',
    endpoint_url: 'https://api.search.brave.com/res/v1/web/search',
    api_key: null,
    is_enabled: false,
    is_default: false,
  },
  {
    id: 'tavily',
    display_name: 'Tavily AI Search',
    provider_type: 'tavily',
    endpoint_url: 'https://api.tavily.com/search',
    api_key: null,
    is_enabled: false,
    is_default: false,
  },
];

export async function getSearchProviders(): Promise<SearchProviderEntry[]> {
  if (isTauri) {
    return await invoke<SearchProviderEntry[]>('cmd_get_search_providers');
  }
  return [...mockSearchProviders];
}

export async function saveSearchProvider(
  entry: SearchProviderEntry
): Promise<void> {
  if (isTauri) {
    await invoke('cmd_save_search_provider', {
      entry,
    });
    return;
  }
  const idx = mockSearchProviders.findIndex((p) => p.id === entry.id);
  if (idx >= 0) {
    if (entry.is_default) {
      mockSearchProviders = mockSearchProviders.map((p) => ({
        ...p,
        is_default: p.id === entry.id,
      }));
    }
    mockSearchProviders[idx] = { ...entry };
  } else {
    if (entry.is_default) {
      mockSearchProviders = mockSearchProviders.map((p) => ({
        ...p,
        is_default: false,
      }));
    }
    mockSearchProviders.push(entry);
  }
}

export async function testSearchProvider(
  providerId: string,
  query: string
): Promise<WebSearchResultItem[]> {
  if (isTauri) {
    return await invoke<WebSearchResultItem[]>('cmd_test_search_provider', {
      providerId,
      provider_id: providerId,
      query,
    });
  }

  // Realistic fallback demo items
  return [
    {
      title: `${query} - Official Architectural Reference & Standard`,
      url: `https://docs.rs/${encodeURIComponent(query)}/latest/reference`,
      snippet: `Comprehensive specification for ${query} with invariants, memory bounds, and zero-overhead trait dispatch patterns.`,
      score: 0.95,
    },
    {
      title: `Understanding ${query} in Modern Systems Engineering`,
      url: `https://example.org/guides/${encodeURIComponent(query)}`,
      snippet: `An in-depth analysis of ${query} performance implications, deterministic execution models, and cache efficiency.`,
      score: 0.88,
    },
    {
      title: `${query}: Implementation Patterns & RFC Discussions`,
      url: `https://github.com/rust-lang/rfcs/issues/${encodeURIComponent(query)}`,
      snippet: `Technical discussion regarding async execution boundaries and multi-query research aggregation strategies.`,
      score: 0.81,
    },
  ];
}

export async function importWebSourceToVault(
  url: string,
  title: string,
  contentMd: string,
  projectId?: string | null
): Promise<string> {
  if (isTauri) {
    return await invoke<string>('cmd_import_web_source_to_vault', {
      url,
      title,
      contentMd,
      content_md: contentMd,
      projectId: projectId || null,
      project_id: projectId || null,
    });
  }

  const id = `art-src-${Date.now()}`;
  const now = Date.now();
  const mockArt: Artifact = {
    id,
    artifact_type: 'source',
    authorship: 'external',
    title: title || 'Imported Web Source',
    content_md: contentMd,
    source_url: url,
    content_hash: `hash-${id}`,
    version: 1,
    file_path: `sources/${id}.md`,
    last_synced_hash: `hash-${id}`,
    is_unfinished_draft: false,
    created_at: now,
    updated_at: now,
  };
  mockArtifactStore.set(id, { artifact: mockArt, project_id: projectId || null });
  return id;
}

export async function executeWebResearch(
  query: string,
  modelId?: string | null
): Promise<WebResearchResult> {
  if (isTauri) {
    return await invoke<WebResearchResult>('cmd_execute_web_research', {
      query,
      modelId: modelId || null,
      model_id: modelId || null,
    });
  }

  // Browser-preview fallback mirroring the Rust `WebResearchResult` contract.
  const quote = `Asynchronous research engines formulate deterministic query branches to verify external citations without altering active workspace files.`;
  const url = `https://docs.rs/spec/${encodeURIComponent(query)}`;
  const favicon = 'https://www.google.com/s2/favicons?domain=docs.rs&sz=32';
  return {
    query,
    search_queries_used: [
      `${query} technical specification`,
      `${query} rust architecture`,
      `${query} invariants`,
    ],
    documents: [
      {
        url,
        title: `${query} Architecture & Safety Specifications`,
        content_md: `# ${query} Reference\n\n${quote}\n`,
        published_date: null,
      },
    ],
    findings: [
      {
        statement: quote,
        quote,
        source_url: url,
        domain: 'docs.rs',
        title: `${query} Architecture & Safety Specifications`,
        favicon_url: favicon,
        quote_start: 0,
        quote_end: quote.length,
      },
    ],
    synthesized_markdown: `# ${query}\n\n## Synthesis\n${quote}\n\n## Contradictions & Evolution\n_None detected._\n`,
    mode_used: 'always_search' as const,
    stages: [
      { stage: 'searching', message: `Searching web for "${query}" via DuckDuckGo...` },
      { stage: 'scraped', message: 'Scraped 1 source...' },
      { stage: 'synthesizing', message: 'Synthesizing research...' },
      { stage: 'done', message: 'Research complete.' },
    ],
    sources: [
      { index: 1, url, domain: 'docs.rs', title: `${query} Architecture & Safety Specifications`, favicon_url: favicon },
    ],
  };
}

export async function streamAdaptiveRun(
  rawInput: string,
  mode: OperationalMode,
  modelId: string | null | undefined,
  onChunk: (chunk: ProgressiveStreamChunk) => void
): Promise<void> {
  if (isTauri) {
    const channel = new Channel<ProgressiveStreamChunk>();
    channel.onmessage = (message) => {
      onChunk(message);
    };
    await invoke('cmd_stream_adaptive_run', {
      rawInput,
      raw_input: rawInput,
      mode,
      modelId: modelId || null,
      model_id: modelId || null,
      onChunk: channel,
      on_chunk: channel,
    });
    return;
  }

  // Fallback simulator for browser preview and vitest
  const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
  const trimmed = rawInput.trim();

  if (mode === 'plan') {
    onChunk({ type: 'text_token', payload: '### Deconstructed Execution Plan\n\n' });
    await delay(20);
    onChunk({
      type: 'text_token',
      payload: `Analyzing objectives and architectural constraints for: **${trimmed}**\n\n`,
    });
    await delay(20);

    onChunk({
      type: 'plan_phase',
      payload: {
        phase_number: 1,
        title: 'Phase 1: Architecture, Invariants & Schemas',
        milestones: [
          'Formalize immutable data contracts and DB migrations',
          'Define strict zero-unsolicited-writes invariants',
          'Establish testkit mock models and harness',
        ],
        prerequisites: [],
      },
    });
    await delay(20);

    onChunk({
      type: 'text_token',
      payload: 'System boundaries and core traits are stabilized before progressive components are wired.\n\n',
    });
    await delay(20);

    onChunk({
      type: 'plan_phase',
      payload: {
        phase_number: 2,
        title: 'Phase 2: Progressive Streaming Engine & Parser',
        milestones: [
          'Implement fail-safe token buffer parser',
          'Wire tokio MPSC channel to IPC webview stream',
          'Add automated malformed JSON fallback verification',
        ],
        prerequisites: ['Phase 1'],
      },
    });
    await delay(20);

    onChunk({
      type: 'decision_point',
      payload: {
        id: 'streaming_transport',
        question: 'Which streaming IPC transport should bridge Rust and React UI?',
        options: [
          'Tauri v2 Typed IPC Channel (zero websocket overhead)',
          'Local WebSocket server (port 8765)',
          'Server-Sent Events (SSE) via localhost HTTP',
        ],
        recommended: 'Tauri v2 Typed IPC Channel (zero websocket overhead)',
      },
    });
    await delay(20);

    onChunk({
      type: 'plan_phase',
      payload: {
        phase_number: 3,
        title: 'Phase 3: Interactive Generative UI Widgets & Reversibility',
        milestones: [
          'Mount MilestonePlanDagWidget with 1-click project creation',
          'Mount EvidenceComparisonMatrixWidget with Warm Stone theme',
          'Bind global UndoToast for atomic change_set reversibility',
        ],
        prerequisites: ['Phase 1', 'Phase 2'],
      },
    });
    await delay(20);

    onChunk({
      type: 'text_token',
      payload: 'All proposed milestones remain interactive and can be promoted directly into your workspace.\n',
    });
    await delay(20);

    onChunk({
      type: 'complete',
      payload: {
        total_tokens: 38,
        duration_ms: 240,
        basis_fingerprint: 'a1b2c3d4e5f60718293a4b5c6d7e8f90',
      },
    });
  } else if (mode === 'research') {
    onChunk({ type: 'text_token', payload: '### Synthesized Research & Comparative Evidence\n\n' });
    await delay(20);
    onChunk({
      type: 'text_token',
      payload: `Investigating technical specifications and trade-offs for: *${trimmed}*\n\n`,
    });
    await delay(20);

    onChunk({
      type: 'comparison_row',
      payload: {
        criteria: 'IPC Latency & Serialization',
        aspect_a: 'Tauri v2 Channel (<0.5ms directly in-process)',
        aspect_b: 'WebSocket HTTP loopback (2.8ms - 5.0ms)',
        verdict: 'Tauri v2 Channel eliminates port conflicts and network socket allocation',
      },
    });
    await delay(20);

    onChunk({
      type: 'comparison_row',
      payload: {
        criteria: 'Failure Recovery (Zero-Broken-JSON)',
        aspect_a: 'Fail-safe adaptive parser (auto-degrades to Markdown)',
        aspect_b: 'Strict JSON parser (throws syntax error on partial tokens)',
        verdict: 'Fail-safe parser prevents layout flashing and unhandled UI exceptions',
      },
    });
    await delay(20);

    onChunk({
      type: 'comparison_row',
      payload: {
        criteria: 'Provenance & Verification',
        aspect_a: 'Grounding with VerifiedQuote::verify() against scraped web text',
        aspect_b: 'Unverified LLM confidence scores',
        verdict: 'Verified quotes enforce absolute fidelity against source artifacts',
      },
    });
    await delay(20);

    onChunk({
      type: 'text_token',
      payload: 'Citations verified against local vault records and active search provider engines.\n',
    });
    await delay(20);

    onChunk({
      type: 'complete',
      payload: {
        total_tokens: 42,
        duration_ms: 270,
        basis_fingerprint: 'f0e1d2c3b4a5968778695a4b3c2d1e0f',
      },
    });
  } else {
    onChunk({
      type: 'text_token',
      payload: `### Synthesis Deliverable\n\n*Inquiry: ${trimmed}*\n\n`,
    });
    await delay(20);
    onChunk({
      type: 'text_token',
      payload:
        'Sthira processes incoming streams adaptively, matching model locality and task structure.\n\n',
    });
    await delay(20);
    onChunk({
      type: 'text_token',
      payload:
        '- [x] Streaming buffer tracks token metrics with zero UI jitter.\n- [x] Zero unsolicited writes invariant preserved.\n- [x] Reversible changesets generated for user actions.\n',
    });
    await delay(20);
    onChunk({
      type: 'complete',
      payload: {
        total_tokens: 28,
        duration_ms: 180,
        basis_fingerprint: '3b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e',
      },
    });
  }
}

export async function getSidebarItems(): Promise<SidebarItemsResponse> {
  if (isTauri) {
    return await invoke<SidebarItemsResponse>('cmd_get_sidebar_items');
  }
  return {
    projects: [],
    notes: [],
    objects: [],
  };
}

export async function togglePin(itemId: string, itemType: string, isPinned: boolean): Promise<void> {
  if (isTauri) {
    await invoke('cmd_toggle_pin', { itemId, itemType, isPinned });
    return;
  }
}

export async function recordAccess(itemId: string, itemType: string): Promise<void> {
  if (isTauri) {
    await invoke('cmd_record_access', { itemId, itemType });
    return;
  }
}

export async function getWorkspaceIndex(tab?: WorkspaceIndexTab, search?: string): Promise<WorkspaceIndexData> {
  if (isTauri) {
    return await invoke<WorkspaceIndexData>('cmd_get_workspace_index', {
      tab: tab || 'all',
      search: search || null,
    });
  }
  return {
    counts: { all: 0, projects: 0, notes: 0, objects: 0, standalone: 0 },
    items: [],
  };
}





// ---------------------------------------------------------------------------
// Phase 18: Continuous & Scheduled Research / Planning Topics
// ---------------------------------------------------------------------------

export async function listScheduledTopics(
  topicType?: 'research' | 'plan' | null
): Promise<ScheduledTopicView[]> {
  if (isTauri) {
    return await invoke<ScheduledTopicView[]>('cmd_list_scheduled_topics', {
      topicType: topicType || null,
      topic_type: topicType || null,
    });
  }
  return [];
}

export async function createScheduledTopic(
  topic: CreateScheduledTopicInput
): Promise<string> {
  if (isTauri) {
    return await invoke<string>('cmd_create_scheduled_topic', { topic });
  }
  return `topic-${Date.now()}`;
}

/** Runs a topic immediately; resolves to the id of the generated artifact. */
export async function triggerScheduledTopicRun(topicId: string): Promise<string> {
  if (isTauri) {
    return await invoke<string>('cmd_trigger_scheduled_topic_run', {
      topicId,
      topic_id: topicId,
    });
  }
  return `art-digest-${Date.now()}`;
}

export async function toggleScheduledTopicActive(
  topicId: string,
  isActive: boolean
): Promise<void> {
  if (isTauri) {
    await invoke('cmd_toggle_scheduled_topic_active', {
      topicId,
      topic_id: topicId,
      isActive,
      is_active: isActive,
    });
  }
}

export async function deleteScheduledTopic(topicId: string): Promise<void> {
  if (isTauri) {
    await invoke('cmd_delete_scheduled_topic', { topicId, topic_id: topicId });
  }
}

/**
 * Subscribes to autonomous-run completions pushed by the desktop scheduler.
 * Returns an unlisten function.
 */
export async function onScheduledTopicReady(
  handler: (event: import('../types/sthira').ScheduledTopicReadyEvent) => void
): Promise<() => void> {
  if (!isTauri) return () => {};
  const { listen } = await import('@tauri-apps/api/event');
  const unlisten = await listen<import('../types/sthira').ScheduledTopicReadyEvent>(
    'sthira://scheduled-topic-ready',
    (e) => handler(e.payload)
  );
  return unlisten;
}


// ---------------------------------------------------------------------------
// Phase 19: Center-Workspace Research Pipeline
// ---------------------------------------------------------------------------

/**
 * Creates the draft research page that will host a run, returning its `artifact_id`.
 *
 * The caller navigates to this id immediately so the run is watched in the **center
 * workspace** rather than trapped in the right rail.
 */
export async function createResearchPage(
  query: string,
  projectId?: string | null
): Promise<string> {
  if (isTauri) {
    return await invoke<string>('cmd_create_research_page', {
      query,
      projectId: projectId || null,
      project_id: projectId || null,
    });
  }
  return `art-research-${Date.now()}`;
}

/**
 * Runs the autonomous research pipeline into an existing page, streaming **real**
 * progress stages so `FocusView` can render a live banner.
 *
 * No URL is required — the pipeline generates its own search queries.
 */
export async function runResearchIntoPage(
  artifactId: string,
  query: string,
  webSearchMode: WebSearchMode,
  onStage?: (stage: ResearchStage) => void,
  templateId?: string | null
): Promise<WebResearchResult> {
  if (isTauri) {
    const channel = new Channel<ResearchStage>();
    channel.onmessage = (message) => {
      onStage?.(message);
    };
    return await invoke<WebResearchResult>('cmd_run_research_into_page', {
      artifactId,
      artifact_id: artifactId,
      query,
      webSearchMode,
      web_search_mode: webSearchMode,
      templateId: templateId || null,
      template_id: templateId || null,
      onProgress: channel,
      on_progress: channel,
    });
  }

  // Browser-preview fallback: replay the same stage narrative locally.
  onStage?.({ stage: 'searching', message: `Searching web for "${query}" via DuckDuckGo...` });
  onStage?.({ stage: 'scraped', message: 'Scraped 3 sources...' });
  onStage?.({ stage: 'synthesizing', message: 'Synthesizing research...' });
  return {
    query,
    search_queries_used: [`${query} overview`],
    documents: [],
    findings: [],
    synthesized_markdown: `# ${query}\n\n## Synthesis\nPreview mode — no live pipeline available.\n\n## Contradictions & Evolution\n_None detected._\n`,
    mode_used: webSearchMode,
    stages: [],
    sources: [],
  };
}

export async function refineArtifact(
  artifactId: string,
  critique: string,
  modelId?: string | null,
  webSearch: boolean = false
): Promise<string> {
  if (isTauri) {
    return await invoke<string>('cmd_refine_artifact', {
      artifactId,
      artifact_id: artifactId,
      critique,
      modelId: modelId || null,
      model_id: modelId || null,
      webSearch,
      web_search: webSearch,
    });
  }

  return `# Refined Note\n\n> Critique: ${critique}\n\nEnhanced synthesis in preview mode.`;
}

export async function deriveArtifact(
  sourceArtifactId: string,
  derivationType: string,
  templateId?: string | null,
  modelId?: string | null
): Promise<string> {
  if (isTauri) {
    return await invoke<string>('cmd_derive_artifact', {
      sourceArtifactId,
      source_artifact_id: sourceArtifactId,
      derivationType,
      derivation_type: derivationType,
      templateId: templateId || null,
      template_id: templateId || null,
      modelId: modelId || null,
      model_id: modelId || null,
    });
  }

  return 'derived-preview-art-' + Date.now();
}

export async function listSynthesisTemplates(
  targetMode?: string
): Promise<SynthesisTemplateView[]> {
  if (isTauri) {
    return await invoke<SynthesisTemplateView[]>('cmd_list_synthesis_templates', {
      targetMode: targetMode || null,
      target_mode: targetMode || null,
    });
  }

  // Preview fallbacks for browser testing
  const allTemplates: SynthesisTemplateView[] = [
    {
      id: 'research-comparative-tech',
      name: 'Comparative Tech Evaluation',
      target_mode: 'research',
      description: 'Systematic architectural comparison matrix evaluating tradeoffs, throughput, memory bounds, and operational invariants.',
      is_default: true,
      template_skeleton_md: `## Executive Summary & Recommendation\n\n## Contenders & Key Criteria\n\n## Comparison Matrix\n| Criteria | Option A | Option B |\n|---|---|---|\n| Architecture | | |\n| Throughput / Latency | | |\n| Operational Invariants | | |\n\n## Grounded Sources & Verifications\n\n## Final Recommendation\n`,
      created_at: 1700000000000,
    },
    {
      id: 'research-deep-dive',
      name: 'Deep Dive Investigation',
      target_mode: 'research',
      description: 'Exhaustive thematic breakdown with historical context and contradictions.',
      is_default: false,
      template_skeleton_md: `## Problem Formulation\n\n## In-Depth Analysis\n\n## Contradictions & Historical Evolution\n\n## Empirical Findings\n\n## Conclusion & Open Questions\n`,
      created_at: 1700000000000,
    },
    {
      id: 'plan-milestone-roadmap',
      name: 'Milestone Roadmap',
      target_mode: 'plan',
      description: 'Phased milestone breakdown with explicit risk boundaries and action items.',
      is_default: true,
      template_skeleton_md: `## Scope & Invariants\n\n## Phase 1: Foundation & Data Contracts\n- [ ] \n\n## Phase 2: Engine Implementation & Safety Bounds\n- [ ] \n\n## Phase 3: Integration & Verifications\n- [ ] \n\n## Open Decisions & Risks\n`,
      created_at: 1700000000000,
    },
    {
      id: 'plan-fast-sprint',
      name: 'Fast Iteration Sprint',
      target_mode: 'plan',
      description: 'Lean 2-stage execution checklist designed for rapid prototypes and tactical fixes.',
      is_default: false,
      template_skeleton_md: `## Objective & Non-Goals\n\n## Stage 1: Quick Wins & Prototyping\n- [ ] \n\n## Stage 2: Hardening & Sanity Checks\n- [ ] \n\n## Verification Criteria\n`,
      created_at: 1700000000000,
    },
  ];

  if (!targetMode) {
    return allTemplates;
  }
  return allTemplates.filter((t) => t.target_mode === targetMode);
}

