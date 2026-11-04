import { useEffect, useRef, useState } from 'react';
import { Calendar, Pencil } from 'lucide-react';
import type { PropertyDefinition } from '../../types/sthira';

interface PropertyFieldRendererProps {
  definition: PropertyDefinition;
  value: any;
  onChange: (value: any) => void;
  isReadOnly?: boolean;
  compact?: boolean;
}

export function PropertyFieldRenderer({ definition, value, onChange, isReadOnly = false, compact = false }: PropertyFieldRendererProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value ?? '');
  const committed = useRef(value ?? '');
  const skipCommit = useRef(false);
  useEffect(() => { setDraft(value ?? ''); committed.current = value ?? ''; }, [value]);
  const begin = () => { skipCommit.current = false; setEditing(true); };
  const commit = () => {
    setEditing(false);
    if (skipCommit.current) { skipCommit.current = false; return; }
    if (draft !== committed.current) { committed.current = draft; onChange(draft); }
  };
  const label = definition.label;
  const text = value === undefined || value === null || value === '' ? '—' : String(value);
  const inputKeys = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') { event.preventDefault(); event.currentTarget.blur(); }
    if (event.key === 'Escape') {
      event.preventDefault(); event.stopPropagation(); skipCommit.current = true;
      setDraft(value ?? ''); event.currentTarget.blur(); setEditing(false);
    }
  };

  if (definition.property_type === 'boolean') return <label className="property-field property-checkbox">
    <input type="checkbox" aria-label={label} checked={Boolean(value)} disabled={isReadOnly} onChange={event => onChange(event.target.checked)} />
    {!compact && <span>{value ? 'Yes' : 'No'}</span>}
  </label>;

  if (definition.property_type === 'select') return isReadOnly ? <span className="property-value">{text}</span> :
    <select className="property-field property-select" aria-label={label} value={value || ''} disabled={isReadOnly}
      onChange={event => onChange(event.target.value)}>
      <option value="">Select…</option>{(definition.options || []).map(option => <option key={option} value={option}>{option}</option>)}
    </select>;

  if (definition.property_type === 'multiselect') {
    const selected: string[] = Array.isArray(value) ? value : [];
    if (isReadOnly) return <span className="property-value">{selected.join(', ') || '—'}</span>;
    return <details className="property-field property-multiselect"><summary aria-label={label}>{selected.join(', ') || 'Select…'}</summary>
      <fieldset><legend className="sr-only">{label}</legend>{(definition.options || []).map(option => <label key={option}>
        <input type="checkbox" checked={selected.includes(option)} onChange={event => onChange(event.target.checked ? [...selected, option] : selected.filter(v => v !== option))} />{option}
      </label>)}</fieldset>
    </details>;
  }

  if (definition.property_type === 'date') return isReadOnly ? <span className="property-value"><Calendar size={14} />{text}</span> :
    <input className="property-field" aria-label={label} type="date" value={value ? String(value) : ''} onChange={event => onChange(event.target.value)} />;

  if (definition.property_type === 'url' && !editing && value) return <span className="property-url">
    <a href={/^https?:\/\//i.test(String(value)) ? String(value) : `https://${value}`} target="_blank" rel="noreferrer" title={String(value)}>{String(value).replace(/^https?:\/\//, '')}</a>
    {!isReadOnly && <button type="button" className="directory-icon-button" aria-label={`Edit ${label}`} onClick={begin}><Pencil size={14} /></button>}
  </span>;

  if (isReadOnly) return <span className="property-value" title={text}>{text}</span>;
  if (definition.property_type !== 'number' && definition.property_type !== 'url' && compact && !editing) return <button type="button" className="property-field property-inline-edit" onClick={begin} aria-label={`Edit ${label}`} title={text}>{text === '—' ? 'Empty' : text}</button>;

  return <input className={`property-field ${definition.property_type === 'number' ? 'property-number' : ''}`} aria-label={label}
    type={definition.property_type === 'number' ? 'number' : 'text'} value={draft}
    autoFocus={editing} placeholder={definition.property_type === 'url' ? 'https://…' : `Enter ${label.toLowerCase()}…`}
    onChange={event => setDraft(event.target.value)} onBlur={commit} onKeyDown={inputKeys} />;
}

export default PropertyFieldRenderer;
