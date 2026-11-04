import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { WorkspaceIndexView } from '../views/WorkspaceIndexView';
import * as ipc from '../api/ipc';
import type { WorkspaceIndexData, WorkspaceIndexItem } from '../types/sthira';

vi.mock('../api/ipc', () => ({
  getWorkspaceIndex: vi.fn(), getWorkspaceTree: vi.fn(), listObjectTypes: vi.fn(),
  togglePin: vi.fn(), addArtifactToProject: vi.fn(), inspectProjectDeletion: vi.fn(),
}));
vi.mock('../components/PageActionMenu', () => ({ PageActionMenu: () => null }));
const item = (id: string, entity_type: 'project' | 'note' = 'note'): WorkspaceIndexItem => ({
  id, title: id, entity_type, standalone: entity_type === 'note',
  is_pinned: false, created_at: 1000, updated_at: 1000, last_accessed_at: 1000,
});
const data = (items: WorkspaceIndexItem[]): WorkspaceIndexData => ({
  counts: { all: items.length, projects: items.filter(x => x.entity_type === 'project').length,
    notes: items.filter(x => x.entity_type === 'note').length, objects: 0,
    standalone: items.filter(x => x.standalone).length }, items,
});
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(ipc.getWorkspaceIndex).mockResolvedValue(data([item('Alpha', 'project')]));
  vi.mocked(ipc.getWorkspaceTree).mockResolvedValue({ projects: [], unassociated_pages: [] });
  vi.mocked(ipc.listObjectTypes).mockResolvedValue([]);
  vi.mocked(ipc.togglePin).mockResolvedValue();
});

it('keyboard activation of project actions does not activate the row', async () => {
  const user = userEvent.setup();
  const onOpenProject = vi.fn();
  render(<WorkspaceIndexView onOpenProject={onOpenProject} onOpenArtifact={vi.fn()} />);
  const menu = await screen.findByTestId('project-menu-btn-Alpha');
  menu.focus();
  await user.keyboard('{Enter}');
  expect(onOpenProject).not.toHaveBeenCalled();
  expect(screen.getByTestId('delete-project-Alpha')).toBeInTheDocument();
});

it('Escape dismisses the project menu', async () => {
  const user = userEvent.setup();
  render(<WorkspaceIndexView onOpenProject={vi.fn()} onOpenArtifact={vi.fn()} />);
  await user.click(await screen.findByTestId('project-menu-btn-Alpha'));
  await user.keyboard('{Escape}');
  expect(screen.queryByTestId('delete-project-Alpha')).not.toBeInTheDocument();
});

it('Tab leaves the portaled menu for the next row action', async () => {
  const user = userEvent.setup();
  render(<WorkspaceIndexView onOpenProject={vi.fn()} onOpenArtifact={vi.fn()} />);
  await user.click(await screen.findByTestId('project-menu-btn-Alpha'));
  await user.tab();
  expect(screen.queryByTestId('delete-project-Alpha')).not.toBeInTheDocument();
  expect(screen.getByTestId('pin-btn-Alpha')).toHaveFocus();
});

it('keeps the menu action mounted when WebKit blurs a button before its click', async () => {
  vi.mocked(ipc.getWorkspaceIndex).mockResolvedValue(data([item('Note')]));
  vi.mocked(ipc.getWorkspaceTree).mockResolvedValue({ projects: [{ id: 'project-1', name: 'Target project', purpose: '', pages: [], canvases: [], updated_at: 1000 }], unassociated_pages: [] });
  vi.mocked(ipc.addArtifactToProject).mockResolvedValue();
  const user = userEvent.setup();
  render(<WorkspaceIndexView onOpenProject={vi.fn()} onOpenArtifact={vi.fn()} />);
  await user.click(await screen.findByTestId('attach-project-btn-Note'));
  const action = screen.getByTestId('attach-target-project-1');
  // macOS WebKit pointer clicks can move focus to the document rather than the button.
  fireEvent.blur(action, { relatedTarget: document.body });
  expect(action).toBeInTheDocument();
  fireEvent.click(action);
  await waitFor(() => expect(ipc.addArtifactToProject).toHaveBeenCalledWith('Note', 'project-1'));
});

it('failed directory loading offers visible recovery instead of an empty vault', async () => {
  vi.mocked(ipc.getWorkspaceIndex).mockRejectedValue(new Error('Vault unavailable'));
  render(<WorkspaceIndexView onOpenProject={vi.fn()} onOpenArtifact={vi.fn()} />);
  expect(await screen.findByRole('alert')).toHaveTextContent('Couldn’t load the workspace directory');
  expect(screen.getByRole('button', { name: /retry|try again/i })).toBeInTheDocument();
});

it('an older search response cannot replace newer results', async () => {
  let finishOld!: (value: WorkspaceIndexData) => void;
  const oldResult = new Promise<WorkspaceIndexData>(resolve => { finishOld = resolve; });
  vi.mocked(ipc.getWorkspaceIndex).mockImplementation(async (_tab, search) => {
    if (search === 'old') return oldResult;
    return data([item(search === 'new' ? 'New result' : 'Initial result')]);
  });
  render(<WorkspaceIndexView onOpenProject={vi.fn()} onOpenArtifact={vi.fn()} />);
  await screen.findByText('Initial result');
  fireEvent.change(screen.getByTestId('search-input'), { target: { value: 'old' } });
  fireEvent.change(screen.getByTestId('search-input'), { target: { value: 'new' } });
  await screen.findByText('New result');
  await act(async () => { finishOld(data([item('Old result')])); });
  expect(screen.queryByText('New result')).toBeInTheDocument();
  expect(screen.queryByText('Old result')).not.toBeInTheDocument();
});

it('Previous works on the first click after refreshed results shrink the page count', async () => {
  const items = Array.from({ length: 51 }, (_, i) => item(`Note ${String(i + 1).padStart(2, '0')}`));
  vi.mocked(ipc.getWorkspaceIndex).mockResolvedValue(data(items));
  render(<WorkspaceIndexView onOpenProject={vi.fn()} onOpenArtifact={vi.fn()} />);
  await screen.findByText('Note 01');
  fireEvent.click(screen.getByRole('button', { name: '3' }));
  await screen.findByText('Note 51');
  vi.mocked(ipc.getWorkspaceIndex).mockResolvedValue(data(items.slice(0, 26)));
  fireEvent.click(screen.getByTestId('pin-btn-Note 51'));
  await waitFor(() => expect(screen.getByText('Showing 26–26 of 26 items')).toBeInTheDocument());
  fireEvent.click(screen.getByRole('button', { name: 'Previous page' }));
  expect(screen.getByText('Showing 1–25 of 26 items')).toBeInTheDocument();
});
