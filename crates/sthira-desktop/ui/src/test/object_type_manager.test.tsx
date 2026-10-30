import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ObjectTypeManagerModal from '../components/objects/ObjectTypeManagerModal';
import { FocusView } from '../components/FocusView';
import type { ObjectType } from '../types/sthira';

// Mock Tauri invoke (the ipc.ts wrappers call through to it)
const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

const IDEA_TYPE: ObjectType = {
  id: 'idea',
  display_name: 'Idea',
  icon: '💡',
  color: '#F59E0B',
  schema: [
    {
      key: 'status',
      label: 'Status',
      property_type: 'select',
      options: ['spark', 'exploring', 'dormant', 'validated'],
      required: true,
      default_value: 'spark',
    },
    { key: 'tags', label: 'Tags', property_type: 'text' },
  ],
  is_system: true,
  created_at: 1000,
  description: 'A nascent thought worth exploring and validating.',
  default_template_md: '## The Spark\n\n## Potential Impact\n\n## Next Experiments',
};

describe('Object Type & Template Management System', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Test 1: ObjectTypeManagerModal → New Object Type → add a Select property
  // with custom option tags → save → cmd_create_object_type payload verified.
  // -------------------------------------------------------------------------
  it('Test 1: ObjectTypeManagerModal creates a type with a custom Select property and options', async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();

    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      switch (cmd) {
        case 'cmd_list_object_types':
          return [IDEA_TYPE];
        case 'cmd_get_type_templates':
          return [];
        case 'cmd_create_object_type':
          return args.typeDef.id;
        case 'cmd_update_object_type':
        case 'cmd_delete_object_type':
        case 'cmd_save_object_template':
          return null;
        default:
          return null;
      }
    });

    render(<ObjectTypeManagerModal isOpen={true} onClose={onClose} />);

    // Modal opens with the type list populated.
    await waitFor(() => {
      expect(screen.getByTestId('object-type-manager-modal')).toBeInTheDocument();
      expect(screen.getByTestId('type-list-item-idea')).toBeInTheDocument();
    });

    // Start a brand new object type.
    await user.click(screen.getByTestId('btn-new-object-type'));

    const nameInput = screen.getByTestId('input-type-name');
    await user.type(nameInput, 'Meeting Note');
    expect(nameInput).toHaveValue('Meeting Note');

    // Switch to the Properties tab and add a property.
    await user.click(screen.getByTestId('tab-properties'));
    await user.click(screen.getByTestId('btn-add-property'));

    const labelInput = screen.getByTestId('input-property-label-0');
    await user.type(labelInput, 'Category');
    // Key is auto-derived from the label (slugified).
    expect(screen.getByTestId('input-property-key-0')).toHaveValue('category');

    // Switch the property type to Select → options editor mounts with two seeded
    // convenience options that we remove before defining our own tags.
    fireEvent.change(screen.getByTestId('select-property-type-0'), {
      target: { value: 'select' },
    });

    await waitFor(() => {
      expect(screen.getByTestId('input-new-option-0')).toBeInTheDocument();
    });

    await user.click(screen.getByTestId('btn-remove-option-0-1'));
    await user.click(screen.getByTestId('btn-remove-option-0-0'));
    await waitFor(() => {
      expect(screen.queryByTestId('input-option-0-0')).not.toBeInTheDocument();
    });

    // Add two custom option tags.
    await user.type(screen.getByTestId('input-new-option-0'), 'agenda');
    await user.click(screen.getByTestId('btn-add-option-0'));
    await user.type(screen.getByTestId('input-new-option-0'), 'decisions');
    await user.click(screen.getByTestId('btn-add-option-0'));

    await waitFor(() => {
      expect(screen.getByTestId('input-option-0-0')).toHaveValue('agenda');
      expect(screen.getByTestId('input-option-0-1')).toHaveValue('decisions');
    });

    // Save the object type.
    await user.click(screen.getByTestId('btn-save-object-type'));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_create_object_type',
        expect.objectContaining({
          typeDef: expect.objectContaining({
            id: 'meeting_note',
            display_name: 'Meeting Note',
            schema: expect.arrayContaining([
              expect.objectContaining({
                key: 'category',
                label: 'Category',
                property_type: 'select',
                options: ['agenda', 'decisions'],
              }),
            ]),
          }),
        })
      );
    });

    // Success flash is surfaced to the user.
    await waitFor(() => {
      expect(screen.getByTestId('object-type-saved')).toBeInTheDocument();
    });
  });

  // -------------------------------------------------------------------------
  // Test 2: Opening a typed object in FocusView mounts DocumentPropertyHeader
  // with interactive, background-saving property chips.
  // -------------------------------------------------------------------------
  it('Test 2: FocusView mounts DocumentPropertyHeader with interactive chips for typed objects', async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'cmd_get_artifact_context':
          return {
            artifact: {
              id: 'art-idea-1',
              artifact_type: 'note',
              authorship: 'user',
              title: 'Sparse Attention Idea',
              content_md:
                '# Sparse Attention Idea\n\n## The Spark\n\nFlash attention variants.\n',
              source_url: null,
              content_hash: 'hash-idea-1',
              version: 1,
              file_path: null,
              last_synced_hash: null,
              is_unfinished_draft: false,
              deleted_at: null,
              created_at: 1000,
              updated_at: 1000,
            },
            epistemic_nodes: [],
            pending_proposals: [],
            project_id: null,
            is_unfinished_draft: false,
          };
        case 'cmd_list_object_types':
          return [IDEA_TYPE];
        case 'cmd_get_object_instance':
          return {
            artifact_id: 'art-idea-1',
            object_type_id: 'idea',
            properties: { status: 'spark', tags: 'attention' },
            updated_at: 1000,
          };
        case 'cmd_get_provider_catalog':
          return [];
        case 'cmd_save_artifact':
          return {
            id: 'art-idea-1',
            artifact_type: 'note',
            authorship: 'user',
            title: 'Sparse Attention Idea',
            content_md: '# Sparse Attention Idea\n\n## The Spark\n\nFlash attention variants.\n',
            source_url: null,
            content_hash: 'hash-idea-1',
            version: 2,
            file_path: null,
            last_synced_hash: null,
            is_unfinished_draft: false,
            deleted_at: null,
            created_at: 1000,
            updated_at: 2000,
          };
        case 'cmd_save_object_instance':
          return null;
        default:
          return null;
      }
    });

    render(<FocusView artifactId="art-idea-1" onBack={vi.fn()} onOpenCanvas={vi.fn()} />);

    // The interactive property tray mounts below the page title.
    await waitFor(() => {
      expect(screen.getByTestId('document-property-header')).toBeInTheDocument();
    });

    expect(screen.getByTestId('property-header-type-badge')).toHaveTextContent('Idea');
    expect(screen.getByTestId('property-chip-status')).toBeInTheDocument();
    expect(screen.getByTestId('property-chip-tags')).toBeInTheDocument();

    // The Select chip is an interactive combobox.
    const combobox = screen.getByRole('combobox');
    await user.selectOptions(combobox, 'validated');

    // Editing a chip saves in the background via cmd_save_object_instance.
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'cmd_save_object_instance',
        expect.objectContaining({
          instance: expect.objectContaining({
            artifact_id: 'art-idea-1',
            object_type_id: 'idea',
            properties: expect.objectContaining({ status: 'validated' }),
          }),
        })
      );
    });
  });

  // -------------------------------------------------------------------------
  // Test 3: Multiple templates → the slash menu surfaces the template selector
  // popover offering Default / Blank / custom presets.
  // -------------------------------------------------------------------------
  it('Test 3: Template selector popover appears when multiple templates exist', async () => {
    const user = userEvent.setup();

    mockInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'cmd_get_artifact_context':
          return {
            artifact: {
              id: 'art-doc-2',
              artifact_type: 'note',
              authorship: 'user',
              title: 'Idea Lab',
              content_md: '# Idea Lab\n\n',
              source_url: null,
              content_hash: 'hash-doc-2',
              version: 1,
              file_path: null,
              last_synced_hash: null,
              is_unfinished_draft: false,
              deleted_at: null,
              created_at: 1000,
              updated_at: 1000,
            },
            epistemic_nodes: [],
            pending_proposals: [],
            project_id: null,
            is_unfinished_draft: false,
          };
        case 'cmd_list_object_types':
          return [IDEA_TYPE];
        case 'cmd_get_object_instance':
          return null;
        case 'cmd_get_provider_catalog':
          return [];
        case 'cmd_save_artifact':
          return {
            id: 'art-doc-2',
            artifact_type: 'note',
            authorship: 'user',
            title: 'Idea Lab',
            content_md: '# Idea Lab\n\n',
            source_url: null,
            content_hash: 'hash-doc-2',
            version: 2,
            file_path: null,
            last_synced_hash: null,
            is_unfinished_draft: false,
            deleted_at: null,
            created_at: 1000,
            updated_at: 2000,
          };
        case 'cmd_get_type_templates':
          return [
            {
              id: 'idea-default',
              object_type_id: 'idea',
              name: 'Idea Exploration',
              icon: '💡',
              is_default: true,
              properties_preset: { status: 'spark' },
              body_template_md: '## The Spark\n\n## Potential Impact\n\n## Next Experiments',
              created_at: 1000,
            },
            {
              id: 'idea-rapid',
              object_type_id: 'idea',
              name: 'Rapid Spark',
              icon: '⚡',
              is_default: false,
              properties_preset: {},
              body_template_md: '## One-liner',
              created_at: 2000,
            },
          ];
        default:
          return null;
      }
    });

    render(<FocusView artifactId="art-doc-2" onBack={vi.fn()} onOpenCanvas={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByTestId('textarea-editor')).toBeInTheDocument();
    });

    const textarea = screen.getByTestId('textarea-editor') as HTMLTextAreaElement;
    fireEvent.change(textarea, { target: { value: '# Idea Lab\n\n/' } });
    (textarea as any).selectionStart = textarea.value.length;
    fireEvent.select(textarea);

    await waitFor(() => {
      expect(screen.getByTestId('slash-cmd-idea')).toBeInTheDocument();
    });

    await user.click(screen.getByTestId('slash-cmd-idea'));

    // Two templates → the selection popover is surfaced instead of the bare creator.
    await waitFor(() => {
      expect(screen.getByTestId('template-selector-popover')).toBeInTheDocument();
    });
    expect(screen.getByTestId('template-option-default')).toHaveTextContent('Idea Exploration');
    expect(screen.getByTestId('template-option-blank')).toHaveTextContent('Blank');
    expect(screen.getByTestId('template-option-idea-rapid')).toHaveTextContent('Rapid Spark');

    // Choosing a preset opens the Quick Object Creator, hydrated by that template.
    await user.click(screen.getByTestId('template-option-default'));
    await waitFor(() => {
      expect(screen.getByTestId('quick-object-creator-card')).toBeInTheDocument();
    });
  });
});
