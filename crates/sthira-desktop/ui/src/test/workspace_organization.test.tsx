import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { App } from '../App';
import { LeftSidebar } from '../components/LeftSidebar';
import { HomeView } from '../components/HomeView';
import { WorkspaceIndexView } from '../views/WorkspaceIndexView';
import * as ipc from '../api/ipc';

// Mock Tauri internals and IPC
vi.mock('../api/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api/ipc')>();
  return {
    ...actual,
    isTauri: false,
    getHomeState: vi.fn(),
    getWorkspaceTree: vi.fn(),
    listObjectTypes: vi.fn(),
    getObjectTypeCounts: vi.fn(),
    getSidebarItems: vi.fn(),
    togglePin: vi.fn(),
    recordAccess: vi.fn(),
    getWorkspaceIndex: vi.fn(),
    addArtifactToProject: vi.fn(),
    createProject: vi.fn(),
    createPage: vi.fn(),
    deleteArtifact: vi.fn(),
    duplicateArtifact: vi.fn(),
    duplicateProject: vi.fn(),
    renameProject: vi.fn(),
    renameArtifact: vi.fn(),
  };
});

describe('Workspace Organization Refactor', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  // -------------------------------------------------------------------------
  // 1. Sidebar Structure, 7-Item Cap, and Pinning
  // -------------------------------------------------------------------------
  it('LeftSidebar displays Projects, Notes & Pages, and Custom Objects with 7-item caps and pin indicators', async () => {
    vi.mocked(ipc.getWorkspaceTree).mockResolvedValue({
      projects: [],
      unassociated_pages: [],
    });
    vi.mocked(ipc.listObjectTypes).mockResolvedValue([]);
    vi.mocked(ipc.getObjectTypeCounts).mockResolvedValue({});
    vi.mocked(ipc.getSidebarItems).mockResolvedValue({
      projects: [
        { id: 'p1', title: 'Alpha Core Engine', item_type: 'project', is_pinned: true, last_accessed_at: 100, updated_at: 100 },
        { id: 'p2', title: 'Beta UI Framework', item_type: 'project', is_pinned: false, last_accessed_at: 90, updated_at: 90 },
      ],
      notes: [
        { id: 'n1', title: 'System Architecture Spec', item_type: 'note', is_pinned: true, last_accessed_at: 100, updated_at: 100 },
        { id: 'n2', title: 'Sprint Retrospective', item_type: 'note', is_pinned: false, last_accessed_at: 80, updated_at: 80 },
      ],
      objects: [
        { id: 'obj1', title: 'Sub-millisecond latency decision', item_type: 'object', object_type_id: 'decision', object_type_name: 'Decision', icon: '⚖️', color: '#10B981', is_pinned: true, last_accessed_at: 100, updated_at: 100 },
        { id: 'obj2', title: 'Cache eviction heuristic', item_type: 'object', object_type_id: 'idea', object_type_name: 'Idea', icon: '💡', color: '#F59E0B', is_pinned: false, last_accessed_at: 70, updated_at: 70 },
      ],
    });

    const onNavigateWorkspaceIndex = vi.fn();
    render(
      <LeftSidebar
        currentView="home"
        onNavigateHome={vi.fn()}
        onNavigateGraph={vi.fn()}
        onNavigateWorkspaceIndex={onNavigateWorkspaceIndex}
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

    // Verify sections exist in correct order
    expect(await screen.findByText('Alpha Core Engine')).toBeInTheDocument();
    expect(screen.getByText('System Architecture Spec')).toBeInTheDocument();
    expect(screen.getByText('Sub-millisecond latency decision')).toBeInTheDocument();

    // Verify pinned chips appear for pinned items
    expect(screen.getByTestId('pinned-chip-p1')).toBeInTheDocument();
    expect(screen.getByTestId('pinned-chip-n1')).toBeInTheDocument();
    expect(screen.getByTestId('pinned-chip-obj1')).toBeInTheDocument();

    // Clicking top navigation button opens All Files
    const indexNavBtn = screen.getByTestId('nav-workspace-index');
    fireEvent.click(indexNavBtn);
    expect(onNavigateWorkspaceIndex).toHaveBeenCalledWith('all');

    // Clicking "View all →" for Projects navigates to All Files focused on projects
    const viewAllProjectsBtn = screen.getByTestId('btn-view-all-projects');
    fireEvent.click(viewAllProjectsBtn);
    expect(onNavigateWorkspaceIndex).toHaveBeenCalledWith('projects');

    // Clicking "View all →" for Notes navigates to All Files focused on notes
    const viewAllNotesBtn = screen.getByTestId('btn-view-all-notes');
    fireEvent.click(viewAllNotesBtn);
    expect(onNavigateWorkspaceIndex).toHaveBeenCalledWith('notes');

    // Clicking "View all →" for Objects navigates to All Files focused on objects
    const viewAllObjectsBtn = screen.getByTestId('btn-view-all-objects');
    fireEvent.click(viewAllObjectsBtn);
    expect(onNavigateWorkspaceIndex).toHaveBeenCalledWith('objects');
  });

  // -------------------------------------------------------------------------
  // 2. Pin / Unpin Toggle Calls IPC
  // -------------------------------------------------------------------------
  it('hover pin toggle button invokes cmd_toggle_pin IPC and updates local state', async () => {
    vi.mocked(ipc.getWorkspaceTree).mockResolvedValue({ projects: [], unassociated_pages: [] });
    vi.mocked(ipc.listObjectTypes).mockResolvedValue([]);
    vi.mocked(ipc.getObjectTypeCounts).mockResolvedValue({});
    vi.mocked(ipc.togglePin).mockResolvedValue();
    vi.mocked(ipc.getSidebarItems).mockResolvedValue({
      projects: [
        { id: 'proj-unpinned', title: 'Unpinned Project', item_type: 'project', is_pinned: false, last_accessed_at: 50, updated_at: 50 },
      ],
      notes: [],
      objects: [],
    });

    render(
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

    const pinBtn = await screen.findByTestId('pin-btn-proj-unpinned');
    expect(pinBtn).toBeInTheDocument();

    fireEvent.click(pinBtn);
    expect(ipc.togglePin).toHaveBeenCalledWith('proj-unpinned', 'project', true);
  });

  // -------------------------------------------------------------------------
  // 3. Home Screen Declutter (No Unfiled Thoughts, Capped Projects & Recent Items)
  // -------------------------------------------------------------------------
  it('HomeView renders decluttered view without Unfiled Thoughts and with capped items', async () => {
    const onNavigateWorkspaceIndex = vi.fn();
    const mockHomeState = {
      projects: [
        { id: 'p1', name: 'Proj 1', category: 'Software', item_count: 5, icon_type: 'code' },
        { id: 'p2', name: 'Proj 2', category: 'Research', item_count: 8, icon_type: 'brain' },
        { id: 'p3', name: 'Proj 3', category: 'Design', item_count: 2, icon_type: 'code' },
        { id: 'p4', name: 'Proj 4 (Should be capped out)', category: 'Ops', item_count: 1, icon_type: 'code' },
      ],
      recent_items: [
        { id: 'r1', title: 'Recent Doc 1', subtitle: 'Modified today', item_type: 'Artifact', artifact_id: 'art-1' },
        { id: 'r2', title: 'Recent Doc 2', subtitle: 'Modified yesterday', item_type: 'Artifact', artifact_id: 'art-2' },
        { id: 'r3', title: 'Recent Canvas 3', subtitle: 'Active canvas', item_type: 'Canvas', canvas_id: 'canv-1' },
        { id: 'r4', title: 'Recent Doc 4 (Should be capped out)', subtitle: 'Older', item_type: 'Artifact', artifact_id: 'art-4' },
      ],
      loose_captures: [
        { id: 'loose-1', title: 'Unfiled thought that should not be on Home', content_md: 'Raw draft', updated_at: 100 },
      ],
    };

    render(
      <HomeView
        homeState={mockHomeState as any}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onOpenCanvas={vi.fn()}
        onRefresh={vi.fn()}
        onOpenFlow={vi.fn()}
        onNavigateWorkspaceIndex={onNavigateWorkspaceIndex}
      />
    );

    // Verify Hero
    expect(screen.getByText('Make room to think.')).toBeInTheDocument();

    // Verify Top Nav components
    expect(screen.getByTestId('home-search-btn')).toBeInTheDocument();
    expect(screen.getByTestId('home-model-chip')).toBeInTheDocument();
    expect(screen.getByTestId('home-settings-btn')).toBeInTheDocument();

    // Verify projects capped to 3
    expect(screen.getByText('Proj 1')).toBeInTheDocument();
    expect(screen.getByText('Proj 2')).toBeInTheDocument();
    expect(screen.getByText('Proj 3')).toBeInTheDocument();
    expect(screen.queryByText('Proj 4 (Should be capped out)')).toBeNull();

    // Verify recent items capped to 3
    expect(screen.getByText('Recent Doc 1')).toBeInTheDocument();
    expect(screen.getByText('Recent Doc 2')).toBeInTheDocument();
    expect(screen.getByText('Recent Canvas 3')).toBeInTheDocument();
    expect(screen.queryByText('Recent Doc 4 (Should be capped out)')).toBeNull();

    // Verify "Unfiled Thoughts" section is completely removed
    expect(screen.queryByText(/Unfiled Thoughts/i)).toBeNull();
    expect(screen.queryByText('Unfiled thought that should not be on Home')).toBeNull();

    // Clicking "View all →" on Home "Your projects" navigates to All Files
    const homeViewAllProjectsBtn = screen.getByTestId('home-view-all-projects');
    fireEvent.click(homeViewAllProjectsBtn);
    expect(onNavigateWorkspaceIndex).toHaveBeenCalledWith('projects');
  });

  // -------------------------------------------------------------------------
  // 4. All Files View Tabs, Note Scope Filter + Sort, and Standalone Attachment
  // -------------------------------------------------------------------------
  it('All Files view renders the four tabs, filters notes by scope + sort, and allows attaching a standalone note', async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.getWorkspaceTree).mockResolvedValue({
      projects: [
        { id: 'p1', name: 'Deep Space Mission', pages: [], canvases: [], purpose: null, updated_at: 100 },
      ],
      unassociated_pages: [],
    } as any);
    vi.mocked(ipc.listObjectTypes).mockResolvedValue([
      { id: 'decision', display_name: 'Decision', icon: '⚖️', color: '#10B981', is_system: true, schema: [], created_at: 100 },
    ]);
    vi.mocked(ipc.addArtifactToProject).mockResolvedValue({ success: true } as any);
    // Faithful mock: like `cmd_get_workspace_index`, the payload is scoped to the tab.
    const indexCounts = { all: 4, projects: 1, notes: 2, objects: 1, standalone: 1 };
    const indexItems = [
        {
          id: 'p1',
          title: 'Deep Space Mission',
          description: 'Autonomous exploration probe',
          entity_type: 'project',
          object_type_id: null,
          object_type_name: null,
          icon: '🚀',
          color: null,
          is_pinned: true,
          last_accessed_at: 100,
          updated_at: 100,
          created_at: 100,
          project_id: null,
          project_name: null,
          item_count: 4,
          standalone: false,
        },
        {
          id: 'n1',
          title: 'Unaffiliated Idea Note',
          description: 'Standalone hypothesis about propulsion',
          entity_type: 'note',
          object_type_id: null,
          object_type_name: null,
          icon: '📝',
          color: null,
          is_pinned: false,
          last_accessed_at: 200,
          updated_at: 200,
          created_at: 90,
          project_id: null,
          project_name: null,
          item_count: null,
          standalone: true,
        },
        {
          id: 'n2',
          title: 'Attached Project Note',
          description: 'Note living inside a project folder',
          entity_type: 'note',
          object_type_id: null,
          object_type_name: null,
          icon: '📝',
          color: null,
          is_pinned: false,
          last_accessed_at: 95,
          updated_at: 95,
          created_at: 95,
          project_id: 'p1',
          project_name: 'Deep Space Mission',
          item_count: null,
          standalone: false,
        },
        {
          id: 'obj1',
          title: 'Thrust Vectoring Decision',
          description: 'Cold gas vs hydrazine reaction',
          entity_type: 'object',
          object_type_id: 'decision',
          object_type_name: 'Decision',
          icon: '⚖️',
          color: '#10B981',
          is_pinned: false,
          last_accessed_at: 80,
          updated_at: 80,
          created_at: 80,
          project_id: 'p1',
          project_name: 'Deep Space Mission',
          item_count: null,
          standalone: false,
        },
    ];

    vi.mocked(ipc.getWorkspaceIndex).mockImplementation(async (tab?: any) => {
      const byTab: Record<string, (i: any) => boolean> = {
        projects: (i) => i.entity_type === 'project',
        notes: (i) => i.entity_type === 'note',
        objects: (i) => i.entity_type === 'object',
      };
      const predicate = tab ? byTab[tab] : undefined;
      return {
        counts: indexCounts,
        items: predicate ? indexItems.filter(predicate) : indexItems,
      } as any;
    });

    const onOpenProject = vi.fn();
    const onOpenArtifact = vi.fn();

    render(
      <WorkspaceIndexView
        initialTab="all"
        onOpenProject={onOpenProject}
        onOpenArtifact={onOpenArtifact}
      />
    );

    // Verify the four tabs render — the "Standalone Notes" tab has been removed
    expect(await screen.findByTestId('tab-all')).toBeInTheDocument();
    expect(screen.getByTestId('tab-projects')).toBeInTheDocument();
    expect(screen.getByTestId('tab-notes')).toBeInTheDocument();
    expect(screen.getByTestId('tab-objects')).toBeInTheDocument();
    expect(screen.queryByTestId('tab-standalone')).not.toBeInTheDocument();

    // Verify items in "All Items" tab
    expect(screen.getByText('Deep Space Mission')).toBeInTheDocument();
    expect(screen.getByText('Unaffiliated Idea Note')).toBeInTheDocument();
    expect(screen.getByText('Thrust Vectoring Decision')).toBeInTheDocument();

    // Notes & Pages now carries the scope toggle + sort mechanism instead of a separate tab
    await user.click(screen.getByTestId('tab-notes'));
    expect(screen.getByTestId('note-scope-filter')).toBeInTheDocument();
    expect(screen.getByTestId('note-scope-all')).toBeInTheDocument();
    expect(screen.getByTestId('note-scope-projects')).toBeInTheDocument();
    expect(screen.getByTestId('note-scope-standalone')).toBeInTheDocument();
    expect(screen.getByTestId('note-sort-select')).toBeInTheDocument();

    // Default scope (All): both the attached and the standalone note are listed
    expect(screen.getByTestId('index-item-n1')).toBeInTheDocument();
    expect(screen.getByTestId('index-item-n2')).toBeInTheDocument();

    // Scope = In Projects: only the project-attached note remains
    await user.click(screen.getByTestId('note-scope-projects'));
    expect(screen.getByTestId('index-item-n2')).toBeInTheDocument();
    expect(screen.queryByTestId('index-item-n1')).not.toBeInTheDocument();

    // Scope = Standalone: only the unaffiliated note remains
    await user.click(screen.getByTestId('note-scope-standalone'));
    expect(screen.getByTestId('index-item-n1')).toBeInTheDocument();
    expect(screen.queryByTestId('index-item-n2')).not.toBeInTheDocument();

    // The [Attach to Project ▾] action survives the tab removal and lives on the row itself
    const attachBtn = screen.getByTestId('attach-project-btn-n1');
    await user.click(attachBtn);
    const selectProjectOpt = await screen.findByTestId('attach-target-p1');
    expect(selectProjectOpt).toBeInTheDocument();

    await user.click(selectProjectOpt);
    expect(ipc.addArtifactToProject).toHaveBeenCalledWith('n1', 'p1');

    // Sort mechanism: "Last Modified" (default) puts n1 (newer) first…
    await user.click(screen.getByTestId('note-scope-all'));
    const idsInOrder = () =>
      screen
        .getAllByTestId(/^index-item-/)
        .map((el) => el.getAttribute('data-testid'));
    expect(idsInOrder()).toEqual(['index-item-n1', 'index-item-n2']);

    // …while "Title (A–Z)" reorders them by title ("Attached…" before "Unaffiliated…")
    fireEvent.change(screen.getByTestId('note-sort-select'), { target: { value: 'title_asc' } });
    expect(idsInOrder()).toEqual(['index-item-n2', 'index-item-n1']);

    // …and "Date Created" sorts by created_at (n2 created 95 > n1 created 90)
    fireEvent.change(screen.getByTestId('note-sort-select'), { target: { value: 'created' } });
    expect(idsInOrder()).toEqual(['index-item-n2', 'index-item-n1']);
  });

  // -------------------------------------------------------------------------
  // 5. End-to-End View Switching in App
  // -------------------------------------------------------------------------
  it('App seamlessly switches to WorkspaceIndexView when clicking View all in LeftSidebar', async () => {
    vi.mocked(ipc.getHomeState).mockResolvedValue({
      projects: [],
      recent_items: [],
      loose_captures: [],
    });
    vi.mocked(ipc.getWorkspaceTree).mockResolvedValue({ projects: [], unassociated_pages: [] });
    vi.mocked(ipc.listObjectTypes).mockResolvedValue([]);
    vi.mocked(ipc.getObjectTypeCounts).mockResolvedValue({});
    vi.mocked(ipc.getSidebarItems).mockResolvedValue({
      projects: [{ id: 'p1', title: 'Quantum Telemetry', item_type: 'project', is_pinned: false, last_accessed_at: 100, updated_at: 100 }],
      notes: [],
      objects: [],
    });
    vi.mocked(ipc.getWorkspaceIndex).mockResolvedValue({
      counts: { all: 1, projects: 1, notes: 0, objects: 0, standalone: 0 },
      items: [
        {
          id: 'p1',
          title: 'Quantum Telemetry',
          description: 'High throughput telemetry',
          entity_type: 'project',
          object_type_id: null,
          object_type_name: null,
          icon: '🔬',
          color: null,
          is_pinned: false,
          last_accessed_at: 100,
          updated_at: 100,
          created_at: 100,
          project_id: null,
          project_name: null,
          item_count: 3,
          standalone: false,
        },
      ],
    } as any);

    render(<App />);

    // Click top nav "All Files"
    const indexBtn = await screen.findByTestId('nav-workspace-index');
    fireEvent.click(indexBtn);

    // WorkspaceIndexView should now be rendered
    expect(await screen.findByTestId('workspace-index-view')).toBeInTheDocument();
    expect(screen.getByTestId('tab-projects')).toBeInTheDocument();
  });
});
