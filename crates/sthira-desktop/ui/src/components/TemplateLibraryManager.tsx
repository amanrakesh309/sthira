import { ContextInfo } from './ContextInfo';
import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Archive, Copy, Eye, FileText, LibraryBig, Pencil, Plus, RotateCcw, Star, Trash2, X, CheckCircle2, AlertTriangle,
} from 'lucide-react';
import { FormattedMarkdownView } from './FormattedMarkdownView';
import {
  archiveSynthesisTemplate, clearSynthesisTemplateDefault, createSynthesisTemplate,
  deleteSynthesisTemplate, duplicateSynthesisTemplate, listSynthesisTemplates,
  restoreSynthesisTemplate, setSynthesisTemplateDefault, updateSynthesisTemplate,
  renderTemplateSkeleton, SYNTHESIS_TEMPLATES_CHANGED,
  type SynthesisTemplate, type SynthesisTemplateDraft, type TemplateTarget,
} from '../api/synthesisTemplates';
import '../styles/template-library.css';

type Filter = 'all' | TemplateTarget;
type Editor = { mode: 'create' | 'edit'; draft: SynthesisTemplateDraft } | null;

const emptyDraft = (target: TemplateTarget): SynthesisTemplateDraft => ({
  id: null,
  name: '',
  target_mode: target === 'capture' ? 'capture' : target,
  description: '',
  template_skeleton_md: '# {{title}}\n\n## Section\n',
  guidance: '',
  expected_version: null,
});

const badge = (template: SynthesisTemplate) => template.is_builtin ? 'Built-in' : 'Custom';
const label = (target: string) => target === 'research' ? 'Research' : target === 'plan' ? 'Plan' : 'Capture';

