vi.mock('../api/conversation', () => ({ getDocumentConversation: vi.fn().mockResolvedValue({mode:'document', initial_prompt:'', turns:[]}), undoDocumentEdit: vi.fn() }));
import { editorMarkdown, setEditorMarkdown } from './editor-helpers';
import { useState } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { FocusView } from '../components/FocusView';
import { ContextualResumePane } from '../components/ContextualResumePane';
import { ProjectHistoryDrawer } from '../components/ProjectHistoryDrawer';
import { RichDocumentRenderer } from '../components/RichDocumentRenderer';
import * as ipc from '../api/ipc';
import { getWorkspaceEntity } from '../api/connections';
import type { ArtifactContext, BriefClaim, ProjectHistory, ResumeBrief, WorkspaceEntityDetail } from '../types/sthira';

vi.mock('../api/connections', async (original) => ({
  ...await original<typeof import('../api/connections')>(), getWorkspaceEntity: vi.fn(),
}));

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
  vi.mocked(getWorkspaceEntity).mockImplementation(async entity => ({
    entity: { ...entity, title: entity.id === 'project-a' ? 'Knowledge > platform' : 'Other project', type_label: 'Project', project_id: null },
    content_md: '', description: '', members: [],
  }));
});

describe('Focus composition and document safety', () => {
  it('labels a model-side research failure honestly and keeps its full message readable', async () => {
    const message="Model error: Ollama's generation endpoint was not found. Check the server address and API format in Settings → AI & Models. Your inquiry is still saved.";
    render(<FocusView {...props()} documentRun={{artifactId:'alpha',query:'How are model architectures chosen?',kind:'research',mode:'always_search',status:'error',message,error:message,streamedMarkdown:'',refreshToken:0}}/>);
    await screen.findByRole('textbox',{name:'Document content'});
    expect(screen.getByTestId('document-run-banner')).toHaveTextContent('Research failed');
    expect(screen.getByTestId('document-run-message')).toHaveTextContent(message);
    expect(screen.getByTestId('document-run-message')).not.toHaveClass('truncate');
    expect(screen.getByRole('textbox',{name:'Document content'})).toHaveTextContent('Original draft.');
  });

  it('does not open Resume when the current draft cannot be saved', async () => {
    vi.mocked(ipc.saveArtifact).mockRejectedValue(new Error('Disk unavailable'));
    const user = userEvent.setup();
    render(<FocusView {...props()} />);
    const editor = await screen.findByRole('textbox', { name: 'Document content' });
    setEditorMarkdown(editor, 'Keep this unsaved draft.');
    await user.click(screen.getByRole('button', { name: /^Resume$/ }));
    expect(ipc.getResumeBrief).not.toHaveBeenCalled();
    expect(screen.queryByTestId('contextual-resume-pane')).not.toBeInTheDocument();
    expect(editor).toHaveTextContent('Keep this unsaved draft.');
  });
  it('bounds Level 1 copy and exposes all Level 2 findings, conflicts and source links', async () => {
    const user = userEvent.setup();
    render(<FocusView {...props()} />);
    await screen.findByRole('textbox', { name: 'Document content' });
    expect(screen.queryByTestId('contextual-resume-pane')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /^Resume$/ }));
    const compact = await screen.findByTestId('contextual-resume-pane');
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

  it('keeps Resume and the margin closed until explicitly requested', async () => {
    vi.mocked(ipc.getResumeBrief).mockResolvedValue({ ...brief, pane_hint: { collapse: { reason: 'active_session' } } });
    const user = userEvent.setup();
    render(<FocusView {...props()} />);
    await screen.findByRole('textbox', { name: 'Document content' });
    expect(screen.queryByTestId('contextual-resume-pane')).not.toBeInTheDocument();
    expect(screen.queryByTestId('epistemic-margin-sidebar')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /^Resume$/ }));
    expect(await screen.findByTestId('contextual-resume-pane')).toBeInTheDocument();
  });

  it('recovers from a load failure without exposing or saving a stale editor', async () => {
    vi.mocked(ipc.getArtifactContext).mockRejectedValueOnce(new Error('Vault unavailable'));
    const user = userEvent.setup();
    render(<FocusView {...props()} />);
    expect(await screen.findByRole('alert')).toHaveTextContent('Couldn’t open this document');
    expect(screen.queryByTestId('document-editor')).not.toBeInTheDocument();
    expect(ipc.saveArtifact).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByRole('textbox', { name: 'Document content' })).toHaveTextContent('Original draft.');
  });

  it('keeps dirty text on save failure and retries the same artifact/project', async () => {
    const user = userEvent.setup();
    const handlers = props();
    render(<FocusView {...handlers} />);
    const editor = await screen.findByRole('textbox', { name: 'Document content' });
    setEditorMarkdown(editor, '# Alpha\n\nUnsaved local text.');
    vi.mocked(ipc.saveArtifact).mockRejectedValue(new Error('Disk unavailable'));
    fireEvent.blur(editor);
    await screen.findByRole('button', { name: 'Retry save' });
    await user.click(screen.getByRole('button', { name: 'Home' }));
    expect(handlers.onBack).not.toHaveBeenCalled();
    expect(editorMarkdown(editor)).toBe('Unsaved local text.');
    vi.mocked(ipc.saveArtifact).mockImplementation(save);
    await user.click(screen.getByRole('button', { name: 'Retry save' }));
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
    expect(ipc.saveArtifact).toHaveBeenLastCalledWith('alpha', '# Alpha\n\nUnsaved local text.', 'Alpha', 'project-a');
  });

  it('saves dirty text under the old project before a sidebar document switch', async () => {
    const handlers = props();
    const { rerender } = render(<FocusView {...handlers} />);
    const editor = await screen.findByRole('textbox', { name: 'Document content' });
    setEditorMarkdown(editor, '# Alpha\n\nChanges before switching.');
    rerender(<FocusView {...handlers} artifactId="beta" projectId="project-b" />);
    await waitFor(() => expect(screen.getByTestId('input-artifact-title')).toHaveValue('Beta'));
    expect(ipc.saveArtifact).toHaveBeenCalledWith('alpha', '# Alpha\n\nChanges before switching.', 'Alpha', 'project-a');
    expect(ipc.saveArtifact).not.toHaveBeenCalledWith('alpha', expect.anything(), expect.anything(), 'project-b');
  });

  it('counts distinct primary and Markdown sources in the margin', async () => {
    const user = userEvent.setup();
    vi.mocked(ipc.getArtifactContext).mockResolvedValue({ ...context(), artifact: { ...context().artifact, source_url: 'https://example.com/primary', content_md: '# Alpha\n\n[One](https://example.com/one) [Again](https://example.com/one)' } });
    render(<FocusView {...props()} />);
    await screen.findByRole('textbox', { name: 'Document content' });
    await user.click(screen.getByRole('button', { name: 'Page actions' }));
    await user.click(screen.getByRole('button', { name: 'Sources & insights' }));
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

it('uses the Notes breadcrumb for standalone navigation and makes the current title focusable', async () => {
  const user=userEvent.setup();const openNotes=vi.fn();
  vi.mocked(ipc.getArtifactContext).mockResolvedValue({ ...context(), project_id: null });
  render(<FocusView {...props()} projectId={undefined} onOpenStandaloneNotes={openNotes} />);
  await screen.findByRole('textbox',{name:'Document content'});
  await user.click(screen.getByRole('button',{name:'Edit document title: Alpha'}));
  expect(screen.getByRole('textbox',{name:'Document title'})).toHaveFocus();
  await user.click(screen.getByRole('button',{name:'Pages'}));
  expect(openNotes).toHaveBeenCalledOnce();
});
it('retains a dirty standalone note when saving before breadcrumb navigation fails', async () => {
  const user=userEvent.setup();const openNotes=vi.fn();
  vi.mocked(ipc.getArtifactContext).mockResolvedValue({ ...context(), project_id: null });
  render(<FocusView {...props()} projectId={undefined} onOpenStandaloneNotes={openNotes} />);
  const editor=await screen.findByRole('textbox',{name:'Document content'});
  setEditorMarkdown(editor,'# Alpha\n\nKeep this draft');
  vi.mocked(ipc.saveArtifact).mockRejectedValue(new Error('Disk unavailable'));
  await user.click(screen.getByRole('button',{name:'Pages'}));
  expect(openNotes).not.toHaveBeenCalled();expect(editorMarkdown(editor)).toContain('Keep this draft');
  vi.mocked(ipc.saveArtifact).mockImplementation(save);
  await user.click(screen.getByRole('button',{name:'Pages'}));
  await waitFor(()=>expect(openNotes).toHaveBeenCalledOnce());
  expect(ipc.saveArtifact).toHaveBeenCalledWith('alpha',expect.stringContaining('Keep this draft'),'Alpha',null);
});

it('uses the actual project name for a note opened without project context', async () => {
  const user = userEvent.setup();
  const onOpenProject = vi.fn();
  const onOpenStandaloneNotes = vi.fn();
  render(<FocusView {...props()} projectId={undefined} onOpenProject={onOpenProject} onOpenStandaloneNotes={onOpenStandaloneNotes} />);
  const breadcrumb = await screen.findByRole('button', { name: 'Knowledge > platform' });
  expect(getWorkspaceEntity).toHaveBeenCalledWith({ kind: 'project', id: 'project-a' });
  expect(ipc.getResumeBrief).not.toHaveBeenCalled();
  expect(screen.queryByRole('button', { name: 'Pages' })).not.toBeInTheDocument();
  await user.click(breadcrumb);
  expect(onOpenProject).toHaveBeenCalledWith('project-a');
  expect(onOpenStandaloneNotes).not.toHaveBeenCalled();
});

it('keeps the selected project for a shared note and does not depend on a resume brief', async () => {
  const user = userEvent.setup();
  const onOpenProject = vi.fn();
  vi.mocked(ipc.getResumeBrief).mockRejectedValue(new Error('Brief unavailable'));
  render(<FocusView {...props()} projectId="project-b" onOpenProject={onOpenProject} />);
  const breadcrumb = await screen.findByRole('button', { name: 'Other project' });
  await screen.findByRole('textbox', { name: 'Document content' });
  await user.click(breadcrumb);
  expect(onOpenProject).toHaveBeenCalledWith('project-b');
});

it('ignores a late project-name response after switching documents', async () => {
  let finishLookup!: (detail: WorkspaceEntityDetail) => void;
  vi.mocked(getWorkspaceEntity).mockImplementationOnce(() => new Promise(resolve => { finishLookup = resolve; }));
  const handlers = { ...props(), onOpenProject: vi.fn() };
  const { rerender } = render(<FocusView {...handlers} />);
  await screen.findByRole('textbox', { name: 'Document content' });
  rerender(<FocusView {...handlers} artifactId="beta" projectId="project-b" />);
  await screen.findByRole('button', { name: 'Other project' });
  await act(async () => finishLookup({ entity: { kind: 'project', id: 'project-a', title: 'Old project', type_label: 'Project', project_id: null }, content_md: '', description: '', members: [] }));
  expect(screen.getByRole('button', { name: 'Other project' })).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Old project' })).not.toBeInTheDocument();
});
