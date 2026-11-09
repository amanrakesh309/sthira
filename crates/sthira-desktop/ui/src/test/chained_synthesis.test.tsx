import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { FocusView } from '../components/FocusView';
import * as ipc from '../api/ipc';
import { setEditorMarkdown, editorMarkdown } from './editor-helpers';

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
    cancelRequest: vi.fn().mockResolvedValue(true),
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
        true,
        expect.any(String)
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

    // 4. Verify deriveArtifact IPC was called with proper derivation type and explicit project context
    await waitFor(() => {
      expect(ipc.deriveArtifact).toHaveBeenCalledWith(
        'test-art-1',
        'plan_from_research',
        null,
        expect.anything(),
        'proj-1',
        expect.any(String)
      );
    });

    // 5. Verify navigation to the newly created note
    await waitFor(() => {
      expect(onOpenArtifactSpy).toHaveBeenCalledWith('derived-new-plan-123', 'proj-1');
    });
  });

  it('waits for the current draft to save before deriving', async () => {
    const user = userEvent.setup(); const onOpen = vi.fn();
    const saved = (await ipc.getArtifactContext('test-art-1')).artifact;
    let finishSave!: (artifact: typeof saved) => void;
    vi.mocked(ipc.saveArtifact).mockReturnValueOnce(new Promise((resolve) => { finishSave = resolve; }));
    render(<FocusView artifactId="test-art-1" onBack={vi.fn()} onOpenCanvas={vi.fn()} onOpenArtifact={onOpen} />);
    const editor = await screen.findByRole('textbox', { name: 'Document content' });
    setEditorMarkdown(editor, 'Latest findings');
    await user.click(screen.getByTestId('btn-derive-menu'));
    await user.click(screen.getByTestId('btn-derive-plan'));
    expect(ipc.deriveArtifact).not.toHaveBeenCalled();
    expect(ipc.saveArtifact).toHaveBeenCalledWith('test-art-1', expect.stringContaining('Latest findings'), expect.anything(), 'proj-1');
    finishSave({ ...saved, content_md: 'Latest findings' });
    await waitFor(() => expect(onOpen).toHaveBeenCalledWith('derived-new-plan-123', 'proj-1'));
  });

  it('keeps the current note open when saving before derivation fails', async () => {
    const user = userEvent.setup(); const onOpen = vi.fn();
    vi.mocked(ipc.saveArtifact).mockRejectedValueOnce(new Error('Disk unavailable'));
    render(<FocusView artifactId="test-art-1" onBack={vi.fn()} onOpenCanvas={vi.fn()} onOpenArtifact={onOpen} />);
    await screen.findByRole('textbox', { name: 'Document content' });
    await user.click(screen.getByTestId('btn-derive-menu'));
    await user.click(screen.getByTestId('btn-derive-plan'));
    expect(await screen.findByText('Couldn’t save your changes. Try saving again.')).toBeInTheDocument();
    expect(ipc.deriveArtifact).not.toHaveBeenCalled(); expect(onOpen).not.toHaveBeenCalled();
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

  it('shows honest error in Refine modal and keeps critique draft when refinement fails', async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.refineArtifact).mockRejectedValueOnce(new Error('Model generation failed: connection refused'));

    render(
      <FocusView
        projectId="proj-1"
        artifactId="test-art-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenArtifact={vi.fn()}
      />
    );

    const refineBtn = await screen.findByTestId('btn-refine-artifact');
    await user.click(refineBtn);

    expect(await screen.findByTestId('refine-artifact-modal')).toBeInTheDocument();
    const critiqueInput = screen.getByTestId('textarea-critique-prompt');
    await user.type(critiqueInput, 'Unsuccessful critique attempt');

    const runBtn = screen.getByTestId('btn-run-refinement');
    await user.click(runBtn);

    // Verify error message is rendered in the modal
    expect(await screen.findByText('Model generation failed: connection refused')).toBeInTheDocument();

    // Verify modal remains open and critique input is preserved
    expect(screen.getByTestId('refine-artifact-modal')).toBeInTheDocument();
    expect(critiqueInput).toHaveValue('Unsuccessful critique attempt');
  });

  it('shows honest action error in FocusView and stays on current note when derivation fails', async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    vi.mocked(ipc.deriveArtifact).mockRejectedValueOnce(new Error('Model returned malformed derivation response'));

    render(
      <FocusView
        projectId="proj-1"
        artifactId="test-art-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenArtifact={onOpen}
      />
    );

    await screen.findByRole('textbox', { name: 'Document content' });
    await user.click(screen.getByTestId('btn-derive-menu'));
    await user.click(screen.getByTestId('btn-derive-plan'));

    // Verify action error is rendered
    expect(await screen.findByText('Couldn’t derive a new artifact. Check the selected provider and try again.')).toBeInTheDocument();

    // Verify navigation was NOT called and current note stays open
    expect(onOpen).not.toHaveBeenCalled();
    expect(screen.getByTestId('input-artifact-title')).toHaveValue('Distributed Systems Blueprint');
  });

  it('allows cancellation during refinement, closes modal, and suppresses late responses from mutating document', async () => {
    const user = userEvent.setup();
    let resolveRefine: (val: string) => void;
    vi.mocked(ipc.refineArtifact).mockImplementationOnce(
      () =>
        new Promise<string>((resolve) => {
          resolveRefine = resolve;
        })
    );

    render(
      <FocusView
        projectId="proj-1"
        artifactId="test-art-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenArtifact={vi.fn()}
      />
    );

    const refineBtn = await screen.findByTestId('btn-refine-artifact');
    await user.click(refineBtn);

    expect(await screen.findByTestId('refine-artifact-modal')).toBeInTheDocument();
    const critiqueInput = screen.getByTestId('textarea-critique-prompt');
    await user.type(critiqueInput, 'Critique that gets cancelled');

    const runBtn = screen.getByTestId('btn-run-refinement');
    await user.click(runBtn);

    // Verify modal is in loading state
    expect(screen.getByText('Refining Note...')).toBeInTheDocument();

    // Click Cancel while in flight
    const cancelBtn = screen.getByTestId('btn-cancel-refine-modal');
    await user.click(cancelBtn);

    // Verify native cancellation was dispatched via IPC
    expect(ipc.cancelRequest).toHaveBeenCalledWith(expect.any(String));

    // Verify modal closes immediately
    expect(screen.queryByTestId('refine-artifact-modal')).not.toBeInTheDocument();

    // Now late response resolves
    resolveRefine!('# Late Refined Content That Must Be Ignored');

    // Wait a tick to ensure any late callback would have fired
    await new Promise((r) => setTimeout(r, 50));

    // Content in document editor must remain unmutated original content
    const editor = screen.getByRole('textbox', { name: 'Document content' });
    expect(editorMarkdown(editor)).toContain('Initial findings on Paxos vs Raft.');
    expect(editorMarkdown(editor)).not.toContain('Late Refined Content');
  });

  it('cancelling then immediately retrying dispatches a new request ID and applies only the retried response', async () => {
    const user = userEvent.setup();
    let resolveFirstRefine: (val: string) => void;
    let resolveSecondRefine: (val: string) => void;

    vi.mocked(ipc.refineArtifact)
      .mockImplementationOnce(
        () =>
          new Promise<string>((resolve) => {
            resolveFirstRefine = resolve;
          })
      )
      .mockImplementationOnce(
        () =>
          new Promise<string>((resolve) => {
            resolveSecondRefine = resolve;
          })
      );

    render(
      <FocusView
        projectId="proj-1"
        artifactId="test-art-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenArtifact={vi.fn()}
      />
    );

    // 1. Open modal and start first refinement
    const refineBtn = await screen.findByTestId('btn-refine-artifact');
    await user.click(refineBtn);
    const critiqueInput = screen.getByTestId('textarea-critique-prompt');
    await user.type(critiqueInput, 'First critique');
    await user.click(screen.getByTestId('btn-run-refinement'));

    expect(screen.getByText('Refining Note...')).toBeInTheDocument();
    const firstRequestId = vi.mocked(ipc.refineArtifact).mock.calls[0][4];
    expect(firstRequestId).toBeTruthy();

    // 2. Cancel first refinement
    await user.click(screen.getByTestId('btn-cancel-refine-modal'));
    expect(ipc.cancelRequest).toHaveBeenCalledWith(firstRequestId);
    expect(screen.queryByTestId('refine-artifact-modal')).not.toBeInTheDocument();

    // 3. Immediately reopen and start second refinement
    await user.click(refineBtn);
    expect(await screen.findByTestId('refine-artifact-modal')).toBeInTheDocument();
    await user.click(screen.getByTestId('btn-run-refinement'));

    expect(screen.getByText('Refining Note...')).toBeInTheDocument();
    const secondRequestId = vi.mocked(ipc.refineArtifact).mock.calls[1][4];
    expect(secondRequestId).toBeTruthy();
    expect(secondRequestId).not.toBe(firstRequestId);

    // 4. Resolve first request late while second is in flight
    resolveFirstRefine!('# First Request Content Should Be Discarded');
    await new Promise((r) => setTimeout(r, 20));

    // Modal is still open and loading for the second request
    expect(screen.getByText('Refining Note...')).toBeInTheDocument();

    // 5. Resolve second request
    resolveSecondRefine!('# Second Request Content Successfully Applied');
    await waitFor(() => {
      expect(screen.queryByTestId('refine-artifact-modal')).not.toBeInTheDocument();
    });
  });

  it('failed refine allows retry without closing modal', async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.refineArtifact)
      .mockRejectedValueOnce(new Error('Provider failure on first attempt'))
      .mockResolvedValueOnce('# Successfully Retried Content');

    render(
      <FocusView
        projectId="proj-1"
        artifactId="test-art-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    await user.click(await screen.findByTestId('btn-refine-artifact'));
    const critiqueInput = screen.getByTestId('textarea-critique-prompt');
    await user.type(critiqueInput, 'Keep this critique across attempts');
    const runBtn = screen.getByTestId('btn-run-refinement');
    await user.click(runBtn);

    // Verify error shown and critique retained
    await screen.findByText('Provider failure on first attempt');
    expect(critiqueInput).toHaveValue('Keep this critique across attempts');
    expect(runBtn).toBeEnabled();

    // Retry immediately without closing modal
    await user.click(runBtn);
    await waitFor(() => {
      expect(screen.queryByTestId('refine-artifact-modal')).not.toBeInTheDocument();
    });

    const calls = vi.mocked(ipc.refineArtifact).mock.calls;
    expect(calls).toHaveLength(2);
    expect(calls[0][4]).not.toBe(calls[1][4]); // Dispatched with new request ID
  });

  it('switching document drops old refine callback and does not mutate new document', async () => {
    const user = userEvent.setup();
    let resolveOldRefine!: (value: string) => void;
    vi.mocked(ipc.refineArtifact).mockImplementationOnce(
      () =>
        new Promise<string>((resolve) => {
          resolveOldRefine = resolve;
        })
    );
    vi.mocked(ipc.getArtifactContext).mockImplementation(
      async (id) =>
        ({
          artifact: {
            id,
            artifact_type: 'note',
            authorship: 'user',
            title: id === 'test-art-1' ? 'Page A' : 'Page B',
            content_md: id === 'test-art-1' ? 'Original page A' : 'Original page B',
            content_hash: 'hash-' + id,
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
        }) as any
    );

    const onContentUpdate = vi.fn();
    const props = { projectId: 'proj-1', onBack: vi.fn(), onOpenCanvas: vi.fn(), onContentUpdate };
    const view = render(<FocusView {...props} artifactId="test-art-1" />);

    await user.click(await screen.findByTestId('btn-refine-artifact'));
    await user.type(screen.getByTestId('textarea-critique-prompt'), 'Refine A only');
    await user.click(screen.getByTestId('btn-run-refinement'));

    // Switch to Page B while refinement for Page A is still in-flight
    view.rerender(<FocusView {...props} artifactId="test-art-2" />);
    await waitFor(() => expect(screen.getByTestId('input-artifact-title')).toHaveValue('Page B'));

    onContentUpdate.mockClear();
    vi.mocked(ipc.saveArtifact).mockClear();

    // Resolve late result for Page A
    resolveOldRefine('# Late result for Page A');
    await new Promise((resolve) => setTimeout(resolve, 50));

    // Must not call onContentUpdate with Page A's text
    expect(onContentUpdate).not.toHaveBeenCalledWith('# Late result for Page A');
    // Must remain on Page B
    expect(screen.getByTestId('input-artifact-title')).toHaveValue('Page B');
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain('Original page B');
    // saveArtifact must not have been called with test-art-2 and Page A content
    const testArt2Saves = vi.mocked(ipc.saveArtifact).mock.calls.filter(call => call[0] === 'test-art-2' && String(call[1]).includes('Page A'));
    expect(testArt2Saves).toHaveLength(0);
  });

  it('late rejection from cancelled request A does not stop loading on pending retry B', async () => {
    const user = userEvent.setup();
    let rejectFirst!: (err: any) => void;
    let resolveSecond!: (val: string) => void;

    vi.mocked(ipc.refineArtifact)
      .mockImplementationOnce(
        () =>
          new Promise<string>((_, reject) => {
            rejectFirst = reject;
          })
      )
      .mockImplementationOnce(
        () =>
          new Promise<string>((resolve) => {
            resolveSecond = resolve;
          })
      );

    render(
      <FocusView
        projectId="proj-1"
        artifactId="test-art-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    // 1. Open modal and start first refinement
    await user.click(await screen.findByTestId('btn-refine-artifact'));
    await user.type(screen.getByTestId('textarea-critique-prompt'), 'Initial critique');
    await user.click(screen.getByTestId('btn-run-refinement'));
    expect(screen.getByText('Refining Note...')).toBeInTheDocument();

    // 2. Cancel first refinement and immediately start second
    await user.click(screen.getByTestId('btn-cancel-refine-modal'));
    await user.click(await screen.findByTestId('btn-refine-artifact'));
    await user.click(screen.getByTestId('btn-run-refinement'));
    expect(screen.getByText('Refining Note...')).toBeInTheDocument();

    // 3. First request rejects late with error
    rejectFirst(new Error('Late network disconnect'));
    await new Promise((r) => setTimeout(r, 20));

    // Second request must still be loading without being aborted by first request's finally block
    expect(screen.getByText('Refining Note...')).toBeInTheDocument();

    // 4. Resolve second request successfully
    resolveSecond('# Clean Second Request');
    await waitFor(() => {
      expect(screen.queryByTestId('refine-artifact-modal')).not.toBeInTheDocument();
    });
  });

  it('cancel rejected after commit does not discard saved result and reconciles editor', async () => {
    const user = userEvent.setup();
    let resolveRefine!: (v: string) => void;
    vi.mocked(ipc.refineArtifact).mockImplementationOnce(
      () =>
        new Promise<string>((resolve) => {
          resolveRefine = resolve;
        })
    );
    // Cancellation is rejected by backend because commit phase has already begun
    vi.mocked(ipc.cancelRequest).mockResolvedValueOnce(false);

    render(
      <FocusView
        projectId="proj-1"
        artifactId="test-art-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    await user.click(await screen.findByTestId('btn-refine-artifact'));
    await user.type(screen.getByTestId('textarea-critique-prompt'), 'Refine committed result');
    await user.click(screen.getByTestId('btn-run-refinement'));

    // User attempts to cancel
    await user.click(screen.getByTestId('btn-cancel-refine-modal'));

    // Simulate backend getArtifactContext returning the committed artifact
    const savedContext = await ipc.getArtifactContext('test-art-1');
    vi.mocked(ipc.getArtifactContext).mockResolvedValue({
      ...savedContext,
      artifact: {
        ...savedContext.artifact,
        content_md: '# Distributed Systems Blueprint\n\nCommitted result must be reconciled',
        title: 'Distributed Systems Blueprint',
      },
    });

    // Resolve refine with the committed content
    resolveRefine('# Distributed Systems Blueprint\n\nCommitted result must be reconciled');
    await new Promise((resolve) => setTimeout(resolve, 75));

    // Editor content must reconcile with the committed result rather than remaining stale
    expect(
      editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))
    ).toContain('Committed result must be reconciled');
  });

  it('cancel rejected after commit preserves newer user draft without overwriting or dispatching save', async () => {
    const user = userEvent.setup();
    let resolveRefine!: (v: string) => void;
    vi.mocked(ipc.refineArtifact).mockImplementationOnce(
      () =>
        new Promise<string>((resolve) => {
          resolveRefine = resolve;
        })
    );
    vi.mocked(ipc.cancelRequest).mockResolvedValueOnce(false);

    render(
      <FocusView
        projectId="proj-1"
        artifactId="test-art-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    await user.click(await screen.findByTestId('btn-refine-artifact'));
    await user.type(screen.getByTestId('textarea-critique-prompt'), 'Refine committed result');
    await user.click(screen.getByTestId('btn-run-refinement'));
    await user.click(screen.getByTestId('btn-cancel-refine-modal'));

    // User continues editing while committed response is still returning from backend
    const editor = screen.getByRole('textbox', { name: 'Document content' });
    setEditorMarkdown(
      editor,
      '# Distributed Systems Blueprint\n\nUSER DRAFT WRITTEN AFTER CLOSING REFINE'
    );
    expect(editorMarkdown(editor)).toContain('USER DRAFT WRITTEN AFTER CLOSING REFINE');

    const savedContext = await ipc.getArtifactContext('test-art-1');
    vi.mocked(ipc.getArtifactContext).mockResolvedValue({
      ...savedContext,
      artifact: {
        ...savedContext.artifact,
        content_md: '# Distributed Systems Blueprint\n\nCommitted model result',
        title: 'Distributed Systems Blueprint',
      },
    });
    vi.mocked(ipc.saveArtifact).mockClear();

    // Model refinement resolves with committed result
    resolveRefine('# Distributed Systems Blueprint\n\nCommitted model result');
    await new Promise((resolve) => setTimeout(resolve, 100));

    // The newer user draft must NOT be overwritten by the committed model result
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
      'USER DRAFT WRITTEN AFTER CLOSING REFINE'
    );

    // saveArtifact must not have been dispatched to overwrite user draft with model result
    const modelSaves = vi
      .mocked(ipc.saveArtifact)
      .mock.calls.filter((call) => String(call[1]).includes('Committed model result'));
    expect(modelSaves).toHaveLength(0);

    // Conflict banner offers explicit reconciliation without destroying the draft
    expect(screen.getByTestId('banner-refinement-conflict')).toBeInTheDocument();
  });

  async function setupConflictFixture(options: {
    waitForAutosave?: boolean;
  } = {}) {
    const user = userEvent.setup();
    let resolveModel!: (value: string) => void;
    let resolveCancel!: (value: boolean) => void;
    const initial = await ipc.getArtifactContext('test-art-1');
    let stored = {
      ...initial,
      artifact: {
        ...initial.artifact,
        id: 'test-art-1',
        title: 'Distributed Systems Blueprint',
        content_md: '# Distributed Systems Blueprint\n\nInitial source content',
      },
    };
    vi.mocked(ipc.getArtifactContext).mockImplementation(async (id?: string) => {
      if (id === 'test-art-2') {
        return {
          ...initial,
          artifact: {
            ...initial.artifact,
            id: 'test-art-2',
            title: 'Second Note',
            content_md: '# Second Note\n\nSecond note body',
          },
        };
      }
      return structuredClone(stored);
    });

    let saveShouldFail = false;
    let resolveDeferredSave: (() => void) | null = null;
    let waitForSavePromise: Promise<void> | null = null;

    vi.mocked(ipc.saveArtifact).mockImplementation(async (id, body, title) => {
      if (saveShouldFail) {
        saveShouldFail = false;
        throw new Error('Disk unavailable');
      }
      if (waitForSavePromise) {
        await waitForSavePromise;
      }
      stored = {
        ...stored,
        artifact: {
          ...stored.artifact,
          id: id || 'test-art-1',
          content_md: body,
          title: title || 'Distributed Systems Blueprint',
        },
      };
      return structuredClone(stored.artifact);
    });

    vi.mocked(ipc.refineArtifact).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveModel = resolve;
        })
    );
    vi.mocked(ipc.cancelRequest).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveCancel = resolve;
        })
    );

    const view = render(
      <FocusView
        projectId="proj-1"
        artifactId="test-art-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    await screen.findByTestId('input-artifact-title');
    await user.click(await screen.findByTestId('btn-refine-artifact'));
    const critiqueInput = await screen.findByTestId('textarea-critique-prompt');
    await user.type(critiqueInput, 'Refine source');
    await user.click(screen.getByTestId('btn-run-refinement'));
    await waitFor(() => expect(ipc.refineArtifact).toHaveBeenCalled());
    await user.click(await screen.findByTestId('btn-cancel-refine-modal'));
    await waitFor(() => expect(ipc.cancelRequest).toHaveBeenCalled());

    setEditorMarkdown(
      screen.getByRole('textbox', { name: 'Document content' }),
      '# Distributed Systems Blueprint\n\nNEWER USER DRAFT TO KEEP'
    );

    if (options.waitForAutosave) {
      await waitFor(() => expect(stored.artifact.content_md).toContain('NEWER USER DRAFT TO KEEP'), {
        timeout: 3000,
      });
    }

    // Model commit wins after the user edit but before cancellation is acknowledged
    stored = {
      ...stored,
      artifact: {
        ...stored.artifact,
        content_md: '# Distributed Systems Blueprint\n\nCOMMITTED MODEL CONTENT',
      },
    };
    resolveCancel(false);
    resolveModel('# Distributed Systems Blueprint\n\nCOMMITTED MODEL CONTENT');

    await screen.findByTestId('banner-refinement-conflict');
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
      'NEWER USER DRAFT TO KEEP'
    );

    return {
      user,
      view,
      readStored: () => stored.artifact.content_md,
      makeNextSaveFail: () => {
        saveShouldFail = true;
      },
      deferNextSave: () => {
        waitForSavePromise = new Promise((r) => {
          resolveDeferredSave = r;
        });
        return () => resolveDeferredSave?.();
      },
    };
  }

  it('conflict Keep My Draft persists user draft when draft was already autosaved before model commit', async () => {
    const { user, view, readStored } = await setupConflictFixture({ waitForAutosave: true });
    vi.mocked(ipc.saveArtifact).mockClear();

    await user.click(screen.getByTestId('btn-dismiss-pending-refinement'));
    await waitFor(() => expect(screen.queryByTestId('banner-refinement-conflict')).not.toBeInTheDocument());

    expect(readStored()).toContain('NEWER USER DRAFT TO KEEP');
    expect(ipc.saveArtifact).toHaveBeenCalledWith(
      'test-art-1',
      expect.stringContaining('NEWER USER DRAFT TO KEEP'),
      expect.anything(),
      'proj-1'
    );

    view.unmount();
    const secondView = render(<FocusView projectId="proj-1" artifactId="test-art-1" onBack={vi.fn()} onOpenCanvas={vi.fn()} />);
    await screen.findByTestId('input-artifact-title');
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
      'NEWER USER DRAFT TO KEEP'
    );
    secondView.unmount();
  });

  it('conflict Keep My Draft persists user draft when draft was unsaved at conflict time', async () => {
    const { user, view, readStored } = await setupConflictFixture({ waitForAutosave: false });
    vi.mocked(ipc.saveArtifact).mockClear();

    await user.click(screen.getByTestId('btn-dismiss-pending-refinement'));
    await waitFor(() => expect(screen.queryByTestId('banner-refinement-conflict')).not.toBeInTheDocument());

    expect(readStored()).toContain('NEWER USER DRAFT TO KEEP');
    expect(ipc.saveArtifact).toHaveBeenCalledWith(
      'test-art-1',
      expect.stringContaining('NEWER USER DRAFT TO KEEP'),
      expect.anything(),
      'proj-1'
    );

    view.unmount();
    const secondView = render(<FocusView projectId="proj-1" artifactId="test-art-1" onBack={vi.fn()} onOpenCanvas={vi.fn()} />);
    await screen.findByTestId('input-artifact-title');
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
      'NEWER USER DRAFT TO KEEP'
    );
    secondView.unmount();
  });

  it('conflict Keep save failure retains banner and draft, allowing successful retry', async () => {
    const { user, readStored, makeNextSaveFail } = await setupConflictFixture({ waitForAutosave: false });
    makeNextSaveFail();

    await user.click(screen.getByTestId('btn-dismiss-pending-refinement'));
    expect(await screen.findByTestId('refinement-conflict-error')).toHaveTextContent(
      'Couldn’t save your draft. Try keeping again.'
    );
    expect(screen.getByTestId('banner-refinement-conflict')).toBeInTheDocument();
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
      'NEWER USER DRAFT TO KEEP'
    );

    // Clicking Keep My Draft again retries the save successfully
    await user.click(screen.getByTestId('btn-dismiss-pending-refinement'));
    await waitFor(() => expect(screen.queryByTestId('banner-refinement-conflict')).not.toBeInTheDocument());
    expect(readStored()).toContain('NEWER USER DRAFT TO KEEP');
  });

  it('conflict Apply Refinement before autosave persists model choice and retains it on reopen', async () => {
    const { user, view, readStored } = await setupConflictFixture({ waitForAutosave: false });
    vi.mocked(ipc.saveArtifact).mockClear();

    await user.click(screen.getByTestId('btn-apply-pending-refinement'));
    await waitFor(() => expect(screen.queryByTestId('banner-refinement-conflict')).not.toBeInTheDocument());

    expect(readStored()).toContain('COMMITTED MODEL CONTENT');
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
      'COMMITTED MODEL CONTENT'
    );
    expect(ipc.saveArtifact).toHaveBeenCalledWith(
      'test-art-1',
      expect.stringContaining('COMMITTED MODEL CONTENT'),
      expect.anything(),
      'proj-1'
    );

    view.unmount();
    const secondView = render(
      <FocusView projectId="proj-1" artifactId="test-art-1" onBack={vi.fn()} onOpenCanvas={vi.fn()} />
    );
    await screen.findByTestId('input-artifact-title');
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
      'COMMITTED MODEL CONTENT'
    );
    secondView.unmount();
  });

  it('conflict Apply Refinement after autosave persists model choice and retains it on reopen', async () => {
    const { user, view, readStored } = await setupConflictFixture({ waitForAutosave: false });
    // Allow autosave to persist the user's newer draft first
    await waitFor(() => expect(readStored()).toContain('NEWER USER DRAFT TO KEEP'), { timeout: 3000 });
    vi.mocked(ipc.saveArtifact).mockClear();

    await user.click(screen.getByTestId('btn-apply-pending-refinement'));
    await waitFor(() => expect(screen.queryByTestId('banner-refinement-conflict')).not.toBeInTheDocument());

    expect(readStored()).toContain('COMMITTED MODEL CONTENT');
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
      'COMMITTED MODEL CONTENT'
    );
    expect(ipc.saveArtifact).toHaveBeenCalledWith(
      'test-art-1',
      expect.stringContaining('COMMITTED MODEL CONTENT'),
      expect.anything(),
      'proj-1'
    );

    // Reopen FocusView against the same backing store
    view.unmount();
    const secondView = render(
      <FocusView projectId="proj-1" artifactId="test-art-1" onBack={vi.fn()} onOpenCanvas={vi.fn()} />
    );
    await screen.findByTestId('input-artifact-title');
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
      'COMMITTED MODEL CONTENT'
    );
    secondView.unmount();
  });

  it('conflict Apply save failure retains banner and draft, allowing successful retry', async () => {
    const { user, readStored, makeNextSaveFail } = await setupConflictFixture({ waitForAutosave: false });
    makeNextSaveFail();

    await user.click(screen.getByTestId('btn-apply-pending-refinement'));
    expect(await screen.findByTestId('refinement-conflict-error')).toHaveTextContent(
      'Couldn’t apply refinement. Try applying again.'
    );
    expect(screen.getByTestId('banner-refinement-conflict')).toBeInTheDocument();
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
      'NEWER USER DRAFT TO KEEP'
    );

    // Clicking Apply Refinement again retries the save successfully
    await user.click(screen.getByTestId('btn-apply-pending-refinement'));
    await waitFor(() => expect(screen.queryByTestId('banner-refinement-conflict')).not.toBeInTheDocument());
    expect(readStored()).toContain('COMMITTED MODEL CONTENT');
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
      'COMMITTED MODEL CONTENT'
    );
  });

  it('navigating while conflict Apply save is pending isolates documents', async () => {
    const { user, view, deferNextSave } = await setupConflictFixture({ waitForAutosave: false });
    const releaseSave = deferNextSave();

    // Click Apply, starting in-flight save for test-art-1
    await user.click(screen.getByTestId('btn-apply-pending-refinement'));

    // Switch FocusView props to test-art-2 while save for test-art-1 is still pending
    view.rerender(
      <FocusView
        projectId="proj-1"
        artifactId="test-art-2"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    // Release the deferred save for test-art-1
    releaseSave();

    // Verify test-art-2 loads its own content cleanly without pollution
    await waitFor(() => {
      expect(screen.getByTestId('input-artifact-title')).toHaveValue('Second Note');
    });
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
      'Second note body'
    );
    expect(screen.queryByTestId('banner-refinement-conflict')).not.toBeInTheDocument();
  });

  it('navigating while conflict Keep save is pending isolates documents', async () => {
    const { user, view, deferNextSave } = await setupConflictFixture({ waitForAutosave: false });
    const releaseSave = deferNextSave();

    // Click Keep, starting in-flight save for test-art-1
    await user.click(screen.getByTestId('btn-dismiss-pending-refinement'));

    // Switch FocusView props to test-art-2 while save for test-art-1 is still pending
    view.rerender(
      <FocusView
        projectId="proj-1"
        artifactId="test-art-2"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    // Release the deferred save for test-art-1
    releaseSave();

    // Verify test-art-2 loads its own content cleanly without pollution
    await waitFor(() => {
      expect(screen.getByTestId('input-artifact-title')).toHaveValue('Second Note');
    });
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
      'Second note body'
    );
    expect(screen.queryByTestId('banner-refinement-conflict')).not.toBeInTheDocument();
  });

  it('Keep refresh rejection after navigation to page B isolates B identity, draft, and autosave targets', async () => {
    const { user, view } = await setupConflictFixture({ waitForAutosave: true });
    const normalGet = vi.mocked(ipc.getArtifactContext).getMockImplementation()!;
    let rejectRefresh!: (error: Error) => void;
    let refreshEntered = false;
    vi.mocked(ipc.getArtifactContext).mockImplementation((id) => {
      if (id === 'test-art-1' && !refreshEntered) {
        refreshEntered = true;
        return new Promise((_resolve, reject) => {
          rejectRefresh = reject;
        });
      }
      return normalGet(id);
    });

    await user.click(screen.getByTestId('btn-dismiss-pending-refinement'));
    await waitFor(() => expect(refreshEntered).toBe(true));

    view.rerender(
      <FocusView projectId="proj-1" artifactId="test-art-2" onBack={vi.fn()} onOpenCanvas={vi.fn()} />
    );
    await waitFor(() => expect(screen.getByTestId('input-artifact-title')).toHaveValue('Second Note'));

    // Old page refresh rejects after Page B has already loaded
    rejectRefresh(new Error('Old-page context refresh failed'));
    await new Promise((resolve) => setTimeout(resolve, 75));

    vi.mocked(ipc.saveArtifact).mockClear();
    setEditorMarkdown(
      screen.getByRole('textbox', { name: 'Document content' }),
      '# Second Note\n\nNEW EDITS ONLY FOR SECOND NOTE'
    );
    await waitFor(() => expect(ipc.saveArtifact).toHaveBeenCalled(), { timeout: 3000 });

    // Assert that Page B's edit only saves to Page B (test-art-2), NEVER test-art-1
    const art1Saves = vi
      .mocked(ipc.saveArtifact)
      .mock.calls.filter((call) => call[0] === 'test-art-1' && String(call[1]).includes('NEW EDITS ONLY FOR SECOND NOTE'));
    expect(art1Saves).toHaveLength(0);

    const art2Saves = vi
      .mocked(ipc.saveArtifact)
      .mock.calls.filter((call) => call[0] === 'test-art-2' && String(call[1]).includes('NEW EDITS ONLY FOR SECOND NOTE'));
    expect(art2Saves.length).toBeGreaterThan(0);

    expect(screen.getByTestId('input-artifact-title')).toHaveValue('Second Note');
    expect(screen.queryByTestId('banner-refinement-conflict')).not.toBeInTheDocument();
  });

  it('Keep late successful refresh after navigation to page B does not overwrite B context or state', async () => {
    const { user, view } = await setupConflictFixture({ waitForAutosave: true });
    const normalGet = vi.mocked(ipc.getArtifactContext).getMockImplementation()!;
    let resolveRefresh!: (ctx: any) => void;
    let refreshEntered = false;
    vi.mocked(ipc.getArtifactContext).mockImplementation((id) => {
      if (id === 'test-art-1' && !refreshEntered) {
        refreshEntered = true;
        return new Promise((resolve) => {
          resolveRefresh = resolve;
        });
      }
      return normalGet(id);
    });

    await user.click(screen.getByTestId('btn-dismiss-pending-refinement'));
    await waitFor(() => expect(refreshEntered).toBe(true));

    view.rerender(
      <FocusView projectId="proj-1" artifactId="test-art-2" onBack={vi.fn()} onOpenCanvas={vi.fn()} />
    );
    await waitFor(() => expect(screen.getByTestId('input-artifact-title')).toHaveValue('Second Note'));

    // Old page refresh succeeds late after Page B has already loaded
    const oldContext = await normalGet('test-art-1');
    resolveRefresh(oldContext);
    await new Promise((resolve) => setTimeout(resolve, 75));

    vi.mocked(ipc.saveArtifact).mockClear();
    setEditorMarkdown(
      screen.getByRole('textbox', { name: 'Document content' }),
      '# Second Note\n\nNEW EDITS AFTER LATE REFRESH SUCCESS'
    );
    await waitFor(() => expect(ipc.saveArtifact).toHaveBeenCalled(), { timeout: 3000 });

    const art1Saves = vi
      .mocked(ipc.saveArtifact)
      .mock.calls.filter((call) => call[0] === 'test-art-1');
    expect(art1Saves).toHaveLength(0);

    const art2Saves = vi
      .mocked(ipc.saveArtifact)
      .mock.calls.filter((call) => call[0] === 'test-art-2');
    expect(art2Saves.length).toBeGreaterThan(0);

    expect(screen.getByTestId('input-artifact-title')).toHaveValue('Second Note');
    expect(screen.queryByTestId('banner-refinement-conflict')).not.toBeInTheDocument();
  });

  it('same-artifact reload with a different load generation rejects stale conflict completion', async () => {
    const { user, view, deferNextSave } = await setupConflictFixture({ waitForAutosave: false });
    const releaseSave = deferNextSave();

    // Click Keep, starting in-flight save for test-art-1 under loadRequest generation 1
    await user.click(screen.getByTestId('btn-dismiss-pending-refinement'));

    // Trigger a same-artifact reload by rerendering FocusView
    view.rerender(
      <FocusView
        projectId="proj-1"
        artifactId="test-art-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    // Release the stale save from generation 1
    releaseSave();
    await new Promise((resolve) => setTimeout(resolve, 75));

    // Stale operation should not have corrupted the fresh load generation
    await screen.findByTestId('input-artifact-title');
    expect(screen.getByTestId('input-artifact-title')).toHaveValue('Distributed Systems Blueprint');
  });
});
