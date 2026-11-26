import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './ipc';

export type TemplateTarget = 'research' | 'plan' | 'capture';

export interface SynthesisTemplate {
  id: string;
  name: string;
  target_mode: TemplateTarget;
  description: string;
  is_default: boolean;
  template_skeleton_md: string;
  created_at: number;
  is_builtin: boolean;
  version: number;
  guidance: string;
  updated_at: number;
  archived_at: number | null;
}

export interface SynthesisTemplateDraft {
  id?: string | null;
  name: string;
  target_mode: TemplateTarget;
  description: string;
  template_skeleton_md: string;
  guidance: string;
  expected_version?: number | null;
}

export const SYNTHESIS_TEMPLATES_CHANGED = 'sthira:synthesis-templates-changed';
export const NO_TEMPLATE = '__none__';
export const INHERIT_DEFAULT = '__inherit__';

const previewKey = 'sthira-preview-synthesis-templates-v2';
const PREVIEW_SEED: SynthesisTemplate[] = [
  {
    id: 'research-deep-dive', name: 'Research: Deep Dive', target_mode: 'research',
    description: 'A focused investigation: what is established, what competes, and what remains uncertain.',
    is_default: false, is_builtin: true, version: 1, guidance: '',
    template_skeleton_md: '# {{title}}\n\n## Question and scope\n\n## Context\n\n## What the evidence shows\n\n## Competing interpretations and limits\n\n## Uncertainty and open questions\n\n## Sources\n',
    created_at: 0, updated_at: 0, archived_at: null,
  },
  {
    id: 'research-evidence-comparison', name: 'Research: Evidence Comparison', target_mode: 'research',
    description: 'Compare claims or options against shared criteria and record where evidence is thin.',
    is_default: false, is_builtin: true, version: 1, guidance: '',
    template_skeleton_md: '# {{title}}\n\n## Question\n\n## Options or claims compared\n\n## Criteria\n\n## Evidence table\n| Claim or option | Supporting evidence | Limits or contradictions |\n| --- | --- | --- |\n| | | |\n\n## What the evidence does not establish\n\n## Sources\n',
    created_at: 0, updated_at: 0, archived_at: null,
  },
  {
    id: 'research-cultural-four-lenses', name: 'Research: Cultural Tradition — Four Lenses', target_mode: 'research',
    description: 'An evidence-led guide to a tradition through description, origins, context, and present understanding.',
    is_default: false, is_builtin: true, version: 1, guidance:
      'Treat explanations of a tradition as evidence-dependent interpretations, not universal claims. Preserve disagreement and uncertainty.',
    template_skeleton_md: '# {{title}}\n\n## Scope\n- Tradition/practice:\n- Region/community:\n- Time period:\n- Research question:\n\n## 1. What is it?\nDescription, variations, and relevant context.\n\n## 2. Why did it form?\nHistorical evidence, proposed explanations, and uncertainty.\n\n## 3. Why might it have worked in its original context?\nClaimed functions, supporting evidence, limitations, and alternatives.\n\n## 4. How is it understood today?\nContemporary perspectives, changes, disagreements, and regional differences.\n\n## Evidence and competing interpretations\n\n## What remains uncertain\n\n## Sources\n',
    created_at: 0, updated_at: 0, archived_at: null,
  },
  {
    id: 'plan-mvp-roadmap', name: 'Plan: MVP / Product Roadmap', target_mode: 'plan',
    description: 'A product roadmap that isolates the smallest useful release and its follow-on work.',
    is_default: false, is_builtin: true, version: 1, guidance:
      'Define measures as proposals for the owner to decide; do not fabricate targets or adoption numbers.',
    template_skeleton_md: '# {{title}}\n\n## Product intent and user problem\n\n## MVP scope\n\n## Out of scope\n\n## Milestones\n### Milestone 1 — \n- [ ] \n\n### Milestone 2 — \n- [ ] \n\n## Risks and assumptions\n\n## Success measures (define, do not invent)\n',
    created_at: 0, updated_at: 0, archived_at: null,
  },
  {
    id: 'capture-reading-note', name: 'Capture: Reading Note', target_mode: 'capture',
    description: 'An optional note structure for a reading session. Capture never calls a model.',
    is_default: false, is_builtin: true, version: 1, guidance:
      'Optional starting structure only. Sthira does not fill this in with AI text during Capture.',
    template_skeleton_md: '# {{title}}\n\n## Citation\n\n## Why I read this\n\n## Key points in my own words\n\n## Quotes to verify\n\n## Questions\n\n## Related notes\n',
    created_at: 0, updated_at: 0, archived_at: null,
  },
];

