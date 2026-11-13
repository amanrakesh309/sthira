import { useCallback, useEffect, useRef, useState } from 'react';
import { Table as TableIcon, Columns, LayoutGrid, Plus, ArrowLeft, X, ExternalLink } from 'lucide-react';
import { listObjectsByType, saveObjectInstance, createTypedObject, getWorkspaceTree, getTypeTemplates,
  instantiateObjectFromTemplate, getArtifactContext, getObjectInstance } from '../../api/ipc';
import { ObjectTableView } from './ObjectTableView';
import { ObjectKanbanView } from './ObjectKanbanView';
import { PropertyFieldRenderer } from './PropertyFieldRenderer';
import { ObjectIdentity } from './ObjectIdentity';
import { useDialogFocus } from '../../hooks/useDialogFocus';
import { directoryDate, directoryExcerpt } from '../../utils/directory';
import type { ObjectType, ObjectItem, ObjectTemplate } from '../../types/sthira';

interface ObjectCollectionContainerProps {
  objectType: ObjectType;
  onBack: () => void;
  onOpenArtifact: (artifactId: string, projectId?: string) => void;
  onOpenCanvas?: (canvasId?: string) => void;
  onWorkspaceMutated?: () => void;
}

export function ObjectCollectionContainer({ objectType, onBack, onOpenArtifact, onWorkspaceMutated }: ObjectCollectionContainerProps) {
  const [viewMode, setViewMode] = useState<'table' | 'kanban' | 'canvas'>('table');
  const [items, setItems] = useState<ObjectItem[]>([]);
  const itemsRef = useRef<ObjectItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saveState, setSaveState] = useState('');
  const request = useRef(0);
  const metadataRequest = useRef(0);
  const saveQueue = useRef<Promise<unknown>>(Promise.resolve());
  const failedSave = useRef<(() => void) | null>(null);
  const setRecords = (records: ObjectItem[]) => { itemsRef.current = records; setItems(records); };
  const [createOpen, setCreateOpen] = useState(false);
  const [title, setTitle] = useState('');
  const [properties, setProperties] = useState<Record<string, any>>({});
  const [projectId, setProjectId] = useState('');
  const [projects, setProjects] = useState<{ id: string; name: string }[]>([]);
  const [templates, setTemplates] = useState<ObjectTemplate[]>([]);
  const [templateId, setTemplateId] = useState('');
  const [creating, setCreating] = useState(false);
  const creatingRef = useRef(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [metadataError, setMetadataError] = useState<string | null>(null);
  const initialProperties = useRef<Record<string, any>>({});
  const editedProperties = useRef<Record<string, any>>({});
  const createdDraft = useRef<ObjectItem | null>(null);
  const dialog = useRef<HTMLDivElement>(null);
  useDialogFocus(createOpen, dialog, () => { if (!creatingRef.current) setCreateOpen(false); });
  const refreshWorkspace = () => { window.dispatchEvent(new CustomEvent('sthira:refresh_objects')); onWorkspaceMutated?.(); };

  const loadItems = useCallback(async () => {
    const id = ++request.current;
    setLoading(true); setLoadError(null);
    try {
      const records = await listObjectsByType(objectType.id);
      if (id === request.current) { itemsRef.current = records; setItems(records); }
    } catch { if (id === request.current) setLoadError('Couldn’t load this collection. Check your vault and try again.'); }
    finally { if (id === request.current) setLoading(false); }
  }, [objectType.id]);
  useEffect(() => {
    itemsRef.current = []; setItems([]); setSaveError(null); setSaveState(''); setCreateOpen(false); failedSave.current = null;
    setTemplates([]); metadataRequest.current += 1;
    void loadItems(); return () => { request.current += 1; };
  }, [loadItems]);

  const handleUpdateProperty = (artifactId: string, key: string, value: any) => {
    const record = itemsRef.current.find(item => item.artifact_id === artifactId);
    if (!record) return;
    const next = { ...record, properties: { ...record.properties, [key]: value }, updated_at: Date.now() };
    setRecords(itemsRef.current.map(item => item.artifact_id === artifactId ? next : item));
    const generation = request.current;
    const persist = async () => {
      if (generation === request.current) { setSaveState('Saving…'); setSaveError(null); }
      try {
        await saveObjectInstance({ artifact_id: artifactId, object_type_id: objectType.id, properties: next.properties, updated_at: next.updated_at });
        if (generation === request.current) { setSaveState('Saved'); failedSave.current = null; }
        refreshWorkspace();
      } catch {
        if (generation === request.current) {
          setSaveState('Save failed'); setSaveError('Couldn’t save property changes. Your edited values are still held here. Retry to save them.');
          failedSave.current = () => {
            const latest = itemsRef.current.find(item => item.artifact_id === artifactId);
            if (latest) handleUpdateProperty(artifactId, key, latest.properties[key]);
          };
        }
      }
    };
    // Full-property JSON writes must keep order, including rapid edits in different cells.
    saveQueue.current = saveQueue.current.then(persist, persist);
  };

  const defaults = (preset: Record<string, any> = {}) => Object.fromEntries(objectType.schema.map(property => [property.key,
    preset[property.key] ?? property.default_value ?? (property.property_type === 'boolean' ? false : property.property_type === 'multiselect' ? [] : property.property_type === 'select' ? property.options?.[0] || '' : '')]));
  const loadCreateMetadata = async () => {
    const id = ++metadataRequest.current;
    setMetadataError(null);
    const [tree, presets] = await Promise.allSettled([getWorkspaceTree(), getTypeTemplates(objectType.id)]);
    if (id !== metadataRequest.current) return;
    if (tree.status === 'fulfilled') setProjects(tree.value.projects.map(project => ({ id: project.id, name: project.name })));
    if (presets.status === 'fulfilled') setTemplates(presets.value);
    if (tree.status === 'rejected' || presets.status === 'rejected') setMetadataError('Some projects or templates could not be loaded. Retry to use them.');
  };
  const openCreate = (values: Record<string, any> = {}) => {
    initialProperties.current = values; editedProperties.current = {}; createdDraft.current = null;
    setTitle(''); setProperties(defaults(values)); setProjectId(''); setTemplateId(''); setCreateError(null); setCreateOpen(true);
    void loadCreateMetadata();
  };
  const create = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!title.trim() || creatingRef.current) return;
    creatingRef.current = true; setCreating(true); setCreateError(null);
    try {
      let record = createdDraft.current;
      if (!record) {
        if (!templateId) record = await createTypedObject(objectType.id, title.trim(), properties, projectId || null);
        else {
          const id = await instantiateObjectFromTemplate(objectType.id, templateId === '__default__' ? null : templateId, title.trim(), projectId || null);
          // Retain identity before subsequent reads/saves so Retry cannot create a duplicate.
          record = { artifact_id: id, title: title.trim(), content_md: '', object_type_id: objectType.id, properties, project_id: projectId || null, updated_at: Date.now() };
          createdDraft.current = record;
        }
      }
      if (templateId) {
        await saveObjectInstance({ artifact_id: record.artifact_id, object_type_id: objectType.id, properties, updated_at: Date.now() });
        const [context, instance] = await Promise.all([getArtifactContext(record.artifact_id), getObjectInstance(record.artifact_id)]);
        record = { ...record, content_md: context.artifact.content_md, properties: instance?.properties || properties };
      }
      setRecords([record, ...itemsRef.current.filter(item => item.artifact_id !== record!.artifact_id)]);
      setCreateOpen(false); createdDraft.current = null; refreshWorkspace();
    } catch {
      setCreateError(createdDraft.current ? 'The document was created, but its properties could not finish saving. Retry to finish this same item.' : 'Couldn’t finish creating this item. Check your vault and try again.');
      if (createdDraft.current) refreshWorkspace();
    }
    finally { creatingRef.current = false; setCreating(false); }
  };

  return <section className="object-collection" data-testid="object-collection" aria-labelledby="object-collection-title">
    <header className="object-collection-header"><div className="object-collection-heading">
      <button type="button" className="directory-icon-button" title="Back to Home" aria-label="Back to Home" onClick={onBack}><ArrowLeft size={18} /></button>
      <div><p className="all-files-eyebrow">Custom objects / Collection</p><h1 id="object-collection-title">{objectType.icon} {objectType.display_name}<span>{items.length}</span></h1>
        <p className="object-collection-description">{objectType.description || (objectType.is_system ? 'System Entity Collection' : 'Custom Entity Collection')}</p></div>
    </div><div className="object-collection-actions"><div className="object-view-modes" role="group" aria-label="Collection view">
      {([{ id: 'table', label: 'Table', icon: TableIcon }, { id: 'kanban', label: 'Kanban', icon: Columns }, { id: 'canvas', label: 'Canvas', icon: LayoutGrid }] as const).map(mode => <button key={mode.id} type="button" aria-pressed={viewMode === mode.id} onClick={() => setViewMode(mode.id)}><mode.icon size={16} />{mode.label}</button>)}
    </div><button type="button" className="sthira-button-primary" onClick={() => openCreate()}><Plus size={16} />New {objectType.display_name}</button></div></header>
    <div className="object-collection-body"><p className="object-save-state" role="status">{saveState}</p>
      {saveError && <div className="directory-notice"><p role="alert">{saveError}</p><button type="button" onClick={() => failedSave.current?.()}>Retry save</button></div>}
      {loading ? <div className="directory-empty"><p role="status">Loading collection…</p></div> : loadError ? <div className="directory-empty"><p role="alert">{loadError}</p><button className="sthira-button-secondary" type="button" onClick={() => { void loadItems(); }}>Try again</button></div>
        : viewMode === 'table' ? <ObjectTableView objectType={objectType} items={items} onUpdateProperty={handleUpdateProperty} onOpenItem={(id, pid) => onOpenArtifact(id, pid || undefined)} />
        : viewMode === 'kanban' ? <ObjectKanbanView objectType={objectType} items={items} onUpdateProperty={handleUpdateProperty} onOpenItem={(id, pid) => onOpenArtifact(id, pid || undefined)} onQuickCreate={openCreate} />
        : <div className="object-card-grid">{items.length === 0 ? <div className="directory-empty"><h2>No items yet</h2><p>Create a {objectType.display_name.toLowerCase()} to start this collection.</p></div>
          : items.map(item => <article className="object-visual-card" key={item.artifact_id}>
            <header><ObjectIdentity name={objectType.display_name} icon={objectType.icon} color={objectType.color} /><time>{directoryDate(item.updated_at)}</time></header>
            <h2><button type="button" onClick={() => onOpenArtifact(item.artifact_id, item.project_id || undefined)}>{item.title || 'Untitled'}</button></h2>
            <p>{directoryExcerpt(item.content_md, item.title)}</p><dl>{objectType.schema.slice(0, 3).map(property => <div key={property.key}><dt>{property.label}</dt><dd><PropertyFieldRenderer definition={property} value={item.properties[property.key]} isReadOnly onChange={() => {}} /></dd></div>)}</dl>
            <button type="button" className="object-card-open" onClick={() => onOpenArtifact(item.artifact_id, item.project_id || undefined)}><ExternalLink size={16} />Open document</button>
          </article>)}</div>}
    </div>
    {createOpen && <div className="object-create-backdrop"><div ref={dialog} className="object-create-dialog" role="dialog" aria-modal="true" aria-labelledby="object-create-title">
      <header><h2 id="object-create-title">New {objectType.display_name}</h2><button type="button" className="directory-icon-button" aria-label="Close new object" disabled={creating} onClick={() => setCreateOpen(false)}><X size={18} /></button></header>
      <form onSubmit={create}><fieldset disabled={creating}>
        <label className="object-create-field">Title<input type="text" aria-label="Title" required autoFocus disabled={!!createdDraft.current} value={title} onChange={event => setTitle(event.target.value)} placeholder={`Enter ${objectType.display_name.toLowerCase()} title…`} /></label>
        {(objectType.default_template_md || templates.length > 0) && <label className="object-create-field">Template<select aria-label="Template" disabled={!!createdDraft.current} value={templateId} onChange={event => { const id = event.target.value; setTemplateId(id); setProperties(defaults({ ...templates.find(template => template.id === id)?.properties_preset, ...initialProperties.current, ...editedProperties.current })); }}>
          <option value="">Blank document</option>{objectType.default_template_md && <option value="__default__">Type default template</option>}{templates.map(template => <option key={template.id} value={template.id}>{template.name}</option>)}
        </select></label>}
        {objectType.schema.map(property => <div className="object-create-field" key={property.key}><span>{property.label}{property.required ? ' *' : ''}</span><PropertyFieldRenderer definition={property} value={properties[property.key]} onChange={value => { editedProperties.current[property.key] = value; setProperties(previous => ({ ...previous, [property.key]: value })); }} /></div>)}
        <label className="object-create-field">Associate with Project (Optional)<select aria-label="Associate with Project (Optional)" disabled={!!createdDraft.current} value={projectId} onChange={event => setProjectId(event.target.value)}><option value="">None (Unassociated)</option>{projects.map(project => <option key={project.id} value={project.id}>{project.name}</option>)}</select></label>
      </fieldset>
      {metadataError && <div className="directory-notice"><p role="alert">{metadataError}</p><button type="button" onClick={() => { void loadCreateMetadata(); }}>Retry</button></div>}
      {createError && <p className="object-create-error" role="alert">{createError}</p>}
      <footer><button type="button" className="sthira-button-secondary" disabled={creating} onClick={() => setCreateOpen(false)}>Cancel</button><button type="submit" className="sthira-button-primary" disabled={creating || !title.trim()}>{creating ? 'Creating…' : createdDraft.current ? 'Retry creation' : `Create ${objectType.display_name}`}</button></footer>
      </form>
    </div></div>}
  </section>;
}
export default ObjectCollectionContainer;
