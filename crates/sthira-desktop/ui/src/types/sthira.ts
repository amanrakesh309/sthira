export type IntentType =
  | 'transient_qa'
  | 'capture_note'
  | 'capture_source'
  | 'start_research'
  | 'guided_project_init'
  | 'resume_project'
  | 'ambiguous_clarify';

export interface RouterPlan {
  intent_type: IntentType;
  title?: string | null;
  target_project_id?: string | null;
  confidence: number;
  payload: Record<string, any>;
}

export interface ApplyOutcome {
  success: boolean;
  created_id?: string | null;
  message: string;
}

export interface ProjectCardView {
  id: string;
  name: string;
  purpose: string;
  last_substantive_at: number;
  item_count: number;
  category: string;
  icon_type: 'brain' | 'code' | string;
}

export interface RecentItemView {
  id: string;
  title: string;
  item_type: 'Document' | 'Canvas' | string;
  subtitle: string;
  project_id?: string | null;
  artifact_id?: string | null;
  canvas_id?: string | null;
  timestamp: number;
}

export interface Artifact {
  id: string;
  artifact_type: 'note' | 'source' | 'snippet';
  authorship: 'user' | 'ai_generated' | 'external';
  title?: string | null;
  content_md: string;
  source_url?: string | null;
  content_hash: string;
  version: number;
  file_path?: string | null;
  last_synced_hash?: string | null;
  is_unfinished_draft: boolean;
  deleted_at?: number | null;
  created_at: number;
  updated_at: number;
}

export interface HomeStateView {
  projects: ProjectCardView[];
  recent_items: RecentItemView[];
  loose_captures: Artifact[];
}

export interface EpistemicNodeView {
  id: string;
  artifact_id: string;
  node_type: 'question' | 'decision' | 'finding' | 'hypothesis' | 'rejected_approach';
  origin: 'user_recorded' | 'source_backed' | 'ai_inferred';
  lifecycle: 'open' | 'resolved' | 'superseded' | 'stale';
  statement: string;
  quote?: string | null;
  quote_start?: number | null;
  quote_end?: number | null;
}

export interface ProposalView {
  id: string;
  kind: string;
  reason: string;
  source_artifact_id: string;
  target_artifact_id: string;
  target_title: string;
}

export interface ArtifactContext {
  artifact: Artifact;
  epistemic_nodes: EpistemicNodeView[];
  pending_proposals: ProposalView[];
  project_id?: string | null;
  is_unfinished_draft: boolean;
}

export type EpistemicOrigin = 'user_recorded' | 'source_backed' | 'ai_inferred';

export interface BriefClaim {
  text: string;
  origin: EpistemicOrigin;
  rendering: 'verbatim_quote' | 'deterministic_template' | 'llm_synthesis';
  evidence_node_ids: string[];
  artifact_ids: string[];
}

export type ResumePaneHint =
  | { show: { reason: 'cold_return' | 'unfinished_draft' | 'conflicting_decisions' | 'active_session' } }
  | { collapse: { reason: 'cold_return' | 'unfinished_draft' | 'conflicting_decisions' | 'active_session' } };

export interface ResumeBrief {
  project_id: string;
  level: number;
  purpose: string;
  recent_progress: BriefClaim[];
  continuation_point?: string | null;
  primary_continuation_artifact_id?: string | null;
  conflicting_decisions: BriefClaim[];
  active_working_breadcrumb: string;
  pane_hint: ResumePaneHint;
}

export interface NarrativeSegment {
  content: string;
  is_ai_interpretation: boolean;
}

export interface Milestone {
  seq: number;
  occurred_at: number;
  title: string;
  attribution: 'you_recorded' | 'supported_by_source' | 'sthira_inferred';
  milestone_type: string;
  artifact_id?: string | null;
}

export interface ProjectHistory {
  project_id: string;
  narrative_segments: NarrativeSegment[];
  milestones: Milestone[];
}

export interface CanvasItemPosition {
  item_id: string;
  x: number;
  y: number;
}

export interface CanvasItemView {
  item_id: string;
  item_kind: 'artifact' | 'text' | 'frame';
  artifact_id?: string | null;
  title?: string | null;
  excerpt?: string | null;
  x: number;
  y: number;
  w?: number | null;
  h?: number | null;
  z: number;
}

