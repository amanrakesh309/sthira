import { useEffect, useState } from 'react';
import { beforeEach, expect, it, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { WorkspaceGuide, hasSeenWorkspaceWelcome } from '../components/WorkspaceGuide';
import { ContextInfo } from '../components/ContextInfo';
import { helpArticles } from '../components/workspaceHelpArticles';
import { App } from '../App';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === 'cmd_get_google_session') return { status: 'signedOut' };
    if (command === 'cmd_get_object_type_counts') return {};
    if (command === 'cmd_get_knowledge_graph') return { nodes: [], edges: [] };
    if (command === 'cmd_get_workspace_tree') return { projects: [], unassociated_pages: [] };
    return [];
  }),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}));

vi.mock('../api/ipc', async (original) => ({
  ...await original<typeof import('../api/ipc')>(),
  isTauri: true,
  getHomeState: vi.fn(async () => ({
    projects: [{ id: 'p', name: 'Existing project', purpose: '', item_count: 0, last_substantive_at: 1 }],
    recent_items: [], loose_captures: [],
  })),
}));

function Harness({ initial = null }: { initial?: 'welcome' | null }) {
  const [entry, setEntry] = useState<'help' | 'welcome' | null>(initial);
  return <>
    <label>Draft<input defaultValue="Keep my notes" /></label>
    <button type="button" onClick={() => setEntry('help')}>Help</button>
    {entry && <WorkspaceGuide entry={entry} onClose={() => setEntry(null)} />}
  </>;
}

beforeEach(() => localStorage.clear());

it('advances through the introduction with missing controls and remembers completion', async () => {
  const user = userEvent.setup();
  render(<Harness initial="welcome" />);
  expect(screen.getByRole('dialog', { name: 'Welcome to Sthira' })).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Start tour' }));
  const first = screen.getByRole('dialog', { name: 'Capture an idea' });
  expect(within(first).getByText('Home → Capture')).toBeInTheDocument();
  expect(within(first).getByText('1 of 4')).toBeInTheDocument();
  expect(within(first).getByRole('button', { name: 'Close help, Capture an idea' }))
    .toHaveAccessibleDescription(expect.stringContaining('Type a thought'));
  await user.click(within(first).getByRole('button', { name: 'Next' }));
  expect(screen.getByRole('button', { name: 'Close help, Open and edit pages' })).toHaveFocus();
  await user.click(screen.getByRole('button', { name: 'Previous step' }));
  expect(screen.getByRole('button', { name: 'Close help, Capture an idea' })).toHaveFocus();
  for (let i = 0; i < 3; i++) await user.click(screen.getByRole('button', { name: 'Next' }));
  await user.click(screen.getByRole('button', { name: 'Done' }));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(hasSeenWorkspaceWelcome()).toBe(true);
  expect(screen.getByRole('textbox', { name: 'Draft' })).toHaveValue('Keep my notes');
});

it('remembers a skip and replays tours from searchable offline Help', async () => {
  const user = userEvent.setup();
  const view = render(<Harness initial="welcome" />);
  await user.click(screen.getByRole('button', { name: 'Skip' }));
  expect(hasSeenWorkspaceWelcome()).toBe(true);
  view.unmount();
  render(<Harness />);
  const help = screen.getByRole('button', { name: 'Help' });
  await user.click(help);
  const dialog = screen.getByRole('dialog', { name: 'Workspace help' });
  const search = within(dialog).getByRole('textbox', { name: 'Search help topics' });
  await user.type(search, 'migration');
  expect(within(dialog).queryByRole('button', { name: /Capture an idea/ })).not.toBeInTheDocument();
  await user.click(within(dialog).getByRole('button', { name: /Bring in existing notes/ }));
  expect(within(dialog).getByText(/Preview supported files/)).toBeVisible();
  await user.clear(search);
  await user.click(within(dialog).getByRole('button', { name: 'Full workspace tour' }));
  expect(screen.getByRole('button', { name: 'Close help, Capture an idea' })).toHaveFocus();
  await user.keyboard('{Escape}');
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(help).toHaveFocus();
});

it('reveals help on demand and dismisses it with Escape', async () => {
  const user = userEvent.setup();
  render(<ContextInfo title="templates">A default applies to future runs.</ContextInfo>);
  const button = screen.getByRole('button', { name: 'About templates' });
  expect(screen.queryByRole('note')).not.toBeInTheDocument();
  await user.click(button);
  expect(screen.getByRole('note')).toHaveTextContent('A default applies to future runs.');
  await user.keyboard('{Escape}');
  expect(screen.queryByRole('note')).not.toBeInTheDocument();
  expect(button).toHaveFocus();
});


it('closes information before Escape reaches its enclosing modal', async () => {
  const user = userEvent.setup();
  const closeModal = vi.fn();
  function ModalFixture() {
    useEffect(() => {
      const onEscape = (event: KeyboardEvent) => {
        if (event.key === 'Escape') closeModal();
      };
      document.addEventListener('keydown', onEscape, true);
      return () => document.removeEventListener('keydown', onEscape, true);
    }, []);
    return <div role="dialog" aria-label="Settings">
      <ContextInfo title="workflow setup">Choose a model for a workflow.</ContextInfo>
    </div>;
  }
  render(<ModalFixture />);
  await user.click(screen.getByRole('button', { name: 'About workflow setup' }));
  await user.keyboard('{Escape}');
  expect(screen.queryByRole('note')).not.toBeInTheDocument();
  expect(closeModal).not.toHaveBeenCalled();
  await user.keyboard('{Escape}');
  expect(closeModal).toHaveBeenCalledOnce();
});

