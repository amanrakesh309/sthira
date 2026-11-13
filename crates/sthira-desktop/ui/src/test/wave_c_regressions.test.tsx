import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { FocusView } from '../components/FocusView';
import { HomeView } from '../components/HomeView';
import type { HomeStateView } from '../types/sthira';

const mockInvoke = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}));

describe('Wave C UI Regressions (F-B5-022 Title Auto-Sizing, Wrapping & Home Card Metadata)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    sessionStorage.clear();

    mockInvoke.mockImplementation(async (cmd: string, args: Record<string, unknown>) => {
      switch (cmd) {
        case 'cmd_get_home_state':
          return {
            projects: [],
            recent_items: [],
            loose_captures: [],
          };
        case 'cmd_list_object_types':
          return [];
        case 'cmd_get_provider_catalog':
          return [];
        case 'cmd_get_search_providers':
          return [];
        case 'cmd_get_vault_session_key':
          return 'wave-c-session';
        case 'cmd_get_workspace_tree':
          return { projects: [], orphaned_artifacts: [] };
        case 'cmd_filter_existing_artifacts':
          return [];
        case 'cmd_get_project_history':
          return { project_id: (args?.project_id as string) || 'proj-c', items: [] };
        case 'cmd_get_document_conversation':
          return null;
        case 'cmd_get_artifact_context':
          return {
            artifact: {
              id: (args?.artifact_id as string) || 'art-wave-c',
              artifact_type: 'note',
              authorship: 'user',
              title: 'Initial Document Title',
              content_md: '# Initial Document Title\n\nInitial document content.',
              content_hash: 'h-init',
              version: 1,
              is_unfinished_draft: false,
              created_at: 1000,
              updated_at: 1000,
              derivation_type: null,
              derived_from_artifact_id: null,
            },
            project_id: (args?.project_id as string) || 'proj-c',
            epistemic_nodes: [],
            pending_proposals: [],
            is_unfinished_draft: false,
            conflicts: [],
          };
        case 'cmd_save_artifact':
          return {
            artifact: {
              id: (args?.artifact_id as string) || 'art-wave-c',
              artifact_type: 'note',
              authorship: 'user',
              title: args?.title as string,
              content_md: args?.content as string,
              content_hash: 'h-updated',
              version: 2,
              is_unfinished_draft: false,
              created_at: 1000,
              updated_at: 2000,
              derivation_type: null,
              derived_from_artifact_id: null,
            },
            mirror_synced: true,
            mirror_sync_error: null,
            unchanged: false,
            repaired_mirror_only: false,
          };
        default:
          return null;
      }
    });
  });

  // ---------------------------------------------------------------------------
  // 1. FocusView Document Title Auto-sizing, Multiline Wrapping & Newline Normalization
  // ---------------------------------------------------------------------------
  it('renders document title in auto-resizing textarea with newline normalization and breadcrumb activation', async () => {
    const user = userEvent.setup();

    render(
      <FocusView
        projectId="proj-c"
        artifactId="art-wave-c"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    // Title element must be a <textarea> with data-testid="input-artifact-title"
    const titleTextarea = await screen.findByTestId('input-artifact-title');
    expect(titleTextarea.tagName.toLowerCase()).toBe('textarea');
    expect(titleTextarea).toHaveAttribute('aria-label', 'Document title');
    expect(titleTextarea).toHaveValue('Initial Document Title');

    // Breadcrumb button exists and displays current title
    const breadcrumb = screen.getByRole('button', {
      name: /Edit document title: Initial Document Title/,
    });
    expect(breadcrumb).toBeInTheDocument();

    // Clicking breadcrumb focuses and selects the textarea
    const selectSpy = vi.spyOn(titleTextarea as HTMLTextAreaElement, 'select');
    await user.click(breadcrumb);
    expect(document.activeElement).toBe(titleTextarea);
    expect(selectSpy).toHaveBeenCalled();

    // Pasting or typing text with newlines normalizes newlines to single spaces
    // to prevent SQLite / markdown header breakage
    fireEvent.paste(titleTextarea, {
      clipboardData: {
        getData: (type: string) => (type === 'text/plain' ? 'Multi\r\nLine\nDocument\rTitle' : ''),
      },
    });

    await waitFor(() => {
      expect(titleTextarea).toHaveValue('Multi Line Document Title');
    });

    // Shortening the title
    fireEvent.change(titleTextarea, { target: { value: 'Short' } });
    expect(titleTextarea).toHaveValue('Short');
  });

  it('pressing Enter in title textarea navigates to editor body, while IME Enter composition does not', async () => {
    render(
      <FocusView
        projectId="proj-c"
        artifactId="art-wave-c"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    const titleTextarea = (await screen.findByTestId(
      'input-artifact-title'
    )) as HTMLTextAreaElement;
    const editor = await screen.findByRole('textbox', { name: 'Document content' });

    titleTextarea.focus();
    expect(document.activeElement).toBe(titleTextarea);

    // When IME composition is active (e.g. Japanese/Chinese character selection),
    // Enter key should NOT navigate away from the title textarea
    fireEvent.keyDown(titleTextarea, {
      key: 'Enter',
      code: 'Enter',
      isComposing: true,
    });
    expect(document.activeElement).toBe(titleTextarea);

    // Regular Enter key navigates focus to the rich editor body
    fireEvent.keyDown(titleTextarea, {
      key: 'Enter',
      code: 'Enter',
      isComposing: false,
    });
    expect(document.activeElement).toBe(editor);
  });

  it('auto-adjusts textarea height based on scrollHeight without triggering unintended saves', async () => {
    const onTitleUpdate = vi.fn();

    render(
      <FocusView
        projectId="proj-c"
        artifactId="art-wave-c"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
        onTitleUpdate={onTitleUpdate}
      />
    );

    const titleTextarea = (await screen.findByTestId(
      'input-artifact-title'
    )) as HTMLTextAreaElement;

    // Simulate multi-line content causing large scrollHeight
    Object.defineProperty(titleTextarea, 'scrollHeight', {
      configurable: true,
      value: 120,
    });

    fireEvent.change(titleTextarea, {
      target: {
        value: 'Very long document title that wraps onto multiple lines across responsive widths',
      },
    });

    expect(onTitleUpdate).toHaveBeenCalledWith(
      'art-wave-c',
      'Very long document title that wraps onto multiple lines across responsive widths'
    );
    expect(titleTextarea.style.height).toBe('120px');

    // When shortened, scrollHeight shrinks back down
    Object.defineProperty(titleTextarea, 'scrollHeight', {
      configurable: true,
      value: 46,
    });

    fireEvent.change(titleTextarea, {
      target: {
        value: 'Short Title',
      },
    });

    expect(titleTextarea.style.height).toBe('46px');

    // No auto-save command should have been invoked merely by editing the title in the textarea
    expect(
      mockInvoke.mock.calls.filter(([cmd]) => cmd === 'cmd_save_artifact').length
    ).toBe(0);
  });

  // ---------------------------------------------------------------------------
  // 2. HomeView Project Card Count Display & Ellipsis Isolation
  // ---------------------------------------------------------------------------
  it('HomeView renders structured metadata with unclipped item counts for 0, 1, and multiple items', async () => {
    const user = userEvent.setup();
    const onOpenProject = vi.fn();

    const homeState: HomeStateView = {
      projects: [
        {
          id: 'proj-0',
          name: 'Empty Project',
          purpose: 'Just created',
          category: 'Scratchpad',
          item_count: 0,
          icon_type: 'folder',
          last_substantive_at: 1000,
        },
        {
          id: 'proj-1',
          name: 'Single Note Project',
          purpose: 'Has exactly one artifact',
          category: 'Personal',
          item_count: 1,
          icon_type: 'folder',
          last_substantive_at: 2000,
        },
        {
          id: 'proj-long',
          name: 'Distributed Systems & WAL Storage Checkpoint Engineering',
          purpose: 'Large multi-topic research repository',
          category: 'Very Long Hierarchical Category Name That Could Otherwise Clip',
          item_count: 42,
          icon_type: 'folder',
          last_substantive_at: 3000,
        },
      ],
      recent_items: [],
      loose_captures: [],
    };

    render(
      <HomeView
        homeState={homeState}
        loadStatus="ready"
        onOpenProject={onOpenProject}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onRefresh={vi.fn()}
        onOpenFlow={vi.fn()}
        onOpenSettings={vi.fn()}
      />
    );

    // Verify all item count elements are rendered with data-testid="project-item-count"
    const countBadges = screen.getAllByTestId('project-item-count');
    expect(countBadges).toHaveLength(3);

    // proj-0: "0 items"
    expect(countBadges[0]).toHaveTextContent('0 items');
    // proj-1: "1 item" (singular)
    expect(countBadges[1]).toHaveTextContent('1 item');
    // proj-long: "42 items" (plural)
    expect(countBadges[2]).toHaveTextContent('42 items');

    // Category and separator are rendered in separate spans
    const categories = document.querySelectorAll('.home-card-category');
    expect(categories).toHaveLength(3);
    expect(categories[0]).toHaveTextContent('Scratchpad');
    expect(categories[1]).toHaveTextContent('Personal');
    expect(categories[2]).toHaveTextContent(
      'Very Long Hierarchical Category Name That Could Otherwise Clip'
    );

    const separators = document.querySelectorAll('.home-card-meta-separator');
    expect(separators).toHaveLength(3);
    expect(separators[0]).toHaveTextContent('·');

    // Clicking a project card activates onOpenProject
    await user.click(screen.getByRole('button', { name: /Open project: Single Note Project/ }));
    expect(onOpenProject).toHaveBeenCalledWith('proj-1');
  });

  // ---------------------------------------------------------------------------
  // 3. Independent Review Reproductions & Element Lifecycle Regressions
  // ---------------------------------------------------------------------------
  it('attaches width observer after async title mount and remeasures without title edits', async () => {
    const observed: Element[] = [];
    const observers: any[] = [];
    class ReviewObserver {
      constructor(public callback: ResizeObserverCallback) {
        observers.push(this);
      }
      observe(el: Element) {
        observed.push(el);
      }
      unobserve() {}
      disconnect() {}
    }
    vi.stubGlobal('ResizeObserver', ReviewObserver);
    const height = vi.spyOn(HTMLTextAreaElement.prototype, 'scrollHeight', 'get').mockReturnValue(180);
    try {
      render(<FocusView projectId="proj-c" artifactId="art-wave-c" onBack={vi.fn()} onOpenCanvas={vi.fn()} />);
      const title = await screen.findByTestId('input-artifact-title');
      expect(title.style.height).toBe('180px');
      const attached = observed.includes(title) || observed.includes(title.parentElement!);
      expect(attached, 'The title or its width-bearing parent must be observed after the asynchronous document load mounts the textarea').toBe(true);
      expect(observers.length).toBeGreaterThan(0);
    } finally {
      height.mockRestore();
      vi.unstubAllGlobals();
    }
  });

  it('same-title reload sizes replacement textarea without mutating title or saving', async () => {
    vi.stubGlobal('ResizeObserver', class {
      observe() {}
      unobserve() {}
      disconnect() {}
    });
    const height = vi.spyOn(HTMLTextAreaElement.prototype, 'scrollHeight', 'get').mockReturnValue(180);
    try {
      const props = { projectId: 'proj-c', artifactId: 'art-wave-c', onBack: vi.fn(), onOpenCanvas: vi.fn() };
      const view = render(<FocusView {...props} />);
      const first = await screen.findByTestId('input-artifact-title');
      expect(first.style.height).toBe('180px');
      const loads = mockInvoke.mock.calls.filter(([cmd]) => cmd === 'cmd_get_artifact_context').length;
      view.rerender(
        <FocusView
          {...props}
          documentRun={{
            artifactId: 'art-wave-c',
            refreshToken: 1,
            status: 'done',
            kind: 'research',
            query: 'Review',
            message: 'Refresh',
            streamedMarkdown: '',
          }}
        />
      );
      await waitFor(() =>
        expect(mockInvoke.mock.calls.filter(([cmd]) => cmd === 'cmd_get_artifact_context').length).toBeGreaterThan(loads)
      );
      const replacement = await screen.findByTestId('input-artifact-title');
      expect(replacement).not.toBe(first);
      expect(
        replacement.style.height,
        'The newly mounted title must be sized even when its saved text did not change'
      ).toBe('180px');
      expect(mockInvoke.mock.calls.filter(([cmd]) => cmd === 'cmd_save_artifact')).toHaveLength(0);
    } finally {
      height.mockRestore();
      vi.unstubAllGlobals();
    }
  });

  it('remeasures title height on container width change and window resize events', async () => {
    let observerCallback: ResizeObserverCallback | null = null;
    class ActiveResizeObserver {
      constructor(public callback: ResizeObserverCallback) {
        observerCallback = callback;
      }
      observe() {}
      unobserve() {}
      disconnect() {}
    }
    vi.stubGlobal('ResizeObserver', ActiveResizeObserver);
    let mockHeight = 60;
    const height = vi.spyOn(HTMLTextAreaElement.prototype, 'scrollHeight', 'get').mockImplementation(() => mockHeight);
    try {
      render(<FocusView projectId="proj-c" artifactId="art-wave-c" onBack={vi.fn()} onOpenCanvas={vi.fn()} />);
      const title = await screen.findByTestId('input-artifact-title');
      expect(title.style.height).toBe('60px');

      // Simulate container resize causing text wrap (narrowing container increases height)
      mockHeight = 120;
      expect(observerCallback).not.toBeNull();
      observerCallback!([
        {
          target: title.parentElement!,
          contentRect: { width: 500, height: 120, top: 0, bottom: 120, left: 0, right: 500, x: 0, y: 0, toJSON: () => ({}) },
        } as unknown as ResizeObserverEntry,
      ], {} as ResizeObserver);

      expect(title.style.height).toBe('120px');

      // Simulate window resize
      mockHeight = 150;
      window.dispatchEvent(new Event('resize'));
      expect(title.style.height).toBe('150px');
    } finally {
      height.mockRestore();
      vi.unstubAllGlobals();
    }
  });

  it('disconnects ResizeObserver cleanly when unmounted', async () => {
    let disconnected = false;
    class TrackingObserver {
      observe() {}
      unobserve() {}
      disconnect() {
        disconnected = true;
      }
    }
    vi.stubGlobal('ResizeObserver', TrackingObserver);
    try {
      const view = render(<FocusView projectId="proj-c" artifactId="art-wave-c" onBack={vi.fn()} onOpenCanvas={vi.fn()} />);
      await screen.findByTestId('input-artifact-title');
      view.unmount();
      expect(disconnected).toBe(true);
    } finally {
      vi.unstubAllGlobals();
    }
  });
});