import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { FocusView } from '../components/FocusView';
import { LeftSidebar } from '../components/LeftSidebar';
import { HomeView } from '../components/HomeView';

// Mock Tauri invoke
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe('Settings 3-Dots Menu, Auto-Save & Real-Time Title Updates', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  const sampleTree = {
    projects: [
      {
        id: 'proj-1',
        name: 'Alpha Project',
        purpose: 'Alpha research',
        pages: [
          {
            id: 'page-1',
            title: 'Alpha Note 1',
            updated_at: 1000,
          },
        ],
        canvases: [],
        updated_at: 1000,
      },
      {
        id: 'proj-2',
        name: 'Beta Project',
        purpose: 'Beta research',
        pages: [],
        canvases: [],
        updated_at: 2000,
      },
    ],
    unassociated_pages: [
      {
        id: 'loose-1',
        title: 'Loose Scratchpad',
        updated_at: 3000,
      },
    ],
  };

  // -------------------------------------------------------------------------
  // Test 1: Project 3-Dots Menu (Pin, Duplicate, Inline Rename)
  // -------------------------------------------------------------------------
  it('opens project 3-dots menu and handles Pin, Duplicate, and Rename', async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_get_workspace_tree') {
        return sampleTree;
      }
      if (cmd === 'cmd_list_object_types') return [];
      if (cmd === 'cmd_get_object_type_counts') return {};
      if (cmd === 'cmd_duplicate_project') {
        return {
          id: 'proj-1-copy',
          name: 'Alpha Project (Copy)',
          purpose: 'Alpha research',
          pages: [],
          canvases: [],
          updated_at: 1050,
        };
      }
      if (cmd === 'cmd_rename_project') {
        return {
          id: args.projectId || args.project_id,
          name: args.name,
        };
      }
      return null;
    });

    render(
      <LeftSidebar
        currentView="focus"
        activeProjectId="proj-1"
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        isCollapsed={false}
        onToggleCollapse={vi.fn()}
      />
    );

    // Find 3-dots button for proj-1
    const menuBtn = await screen.findByTestId('btn-project-menu-proj-1');
    expect(menuBtn).toBeInTheDocument();

    // 1. Test Pin toggle
    await user.click(menuBtn);
    const pinBtn = await screen.findByTestId('btn-pin-project-proj-1');
    await user.click(pinBtn);

    // Verify localStorage has proj-1 pinned
    const pinned = JSON.parse(localStorage.getItem('sthira_pinned_items') || '{}');
    expect(pinned['proj-1']).toBe(true);

    // 2. Test Duplicate
    await user.click(menuBtn);
    const duplicateBtn = await screen.findByTestId('btn-duplicate-project-proj-1');
    await user.click(duplicateBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_duplicate_project',
        expect.objectContaining({
          projectId: 'proj-1',
        })
      );
    });

    // 3. Test Inline Rename
    await user.click(menuBtn);
    const renameBtn = await screen.findByText(/Rename/i);
    await user.click(renameBtn);

    const renameInput = await screen.findByTestId('input-rename-project-proj-1');
    expect(renameInput).toBeInTheDocument();
    await user.clear(renameInput);
    await user.type(renameInput, 'Renamed Alpha{enter}');

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_rename_project',
        expect.objectContaining({
          projectId: 'proj-1',
          newName: 'Renamed Alpha',
        })
      );
    });
  });

  // -------------------------------------------------------------------------
  // Test 2: Note / Page 3-Dots Menu (Duplicate, Add to Project, Rename)
  // -------------------------------------------------------------------------
  it('opens note 3-dots menu and handles Duplicate, Add to Project, and Rename', async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_get_workspace_tree') {
        return sampleTree;
      }
      if (cmd === 'cmd_list_object_types') return [];
      if (cmd === 'cmd_get_object_type_counts') return {};
      if (cmd === 'cmd_duplicate_artifact') {
        return 'page-1-copy';
      }
      if (cmd === 'cmd_add_artifact_to_project') {
        return true;
      }
      if (cmd === 'cmd_rename_artifact') {
        return {
          id: args.artifactId || args.artifact_id,
          title: args.title,
        };
      }
      return null;
    });

    render(
      <LeftSidebar
        currentView="focus"
        activeProjectId="proj-1"
        activeArtifactId="page-1"
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        isCollapsed={false}
        onToggleCollapse={vi.fn()}
      />
    );

    // Find 3-dots button for page-1 (rendered because activeProjectId="proj-1" auto-expands proj-1)
    const pageMenuBtn = await screen.findByTestId('btn-page-menu-page-1');
    expect(pageMenuBtn).toBeInTheDocument();

    // 1. Test Duplicate Note
    await user.click(pageMenuBtn);
    const duplicatePageBtn = await screen.findByTestId('page-action-duplicate');
    await user.click(duplicatePageBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_duplicate_artifact',
        expect.objectContaining({
          artifactId: 'page-1',
          targetProjectId: 'proj-1',
        })
      );
    });

    // 2. Test Add to Project Modal
    await user.click(pageMenuBtn);
    const addToProjectBtn = await screen.findByTestId('page-action-link');
    await user.click(addToProjectBtn);

    // Modal opens, choose "Beta Project" and confirm
    const targetProjectBtn = await screen.findByTestId('select-project-proj-2');
    await user.click(targetProjectBtn);

    const confirmBtn = await screen.findByTestId('btn-confirm-add-to-project');
    await user.click(confirmBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_add_artifact_to_project',
        expect.objectContaining({
          artifactId: 'page-1',
          projectId: 'proj-2',
        })
      );
    });

    // 3. Test Inline Rename of Note
    await user.click(pageMenuBtn);
    const renameNoteBtn = await screen.findByTestId('page-action-rename');
    await user.click(renameNoteBtn);

    const renameInput = await screen.findByTestId('input-rename-page-page-1');
    expect(renameInput).toBeInTheDocument();
    await user.clear(renameInput);
    await user.type(renameInput, 'Refactored Design Doc{enter}');

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_rename_artifact',
        expect.objectContaining({
          artifactId: 'page-1',
          newTitle: 'Refactored Design Doc',
        })
      );
    });
  });

  // -------------------------------------------------------------------------
  // Test 3: Debounced Auto-Save in FocusView
  // -------------------------------------------------------------------------
  it('automatically auto-saves content without requiring manual save button click', async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_get_artifact_context') {
        return {
          artifact: {
            id: 'art-autosave-1',
            artifact_type: 'note',
            authorship: 'user',
            title: 'AutoSave Doc',
            content_md: '# AutoSave Doc\n\nInitial draft.',
            source_url: null,
            content_hash: 'hash-init',
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
          project_id: 'proj-1',
          is_unfinished_draft: false,
        };
      }
      if (cmd === 'cmd_get_resume_brief') {
        return { project_id: 'proj-1', level: 1, text: 'Summary', created_at: 1000 };
      }
      if (cmd === 'cmd_get_project_history') {
        return { project_id: 'proj-1', entries: [], checkpoint_count: 0 };
      }
      if (cmd === 'cmd_save_artifact') {
        return {
          id: args.artifactId || args.artifact_id,
          artifact_type: 'note',
          authorship: 'user',
          title: args.title || 'AutoSave Doc',
          content_md: args.contentMd || args.content_md,
          source_url: null,
          content_hash: 'hash-auto-saved',
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
        projectId="proj-1"
        artifactId="art-autosave-1"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    const editor = await screen.findByTestId('textarea-editor');
    expect(editor).toBeInTheDocument();

    // Type new text into editor
    await user.type(editor, ' Adding automatic thoughts.');

    // Auto-save triggers after debounce (600ms) without any manual save button click
    await waitFor(
      () => {
        expect(mockInvoke).toHaveBeenCalledWith(
          'cmd_save_artifact',
          expect.objectContaining({
            artifactId: 'art-autosave-1',
            contentMd: expect.stringContaining('Adding automatic thoughts.'),
          })
        );
      },
      { timeout: 3000 }
    );
  });

  // -------------------------------------------------------------------------
  // Test 4: Real-time Sidebar Title Synchronization
  // -------------------------------------------------------------------------
  it('updates title immediately in left sidebar via realtimeTitleUpdate prop', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'cmd_get_workspace_tree') return sampleTree;
      if (cmd === 'cmd_list_object_types') return [];
      if (cmd === 'cmd_get_object_type_counts') return {};
      return null;
    });

    const { rerender } = render(
      <LeftSidebar
        currentView="focus"
        activeProjectId="proj-1"
        activeArtifactId="page-1"
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        isCollapsed={false}
        onToggleCollapse={vi.fn()}
      />
    );

    // Initial title is visible
    expect(await screen.findByText('Alpha Note 1')).toBeInTheDocument();

    // Rerender with real-time title update while user is typing in editor
    rerender(
      <LeftSidebar
        currentView="focus"
        activeProjectId="proj-1"
        activeArtifactId="page-1"
        realtimeTitleUpdate={{ id: 'page-1', title: 'Immediate Realtime Title' }}
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        isCollapsed={false}
        onToggleCollapse={vi.fn()}
      />
    );

    // Should immediately display the new title without waiting for reload
    expect(await screen.findByText('Immediate Realtime Title')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // Test 5: Renaming title via input synchronizes markdown heading and auto-saves
  // -------------------------------------------------------------------------
  it('renaming title in title input field synchronizes line 1 markdown heading and persists', async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_get_artifact_context') {
        return {
          artifact: {
            id: 'art-rename-test',
            artifact_type: 'note',
            authorship: 'user',
            title: 'Initial Note Title',
            content_md: '# Initial Note Title\n\nNote content goes here.',
            source_url: null,
            content_hash: 'hash-init',
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
          project_id: null,
          is_unfinished_draft: false,
        };
      }
      if (cmd === 'cmd_save_artifact') {
        return {
          id: args.artifactId || args.artifact_id,
          artifact_type: 'note',
          authorship: 'user',
          title: args.title || 'Updated Title',
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

    const onTitleUpdateMock = vi.fn();

    render(
      <FocusView
        artifactId="art-rename-test"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
        onTitleUpdate={onTitleUpdateMock}
      />
    );

    const titleInput = await screen.findByTestId('input-artifact-title');
    const editor = screen.getByTestId('textarea-editor') as HTMLTextAreaElement;

    expect(titleInput).toHaveValue('Initial Note Title');
    expect(editor.value).toContain('# Initial Note Title');

    // Type a new title in the title input field
    await user.clear(titleInput);
    await user.type(titleInput, 'Brand New Title');

    // Assert that editor line 1 immediately updated to match the new title
    expect(editor.value).toContain('# Brand New Title');
    expect(onTitleUpdateMock).toHaveBeenCalledWith('art-rename-test', 'Brand New Title');

    // Auto-save triggers with the new title
    await waitFor(
      () => {
        expect(mockInvoke).toHaveBeenCalledWith(
          'cmd_save_artifact',
          expect.objectContaining({
            artifactId: 'art-rename-test',
            title: 'Brand New Title',
            contentMd: expect.stringContaining('# Brand New Title'),
          })
        );
      },
      { timeout: 3000 }
    );
  });

  // -------------------------------------------------------------------------
  // Test 6: Renaming line 1 heading in editor textarea updates title input field
  // -------------------------------------------------------------------------
  it('editing line 1 heading in markdown editor immediately updates title input', async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation(async (cmd: string, _args: any) => {
      if (cmd === 'cmd_get_artifact_context') {
        return {
          artifact: {
            id: 'art-heading-test',
            artifact_type: 'note',
            authorship: 'user',
            title: 'Original Heading Title',
            content_md: '# Original Heading Title\n\nBody text.',
            source_url: null,
            content_hash: 'hash-init',
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
          project_id: null,
          is_unfinished_draft: false,
        };
      }
      return null;
    });

    const onTitleUpdateMock = vi.fn();

    render(
      <FocusView
        artifactId="art-heading-test"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
        onTitleUpdate={onTitleUpdateMock}
      />
    );

    const titleInput = await screen.findByTestId('input-artifact-title');
    const editor = screen.getByTestId('textarea-editor');

    expect(titleInput).toHaveValue('Original Heading Title');

    // Change heading in markdown editor
    await user.clear(editor);
    await user.type(editor, '# Refactored Note Heading\n\nNew body lines.');

    // Assert that title input updated to the new heading
    await waitFor(() => {
      expect(titleInput).toHaveValue('Refactored Note Heading');
    });
    expect(onTitleUpdateMock).toHaveBeenCalledWith('art-heading-test', 'Refactored Note Heading');
  });

  // -------------------------------------------------------------------------
  // Test 7: Clicking Home button flushes save and triggers onBack
  // -------------------------------------------------------------------------
  it('clicking Home button saves unsaved title/content before invoking onBack', async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_get_artifact_context') {
        return {
          artifact: {
            id: 'art-back-test',
            artifact_type: 'note',
            authorship: 'user',
            title: 'Pre-Back Title',
            content_md: '# Pre-Back Title\n\nContent.',
            source_url: null,
            content_hash: 'hash-init',
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
          project_id: null,
          is_unfinished_draft: false,
        };
      }
      if (cmd === 'cmd_save_artifact') {
        return {
          id: args.artifactId || args.artifact_id,
          artifact_type: 'note',
          authorship: 'user',
          title: args.title || 'Saved Before Back',
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

    const onBackMock = vi.fn();

    render(
      <FocusView
        artifactId="art-back-test"
        onBack={onBackMock}
        onOpenCanvas={vi.fn()}
      />
    );

    const titleInput = await screen.findByTestId('input-artifact-title');
    await user.clear(titleInput);
    await user.type(titleInput, 'Saved Before Back');

    // Click Home button immediately
    const homeBtn = screen.getByTitle('Return to Home');
    await user.click(homeBtn);

    // Verify cmd_save_artifact was called with the new title before onBack
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_save_artifact',
        expect.objectContaining({
          artifactId: 'art-back-test',
          title: 'Saved Before Back',
        })
      );
    });

    expect(onBackMock).toHaveBeenCalled();
  });

  // -------------------------------------------------------------------------
  // Test 8: LeftSidebar Resizable Handle & LocalStorage Persistence
  // -------------------------------------------------------------------------
  it('allows resizing the left sidebar via drag handle and persists width to localStorage', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'cmd_get_workspace_tree') return sampleTree;
      if (cmd === 'cmd_list_object_types') return [];
      if (cmd === 'cmd_get_object_type_counts') return {};
      return null;
    });

    const { container } = render(
      <LeftSidebar
        currentView="home"
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onOpenSearch={vi.fn()}
        onOpenSettings={vi.fn()}
        onOpenAccount={vi.fn()}
        isCollapsed={false}
        onToggleCollapse={vi.fn()}
      />
    );

    const aside = container.querySelector('aside');
    expect(aside).toBeInTheDocument();
    expect(aside).toHaveStyle({ width: '280px' });

    const resizeHandle = screen.getByTestId('sidebar-resize-handle');
    expect(resizeHandle).toBeInTheDocument();

    // Mouse down on handle to initiate drag
    act(() => {
      fireEvent.mouseDown(resizeHandle, { clientX: 280 });
    });

    // Move mouse to 380px while dragging
    act(() => {
      fireEvent.mouseMove(document, { clientX: 380 });
    });

    // Verify width updated
    expect(aside).toHaveStyle({ width: '380px' });

    // Release mouse
    act(() => {
      fireEvent.mouseUp(document);
    });

    // Verify persisted in localStorage
    expect(localStorage.getItem('sthira_sidebar_width')).toBe('380');

    // Double-click to reset width back to default (280px)
    act(() => {
      fireEvent.doubleClick(resizeHandle);
    });
    expect(aside).toHaveStyle({ width: '280px' });
    expect(localStorage.getItem('sthira_sidebar_width')).toBe('280');
  });

  // -------------------------------------------------------------------------
  // Test 9: HomeView does not render sthira word in the middle header
  // -------------------------------------------------------------------------
  it('does not display the word sthira in the middle place in HomeView', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'cmd_get_selected_provider_and_model') return null;
      if (cmd === 'cmd_list_all_providers_with_models') return [];
      if (cmd === 'cmd_get_provider_catalog') return [];
      return null;
    });

    await act(async () => {
      render(
        <HomeView
          homeState={{
            projects: [],
            recent_items: [],
            loose_captures: [],
          }}
          onOpenProject={vi.fn()}
          onOpenArtifact={vi.fn()}
          onOpenCanvas={vi.fn()}
          onPromoteLooseCapture={vi.fn()}
          onRefresh={vi.fn()}
          onOpenSearch={vi.fn()}
          onOpenSettings={vi.fn()}
          onOpenAccount={vi.fn()}
          onOpenFlow={vi.fn()}
        />
      );
    });

    // Header should exist and have brand icon, but no "sthira" text span
    const brandText = screen.queryByText(/^sthira$/i);
    expect(brandText).toBeNull();
  });
});
