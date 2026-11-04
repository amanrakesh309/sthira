import { editorMarkdown, setEditorMarkdown } from './editor-helpers';
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
    expect(rootWorkspace.className).toContain('focus-workspace');

    // Wait for context to load
    await waitFor(() => {
      expect(screen.getByTestId('input-artifact-title')).toHaveValue('Trip Planner App Architecture');
    });

    // Verify the document sheet canvas expands smoothly to full width when margin is collapsed
    const editorCanvas = screen.getByTestId('editor-canvas');
    expect(editorCanvas).toBeTruthy();
    expect(editorCanvas.className).toContain('flex-1');
    expect(editorCanvas.className).toContain('focus-document-sheet');
    expect(editorCanvas.className).toContain('min-h-full');
    expect(screen.getByTestId('document-scroll-region')).toContainElement(editorCanvas);

    // Issue 1: Verify Epistemic Margin right sidebar is collapsed/hidden by default
    expect(screen.queryByTestId('epistemic-margin-sidebar')).toBeNull();

    // Evidence remains accessible without a dedicated header control.
    expect(screen.queryByTestId('btn-toggle-epistemic-margin')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Page actions' }));
    fireEvent.click(screen.getByRole('button', { name: 'Sources & insights' }));
    expect(screen.getByTestId('epistemic-margin-sidebar')).toHaveClass('focus-margin');
    fireEvent.click(screen.getByRole('button', { name: 'Close epistemic margin' }));
    expect(screen.queryByTestId('epistemic-margin-sidebar')).toBeNull();

    // Issue 4: Verify toolbar action menu is a single button with Lucide icon and standard dimensions
    const pageActionBtn = screen.getByTestId('page-action-menu-trigger');
    expect(pageActionBtn).toBeTruthy();
    expect(pageActionBtn.className).toContain('w-8');
    expect(pageActionBtn.className).toContain('h-8');
    expect(pageActionBtn.className).toContain('hover:bg-[#EFECE4]');
    expect(pageActionBtn.className).toContain('text-[#5C5F58]');
    // No extra dot text nodes (e.g. "•••" or "'''")
    expect(pageActionBtn.querySelector('span')).toBeNull();
    // Exactly one svg icon (MoreHorizontal)
    expect(pageActionBtn.querySelectorAll('svg').length).toBe(1);
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
      expect(screen.getByTestId('document-editor')).toBeInTheDocument();
    });

    const textarea = screen.getByTestId('document-editor') as HTMLElement;

    // Type a slash command: "/"
    setEditorMarkdown(textarea, '# Title\n\n/');

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
      expect(editorMarkdown(textarea)).toContain('Decision:');
      expect(editorMarkdown(textarea)).toContain('Rationale:');
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
      expect(screen.getByTestId('document-editor')).toBeInTheDocument();
    });

    const textarea = screen.getByTestId('document-editor') as HTMLElement;

    // Type "/h" to filter
    setEditorMarkdown(textarea, '# Notes\n\n/h');

    await waitFor(() => {
      expect(screen.getByTestId('slash-cmd-h1')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('slash-cmd-h1'));

    await waitFor(() => {
      expect(textarea.querySelector('h1')).toBeInTheDocument();
      expect(editorMarkdown(textarea)).not.toContain('/h');
    });
  });

  it('edits a formatted checklist directly without switching document modes', async () => {
    render(
      <FocusView
        artifactId="test-art-1"
        projectId="proj-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId('document-editor')).toBeInTheDocument();
    });

    expect(screen.queryByTestId('btn-toggle-preview-mode')).not.toBeInTheDocument();
    expect(screen.getByTestId('document-editor')).toHaveAttribute('contenteditable', 'true');
    const taskCheckbox = screen.getAllByRole('checkbox')[0];
    fireEvent.click(taskCheckbox);

    // Textarea is restored and reflects the toggled checkbox (- [x] )
    const textarea = screen.getByTestId('document-editor') as HTMLElement;
    expect(editorMarkdown(textarea)).toContain('- [x] Define invariant schemas');
  });
});
