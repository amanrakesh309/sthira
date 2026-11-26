import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../api/ipc', async (importOriginal) => {
  const original = await importOriginal<typeof import('../api/ipc')>();
  return { ...original, isTauri: false };
});

import { TemplateLibraryManager } from '../components/TemplateLibraryManager';
import { CaptureTemplateDialog } from '../components/CaptureTemplateDialog';
import {
  createSynthesisTemplate, deleteSynthesisTemplate, listSynthesisTemplates,
  updateSynthesisTemplate, renderTemplateSkeleton,
} from '../api/synthesisTemplates';

beforeEach(() => localStorage.clear());

describe('template library (browser fixture)', () => {
  it('lists built-ins, filters by workflow, and previews rendered structure', async () => {
    const user = userEvent.setup();
    render(<TemplateLibraryManager />);
    expect(await screen.findByText('Research: Deep Dive')).toBeInTheDocument();
    expect(screen.getByText('Plan: MVP / Product Roadmap')).toBeInTheDocument();
    await user.click(screen.getByTestId('template-filter-plan'));
    expect(screen.queryByText('Research: Deep Dive')).not.toBeInTheDocument();
    expect(screen.getByText('Plan: MVP / Product Roadmap')).toBeInTheDocument();
    await user.click(screen.getByTestId('template-preview-plan-mvp-roadmap'));
    const preview = await screen.findByTestId('template-preview-body');
    expect(preview.textContent).toContain('Example page title');
    expect(preview.textContent).toContain('MVP scope');
  });

  it('creates, edits, duplicates, defaults, archives, and deletes a custom template', async () => {
    const user = userEvent.setup();
    render(<TemplateLibraryManager />);
    await screen.findByText('Research: Deep Dive');
    await user.click(screen.getByTestId('template-new'));
    await user.type(screen.getByTestId('template-name'), 'Roots and Reason');
    expect((screen.getByTestId('template-name') as HTMLInputElement).value).toBe('Roots and Reason');
    await user.clear(screen.getByTestId('template-skeleton'));
    await user.click(screen.getByTestId('template-skeleton'));
    await user.paste('# {{title}}\n\n## Scope\n\n## Sources');
    await user.selectOptions(screen.getByTestId('template-target-mode'), 'research');
    await waitFor(() => expect(screen.getByTestId('template-save')).toBeEnabled());
    await user.click(screen.getByTestId('template-save'));
    const created = (await listSynthesisTemplates('research')).find(t => t.name === 'Roots and Reason')!;
    expect(await screen.findByTestId(`template-list-item-${created.id}`)).toHaveTextContent('Roots and Reason');
    expect(created.is_builtin).toBe(false);
    expect(created.version).toBe(1);

    await user.click(screen.getByTestId(`template-edit-${created.id}`));
    const name = screen.getByTestId('template-name');
    await user.clear(name);
    await user.type(name, 'Roots and Reason v2');
    await user.click(screen.getByTestId('template-save'));
    const edited = (await listSynthesisTemplates('research')).find(t => t.id === created.id)!;
    expect(edited.name).toBe('Roots and Reason v2');
    expect(edited.version).toBe(2);

    await user.click(screen.getByTestId(`template-default-${created.id}`));
    await waitFor(async () => {
      const rows = await listSynthesisTemplates('research');
      expect(rows.filter(t => t.is_default)).toHaveLength(1);
    });

    await user.click(screen.getByTestId(`template-duplicate-${created.id}`));
    const copy = (await listSynthesisTemplates('research')).find(t => t.name === 'Roots and Reason v2 (copy)')!;
    expect(await screen.findByTestId(`template-list-item-${copy.id}`)).toHaveTextContent('Roots and Reason v2 (copy)');

    vi.spyOn(window, 'confirm').mockReturnValue(true);
    await user.click(screen.getByTestId(`template-archive-${created.id}`));
    await waitFor(async () => {
      const rows = await listSynthesisTemplates('research');
      expect(rows.some(t => t.id === created.id)).toBe(false);
    });
    await user.click(screen.getByLabelText(/Show archived/i));
    expect(await screen.findByText('Archived')).toBeInTheDocument();
    await user.click(screen.getByTestId(`template-delete-${created.id}`));
    await waitFor(async () => {
      const rows = await listSynthesisTemplates('research', { includeArchived: true });
      expect(rows.some(t => t.id === created.id)).toBe(false);
    });
  });

  it('rejects stale edits, blank names, and oversized skeletons in the domain layer', async () => {
    const created = await createSynthesisTemplate({ name: 'Versioned', target_mode: 'plan', description: '', template_skeleton_md: '# {{title}}', guidance: '', expected_version: null });
    const first = await updateSynthesisTemplate({ id: created.id, name: 'Versioned', target_mode: 'plan', description: '', template_skeleton_md: '# {{title}}\n\n## One', guidance: '', expected_version: 1 });
    expect(first.version).toBe(2);
    await expect(updateSynthesisTemplate({ id: created.id, name: 'Stale', target_mode: 'plan', description: '', template_skeleton_md: '# {{title}}', guidance: '', expected_version: 1 })).rejects.toThrow(/changed since/);
    await expect(createSynthesisTemplate({ name: '   ', target_mode: 'plan', description: '', template_skeleton_md: '# x', guidance: '', expected_version: null })).rejects.toThrow(/name/i);
    await expect(createSynthesisTemplate({ name: 'Huge', target_mode: 'plan', description: '', template_skeleton_md: 'x'.repeat(20_001), guidance: '', expected_version: null })).rejects.toThrow();
    await deleteSynthesisTemplate(created.id);
  });

  it('keeps unknown placeholders visible and substitutes only title', () => {
    const rendered = renderTemplateSkeleton('# {{title}}\n\n## {{unknown}} {Title}', 'Example');
    expect(rendered).toContain('# Example');
    expect(rendered).toContain('{{unknown}}');
    expect(rendered).toContain('Example');
  });
});

describe('capture structure dialog', () => {
  it('previews before inserting, warns on existing content, and performs no model call', async () => {
    const user = userEvent.setup();
    const onInsert = vi.fn();
    render(<CaptureTemplateDialog onClose={() => {}} onInsert={onInsert} hasContent suggestedTitle="My reading" />);
    expect(await screen.findByText('Capture: Reading Note')).toBeInTheDocument();
    expect(screen.getByTestId('capture-template-existing-note')).toBeInTheDocument();
    const preview = screen.getByText(/Citation/);
    expect(preview).toBeInTheDocument();
    await user.click(screen.getByTestId('capture-template-insert'));
    expect(onInsert).toHaveBeenCalledTimes(1);
    const markdown = onInsert.mock.calls[0][0] as string;
    expect(markdown).toContain('# My reading');
    expect(markdown).toContain('## Citation');
  });
});
