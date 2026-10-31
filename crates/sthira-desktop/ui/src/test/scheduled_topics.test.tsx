import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ScheduledTopicsList } from '../components/ScheduledTopicsList';
import { ScheduledTopicsView } from '../views/ScheduledTopicsView';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
  Channel: class {},
}));
// The scheduler push subscription opens a real Tauri event channel. `setup.ts` makes
// `isTauri` truthy, so the real `listen()` would reject with an unhandled error under
// jsdom — stub the subscription itself rather than the underlying event bus.
vi.mock('../api/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api/ipc')>();
  return {
    ...actual,
    onScheduledTopicReady: vi.fn(async () => () => {}),
  };
});
// The panel subscribes to scheduler pushes; jsdom has no Tauri event bus.
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const RESEARCH_TOPIC = {
  id: 't1',
  title: 'SSD Streaming & NVMe',
  prompt_query: 'Track PCIe Gen5 streaming benchmarks',
  topic_type: 'research',
  project_id: null,
  project_name: null,
  cadence: 'daily',
  model_id: null,
  search_provider_id: null,
  is_active: true,
  last_run_at: null,
  created_at: 1000,
  schedule_time: '06:00',
  schedule_weekday: null,
  next_run_at: Date.now() + 3_600_000,
  schedule_label: 'Daily at 06:00 (UTC)',
  runs: [
    {
      id: 'r1',
      artifact_id: 'art-digest-1',
      artifact_title: 'Digest: SSD Streaming & NVMe — 2026-10-30',
      sources_scraped_count: 3,
      executed_at: 2000,
    },
  ],
};

function installDefaultMocks(listResponses: any[] = []) {
  let listCall = 0;
  mockInvoke.mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case 'cmd_list_scheduled_topics': {
        const resp = listResponses[Math.min(listCall, listResponses.length - 1)];
        listCall += 1;
        return resp ?? [];
      }
      case 'cmd_get_workspace_tree':
        return {
          projects: [{ id: 'p1', name: 'Storage Platform', pages: [], canvases: [] }],
          unassociated_pages: [],
        };
      case 'cmd_get_provider_catalog':
        return [
          {
            id: 'ollama',
            display_name: 'Ollama',
            provider_type: 'ollama',
            base_url: '',
            has_api_key: false,
            is_enabled: true,
            models: [
              {
                id: 'm1',
                provider_id: 'ollama',
                model_identifier: 'llama3.2',
                display_name: 'Llama 3.2',
                context_window: 8192,
                supports_structured_output: true,
                is_enabled: true,
                is_default: true,
              },
            ],
          },
        ];
      case 'cmd_create_scheduled_topic':
        return 't-new';
      case 'cmd_trigger_scheduled_topic_run':
        return 'art-digest-new';
      case 'cmd_toggle_scheduled_topic_active':
      case 'cmd_delete_scheduled_topic':
        return null;
      default:
        return null;
    }
  });
  return () => listCall;
}

