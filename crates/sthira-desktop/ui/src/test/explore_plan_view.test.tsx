import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ExplorePlanView } from '../views/ExplorePlanView';
import { App } from '../App';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
  Channel: class {
    onmessage?: (data: any) => void;
  },
}));

vi.mock('../api/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api/ipc')>();
  return {
    ...actual,
    onScheduledTopicReady: vi.fn(async () => () => {}),
  };
});

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const MOCK_RESEARCH_TOPIC = {
  id: 'topic-res-1',
  title: 'NVMe & SSD Streaming',
  prompt_query: 'Track PCIe Gen5 streaming benchmarks across Linux kernels',
  topic_type: 'research',
  project_id: 'p1',
  project_name: 'Storage Platform',
  cadence: 'daily',
  model_id: 'llama3.2',
  search_provider_id: 'ddg',
  is_active: true,
  last_run_at: 1730000000000,
  created_at: 1729000000000,
  schedule_time: '06:00',
  schedule_weekday: null,
  next_run_at: Date.now() + 3_600_000,
  schedule_label: 'Daily at 06:00 (UTC)',
  runs: [
    {
      id: 'run-1',
      artifact_id: 'art-digest-1',
      artifact_title: 'Digest: NVMe & SSD Streaming — 2026-10-30',
      sources_scraped_count: 4,
      executed_at: 1730000000000,
    },
  ],
};

const MOCK_PLAN_TOPIC = {
  id: 'topic-plan-1',
  title: 'Quarterly Systems Review',
  prompt_query: 'Assess distributed storage milestones and blockers',
  topic_type: 'plan',
  project_id: 'p1',
  project_name: 'Storage Platform',
  cadence: 'weekly',
  model_id: 'llama3.2',
  search_provider_id: null,
  is_active: true,
  last_run_at: 1730100000000,
  created_at: 1729100000000,
  schedule_time: '09:00',
  schedule_weekday: 0,
  next_run_at: Date.now() + 86_400_000,
  schedule_label: 'Weekly on Mon at 09:00 (UTC)',
  runs: [
    {
      id: 'run-2',
      artifact_id: 'art-digest-2',
      artifact_title: 'Digest: Quarterly Systems Review — 2026-10-28',
      sources_scraped_count: 0,
      executed_at: 1730100000000,
    },
  ],
};

const MOCK_NOTES = [
  {
    id: 'note-res-1',
    title: 'Research: Zero-Copy Serialization in Rust',
    entity_type: 'note',
    description: 'Status: draft — researching… Comprehensive analysis of rkyv vs flatbuffers',
    is_pinned: false,
    last_accessed_at: 1730000000000,
    updated_at: 1730000000000,
    created_at: 1729900000000,
    project_id: 'p1',
    project_name: 'Storage Platform',
    standalone: false,
    item_count: null,
    object_type_id: null,
    object_type_name: null,
    icon: 'file-text',
    color: null,
  },
  {
    id: 'note-plan-1',
    title: 'Plan: Migration to io_uring',
    entity_type: 'note',
    description: 'Status: drafting — planning… Core milestones for asynchronous engine transition',
    is_pinned: false,
    last_accessed_at: 1730100000000,
    updated_at: 1730100000000,
    created_at: 1730000000000,
    project_id: null,
    project_name: null,
    standalone: true,
    item_count: null,
    object_type_id: null,
    object_type_name: null,
    icon: 'file-text',
    color: null,
  },
];

