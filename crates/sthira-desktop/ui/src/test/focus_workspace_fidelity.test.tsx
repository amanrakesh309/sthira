import { useState } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { FocusView } from '../components/FocusView';
import { ContextualResumePane } from '../components/ContextualResumePane';
import { ProjectHistoryDrawer } from '../components/ProjectHistoryDrawer';
import { RichDocumentRenderer } from '../components/RichDocumentRenderer';
import * as ipc from '../api/ipc';
import type { ArtifactContext, BriefClaim, ProjectHistory, ResumeBrief } from '../types/sthira';

vi.mock('../api/ipc', async (original) => ({
  ...await original<typeof ipc>(), getArtifactContext: vi.fn(), getResumeBrief: vi.fn(),
  getProjectHistory: vi.fn(), getProviderCatalog: vi.fn(), listObjectTypes: vi.fn(),
  getObjectInstance: vi.fn(), saveArtifact: vi.fn(),
}));
const context = (id = 'alpha'): ArtifactContext => ({
  artifact: { id, title: id === 'alpha' ? 'Alpha' : 'Beta', artifact_type: 'note', authorship: 'user',
    content_md: '# Alpha\n\nOriginal draft.', content_hash: 'hash', version: 1,
    is_unfinished_draft: false, created_at: 1000, updated_at: 1000 },
  epistemic_nodes: [], pending_proposals: [], project_id: 'project-a', is_unfinished_draft: false,
});
const claim = (i: number): BriefClaim => ({
  text: `Finding ${i}: ${'A grounded statement with supporting evidence and detail for the complete review. '.repeat(5)}`,
  origin: i === 2 ? 'ai_inferred' : i === 3 ? 'source_backed' : 'user_recorded', rendering: 'verbatim_quote',
  evidence_node_ids: [`node-${i}`], artifact_ids: [`source-${i}`, `source-${i}-b`],
});
const brief: ResumeBrief = {
  project_id: 'project-a', level: 1, purpose: 'A deliberately long project purpose to exercise the compact word budget without dropping evidence provenance or navigation.',
  recent_progress: [claim(1), claim(2), claim(3), claim(4)], conflicting_decisions: [claim(5)],
  active_working_breadcrumb: 'Project: Alpha > Active thread', pane_hint: { show: { reason: 'cold_return' } },
  continuation_point: 'Continue the unfinished analysis by reviewing saved findings and comparing recorded alternatives against the current project purpose.',
  primary_continuation_artifact_id: 'alpha',
};
const history: ProjectHistory = { project_id: 'project-a', narrative_segments: [], milestones: [] };
const props = () => ({ artifactId: 'alpha', projectId: 'project-a', onBack: vi.fn(), onOpenCanvas: vi.fn() });
const save = async (id: string, content: string, title?: string | null) => ({ ...context(id).artifact, content_md: content, title });

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(ipc.getArtifactContext).mockImplementation(async (id) => context(id));
  vi.mocked(ipc.getResumeBrief).mockResolvedValue(brief);
  vi.mocked(ipc.getProjectHistory).mockResolvedValue(history);
  vi.mocked(ipc.getProviderCatalog).mockResolvedValue([]);
  vi.mocked(ipc.listObjectTypes).mockResolvedValue([]);
  vi.mocked(ipc.getObjectInstance).mockResolvedValue(null);
  vi.mocked(ipc.saveArtifact).mockImplementation(save);
});

