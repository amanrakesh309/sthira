import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { GraphView } from '../views/GraphView';
import type { KnowledgeGraphData, ObjectType } from '../types/sthira';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe('Knowledge Graph Fidelity & Interaction Tests', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const mockGraphData: KnowledgeGraphData = {
    nodes: [
      {
        id: 'proj-1',
        label: 'Testing core functionality',
        kind: 'project',
        item_count: 2,
        color: '#49583D',
        icon: '📁',
      },
      {
        id: 'proj-2',
        label: 'Test research',
        kind: 'project',
        item_count: 1,
        color: '#49583D',
        icon: '📁',
      },
      {
        id: 'note-1',
        label: 'Executive Summary',
        kind: 'artifact',
        authorship: 'user',
        draft_status: false,
        icon: '📄',
        project_id: 'proj-1',
      },
      {
        id: 'note-2',
        label: 'Learning System Design from Scratch',
        kind: 'artifact',
        authorship: 'user',
        draft_status: true,
        icon: '📄',
        project_id: 'proj-1',
      },
      {
        id: 'obj-decision-1',
        label: 'requirement data',
        kind: 'object',
        object_type: 'decision',
        authorship: 'user',
        draft_status: false,
        icon: '⚖️',
        color: '#7C3AED',
        project_id: 'proj-1',
      },
    ],
    edges: [
      { source: 'note-1', target: 'proj-1', relationship: 'belongs_to' },
      { source: 'note-2', target: 'proj-1', relationship: 'belongs_to' },
      { source: 'obj-decision-1', target: 'proj-1', relationship: 'belongs_to' },
    ],
  };

  const mockObjectTypes: ObjectType[] = [
    {
      id: 'decision',
      display_name: 'Decision',
      icon: '⚖️',
      color: '#7C3AED',
      schema: [],
      is_system: true,
      created_at: 1000,
    },
  ];

  it('renders editorial header, eyebrow, title, and live count badge', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'cmd_get_knowledge_graph') return mockGraphData;
      if (cmd === 'cmd_list_object_types') return mockObjectTypes;
      return null;
    });

    render(
      <GraphView
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onBack={vi.fn()}
      />
    );

    expect(await screen.findByTestId('graph-view-container')).toBeInTheDocument();
    expect(screen.getByText('Explore Your Knowledge')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Knowledge Graph' })).toBeInTheDocument();

    const nodeCount = screen.getByTestId('graph-node-count');
    const edgeCount = screen.getByTestId('graph-edge-count');
    expect(nodeCount).toHaveTextContent('5');
    expect(edgeCount).toHaveTextContent('3');
  });

  it('filters nodes dynamically by type pills and updates counts', async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'cmd_get_knowledge_graph') return mockGraphData;
      if (cmd === 'cmd_list_object_types') return mockObjectTypes;
      return null;
    });

    render(
      <GraphView
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onBack={vi.fn()}
      />
    );

    expect(await screen.findByTestId('graph-node-count')).toHaveTextContent('5');

    // Toggle off projects
    const filterProjects = screen.getByTestId('filter-projects');
    await user.click(filterProjects);

    await waitFor(() => {
      // 5 total - 2 projects = 3
      expect(screen.getByTestId('graph-node-count')).toHaveTextContent('3');
    });

    // Toggle off pages
    const filterPages = screen.getByTestId('filter-pages');
    await user.click(filterPages);

    await waitFor(() => {
      // 3 remaining - 2 pages = 1 (decision object)
      expect(screen.getByTestId('graph-node-count')).toHaveTextContent('1');
    });
  });

  it('supports real-time search filtering via search input and keyboard focus', async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'cmd_get_knowledge_graph') return mockGraphData;
      if (cmd === 'cmd_list_object_types') return mockObjectTypes;
      return null;
    });

    render(
      <GraphView
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onBack={vi.fn()}
      />
    );

    const searchInput = await screen.findByTestId('graph-search-input');
    await user.type(searchInput, 'Executive');
    expect(searchInput).toHaveValue('Executive');

    // Press Escape to clear search
    await user.keyboard('{Escape}');
    expect(searchInput).toHaveValue('');
  });

  it('renders floating legend card and viewport controls', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'cmd_get_knowledge_graph') return mockGraphData;
      if (cmd === 'cmd_list_object_types') return mockObjectTypes;
      return null;
    });

    render(
      <GraphView
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
        onBack={vi.fn()}
      />
    );

    expect(await screen.findByText('Legend')).toBeInTheDocument();
    expect(screen.getByText('A workspace for focused work (e.g., research, build, plan)')).toBeInTheDocument();
    expect(screen.getByText('Knowledge, research, and content')).toBeInTheDocument();
    expect(screen.getByText('Structured data, decisions, or custom entities')).toBeInTheDocument();

    // Viewport zoom toolbar
    expect(screen.getByTitle('Zoom In')).toBeInTheDocument();
    expect(screen.getByTitle('Zoom Out')).toBeInTheDocument();
    expect(screen.getByTitle('Fullscreen View')).toBeInTheDocument();
    expect(screen.getByTitle('Fit to View')).toBeInTheDocument();
    expect(screen.getByTitle('Reset View')).toBeInTheDocument();
    expect(screen.getByTitle('Minimap Radar View')).toBeInTheDocument();
  });
});
