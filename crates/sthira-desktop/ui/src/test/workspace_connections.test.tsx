vi.mock('../api/conversation', () => ({ getDocumentConversation: vi.fn().mockResolvedValue({mode:'document', initial_prompt:'', turns:[]}), undoDocumentEdit: vi.fn() }));
import { richEditor, editorMarkdown, setEditorMarkdown } from './editor-helpers';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ConnectionsPanel } from '../components/ConnectionsPanel';
import { FocusView } from '../components/FocusView';
import { RichDocumentRenderer } from '../components/RichDocumentRenderer';
import * as connections from '../api/connections';
import * as ipc from '../api/ipc';
import type { WorkspaceConnection, WorkspaceEntity, WorkspaceEntityDetail } from '../types/sthira';

vi.mock('../api/connections', async (original) => ({
  ...await original<typeof connections>(), getWorkspaceConnections: vi.fn(), getWorkspaceEntity: vi.fn(),
  searchConnectionTargets: vi.fn(), createWorkspaceConnection: vi.fn(), deleteWorkspaceConnection: vi.fn(),
}));
vi.mock('../api/ipc', async (original) => ({
  ...await original<typeof ipc>(), getArtifactContext: vi.fn(), getResumeBrief: vi.fn(), getProjectHistory: vi.fn(),
  getProviderCatalog: vi.fn(), listObjectTypes: vi.fn(), getObjectInstance: vi.fn(), saveArtifact: vi.fn(),
}));

const a: WorkspaceEntity = { kind: 'artifact', id: 'a', title: 'Working notes', type_label: 'Page', project_id: null };
const b: WorkspaceEntity = { ...a, id: 'b', title: 'A decision', type_label: 'Decision' };
const p: WorkspaceEntity = { kind: 'project', id: 'p', title: 'A project', type_label: 'Project', project_id: null };
const details = (entity: WorkspaceEntity): WorkspaceEntityDetail => ({ entity, content_md: `# ${entity.title}\n\nSaved content for ${entity.id}`, description: entity.kind === 'project' ? 'Project purpose' : '', members: entity.kind === 'project' ? [b] : [], ...(entity.id === 'b' ? { object_instance: { artifact_id: 'b', object_type_id: 'decision', properties: { status: 'Recorded', accepted: false }, updated_at: 1 } } : {}) });
let links: WorkspaceConnection[];
const panelProps = () => ({ entity: a, title: a.title, scope: 'page' as const, onScopeChange: vi.fn(), onClose: vi.fn(), onPreviewChange: vi.fn(), onInsertLink: vi.fn() });

beforeEach(() => {
  vi.clearAllMocks(); links = [{ id: 'ab', source: a, target: b, created_at: 1 }, { id: 'bp', source: b, target: p, created_at: 2 }];
  vi.mocked(connections.getWorkspaceEntity).mockImplementation(async (entity) => details([a, b, p].find((item) => connections.sameEntity(item, entity))!));
  vi.mocked(connections.getWorkspaceConnections).mockImplementation(async (entity) => links.filter((link) => connections.sameEntity(link.source, entity) || connections.sameEntity(link.target, entity)));
  vi.mocked(connections.searchConnectionTargets).mockImplementation(async (query) => [a, b, p].filter((item) => item.title.toLowerCase().includes(query.toLowerCase())));
  vi.mocked(connections.createWorkspaceConnection).mockImplementation(async (source, target) => {
    const id = 'created'; links.push({ id, source: [a,b,p].find((e) => connections.sameEntity(e, source))!, target: [a,b,p].find((e) => connections.sameEntity(e, target))!, created_at: 3 }); return id;
  });
  vi.mocked(connections.deleteWorkspaceConnection).mockImplementation(async (id) => { links = links.filter((link) => link.id !== id); });
  vi.mocked(ipc.getArtifactContext).mockResolvedValue({ artifact: { id: 'a', artifact_type: 'note', authorship: 'user', title: a.title, content_md: '# Working notes\n\nOriginal draft', content_hash: '', version: 1, is_unfinished_draft: false, created_at: 1, updated_at: 1 }, epistemic_nodes: [], pending_proposals: [], project_id: null, is_unfinished_draft: false });
  vi.mocked(ipc.getProviderCatalog).mockResolvedValue([]); vi.mocked(ipc.listObjectTypes).mockResolvedValue([]); vi.mocked(ipc.getObjectInstance).mockResolvedValue(null);
  vi.mocked(ipc.getProjectHistory).mockResolvedValue({ project_id: 'p', narrative_segments: [], milestones: [] });
  vi.mocked(ipc.getResumeBrief).mockResolvedValue({ project_id: 'p', level: 1, purpose: 'Project purpose', recent_progress: [], conflicting_decisions: [], active_working_breadcrumb: 'Project: A project', continuation_point: '', primary_continuation_artifact_id: 'a', pane_hint: { collapse: { reason: 'active_session' } } });
  vi.mocked(ipc.saveArtifact).mockImplementation(async (id, content, title) => ({ ...(await ipc.getArtifactContext(id)).artifact, content_md: content, title }));
});