export interface ArtifactLinkView {
  source_artifact_id: string;
  target_artifact_id: string;
  relationship_type: 'supports' | 'contradicts' | 'extends' | 'references';
  origin: 'user_recorded' | 'ai_inferred';
}

export interface CanvasDataView {
  id: string;
  project_id?: string | null;
  name: string;
  items: CanvasItemView[];
  links: ArtifactLinkView[];
}

export interface CanvasOpsResponse {
  canvas?: CanvasDataView | null;
  change_set_id?: string | null;
  success: boolean;
}

export interface GuidedProjectResult {
  project_id: string;
  artifact_id: string;
}

export interface SearchResultItem {
  id: string;
  title: string;
  kind: 'project' | 'document' | string;
  excerpt: string;
  project_id?: string | null;
}

export type CoreEvent =
  | { type: 'artifact_changed'; id: string }
  | { type: 'extraction_finished'; id: string }
  | { type: 'proposals_ready'; count: number }
  | { type: 'change_set_applied'; id: string }
  | { type: 'change_set_reverted'; id: string }
  | { type: 'external_edit_detected'; id: string };

export type OperationalMode = 'capture' | 'research' | 'plan' | 'act';

export interface ProviderModelEntry {
  id: string;
  provider_id: string;
  model_identifier: string;
  display_name: string;
  context_window: number;
  supports_structured_output: boolean;
  is_enabled: boolean;
  is_default: boolean;
}

export interface ProviderEntry {
  id: string;
  display_name: string;
  provider_type: string;
  base_url: string;
  has_api_key: boolean;
  is_enabled: boolean;
  models: ProviderModelEntry[];
}

export type PlanProposedAction =
  | {
      action_type: 'create_project';
      name: string;
      purpose: string;
      initial_milestones: string[];
    }
  | {
      action_type: 'add_page_to_project';
      project_id: string;
      project_name: string;
      title: string;
      content_md: string;
    }
  | {
      action_type: 'save_standalone_note';
      title: string;
      content_md: string;
    };

export interface StructuredPlanPreview {
  mode: OperationalMode;
  provider_id: string;
  model_id: string;
  summary: string;
  identified_phases: string[];
  open_questions: string[];
  proposed_actions: PlanProposedAction[];
}

export type RouteOutcome =
  | { status: 'direct_captured'; artifact_id: string }
  | ({ status: 'structured_preview' } & StructuredPlanPreview)
  | { status: 'standard_plan'; plan: RouterPlan };

export interface WorkspaceTreePage {
  id: string;
  title: string;
  artifact_type: string;
  is_draft: boolean;
  updated_at: number;
}

export interface WorkspaceTreeCanvas {
  id: string;
  name: string;
  created_at: number;
}

export interface WorkspaceTreeProject {
  id: string;
  name: string;
  purpose: string;
  pages: WorkspaceTreePage[];
  canvases: WorkspaceTreeCanvas[];
  updated_at: number;
}

export interface WorkspaceTree {
  projects: WorkspaceTreeProject[];
  unassociated_pages: WorkspaceTreePage[];
}

export type PropertyType =
  | 'text'
  | 'number'
  | 'boolean'
  | 'select'
  | 'multiselect'
  | 'date'
  | 'url';

export interface PropertyDefinition {
  key: string;
  label: string;
  property_type: PropertyType;
  options?: string[];
  required?: boolean;
  default_value?: any;
}

export interface ObjectType {
  id: string;
  display_name: string;
  icon: string;
  color: string;
  schema: PropertyDefinition[];
  is_system: boolean;
  created_at: number;
}

export interface ObjectInstance {
  artifact_id: string;
  object_type_id: string;
  properties: Record<string, any>;
  updated_at: number;
}

export interface ObjectItem {
  artifact_id: string;
  title: string;
  content_md: string;
  object_type_id: string;
  properties: Record<string, any>;
  project_id?: string | null;
  updated_at: number;
}

export interface ArtifactSummary {
  id: string;
  title: string;
  artifact_type: string;
  updated_at: number;
}