function readPreview(): SynthesisTemplate[] {
  try {
    const stored = JSON.parse(localStorage.getItem(previewKey) || 'null');
    if (Array.isArray(stored)) return stored as SynthesisTemplate[];
  } catch {
    // fall through to the fixture seed
  }
  return PREVIEW_SEED.map(template => ({ ...template }));
}
function writePreview(templates: SynthesisTemplate[]) {
  localStorage.setItem(previewKey, JSON.stringify(templates));
}
function notify() {
  window.dispatchEvent(new Event(SYNTHESIS_TEMPLATES_CHANGED));
}
function previewDraftId() {
  return `preview-${Math.random().toString(36).slice(2, 10)}`;
}
function requireName(draft: SynthesisTemplateDraft) {
  if (!draft.name.trim()) throw new Error('Template name cannot be empty.');
  if (draft.name.length > 120) throw new Error('Template name is too long (maximum 120 characters).');
  if (!['research', 'plan', 'capture'].includes(draft.target_mode)) throw new Error('Choose Research, Plan, or Capture.');
  if (!draft.template_skeleton_md.trim()) throw new Error('Template structure cannot be empty.');
  if (draft.template_skeleton_md.length > 20_000) throw new Error('Template structure is too long (maximum 20,000 characters).');
  if (draft.description.length > 2_000) throw new Error('Template description is too long (maximum 2,000 characters).');
  if (draft.guidance.length > 4_000) throw new Error('Template guidance is too long (maximum 4,000 characters).');
}

export async function listSynthesisTemplates(
  targetMode?: TemplateTarget | null,
  options: { includeArchived?: boolean; limit?: number; offset?: number } = {}
): Promise<SynthesisTemplate[]> {
  if (isTauri) {
    return invoke<SynthesisTemplate[]>('cmd_list_synthesis_templates', {
      targetMode: targetMode || null,
      includeArchived: options.includeArchived === true,
      limit: options.limit ?? 100,
      offset: options.offset ?? 0,
    });
  }
  const all = readPreview()
    .filter(template => (options.includeArchived ? true : template.archived_at === null))
    .filter(template => !targetMode || template.target_mode === targetMode)
    .sort((a, b) => a.target_mode.localeCompare(b.target_mode) || Number(b.is_builtin) - Number(a.is_builtin) || a.name.localeCompare(b.name));
  const offset = options.offset ?? 0;
  return all.slice(offset, offset + (options.limit ?? 100));
}

export async function getSynthesisTemplate(id: string): Promise<SynthesisTemplate | null> {
  if (isTauri) return invoke<SynthesisTemplate | null>('cmd_get_synthesis_template', { id });
  return readPreview().find(template => template.id === id) || null;
}

export async function createSynthesisTemplate(draft: SynthesisTemplateDraft): Promise<SynthesisTemplate> {
  if (isTauri) {
    const value = await invoke<SynthesisTemplate>('cmd_create_synthesis_template', { draft });
    notify();
    return value;
  }
  requireName(draft);
  const templates = readPreview();
  const now = Date.now();
  const created: SynthesisTemplate = {
    id: draft.id?.trim() || previewDraftId(),
    name: draft.name.trim(),
    target_mode: draft.target_mode,
    description: draft.description.trim(),
    is_default: false,
    template_skeleton_md: draft.template_skeleton_md,
    created_at: now,
    is_builtin: false,
    version: 1,
    guidance: draft.guidance.trim(),
    updated_at: now,
    archived_at: null,
  };
  if (templates.some(template => template.id === created.id)) throw new Error('A template with this identifier already exists.');
  writePreview([...templates, created]);
  notify();
  return created;
}

export async function updateSynthesisTemplate(draft: SynthesisTemplateDraft): Promise<SynthesisTemplate> {
  if (isTauri) {
    const value = await invoke<SynthesisTemplate>('cmd_update_synthesis_template', { draft });
    notify();
    return value;
  }
  requireName(draft);
  if (!draft.id) throw new Error('Choose the template to edit.');
  if (draft.expected_version == null) throw new Error('Reload this template before saving.');
  const templates = readPreview();
  const index = templates.findIndex(template => template.id === draft.id);
  if (index < 0) throw new Error('This template no longer exists.');
  const current = templates[index];
  if (current.is_builtin) throw new Error('Built-in templates are read-only. Duplicate this template to customize it.');
  if (current.version !== draft.expected_version) throw new Error('This template changed since you opened it. Reload it and reapply your edits.');
  const updated: SynthesisTemplate = {
    ...current,
    name: draft.name.trim(),
    target_mode: draft.target_mode,
    description: draft.description.trim(),
    template_skeleton_md: draft.template_skeleton_md,
    guidance: draft.guidance.trim(),
    version: current.version + 1,
    updated_at: Date.now(),
    is_default: draft.target_mode === current.target_mode ? current.is_default : false,
  };
  templates[index] = updated;
  writePreview(templates);
  notify();
  return updated;
}