describe('Workspace connections', () => {
  it('opens objects and projects in place and walks back through nested references', async () => {
    const user = userEvent.setup(); const props = panelProps(); render(<ConnectionsPanel {...props} />);
    await user.click(await screen.findByRole('button', { name: 'Preview A decision' }));
    expect(await screen.findByText('Saved content for b')).toBeInTheDocument();
    expect(screen.getByText('Recorded')).toBeInTheDocument(); expect(screen.getByText('No')).toBeInTheDocument();
    expect(within(screen.getByRole('region', { name: 'Backlinks to this item' })).getByRole('button', { name: 'Preview Working notes' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Preview A project' }));
    expect(await screen.findByText('Project purpose')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /A decision\s*Decision/ })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Back' }));
    expect(await screen.findByText('Saved content for b')).toBeInTheDocument();
    await user.click(within(screen.getByRole('complementary', { name: 'Connections' })).getByRole('button', { name: 'Connections' }));
    expect(await screen.findByRole('button', { name: 'Preview A decision' })).toBeInTheDocument();
    expect(screen.queryByLabelText('Reference content')).not.toBeInTheDocument();
    expect(props.onPreviewChange).toHaveBeenLastCalledWith(false);
    expect(screen.getByRole('button', { name: 'Link an item' })).toHaveFocus();
    await user.keyboard('{Escape}');
    expect(props.onClose).toHaveBeenCalledOnce();
  });

  it('searches real targets, omits self, marks existing links, and inserts selected references', async () => {
    const user = userEvent.setup(); const props = panelProps(); render(<ConnectionsPanel {...props} />);
    await user.click(await screen.findByRole('button', { name: 'Link an item' }));
    expect(await screen.findByRole('button', { name: 'A decision already linked' })).toBeDisabled();
    expect(screen.queryByRole('button', { name: 'Link Working notes' })).not.toBeInTheDocument();
    await user.type(screen.getByRole('textbox', { name: 'Search connection targets' }), 'project');
    await user.click(await screen.findByRole('button', { name: 'Insert link to A project' }));
    expect(connections.createWorkspaceConnection).toHaveBeenCalledWith(a, p);
    expect(props.onInsertLink).toHaveBeenCalledWith(p);
    expect(await screen.findByRole('button', { name: 'Preview A project' })).toBeInTheDocument();
  });

  it('unlinks with undo while keeping both items available', async () => {
    const user = userEvent.setup(); render(<ConnectionsPanel {...panelProps()} />);
    await user.click(await screen.findByRole('button', { name: 'Remove connection Working notes to A decision' }));
    expect(await screen.findByText('Connection removed.')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Preview A decision' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Undo' }));
    expect(await screen.findByRole('button', { name: 'Preview A decision' })).toBeInTheDocument();
    expect(connections.createWorkspaceConnection).toHaveBeenCalledWith(a, b);
  });

  it('recovers from a missing reference and ignores a late response after navigation', async () => {
    const user = userEvent.setup(); render(<ConnectionsPanel {...panelProps()} />);
    await screen.findByRole('button', { name: 'Preview A decision' });
    let resolveLate!: (value: WorkspaceEntityDetail) => void;
    vi.mocked(connections.getWorkspaceEntity).mockImplementationOnce(() => new Promise((resolve) => { resolveLate = resolve; }));
    await user.click(screen.getByRole('button', { name: 'Preview A decision' }));
    await user.click(within(screen.getByRole('complementary')).getByRole('button', { name: 'Connections' }));
    await screen.findByRole('button', { name: 'Preview A decision' });
    resolveLate(details(b));
    await waitFor(() => expect(screen.queryByText('Saved content for b')).not.toBeInTheDocument());
    vi.mocked(connections.getWorkspaceEntity).mockRejectedValueOnce(new Error('Missing'));
    await user.click(screen.getByRole('button', { name: 'Preview A decision' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Couldn’t load this reference');
    await user.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByText('Saved content for b')).toBeInTheDocument();
  });

  it('keeps the original editor and unsaved draft mounted while references are browsed', async () => {
    const user = userEvent.setup(); render(<FocusView artifactId="a" onBack={vi.fn()} onOpenCanvas={vi.fn()} />);
    const editor = await screen.findByRole('textbox', { name: 'Document content' });
    setEditorMarkdown(editor, '# Working notes\n\nUnsaved text stays here');
    await user.click(screen.getByRole('button', { name: 'Connections' }));
    await user.click(await screen.findByRole('button', { name: 'Preview A decision' }));
    await screen.findByText('Saved content for b');
    expect(screen.getByRole('textbox', { name: 'Document content' })).toBe(editor);
    expect(editorMarkdown(editor)).toBe('Unsaved text stays here');
    await user.click(screen.getByRole('button', { name: 'Close connections' }));
    expect(screen.getByRole('button', { name: 'Connections' })).toHaveFocus();
    expect(editorMarkdown(editor)).toBe('Unsaved text stays here');
  });

  it('previews a legacy generated citation without saving the source or creating a connection', async () => {
    const original = (await ipc.getArtifactContext('a')).artifact;
    vi.mocked(ipc.getArtifactContext).mockResolvedValue({artifact:{...original,content_md:'# Working notes\n\n## References\n\n1. A decision – `sthira://artifact/b`'},epistemic_nodes:[],pending_proposals:[],project_id:null,is_unfinished_draft:false});
    const user=userEvent.setup();render(<FocusView artifactId="a" onBack={vi.fn()} onOpenCanvas={vi.fn()}/>);
    await user.click(await screen.findByRole('link',{name:'A decision'}));
    expect(await screen.findByText('Saved content for b')).toBeInTheDocument();
    expect(ipc.saveArtifact).not.toHaveBeenCalled();expect(connections.createWorkspaceConnection).not.toHaveBeenCalled();
  });

  it('opens stable inline entity links locally while leaving ordinary web links intact', async () => {
    const user = userEvent.setup(); const open = vi.fn();
    render(<RichDocumentRenderer content="[A decision](#/sthira/artifact/b) and [External](https://example.com)" onOpenEntity={open} />);
    await user.click(screen.getByRole('button', { name: 'A decision' }));
    expect(open).toHaveBeenCalledWith({ kind: 'artifact', id: 'b' });
    expect(screen.getByRole('link', { name: 'External' })).toHaveAttribute('href', 'https://example.com');
    expect(connections.parseEntityHref('#/sthira/project/hello%20world')).toEqual({ kind: 'project', id: 'hello world' });
    expect(connections.parseEntityHref('#/sthira/artifact/%invalid')).toBeNull();
  });

  it('inserts a selected item at the editor caret and leaves content intact on connection failure', async () => {
    const user = userEvent.setup(); render(<FocusView artifactId="a" onBack={vi.fn()} onOpenCanvas={vi.fn()} />);
    const editor = await screen.findByRole('textbox', { name: 'Document content' }) as HTMLElement;
    richEditor(editor).commands.setTextSelection(richEditor(editor).state.doc.content.size - 1);
    await user.click(screen.getByRole('button', { name: 'Connections' }));
    await user.click(await screen.findByRole('button', { name: 'Link an item' }));
    vi.mocked(connections.createWorkspaceConnection).mockRejectedValueOnce(new Error('Unavailable'));
    await user.click(await screen.findByRole('button', { name: 'Insert link to A project' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Couldn’t create the connection');
    expect(editorMarkdown(editor)).toBe('Original draft');
    await user.click(screen.getByRole('button', { name: 'Insert link to A project' }));
    expect(editorMarkdown(editor)).toBe('Original draft[A project](#/sthira/project/p)');
  });

  it('lets a project create its own references independently of the open page', async () => {
    const user = userEvent.setup(); render(<FocusView artifactId="a" projectId="p" onBack={vi.fn()} onOpenCanvas={vi.fn()} />);
    await screen.findByRole('textbox', { name: 'Document content' });
    await user.click(screen.getByRole('button', { name: 'Connections' }));
    await screen.findByRole('button', { name: 'Preview A decision' });
    await user.click(within(screen.getByRole('group', { name: 'Connection scope' })).getByRole('button', { name: 'Project' }));
    await screen.findByRole('heading', { name: 'A project' });
    await user.click(await screen.findByRole('button', { name: 'Link an item' }));
    await user.click(await screen.findByRole('button', { name: 'Link Working notes' }));
    expect(connections.createWorkspaceConnection).toHaveBeenCalledWith({ kind: 'project', id: 'p' }, a);
    expect(await screen.findByRole('button', { name: 'Preview Working notes' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Insert link to Working notes' })).not.toBeInTheDocument();
  });

  it('syncs ConnectionsPanel heading immediately when the parent title changes (F-B5-009)', async () => {
    const initial = panelProps();
    const { rerender } = render(<ConnectionsPanel {...initial} />);
    await screen.findByRole('button', { name: 'Preview A decision' });
    expect(screen.getByRole('heading', { level: 2, name: 'Working notes' })).toBeInTheDocument();

    rerender(<ConnectionsPanel {...initial} title="Renamed Architecture Spec" />);
    expect(screen.getByRole('heading', { level: 2, name: 'Renamed Architecture Spec' })).toBeInTheDocument();
  });

  it('exposes isolated role=alert text without retry buttons in ConnectionsPanel errors (F-B5-014)', async () => {
    vi.mocked(connections.getWorkspaceConnections).mockRejectedValueOnce(new Error('vault offline'));
    render(<ConnectionsPanel {...panelProps()} />);
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('Couldn’t load this reference. It may have been removed, or the vault may be unavailable.');
    expect(alert.querySelector('button')).toBeNull();
    expect(within(screen.getByRole('complementary', { name: 'Connections' })).getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });
});
