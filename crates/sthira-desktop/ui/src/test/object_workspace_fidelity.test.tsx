import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ObjectCollectionContainer } from '../components/objects/ObjectCollectionContainer';
import { ObjectKanbanView } from '../components/objects/ObjectKanbanView';
import { PropertyFieldRenderer } from '../components/objects/PropertyFieldRenderer';
import * as ipc from '../api/ipc';
import type { ObjectItem, ObjectTemplate, ObjectType } from '../types/sthira';
vi.mock('../api/ipc', () => ({ listObjectsByType: vi.fn(), saveObjectInstance: vi.fn(), createTypedObject: vi.fn(), getWorkspaceTree: vi.fn(), getTypeTemplates: vi.fn(), instantiateObjectFromTemplate: vi.fn(), getArtifactContext: vi.fn(), getObjectInstance: vi.fn() }));
const type: ObjectType = { id: 'review', display_name: 'Review', icon: '🧪', color: '#123456', created_at: 1000, is_system: false, default_template_md: '## Evidence', schema: [
  { key: 'status', label: 'Status', property_type: 'select', options: ['queued', 'done'], default_value: 'queued' },
  { key: 'summary', label: 'Summary', property_type: 'text' }, { key: 'reviewed', label: 'Reviewed', property_type: 'boolean' },
] };
const record: ObjectItem = { artifact_id: 'review-1', title: 'Saved review', object_type_id: 'review', properties: { status: 'queued', summary: 'Recorded evidence', reviewed: false }, content_md: '# Saved review\n\n## Evidence\n\nActual recorded text.', project_id: 'project-1', updated_at: 1000 };
const template: ObjectTemplate = { id: 'preset-1', object_type_id: 'review', name: 'Review preset', icon: '🧪', is_default: false, properties_preset: { status: 'done', summary: 'Preset summary' }, body_template_md: '## Evidence', created_at: 1000 };
const props = () => ({ objectType: type, onBack: vi.fn(), onOpenArtifact: vi.fn(), onWorkspaceMutated: vi.fn() });
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(ipc.listObjectsByType).mockResolvedValue([record]); vi.mocked(ipc.saveObjectInstance).mockResolvedValue();
  vi.mocked(ipc.getWorkspaceTree).mockResolvedValue({ projects: [{ id: 'project-1', name: 'Real project', pages: [], canvases: [], purpose: '', updated_at: 1000 }], unassociated_pages: [] });
  vi.mocked(ipc.getTypeTemplates).mockResolvedValue([template]);
  vi.mocked(ipc.createTypedObject).mockImplementation(async (id, title, properties, projectId) => ({ ...record, artifact_id: 'created-1', object_type_id: id, title, properties, project_id: projectId }));
  vi.mocked(ipc.instantiateObjectFromTemplate).mockResolvedValue('templated-1');
  vi.mocked(ipc.getArtifactContext).mockResolvedValue({ artifact: { id: 'templated-1', title: 'Templated review', content_md: '# Templated review\n\n## Evidence\n\nTemplate body retained.', authorship: 'user', artifact_type: 'note', content_hash: 'hash', version: 1, is_unfinished_draft: false, created_at: 1000, updated_at: 1000 }, epistemic_nodes: [], pending_proposals: [], project_id: 'project-1', is_unfinished_draft: false });
  vi.mocked(ipc.getObjectInstance).mockResolvedValue(null);
});
describe('Object collection behavior', () => {
  it('opens actual records and searches their properties', async () => {
    const handlers = props(), user = userEvent.setup(); render(<ObjectCollectionContainer {...handlers} />);
    await user.click(await screen.findByRole('button', { name: 'Saved review' }));
    expect(handlers.onOpenArtifact).toHaveBeenCalledWith('review-1', 'project-1');
    const search = screen.getByRole('searchbox', { name: 'Search Review collection' });
    await user.type(search, 'nonexistent'); expect(screen.queryByRole('button', { name: 'Saved review' })).not.toBeInTheDocument();
    await user.clear(search); await user.type(search, 'Recorded evidence'); expect(screen.getByRole('button', { name: 'Saved review' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Name' })); expect(screen.getByRole('columnheader', { name: 'Name' })).toHaveAttribute('aria-sort', 'ascending');
  });
  it('retries failed loading without claiming the collection is empty', async () => {
    vi.mocked(ipc.listObjectsByType).mockRejectedValueOnce(new Error('Offline'));
    const user = userEvent.setup(); render(<ObjectCollectionContainer {...props()} />);
    expect(await screen.findByRole('alert')).toHaveTextContent('Couldn’t load this collection');
    await user.click(screen.getByRole('button', { name: 'Try again' })); expect(await screen.findByRole('button', { name: 'Saved review' })).toBeInTheDocument();
  });
  it('serializes rapid full-property saves without losing another cell edit', async () => {
    let finish!: () => void; vi.mocked(ipc.saveObjectInstance).mockImplementationOnce(() => new Promise<void>(resolve => { finish = resolve; }));
    const user = userEvent.setup(); render(<ObjectCollectionContainer {...props()} />); await screen.findByRole('button', { name: 'Saved review' });
    fireEvent.change(screen.getByRole('combobox', { name: 'Status' }), { target: { value: 'done' } });
    await waitFor(() => expect(ipc.saveObjectInstance).toHaveBeenCalledTimes(1)); await user.click(screen.getByRole('checkbox', { name: 'Reviewed' })); expect(ipc.saveObjectInstance).toHaveBeenCalledTimes(1);
    await act(async () => finish()); await waitFor(() => expect(ipc.saveObjectInstance).toHaveBeenCalledTimes(2));
    expect(ipc.saveObjectInstance).toHaveBeenLastCalledWith(expect.objectContaining({ artifact_id: 'review-1', object_type_id: 'review', properties: { status: 'done', summary: 'Recorded evidence', reviewed: true } }));
  });
  it('holds edited values after save failure and retries them', async () => {
    vi.mocked(ipc.saveObjectInstance).mockRejectedValueOnce(new Error('Disk unavailable'));
    const user = userEvent.setup(); render(<ObjectCollectionContainer {...props()} />); await screen.findByRole('button', { name: 'Saved review' });
    await user.selectOptions(screen.getByRole('combobox', { name: 'Status' }), 'done'); expect(await screen.findByRole('alert')).toHaveTextContent('edited values are still held here');
    expect(screen.getByRole('combobox', { name: 'Status' })).toHaveValue('done'); await user.click(screen.getByRole('button', { name: 'Retry save' }));
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument()); expect(ipc.saveObjectInstance).toHaveBeenLastCalledWith(expect.objectContaining({ properties: expect.objectContaining({ status: 'done' }) }));
  });
  it('creates with the actual type, edited properties and selected project', async () => {
    const user = userEvent.setup(); render(<ObjectCollectionContainer {...props()} />); await screen.findByRole('button', { name: 'Saved review' });
    await user.click(screen.getByRole('button', { name: 'New Review' })); const dialog = within(screen.getByRole('dialog'));
    await user.type(dialog.getByRole('textbox', { name: 'Title' }), 'New local review'); await user.type(dialog.getByRole('textbox', { name: 'Summary' }), 'A factual description');
    await user.selectOptions(dialog.getByRole('combobox', { name: 'Associate with Project (Optional)' }), 'project-1'); await user.click(dialog.getByRole('button', { name: 'Create Review' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(ipc.createTypedObject).toHaveBeenCalledWith('review', 'New local review', { status: 'queued', summary: 'A factual description', reviewed: false }, 'project-1');
    expect(ipc.instantiateObjectFromTemplate).not.toHaveBeenCalled();
  });
  it('retains the created ID when retrying a failed template property save', async () => {
    vi.mocked(ipc.saveObjectInstance).mockRejectedValueOnce(new Error('Write failed'));
    const user = userEvent.setup(); render(<ObjectCollectionContainer {...props()} />); await screen.findByRole('button', { name: 'Saved review' });
    await user.click(screen.getByRole('button', { name: 'New Review' })); const dialog = within(screen.getByRole('dialog'));
    await user.type(dialog.getByRole('textbox', { name: 'Title' }), 'Templated review'); await user.selectOptions(await dialog.findByRole('combobox', { name: 'Template' }), 'preset-1');
    await user.click(dialog.getByRole('button', { name: 'Create Review' })); expect(await dialog.findByRole('alert')).toHaveTextContent('document was created');
    await user.click(dialog.getByRole('button', { name: 'Retry creation' })); await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(ipc.instantiateObjectFromTemplate).toHaveBeenCalledTimes(1); expect(ipc.instantiateObjectFromTemplate).toHaveBeenCalledWith('review', 'preset-1', 'Templated review', null);
    expect(ipc.saveObjectInstance).toHaveBeenLastCalledWith(expect.objectContaining({ artifact_id: 'templated-1', properties: expect.objectContaining({ status: 'done', summary: 'Preset summary' }) }));
    await user.click(screen.getByRole('button', { name: 'Canvas' })); expect(screen.getByText(/Template body retained/)).toBeInTheDocument();
  });
  it('preserves edited properties when a template supplies defaults for untouched fields', async () => {
    const user = userEvent.setup(); render(<ObjectCollectionContainer {...props()} />);
    await screen.findByRole('button', { name: 'Saved review' });
    await user.click(screen.getByRole('button', { name: 'New Review' })); const dialog = within(screen.getByRole('dialog'));
    await user.type(dialog.getByRole('textbox', { name: 'Title' }), 'Edited before template');
    await user.type(dialog.getByRole('textbox', { name: 'Summary' }), 'Keep my description');
    await user.click(dialog.getByRole('checkbox', { name: 'Reviewed' }));
    await user.selectOptions(await dialog.findByRole('combobox', { name: 'Template' }), 'preset-1');
    expect(dialog.getByRole('textbox', { name: 'Summary' })).toHaveValue('Keep my description');
    expect(dialog.getByRole('checkbox', { name: 'Reviewed' })).toBeChecked();
    expect(dialog.getByRole('combobox', { name: 'Status' })).toHaveValue('done');
    await user.click(dialog.getByRole('button', { name: 'Create Review' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(ipc.saveObjectInstance).toHaveBeenLastCalledWith(expect.objectContaining({ properties: { status: 'done', summary: 'Keep my description', reviewed: true } }));
  });
  it('restores focus after Escape and inherits board creation defaults', async () => {
    const user = userEvent.setup(); render(<ObjectCollectionContainer {...props()} />); await screen.findByRole('button', { name: 'Saved review' });
    await user.click(screen.getByRole('button', { name: 'Kanban' })); const trigger = screen.getByRole('button', { name: 'Add Review in done' }); await user.click(trigger);
    expect(within(screen.getByRole('dialog')).getByRole('combobox', { name: 'Status' })).toHaveValue('done'); await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument(); expect(trigger).toHaveFocus();
  });
  it('moves a board record with either the select or drag and drop', async () => {
    const update = vi.fn(), user = userEvent.setup(); render(<ObjectKanbanView objectType={type} items={[record]} onUpdateProperty={update} onOpenItem={vi.fn()} />);
    await user.selectOptions(screen.getByRole('combobox', { name: 'Move Saved review by Status' }), 'value:done'); expect(update).toHaveBeenLastCalledWith('review-1', 'status', 'done');
    fireEvent.drop(screen.getByRole('region', { name: 'done column' }), { dataTransfer: { getData: () => 'review-1' } }); expect(update).toHaveBeenLastCalledWith('review-1', 'status', 'done');
  });
  it('distinguishes an option named unassigned from missing values', () => {
    const custom = { ...type, schema: [{ key: 'status', label: 'Status', property_type: 'select' as const, options: ['unassigned'] }] };
    render(<ObjectKanbanView objectType={custom} items={[{ ...record, properties: { status: 'unassigned' } }]} onUpdateProperty={vi.fn()} onOpenItem={vi.fn()} />);
    expect(within(screen.getByRole('region', { name: 'unassigned column' })).getByRole('button', { name: 'Saved review' })).toBeInTheDocument();
    expect(within(screen.getByRole('region', { name: 'Unassigned column' })).queryByRole('button', { name: 'Saved review' })).not.toBeInTheDocument();
  });
});
describe('Property editing', () => {
  it('commits Enter once and cancels Escape without saving the discarded draft', async () => {
    const user = userEvent.setup(), change = vi.fn(); render(<PropertyFieldRenderer definition={{ key: 'summary', label: 'Summary', property_type: 'text' }} value="Old value" onChange={change} compact />);
    await user.click(screen.getByRole('button', { name: 'Edit Summary' })); const input = screen.getByRole('textbox', { name: 'Summary' }); await user.clear(input); await user.type(input, 'New value{Enter}');
    expect(change).toHaveBeenCalledTimes(1); expect(change).toHaveBeenCalledWith('New value'); change.mockClear();
    await user.click(screen.getByRole('button', { name: 'Edit Summary' })); await user.type(screen.getByRole('textbox', { name: 'Summary' }), ' discarded{Escape}'); expect(change).not.toHaveBeenCalled();
  });
  it('edits multiselect arrays through labeled checkboxes', async () => {
    const user = userEvent.setup(), change = vi.fn(); render(<PropertyFieldRenderer definition={{ key: 'tags', label: 'Tags', property_type: 'multiselect', options: ['Research', 'Review'] }} value={['Research']} onChange={change} />);
    await user.click(screen.getByText('Research', { selector: 'summary' })); await user.click(screen.getByRole('checkbox', { name: 'Review' })); expect(change).toHaveBeenCalledWith(['Research', 'Review']);
  });
});