export interface ProjectDeletionImpact {
  project_id: string;
  project_name: string;
  exclusive_artifacts: ArtifactSummary[];
  shared_artifacts: ArtifactSummary[];
}

export interface GraphNode {
  id: string;
  label: string;
  kind: 'project' | 'artifact' | 'object';
  item_count?: number;
  authorship?: string;
  draft_status?: boolean;
  object_type?: string;
  color?: string;
  icon?: string;
  project_id?: string | null;
}

export interface GraphEdge {
  source: string;
  target: string;
  relationship: string;
}

export interface KnowledgeGraphData {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export interface WebSearchResultItem {
  title: string;
  url: string;
  snippet: string;
  score?: number;
}

export interface SearchProviderEntry {
  id: string;
  display_name: string;
  provider_type: 'searxng' | 'brave' | 'duckduckgo' | 'duckduckgo_scraper' | 'tavily' | string;
  endpoint_url: string;
  api_key?: string | null;
  is_enabled: boolean;
  is_default: boolean;
}

export interface WebResearchFinding {
  query: string;
  url: string;
  domain: string;
  favicon_url: string;
  title: string;
  quote: string;
  start_offset: number;
  end_offset: number;
  markdown_excerpt: string;
  verified: boolean;
}

export interface WebResearchResult {
  original_prompt: string;
  generated_queries: string[];
  findings: WebResearchFinding[];
  sources_scraped: number;
  provider_used: string;
}

export type ProgressiveStreamChunk =
  | { type: 'text_token'; payload: string }
  | {
      type: 'plan_phase';
      payload: {
        phase_number: number;
        title: string;
        milestones: string[];
        prerequisites: string[];
      };
    }
  | {
      type: 'decision_point';
      payload: {
        id: string;
        question: string;
        options: string[];
        recommended?: string | null;
      };
    }
  | {
      type: 'comparison_row';
      payload: {
        criteria: string;
        aspect_a: string;
        aspect_b: string;
        verdict?: string | null;
      };
    }
  | {
      type: 'complete';
      payload: {
        total_tokens: number;
        duration_ms: number;
        basis_fingerprint: string;
      };
    }
  | {
      type: 'fallback_notice';
      payload: {
        reason: string;
      };
    };

export interface PlanPhase {
  phase_number: number;
  title: string;
  milestones: string[];
  prerequisites: string[];
}

export interface DecisionPoint {
  id: string;
  question: string;
  options: string[];
  recommended?: string | null;
}

export interface ComparisonRow {
  criteria: string;
  aspect_a: string;
  aspect_b: string;
  verdict?: string | null;
}

export interface StreamMetrics {
  total_tokens: number;
  duration_ms: number;
  basis_fingerprint: string;
}

export type StreamStatus = 'idle' | 'streaming' | 'completed' | 'degraded_to_markdown';

export interface SidebarItem {
  id: string;
  title: string;
  item_type: 'project' | 'note' | 'object' | string;
  is_pinned: boolean;
  last_accessed_at: number;
  updated_at: number;
  project_id?: string | null;
  project_name?: string | null;
  object_type_id?: string | null;
  object_type_name?: string | null;
  icon?: string | null;
  color?: string | null;
}

export interface SidebarItemsResponse {
  projects: SidebarItem[];
  notes: SidebarItem[];
  objects: SidebarItem[];
}

export interface WorkspaceCounts {
  all: number;
  projects: number;
  notes: number;
  objects: number;
  standalone: number;
}

export interface WorkspaceIndexItem {
  id: string;
  title: string;
  entity_type: 'project' | 'note' | 'object' | string;
  description?: string | null;
  is_pinned: boolean;
  last_accessed_at: number;
  updated_at: number;
  created_at: number;
  project_id?: string | null;
  project_name?: string | null;
  standalone: boolean;
  item_count?: number | null;
  object_type_id?: string | null;
  object_type_name?: string | null;
  icon?: string | null;
  color?: string | null;
}

export interface WorkspaceIndexData {
  counts: WorkspaceCounts;
  items: WorkspaceIndexItem[];
}

export type WorkspaceIndexTab = 'all' | 'projects' | 'notes' | 'objects' | 'standalone';