describe('Phase 18: Continuous & Scheduled Research / Planning Topics', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Test 1: ScheduledTopicsView hosts BOTH research and plan feeds in the
  //         full-width center workspace (relocated out of the removed rail).
  // -------------------------------------------------------------------------
  it('Test 1: ScheduledTopicsView renders research and plan feeds in the center workspace', async () => {
    installDefaultMocks([[]]);

    render(<ScheduledTopicsView onOpenArtifact={vi.fn()} onWorkspaceMutated={vi.fn()} />);

    expect(screen.getByText(/⏱️ Scheduled Feeds/i)).toBeInTheDocument();
    expect(screen.getByTestId('scheduled-section-research')).toBeInTheDocument();
    expect(screen.getByTestId('scheduled-section-plan')).toBeInTheDocument();

    // Both feed types are queried on mount.
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_list_scheduled_topics',
        expect.objectContaining({ topicType: 'research' })
      );
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_list_scheduled_topics',
        expect.objectContaining({ topicType: 'plan' })
      );
    });
  });

  // -------------------------------------------------------------------------
  // Test 2: the dashboard exposes the creation form (cadence + time controls)
  // -------------------------------------------------------------------------
  it('Test 2: ScheduledTopicsView exposes the add-feed form with interval controls', async () => {
    installDefaultMocks([[], []]);

    render(<ScheduledTopicsView />);

    // The add-topic form is reachable from the full-width dashboard.
    expect(screen.getAllByTestId('scheduled-topics-list').length).toBeGreaterThan(0);
    await waitFor(() => {
      expect(screen.getAllByText(/New/i).length).toBeGreaterThan(0);
    });
  });

  // -------------------------------------------------------------------------
  // Test 3: submitting a topic dispatches cmd_create_scheduled_topic and refreshes
  // -------------------------------------------------------------------------
  it('Test 3: saving a topic dispatches cmd_create_scheduled_topic and refreshes the list', async () => {
    const user = userEvent.setup();
    // First list call is empty; the post-create refresh returns the new topic.
    const listCalls = installDefaultMocks([[], [RESEARCH_TOPIC]]);
    const onChanged = vi.fn();

    render(<ScheduledTopicsList topicType="research" projectId={null} onChanged={onChanged} />);

    await waitFor(() => {
      expect(screen.getByTestId('scheduled-topics-list')).toBeInTheDocument();
    });
    const before = listCalls();

    await user.type(screen.getByTestId('input-topic-title'), 'SSD Streaming & NVMe');
    await user.type(
      screen.getByTestId('input-topic-objective'),
      'Track PCIe Gen5 streaming benchmarks'
    );
    await user.selectOptions(screen.getByTestId('select-topic-cadence'), 'daily');

    await user.click(screen.getByTestId('btn-save-scheduled-topic'));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_create_scheduled_topic',
        expect.objectContaining({
          topic: expect.objectContaining({
            title: 'SSD Streaming & NVMe',
            prompt_query: 'Track PCIe Gen5 streaming benchmarks',
            topic_type: 'research',
            cadence: 'daily',
          }),
        })
      );
    });

    // The list was refreshed after creation and the new card rendered.
    await waitFor(() => {
      expect(listCalls()).toBeGreaterThan(before);
      expect(screen.getByTestId('topic-card-t1')).toBeInTheDocument();
    });
    expect(onChanged).toHaveBeenCalled();

    // The creation form was reset.
    expect(screen.getByTestId('input-topic-title')).toHaveValue('');
  });

  // -------------------------------------------------------------------------
  // Test 4: [ Run Now ] triggers cmd_trigger_scheduled_topic_run
  // -------------------------------------------------------------------------
  it('Test 4: clicking Run Now triggers cmd_trigger_scheduled_topic_run', async () => {
    const user = userEvent.setup();
    installDefaultMocks([[RESEARCH_TOPIC]]);
    const onOpenArtifact = vi.fn();

    render(
      <ScheduledTopicsList
        topicType="research"
        projectId={null}
        onOpenArtifact={onOpenArtifact}
        onChanged={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId('topic-card-t1')).toBeInTheDocument();
    });
    // Cadence badge is rendered from the topic record.
    expect(screen.getByTestId('cadence-badge-t1')).toHaveTextContent('daily');

    await user.click(screen.getByTestId('btn-run-now-t1'));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_trigger_scheduled_topic_run',
        expect.objectContaining({ topicId: 't1' })
      );
    });

    // The generated artifact is surfaced to the caller.
    await waitFor(() => {
      expect(onOpenArtifact).toHaveBeenCalledWith('art-digest-new', null);
    });
  });

  // -------------------------------------------------------------------------
  // Test 5: pause/resume toggle + previous runs open in FocusView
  // -------------------------------------------------------------------------
  it('Test 5: toggling active dispatches the command and past runs open in FocusView', async () => {
    const user = userEvent.setup();
    installDefaultMocks([[RESEARCH_TOPIC]]);
    const onOpenArtifact = vi.fn();

    render(
      <ScheduledTopicsList
        topicType="research"
        projectId={null}
        onOpenArtifact={onOpenArtifact}
        onChanged={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId('topic-card-t1')).toBeInTheDocument();
    });

    // Pause the topic.
    await user.click(screen.getByTestId('btn-toggle-active-t1'));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_toggle_scheduled_topic_active',
        expect.objectContaining({ topicId: 't1', isActive: false })
      );
    });

    // Expand previous digests and open one.
    await user.click(screen.getByTestId('btn-toggle-runs-t1'));
    const runItem = await screen.findByTestId('run-item-art-digest-1');
    await user.click(runItem);
    expect(onOpenArtifact).toHaveBeenCalledWith('art-digest-1', null);
  });

  // -------------------------------------------------------------------------
  // Test 6: wall-clock time + weekday controls appear per cadence
  // -------------------------------------------------------------------------
  it('Test 6: time picker is shown for daily/weekly and hidden for manual', async () => {
    const user = userEvent.setup();
    installDefaultMocks([[]]);

    render(<ScheduledTopicsList topicType="research" projectId={null} onChanged={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByTestId('scheduled-topics-list')).toBeInTheDocument();
    });

    // Daily (default): time picker present, no weekday selector.
    expect(screen.getByTestId('input-topic-time')).toHaveValue('06:00');
    expect(screen.queryByTestId('select-topic-weekday')).not.toBeInTheDocument();

    // Weekly: the weekday anchor appears.
    await user.selectOptions(screen.getByTestId('select-topic-cadence'), 'weekly');
    expect(screen.getByTestId('input-topic-time')).toBeInTheDocument();
    expect(screen.getByTestId('select-topic-weekday')).toBeInTheDocument();

    // Manual: neither control is shown — a time is meaningless without a cadence.
    await user.selectOptions(screen.getByTestId('select-topic-cadence'), 'manual');
    expect(screen.queryByTestId('input-topic-time')).not.toBeInTheDocument();
    expect(screen.queryByTestId('select-topic-weekday')).not.toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // Test 7: the chosen time/day is sent to cmd_create_scheduled_topic
  // -------------------------------------------------------------------------
  it('Test 7: the selected time and weekday are dispatched with the topic', async () => {
    const user = userEvent.setup();
    installDefaultMocks([[], [RESEARCH_TOPIC]]);

    render(<ScheduledTopicsList topicType="plan" projectId={null} onChanged={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByTestId('scheduled-topics-list')).toBeInTheDocument();
    });

    await user.type(screen.getByTestId('input-topic-title'), 'Weekly Architecture Review');
    await user.type(screen.getByTestId('input-topic-objective'), 'Assess blockers');

    await user.selectOptions(screen.getByTestId('select-topic-cadence'), 'weekly');
    await user.selectOptions(screen.getByTestId('select-topic-weekday'), '2'); // Wednesday
    fireEvent.change(screen.getByTestId('input-topic-time'), { target: { value: '07:30' } });

    await user.click(screen.getByTestId('btn-save-scheduled-topic'));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_create_scheduled_topic',
        expect.objectContaining({
          topic: expect.objectContaining({
            topic_type: 'plan',
            cadence: 'weekly',
            schedule_time: '07:30',
            schedule_weekday: 2,
          }),
        })
      );
    });
  });

  // -------------------------------------------------------------------------
  // Test 8: the topic card surfaces the schedule label + next run
  // -------------------------------------------------------------------------
  it('Test 8: the topic card renders the schedule label and next run', async () => {
    installDefaultMocks([[RESEARCH_TOPIC]]);

    render(<ScheduledTopicsList topicType="research" projectId={null} onChanged={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByTestId('topic-card-t1')).toBeInTheDocument();
    });

    expect(screen.getByTestId('schedule-label-t1')).toHaveTextContent('Daily at 06:00 (UTC)');
    expect(screen.getByTestId('next-run-t1')).toHaveTextContent(/Next in /);
  });
});