it.each([
  ['Connections', 'Explore connections', 'Follow a reference or backlink'],
  ['Memory', 'Save shared memory', 'Choose what the agent can remember'],
  ['Agent review', 'Give the agent a goal', 'Review proposals and tasks'],
  ['Resume', 'Open the factual brief', 'Check the sources and continue'],
  ['AI setup', 'Connect models and search', 'Enable a workflow'],
])('opens the dedicated %s walkthrough', async (title, first, second) => {
  const user = userEvent.setup();
  render(<Harness />);
  await user.click(screen.getByRole('button', { name: 'Help' }));
  await user.click(screen.getByRole('button', { name: title }));
  expect(screen.getByRole('button', { name: `Close help, ${first}` })).toHaveFocus();
  await user.click(screen.getByRole('button', { name: 'Next' }));
  expect(screen.getByRole('button', { name: `Close help, ${second}` })).toHaveFocus();
});

it('searches bundled article bodies and follows an offline article link', async () => {
  const user = userEvent.setup();
  render(<Harness />);
  await user.click(screen.getByRole('button', { name: 'Help' }));
  const search = screen.getByRole('textbox', { name: 'Search help topics' });
  await user.type(search, 'sthira.db');
  await user.click(screen.getByRole('button', { name: 'Back up and recover your vault' }));
  expect(screen.getByRole('button', { name: 'Close help, Back up and recover your vault' })).toHaveFocus();
  expect(screen.getByText('sthira.db', { exact: true })).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Back to Help' }));
  await user.clear(screen.getByRole('textbox', { name: 'Search help topics' }));
  await user.click(screen.getByRole('button', { name: 'Workspace basics' }));
  await user.click(screen.getByRole('link', { name: 'References and backlinks' }));
  expect(screen.getByRole('button', { name: 'Close help, References and backlinks' })).toHaveFocus();
  expect(window.location.hash).toBe('');
});

it('loads a contextual article and opens its specified settings section', async () => {
  const user = userEvent.setup();
  const open = vi.fn();
  render(<WorkspaceGuide entry="help" initialArticle="troubleshooting" onClose={vi.fn()} onOpenSettings={open} />);
  expect(screen.getByRole('button', { name: 'Close help, Troubleshoot setup and failed runs' })).toHaveFocus();
  await user.click(screen.getByRole('button', { name: 'Open Workflows' }));
  expect(open).toHaveBeenCalledWith('workflows');
});


it('bundles a nonempty excerpt for each article', () => {
  for (const article of Object.values(helpArticles)) expect(article.markdown.length, article.title).toBeGreaterThan(100);
});

it('keeps the recorded welcome dismissal when new walkthroughs are available', async () => {
  localStorage.setItem('sthira.guide.welcome.v1', 'seen');
  const user = userEvent.setup();
  render(<Harness />);
  expect(hasSeenWorkspaceWelcome()).toBe(true);
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Help' }));
  expect(screen.getByRole('button', { name: 'AI setup' })).toBeVisible();
});

it('shows welcome once on a populated app, automatically offers each page tour once, and replays on request', async () => {
  const user = userEvent.setup();
  const firstLaunch = render(<App />);
  const welcome = await screen.findByRole('dialog', { name: 'Welcome to Sthira' });
  await user.click(within(welcome).getByRole('button', { name: 'Skip' }));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

  const openGraph = async () => { await user.click(screen.getByRole('button', { name: 'Knowledge Graph' })); };
  const openHome = async () => { await user.click(screen.getByRole('button', { name: 'Home' })); };
  await openGraph();
  expect(await screen.findByRole('dialog', { name: 'See the knowledge graph' })).toBeInTheDocument();
  await user.keyboard('{Escape}');
  expect(screen.getByRole('heading', { name: 'Knowledge Graph' })).toBeInTheDocument();
  await openHome();
  await openGraph();
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

  const replay = screen.getByRole('button', { name: 'Show page tour' });
  await user.click(replay);
  await user.click(within(screen.getByRole('dialog', { name: 'See the knowledge graph' })).getByRole('button', { name: 'Done' }));
  expect(replay).toHaveFocus();
  firstLaunch.unmount();

  render(<App />);
  await screen.findByText('Existing project');
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  await openGraph();
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

  await user.click(screen.getByRole('button', { name: 'Memory' }));
  expect(await screen.findByRole('dialog', { name: 'Save shared memory' })).toBeInTheDocument();
  await user.keyboard('{Escape}');

  const settings = screen.getByRole('button', { name: 'Settings' });
  await user.click(settings);
  expect(await screen.findByRole('dialog', { name: 'Connect models and search' })).toBeInTheDocument();
  await user.keyboard('{Escape}');
  expect(screen.getByRole('dialog', { name: 'Settings' })).toBeInTheDocument();
  await user.keyboard('{Escape}');
  await user.click(settings);
  expect(screen.queryByRole('dialog', { name: 'Connect models and search' })).not.toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Show Settings tour' }));
  expect(screen.getByRole('dialog', { name: 'Connect models and search' })).toBeInTheDocument();
});
