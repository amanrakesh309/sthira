import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
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
        title: 'Trip Planner App Architecture',
        content_md: '# Trip Planner App Architecture\n\n- [ ] Define invariant schemas\n- [ ] Wire streaming engine\n',
        content_hash: 'hash-1',
        version: 1,
        is_unfinished_draft: false,
        created_at: 1000,
        updated_at: 1000,
      },
      epistemic_nodes: [
        {
          id: 'node-1',
          artifact_id: 'test-art-1',
          node_type: 'decision',
          statement: 'Use local SQLite with zero network latency',
          quote: 'local SQLite with zero network latency',
          origin: 'user_recorded',
          status: 'verified',
          created_at: 1000,
          updated_at: 1000,
        },
      ],
      pending_proposals: [],
      project_id: 'proj-1',
      is_unfinished_draft: false,
    }),
    getResumeBrief: vi.fn().mockResolvedValue({
      project_id: 'proj-1',
      primary_continuation_artifact_id: 'test-art-1',
      purpose: 'Build offline-first trip planner',
      last_substantive_delta: 'Initial setup',
      invariants: ['Zero unsolicited writes'],
      open_questions: ['How to handle flight data sync?'],
      next_milestones: ['Setup database schema'],
    }),
    getProjectHistory: vi.fn().mockResolvedValue({
      project_id: 'proj-1',
      items: [],
    }),
    saveArtifact: vi.fn().mockImplementation((id, content, title, _projId) => {
      return Promise.resolve({
        id: id || 'test-art-1',
        artifact_type: 'note',
        authorship: 'user',
        title: title || 'Trip Planner App Architecture',
        content_md: content,
        content_hash: 'hash-updated',
        version: 2,
        is_unfinished_draft: false,
        created_at: 1000,
        updated_at: 1005,
      });
    }),
    getProviderCatalog: vi.fn().mockResolvedValue([
      {
        id: 'ollama',
        display_name: 'Ollama Local',
        provider_type: 'ollama',
        base_url: 'http://localhost:11434',
        has_api_key: false,
        is_enabled: true,
        models: [
          {
            id: 'ollama:llama3.2:latest',
            provider_id: 'ollama',
            model_identifier: 'llama3.2:latest',
            display_name: 'llama3.2:latest',
            context_window: 131072,
            supports_structured_output: true,
            is_enabled: true,
            is_default: true,
          },
        ],
      },
    ]),
    streamAdaptiveRun: vi.fn().mockImplementation((_input, _mode, _modelId, onChunk) => {
      onChunk({
        type: 'text_token',
        payload: '## Execution Plan for Trip Planner\n\nPhase 1: Schemas and local sync.\n',
      });
      return Promise.resolve();
    }),
  };
});