describe('Focus composition and document safety', () => {
  it('bounds Level 1 copy and exposes all Level 2 findings, conflicts and source links', async () => {
    const user = userEvent.setup();
    render(<FocusView {...props()} />);
    await screen.findByRole('textbox', { name: 'Document Markdown' });
    const compact = screen.getByTestId('contextual-resume-pane');
    expect((compact.textContent || '').trim().split(/\s+/).length).toBeLessThanOrEqual(75);
    expect(within(compact).getByText('AI inference')).toBeInTheDocument();
    expect(within(compact).getByText('Conflicting decisions')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Expand Review' }));
    expect(ipc.getResumeBrief).toHaveBeenLastCalledWith('project-a', 2);
    for (const item of [...brief.recent_progress, ...brief.conflicting_decisions]) expect(screen.getByText(item.text.trim())).toBeInTheDocument();
    expect(screen.getByText('Source-backed')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: /Open source/ })).toHaveLength(10);
  });

  it('opens every real evidence destination from expanded review', async () => {
    const user = userEvent.setup();
    const onSelectArtifact = vi.fn();
    render(<ContextualResumePane brief={brief} isOpen isExpandedReview onToggleOpen={vi.fn()} onSelectArtifact={onSelectArtifact} />);
    await user.click(screen.getAllByRole('button', { name: /Open source 2/ })[0]);
    expect(onSelectArtifact).toHaveBeenCalledWith('source-1-b');
  });

  it('respects the collapse hint and keeps the margin closed by default', async () => {
    vi.mocked(ipc.getResumeBrief).mockResolvedValue({ ...brief, pane_hint: { collapse: { reason: 'active_session' } } });
    const user = userEvent.setup();
    render(<FocusView {...props()} />);
    await screen.findByRole('textbox', { name: 'Document Markdown' });
    expect(screen.queryByTestId('contextual-resume-pane')).not.toBeInTheDocument();
    expect(screen.queryByTestId('epistemic-margin-sidebar')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /Resume Brief/ }));
    expect(screen.getByTestId('contextual-resume-pane')).toBeInTheDocument();
  });

  it('recovers from a load failure without exposing or saving a stale editor', async () => {
    vi.mocked(ipc.getArtifactContext).mockRejectedValueOnce(new Error('Vault unavailable'));
    const user = userEvent.setup();
    render(<FocusView {...props()} />);
    expect(await screen.findByRole('alert')).toHaveTextContent('Couldn’t open this document');
    expect(screen.queryByTestId('textarea-editor')).not.toBeInTheDocument();
    expect(ipc.saveArtifact).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByRole('textbox', { name: 'Document Markdown' })).toHaveValue('# Alpha\n\nOriginal draft.');
  });

  it('keeps dirty text on save failure and retries the same artifact/project', async () => {
    const user = userEvent.setup();
    const handlers = props();
    render(<FocusView {...handlers} />);
    const editor = await screen.findByRole('textbox', { name: 'Document Markdown' });
    fireEvent.change(editor, { target: { value: '# Alpha\n\nUnsaved local text.' } });
    vi.mocked(ipc.saveArtifact).mockRejectedValue(new Error('Disk unavailable'));
    fireEvent.blur(editor);
    await screen.findByRole('button', { name: 'Retry save' });
    await user.click(screen.getByRole('button', { name: 'Home' }));
    expect(handlers.onBack).not.toHaveBeenCalled();
    expect(editor).toHaveValue('# Alpha\n\nUnsaved local text.');
    vi.mocked(ipc.saveArtifact).mockImplementation(save);
    await user.click(screen.getByRole('button', { name: 'Retry save' }));
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
    expect(ipc.saveArtifact).toHaveBeenLastCalledWith('alpha', '# Alpha\n\nUnsaved local text.', 'Alpha', 'project-a');
  });

  it('saves dirty text under the old project before a sidebar document switch', async () => {
    const handlers = props();
    const { rerender } = render(<FocusView {...handlers} />);
    const editor = await screen.findByRole('textbox', { name: 'Document Markdown' });
    fireEvent.change(editor, { target: { value: '# Alpha\n\nChanges before switching.' } });
    rerender(<FocusView {...handlers} artifactId="beta" projectId="project-b" />);
    await waitFor(() => expect(screen.getByTestId('input-artifact-title')).toHaveValue('Beta'));
    expect(ipc.saveArtifact).toHaveBeenCalledWith('alpha', '# Alpha\n\nChanges before switching.', 'Alpha', 'project-a');
    expect(ipc.saveArtifact).not.toHaveBeenCalledWith('alpha', expect.anything(), expect.anything(), 'project-b');
  });

  it('counts distinct primary and Markdown sources in the margin', async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.getArtifactContext).mockResolvedValue({ ...context(), artifact: { ...context().artifact, source_url: 'https://example.com/primary', content_md: '# Alpha\n\n[One](https://example.com/one) [Again](https://example.com/one)' } });
    render(<FocusView {...props()} />);
    await screen.findByRole('textbox', { name: 'Document Markdown' });
    await user.click(screen.getByTestId('btn-toggle-epistemic-margin'));
    await user.click(screen.getByTestId('btn-margin-tab-sources'));
    const margin = screen.getByTestId('epistemic-margin-sidebar');
    expect(within(margin).getByText('2')).toBeInTheDocument();
    expect(within(margin).getAllByRole('link')).toHaveLength(3);
    expect(screen.getByTestId('btn-margin-tab-sources')).toHaveAttribute('aria-pressed', 'true');
  });

  it('hides only the duplicate leading H1 without changing the saved Markdown', async () => {
    const user = userEvent.setup();
    const onUpdateContent = vi.fn();
    render(<RichDocumentRenderer documentTitle="Alpha" content={'# Alpha\n\n## Next\n\n- [ ] Preserve title\n\n# A different heading'} onUpdateContent={onUpdateContent} />);
    expect(screen.queryByRole('heading', { name: 'Alpha' })).not.toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'A different heading' })).toBeInTheDocument();
    await user.click(screen.getByRole('checkbox', { name: 'Preserve title' }));
    expect(onUpdateContent).toHaveBeenCalledWith('# Alpha\n\n## Next\n\n- [x] Preserve title\n\n# A different heading');
  });

  it('shows empty history and restores keyboard focus after Escape', async () => {
    const user = userEvent.setup();
    const Harness = () => {
      const [open, setOpen] = useState(false);
      return <><button onClick={() => setOpen(true)}>Open history</button><ProjectHistoryDrawer history={history} isOpen={open} onClose={() => setOpen(false)} /></>;
    };
    render(<Harness />);
    const trigger = screen.getByRole('button', { name: 'Open history' });
    await user.click(trigger);
    expect(screen.getByRole('dialog', { name: 'Project History' })).toBeInTheDocument();
    expect(screen.getByText('No milestones recorded yet.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Close Drawer' })).toHaveFocus();
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it('keeps evidence disclosures reachable inside the history keyboard loop', async () => {
    const user = userEvent.setup();
    render(<ProjectHistoryDrawer history={{ ...history, milestones: [{
      seq: 1, occurred_at: 1000, title: 'Initial goal', attribution: 'you_recorded',
      milestone_type: 'initial_purpose', evidence_ids: ['purpose-1'],
    }] }} isOpen onClose={vi.fn()} />);
    const close = screen.getByRole('button', { name: 'Close Drawer' });
    expect(close).toHaveFocus();
    await user.tab();
    expect(screen.getByText('Evidence references (1)')).toHaveFocus();
    await user.click(screen.getByText('Evidence references (1)'));
    expect(screen.getByText('purpose-1')).toBeVisible();
    await user.tab();
    expect(close).toHaveFocus();
    await user.tab({ shift: true });
    expect(screen.getByText('Evidence references (1)')).toHaveFocus();
  });
});
