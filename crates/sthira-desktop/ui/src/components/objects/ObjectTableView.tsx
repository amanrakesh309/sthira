import { useMemo, useState } from 'react';
import { ArrowDown, ArrowUp, ArrowUpDown, ExternalLink, Search } from 'lucide-react';
import { PropertyFieldRenderer } from './PropertyFieldRenderer';
import { directoryDate } from '../../utils/directory';
import type { ObjectType, ObjectItem } from '../../types/sthira';

interface ObjectTableViewProps {
  objectType: ObjectType;
  items: ObjectItem[];
  onUpdateProperty: (artifactId: string, propertyKey: string, newValue: any) => void;
  onOpenItem: (artifactId: string, projectId?: string | null) => void;
}

export function ObjectTableView({ objectType, items, onUpdateProperty, onOpenItem }: ObjectTableViewProps) {
  const [query, setQuery] = useState('');
  const [sort, setSort] = useState({ key: 'updated_at', direction: 'desc' as 'asc' | 'desc' });
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    const field = (item: ObjectItem): unknown => sort.key === 'title' ? item.title : sort.key === 'updated_at' ? item.updated_at : item.properties[sort.key];
    return items.filter(item => !q || item.title.toLowerCase().includes(q) || Object.values(item.properties).some(value => String(value ?? '').toLowerCase().includes(q)))
      .sort((a, b) => {
        const left = field(a) ?? '', right = field(b) ?? '';
        const result = typeof left === 'number' && typeof right === 'number' ? left - right : String(left).localeCompare(String(right), undefined, { numeric: true, sensitivity: 'base' });
        return sort.direction === 'asc' ? result : -result;
      });
  }, [items, query, sort]);
  const headers = [{ key: 'title', label: 'Name' }, ...objectType.schema.map(property => ({ key: property.key, label: property.label })), { key: 'updated_at', label: 'Updated' }];
  return <div className="object-table-panel">
    <div className="object-table-toolbar"><div className="directory-search"><Search size={16} aria-hidden="true" />
      <input type="search" aria-label={`Search ${objectType.display_name} collection`} placeholder={`Search ${objectType.display_name.toLowerCase()}…`} value={query} onChange={event => setQuery(event.target.value)} />
    </div><p role="status">Showing {filtered.length} of {items.length} items</p></div>
    <div className="object-table-scroll" role="region" aria-label={`${objectType.display_name} properties`} tabIndex={0}>
      <table className="object-property-table"><caption className="sr-only">{objectType.display_name} collection</caption>
        <thead><tr>{headers.map(header => <th key={header.key} scope="col" aria-sort={sort.key === header.key ? sort.direction === 'asc' ? 'ascending' : 'descending' : undefined}>
          <button type="button" onClick={() => setSort(previous => ({ key: header.key, direction: previous.key === header.key && previous.direction === 'asc' ? 'desc' : 'asc' }))}>
            {header.label}{sort.key === header.key ? sort.direction === 'asc' ? <ArrowUp size={14} /> : <ArrowDown size={14} /> : <ArrowUpDown size={14} />}
          </button>
        </th>)}<th scope="col"><span className="sr-only">Open</span></th></tr></thead>
        <tbody>{filtered.length === 0 ? <tr><td colSpan={objectType.schema.length + 3} className="object-table-empty">{items.length === 0 ? `No ${objectType.display_name.toLowerCase()} items yet. Create one to start this collection.` : 'No matching items. Try a different search.'}</td></tr>
          : filtered.map(item => <tr key={item.artifact_id}>
            <th scope="row"><button type="button" className="object-item-link" onClick={() => onOpenItem(item.artifact_id, item.project_id)}><span aria-hidden="true">{objectType.icon}</span><span>{item.title || 'Untitled'}</span></button></th>
            {objectType.schema.map(property => <td key={property.key}><PropertyFieldRenderer definition={property} value={item.properties[property.key]} compact onChange={value => onUpdateProperty(item.artifact_id, property.key, value)} /></td>)}
            <td className="object-table-date">{directoryDate(item.updated_at)}</td><td><button type="button" className="directory-icon-button" aria-label={`Open ${item.title || 'Untitled'}`} onClick={() => onOpenItem(item.artifact_id, item.project_id)}><ExternalLink size={16} /></button></td>
          </tr>)}
        </tbody>
      </table>
    </div>
  </div>;
}
export default ObjectTableView;
