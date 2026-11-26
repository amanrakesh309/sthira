import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { AgentView } from '../views/AgentView';
import { decodeSourceEntities, linkResearchCitations } from '../components/AgentWorkflowReview';
import { DocumentEditReview } from '../components/DocumentEditReview';
import * as api from '../api/workspaceAgent';
import { defaultWorkflowSettings } from '../api/workflowSettings';
import type { AgentRun } from '../api/workspaceAgent';

const searchProviders = vi.hoisted(() => ({ value: [] as { id: string; display_name: string; is_enabled: boolean; is_default: boolean }[] }));
vi.mock('../api/workspaceAgent', async original => ({ ...await original<typeof import('../api/workspaceAgent')>(), listAgentRuns: vi.fn(), getAgentRun: vi.fn(), createAgentRun: vi.fn(), executeAgentRun: vi.fn(), cancelAgentRun: vi.fn(), reviewAgentStep: vi.fn(), undoAgentEdit: vi.fn(), followUpAgentRun: vi.fn() }));
vi.mock('../api/ipc', () => ({
  isTauri: true,
  getWorkspaceTree: vi.fn(async () => ({ projects: [{ id: 'project-a', name: 'Roots and Reason' }] })),
  getProviderCatalog: vi.fn(async () => [{ id: 'p', display_name: 'Configured provider', is_enabled: true, models: [{ id: 'm', provider_id: 'p', display_name: 'Chosen model', is_enabled: true, is_default: true }] }]),
  getArtifactContext: vi.fn(),
  getSearchProviders: vi.fn(async () => searchProviders.value),
}));
vi.mock('../api/agentHandoffs', () => ({ listAgentHandoffs: vi.fn(async () => []), listHandoffConnections: vi.fn(async () => []), getAgentHandoff: vi.fn(), saveAgentHandoff: vi.fn(), closeAgentHandoff: vi.fn(), continueAgentHandoff: vi.fn() }));
vi.mock('../api/sharedMemory', () => ({ memoryAction: vi.fn() }));
vi.mock('../context/AgentTasksContext', () => ({ useAgentTasks: () => ({ tasks: [], loading: false, error: null, refresh: vi.fn(), revision: 0 }) }));
vi.mock('../hooks/useWorkflowSettings', () => ({ useWorkflowSettings: () => ({ settings: { ...defaultWorkflowSettings(), agent: { enabled: true, model_id: 'm', templates_enabled: false }, research: { enabled: true, model_id: 'm', templates_enabled: true }, plan: { enabled: true, model_id: 'm', templates_enabled: true } }, loading: false, error: null }) }));
vi.mock('../api/synthesisTemplates', async original => ({
  ...await original<typeof import('../api/synthesisTemplates')>(),
  listSynthesisTemplates: vi.fn(async () => [
    { id: 'research-four-lenses', name: 'Research: Cultural Tradition — Four Lenses', target_mode: 'research', description: '', is_default: false, template_skeleton_md: '# {{title}}\n\n## Scope', created_at: 0, is_builtin: true, version: 2, guidance: '', updated_at: 0, archived_at: null },
    { id: 'plan-mvp-roadmap', name: 'Plan: MVP / Product Roadmap', target_mode: 'plan', description: '', is_default: false, template_skeleton_md: '# {{title}}', created_at: 0, is_builtin: true, version: 1, guidance: '', updated_at: 0, archived_at: null },
  ]),
}));
const saved: AgentRun = { id: 'run-a', goal: 'Research a tradition', project_id: 'project-a', workspace_memory: false, model_id: 'm', model_name: 'Chosen model', provider_id: 'p', provider_name: 'Configured provider', status: 'completed', summary: 'Done.', handoff: '', error: null, created_at: 1, updated_at: 1, steps: [], allow_web: false };
beforeEach(() => { vi.clearAllMocks(); searchProviders.value = []; vi.mocked(api.listAgentRuns).mockResolvedValue([]); vi.mocked(api.createAgentRun).mockResolvedValue({ ...saved, status: 'queued' }); vi.mocked(api.executeAgentRun).mockResolvedValue(saved); vi.mocked(api.getAgentRun).mockResolvedValue(saved); });
afterEach(cleanup);

