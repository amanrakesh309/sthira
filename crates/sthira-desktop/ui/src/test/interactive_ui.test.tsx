import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { FocusView } from '../components/FocusView';
import { LeftSidebar } from '../components/LeftSidebar';
import ObjectTypeBuilderModal from '../components/objects/ObjectTypeBuilderModal';

// Mock Tauri invoke
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe('Autonomous Interactive UI Test Harness', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Test A: Type text into editor -> Click Save -> Assert invoke called with
  // matching save command and correct payload.
  // -------------------------------------------------------------------------
  it('Test A: Type text into editor -> Click Save -> calls cmd_save_artifact with correct payload', async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_get_artifact_context') {
        return {
          artifact: {
            id: 'art-demo-1',
            artifact_type: 'note',
            authorship: 'user',
            title: 'Initial Title',
            content_md: '# Initial Title\n\nOriginal draft text.',
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
          project_id: 'proj-123',
          is_unfinished_draft: false,
        };
      }
      if (cmd === 'cmd_get_resume_brief') {
        return {
          project_id: 'proj-123',
          level: 1,
          text: 'Project test summary.',
          created_at: 1000,
        };
      }
      if (cmd === 'cmd_get_project_history') {
        return {
          project_id: 'proj-123',
          entries: [],
          checkpoint_count: 0,
        };
      }
      if (cmd === 'cmd_save_artifact') {
        return {
          id: args.artifactId || args.artifact_id,
          artifact_type: 'note',
          authorship: 'user',
          title: 'Updated Test Note',
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
        projectId="proj-123"
        artifactId="art-demo-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    // Wait for the editor to load the initial content
    const editor = await screen.findByTestId('textarea-editor');
    expect(editor).toBeInTheDocument();
    await waitFor(() => {
      expect(editor).toHaveValue('# Initial Title\n\nOriginal draft text.');
    });

    // Clear and type new content
    await user.clear(editor);
    await user.type(editor, '# Updated Test Note\n\nAutonomous testing content verification.');

    // Click Save button
    const saveButton = screen.getByTestId('btn-save-artifact');
    expect(saveButton).toBeInTheDocument();
    await user.click(saveButton);

    // Verify cmd_save_artifact was invoked with matching parameters
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_save_artifact',
        expect.objectContaining({
          artifactId: 'art-demo-1',
          contentMd: '# Updated Test Note\n\nAutonomous testing content verification.',
          projectId: 'proj-123',
        })
      );
    });
  });

  // -------------------------------------------------------------------------
  // Test B: Click + New Project -> Fill name -> Submit -> Assert invoke called
  // with project creation command.
  // -------------------------------------------------------------------------
  it('Test B: Click + New Project -> Fill name -> Submit -> calls cmd_create_project', async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_get_workspace_tree') {
        return { projects: [], unassociated_pages: [] };
      }
      if (cmd === 'cmd_list_object_types') {
        return [];
      }
      if (cmd === 'cmd_get_object_type_counts') {
        return {};
      }
      if (cmd === 'cmd_create_project') {
        return {
          id: 'proj-created-999',
          name: args.name,
          purpose: args.purpose || '',
          pages: [],
          canvases: [],
          updated_at: 2000,
        };
      }
      return null;
    });

    const onOpenProjectMock = vi.fn();

    render(
      <LeftSidebar
        currentView="home"
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onOpenProject={onOpenProjectMock}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        isCollapsed={false}
        onToggleCollapse={vi.fn()}
      />
    );

    // Click "+ New Project" button in sidebar header
    const newProjectBtn = await screen.findByTestId('btn-new-project');
    await user.click(newProjectBtn);

    // Inline form should appear
    const nameInput = await screen.findByTestId('input-new-project-name');
    const purposeInput = screen.getByTestId('input-new-project-purpose');
    const submitBtn = screen.getByTestId('btn-submit-new-project');

    await user.type(nameInput, 'Project Hyperion');
    await user.type(purposeInput, 'Next generation testing system');
    await user.click(submitBtn);

    // Verify cmd_create_project was invoked with exact name & purpose
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('cmd_create_project', {
        name: 'Project Hyperion',
        purpose: 'Next generation testing system',
      });
    });

    // Verify project selection callback
    await waitFor(() => {
      expect(onOpenProjectMock).toHaveBeenCalledWith('proj-created-999');
    });
  });

  // -------------------------------------------------------------------------
  // Test C: Click + New Page under a project -> Assert invoke called with project_id
  // -------------------------------------------------------------------------
  it('Test C: Click + New Page under project -> calls cmd_create_page with matching project_id', async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation(async (cmd: string, _args: any) => {
      if (cmd === 'cmd_get_workspace_tree') {
        return {
          projects: [
            {
              id: 'proj-atlas-1',
              name: 'Project Atlas',
              purpose: 'Mapping the workspace',
              pages: [],
              canvases: [],
              updated_at: 1500,
            },
          ],
          unassociated_pages: [],
        };
      }
      if (cmd === 'cmd_list_object_types') {
        return [];
      }
      if (cmd === 'cmd_get_object_type_counts') {
        return {};
      }
      if (cmd === 'cmd_create_page') {
        return {
          id: 'page-new-777',
          artifact_type: 'note',
          authorship: 'user',
          title: 'Untitled Note',
          content_md: '# Untitled Note\n\n',
          source_url: null,
          content_hash: 'hash-new',
          version: 1,
          file_path: null,
          last_synced_hash: null,
          is_unfinished_draft: false,
          deleted_at: null,
          created_at: 2000,
          updated_at: 2000,
        };
      }
      return null;
    });

    const onOpenArtifactMock = vi.fn();

    render(
      <LeftSidebar
        activeProjectId="proj-atlas-1"
        currentView="focus"
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onOpenProject={vi.fn()}
        onOpenArtifact={onOpenArtifactMock}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        isCollapsed={false}
        onToggleCollapse={vi.fn()}
      />
    );

    // Wait for Project Atlas to appear
    expect(await screen.findByText('Project Atlas')).toBeInTheDocument();

    // Click "+ New Page" inside Project Atlas
    const newPageBtn = await screen.findByTestId('btn-new-page-in-project');
    await user.click(newPageBtn);

    // Verify cmd_create_page was invoked with project_id
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_create_page',
        expect.objectContaining({
          projectId: 'proj-atlas-1',
        })
      );
    });

    // Verify newly created page was auto-opened
    await waitFor(() => {
      expect(onOpenArtifactMock).toHaveBeenCalledWith('page-new-777', 'proj-atlas-1');
    });
  });

  // -------------------------------------------------------------------------
  // Test D: Open Object Type Modal -> Fill name and property -> Click Save ->
  // Assert invoke called with valid schema
  // -------------------------------------------------------------------------
  it('Test D: Fill object type modal -> Submit -> calls cmd_create_object_type with valid schema', async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_create_object_type') {
        return (args.typeDef || args.type_def).id;
      }
      return null;
    });

    const onCreatedMock = vi.fn();
    const onCloseMock = vi.fn();

    render(
      <ObjectTypeBuilderModal
        isOpen={true}
        onClose={onCloseMock}
        onTypeCreated={onCreatedMock}
      />
    );

    // Fill Display Name & ID
    const nameInput = screen.getByTestId('input-object-type-name');
    const idInput = screen.getByTestId('input-object-type-id');

    await user.type(nameInput, 'Experiment Log');
    // ID automatically derives or can be typed
    await user.clear(idInput);
    await user.type(idInput, 'experiment_log');

    // Add a custom property
    const addPropBtn = screen.getByTestId('btn-add-property');
    await user.click(addPropBtn);

    const propLabels = await screen.findAllByTestId('input-property-label');
    const propKeys = screen.getAllByTestId('input-property-key');
    const newPropLabel = propLabels[propLabels.length - 1];
    const newPropKey = propKeys[propKeys.length - 1];

    await user.clear(newPropLabel);
    await user.type(newPropLabel, 'Hypothesis Outcome');
    await user.clear(newPropKey);
    await user.type(newPropKey, 'hypothesis_outcome');

    // Click Create Object Type button
    const submitBtn = screen.getByTestId('btn-submit-object-type');
    await user.click(submitBtn);

    // Verify cmd_create_object_type was called with valid schema
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_create_object_type',
        expect.objectContaining({
          typeDef: expect.objectContaining({
            id: 'experiment_log',
            display_name: 'Experiment Log',
            schema: expect.arrayContaining([
              expect.objectContaining({
                key: 'hypothesis_outcome',
                label: 'Hypothesis Outcome',
              }),
            ]),
          }),
        })
      );
    });

    // Verify modal callback
    await waitFor(() => {
      expect(onCreatedMock).toHaveBeenCalled();
    });
  });
});
