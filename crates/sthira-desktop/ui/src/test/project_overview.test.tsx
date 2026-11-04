import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ProjectOverviewView } from '../views/ProjectOverviewView';
import { getProjectOverview } from '../api/projects';
import * as ipc from '../api/ipc';
import * as connections from '../api/connections';
import type { ProjectContentItem, ProjectOverviewData } from '../types/sthira';

vi.mock('../api/projects', () => ({ getProjectOverview: vi.fn() }));
vi.mock('../api/ipc', async (original) => ({ ...await original<typeof ipc>(), createPage: vi.fn(), linkArtifactToProject: vi.fn() }));
vi.mock('../api/connections', async (original) => ({ ...await original<typeof connections>(), searchConnectionTargets: vi.fn(), getWorkspaceEntity: vi.fn(), getWorkspaceConnections: vi.fn(), createWorkspaceConnection: vi.fn() }));

const time = 1_700_000_000_000;
const item = (id: string, title: string, extra: Partial<ProjectContentItem> = {}): ProjectContentItem => ({
  id, title, kind: 'artifact', item_type: 'note', description: `# ${title}\n\nSaved context about ${title}.`, project_id: 'p',
  object_type_id: null, object_type_name: null, icon: null, color: null, created_at: time, updated_at: time, is_draft: false, roles: ['member'], item_count: null, ...extra,
});
const initial = (): ProjectOverviewData => ({
  project: { id: 'p', name: 'Knowledge platform', purpose: 'Build a connected, local knowledge workspace.', status: 'active', created_at: time, last_substantive_at: time + 1000 },
  items: [item('a', 'Architecture notes', { updated_at: time + 4000 }), item('b', 'Backend decision', { item_type: 'object', object_type_id: 'decision', object_type_name: 'Decision', icon: '⚖️', color: '#49583D', updated_at: time + 3000 }),
    item('c', 'Concept map', { kind: 'canvas', item_type: 'canvas', description: '', item_count: 3 }),
    item('d', 'Evidence source', { item_type: 'source', updated_at: time + 2000 }),
    item('e', 'Related research', { kind: 'project', item_type: 'project', project_id: null, roles: ['reference'], item_count: 2 }),
    item('f', 'Remote reference', { project_id: 'other-project', roles: ['reference', 'backlink'] })],
});
let data: ProjectOverviewData;
const props = () => ({ projectId: 'p', onBack: vi.fn(), onOpenArtifact: vi.fn(), onOpenCanvas: vi.fn(), onOpenProject: vi.fn(), onWorkspaceMutated: vi.fn() });
const names = () => within(screen.getByRole('table')).getAllByRole('row').slice(1).map((row) => within(within(row).getAllByRole('cell')[0]).getByRole('button').getAttribute('aria-label'));

beforeEach(() => {
  vi.clearAllMocks(); data = initial();
  vi.mocked(getProjectOverview).mockImplementation(async () => data);
  vi.mocked(ipc.createPage).mockResolvedValue({ id: 'new-page', artifact_type: 'note', authorship: 'user', title: 'Untitled page', content_md: '# Untitled page\n\n', content_hash: '', version: 1, is_unfinished_draft: false, created_at: time, updated_at: time });
  vi.mocked(connections.searchConnectionTargets).mockResolvedValue([
    { kind: 'artifact', id: 'a', title: 'Architecture notes', type_label: 'Page', project_id: 'p' },
    { kind: 'artifact', id: 'outside', title: 'Reusable knowledge', type_label: 'Source', project_id: 'other-project' },
  ]);
  vi.mocked(ipc.linkArtifactToProject).mockImplementation(async () => { data = { ...data, items: [...data.items, item('outside', 'Reusable knowledge')] }; });
  vi.mocked(connections.getWorkspaceEntity).mockResolvedValue({ entity: { kind: 'project', id: 'p', title: 'Knowledge platform', type_label: 'Project', project_id: null }, content_md: '', description: '', members: [] });
  vi.mocked(connections.getWorkspaceConnections).mockResolvedValue([]);
  vi.mocked(connections.createWorkspaceConnection).mockImplementation(async () => { data = { ...data, items: [...data.items, item('outside', 'Reusable knowledge', { roles: ['reference'], project_id: 'other-project' })] }; return 'link'; });
});

