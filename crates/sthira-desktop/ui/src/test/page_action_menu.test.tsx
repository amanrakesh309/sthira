import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { PageActionMenu } from '../components/PageActionMenu';

// Mock Tauri invoke (ipc.ts wrappers call through to it)
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

const IDEA_TYPE = {
  id: 'idea',
  display_name: 'Idea',
  icon: '💡',
  color: '#F59E0B',
  schema: [],
  is_system: true,
  created_at: 1000,
};

describe('Phase 17: Page Action Menu', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'cmd_get_workspace_tree':
          return {
            projects: [
              { id: 'proj-1', name: 'Alpha Project', pages: [], canvases: [], purpose: '', updated_at: 1 },
              { id: 'proj-2', name: 'Beta Project', pages: [], canvases: [], purpose: '', updated_at: 1 },
            ],
            unassociated_pages: [],
          };
        case 'cmd_list_object_types':
          return [IDEA_TYPE];
        case 'cmd_duplicate_artifact':
          return 'art-copy-1';
        default:
          return null;
      }
    });
  });

  // -------------------------------------------------------------------------
  // Test 1: opening the menu renders every action group + item.
  // -------------------------------------------------------------------------
  it('Test 1: opening PageActionMenu renders all action options', async () => {
    const user = userEvent.setup();
    render(
      <PageActionMenu
        artifactId="art-1"
        artifactTitle="Alpha Note"
        contentMd="# Alpha Note\n\nBody"
        projectId="proj-1"
        objectTypes={[IDEA_TYPE as any]}
      />
    );

    // Closed by default.
    expect(screen.queryByTestId('page-action-dropdown')).not.toBeInTheDocument();

    await user.click(screen.getByTestId('page-action-menu-trigger'));

    await waitFor(() => {
      expect(screen.getByTestId('page-action-dropdown')).toBeInTheDocument();
    });

    // Group 1: Project Management
    expect(screen.getByTestId('page-action-link')).toHaveTextContent('Link to Project');
    expect(screen.getByTestId('page-action-move')).toHaveTextContent('Move to Project');
    expect(screen.getByTestId('page-action-create-project')).toHaveTextContent(
      'Create Project with this Note'
    );
    // Detach is only rendered when the note currently belongs to a project.
    expect(screen.getByTestId('page-action-detach')).toHaveTextContent('Detach from Project');

    // Group 2: Duplication & Transform
    expect(screen.getByTestId('page-action-duplicate')).toHaveTextContent('Duplicate Note');
    expect(screen.getByTestId('page-action-convert')).toHaveTextContent('Convert to Object Type');

    // Group 3: Clipboard & Links
    expect(screen.getByTestId('page-action-copy-markdown')).toHaveTextContent('Copy Markdown');
    expect(screen.getByTestId('page-action-copy-link')).toHaveTextContent('Copy Note Link');

    // Group 4: Destructive
    expect(screen.getByTestId('page-action-delete')).toHaveTextContent('Delete Page');
  });

  // -------------------------------------------------------------------------
  // Test 2: "Duplicate Note" calls cmd_duplicate_artifact and offers [Open].
  // -------------------------------------------------------------------------
  it('Test 2: clicking Duplicate Note calls cmd_duplicate_artifact and shows an [Open] toast', async () => {
    const user = userEvent.setup();
    const onOpenArtifact = vi.fn();

    render(
      <PageActionMenu
        artifactId="art-1"
        artifactTitle="Alpha Note"
        projectId="proj-1"
        objectTypes={[IDEA_TYPE as any]}
        onOpenArtifact={onOpenArtifact}
      />
    );

    await user.click(screen.getByTestId('page-action-menu-trigger'));
    await user.click(await screen.findByTestId('page-action-duplicate'));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_duplicate_artifact',
        expect.objectContaining({
          artifactId: 'art-1',
          targetProjectId: 'proj-1',
        })
      );
    });

    // Toast surfaces the new copy with an [Open] affordance.
    const toast = await screen.findByTestId('page-action-toast');
    expect(toast).toHaveTextContent('Note duplicated');

    await user.click(screen.getByTestId('page-action-toast-open'));
    expect(onOpenArtifact).toHaveBeenCalledWith('art-copy-1', 'proj-1');
  });

  // -------------------------------------------------------------------------
  // Test 3: standalone notes hide Detach; Link uses the searchable picker.
  // -------------------------------------------------------------------------
  it('Test 3: standalone notes hide Detach and Link to Project uses the searchable picker', async () => {
    const user = userEvent.setup();

    render(
      <PageActionMenu
        artifactId="art-loose"
        artifactTitle="Loose Note"
        projectId={null}
        objectTypes={[IDEA_TYPE as any]}
      />
    );

    await user.click(screen.getByTestId('page-action-menu-trigger'));

    await waitFor(() => {
      expect(screen.getByTestId('page-action-dropdown')).toBeInTheDocument();
    });
    // No Detach for a standalone note.
    expect(screen.queryByTestId('page-action-detach')).not.toBeInTheDocument();

    // Link to Project opens the built-in searchable picker.
    await user.click(screen.getByTestId('page-action-link'));
    const search = await screen.findByTestId('page-action-project-search');
    expect(search).toBeInTheDocument();
    expect(screen.getByTestId('page-action-project-proj-1')).toBeInTheDocument();
    expect(screen.getByTestId('page-action-project-proj-2')).toBeInTheDocument();

    // Search filters the list.
    fireEvent.change(search, { target: { value: 'beta' } });
    await waitFor(() => {
      expect(screen.queryByTestId('page-action-project-proj-1')).not.toBeInTheDocument();
      expect(screen.getByTestId('page-action-project-proj-2')).toBeInTheDocument();
    });

    // Selecting a project links the note.
    await user.click(screen.getByTestId('page-action-project-proj-2'));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_link_artifact_to_project',
        expect.objectContaining({ artifactId: 'art-loose', projectId: 'proj-2' })
      );
    });
  });

  // -------------------------------------------------------------------------
  // Test 4: Convert submenu lists registered object types and converts.
  // -------------------------------------------------------------------------
  it('Test 4: Convert submenu lists object types and calls cmd_convert_artifact_to_object', async () => {
    const user = userEvent.setup();

    render(
      <PageActionMenu
        artifactId="art-9"
        artifactTitle="Untyped Note"
        projectId={null}
        objectTypes={[IDEA_TYPE as any]}
      />
    );

    await user.click(screen.getByTestId('page-action-menu-trigger'));
    await user.click(await screen.findByTestId('page-action-convert'));

    const ideaOption = await screen.findByTestId('page-action-convert-idea');
    expect(ideaOption).toHaveTextContent('Idea');

    await user.click(ideaOption);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_convert_artifact_to_object',
        expect.objectContaining({ artifactId: 'art-9', objectTypeId: 'idea' })
      );
    });
  });
});

