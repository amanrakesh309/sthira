import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { FocusView } from '../components/FocusView';
import * as ipc from '../api/ipc';

// Mock ipc calls
vi.mock('../api/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof ipc>();
  return {
    ...actual,
    getArtifactContext: vi.fn().mockResolvedValue({
      artifact: {
        id: 'test-art-1',
        artifact_type: 'note',
        authorship: 'user',
        title: 'Distributed Systems Blueprint',
        content_md: '# Distributed Systems Blueprint\n\nInitial findings on Paxos vs Raft.',
        content_hash: 'hash-1',
        version: 1,
        is_unfinished_draft: false,
        created_at: 1000,
        updated_at: 1000,
      },
      epistemic_nodes: [],
      pending_proposals: [],
      project_id: 'proj-1',
      is_unfinished_draft: false,
      derived_from_title: null,
    }),
    getResumeBrief: vi.fn().mockResolvedValue({
      project_id: 'proj-1',
      primary_continuation_artifact_id: 'test-art-1',
      purpose: 'Distributed Systems',
      last_substantive_delta: 'Initial setup',
      invariants: [],
      open_questions: [],
      next_milestones: [],
    }),
    getProjectHistory: vi.fn().mockResolvedValue({
      project_id: 'proj-1',
      items: [],
    }),
    saveArtifact: vi.fn().mockImplementation((id, content, title) => {
      return Promise.resolve({
        id: id || 'test-art-1',
        artifact_type: 'note',
        authorship: 'user',
        title: title || 'Distributed Systems Blueprint',
        content_md: content,
        content_hash: 'hash-saved',
        version: 2,
        is_unfinished_draft: false,
        created_at: 1000,
        updated_at: 2000,
      });
    }),
    getProviderCatalog: vi.fn().mockResolvedValue([
      {
        id: 'ollama',
        display_name: 'Ollama Local',
        provider_type: 'ollama',
        base_url: 'http://localhost:11434',
        is_enabled: true,
        models: [
          {
            id: 'ollama:llama3.2',
            provider_id: 'ollama',
            model_identifier: 'llama3.2:latest',
            display_name: 'Llama 3.2',
            context_window: 8192,
            supports_structured_output: true,
            is_enabled: true,
            is_default: true,
            web_search_capability: 'engine_augmented_only',
          },
          {
            id: 'ollama:qwen2.5',
            provider_id: 'ollama',
            model_identifier: 'qwen2.5:32b',
            display_name: 'Qwen 2.5 (32B)',
            context_window: 32768,
            supports_structured_output: true,
            is_enabled: true,
            is_default: false,
            web_search_capability: 'agentic_tool_call',
          },
        ],
      },
    ]),
    refineArtifact: vi.fn().mockResolvedValue(
      '# Distributed Systems Blueprint\n\nRefined with benchmark comparisons and Paxos trade-offs.'
    ),
    deriveArtifact: vi.fn().mockResolvedValue('derived-new-plan-123'),
    listObjectTypes: vi.fn().mockResolvedValue([]),
    getTypeTemplates: vi.fn().mockResolvedValue([]),
    getObjectInstance: vi.fn().mockResolvedValue(null),
  };
});

