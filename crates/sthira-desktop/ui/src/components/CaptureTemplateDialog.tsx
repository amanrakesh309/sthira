import { useEffect, useState } from 'react';
import { X, LibraryBig, CheckCircle2 } from 'lucide-react';
import { FormattedMarkdownView } from './FormattedMarkdownView';
import { listSynthesisTemplates, renderTemplateSkeleton, type SynthesisTemplate } from '../api/synthesisTemplates';

interface Props {
  onClose: () => void;
  onInsert: (markdown: string, template: SynthesisTemplate) => void;
  hasContent: boolean;
  suggestedTitle: string;
}

/** Optional, model-free starting structure. Insertion preserves existing text and undo. */
export function CaptureTemplateDialog({ onClose, onInsert, hasContent, suggestedTitle }: Props) {
  const [templates, setTemplates] = useState<SynthesisTemplate[]>([]);
  const [selected, setSelected] = useState<string>('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let current = true;
    listSynthesisTemplates('capture')
      .then(rows => {
        if (!current) return;
        setTemplates(rows);
        setSelected(rows[0]?.id || '');
      })
      .catch(failure => { if (current) setError(String(failure)); })
      .finally(() => { if (current) setLoading(false); });
    return () => { current = false; };
  }, []);

  useEffect(() => {
    const keyboard = (event: KeyboardEvent) => { if (event.key === 'Escape') { event.preventDefault(); onClose(); } };
    document.addEventListener('keydown', keyboard, true);
    return () => document.removeEventListener('keydown', keyboard, true);
  }, [onClose]);

  useEffect(() => {
    const first = document.querySelector<HTMLButtonElement>('.capture-template-list button');
    first?.focus();
  }, [loading]);

  const template = templates.find(value => value.id === selected) || null;
  const rendered = template ? renderTemplateSkeleton(template.template_skeleton_md, suggestedTitle || 'Untitled page') : '';

  return (
    <div className="capture-template-backdrop" role="presentation">
      <div className="capture-template-dialog" role="dialog" aria-modal="true" aria-labelledby="capture-template-title" data-testid="capture-template-dialog">
        <header>
          <div>
            <p className="capture-template-kicker"><LibraryBig size={13} /> Capture structure</p>
            <h3 id="capture-template-title">Insert an optional structure</h3>
            <p>These are starting headings only. Sthira will not fill them with AI text, and placeholders are not recorded facts.</p>
          </div>
          <button type="button" className="capture-template-close" aria-label="Close structure picker" onClick={onClose}><X size={16} /></button>
        </header>
        {error && <p role="alert" className="capture-template-error">{error}</p>}
        <div className="capture-template-body">
          <div className="capture-template-list" role="listbox" aria-label="Capture structures">
            {loading && <p>Loading structures…</p>}
            {!loading && templates.length === 0 && <p data-testid="capture-template-empty">No Capture structures yet. Create one in Settings → Templates.</p>}
            {templates.map(value => (
              <button key={value.id} type="button" role="option" aria-selected={selected === value.id}
                data-testid={`capture-template-${value.id}`}
                className={selected === value.id ? 'selected' : ''} onClick={() => setSelected(value.id)}>
                <CheckCircle2 size={14} />
                <span><strong>{value.name}</strong><small>{value.description || 'Optional starting structure'}</small></span>
              </button>
            ))}
          </div>
          <div className="capture-template-preview">
            <p className="capture-template-kicker">Preview</p>
            {template ? <FormattedMarkdownView content={rendered} /> : <p>Choose a structure to preview it.</p>}
          </div>
        </div>
        {hasContent && <p className="capture-template-note" data-testid="capture-template-existing-note">This page already has text. Inserting adds the structure at your cursor and leaves existing text intact; Undo removes the insertion.</p>}
        <footer>
          <button type="button" className="capture-template-secondary" onClick={onClose}>Cancel</button>
          <button type="button" className="capture-template-primary" data-testid="capture-template-insert" disabled={!template}
            onClick={() => { if (template) onInsert(rendered, template); }}>Insert structure</button>
        </footer>
      </div>
    </div>
  );
}
