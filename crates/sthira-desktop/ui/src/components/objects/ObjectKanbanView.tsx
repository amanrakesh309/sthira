import { useMemo, useState } from 'react';
import { ExternalLink, Plus } from 'lucide-react';
import { PropertyFieldRenderer } from './PropertyFieldRenderer';
import type { ObjectType, ObjectItem } from '../../types/sthira';

interface ObjectKanbanViewProps {
  objectType: ObjectType;
  items: ObjectItem[];
  onUpdateProperty: (artifactId: string, propertyKey: string, newValue: any) => void;
  onOpenItem: (artifactId: string, projectId?: string | null) => void;
  onQuickCreate?: (initialProperties: Record<string, any>) => void;
}

export function ObjectKanbanView({ objectType, items, onUpdateProperty, onOpenItem, onQuickCreate }: ObjectKanbanViewProps) {
  const properties = useMemo(() => objectType.schema.filter(property => property.property_type === 'select'), [objectType.schema]);
  const [groupKey, setGroupKey] = useState(properties[0]?.key || '');
  const [draggedId, setDraggedId] = useState<string | null>(null);
  const [dropKey, setDropKey] = useState<string | null>(null);
  const group = properties.find(property => property.key === groupKey) || properties[0];
  const columns = useMemo(() => [...(group?.options || []).map(value => ({ key: `value:${value}`, value, label: value })),
    { key: 'unassigned', value: '', label: 'Unassigned' }], [group]);
  const move = (artifactId: string, value: string) => { if (group && items.some(item => item.artifact_id === artifactId)) onUpdateProperty(artifactId, group.key, value); };
  if (!group) return <div className="directory-empty"><h2>No Categorical Groupings Available</h2><p>To display a Kanban board, the “{objectType.display_name}” schema needs at least one select property, such as status or priority.</p></div>;
  return <div className="object-board">
    <div className="object-board-toolbar"><label>Group by: <select aria-label="Group by" value={group.key} onChange={event => setGroupKey(event.target.value)}>
      {properties.map(property => <option key={property.key} value={property.key}>{property.label}</option>)}
    </select></label><p>Drag items between columns, or use Move on each card.</p></div>
    <div className="object-board-scroll" role="region" aria-label={`${objectType.display_name} Kanban board`} tabIndex={0}>
      {columns.map(column => {
        const inColumn = items.filter(item => column.value ? item.properties[group.key] === column.value : !(group.options || []).includes(item.properties[group.key]));
        return <section className="object-board-column" key={column.key} aria-label={`${column.label} column`} data-drop-active={dropKey === column.key}
          onDragOver={event => { event.preventDefault(); setDropKey(column.key); }} onDragLeave={event => { if (!event.currentTarget.contains(event.relatedTarget as Node)) setDropKey(null); }}
          onDrop={event => { event.preventDefault(); move(event.dataTransfer.getData('text/plain'), column.value); setDropKey(null); setDraggedId(null); }}>
          <header><h2>{column.label}<span>{inColumn.length}</span></h2>{onQuickCreate && <button className="directory-icon-button" type="button" aria-label={`Add ${objectType.display_name} in ${column.label}`} onClick={() => onQuickCreate({ [group.key]: column.value })}><Plus size={16} /></button>}</header>
          <div className="object-board-cards">{inColumn.length === 0 ? <p className="object-board-empty">No items in this column.</p> : inColumn.map(item => <article key={item.artifact_id}
            className="object-board-card" draggable data-dragging={draggedId === item.artifact_id}
            onDragStart={event => { event.dataTransfer.setData('text/plain', item.artifact_id); event.dataTransfer.effectAllowed = 'move'; setDraggedId(item.artifact_id); }}
            onDragEnd={() => { setDraggedId(null); setDropKey(null); }}>
            <div className="object-board-card-heading"><button className="object-item-link" type="button" onClick={() => onOpenItem(item.artifact_id, item.project_id)}><span aria-hidden="true">{objectType.icon}</span> {item.title || 'Untitled'}</button>
              <button type="button" className="directory-icon-button" aria-label={`Open ${item.title || 'Untitled'}`} onClick={() => onOpenItem(item.artifact_id, item.project_id)}><ExternalLink size={16} /></button></div>
            <dl>{objectType.schema.filter(property => property.key !== group.key).slice(0, 3).map(property => {
              const value = item.properties[property.key];
              if (value === undefined || value === null || value === '') return null;
              return <div key={property.key}><dt>{property.label}</dt><dd><PropertyFieldRenderer definition={property} value={value} isReadOnly onChange={() => {}} /></dd></div>;
            })}</dl>
            <label className="object-board-move">Move: <select aria-label={`Move ${item.title || 'Untitled'} by ${group.label}`} value={columns.find(option => option.value === item.properties[group.key])?.key || 'unassigned'}
              onChange={event => move(item.artifact_id, columns.find(option => option.key === event.target.value)?.value || '')}>
              {columns.map(option => <option key={option.key} value={option.key}>{option.label}</option>)}
            </select></label>
          </article>)}</div>
        </section>;
      })}
    </div>
  </div>;
}
export default ObjectKanbanView;