it('defaults web research off and sends the chosen template modes and explicit template', async () => {
  const user = userEvent.setup();
  render(<AgentView onOpenArtifact={vi.fn()} />);
  await screen.findByRole('option', { name: 'Research: Cultural Tradition — Four Lenses · r2' });
  await user.click(screen.getByText('Research & Plan tools (optional)'));
  expect(screen.getByLabelText('Allow web research')).not.toBeChecked();
  await user.selectOptions(screen.getByLabelText('Agent research template'), 'research-four-lenses');
  await user.selectOptions(screen.getByLabelText('Agent plan template'), 'none');
  await user.type(screen.getByRole('textbox', { name: 'Agent goal' }), 'Research a tradition');
  await user.click(screen.getByRole('button', { name: 'Start working' }));
  await waitFor(() => expect(api.createAgentRun).toHaveBeenCalledWith({
    goal: 'Research a tradition', project_id: null, workspace_memory: false, model_id: 'm',
    allow_web: false, research_template_mode: 'explicit', research_template_id: 'research-four-lenses',
    plan_template_mode: 'none', plan_template_id: null,
  }));
});

it('blocks allowing web research when no search provider is configured', async () => {
  const user = userEvent.setup();
  render(<AgentView onOpenArtifact={vi.fn()} />);
  await screen.findByRole('option', { name: /Chosen model/ });
  await user.click(screen.getByText('Research & Plan tools (optional)'));
  await user.click(screen.getByLabelText('Allow web research'));
  await user.type(screen.getByRole('textbox', { name: 'Agent goal' }), 'Search the web');
  await user.click(screen.getByRole('button', { name: 'Start working' }));
  expect(await screen.findByRole('alert')).toHaveTextContent(/no enabled search provider/i);
  expect(api.createAgentRun).not.toHaveBeenCalled();
});

it.each([true, false])('reviews a research proposal in the scoped project before applying once (destination included: %s)', async (hasDestination) => {
  const step = {
    id: 'step-r', sequence: 1,
    action: { type: 'propose_research' as const, query: 'Research a tradition', use_web: true, template_id: null },
    state: 'pending', artifact_id: null, turn_id: null, change_set_id: null, basis_content_md: null,
    result: { packet: {
      operation: 'research', title: 'Roots and Reason: Four Lenses', content_md: '# Four Lenses\n\nReported practice varies.',
      ...(hasDestination ? { destination: { kind: 'new_page', project_id: 'project-a' } } : {}),
      sources: [{ url: 'https://example.org/tradition', title: 'Tradition source', index: 1 }],
      external_sources_count: 1, search_attempted: true,
      template: { id: 'research-four-lenses', name: 'Research: Cultural Tradition — Four Lenses', version: 2 },
      model_name: 'Chosen model', provider_name: 'Configured provider', search_status: 'web_search_success', search_provider: 'Local SearXNG',
      web_grant: true, web_used: true, warnings: ['Check regional differences.'], stages: [{ message: 'Searching the web…' }],
    } },
  };
  const run = { ...saved, status: 'review', steps: [step] };
  vi.mocked(api.listAgentRuns).mockResolvedValue([run]); vi.mocked(api.getAgentRun).mockResolvedValue(run);
  vi.mocked(api.reviewAgentStep).mockResolvedValue({ ...run, status: 'completed', steps: [{ ...step, state: 'applied', artifact_id: 'created-a' }] });
  const user = userEvent.setup();
  render(<AgentView onOpenArtifact={vi.fn()} />);
  await user.click(await screen.findByRole('button', { name: /Research a tradition/ }));
  await user.click(screen.getByRole('button', { name: 'Review proposal' }));
  const dialog = await screen.findByRole('dialog', { name: 'Review changes' });
  expect(dialog).toHaveTextContent('Research proposal');
  expect(dialog).toHaveTextContent('Create research page');
  expect(dialog).toHaveTextContent('Create an AI-authored research page in Roots and Reason.');
  expect(dialog).toHaveTextContent('web_search_success');
  expect(dialog).toHaveTextContent('Local SearXNG');
  expect(dialog).toHaveTextContent('Research: Cultural Tradition — Four Lenses (revision 2)');
  expect(dialog).toHaveTextContent('Check regional differences.');
  expect(dialog).toHaveTextContent('Tradition source');
  expect(api.reviewAgentStep).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'Create research page' }));
  await waitFor(() => expect(api.reviewAgentStep).toHaveBeenCalledWith('run-a', 'step-r', true));
});