function installMocks() {
  mockInvoke.mockImplementation(async (cmd: string, args: any) => {
    switch (cmd) {
      case 'cmd_get_home_state':
        return {
          projects: [{ id: 'p1', name: 'Storage Platform', purpose: 'IO engine', item_count: 3 }],
          recent_items: [],
          loose_captures: [],
        };
      case 'cmd_get_workspace_tree':
        return {
          projects: [{ id: 'p1', name: 'Storage Platform', pages: [], canvases: [] }],
          unassociated_pages: [],
        };
      case 'cmd_get_sidebar_items':
        return {
          projects: [{ id: 'p1', name: 'Storage Platform', is_pinned: false, last_substantive_at: 1 }],
          notes: [],
          objects: [],
        };
      case 'cmd_list_object_types':
      case 'cmd_get_object_type_counts':
        return [];
      case 'cmd_get_provider_catalog':
        return [
          {
            id: 'ollama',
            display_name: 'Ollama Local',
            provider_type: 'ollama',
            base_url: 'http://localhost:11434',
            has_api_key: false,
            is_enabled: true,
            models: [
              {
                id: 'm1',
                provider_id: 'ollama',
                model_identifier: 'llama3.2:latest',
                display_name: 'Llama 3.2 (3.2B)',
                context_window: 8192,
                supports_structured_output: true,
                is_enabled: true,
                is_default: true,
              },
            ],
          },
        ];
      case 'cmd_get_workspace_index':
        return {
          counts: { all: 2, projects: 1, notes: 2, objects: 0, standalone: 1 },
          items: MOCK_NOTES,
        };
      case 'cmd_list_scheduled_topics':
        if (args?.topicType === 'research') {
          return [MOCK_RESEARCH_TOPIC];
        }
        if (args?.topicType === 'plan') {
          return [MOCK_PLAN_TOPIC];
        }
        return [MOCK_RESEARCH_TOPIC, MOCK_PLAN_TOPIC];
      case 'cmd_create_scheduled_topic':
        return 'topic-new-1';
      case 'cmd_trigger_scheduled_topic_run':
        return 'art-digest-new-run';
      case 'cmd_toggle_scheduled_topic_active':
      case 'cmd_delete_scheduled_topic':
        return null;
      case 'cmd_create_research_page':
        return 'art-research-page-1';
      case 'cmd_create_page':
        return { id: 'art-plan-page-1', title: args?.title || 'Plan' };
      case 'cmd_save_artifact':
        return null;
      default:
        return null;
    }
  });
}