describe('Project overview', () => {
  it('opens the project brief and all saved entity types without auto-opening any note', async () => {
    const handlers = props(); render(<ProjectOverviewView {...handlers} />);
    expect(await screen.findByRole('heading', { name: 'Knowledge platform' })).toBeInTheDocument();
    expect(screen.getByText('Build a connected, local knowledge workspace.')).toBeInTheDocument();
    expect(names()).toHaveLength(6);
    expect(handlers.onOpenArtifact).not.toHaveBeenCalled(); expect(handlers.onOpenCanvas).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Open Backend decision' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Open Concept map' })).toBeInTheDocument();
  });

  it('combines search, object type, relationship filters and explicit sorting', async () => {
    const user = userEvent.setup(); render(<ProjectOverviewView {...props()} />);
    await screen.findByRole('table');
    await user.type(screen.getByRole('textbox', { name: 'Search project contents' }), 'decision');
    expect(names()).toEqual(['Open Backend decision']);
    await user.clear(screen.getByRole('textbox', { name: 'Search project contents' }));
    await user.selectOptions(screen.getByLabelText('Filter project type'), 'object:decision');
    expect(names()).toEqual(['Open Backend decision']);
    await user.selectOptions(screen.getByLabelText('Filter project type'), 'all');
    await user.selectOptions(screen.getByLabelText('Filter project relationship'), 'reference');
    expect(names()).toHaveLength(2);
    await user.selectOptions(screen.getByLabelText('Sort project contents'), 'title_desc');
    expect(names()).toEqual(['Open Remote reference', 'Open Related research']);
    await user.selectOptions(screen.getByLabelText('Filter project relationship'), 'backlink');
    expect(names()).toEqual(['Open Remote reference']);
    await user.click(screen.getByRole('button', { name: 'Clear filters' }));
    expect(names()).toHaveLength(6);
  });

  it('opens selected artifacts with their actual project context, and routes canvases and projects separately', async () => {
    const user = userEvent.setup(); const handlers = props(); render(<ProjectOverviewView {...handlers} />);
    await user.click(await screen.findByRole('button', { name: 'Open Remote reference' }));
    expect(handlers.onOpenArtifact).toHaveBeenCalledWith('f', 'other-project');
    await user.click(screen.getByRole('button', { name: 'Open Concept map' }));
    expect(handlers.onOpenCanvas).toHaveBeenCalledWith('c', 'p');
    await user.click(screen.getByRole('button', { name: 'Open Related research' }));
    expect(handlers.onOpenProject).toHaveBeenCalledWith('e');
    await user.click(screen.getByRole('button', { name: 'Grid view' }));
    expect(screen.getByRole('button', { name: 'Grid view' })).toHaveAttribute('aria-pressed', 'true');
    await user.click(screen.getByRole('button', { name: 'Open Architecture notes' }));
    expect(handlers.onOpenArtifact).toHaveBeenLastCalledWith('a', 'p');
  });

  it('paginates the filtered result set and resets the page when filters change', async () => {
    data.items = Array.from({ length: 31 }, (_, index) => item(`page-${index}`, `Page ${String(index + 1).padStart(2, '0')}`));
    const user = userEvent.setup(); render(<ProjectOverviewView {...props()} />);
    await screen.findByRole('table');
    expect(names()).toHaveLength(25);
    await user.click(screen.getByRole('button', { name: 'Next project page' }));
    expect(names()).toHaveLength(6); expect(screen.getByRole('status')).toHaveTextContent('26–31 of 31 items');
    await user.type(screen.getByRole('textbox', { name: 'Search project contents' }), 'Page 01');
    expect(names()).toEqual(['Open Page 01']); expect(screen.getByRole('status')).toHaveTextContent('1–1 of 1 items');
    expect(screen.getByRole('button', { name: 'Next project page' })).toBeDisabled();
  });

  it('shows distinct empty-project, no-match and recoverable load-error states', async () => {
    data.items = []; vi.mocked(getProjectOverview).mockRejectedValueOnce(new Error('Unavailable'));
    const user = userEvent.setup(); render(<ProjectOverviewView {...props()} />);
    expect(await screen.findByRole('alert')).toHaveTextContent('Project unavailable');
    expect(screen.queryByText('No items in this project yet')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByText('No items in this project yet')).toBeInTheDocument();
    await user.type(screen.getByRole('textbox', { name: 'Search project contents' }), 'missing');
    expect(screen.getByText('No items match these filters')).toBeInTheDocument();
  });

  it('creates a page only on request and opens the returned saved identity', async () => {
    const user = userEvent.setup(); const handlers = props(); render(<ProjectOverviewView {...handlers} />);
    await screen.findByRole('table'); expect(ipc.createPage).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'New page' }));
    expect(ipc.createPage).toHaveBeenCalledWith('p', 'Untitled page');
    await waitFor(() => expect(handlers.onOpenArtifact).toHaveBeenCalledWith('new-page', 'p'));
  });

  it('adds existing content without duplicating it and refreshes the contents list', async () => {
    const user = userEvent.setup(); const handlers = props(); render(<ProjectOverviewView {...handlers} />);
    await user.click(await screen.findByRole('button', { name: 'Add existing' }));
    const dialog = screen.getByRole('dialog', { name: 'Add an existing item' });
    expect(await within(dialog).findByRole('button', { name: 'Architecture notes already in project' })).toBeDisabled();
    await user.click(within(dialog).getByRole('button', { name: 'Add Reusable knowledge to project' }));
    expect(ipc.linkArtifactToProject).toHaveBeenCalledWith('outside', 'p'); expect(ipc.createPage).not.toHaveBeenCalled();
    expect(await screen.findByRole('button', { name: 'Open Reusable knowledge' })).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(handlers.onWorkspaceMutated).toHaveBeenCalled();
  });

  it('retains add-existing errors and leaves the project unchanged on failure', async () => {
    vi.mocked(ipc.linkArtifactToProject).mockRejectedValueOnce(new Error('Unavailable'));
    const user = userEvent.setup(); render(<ProjectOverviewView {...props()} />);
    await user.click(await screen.findByRole('button', { name: 'Add existing' }));
    await user.click(await screen.findByRole('button', { name: 'Add Reusable knowledge to project' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Couldn’t add this item');
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Open Reusable knowledge' })).not.toBeInTheDocument();
  });

  it('restores browsing state when a selected item is closed', async () => {
    const user = userEvent.setup(); const onBrowseChange = vi.fn();
    const rendered = render(<ProjectOverviewView {...props()} onBrowseChange={onBrowseChange} />);
    await user.type(await screen.findByRole('textbox', { name: 'Search project contents' }), 'Remote');
    await user.click(screen.getByRole('button', { name: 'Grid view' }));
    const browse = onBrowseChange.mock.lastCall![0]; rendered.unmount();
    render(<ProjectOverviewView {...props()} initialBrowse={browse} />);
    expect(await screen.findByRole('button', { name: 'Open Remote reference' })).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: 'Search project contents' })).toHaveValue('Remote');
    expect(screen.getByRole('button', { name: 'Grid view' })).toHaveAttribute('aria-pressed', 'true');
  });

  it('updates the overview when a project connection is added in the side panel', async () => {
    const user = userEvent.setup(); render(<ProjectOverviewView {...props()} />);
    await screen.findByRole('table');
    await user.click(screen.getByRole('button', { name: 'Project connections' }));
    await user.click(await screen.findByRole('button', { name: 'Link an item' }));
    await user.click(await screen.findByRole('button', { name: 'Link Reusable knowledge' }));
    expect(await screen.findByRole('button', { name: 'Open Reusable knowledge' })).toBeInTheDocument();
  });

  it('focuses scoped search with the keyboard shortcut', async () => {
    render(<ProjectOverviewView {...props()} />); await screen.findByRole('table');
    fireEvent.keyDown(document.body, { key: 'f', ctrlKey: true });
    expect(screen.getByRole('textbox', { name: 'Search project contents' })).toHaveFocus();
  });
});