it('keeps fallback wording truthful when web was allowed and attempted but zero external sources contributed', async () => {
  const step = {
    id: 'step-fallback', sequence: 1,
    action: { type: 'propose_research' as const, query: 'Research a tradition', use_web: true, template_id: null },
    state: 'pending', artifact_id: null, turn_id: null, change_set_id: null, basis_content_md: null,
    result: { packet: {
      operation: 'research', title: 'Local fallback', content_md: '# Local fallback\n\nNo external evidence.',
      sources: [], destination: { kind: 'new_page', project_id: 'project-a' },
      search_status: 'web_search_failed', evidence_mode: 'local_vault_only',
      web_grant: true, web_used: false, search_requested: true, search_attempted: true,
      external_sources_count: 0, template: null, model_name: 'Chosen model', provider_name: 'Configured provider',
      warnings: ['Web search blocked by anti-bot challenge.'], stages: [{ message: 'Web search blocked by anti-bot challenge.' }],
    } },
  };
  const run = { ...saved, status: 'review', steps: [step] };
  vi.mocked(api.listAgentRuns).mockResolvedValue([run]); vi.mocked(api.getAgentRun).mockResolvedValue(run);
  const user = userEvent.setup();
  render(<AgentView onOpenArtifact={vi.fn()} />);
  await user.click(await screen.findByRole('button', { name: /Research a tradition/ }));
  await user.click(screen.getByRole('button', { name: 'Review proposal' }));
  const dialog = await screen.findByRole('dialog', { name: 'Review changes' });
  expect(dialog).toHaveTextContent('Web research was allowed and attempted, but this proposal used zero external sources.');
  expect(dialog).not.toHaveTextContent('Web research contributed');
  expect(dialog).toHaveTextContent('web_search_failed');
  expect(dialog).toHaveTextContent('Web search blocked by anti-bot challenge.');
});

it('decodes source titles and navigates valid citation groups and ranges to numbered sources', async () => {
  const step = {
    id: 'step-citations', sequence: 1,
    action: { type: 'propose_research' as const, query: 'Research a tradition', use_web: true, template_id: null },
    state: 'pending', artifact_id: null, turn_id: null, change_set_id: null, basis_content_md: null,
    result: { packet: {
      operation: 'research', title: 'Citations',
      content_md: '# Citations\n\nFirst claim [Source 1]. Second claim [Sources 1–2]. Invalid [Source 9].',
      sources: [
        { url: 'https://example.org/one', title: 'NASA&#039;s first article', index: 1 },
        { url: 'https://example.org/two', title: 'Second &amp; final article &#1114112; title', index: 2 },
      ],
      documents: [
        { url: 'https://example.org/one', from_cache: false, fetched_at: 100 },
        { url: 'https://example.org/two', from_cache: true, fetched_at: 50 },
      ],
      destination: { kind: 'new_page', project_id: 'project-a' }, search_status: 'web_search_success',
      web_grant: true, web_used: true, search_requested: true, search_attempted: true, external_sources_count: 2,
      template: null, model_name: 'Chosen model', provider_name: 'Configured provider', warnings: [], stages: [],
    } },
  };
  const run = { ...saved, status: 'review', steps: [step] };
  vi.mocked(api.listAgentRuns).mockResolvedValue([run]); vi.mocked(api.getAgentRun).mockResolvedValue(run);
  const user = userEvent.setup();
  render(<AgentView onOpenArtifact={vi.fn()} />);
  await user.click(await screen.findByRole('button', { name: /Research a tradition/ }));
  await user.click(screen.getByRole('button', { name: 'Review proposal' }));
  const dialog = await screen.findByRole('dialog', { name: 'Review changes' });
  expect(dialog).toHaveTextContent("NASA's first article");
  expect(dialog).toHaveTextContent('Second & final article &#1114112; title');
  expect(dialog).toHaveTextContent('Article provenance: 1 fetched this run, 1 from cache.');
  expect(dialog).not.toHaveTextContent('&#039;');
  expect(dialog).toHaveTextContent('[Source 9]');
  expect(screen.queryByRole('link', { name: 'Source 9' })).not.toBeInTheDocument();
  const citationLinks = dialog.querySelectorAll<HTMLAnchorElement>('.research-citation-link');
  expect(citationLinks.length).toBe(3);
  expect(citationLinks[0]).toHaveAttribute('href', '#research-source-1');
  expect(citationLinks[2]).toHaveAttribute('href', '#research-source-2');
  citationLinks[0].focus();
  await user.keyboard('{Enter}');
  expect(document.activeElement?.id).toBe('research-source-1');
  await user.click(citationLinks[2]);
  expect(document.activeElement?.id).toBe('research-source-2');
  const sourceLink = screen.getByRole('link', { name: "NASA's first article" });
  expect(sourceLink).toHaveAttribute('href', 'https://example.org/one');
  expect(sourceLink).toHaveAttribute('target', '_blank');
});


