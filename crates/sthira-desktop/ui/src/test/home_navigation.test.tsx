import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { HomeView } from '../components/HomeView';
import type { HomeStateView } from '../types/sthira';

vi.mock('../api/ipc', () => ({
  getProviderCatalog: vi.fn().mockResolvedValue([]),
  createPage: vi.fn(), saveArtifact: vi.fn(), executePlanAction: vi.fn(), streamAdaptiveRun: vi.fn(),
}));

const empty: HomeStateView = { projects: [], recent_items: [], loose_captures: [] };
const callbacks = () => ({
  onOpenProject: vi.fn(), onOpenArtifact: vi.fn(), onOpenCanvas: vi.fn(),
  onRefresh: vi.fn(), onOpenFlow: vi.fn(), onOpenSettings: vi.fn(),
});
beforeEach(() => vi.clearAllMocks());

describe('Home navigation and input accessibility', () => {
  it('opens projects and project-only continuations with the keyboard', async () => {
    const user = userEvent.setup();
    const handlers = callbacks();
    render(<HomeView {...handlers} homeState={{ ...empty,
      projects: [{ id: 'project', name: 'Architecture', purpose: 'Evaluate alternatives', category: 'Research', item_count: 2, icon_type: 'brain', last_substantive_at: 0 }],
      recent_items: [{ id: 'recent', title: 'Project continuation', subtitle: '', item_type: 'Project', project_id: 'project', timestamp: 0 }],
    }} />);
    screen.getByRole('button', { name: 'Open project: Architecture' }).focus();
    await user.keyboard('{Enter}');
    expect(handlers.onOpenProject).toHaveBeenCalledWith('project');
    handlers.onOpenProject.mockClear();
    screen.getByRole('button', { name: /Project continuation/ }).focus();
    await user.keyboard(' ');
    expect(handlers.onOpenProject).toHaveBeenCalledWith('project');
  });

  it('keeps recent loose captures available and deduplicates actual destinations', async () => {
    const user = userEvent.setup();
    const handlers = callbacks();
    const recent = { id: 'recent', title: 'Captured thought', subtitle: '', item_type: 'Document', artifact_id: 'note', project_id: null, timestamp: 0 };
    render(<HomeView {...handlers} homeState={{ ...empty,
      recent_items: [recent, { ...recent, id: 'duplicate' }],
      loose_captures: [{ id: 'note' } as HomeStateView['loose_captures'][number]],
    }} />);
    expect(screen.getAllByText('Captured thought')).toHaveLength(1);
    await user.click(screen.getByRole('button', { name: /Captured thought/ }));
    expect(handlers.onOpenArtifact).toHaveBeenCalledWith('note', null);
    expect(screen.queryByText(/Review evidence and continue active drafting/)).not.toBeInTheDocument();
  });

  it('opens the actual saved canvas', async () => {
    const user = userEvent.setup();
    const handlers = callbacks();
    render(<HomeView {...handlers} homeState={{ ...empty, recent_items: [
      { id: 'recent', title: 'Concept map', subtitle: '', item_type: 'Canvas', canvas_id: 'canvas-1', timestamp: 0 },
    ] }} />);
    await user.click(screen.getByRole('button', { name: /Concept map/ }));
    expect(handlers.onOpenCanvas).toHaveBeenCalledWith('canvas-1');
  });

  it('exposes loading and retry without claiming an empty vault on failure', async () => {
    const user = userEvent.setup();
    const handlers = callbacks();
    const { rerender } = render(<HomeView {...handlers} homeState={empty} loadStatus="loading" />);
    expect(screen.getByRole('status')).toHaveTextContent('Loading your workspace');
    expect(screen.queryByText('No projects yet')).not.toBeInTheDocument();
    rerender(<HomeView {...handlers} homeState={empty} loadStatus="error" />);
    expect(screen.getByRole('alert')).toHaveTextContent('Couldn’t load your workspace');
    expect(screen.queryByText('No projects yet')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Try again' }));
    expect(handlers.onRefresh).toHaveBeenCalledOnce();
  });

  it('keeps creation flows reachable and dismisses the New popover with Escape', async () => {
    const user = userEvent.setup();
    const handlers = callbacks();
    render(<HomeView {...handlers} homeState={empty} />);
    const trigger = screen.getByRole('button', { name: 'New' });
    await user.click(trigger);
    await user.keyboard('{Escape}');
    expect(trigger).toHaveAttribute('aria-expanded', 'false');
    expect(trigger).toHaveFocus();
    for (const [label, mode] of [['New Note', 'capture'], ['New Research', 'research'], ['New Plan', 'plan']]) {
      await user.click(trigger);
      await user.click(screen.getByRole('button', { name: label }));
      expect(handlers.onOpenFlow).toHaveBeenLastCalledWith(mode);
    }
  });

  it('supports mode cycling and a keyboard exit to accessible controls', async () => {
    const user = userEvent.setup();
    render(<HomeView {...callbacks()} homeState={empty} />);
    const input = screen.getByRole('textbox', { name: 'Your thought or question' });
    await user.click(input);
    await user.keyboard('{Tab}');
    expect(screen.getByTestId('mode-btn-research')).toHaveAttribute('aria-pressed', 'true');
    await user.keyboard('{Escape}');
    expect(screen.getByTestId('mode-btn-research')).toHaveFocus();
    await user.keyboard('{ArrowRight}');
    expect(screen.getByTestId('mode-btn-plan')).toHaveFocus();
    expect(screen.getByTestId('mode-btn-plan')).toHaveAttribute('aria-pressed', 'true');
  });

  it('uses an honest empty-model state and keeps configuration reachable', async () => {
    const user = userEvent.setup();
    const handlers = callbacks();
    render(<HomeView {...handlers} homeState={empty} />);
    expect(screen.queryByTestId('home-model-selector')).not.toBeInTheDocument();
    await user.click(screen.getByTestId('mode-btn-research'));
    await user.click(screen.getByRole('button', { name: 'Select a model' }));
    expect(screen.getByText('No models configured. Capture works without a model.')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /Connect Providers/ }));
    expect(handlers.onOpenSettings).toHaveBeenCalledOnce();
  });
});
