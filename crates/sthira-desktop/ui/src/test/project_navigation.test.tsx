import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { App } from '../App';
import * as ipc from '../api/ipc';
import { getProjectOverview } from '../api/projects';
import { setEditorMarkdown } from './editor-helpers';
import type { Artifact, ProjectOverviewData } from '../types/sthira';

vi.mock('../api/projects', () => ({ getProjectOverview: vi.fn() }));
vi.mock('../api/ipc', async (original) => ({
  ...await original<typeof ipc>(), isTauri: false,
  getHomeState: vi.fn(), getWorkspaceTree: vi.fn(async () => ({ projects: [], unassociated_pages: [] })), getSidebarItems: vi.fn(async () => ({ projects: [], notes: [], objects: [] })),
  getProviderCatalog: vi.fn(async () => []), listObjectTypes: vi.fn(async () => []), getObjectTypeCounts: vi.fn(async () => ({})), getObjectInstance: vi.fn(async () => null),
  getArtifactContext: vi.fn(), getResumeBrief: vi.fn(), getProjectHistory: vi.fn(async (projectId) => ({ project_id: projectId, narrative_segments: [], milestones: [] })), saveArtifact: vi.fn(), onScheduledTopicReady: vi.fn(async () => () => {}),
}));
vi.mock('../components/CanvasView', () => ({ CanvasView: ({ canvasId, projectId, onBack }: { canvasId: string; projectId: string; onBack: () => void }) => <section data-testid="selected-project-canvas" data-canvas-id={canvasId} data-project-id={projectId}><button type="button" onClick={onBack}>Back from canvas</button></section> }));

const timestamp = 1_700_000_000_000;
const overview: ProjectOverviewData = {
  project: { id: 'p', name: 'Knowledge platform', purpose: 'Keep project work connected', status: 'active', created_at: timestamp, last_substantive_at: timestamp },
  items: [
    { id: 'own', title: 'Architecture notes', kind: 'artifact', item_type: 'note', description: 'Saved architecture notes', project_id: 'p', object_type_id: null, object_type_name: null, icon: null, color: null, roles: ['member'], created_at: timestamp, updated_at: timestamp, is_draft: false, item_count: null },
    { id: 'remote', title: 'Remote source', kind: 'artifact', item_type: 'note', description: 'Referenced from another project', project_id: 'q', object_type_id: null, object_type_name: null, icon: null, color: null, roles: ['reference'], created_at: timestamp, updated_at: timestamp, is_draft: false, item_count: null },
    { id: 'canvas', title: 'Concept board', kind: 'canvas', item_type: 'canvas', description: '', project_id: 'p', object_type_id: null, object_type_name: null, icon: null, color: null, roles: ['member'], created_at: timestamp, updated_at: timestamp, is_draft: false, item_count: 0 },
  ],
};
const artifact = (id: string): Artifact => ({ id, artifact_type: 'note', authorship: 'user', title: id === 'remote' ? 'Remote source' : 'Architecture notes', content_md: '# Saved note\n\nSaved document content.', content_hash: '', version: 1, is_unfinished_draft: false, created_at: timestamp, updated_at: timestamp });

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getProjectOverview).mockResolvedValue(overview);
  vi.mocked(ipc.getHomeState).mockResolvedValue({ projects: [{ id: 'p', name: 'Knowledge platform', purpose: 'Keep project work connected', item_count: 2, last_substantive_at: timestamp, category: 'Active', icon_type: 'folder' }], recent_items: [], loose_captures: [] });
  vi.mocked(ipc.getArtifactContext).mockImplementation(async (id) => ({ artifact: artifact(id), epistemic_nodes: [], pending_proposals: [], project_id: id === 'remote' ? 'q' : 'p', is_unfinished_draft: false }));
  vi.mocked(ipc.getResumeBrief).mockImplementation(async (id) => ({ project_id: id, level: 1, purpose: id === 'p' ? 'Root project' : 'Other project', recent_progress: [], conflicting_decisions: [], active_working_breadcrumb: `Project: ${id}`, continuation_point: '', primary_continuation_artifact_id: 'own', pane_hint: { collapse: { reason: 'active_session' } } }));
  vi.mocked(ipc.saveArtifact).mockImplementation(async (id, content, title) => ({ ...artifact(id), content_md: content, title }));
});