it('shows genuine live progress for the active run and keeps it scoped to that run', async () => {
  const running: AgentRun = { ...saved, status: 'running', summary: null, progress_stage: 'generating', progress_message: 'Generating a plan with Research: Cultural Tradition — Four Lenses (revision 2)…' };
  vi.mocked(api.listAgentRuns).mockResolvedValue([running]);
  vi.mocked(api.getAgentRun).mockResolvedValue(running);
  const user = userEvent.setup();
  render(<AgentView onOpenArtifact={vi.fn()} />);
  await user.click(await screen.findByRole('button', { name: /Research a tradition/ }));
  const progress = await screen.findByTestId('agent-progress');
  expect(progress).toHaveTextContent('Generating a plan with Research: Cultural Tradition — Four Lenses (revision 2)…');
  expect(screen.getByText(/Web research off/)).toBeInTheDocument();
});


it('decodes malformed entities safely and leaves citation-shaped code or mixed groups inert', () => {
  expect(decodeSourceEntities("Webb&#039;s &rsquo;Article&rdquo; &#1114112; &#12ab; &#0; title")).toBe("Webb's ’Article” &#1114112; &#12ab; &#0; title");
  const numbers = new Set([1, 2, 3]);
  expect(linkResearchCitations('[Source 1, nonsense]', numbers)).toBe('[Source 1, nonsense]');
  expect(linkResearchCitations('[Source 1e1] [Source +1]', numbers)).toBe('[Source 1e1] [Source +1]');
  expect(linkResearchCitations('`[Source 1]` [Source 1]', numbers)).toBe('`[Source 1]` [Source 1](#research-source-1)');
  expect(linkResearchCitations('```\n```rust\n[Source 1]\n```\n[Source 1]', numbers)).toBe('```\n```rust\n[Source 1]\n```\n[Source 1](#research-source-1)');
  expect(linkResearchCitations('[Source 1](https://example.test/unrelated)', numbers)).toBe('[Source 1](#research-source-1)');
  expect(linkResearchCitations('[Source 1](sthira://artifact/local-1)', numbers)).toBe('[Source 1](#research-source-1)');
  expect(linkResearchCitations('[Source 9](https://example.test/unrelated)', numbers)).toBe('[Source 9]');
  expect(linkResearchCitations('[ordinary](https://example.test/page)', numbers)).toBe('[ordinary](https://example.test/page)');
  expect(linkResearchCitations('[Sources 1–3]', numbers)).toBe('[Source 1](#research-source-1), [Source 2](#research-source-2), [Source 3](#research-source-3)');
  const sparse = new Set([2]);
  expect(linkResearchCitations('[Source 1] [Source 2](https://example.test/two)', sparse)).toBe('[Source 1] [Source 2](#research-source-2)');
  expect(linkResearchCitations('`multiline\n[Source 1]\ncode` and ``two\n[Source 2]\nlines`` then [Source 1]', numbers)).toBe('`multiline\n[Source 1]\ncode` and ``two\n[Source 2]\nlines`` then [Source 1](#research-source-1)');
  expect(linkResearchCitations('Summary:\n\n    const ref = "[Source 1]";\n    // [Source 2]\n\nFollow-up [Source 2].', numbers)).toBe('Summary:\n\n    const ref = "[Source 1]";\n    // [Source 2]\n\nFollow-up [Source 2](#research-source-2).');
  expect(linkResearchCitations('\\[Source 1\\] and [see](https://example.test/[Source 1]) and [Source 3]', numbers)).toBe('\\[Source 1\\] and [see](https://example.test/[Source 1]) and [Source 3](#research-source-3)');
  expect(linkResearchCitations('| Note | Ref |\n| --- | --- |\n| Item | [Source 1] |', numbers)).toBe('| Note | Ref |\n| --- | --- |\n| Item | [Source 1](#research-source-1) |');
});

