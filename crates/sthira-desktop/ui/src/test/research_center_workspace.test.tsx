import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { App } from '../App';
import * as ipc from '../api/ipc';
import type { ResearchStage, WebResearchResult } from '../types/sthira';

/**
 * Phase 19/20: all research, planning and synthesis must stream and display directly in
 * the full-width center document editor. There is no persistent right-hand drawer.
 *
 * The IPC layer is mocked so the pipeline's stage stream can be driven deterministically.
 * `isTauri: false` makes every other call fall through to the real browser-preview
 * implementations, so the whole app still mounts realistically.
 */
vi.mock('../api/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api/ipc')>();
  return {
    ...actual,
    isTauri: false,
    createResearchPage: vi.fn(),
    runResearchIntoPage: vi.fn(),
    createPage: vi.fn(),
    saveArtifact: vi.fn(),
    streamAdaptiveRun: vi.fn(),
    getArtifactContext: vi.fn(),
    // Silence incidental background IPC calls with no bearing on this suite.
    getObjectInstance: vi.fn(async () => null),
    listObjectTypes: vi.fn(async () => []),
    getWorkspaceTree: vi.fn(async () => ({ projects: [], unassociated_pages: [] })),
    getObjectTypeCounts: vi.fn(async () => ({})),
    getSidebarItems: vi.fn(async () => ({ projects: [], notes: [], objects: [] })),
    // The scheduler push subscription opens a real Tauri event channel; stub it so the
    // dashboard never attempts live IPC under jsdom.
    onScheduledTopicReady: vi.fn(async () => () => {}),
  };
});

const RESEARCH_ARTIFACT_ID = 'art-research-nvme';
const PLAN_ARTIFACT_ID = 'art-plan-1';
const QUERY = 'PCIe Gen5 NVMe streaming limits';
const GOAL = 'Ship the Phase 20 center-workspace refactor';

const GROUNDED_MARKDOWN = [
  `# Research: ${QUERY}`,
  '',
  '## Synthesis',
  'PCIe Gen5 NVMe streaming sustains fourteen gigabytes per second [Source 1].',
  '',
  '## Contradictions & Evolution',
  'Older SATA-era assumptions about 550 MB/s ceilings no longer hold.',
  '',
  '## Sources',
  '1. [PCIe Gen5 NVMe Streaming](https://nvme.example.test/pcie-gen5)',
].join('\n');

const RESEARCH_RESULT: WebResearchResult = {
  query: QUERY,
  search_queries_used: [QUERY],
  documents: [],
  findings: [],
  synthesized_markdown: GROUNDED_MARKDOWN,
  mode_used: 'always_search',
  stages: [],
  sources: [],
};

/** Body returned by the mocked `getArtifactContext`, mutated once a run completes. */
let currentBody = `# Research: ${QUERY}\n\n> **Status:** draft — researching…\n`;
let currentArtifactId = RESEARCH_ARTIFACT_ID;
let currentTitle = `Research: ${QUERY}`;

const artifactContext = () => ({
  artifact: {
    id: currentArtifactId,
    artifact_type: 'note',
    authorship: 'external',
    title: currentTitle,
    content_md: currentBody,
    content_hash: 'hash-run-1',
    version: 1,
    is_unfinished_draft: false,
    created_at: Date.now(),
    updated_at: Date.now(),
  },
  epistemic_nodes: [],
  pending_proposals: [],
  project_id: null,
  is_unfinished_draft: false,
});

/** Renders the app and waits for the Home hero to settle. */
const renderApp = async () => {
  render(<App />);
  await waitFor(() => {
    expect(screen.getByText(/Make room to think/i)).toBeInTheDocument();
  });
};


/** Types a prompt into the Universal Input Bar in the given mode and presses Enter. */
const submitFromUniversalInput = (mode: 'research' | 'plan', text: string) => {
  fireEvent.click(screen.getByTestId(`mode-btn-${mode}`));
  const input = screen.getByPlaceholderText(/Ask, research, capture or plan/i);
  fireEvent.change(input, { target: { value: text } });
  fireEvent.keyDown(input, { key: 'Enter', shiftKey: false });
};


