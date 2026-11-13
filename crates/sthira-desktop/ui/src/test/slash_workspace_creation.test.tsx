import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { FocusView } from '../components/FocusView';
import { richEditor } from './editor-helpers';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

const initial = {
  id: 'source', artifact_type: 'note', authorship: 'user', title: 'Source',
  content_md: '# Source\n\nKeep this draft', content_hash: 'hash', version: 1,
  is_unfinished_draft: false, created_at: 1000, updated_at: 1000,
};
let saved = { ...initial };
let projectId: string | null;
beforeEach(() => {
  vi.clearAllMocks(); saved = { ...initial }; projectId = 'project';
  invoke.mockImplementation(async (command: string, args: Record<string, any>) => {
    switch (command) {
      case 'cmd_get_artifact_context': return { artifact: saved, project_id: projectId, project_ids: projectId ? [projectId] : [], epistemic_nodes: [], pending_proposals: [] };
      case 'cmd_get_provider_catalog': case 'cmd_list_object_types': case 'cmd_get_type_templates': return [];
      case 'cmd_get_object_instance': case 'cmd_get_document_conversation': return null;
      case 'cmd_get_workspace_connections': return [];
      case 'cmd_get_workspace_entity': return { entity: { id: 'project', kind: 'project', title: 'Project' } };
      case 'cmd_save_artifact': saved = { ...saved, content_md: args.contentMd, title: args.title }; return saved;
      case 'cmd_create_page': return { ...initial, id: 'new-page', title: args.title };
      case 'cmd_create_project': return { id: 'new-project', name: args.name, pages: [], canvases: [] };
      default: return null;
    }
  });
});

async function openCreation(command: string, props: Partial<React.ComponentProps<typeof FocusView>> = {}) {
  const onOpenArtifact = vi.fn(); const onOpenProject = vi.fn();
  render(<FocusView artifactId="source" onBack={vi.fn()} onOpenCanvas={vi.fn()} onOpenArtifact={onOpenArtifact} onOpenProject={onOpenProject} {...props} />);
  const element = await screen.findByRole('textbox', { name: 'Document content' });
  const editor = richEditor(element);
  act(() => { editor.commands.focus('end'); editor.commands.insertContent(` /new ${command}`); });
  fireEvent.keyDown(element, { key: 'Enter' });
  await screen.findByRole('dialog');
  return { onOpenArtifact, onOpenProject, editor };
}

describe('Workspace creation from slash commands (mocked IPC)', () => {
  it.each([['project', 'project'], [null, null]])('saves the source before opening a new page with membership %s', async (membership, expected) => {
    projectId = membership;
    const user = userEvent.setup();
    const { onOpenArtifact } = await openCreation('page');
    await user.type(screen.getByLabelText('Title'), 'Context notes');
    await user.click(screen.getByRole('button', { name: 'Create page' }));
    await waitFor(() => expect(onOpenArtifact).toHaveBeenCalledWith('new-page', expected));
    expect(saved.content_md).toContain('Keep this draft');
    expect(saved.content_md).not.toContain('/new page');
    const calls = invoke.mock.calls.map(call => call[0]);
    expect(calls.indexOf('cmd_save_artifact')).toBeLessThan(calls.indexOf('cmd_create_page'));
    expect(invoke).toHaveBeenCalledWith('cmd_create_page', { title: 'Context notes', projectId: expected });
  });

  it('creates a named project with a purpose and opens its overview', async () => {
    const user = userEvent.setup(); const { onOpenProject } = await openCreation('project');
    await user.type(screen.getByLabelText('Project name'), 'Alpha Planner');
    await user.type(screen.getByLabelText('Purpose (optional)'), 'Shared context');
    await user.click(screen.getByRole('button', { name: 'Create project' }));
    await waitFor(() => expect(onOpenProject).toHaveBeenCalledWith('new-project'));
    expect(invoke).toHaveBeenCalledWith('cmd_create_project', { name: 'Alpha Planner', purpose: 'Shared context' });
  });

  it('retains the current draft and creates nothing when its prerequisite save fails', async () => {
    const user = userEvent.setup(); const { onOpenArtifact, editor } = await openCreation('page');
    const original = invoke.getMockImplementation()!;
    invoke.mockImplementation((command, args) => command === 'cmd_save_artifact' ? Promise.reject(new Error('disk unavailable')) : original(command, args));
    await user.type(screen.getByLabelText('Title'), 'Unsaved target');
    await user.click(screen.getByRole('button', { name: 'Create page' }));
    expect(await screen.findByText('Save the current page before creating a new item. Your draft is still open.')).toBeInTheDocument();
    expect(editor.getText()).toContain('Keep this draft');
    expect(invoke).not.toHaveBeenCalledWith('cmd_create_page', expect.anything());
    expect(onOpenArtifact).not.toHaveBeenCalled();
    expect(screen.getByLabelText('Title')).toHaveValue('Unsaved target');
  });

  it('cancels with Escape without creating anything or consuming the command', async () => {
    const user = userEvent.setup(); const { editor } = await openCreation('project');
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(editor.getText()).toContain('/new project');
    expect(invoke).not.toHaveBeenCalledWith('cmd_create_project', expect.anything());
  });

  it('continues from a new custom type into its object creator and inserts the instance', async () => {
    const user = userEvent.setup();
    await openCreation('custom object');
    expect(screen.getByRole('dialog', { name: 'New Object Type' })).toBeInTheDocument();
    await user.type(screen.getByTestId('input-object-type-name'), 'Experiment');
    await user.click(screen.getByTestId('btn-submit-object-type'));
    expect(await screen.findByTestId('quick-object-creator-card')).toBeInTheDocument();
    const original = invoke.getMockImplementation()!;
    invoke.mockImplementation((command, args) => command === 'cmd_instantiate_object_from_template' ? Promise.resolve('experiment-instance') : original(command, args));
    await user.type(screen.getByTestId('input-object-title'), 'Memory experiment');
    await user.click(screen.getByTestId('btn-create-insert-object'));
    await waitFor(() => expect(saved.content_md).toContain('experiment-instance'));
    expect(saved.content_md).not.toContain('/new custom object');
    expect(invoke).toHaveBeenCalledWith('cmd_create_object_type', expect.objectContaining({ typeDef: expect.objectContaining({ display_name: 'Experiment' }) }));
    expect(invoke).toHaveBeenCalledWith('cmd_instantiate_object_from_template', expect.objectContaining({ typeId: 'experiment', projectId: 'project', title: 'Memory experiment' }));
  });

  it('preserves the naming form after a create failure and allows a retry', async () => {
    const user = userEvent.setup(); const { onOpenArtifact } = await openCreation('page');
    const original = invoke.getMockImplementation()!;
    let attempts = 0;
    invoke.mockImplementation((command, args) => {
      if (command === 'cmd_create_page' && ++attempts === 1) return Promise.reject('write failed');
      return original(command, args);
    });
    await user.type(screen.getByLabelText('Title'), 'Retry target');
    await user.click(screen.getByRole('button', { name: 'Create page' }));
    expect(await screen.findByText('Couldn’t create this page. Try again.')).toBeInTheDocument();
    expect(screen.getByLabelText('Title')).toHaveValue('Retry target');
    await user.click(screen.getByRole('button', { name: 'Create page' }));
    await waitFor(() => expect(onOpenArtifact).toHaveBeenCalledOnce());
  });
});