it('renders actual sparse source numbers in Retrieved sources and focuses sparse Source 2 on keyboard activation', async () => {
  const step = {
    id: 'step-sparse-r', sequence: 1,
    action: { type: 'propose_research' as const, query: 'sparse citations', use_web: true, template_id: null },
    state: 'pending', artifact_id: null, turn_id: null, change_set_id: null, basis_content_md: null,
    result: { packet: {
      operation: 'research', title: 'Sparse Citation Report',
      content_md: [
        'Valid claim [Source 2] and missing [Source 1] and [Source 2, bogus].',
        '',
        'Inline `multiline\n[Source 2]\ncode` and [ordinary](https://example.org/other).',
        '',
        '    indented = "[Source 2]"',
      ].join('\n'),
      sources: [{ url: 'https://example.org/two', title: 'Second &amp; Sparse', index: 2 }],
      documents: [{ url: 'https://example.org/two', from_cache: false, fetched_at: 200 }],
      destination: { kind: 'new_page', project_id: 'project-a' }, search_status: 'web_search_success',
      web_grant: true, web_used: true, search_requested: true, search_attempted: true, external_sources_count: 1,
      template: null, model_name: 'Chosen model', provider_name: 'Configured provider', warnings: [], stages: [],
    } },
  };
  const run = { ...saved, status: 'review', steps: [step] };
  vi.mocked(api.listAgentRuns).mockResolvedValue([run]); vi.mocked(api.getAgentRun).mockResolvedValue(run);
  const user = userEvent.setup();
  render(<AgentView onOpenArtifact={vi.fn()} />);
  await user.click(await screen.findByRole('button', { name: /Research a tradition/ }));
  await user.click(screen.getByRole('button', { name: 'Review proposal' }));
  const dialog = await screen.findByRole('dialog', { name: 'Review changes' });
  const sourceItem = dialog.querySelector('#research-source-2');
  expect(sourceItem).not.toBeNull();
  expect(sourceItem).toHaveAttribute('value', '2');
  expect(sourceItem).toHaveTextContent(/^2\.Second & Sparse$/);
  expect(dialog.querySelector('ol.agent-source-list')).not.toHaveTextContent(/^1\./);
  expect(screen.queryByRole('link', { name: 'Source 1' })).not.toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'ordinary' })).toHaveAttribute('href', 'https://example.org/other');
  const citationLinks = dialog.querySelectorAll<HTMLAnchorElement>('.research-citation-link');
  expect(citationLinks).toHaveLength(1);
  expect(citationLinks[0]).toHaveAttribute('href', '#research-source-2');
  citationLinks[0].focus();
  await user.keyboard('{Enter}');
  expect(document.activeElement?.id).toBe('research-source-2');
});

it('reviews a local project research proposal with in-app sources and dismisses without applying', async () => {
  const step = {
    id: 'step-local-r', sequence: 1,
    action: { type: 'propose_research' as const, query: 'tradition', use_web: false, template_id: null },
    state: 'pending', artifact_id: null, turn_id: null, change_set_id: null, basis_content_md: null,
    result: { packet: {
      operation: 'research', title: 'Local Roots Research',
      content_md: '# Local Roots Research\n\nObserved practice [Source 1](sthira://artifact/local-note-1).\n\n- [Allowed tradition](sthira://artifact/local-note-1)',
      destination: { kind: 'new_page', project_id: 'project-a' },
      sources: [{ url: 'sthira://artifact/local-note-1', title: 'Allowed &amp; Local Tradition', index: 1 }],
      documents: [{ url: 'sthira://artifact/local-note-1', from_cache: false, fetched_at: null }],
      external_sources_count: 0, search_attempted: false,
      template: { id: 'research-four-lenses', name: 'Research: Cultural Tradition — Four Lenses', version: 2 },
      model_name: 'Chosen model', provider_name: 'Configured provider', search_status: 'local_vault_only',
      web_grant: false, web_used: false, warnings: [], stages: [],
    } },
  };
  const run = { ...saved, status: 'review', steps: [step] };
  vi.mocked(api.listAgentRuns).mockResolvedValue([run]); vi.mocked(api.getAgentRun).mockResolvedValue(run);
  vi.mocked(api.reviewAgentStep).mockResolvedValue({ ...run, status: 'completed', steps: [{ ...step, state: 'dismissed' }] });
  const open = vi.fn(), user = userEvent.setup();
  render(<AgentView onOpenArtifact={open} />);
  await user.click(await screen.findByRole('button', { name: /Research a tradition/ }));
  await user.click(screen.getByRole('button', { name: 'Review proposal' }));
  const dialog = await screen.findByRole('dialog', { name: 'Review changes' });
  expect(dialog).toHaveTextContent('Create an AI-authored research page in Roots and Reason.');
  expect(dialog).toHaveTextContent('Research: Cultural Tradition — Four Lenses (revision 2)');
  expect(dialog).toHaveTextContent('Web research was off for this run.');
  expect(dialog).not.toHaveTextContent('Article provenance:');
  const localSourceBtn = screen.getByRole('button', { name: 'Allowed & Local Tradition' });
  await user.click(localSourceBtn);
  expect(open).toHaveBeenCalledWith('local-note-1');
  await user.click(await screen.findByRole('button', { name: 'Review proposal' }));
  await user.click(screen.getByRole('button', { name: 'Dismiss' }));
  await waitFor(() => expect(api.reviewAgentStep).toHaveBeenCalledWith('run-a', 'step-local-r', false));
});