describe('🧭 Explore & Plan Hub', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    installMocks();
  });

  // ---------------------------------------------------------------------------
  // 1. Navigation & Switching Across All 4 Tabs
  // ---------------------------------------------------------------------------
  it('Step 1: LeftSidebar navigates to Explore & Plan and switches across all 4 tabs', async () => {
    const user = userEvent.setup();
    render(<App />);

    // Sidebar entry: 🧭 Explore & Plan
    const navBtn = screen.getByTestId('nav-explore-plan');
    expect(navBtn).toHaveTextContent(/Explore & Plan/i);
    await user.click(navBtn);

    // ExplorePlanView renders in the center workspace
    await waitFor(() => {
      expect(screen.getByTestId('explore-plan-view')).toBeInTheDocument();
    });

    // Check header
    expect(screen.getByRole('heading', { level: 1, name: /Explore & Plan/i })).toBeInTheDocument();

    // Verify all 4 tabs are rendered
    expect(screen.getByTestId('tab-research')).toBeInTheDocument();
    expect(screen.getByTestId('tab-plan')).toBeInTheDocument();
    expect(screen.getByTestId('tab-trackers')).toBeInTheDocument();
    expect(screen.getByTestId('tab-digests')).toBeInTheDocument();

    // Tab 1: Research active by default
    expect(screen.getByTestId('tab-content-research')).toBeInTheDocument();
    expect(screen.getByTestId('input-research-query')).toBeInTheDocument();

    // Switch to Tab 2: Plan
    await user.click(screen.getByTestId('tab-plan'));
    await waitFor(() => {
      expect(screen.getByTestId('tab-content-plan')).toBeInTheDocument();
      expect(screen.getByTestId('input-plan-goal')).toBeInTheDocument();
    });

    // Switch to Tab 3: Trackers (renders badge of active topics)
    await user.click(screen.getByTestId('tab-trackers'));
    await waitFor(() => {
      expect(screen.getByTestId('tab-content-trackers')).toBeInTheDocument();
      expect(screen.getByTestId('btn-new-tracker')).toBeInTheDocument();
    });
    // Active trackers badge should display 2
    expect(screen.getByTestId('badge-active-trackers')).toHaveTextContent('2');

    // Switch to Tab 4: All Digests
    await user.click(screen.getByTestId('tab-digests'));
    await waitFor(() => {
      expect(screen.getByTestId('tab-content-digests')).toBeInTheDocument();
      expect(screen.getByTestId('digests-table')).toBeInTheDocument();
    });
  });

  // ---------------------------------------------------------------------------
  // 2. View all research notes → switches to All Digests tab filtered by research
  // ---------------------------------------------------------------------------
  it('Step 2: Clicking "View all research notes →" switches to All Digests filtered by research', async () => {
    const user = userEvent.setup();
    render(<ExplorePlanView initialTab="research" />);

    await waitFor(() => {
      expect(screen.getByTestId('tab-content-research')).toBeInTheDocument();
    });

    // Click "View all research notes →" button
    const viewAllResearchBtn = screen.getByTestId('btn-view-all-research');
    expect(viewAllResearchBtn).toBeInTheDocument();
    await user.click(viewAllResearchBtn);

    // Switches to Tab 4: All Digests
    await waitFor(() => {
      expect(screen.getByTestId('tab-content-digests')).toBeInTheDocument();
    });

    // Filter pill for Research is active
    const researchFilterBtn = screen.getByTestId('filter-type-research');
    expect(researchFilterBtn.className).toContain('bg-[#282C25]');

    // Table renders the research items
    expect(screen.getByText('Research: Zero-Copy Serialization in Rust')).toBeInTheDocument();
  });

  // ---------------------------------------------------------------------------
  // 3. Trackers tab renders existing topics and triggers cmd_trigger_scheduled_topic_run
  // ---------------------------------------------------------------------------
  it('Step 3: Trackers tab renders existing topics and can trigger cmd_trigger_scheduled_topic_run', async () => {
    const user = userEvent.setup();
    const onOpenArtifact = vi.fn();
    const onWorkspaceMutated = vi.fn();

    render(
      <ExplorePlanView
        initialTab="trackers"
        onOpenArtifact={onOpenArtifact}
        onWorkspaceMutated={onWorkspaceMutated}
      />
    );

    // Renders research and plan trackers
    await waitFor(() => {
      expect(screen.getByTestId('topic-card-topic-res-1')).toBeInTheDocument();
      expect(screen.getByTestId('topic-card-topic-plan-1')).toBeInTheDocument();
    });

    expect(screen.getByText('NVMe & SSD Streaming')).toBeInTheDocument();
    expect(screen.getByTestId('cadence-badge-topic-res-1')).toHaveTextContent('daily');
    expect(screen.getByTestId('status-badge-topic-res-1')).toHaveTextContent('Active');

    // Click Run Now
    const runBtn = screen.getByTestId('btn-run-now-topic-res-1');
    await user.click(runBtn);

    // Verifies invocation of cmd_trigger_scheduled_topic_run
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_trigger_scheduled_topic_run',
        expect.objectContaining({ topicId: 'topic-res-1' })
      );
    });

    // Opens the generated artifact in FocusView
    await waitFor(() => {
      expect(onOpenArtifact).toHaveBeenCalledWith('art-digest-new-run', 'p1');
    });
  });

  // ---------------------------------------------------------------------------
  // 4. Research Submission Immediately Routes to FocusView
  // ---------------------------------------------------------------------------
  it('submitting research inquiry triggers onStartResearch or createResearchPage', async () => {
    const user = userEvent.setup();
    const onStartResearch = vi.fn().mockResolvedValue(null);

    render(<ExplorePlanView initialTab="research" onStartResearch={onStartResearch} />);

    await waitFor(() => {
      expect(screen.getByTestId('input-research-query')).toBeInTheDocument();
    });

    await user.type(screen.getByTestId('input-research-query'), 'Speculative Decoding Algorithms');

    // Select Web Search Enabled
    await user.click(screen.getByTestId('search-mode-always'));

    // Submit
    await user.click(screen.getByTestId('btn-start-research'));

    expect(onStartResearch).toHaveBeenCalledWith(
      'Speculative Decoding Algorithms',
      'always_search',
      null,
      'research-comparative-tech'
    );
  });

  // ---------------------------------------------------------------------------
  // 5. Template Dropdown Updates on Mode Change (Research vs Plan)
  // ---------------------------------------------------------------------------
  it('Step 5: Template dropdown updates its selectable templates when switching between Research and Plan tabs', async () => {
    const user = userEvent.setup();
    const onStartResearch = vi.fn().mockResolvedValue(null);
    const onStartPlan = vi.fn().mockResolvedValue(null);

    render(
      <ExplorePlanView
        initialTab="research"
        onStartResearch={onStartResearch}
        onStartPlan={onStartPlan}
      />
    );

    // 1. In Research tab, template selector has research options
    await waitFor(() => {
      expect(screen.getByTestId('select-synthesis-template')).toBeInTheDocument();
    });

    const researchTemplateSelect = screen.getByTestId('select-synthesis-template') as HTMLSelectElement;
    expect(researchTemplateSelect.value).toBe('research-comparative-tech');
    expect(screen.getByRole('option', { name: /Comparative Tech Evaluation/i })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: /Deep Dive Investigation/i })).toBeInTheDocument();

    // Select alternative template: Deep Dive Investigation
    await user.selectOptions(researchTemplateSelect, 'research-deep-dive');
    expect(researchTemplateSelect.value).toBe('research-deep-dive');

    // Submit research with selected alternative template
    await user.type(screen.getByTestId('input-research-query'), 'CRDT vs OT Tradeoffs');
    await user.click(screen.getByTestId('btn-start-research'));

    expect(onStartResearch).toHaveBeenCalledWith(
      'CRDT vs OT Tradeoffs',
      'auto',
      null,
      'research-deep-dive'
    );

    // 2. Switch to Plan tab
    await user.click(screen.getByTestId('tab-plan'));

    await waitFor(() => {
      expect(screen.getByTestId('tab-content-plan')).toBeInTheDocument();
    });

    const planTemplateSelect = screen.getByTestId('select-synthesis-template') as HTMLSelectElement;
    expect(planTemplateSelect.value).toBe('plan-milestone-roadmap');
    expect(screen.getByRole('option', { name: /Milestone Roadmap/i })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: /Fast Iteration Sprint/i })).toBeInTheDocument();

    // Select alternative plan template: Fast Iteration Sprint
    await user.selectOptions(planTemplateSelect, 'plan-fast-sprint');
    expect(planTemplateSelect.value).toBe('plan-fast-sprint');

    // Submit plan with selected template
    await user.type(screen.getByTestId('input-plan-goal'), 'Ship SQLite v2 schema');
    await user.click(screen.getByTestId('btn-start-plan'));

    expect(onStartPlan).toHaveBeenCalledWith(
      'Ship SQLite v2 schema',
      null,
      'plan-fast-sprint'
    );
  });

  // ---------------------------------------------------------------------------
  // 6. Trackers Creation Modal
  // ---------------------------------------------------------------------------
  it('opens New Tracker modal and dispatches cmd_create_scheduled_topic', async () => {
    const user = userEvent.setup();
    render(<ExplorePlanView initialTab="trackers" />);

    await waitFor(() => {
      expect(screen.getByTestId('btn-new-tracker')).toBeInTheDocument();
    });

    await user.click(screen.getByTestId('btn-new-tracker'));

    expect(screen.getByTestId('modal-new-tracker')).toBeInTheDocument();
    await user.type(screen.getByTestId('input-topic-title'), 'Distributed Vector Indexes');
    await user.type(screen.getByTestId('input-topic-objective'), 'Track HNSW clustering breakthroughs');

    await user.click(screen.getByTestId('btn-save-scheduled-topic'));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_create_scheduled_topic',
        expect.objectContaining({
          topic: expect.objectContaining({
            title: 'Distributed Vector Indexes',
            prompt_query: 'Track HNSW clustering breakthroughs',
          }),
        })
      );
    });
  });
});