describe('Phase 20: research & plan stream into the full-width center workspace', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    currentBody = `# Research: ${QUERY}\n\n> **Status:** draft — researching…\n`;
    currentArtifactId = RESEARCH_ARTIFACT_ID;
    currentTitle = `Research: ${QUERY}`;
    vi.mocked(ipc.getArtifactContext).mockImplementation(async () => artifactContext() as any);
  });

  // ---------------------------------------------------------------------------
  // 1. Research from the Universal Input Bar navigates straight into FocusView
  // ---------------------------------------------------------------------------
  it('submitting a research query opens the artifact in FocusView and streams live, with no side drawer', async () => {
    vi.mocked(ipc.createResearchPage).mockResolvedValue(RESEARCH_ARTIFACT_ID);

    // Hold the run open so the stage stream can be driven by hand.
    let pushStage: ((stage: ResearchStage) => void) | null = null;
    let resolveRun: ((result: WebResearchResult) => void) | null = null;
    vi.mocked(ipc.runResearchIntoPage).mockImplementation(
      async (_artifactId, _query, _mode, onStage) => {
        pushStage = onStage ?? null;
        return await new Promise<WebResearchResult>((resolve) => {
          resolveRun = resolve;
        });
      }
    );

    await renderApp();

    // Nothing has been opened yet, and the removed rail's surfaces are absent.
    expect(screen.queryByTestId('document-run-banner')).not.toBeInTheDocument();
    expect(screen.queryByTestId('research-query-input')).toBeNull();
    expect(screen.queryByTestId('web-search-mode-pill')).toBeNull();

    submitFromUniversalInput('research', QUERY);

    // 1. A draft artifact was initialised for the query.
    await waitFor(() => {
      expect(ipc.createResearchPage).toHaveBeenCalledWith(QUERY, null);
    });

    // 2. The view transitioned to the center workspace with the new artifact loaded.
    await waitFor(() => {
      expect(screen.getByTestId('document-run-banner')).toBeInTheDocument();
    });
    expect(screen.getByTestId('document-run-banner')).toHaveAttribute(
      'data-run-kind',
      'research'
    );

    // The banner lives INSIDE the full-width center document canvas.
    const banner = screen.getByTestId('document-run-banner');
    expect(banner.closest('.max-w-5xl') || banner.closest('.max-w-4xl')).not.toBeNull();
    expect(screen.getByTestId('center-workspace').contains(banner)).toBe(true);

    // The pipeline ran against the newly created page.
    expect(ipc.runResearchIntoPage).toHaveBeenCalledWith(
      RESEARCH_ARTIFACT_ID,
      QUERY,
      'auto',
      expect.any(Function)
    );

    // 3. Stages stream in live, in order.
    await act(async () => {
      pushStage!({ stage: 'searching', message: `Searching web for "${QUERY}" via SearXNG...` });
    });
    await waitFor(() => {
      expect(screen.getByTestId('document-run-message')).toHaveTextContent('via SearXNG');
    });

    await act(async () => {
      pushStage!({ stage: 'scraped', message: 'Scraped 2 sources...' });
    });
    await waitFor(() => {
      expect(screen.getByTestId('document-run-message')).toHaveTextContent('Scraped 2 sources');
    });

    await act(async () => {
      pushStage!({ stage: 'synthesizing', message: 'Synthesizing research...' });
    });
    await waitFor(() => {
      expect(screen.getByTestId('document-run-message')).toHaveTextContent('Synthesizing research');
    });

    // 4. Completion flips the banner and reloads the body with the grounded document.
    currentBody = GROUNDED_MARKDOWN;
    await act(async () => {
      resolveRun!(RESEARCH_RESULT);
    });

    await waitFor(() => {
      expect(screen.getByTestId('document-run-message')).toHaveTextContent('Research complete');
    });
    expect(screen.getByTestId('btn-dismiss-document-run')).toBeInTheDocument();

    await waitFor(() => {
      const editor = screen.getByTestId('textarea-editor') as HTMLTextAreaElement;
      expect(editor.value).toContain('## Synthesis');
      expect(editor.value).toContain('[Source 1]');
    });
  });

  // ---------------------------------------------------------------------------
  // 2. Plan mode streams tokens into the center editor
  // ---------------------------------------------------------------------------
  it('plan mode creates a "Plan: <Goal>" draft and streams tokens into the center editor', async () => {
    currentArtifactId = PLAN_ARTIFACT_ID;
    currentTitle = `Plan: ${GOAL}`;
    currentBody = `# Plan: ${GOAL}\n`;

    vi.mocked(ipc.createPage).mockResolvedValue({
      id: PLAN_ARTIFACT_ID,
      artifact_type: 'note',
      authorship: 'user',
      title: `Plan: ${GOAL}`,
      content_md: '',
      version: 1,
      is_unfinished_draft: true,
      created_at: Date.now(),
      updated_at: Date.now(),
    } as any);
    vi.mocked(ipc.saveArtifact).mockResolvedValue({} as any);

    // Hold the token stream open so tokens can be pushed by hand.
    let pushChunk: ((chunk: any) => void) | null = null;
    let finishStream: (() => void) | null = null;
    vi.mocked(ipc.streamAdaptiveRun).mockImplementation(
      async (_raw, _mode, _modelId, onChunk) => {
        pushChunk = onChunk;
        await new Promise<void>((resolve) => {
          finishStream = resolve;
        });
      }
    );

    await renderApp();
    submitFromUniversalInput('plan', GOAL);

    // 1. A `Plan: <Goal>` draft artifact was initialised.
    await waitFor(() => {
      expect(ipc.createPage).toHaveBeenCalledWith(null, `Plan: ${GOAL}`);
    });

    // 2. FocusView mounted; the plan banner is live inside the center canvas.
    await waitFor(() => {
      expect(screen.getByTestId('document-run-banner')).toHaveAttribute('data-run-kind', 'plan');
    });
    expect(
      screen.getByTestId('center-workspace').contains(screen.getByTestId('document-run-banner'))
    ).toBe(true);

    // 3. Tokens render live in the center document canvas as they arrive.
    await act(async () => {
      pushChunk!({ type: 'text_token', payload: '## Phase 1\n' });
    });
    await waitFor(() => {
      expect(screen.getByTestId('document-live-stream')).toHaveTextContent('## Phase 1');
    });

    await act(async () => {
      pushChunk!({ type: 'text_token', payload: 'Ship the shell refactor.' });
    });
    await waitFor(() => {
      expect(screen.getByTestId('document-live-stream')).toHaveTextContent('Ship the shell refactor.');
    });

    // 4. The finished plan is persisted back into the artifact.
    await act(async () => {
      finishStream!();
    });
    await waitFor(() => {
      expect(ipc.saveArtifact).toHaveBeenCalledWith(
        PLAN_ARTIFACT_ID,
        expect.stringContaining('Ship the shell refactor.'),
        `Plan: ${GOAL}`,
        null
      );
    });
  });

  // ---------------------------------------------------------------------------
  // 3. Structural: full-width center workspace, no right-hand drawer
  // ---------------------------------------------------------------------------
  it('the center workspace spans full width with no right-rail drawer anywhere', async () => {
    await renderApp();

    const center = screen.getByTestId('center-workspace');
    const sidebar = document.querySelector('aside');

    // The workspace takes all remaining width between the sidebar and the border.
    expect(sidebar).not.toBeNull();
    expect(center.className).toContain('flex-1');
    expect(center.className).toContain('min-w-0');
    // No right-hand margin/padding reserved for a rail.
    expect(center.className).not.toMatch(/\b(mr|pr)-\d/);

    // The workspace sits IMMEDIATELY after the sidebar — nothing is wedged between them.
    expect(sidebar!.nextElementSibling).toBe(center);
    // And nothing drawer-like is rendered to its right.
    expect(center.nextElementSibling?.tagName).not.toBe('ASIDE');

    // The removed rail's surfaces no longer exist anywhere in the shell.
    expect(screen.queryByTestId('research-query-input')).toBeNull();
    expect(screen.queryByTestId('web-search-mode-pill')).toBeNull();
    expect(screen.queryByTestId('panel-web-search-mode-pill')).toBeNull();
  });

  // ---------------------------------------------------------------------------
  // 4. Scheduled Feeds relocated to the Left Sidebar
  // ---------------------------------------------------------------------------
  it('the Left Sidebar routes to the Scheduled Feeds dashboard in the center workspace', async () => {
    await renderApp();

    fireEvent.click(screen.getByTestId('nav-scheduled-feeds'));

    await waitFor(() => {
      expect(screen.getByTestId('scheduled-section-research')).toBeInTheDocument();
    });

    // The dashboard renders inside the full-width center workspace, not a drawer.
    const center = screen.getByTestId('center-workspace');
    expect(center.contains(screen.getByTestId('scheduled-section-research'))).toBe(true);
    expect(screen.getByTestId('scheduled-section-plan')).toBeInTheDocument();
    expect(center.nextElementSibling?.tagName).not.toBe('ASIDE');
  });

});
