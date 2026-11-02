import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { WorkspaceIndexView } from '../views/WorkspaceIndexView';
import * as ipc from '../api/ipc';
import type { WorkspaceIndexData, ObjectType } from '../types/sthira';

vi.mock('../api/ipc', () => ({
  getWorkspaceIndex: vi.fn(),
  getWorkspaceTree: vi.fn(),
  listObjectTypes: vi.fn(),
  togglePin: vi.fn(),
  addArtifactToProject: vi.fn(),
  deleteProject: vi.fn(),
  inspectProjectDeletion: vi.fn(),
}));

const mockObjectTypes: ObjectType[] = [
  {
    id: 'decision',
    display_name: 'Decision',
    description: 'Decision items',
    icon: '⚖️',
    color: '#818CF8',
    schema: [],
    is_system: true,
    created_at: 100,
  },
  {
    id: 'idea',
    display_name: 'Idea',
    description: 'Ideas and concepts',
    icon: '💡',
    color: '#FBBF24',
    schema: [],
    is_system: true,
    created_at: 100,
  },
  {
    id: 'source',
    display_name: 'Source',
    description: 'Citations and sources',
    icon: '🔗',
    color: '#60A5FA',
    schema: [],
    is_system: true,
    created_at: 100,
  },
];

const mockIndexData: WorkspaceIndexData = {
  counts: {
    all: 5,
    projects: 2,
    notes: 2,
    objects: 1,
    standalone: 1,
  },
  items: [
    {
      id: 'proj-1',
      title: 'Deep Space Mission',
      entity_type: 'project',
      description: 'Orbital trajectories and mission parameters',
      item_count: 3,
      standalone: false,
      is_pinned: true,
      last_accessed_at: 3000,
      created_at: 1000,
      updated_at: 3000,
    },
    {
      id: 'proj-2',
      title: 'Solar Array System',
      entity_type: 'project',
      description: 'Photovoltaic efficiency logs',
      item_count: 1,
      standalone: false,
      is_pinned: false,
      last_accessed_at: 2500,
      created_at: 1100,
      updated_at: 2500,
    },
    {
      id: 'note-1',
      title: 'Thermal Shielding Analysis',
      entity_type: 'note',
      description: 'Heat resistance calculations for atmospheric re-entry',
      project_id: 'proj-1',
      project_name: 'Deep Space Mission',
      standalone: false,
      is_pinned: false,
      last_accessed_at: 2800,
      created_at: 1200,
      updated_at: 2800,
    },
    {
      id: 'note-2',
      title: 'Quick Capture Thoughts',
      entity_type: 'note',
      description: 'Loose brainstorming on thruster telemetry',
      standalone: true,
      is_pinned: false,
      last_accessed_at: 2700,
      created_at: 1300,
      updated_at: 2700,
    },
    {
      id: 'obj-1',
      title: 'Thruster Valve Spec',
      entity_type: 'object',
      object_type_id: 'decision',
      object_type_name: 'Decision',
      description: 'Formal architectural selection of titanium solenoid valves',
      project_id: 'proj-1',
      project_name: 'Deep Space Mission',
      standalone: false,
      is_pinned: false,
      last_accessed_at: 2900,
      created_at: 1400,
      updated_at: 2900,
    },
  ],
};