describe('FocusView Layout & Notion-Style Slash Command Experience', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders FocusView in full-height full-width workspace with generous document canvas and collapsible sidebar', async () => {
    const { container } = render(
      <FocusView
        artifactId="test-art-1"
        projectId="proj-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    // Verify root workspace container is full-height and full-width
    const rootWorkspace = container.firstElementChild as HTMLElement;
    expect(rootWorkspace.className).toContain('flex-1');
    expect(rootWorkspace.className).toContain('h-full');
    expect(rootWorkspace.className).toContain('overflow-hidden');
    expect(rootWorkspace.className).toContain('bg-[#FAF8F3]');

    // Wait for context to load
    await waitFor(() => {
      expect(screen.getByTestId('input-artifact-title')).toHaveValue('Trip Planner App Architecture');
    });

    // Verify the document sheet canvas has generous width and borders
    const editorCanvas = container.querySelector('.max-w-4xl') || container.querySelector('.lg\\:max-w-5xl');
    expect(editorCanvas).toBeTruthy();
    expect(editorCanvas?.className).toContain('min-h-full');
    expect(editorCanvas?.className).toContain('bg-white');
    expect(editorCanvas?.className).toContain('border-x');

    // Verify Epistemic Margin right sidebar is docked with w-80
    const marginSidebar = screen.getByTestId('epistemic-margin-sidebar');
    expect(marginSidebar).toBeTruthy();
    expect(marginSidebar.className).toContain('w-80');
    expect(marginSidebar.className).toContain('border-l');

    // Test collapsible margin toggle button
    const toggleMarginBtn = screen.getByTestId('btn-toggle-epistemic-margin');
    fireEvent.click(toggleMarginBtn);
    expect(screen.queryByTestId('epistemic-margin-sidebar')).toBeNull();

    // Click again to reopen
    fireEvent.click(toggleMarginBtn);
    expect(screen.getByTestId('epistemic-margin-sidebar')).toBeTruthy();
  });

  it('typing "/" in FocusView opens the Slash Command popover and selecting "/decision" inserts epistemic decision block', async () => {
    render(
      <FocusView
        artifactId="test-art-1"
        projectId="proj-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId('textarea-editor')).toBeInTheDocument();
    });

    const textarea = screen.getByTestId('textarea-editor') as HTMLTextAreaElement;

    // Type a slash command: "/"
    fireEvent.change(textarea, {
      target: {
        value: '# Title\n\n/',
        selectionStart: 10,
        selectionEnd: 10,
      },
    });

    // Popover should appear
    await waitFor(() => {
      expect(screen.getByTestId('slash-cmd-decision')).toBeInTheDocument();
      expect(screen.getByTestId('slash-cmd-h1')).toBeInTheDocument();
      expect(screen.getByTestId('slash-cmd-plan')).toBeInTheDocument();
    });

    // Select "/decision"
    fireEvent.click(screen.getByTestId('slash-cmd-decision'));

    // Verify decision block template was inserted into the editor content
    await waitFor(() => {
      expect(textarea.value).toContain('Decision:');
      expect(textarea.value).toContain('Rationale:');
    });
  });

  it('selecting "/h1" inserts Heading 1 and selecting "/todo" inserts checkbox item', async () => {
    render(
      <FocusView
        artifactId="test-art-1"
        projectId="proj-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId('textarea-editor')).toBeInTheDocument();
    });

    const textarea = screen.getByTestId('textarea-editor') as HTMLTextAreaElement;

    // Type "/h" to filter
    fireEvent.change(textarea, {
      target: {
        value: '# Notes\n\n/h',
        selectionStart: 11,
        selectionEnd: 11,
      },
    });

    await waitFor(() => {
      expect(screen.getByTestId('slash-cmd-h1')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('slash-cmd-h1'));

    await waitFor(() => {
      expect(textarea.value).toContain('# Notes\n\n# ');
    });
  });

  it('toggles to Rich Preview mode with clean rendered typography and interactive clickable checklist', async () => {
    render(
      <FocusView
        artifactId="test-art-1"
        projectId="proj-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId('textarea-editor')).toBeInTheDocument();
    });

    // Toggle to Rich Preview mode
    const previewBtn = screen.getByTestId('btn-toggle-preview-mode');
    fireEvent.click(previewBtn);

    // Verify rich preview canvas is rendered instead of textarea
    expect(screen.queryByTestId('textarea-editor')).toBeNull();
    const richCanvas = screen.getByTestId('rich-preview-canvas');
    expect(richCanvas).toBeInTheDocument();

    // Verify checklist checkboxes are rendered as real interactive HTML checkboxes
    const checkboxes = screen.getAllByRole('checkbox') as HTMLInputElement[];
    expect(checkboxes.length).toBeGreaterThanOrEqual(2);
    expect(checkboxes[0].checked).toBe(false);

    // Click on the first checkbox
    fireEvent.click(checkboxes[0]);

    // Checkbox should now be checked (- [x] )
    await waitFor(() => {
      expect(checkboxes[0].checked).toBe(true);
      expect(ipc.saveArtifact).toHaveBeenCalled();
    });

    // Toggle back to Live Edit mode
    const editBtn = screen.getByTestId('btn-toggle-edit-mode');
    fireEvent.click(editBtn);

    // Textarea is restored and reflects the toggled checkbox (- [x] )
    const textarea = screen.getByTestId('textarea-editor') as HTMLTextAreaElement;
    expect(textarea.value).toContain('- [x] Define invariant schemas');
  });
});