export function TemplateLibraryManager() {
  const [templates, setTemplates] = useState<SynthesisTemplate[]>([]);
  const [filter, setFilter] = useState<Filter>('all');
  const [includeArchived, setIncludeArchived] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [pending, setPending] = useState(false);
  const [editor, setEditor] = useState<Editor>(null);
  const [previewId, setPreviewId] = useState<string | null>(null);
  const [pageSize, setPageSize] = useState(50);

  const load = useCallback(async () => {
    setLoading(true);
    setError('');
    try {
      const rows = await listSynthesisTemplates(null, { includeArchived, limit: pageSize });
      setTemplates(rows);
    } catch (failure) {
      setError(String(failure));
    } finally {
      setLoading(false);
    }
  }, [includeArchived, pageSize]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    const refresh = () => { void load(); };
    const filterEvent = (event: Event) => {
      const value = (event as CustomEvent<TemplateTarget>).detail;
      if (value === 'research' || value === 'plan' || value === 'capture') setFilter(value);
    };
    window.addEventListener(SYNTHESIS_TEMPLATES_CHANGED, refresh);
    window.addEventListener('sthira:template-manager-filter', filterEvent);
    return () => {
      window.removeEventListener(SYNTHESIS_TEMPLATES_CHANGED, refresh);
      window.removeEventListener('sthira:template-manager-filter', filterEvent);
    };
  }, [load]);

  useEffect(() => {
    if (!editor) return;
    const keyboard = (event: KeyboardEvent) => { if (event.key === 'Escape') { event.preventDefault(); setEditor(null); } };
    document.addEventListener('keydown', keyboard, true);
    return () => document.removeEventListener('keydown', keyboard, true);
  }, [editor]);

  const visible = useMemo(
    () => templates.filter(template => filter === 'all' || template.target_mode === filter),
    [templates, filter]
  );
  const preview = templates.find(template => template.id === previewId) || null;
  const defaults = useMemo(() => ({
    research: templates.find(template => template.target_mode === 'research' && template.is_default && template.archived_at === null) || null,
    plan: templates.find(template => template.target_mode === 'plan' && template.is_default && template.archived_at === null) || null,
  }), [templates]);

  const run = async (action: () => Promise<void>, message: string) => {
    setPending(true);
    setError('');
    setNotice('');
    try {
      await action();
      setNotice(message);
      await load();
    } catch (failure) {
      setError(String(failure instanceof Error ? failure.message : failure));
    } finally {
      setPending(false);
    }
  };

  const save = async () => {
    if (!editor) return;
    const draft = editor.draft;
    await run(async () => {
      if (editor.mode === 'create') {
        const created = await createSynthesisTemplate(draft);
        setPreviewId(created.id);
      } else {
        const updated = await updateSynthesisTemplate(draft);
        setPreviewId(updated.id);
      }
      setEditor(null);
    }, 'Template saved.');
  };

  const duplicate = (template: SynthesisTemplate) => void run(async () => {
    const copy = await duplicateSynthesisTemplate(template.id);
    setPreviewId(copy.id);
  }, 'Template duplicated. Edit the copy to customize it.');

  const remove = (template: SynthesisTemplate) => {
    const confirmation = window.confirm(
      `Delete “${template.name}”? Existing pages and saved research runs keep their recorded template snapshot. This cannot be undone.`
    );
    if (!confirmation) return;
    void run(async () => { await deleteSynthesisTemplate(template.id); }, 'Template deleted. Existing pages were not changed.');
  };

  const archive = (template: SynthesisTemplate) => {
    const confirmation = window.confirm(
      `Archive “${template.name}”? It stops appearing for new runs, stops being a default, and existing pages stay unchanged.`
    );
    if (!confirmation) return;
    void run(async () => { await archiveSynthesisTemplate(template.id, template.version); }, 'Template archived.');
  };

  return (
    <section className="template-library" data-testid="template-library">
      <div className="settings-section-header template-library-header">
        <div className="context-info-heading">
          <h3 className="settings-section-title">Research &amp; Plan templates</h3>
          <ContextInfo title="templates">Reuse a structure for Research, Plan, or Capture. Templates guide structure and leave source access and approvals under your control.</ContextInfo>
        </div>
        <button type="button" className="template-primary" data-testid="template-new" disabled={pending}
          onClick={() => { setEditor({ mode: 'create', draft: emptyDraft(filter === 'all' ? 'research' : filter) }); setPreviewId(null); }}>
          <Plus size={15} /> New template
        </button>
      </div>

      <div className="template-defaults" aria-label="Workflow defaults">
        {(['research', 'plan'] as const).map(mode => (
          <div key={mode} className="template-default-row" data-testid={`template-default-row-${mode}`}>
            <span className="template-default-label">{label(mode)} default</span>
            <span className="template-default-value">{defaults[mode]?.name || 'No default — inherit nothing'}</span>
            {defaults[mode] && (
              <button type="button" className="template-link" data-testid={`template-clear-default-${mode}`} disabled={pending}
                onClick={() => void run(() => clearSynthesisTemplateDefault(mode), 'Default cleared for ' + label(mode) + '.')}>
                Clear
              </button>
            )}
          </div>
        ))}
      </div>

      <div className="template-filter-bar">
        <div className="template-filters" role="group" aria-label="Filter templates">
          {(['all', 'research', 'plan', 'capture'] as const).map(value => (
            <button key={value} type="button" data-testid={`template-filter-${value}`}
              aria-pressed={filter === value} className={filter === value ? 'active' : ''}
              onClick={() => setFilter(value)}>
              {value === 'all' ? 'All' : label(value)}
            </button>
          ))}
        </div>
        <label className="template-archive-toggle">
          <input type="checkbox" checked={includeArchived} onChange={event => setIncludeArchived(event.target.checked)} />
          <span>Show archived</span>
        </label>
      </div>

      {error && <p role="alert" className="template-alert error" data-testid="template-error">{error}</p>}
      {notice && <p role="status" className="template-alert success" data-testid="template-notice">{notice}</p>}

      <div className="template-library-grid">
        <div className="template-list" aria-busy={loading} data-testid="template-list">
          {loading && <p className="template-empty">Loading templates…</p>}
          {!loading && visible.length === 0 && (
            <p className="template-empty" data-testid="template-empty">
              No {filter === 'all' ? '' : label(filter).toLowerCase() + ' '}templates yet. Create one, or duplicate a built-in structure.
            </p>
          )}
          {visible.map(template => (
            <article key={template.id} data-testid={`template-list-item-${template.id}`}
              className={`template-card${previewId === template.id ? ' selected' : ''}${template.archived_at ? ' archived' : ''}`}>
              <div className="template-card-head">
                <div>
                  <h4 title={template.name}>{template.name}</h4>
                  {template.description && <p>{template.description}</p>}
                </div>
                <span className={`template-kind ${template.is_builtin ? 'builtin' : 'custom'}`}>{badge(template)}</span>
              </div>
              <div className="template-meta">
                <span>{label(template.target_mode)}</span>
                <span>Revision {template.version}</span>
                {template.is_default && <span className="template-default-badge"><Star size={11} /> Default</span>}
                {template.archived_at && <span className="template-archived-badge">Archived</span>}
              </div>
              <div className="template-card-actions">
                <button type="button" data-testid={`template-preview-${template.id}`} onClick={() => setPreviewId(template.id)}><Eye size={14} /> Preview</button>
                {!template.is_builtin && !template.archived_at && (
                  <button type="button" data-testid={`template-edit-${template.id}`} disabled={pending}
                    onClick={() => setEditor({ mode: 'edit', draft: {
                      id: template.id, name: template.name, target_mode: template.target_mode,
                      description: template.description, template_skeleton_md: template.template_skeleton_md,
                      guidance: template.guidance, expected_version: template.version,
                    } })}><Pencil size={14} /> Edit</button>
                )}
                <button type="button" data-testid={`template-duplicate-${template.id}`} disabled={pending} onClick={() => duplicate(template)}><Copy size={14} /> Duplicate</button>
                {template.target_mode !== 'capture' && !template.archived_at && !template.is_default && (
                  <button type="button" data-testid={`template-default-${template.id}`} disabled={pending}
                    onClick={() => void run(() => setSynthesisTemplateDefault(template.target_mode, template.id), label(template.target_mode) + ' default updated.')}>
                    <Star size={14} /> Set default
                  </button>
                )}
                {!template.archived_at ? (
                  <button type="button" data-testid={`template-archive-${template.id}`} disabled={pending} onClick={() => archive(template)}><Archive size={14} /> Archive</button>
                ) : (
                  <button type="button" data-testid={`template-restore-${template.id}`} disabled={pending}
                    onClick={() => void run(async () => { await restoreSynthesisTemplate(template.id); }, 'Template restored.')}><RotateCcw size={14} /> Restore</button>
                )}
                {!template.is_builtin && (
                  <button type="button" className="danger" data-testid={`template-delete-${template.id}`} disabled={pending} onClick={() => remove(template)}><Trash2 size={14} /> Delete</button>
                )}
              </div>
            </article>
          ))}
          {templates.length >= pageSize && (
            <button type="button" className="template-secondary" data-testid="template-load-more" disabled={pending}
              onClick={() => setPageSize(size => size + 50)}>Load more templates</button>
          )}
        </div>

        <aside className="template-preview-pane" data-testid="template-preview">
          {preview ? (
            <>
              <div className="template-preview-head">
                <div>
                  <p className="template-preview-kicker">{label(preview.target_mode)} · {badge(preview)} · Revision {preview.version}</p>
                  <h4>{preview.name}</h4>
                </div>
                <button type="button" aria-label="Close preview" className="template-icon" onClick={() => setPreviewId(null)}><X size={15} /></button>
              </div>
              {preview.description && <p className="template-preview-description">{preview.description}</p>}
              {preview.guidance && (
                <p className="template-preview-guidance"><strong>Guidance</strong><span>{preview.guidance}</span></p>
              )}
              <div className="template-preview-body" data-testid="template-preview-body">
                <FormattedMarkdownView content={renderTemplateSkeleton(preview.template_skeleton_md, 'Example page title')} />
              </div>
              <p className="template-preview-note">Preview shows the structure with an example title. Unknown placeholders stay visible; templates cannot execute code, read files, fetch URLs, or grant tools.</p>
            </>
          ) : (
            <div className="template-empty preview">
              <LibraryBig size={20} />
              <p>Choose a template to preview.</p>
            </div>
          )}
        </aside>
      </div>

      {editor && (
        <div className="template-editor" role="dialog" aria-modal="true" aria-label={editor.mode === 'create' ? 'Create template' : 'Edit template'} data-testid="template-editor">
          <div className="template-editor-card">
            <div className="template-editor-head">
              <h4>{editor.mode === 'create' ? 'New template' : 'Edit template'}</h4>
              <button type="button" className="template-icon" aria-label="Close template editor" onClick={() => setEditor(null)}><X size={16} /></button>
            </div>
            <div className="template-form">
              <label>Name
                <input data-testid="template-name" autoFocus value={editor.draft.name} maxLength={120} disabled={pending}
                  onChange={event => setEditor({ ...editor, draft: { ...editor.draft, name: event.target.value } })} />
              </label>
              <label>Workflow
                <select data-testid="template-target-mode" value={editor.draft.target_mode} disabled={pending}
                  onChange={event => setEditor({ ...editor, draft: { ...editor.draft, target_mode: event.target.value as TemplateTarget } })}>
                  <option value="research">Research</option>
                  <option value="plan">Plan</option>
                  <option value="capture">Capture (optional editor structure)</option>
                </select>
              </label>
              <label className="wide">Description
                <textarea data-testid="template-description" rows={2} maxLength={2000} value={editor.draft.description} disabled={pending}
                  onChange={event => setEditor({ ...editor, draft: { ...editor.draft, description: event.target.value } })} />
              </label>
              <label className="wide">Markdown structure
                <textarea data-testid="template-skeleton" rows={10} maxLength={20000} value={editor.draft.template_skeleton_md} disabled={pending}
                  onChange={event => setEditor({ ...editor, draft: { ...editor.draft, template_skeleton_md: event.target.value } })} />
              </label>
              <label className="wide">Generation guidance (optional)
                <textarea data-testid="template-guidance" rows={3} maxLength={4000} value={editor.draft.guidance} disabled={pending}
                  onChange={event => setEditor({ ...editor, draft: { ...editor.draft, guidance: event.target.value } })} />
              </label>
              <p className="template-form-note">
                Only <code>{'{{title}}'}</code> is substituted. Other placeholders stay visible. Guidance is subordinate to evidence, scope, and owner-review rules.
              </p>
              {editor.draft.target_mode === 'capture' && (
                <p className="template-form-note"><AlertTriangle size={13} /> Capture structures are optional starting points and never call a model.</p>
              )}
            </div>
            <div className="template-editor-actions">
              <button type="button" className="template-secondary" onClick={() => setEditor(null)} disabled={pending}>Cancel</button>
              <button type="button" className="template-primary" data-testid="template-save" disabled={pending || !editor.draft.name.trim() || !editor.draft.template_skeleton_md.trim()} onClick={() => void save()}>
                <CheckCircle2 size={15} /> {editor.mode === 'create' ? 'Create template' : 'Save changes'}
              </button>
            </div>
            <div className="template-editor-preview">
              <FileText size={14} />
              <p>{renderTemplateSkeleton(editor.draft.template_skeleton_md, 'Example page title').split('\n').filter((line: string) => line.startsWith('#')).slice(0, 6).join(' · ') || 'Add Markdown headings to see a structure summary.'}</p>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