export async function duplicateSynthesisTemplate(id: string): Promise<SynthesisTemplate> {
  if (isTauri) {
    const value = await invoke<SynthesisTemplate>('cmd_duplicate_synthesis_template', { id });
    notify();
    return value;
  }
  const templates = readPreview();
  const source = templates.find(template => template.id === id);
  if (!source) throw new Error('The template to duplicate no longer exists.');
  const copy: SynthesisTemplate = {
    ...source,
    id: previewDraftId(),
    name: `${source.name} (copy)`.slice(0, 120),
    is_builtin: false,
    is_default: false,
    version: 1,
    created_at: Date.now(),
    updated_at: Date.now(),
    archived_at: null,
  };
  writePreview([...templates, copy]);
  notify();
  return copy;
}

export async function setSynthesisTemplateDefault(targetMode: TemplateTarget, id: string): Promise<void> {
  if (isTauri) {
    await invoke('cmd_set_synthesis_template_default', { targetMode, id });
    notify();
    return;
  }
  writePreview(readPreview().map(template =>
    template.target_mode === targetMode && template.archived_at === null
      ? { ...template, is_default: template.id === id }
      : template
  ));
  notify();
}

export async function clearSynthesisTemplateDefault(targetMode: TemplateTarget): Promise<void> {
  if (isTauri) {
    await invoke('cmd_clear_synthesis_template_default', { targetMode });
    notify();
    return;
  }
  writePreview(readPreview().map(template => template.target_mode === targetMode ? { ...template, is_default: false } : template));
  notify();
}

export async function archiveSynthesisTemplate(id: string, expectedVersion?: number | null): Promise<SynthesisTemplate> {
  if (isTauri) {
    const value = await invoke<SynthesisTemplate>('cmd_archive_synthesis_template', { id, expectedVersion: expectedVersion ?? null });
    notify();
    return value;
  }
  const templates = readPreview();
  const index = templates.findIndex(template => template.id === id);
  if (index < 0) throw new Error('This template no longer exists.');
  if (expectedVersion != null && templates[index].version !== expectedVersion) throw new Error('This template changed since it was loaded. Reload it before archiving.');
  templates[index] = { ...templates[index], archived_at: Date.now(), is_default: false, version: templates[index].version + 1, updated_at: Date.now() };
  writePreview(templates);
  notify();
  return templates[index];
}

export async function restoreSynthesisTemplate(id: string): Promise<SynthesisTemplate> {
  if (isTauri) {
    const value = await invoke<SynthesisTemplate>('cmd_restore_synthesis_template', { id });
    notify();
    return value;
  }
  const templates = readPreview();
  const index = templates.findIndex(template => template.id === id);
  if (index < 0) throw new Error('This template no longer exists.');
  templates[index] = { ...templates[index], archived_at: null, version: templates[index].version + 1, updated_at: Date.now() };
  writePreview(templates);
  notify();
  return templates[index];
}

export async function deleteSynthesisTemplate(id: string): Promise<void> {
  if (isTauri) {
    await invoke('cmd_delete_synthesis_template', { id });
    notify();
    return;
  }
  const templates = readPreview();
  const target = templates.find(template => template.id === id);
  if (!target) throw new Error('This template no longer exists.');
  if (target.is_builtin) throw new Error('Built-in templates cannot be deleted. Archive it instead.');
  writePreview(templates.filter(template => template.id !== id));
  notify();
}

export function openTemplateManager(targetMode?: TemplateTarget) {
  window.dispatchEvent(new CustomEvent('sthira:open-settings', { detail: 'templates' }));
  if (targetMode) {
    // Let Settings mount the manager before applying the requested filter.
    window.setTimeout(() => {
      window.dispatchEvent(new CustomEvent('sthira:template-manager-filter', { detail: targetMode }));
    }, 0);
  }
}

/** Explicit, bounded placeholder substitution shared with previews. Unknown placeholders stay visible. */
export function renderTemplateSkeleton(skeleton: string, title: string) {
  return skeleton.replace(/\{\{title\}\}/g, title.trim()).replace(/\{Title\}/g, title.trim());
}