describe('All Files Fidelity and Interaction Tests', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.getWorkspaceIndex).mockResolvedValue(mockIndexData);
    vi.mocked(ipc.getWorkspaceTree).mockResolvedValue({
      projects: [{ id: 'proj-1', name: 'Deep Space Mission', purpose: '', pages: [], canvases: [], updated_at: 1000 }],
      unassociated_pages: [],
    });
    vi.mocked(ipc.listObjectTypes).mockResolvedValue(mockObjectTypes);
    vi.mocked(ipc.inspectProjectDeletion).mockResolvedValue({
      project_id: 'proj-1',
      project_name: 'Deep Space Mission',
      exclusive_artifacts: [],
      shared_artifacts: [],
    });
  });

  it('renders directory header, search bar, and category tabs with authentic counts', async () => {
    render(
      <WorkspaceIndexView
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
      />
    );

    expect(await screen.findByRole('heading', { level: 1, name: /All Files/i })).toBeInTheDocument();
    expect(screen.getByText('WORKSPACE')).toBeInTheDocument();
    expect(screen.getByTestId('search-input')).toBeInTheDocument();

    // Verify tabs with real counts
    expect(screen.getByTestId('tab-all')).toHaveTextContent(/All Items/);
    expect(screen.getByTestId('tab-all')).toHaveTextContent('5');
    expect(screen.getByTestId('tab-projects')).toHaveTextContent(/Projects/);
    expect(screen.getByTestId('tab-projects')).toHaveTextContent('2');
    expect(screen.getByTestId('tab-notes')).toHaveTextContent(/Notes & Pages/);
    expect(screen.getByTestId('tab-notes')).toHaveTextContent('2');
    expect(screen.getByTestId('tab-objects')).toHaveTextContent(/Custom Objects/);
    expect(screen.getByTestId('tab-objects')).toHaveTextContent('1');
  });

  it('filters by category tab and respects dedicated project grid card presentation', async () => {
    const onOpenProject = vi.fn();
    render(
      <WorkspaceIndexView
        onOpenProject={onOpenProject}
        onOpenArtifact={vi.fn()}
      />
    );

    await screen.findByRole('heading', { level: 1, name: /All Files/i });

    // Switch to Projects tab
    fireEvent.click(screen.getByTestId('tab-projects'));

    // Should display project cards
    expect(await screen.findByText('Deep Space Mission')).toBeInTheDocument();
    expect(screen.getByText('Solar Array System')).toBeInTheDocument();
    // Notes should not be displayed in projects tab
    expect(screen.queryByText('Thermal Shielding Analysis')).not.toBeInTheDocument();

    // Clicking a project card opens it
    fireEvent.click(screen.getByTestId('index-item-proj-2'));
    expect(onOpenProject).toHaveBeenCalledWith('proj-2');
  });

  it('filters notes by scope (All / In Projects / Standalone)', async () => {
    render(
      <WorkspaceIndexView
        initialTab="notes"
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
      />
    );

    await screen.findByText('Thermal Shielding Analysis');
    expect(screen.getByText('Quick Capture Thoughts')).toBeInTheDocument();

    // Filter to Standalone
    fireEvent.click(screen.getByTestId('note-scope-standalone'));
    expect(screen.getByText('Quick Capture Thoughts')).toBeInTheDocument();
    expect(screen.queryByText('Thermal Shielding Analysis')).not.toBeInTheDocument();

    // Filter to In Projects
    fireEvent.click(screen.getByTestId('note-scope-projects'));
    expect(screen.getByText('Thermal Shielding Analysis')).toBeInTheDocument();
    expect(screen.queryByText('Quick Capture Thoughts')).not.toBeInTheDocument();
  });

  it('filters by registered object type badge', async () => {
    render(
      <WorkspaceIndexView
        initialTab="objects"
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
      />
    );

    await screen.findByText('Thruster Valve Spec');

    // Click Decision type pill
    const decisionPill = screen.getByTestId('type-filter-decision');
    fireEvent.click(decisionPill);
    expect(screen.getByText('Thruster Valve Spec')).toBeInTheDocument();

    // Click Idea type pill (which has 0 items)
    const ideaPill = screen.getByTestId('type-filter-idea');
    fireEvent.click(ideaPill);
    expect(screen.getByText('No items found')).toBeInTheDocument();
  });

  it('supports search input, live filtering, and clear button', async () => {
    render(
      <WorkspaceIndexView
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
      />
    );

    const searchInput = await screen.findByTestId('search-input');
    fireEvent.change(searchInput, { target: { value: 'Thermal' } });

    // Should show clear button
    const clearBtn = screen.getByRole('button', { name: /Clear/i });
    expect(clearBtn).toBeInTheDocument();

    fireEvent.click(clearBtn);
    expect(searchInput).toHaveValue('');
  });

  it('toggles item pin without opening row', async () => {
    const onOpenArtifact = vi.fn();
    render(
      <WorkspaceIndexView
        onOpenProject={vi.fn()}
        onOpenArtifact={onOpenArtifact}
      />
    );

    await screen.findByText('Thermal Shielding Analysis');
    const pinBtn = screen.getByTestId('pin-btn-note-1');

    fireEvent.click(pinBtn);
    expect(ipc.togglePin).toHaveBeenCalledWith('note-1', 'note', true);
    expect(onOpenArtifact).not.toHaveBeenCalled();
  });

  it('triggers project deletion confirmation modal from project options menu', async () => {
    render(
      <WorkspaceIndexView
        initialTab="projects"
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
      />
    );

    await screen.findByText('Deep Space Mission');

    // Open project menu
    const menuBtn = screen.getByTestId('project-menu-btn-proj-1');
    fireEvent.click(menuBtn);

    // Click delete option
    const deleteBtn = screen.getByTestId('delete-project-proj-1');
    fireEvent.click(deleteBtn);

    // ProjectDeletionModal should open
    expect(await screen.findByTestId('btn-confirm-delete-project')).toBeInTheDocument();
  });
});
