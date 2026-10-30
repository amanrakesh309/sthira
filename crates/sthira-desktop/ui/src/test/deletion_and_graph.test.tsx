import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { FocusView } from '../components/FocusView';
import { ProjectDeletionModal } from '../components/ProjectDeletionModal';
import { GraphView } from '../views/GraphView';
import type { KnowledgeGraphData, ProjectDeletionImpact } from '../types/sthira';

// Mock Tauri invoke
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe('Phase 9.3: Title Persistence, Deletion Semantics & Knowledge Graph Tests', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Test 1: Title edit -> Save -> Assert cmd_save_artifact receives new title
  // -------------------------------------------------------------------------
  it('Title edit -> Save -> Assert cmd_save_artifact receives new title and updates state', async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_get_artifact_context') {
        return {
          artifact: {
            id: 'art-title-test-1',
            artifact_type: 'note',
            authorship: 'user',
            title: 'Original Title',
            content_md: '# Original Title\n\nSome body text.',
            source_url: null,
            content_hash: 'hash-abc',
            version: 1,
            file_path: null,
            last_synced_hash: null,
            is_unfinished_draft: false,
            deleted_at: null,
            created_at: 1000,
            updated_at: 1000,
          },
          epistemic_nodes: [],
          pending_proposals: [],
          project_id: 'proj-demo',
          is_unfinished_draft: false,
        };
      }
      if (cmd === 'cmd_get_resume_brief') {
        return {
          project_id: 'proj-demo',
          level: 1,
          text: 'Project test summary.',
          created_at: 1000,
        };
      }
      if (cmd === 'cmd_get_project_history') {
        return {
          project_id: 'proj-demo',
          entries: [],
          checkpoint_count: 0,
        };
      }
      if (cmd === 'cmd_save_artifact') {
        return {
          id: args.artifactId || args.artifact_id,
          artifact_type: 'note',
          authorship: 'user',
          title: args.title || 'Extracted Title',
          content_md: args.contentMd || args.content_md,
          source_url: null,
          content_hash: 'hash-updated',
          version: 2,
          file_path: null,
          last_synced_hash: null,
          is_unfinished_draft: false,
          deleted_at: null,
          created_at: 1000,
          updated_at: 2000,
        };
      }
      return null;
    });

    render(
      <FocusView
        projectId="proj-demo"
        artifactId="art-title-test-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    // Wait for the title input field and editor to populate
    const titleInput = await screen.findByTestId('input-artifact-title');
    expect(titleInput).toBeInTheDocument();
    await waitFor(() => {
      expect(titleInput).toHaveValue('Original Title');
    });

    // Edit title
    await user.clear(titleInput);
    await user.type(titleInput, 'Advanced Neural Architecture');

    // Edit content
    const editor = screen.getByTestId('textarea-editor');
    await user.clear(editor);
    await user.type(editor, '# Advanced Neural Architecture\n\nRefined content specification.');

    // Save
    const saveBtn = screen.getByTestId('btn-save-artifact');
    await user.click(saveBtn);

    // Verify cmd_save_artifact received the explicit title parameter alongside content
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_save_artifact',
        expect.objectContaining({
          artifactId: 'art-title-test-1',
          title: 'Advanced Neural Architecture',
          contentMd: '# Advanced Neural Architecture\n\nRefined content specification.',
          projectId: 'proj-demo',
        })
      );
    });

    // Assert UI title input reflects the updated title
    await waitFor(() => {
      expect(titleInput).toHaveValue('Advanced Neural Architecture');
    });
  });

  // -------------------------------------------------------------------------
  // Test 2: Project delete dialog -> Choose cascade vs detach -> Assert correct IPC payload
  // -------------------------------------------------------------------------
  it('Project delete dialog -> inspect impact and execute safe default vs cascade delete', async () => {
    const user = userEvent.setup();

    const mockImpact: ProjectDeletionImpact = {
      project_id: 'proj-delete-target',
      project_name: 'Project Target',
      exclusive_artifacts: [
        { id: 'art-ex-1', title: 'Exclusive Note 1', artifact_type: 'note', updated_at: 1000 },
        { id: 'art-ex-2', title: 'Exclusive Note 2', artifact_type: 'note', updated_at: 1100 },
      ],
      shared_artifacts: [
        { id: 'art-sh-1', title: 'Shared Note 1', artifact_type: 'note', updated_at: 1200 },
      ],
    };

    mockInvoke.mockImplementation(async (cmd: string, _args: any) => {
      if (cmd === 'cmd_inspect_project_deletion') {
        return mockImpact;
      }
      if (cmd === 'cmd_delete_project') {
        return null;
      }
      return null;
    });

    const onClose = vi.fn();
    const onDeleted = vi.fn();

    // 2a: Test Safe Default (keep unassociated: cascade_exclusive = false)
    const { rerender } = render(
      <ProjectDeletionModal
        isOpen={true}
        projectId="proj-delete-target"
        onClose={onClose}
        onDeleted={onDeleted}
      />
    );

    // Wait for inspect data to load
    await waitFor(() => {
      expect(screen.getAllByText(/exclusive/i).length).toBeGreaterThan(0);
      expect(screen.getAllByText(/shared/i).length).toBeGreaterThan(0);
    });

    // Verify radio safe default is selected
    const radioKeep = screen.getByTestId('radio-keep-unassociated') as HTMLInputElement;
    expect(radioKeep.checked).toBe(true);

    // Click confirm delete button
    const confirmBtn = screen.getByTestId('btn-confirm-delete-project');
    await user.click(confirmBtn);

    // Verify cmd_delete_project was called with cascadeExclusive: false
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_delete_project',
        expect.objectContaining({
          projectId: 'proj-delete-target',
          cascadeExclusive: false,
        })
      );
    });
    expect(onDeleted).toHaveBeenCalled();

    // 2b: Test Cascade Option (cascade_exclusive = true)
    vi.clearAllMocks();
    rerender(
      <ProjectDeletionModal
        isOpen={true}
        projectId="proj-delete-target"
        onClose={onClose}
        onDeleted={onDeleted}
      />
    );

    // Select cascade radio
    const radioCascade = await screen.findByTestId('radio-cascade-exclusive');
    await user.click(radioCascade);

    // Click confirm
    const confirmBtn2 = screen.getByTestId('btn-confirm-delete-project');
    await user.click(confirmBtn2);

    // Verify cmd_delete_project was called with cascadeExclusive: true
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_delete_project',
        expect.objectContaining({
          projectId: 'proj-delete-target',
          cascadeExclusive: true,
        })
      );
    });
  });

  // -------------------------------------------------------------------------
  // Test 3: Graph view renders nodes and edges received from cmd_get_knowledge_graph
  // -------------------------------------------------------------------------
  it('Graph view renders nodes, edges, filter pills, and search controls', async () => {
    const user = userEvent.setup();

    const mockGraphData: KnowledgeGraphData = {
      nodes: [
        {
          id: 'proj-hub-1',
          label: 'System Core Project',
          kind: 'project',
          item_count: 2,
          color: '#282C25',
          icon: '📂',
        },
        {
          id: 'art-note-1',
          label: 'Architecture Design',
          kind: 'artifact',
          authorship: 'user',
          draft_status: false,
          color: '#F5F3EC',
          icon: '📄',
          project_id: 'proj-hub-1',
        },
        {
          id: 'art-idea-1',
          label: 'Vector Cache Idea',
          kind: 'object',
          object_type: 'idea',
          authorship: 'user',
          draft_status: false,
          color: '#F59E0B',
          icon: '💡',
          project_id: 'proj-hub-1',
        },
      ],
      edges: [
        {
          source: 'art-note-1',
          target: 'proj-hub-1',
          relationship: 'belongs_to',
        },
        {
          source: 'art-idea-1',
          target: 'art-note-1',
          relationship: 'supports',
        },
      ],
    };

    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'cmd_get_knowledge_graph') {
        return mockGraphData;
      }
      return null;
    });

    const onOpenProjectMock = vi.fn();
    const onOpenArtifactMock = vi.fn();

    render(
      <GraphView
        onOpenProject={onOpenProjectMock}
        onOpenArtifact={onOpenArtifactMock}
        onBack={vi.fn()}
      />
    );

    // Verify container and canvas exist
    expect(await screen.findByTestId('graph-view-container')).toBeInTheDocument();
    expect(screen.getByTestId('graph-canvas')).toBeInTheDocument();

    // Verify node and edge counters
    const nodeCount = await screen.findByTestId('graph-node-count');
    const edgeCount = screen.getByTestId('graph-edge-count');
    expect(nodeCount).toHaveTextContent('3');
    expect(edgeCount).toHaveTextContent('2');

    // Verify filter pills exist
    expect(screen.getByTestId('filter-projects')).toBeInTheDocument();
    expect(screen.getByTestId('filter-pages')).toBeInTheDocument();
    expect(screen.getByTestId('filter-ideas')).toBeInTheDocument();
    expect(screen.getByTestId('filter-tasks')).toBeInTheDocument();
    expect(screen.getByTestId('filter-sources')).toBeInTheDocument();

    // Verify search input filters nodes
    const searchInput = screen.getByTestId('graph-search-input');
    await user.type(searchInput, 'Vector');
    expect(searchInput).toHaveValue('Vector');

    // Toggle filter: hide projects -> node count decreases
    const filterProjectsBtn = screen.getByTestId('filter-projects');
    await user.click(filterProjectsBtn);

    await waitFor(() => {
      expect(screen.getByTestId('graph-node-count')).toHaveTextContent('2');
    });
  });
});