describe('Project overview routing', () => {
  it('opens project contents instead of resolving the first continuation artifact', async () => {
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole('button', { name: 'Open project: Knowledge platform' }));
    expect(await screen.findByTestId('project-overview')).toBeInTheDocument();
    expect(await screen.findByRole('button', { name: 'Open Architecture notes' })).toBeInTheDocument();
    expect(screen.queryByRole('textbox', { name: 'Document content' })).not.toBeInTheDocument();
    expect(ipc.getArtifactContext).not.toHaveBeenCalled(); expect(ipc.getResumeBrief).not.toHaveBeenCalled();
  });

  it('returns from a referenced page to the original project and restores its filters', async () => {
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole('button', { name: 'Open project: Knowledge platform' }));
    await user.type(await screen.findByRole('textbox', { name: 'Search project contents' }), 'Remote');
    await user.click(screen.getByRole('button', { name: 'Open Remote source' }));
    expect(await screen.findByRole('textbox', { name: 'Document content' })).toBeInTheDocument();
    expect(ipc.getArtifactContext).toHaveBeenCalledWith('remote');
    expect(ipc.getResumeBrief).toHaveBeenCalledWith('q', 1);
    await user.click(screen.getByRole('button', { name: 'Project' }));
    expect(await screen.findByRole('heading', { name: 'Knowledge platform' })).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: 'Search project contents' })).toHaveValue('Remote');
    expect(screen.queryByRole('button', { name: 'Open Architecture notes' })).not.toBeInTheDocument();
  });

  it('saves edits under the selected page’s project before returning to the overview', async () => {
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole('button', { name: 'Open project: Knowledge platform' }));
    await user.click(await screen.findByRole('button', { name: 'Open Architecture notes' }));
    const editor = await screen.findByRole('textbox', { name: 'Document content' });
    setEditorMarkdown(editor, 'Edited project content');
    await user.click(screen.getByRole('button', { name: 'Project' }));
    await screen.findByRole('heading', { name: 'Knowledge platform' });
    expect(ipc.saveArtifact).toHaveBeenCalledWith('own', expect.stringContaining('Edited project content'), expect.anything(), 'p');
  });

  it('opens the selected project canvas in the center and returns to project contents', async () => {
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole('button', { name: 'Open project: Knowledge platform' }));
    await user.click(await screen.findByRole('button', { name: 'Open Concept board' }));
    const canvas = await screen.findByTestId('selected-project-canvas');
    expect(canvas).toHaveAttribute('data-canvas-id', 'canvas'); expect(canvas).toHaveAttribute('data-project-id', 'p');
    await user.click(screen.getByRole('button', { name: 'Back from canvas' }));
    expect(await screen.findByRole('button', { name: 'Open Concept board' })).toBeInTheDocument();
    expect(ipc.getArtifactContext).not.toHaveBeenCalled();
  });

  it('returns to the Projects directory from the project overview', async () => {
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole('button', { name: 'Open project: Knowledge platform' }));
    await screen.findByRole('heading', { name: 'Knowledge platform' });
    await user.click(screen.getByRole('button', { name: 'Projects' }));
    await waitFor(() => expect(screen.getByTestId('workspace-index-view')).toBeInTheDocument());
  });

  it('handles invalid project query parameters gracefully without crashing (NAV-06)', async () => {
    vi.mocked(getProjectOverview).mockRejectedValueOnce(new Error('Project not found'));
    const user = userEvent.setup(); render(<App />);
    await user.click(await screen.findByRole('button', { name: 'Open project: Knowledge platform' }));
    expect(await screen.findByRole('heading', { name: 'Project unavailable' })).toBeInTheDocument();
    expect(screen.getByText(/Couldn’t load this project/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });
});