it('reviews a follow-up plan proposal showing the accepted research source and lineage', async () => {
  const step = {
    id: 'step-plan', sequence: 1,
    action: { type: 'propose_plan' as const, goal: 'Turn saved research into a launch plan', source_artifact_id: 'research-1', template_id: null },
    state: 'pending', artifact_id: null, turn_id: null, change_set_id: null, basis_content_md: null,
    result: { packet: {
      operation: 'plan', title: 'Launch Plan', content_md: '# Launch Plan\n\n- Publish guide.',
      destination: { kind: 'new_page', project_id: 'project-a' },
      source: { artifact_id: 'research-1', title: 'Local Roots Research', version: 1, content_hash: 'h1' },
      derivation: { type: 'plan_from_research', source: { artifact_id: 'research-1' } },
      template: { id: 'plan-mvp-roadmap', name: 'Plan: MVP / Product Roadmap', version: 1 },
      model_name: 'Chosen model', provider_name: 'Configured provider',
      web_grant: false, web_used: false, warnings: [], stages: [],
    } },
  };
  const run = { ...saved, status: 'review', steps: [step] };
  vi.mocked(api.listAgentRuns).mockResolvedValue([run]); vi.mocked(api.getAgentRun).mockResolvedValue(run);
  vi.mocked(api.reviewAgentStep).mockResolvedValue({ ...run, status: 'completed', steps: [{ ...step, state: 'applied', artifact_id: 'plan-1' }] });
  const open = vi.fn(), user = userEvent.setup();
  render(<AgentView onOpenArtifact={open} />);
  await user.click(await screen.findByRole('button', { name: /Research a tradition/ }));
  await user.click(screen.getByRole('button', { name: 'Review proposal' }));
  const dialog = await screen.findByRole('dialog', { name: 'Review changes' });
  expect(dialog).toHaveTextContent('Plan proposal');
  expect(dialog).toHaveTextContent('Derived from the saved page “Local Roots Research” (version 1).');
  expect(dialog).toHaveTextContent('Plan: MVP / Product Roadmap (revision 1)');
  expect(screen.getByRole('button', { name: 'Open saved source page' })).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Create plan page' }));
  await waitFor(() => expect(api.reviewAgentStep).toHaveBeenCalledWith('run-a', 'step-plan', true));
});

it('only intercepts internal citation fragments in Research review', () => {
  const noop = vi.fn();
  const { rerender } = render(<DocumentEditReview current="Before" proposed="[Jump](#research-source-1)" summary="Ordinary" stale={false} pending={false} busy={false} error={null} onClose={noop} onApply={noop} onDismiss={noop} />);
  expect(screen.getByRole('link', { name: 'Jump' })).toHaveAttribute('href', '#research-source-1');
  rerender(<DocumentEditReview researchCitations current="Before" proposed="[Source 1](#research-source-1)" summary="Research" stale={false} pending={false} busy={false} error={null} onClose={noop} onApply={noop} onDismiss={noop} />);
  expect(screen.getByRole('link', { name: 'Source 1' })).toHaveClass('research-citation-link');
});