describe('Chained Synthesis: Re-Research & Note Derivation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('Test A: Clicking [Refine] opens modal, allows critique input, web search toggle, and dispatches refinement', async () => {
    const user = userEvent.setup();

    render(
      <FocusView
        projectId="proj-1"
        artifactId="test-art-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    // Wait for artifact to load
    await waitFor(() => {
      expect(screen.getByTestId('input-artifact-title')).toHaveValue(
        'Distributed Systems Blueprint'
      );
    });

    // 1. Locate and click [Refine] toolbar button
    const refineBtn = screen.getByTestId('btn-refine-artifact');
    expect(refineBtn).toBeInTheDocument();
    await user.click(refineBtn);

    // 2. Refine modal should open
    expect(await screen.findByTestId('refine-artifact-modal')).toBeInTheDocument();

    // 3. Enter critique into textarea
    const critiqueInput = screen.getByTestId('textarea-critique-prompt');
    await user.type(
      critiqueInput,
      'Add throughput latency benchmarks and Paxos comparison'
    );
    expect(critiqueInput).toHaveValue(
      'Add throughput latency benchmarks and Paxos comparison'
    );

    // 4. Toggle web search
    const webSearchCheckbox = screen.getByTestId('checkbox-web-search-refine');
    expect(webSearchCheckbox).not.toBeChecked();
    await user.click(webSearchCheckbox);
    expect(webSearchCheckbox).toBeChecked();

    // 5. Submit refinement
    const runBtn = screen.getByTestId('btn-run-refinement');
    expect(runBtn).toBeEnabled();
    await user.click(runBtn);

    // 6. Verify refineArtifact IPC call
    await waitFor(() => {
      expect(ipc.refineArtifact).toHaveBeenCalledWith(
        'test-art-1',
        'Add throughput latency benchmarks and Paxos comparison',
        expect.anything(),
        true
      );
    });

    // 7. Modal closes upon completion
    await waitFor(() => {
      expect(screen.queryByTestId('refine-artifact-modal')).not.toBeInTheDocument();
    });
  });

  it('Test B: Clicking Derive menu and selecting execution plan triggers deriveArtifact and navigates to new note', async () => {
    const user = userEvent.setup();
    const onOpenArtifactSpy = vi.fn();

    render(
      <FocusView
        projectId="proj-1"
        artifactId="test-art-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenArtifact={onOpenArtifactSpy}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId('input-artifact-title')).toBeInTheDocument();
    });

    // 1. Click Derive dropdown trigger
    const deriveMenuBtn = screen.getByTestId('btn-derive-menu');
    expect(deriveMenuBtn).toBeInTheDocument();
    await user.click(deriveMenuBtn);

    // 2. Derive dropdown menu opens with 3 derivation options
    expect(await screen.findByTestId('derive-dropdown-menu')).toBeInTheDocument();
    expect(screen.getByTestId('btn-derive-plan')).toBeInTheDocument();
    expect(screen.getByTestId('btn-derive-ideas')).toBeInTheDocument();
    expect(screen.getByTestId('btn-derive-tasks')).toBeInTheDocument();

    // 3. Click "Derive Execution Plan"
    await user.click(screen.getByTestId('btn-derive-plan'));

    // 4. Verify deriveArtifact IPC was called with proper derivation type
    await waitFor(() => {
      expect(ipc.deriveArtifact).toHaveBeenCalledWith(
        'test-art-1',
        'plan_from_research',
        null,
        expect.anything()
      );
    });

    // 5. Verify navigation to the newly created note
    await waitFor(() => {
      expect(onOpenArtifactSpy).toHaveBeenCalledWith('derived-new-plan-123', 'proj-1');
    });
  });

  it('Test C: Derived artifact renders parent lineage badge above document title and navigates on click', async () => {
    const user = userEvent.setup();
    const onOpenArtifactSpy = vi.fn();

    // Mock an artifact that is derived from another artifact
    vi.mocked(ipc.getArtifactContext).mockResolvedValueOnce({
      artifact: {
        id: 'derived-plan-456',
        artifact_type: 'note',
        authorship: 'ai_generated',
        title: 'Execution Plan: Raft Integration',
        content_md: '# Execution Plan: Raft Integration\n\nPhased milestones derived from research.',
        content_hash: 'hash-derived',
        version: 1,
        is_unfinished_draft: false,
        created_at: 2000,
        updated_at: 2000,
        derived_from_artifact_id: 'parent-art-001',
        derivation_type: 'plan_from_research',
      },
      epistemic_nodes: [],
      pending_proposals: [],
      project_id: 'proj-1',
      is_unfinished_draft: false,
      derived_from_title: 'Consensus Research Foundation',
    });

    render(
      <FocusView
        projectId="proj-1"
        artifactId="derived-plan-456"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenArtifact={onOpenArtifactSpy}
      />
    );

    // 1. Verify parent lineage chip is rendered directly above the title
    const lineageBadge = await screen.findByTestId('badge-lineage-derived-from');
    expect(lineageBadge).toBeInTheDocument();
    expect(lineageBadge).toHaveTextContent('Derived from:');
    expect(lineageBadge).toHaveTextContent('Consensus Research Foundation');
    expect(lineageBadge).toHaveTextContent('plan_from_research');

    // 2. Click lineage badge -> should navigate to parent artifact
    await user.click(lineageBadge);
    expect(onOpenArtifactSpy).toHaveBeenCalledWith('parent-art-001', 'proj-1');
  });
});
