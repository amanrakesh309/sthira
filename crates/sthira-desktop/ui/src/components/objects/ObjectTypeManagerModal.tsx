import React, { useState, useEffect, useCallback } from 'react';
import {
  X,
  Plus,
  Trash2,
  Settings2,
  Check,
  FileText,
  ListChecks,
  Palette,
  AlertCircle,
  Save,
  Sparkles,
} from 'lucide-react';
import {
  listObjectTypes,
  createObjectType,
  updateObjectType,
  deleteObjectType,
  getTypeTemplates,
  saveObjectTemplate,
} from '../../api/ipc';
import type {
  ObjectType,
  ObjectTemplate,
  PropertyDefinition,
  PropertyType,
} from '../../types/sthira';

interface ObjectTypeManagerModalProps {
  isOpen: boolean;
  onClose: () => void;
  initialTypeId?: string | null;
  onTypesChanged?: () => void;
}

const EMOJI_OPTIONS = [
  '💡', '☑️', '⚖️', '🔗', '🏷️', '🎯', '📑', '👤',
  '🐞', '📚', '📌', '🚀', '💬', '🧪', '📦', '⭐',
  '🗓️', '🧠', '🔬', '🎨', '🧾', '🌱', '🛠️', '📈',
];

const COLOR_OPTIONS = [
  '#F59E0B', // Amber
  '#10B981', // Emerald
  '#6366F1', // Indigo
  '#3B82F6', // Blue
  '#EC4899', // Pink
  '#8B5CF6', // Purple
  '#14B8A6', // Teal
  '#F97316', // Orange
  '#64748B', // Slate
];

const PROPERTY_TYPE_OPTIONS: { value: PropertyType; label: string }[] = [
  { value: 'text', label: 'Text' },
  { value: 'number', label: 'Number' },
  { value: 'boolean', label: 'Checkbox' },
  { value: 'select', label: 'Select' },
  { value: 'multiselect', label: 'Multi-Select' },
  { value: 'date', label: 'Date' },
  { value: 'url', label: 'URL' },
];

/** Deterministic palette colour for a Select option tag. */
export const optionColorFor = (option: string, index: number): string => {
  const known: Record<string, string> = {
    spark: '#F59E0B',
    exploring: '#3B82F6',
    dormant: '#64748B',
    validated: '#10B981',
    low: '#3B82F6',
    medium: '#F59E0B',
    high: '#EC4899',
    unread: '#F59E0B',
    reading: '#3B82F6',
    processed: '#10B981',
  };
  const key = option.trim().toLowerCase();
  return known[key] || COLOR_OPTIONS[index % COLOR_OPTIONS.length];
};

export const slugifyTypeId = (raw: string): string =>
  raw
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');

interface TypeDraft {
  id: string;
  display_name: string;
  icon: string;
  color: string;
  description: string;
  schema: PropertyDefinition[];
  default_template_md: string;
  is_system: boolean;
  created_at: number;
}

const blankDraft = (): TypeDraft => ({
  id: '',
  display_name: '',
  icon: '📄',
  color: '#6366F1',
  description: '',
  schema: [],
  default_template_md: '',
  is_system: false,
  created_at: Date.now(),
});

const draftFromType = (t: ObjectType): TypeDraft => ({
  id: t.id,
  display_name: t.display_name,
  icon: t.icon || '📄',
  color: t.color || '#6366F1',
  description: t.description || '',
  schema: (t.schema || []).map((p) => ({ ...p, options: p.options ? [...p.options] : [] })),
  default_template_md: t.default_template_md || '',
  is_system: t.is_system,
  created_at: t.created_at,
});

type ManagerTab = 'general' | 'properties' | 'template';

interface SelectOptionsEditorProps {
  propertyIndex: number;
  options: string[];
  onChange: (options: string[]) => void;
}

/**
 * Inline editor for `Select` / `Multi-Select` option tags.
 * Supports Enter / comma-to-add, inline rename, reordering, and a palette colour preview.
 */
const SelectOptionsEditor: React.FC<SelectOptionsEditorProps> = ({
  propertyIndex,
  options,
  onChange,
}) => {
  const [newOptionText, setNewOptionText] = useState('');

  const commitNewOption = () => {
    const trimmed = newOptionText.trim();
    if (!trimmed) return;
    const parts = trimmed
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean);
    if (parts.length === 0) return;
    onChange([...options, ...parts]);
    setNewOptionText('');
  };

  const handleInputChange = (val: string) => {
    if (val.includes(',')) {
      const parts = val
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean);
      if (parts.length > 0) {
        onChange([...options, ...parts]);
        setNewOptionText('');
        return;
      }
    }
    setNewOptionText(val);
  };

  const handleEditOption = (optIdx: number, val: string) => {
    const updated = [...options];
    updated[optIdx] = val;
    onChange(updated);
  };

  const handleRemoveOption = (optIdx: number) => {
    onChange(options.filter((_, i) => i !== optIdx));
  };

  const handleMoveOption = (optIdx: number, direction: -1 | 1) => {
    const targetIdx = optIdx + direction;
    if (targetIdx < 0 || targetIdx >= options.length) return;
    const updated = [...options];
    const temp = updated[optIdx];
    updated[optIdx] = updated[targetIdx];
    updated[targetIdx] = temp;
    onChange(updated);
  };

  return (
    <div className="mt-2 pt-2 border-t border-[#ECE9E0] dark:border-[#262822] space-y-2">
      <div className="flex items-center justify-between">
        <span className="text-[10px] font-bold text-[#6E6D66] dark:text-[#9E9C93] uppercase tracking-wider">
          Option Tags ({options.length})
        </span>
        <span className="text-[10px] text-[#A09D94] dark:text-[#6E6D66]">
          Enter or comma to add
        </span>
      </div>

      <div className="space-y-1.5">
        {options.length === 0 ? (
          <div className="text-[11px] text-[#8C8A82] italic py-1">
            No options yet. Type below and press Enter.
          </div>
        ) : (
          options.map((opt, optIdx) => {
            const color = optionColorFor(opt, optIdx);
            return (
              <div key={`${opt}-${optIdx}`} className="flex items-center gap-1.5">
                <span
                  data-testid={`option-swatch-${propertyIndex}-${optIdx}`}
                  className="w-3.5 h-3.5 rounded-full border shrink-0"
                  style={{ backgroundColor: `${color}33`, borderColor: color }}
                />
                <input
                  type="text"
                  data-testid={`input-option-${propertyIndex}-${optIdx}`}
                  value={opt}
                  onChange={(e) => handleEditOption(optIdx, e.target.value)}
                  className="flex-1 px-2 py-0.5 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-white dark:bg-[#1F201B] text-xs outline-none focus:border-[#282C25] dark:focus:border-[#EDEAE2]"
                />
                <span
                  className="text-[9px] font-mono px-1.5 py-0.5 rounded"
                  style={{ backgroundColor: `${color}1A`, color }}
                >
                  {color}
                </span>
                <button
                  type="button"
                  onClick={() => handleMoveOption(optIdx, -1)}
                  disabled={optIdx === 0}
                  className="p-0.5 text-[#8C8A82] hover:text-[#1E201C] disabled:opacity-30 cursor-pointer"
                  title="Move up"
                >
                  ▲
                </button>
                <button
                  type="button"
                  onClick={() => handleMoveOption(optIdx, 1)}
                  disabled={optIdx === options.length - 1}
                  className="p-0.5 text-[#8C8A82] hover:text-[#1E201C] disabled:opacity-30 cursor-pointer"
                  title="Move down"
                >
                  ▼
                </button>
                <button
                  type="button"
                  data-testid={`btn-remove-option-${propertyIndex}-${optIdx}`}
                  onClick={() => handleRemoveOption(optIdx)}
                  className="p-0.5 text-[#8C8A82] hover:text-red-600 cursor-pointer"
                  title="Remove option"
                >
                  <Trash2 className="w-3 h-3" />
                </button>
              </div>
            );
          })
        )}
      </div>

      <div className="flex items-center gap-1.5">
        <input
          type="text"
          data-testid={`input-new-option-${propertyIndex}`}
          value={newOptionText}
          onChange={(e) => handleInputChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              commitNewOption();
            }
          }}
          placeholder="Add option tag..."
          className="flex-1 px-2 py-1 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-[#FAF8F3] dark:bg-[#161714] text-xs outline-none focus:border-[#282C25] dark:focus:border-[#EDEAE2]"
        />
        <button
          type="button"
          data-testid={`btn-add-option-${propertyIndex}`}
          onClick={commitNewOption}
          className="px-2.5 py-1 rounded-lg text-xs font-semibold bg-[#EFECE4] dark:bg-[#252820] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] cursor-pointer"
        >
          Add
        </button>
      </div>
    </div>
  );
};

/**
 * Phase 16: Object Type & Template Manager.
 *
 * Left pane: every object type (system + user-created) plus `[+ New Object Type]`.
 * Right pane: three-tab type editor — General / Properties / Default Template.
 */
export const ObjectTypeManagerModal: React.FC<ObjectTypeManagerModalProps> = ({
  isOpen,
  onClose,
  initialTypeId,
  onTypesChanged,
}) => {
  const [types, setTypes] = useState<ObjectType[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<TypeDraft | null>(null);
  const [isNew, setIsNew] = useState(false);
  const [activeTab, setActiveTab] = useState<ManagerTab>('general');
  const [templates, setTemplates] = useState<ObjectTemplate[]>([]);
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [savedFlash, setSavedFlash] = useState<string | null>(null);
  const [templateName, setTemplateName] = useState('');

  const loadTypes = useCallback(async () => {
    try {
      const loaded = await listObjectTypes();
      setTypes(loaded);
      return loaded;
    } catch (err) {
      console.error('Failed to load object types', err);
      setErrorMessage('Failed to load object types.');
      return [] as ObjectType[];
    }
  }, []);

  const loadTemplates = useCallback(async (typeId: string | null) => {
    if (!typeId) {
      setTemplates([]);
      return;
    }
    try {
      const list = await getTypeTemplates(typeId);
      setTemplates(list);
    } catch (err) {
      console.error('Failed to load templates', err);
      setTemplates([]);
    }
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    setErrorMessage(null);
    setSavedFlash(null);
    (async () => {
      const loaded = await loadTypes();
      const target =
        (initialTypeId && loaded.find((t) => t.id === initialTypeId)) || loaded[0] || null;
      if (target) {
        setSelectedId(target.id);
        setDraft(draftFromType(target));
        setIsNew(false);
        setActiveTab('general');
        await loadTemplates(target.id);
      } else {
        setSelectedId(null);
        setDraft(blankDraft());
        setIsNew(true);
        setActiveTab('general');
        setTemplates([]);
      }
    })();
  }, [isOpen, initialTypeId, loadTypes, loadTemplates]);

  const selectType = async (type: ObjectType) => {
    setSelectedId(type.id);
    setDraft(draftFromType(type));
    setIsNew(false);
    setActiveTab('general');
    setErrorMessage(null);
    setSavedFlash(null);
    await loadTemplates(type.id);
  };

  const startNewType = () => {
    setSelectedId(null);
    setDraft(blankDraft());
    setIsNew(true);
    setActiveTab('general');
    setTemplates([]);
    setErrorMessage(null);
    setSavedFlash(null);
  };

  const updateDraft = (patch: Partial<TypeDraft>) => {
    setDraft((prev) => (prev ? { ...prev, ...patch } : prev));
  };

  const updateProperty = (index: number, patch: Partial<PropertyDefinition>) => {
    setDraft((prev) => {
      if (!prev) return prev;
      const schema = prev.schema.map((p, i) => (i === index ? { ...p, ...patch } : p));
      return { ...prev, schema };
    });
  };

  const addProperty = () => {
    setDraft((prev) => {
      if (!prev) return prev;
      return {
        ...prev,
        schema: [
          ...prev.schema,
          {
            key: '',
            label: '',
            property_type: 'text',
            options: [],
            required: false,
          } as PropertyDefinition,
        ],
      };
    });
  };

  const removeProperty = (index: number) => {
    setDraft((prev) => {
      if (!prev) return prev;
      return { ...prev, schema: prev.schema.filter((_, i) => i !== index) };
    });
  };

  const handleSave = async () => {
    if (!draft) return;
    setErrorMessage(null);

    const displayName = draft.display_name.trim();
    if (!displayName) {
      setErrorMessage('Please provide a display name for the object type.');
      return;
    }

    const id = (draft.id || slugifyTypeId(displayName)).trim();
    if (!id) {
      setErrorMessage('Please provide a valid object type id.');
      return;
    }

    // Normalise schema: derive keys from labels when blank, drop fully-empty rows.
    const schema: PropertyDefinition[] = draft.schema
      .map((p) => {
        const label = (p.label || '').trim();
        const key = (p.key || '').trim() || slugifyTypeId(label);
        return { ...p, key, label: label || key };
      })
      .filter((p) => p.key.length > 0);

    const payload: ObjectType = {
      id,
      display_name: displayName,
      icon: draft.icon || '📄',
      color: draft.color || '#6366F1',
      schema,
      is_system: draft.is_system,
      created_at: draft.created_at || Date.now(),
      description: draft.description || '',
      default_template_md: draft.default_template_md || '',
    };

    try {
      setIsSaving(true);
      if (isNew) {
        await createObjectType(payload);
      } else {
        await updateObjectType(payload);
      }

      const loaded = await loadTypes();
      const refreshed = loaded.find((t) => t.id === id);
      setDraft(refreshed ? draftFromType(refreshed) : draftFromType(payload));
      setSelectedId(id);
      setIsNew(false);
      setSavedFlash(`Saved "${payload.display_name}"`);
      window.dispatchEvent(new CustomEvent('sthira:refresh_objects'));
      onTypesChanged?.();
    } catch (err) {
      console.error('Failed to save object type', err);
      setErrorMessage('Failed to save the object type. Please try again.');
    } finally {
      setIsSaving(false);
    }
  };

  const handleDeleteType = async () => {
    if (!draft || isNew || draft.is_system) return;
    try {
      await deleteObjectType(draft.id);
      const loaded = await loadTypes();
      const next = loaded[0] || null;
      if (next) {
        await selectType(next);
      } else {
        startNewType();
      }
      window.dispatchEvent(new CustomEvent('sthira:refresh_objects'));
      onTypesChanged?.();
    } catch (err) {
      console.error('Failed to delete object type', err);
      setErrorMessage('Cannot delete this object type.');
    }
  };

  const handleSaveAsTemplate = async () => {
    if (!draft) return;
    const name = templateName.trim();
    if (!name) return;
    try {
      await saveObjectTemplate({
        id: '',
        object_type_id: draft.id || slugifyTypeId(draft.display_name),
        name,
        icon: draft.icon || '📄',
        is_default: templates.length === 0,
        properties_preset: {},
        body_template_md: draft.default_template_md || '',
        created_at: 0,
      });
      setTemplateName('');
      await loadTemplates(draft.id);
      window.dispatchEvent(new CustomEvent('sthira:refresh_objects'));
    } catch (err) {
      console.error('Failed to save object template', err);
      setErrorMessage('Failed to save the template.');
    }
  };

  if (!isOpen) return null;

  return (
    <div
      data-testid="object-type-manager-modal"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-sm animate-in fade-in"
    >
      <div className="w-full max-w-5xl h-[86vh] bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl shadow-2xl overflow-hidden flex flex-col">
        {/* Header */}
        <div className="px-6 py-4 border-b border-[#ECE9E0] dark:border-[#2C2E27] flex items-center justify-between bg-[#FAF8F3] dark:bg-[#161714] shrink-0">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-xl bg-[#EFECE4] dark:bg-[#252820] text-[#282C25] dark:text-[#EDEAE2] flex items-center justify-center">
              <Settings2 className="w-4 h-4" />
            </div>
            <div>
              <h3 className="text-sm font-bold text-[#1E201C] dark:text-[#EDEAE2]">
                Object Types &amp; Templates
              </h3>
              <p className="text-[11px] text-[#6E6D66] dark:text-[#9E9C93]">
                Define schemas, properties, and default Markdown bodies.
              </p>
            </div>
          </div>
          <button
            type="button"
            data-testid="btn-close-object-type-manager"
            onClick={onClose}
            className="p-1.5 rounded-lg text-[#6E6D66] dark:text-[#9E9C93] hover:bg-[#EFECE4] dark:hover:bg-[#252820] transition-colors cursor-pointer"
            title="Close"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Body: Left Type List + Right Editor */}
        <div className="flex-1 flex overflow-hidden min-h-0">
          {/* Left Pane */}
          <aside className="w-64 shrink-0 border-r border-[#ECE9E0] dark:border-[#2C2E27] bg-[#FAF8F3] dark:bg-[#161714] flex flex-col overflow-hidden">
            <div className="px-3 pt-3 pb-2 flex items-center justify-between">
              <span className="text-[10px] font-bold text-[#8C8A82] dark:text-[#A8A59C] uppercase tracking-wider">
                Object Types ({types.length})
              </span>
              <button
                type="button"
                data-testid="btn-new-object-type"
                onClick={startNewType}
                className="inline-flex items-center gap-1 px-2 py-1 rounded-lg text-[11px] font-semibold bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] hover:opacity-90 transition-opacity cursor-pointer"
              >
                <Plus className="w-3 h-3" />
                New
              </button>
            </div>

            <div className="flex-1 overflow-y-auto px-2 pb-3 space-y-0.5">
              {types.map((t) => {
                const isActive = !isNew && selectedId === t.id;
                return (
                  <button
                    key={t.id}
                    type="button"
                    data-testid={`type-list-item-${t.id}`}
                    onClick={() => selectType(t)}
                    className={`w-full text-left px-2.5 py-2 rounded-xl flex items-center gap-2.5 transition-colors cursor-pointer ${
                      isActive
                        ? 'bg-[#282C25] dark:bg-[#282B24] text-white shadow-xs'
                        : 'text-[#2E2F2B] dark:text-[#EDEAE2] hover:bg-[#EFECE4] dark:hover:bg-[#22241F]'
                    }`}
                  >
                    <span
                      className="w-6 h-6 rounded-lg flex items-center justify-center text-sm shrink-0"
                      style={{
                        backgroundColor: isActive ? 'rgba(255,255,255,0.18)' : `${t.color}1A`,
                      }}
                    >
                      {t.icon || '📄'}
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-xs font-semibold">
                        {t.display_name}
                      </span>
                      <span
                        className={`block truncate text-[10px] ${
                          isActive ? 'text-zinc-300' : 'text-[#8C8A82] dark:text-[#9E9C93]'
                        }`}
                      >
                        {t.is_system ? 'Built-in' : 'Custom'} · {t.schema?.length || 0} props
                      </span>
                    </span>
                  </button>
                );
              })}

              {isNew && (
                <div className="px-2.5 py-2 rounded-xl bg-[#EFECE4] dark:bg-[#22241F] text-xs font-semibold text-[#3A3B36] dark:text-[#EDEAE2] flex items-center gap-2.5">
                  <span className="w-6 h-6 rounded-lg flex items-center justify-center text-sm shrink-0 bg-white/60 dark:bg-[#1C1D19]">
                    {draft?.icon || '📄'}
                  </span>
                  <span className="truncate">{draft?.display_name || 'Untitled type'}</span>
                </div>
              )}
            </div>
          </aside>

          {/* Right Pane */}
          <section className="flex-1 flex flex-col overflow-hidden min-w-0">



            {draft ? (
              <>
                {/* Tab Bar */}
                <div className="px-6 pt-3 border-b border-[#ECE9E0] dark:border-[#2C2E27] flex items-center justify-between shrink-0">
                  <div className="flex items-center gap-1">
                    {(
                      [
                        { id: 'general', label: 'General', icon: <Palette className="w-3.5 h-3.5" /> },
                        { id: 'properties', label: 'Properties', icon: <ListChecks className="w-3.5 h-3.5" /> },
                        { id: 'template', label: 'Default Template', icon: <FileText className="w-3.5 h-3.5" /> },
                      ] as { id: ManagerTab; label: string; icon: React.ReactNode }[]
                    ).map((tab) => (
                      <button
                        key={tab.id}
                        type="button"
                        data-testid={`tab-${tab.id}`}
                        onClick={() => setActiveTab(tab.id)}
                        className={`inline-flex items-center gap-1.5 px-3 py-2 text-xs font-semibold border-b-2 -mb-px transition-colors cursor-pointer ${
                          activeTab === tab.id
                            ? 'border-[#282C25] dark:border-[#EDEAE2] text-[#1E201C] dark:text-[#EDEAE2]'
                            : 'border-transparent text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2]'
                        }`}
                      >
                        {tab.icon}
                        {tab.label}
                      </button>
                    ))}
                  </div>

                  <span className="text-[10px] font-mono text-[#A09D94] dark:text-[#6E6D66]">
                    {isNew ? 'new type' : draft.id}
                  </span>
                </div>

                {/* Scrollable Tab Body */}
                <div className="flex-1 overflow-y-auto px-6 py-5 space-y-5 text-xs">
                  {activeTab === 'general' && (
                    <div className="space-y-4 max-w-2xl">
                      <div>
                        <label className="block text-[11px] font-semibold text-[#5A5852] dark:text-[#9E9C93] mb-1">
                          Display Name <span className="text-red-500">*</span>
                        </label>
                        <input
                          type="text"
                          data-testid="input-type-name"
                          value={draft.display_name}
                          onChange={(e) => {
                            const name = e.target.value;
                            updateDraft({
                              display_name: name,
                              id: isNew ? slugifyTypeId(name) : draft.id,
                            });
                          }}
                          placeholder="e.g. Meeting Note"
                          className="w-full px-3 py-2 rounded-xl border border-[#E8E5DC] dark:border-[#2C2E27] bg-[#FAF8F3] dark:bg-[#161714] text-sm text-[#1E201C] dark:text-[#EDEAE2] outline-none focus:border-[#282C25] dark:focus:border-[#EDEAE2]"
                        />
                        <p className="mt-1 text-[10px] text-[#8C8A82] dark:text-[#6E6D66] font-mono">
                          id: {draft.id || '(auto-generated from name)'}
                        </p>
                      </div>

                      <div>
                        <label className="block text-[11px] font-semibold text-[#5A5852] dark:text-[#9E9C93] mb-1">
                          Description
                        </label>
                        <textarea
                          data-testid="textarea-type-description"
                          value={draft.description}
                          onChange={(e) => updateDraft({ description: e.target.value })}
                          rows={2}
                          placeholder="What does this object type represent?"
                          className="w-full px-3 py-2 rounded-xl border border-[#E8E5DC] dark:border-[#2C2E27] bg-[#FAF8F3] dark:bg-[#161714] text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none resize-none focus:border-[#282C25] dark:focus:border-[#EDEAE2]"
                        />
                      </div>

                      <div>
                        <label className="block text-[11px] font-semibold text-[#5A5852] dark:text-[#9E9C93] mb-1.5">
                          Icon
                        </label>
                        <div className="flex flex-wrap gap-1.5">
                          {EMOJI_OPTIONS.map((emoji) => (
                            <button
                              key={emoji}
                              type="button"
                              data-testid={`icon-option-${emoji}`}
                              onClick={() => updateDraft({ icon: emoji })}
                              className={`w-8 h-8 rounded-lg flex items-center justify-center text-base border transition-colors cursor-pointer ${
                                draft.icon === emoji
                                  ? 'border-[#282C25] dark:border-[#EDEAE2] bg-[#EFECE4] dark:bg-[#252820]'
                                  : 'border-[#E8E5DC] dark:border-[#2C2E27] hover:bg-[#FAF8F3] dark:hover:bg-[#22241F]'
                              }`}
                            >
                              {emoji}
                            </button>
                          ))}
                        </div>
                      </div>

                      <div>
                        <label className="block text-[11px] font-semibold text-[#5A5852] dark:text-[#9E9C93] mb-1.5">
                          Color
                        </label>
                        <div className="flex flex-wrap items-center gap-2">
                          {COLOR_OPTIONS.map((hex) => (
                            <button
                              key={hex}
                              type="button"
                              data-testid={`color-option-${hex}`}
                              onClick={() => updateDraft({ color: hex })}
                              title={hex}
                              className={`w-7 h-7 rounded-full border-2 transition-transform cursor-pointer ${
                                draft.color === hex
                                  ? 'border-[#282C25] dark:border-[#EDEAE2] scale-110'
                                  : 'border-transparent hover:scale-105'
                              }`}
                              style={{ backgroundColor: hex }}
                            />
                          ))}
                          <span className="text-[10px] font-mono text-[#8C8A82] dark:text-[#6E6D66]">
                            {draft.color}
                          </span>
                        </div>
                      </div>
                    </div>
                  )}


                  {activeTab === 'properties' && (
                    <div className="space-y-3 max-w-3xl">
                      <div className="flex items-center justify-between">
                        <span className="text-[11px] font-semibold text-[#5A5852] dark:text-[#9E9C93]">
                          Property Schema ({draft.schema.length})
                        </span>
                        <button
                          type="button"
                          data-testid="btn-add-property"
                          onClick={addProperty}
                          className="inline-flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-[11px] font-semibold bg-[#EFECE4] dark:bg-[#252820] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] transition-colors cursor-pointer"
                        >
                          <Plus className="w-3 h-3" />
                          Add Property
                        </button>
                      </div>

                      {draft.schema.length === 0 && (
                        <div className="text-[11px] text-[#8C8A82] dark:text-[#6E6D66] italic py-6 text-center border border-dashed border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl">
                          No properties yet. Add Text, Select, Date, Checkbox, Number, URL, or
                          Multi-Select fields to shape this type.
                        </div>
                      )}



                      {draft.schema.map((prop, idx) => (
                        <div
                          key={idx}
                          data-testid={`property-row-${idx}`}
                          className="p-3 rounded-xl border border-[#E8E5DC] dark:border-[#2C2E27] bg-[#FAF8F3] dark:bg-[#161714] space-y-2"
                        >
                          <div className="flex items-center gap-2">
                            <input
                              type="text"
                              data-testid={`input-property-label-${idx}`}
                              value={prop.label}
                              onChange={(e) => {
                                const label = e.target.value;
                                updateProperty(idx, { label, key: slugifyTypeId(label) });
                              }}
                              placeholder="Property label"
                              className="flex-1 px-2 py-1 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-white dark:bg-[#1F201B] text-xs font-medium outline-none focus:border-[#282C25] dark:focus:border-[#EDEAE2]"
                            />
                            <select
                              value={prop.property_type}
                              data-testid={`select-property-type-${idx}`}
                              onChange={(e) => {
                                const newType = e.target.value as PropertyType;
                                updateProperty(idx, {
                                  property_type: newType,
                                  options:
                                    (newType === 'select' || newType === 'multiselect') &&
                                    (!prop.options || prop.options.length === 0)
                                      ? ['Option 1', 'Option 2']
                                      : prop.options || [],
                                });
                              }}
                              className="w-32 px-2 py-1 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-white dark:bg-[#1F201B] text-xs font-medium outline-none cursor-pointer focus:border-[#282C25] dark:focus:border-[#EDEAE2]"
                            >
                              {PROPERTY_TYPE_OPTIONS.map((opt) => (
                                <option key={opt.value} value={opt.value}>
                                  {opt.label}
                                </option>
                              ))}
                            </select>
                            <label className="inline-flex items-center gap-1 text-[10px] text-[#6E6D66] dark:text-[#9E9C93] cursor-pointer">
                              <input
                                type="checkbox"
                                data-testid={`checkbox-property-required-${idx}`}
                                checked={!!prop.required}
                                onChange={(e) =>
                                  updateProperty(idx, { required: e.target.checked })
                                }
                                className="w-3 h-3 accent-[#282C25]"
                              />
                              Required
                            </label>
                            <button
                              type="button"
                              data-testid={`btn-remove-property-${idx}`}
                              onClick={() => removeProperty(idx)}
                              className="p-1 text-[#8C8A82] hover:text-red-600 rounded hover:bg-red-50 dark:hover:bg-red-950/30 transition-colors cursor-pointer"
                              title="Remove property"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                            </button>
                          </div>

                          <div className="flex items-center gap-2">
                            <span className="text-[10px] font-mono text-[#8C8A82] dark:text-[#6E6D66]">
                              key:
                            </span>
                            <input
                              type="text"
                              data-testid={`input-property-key-${idx}`}
                              value={prop.key}
                              onChange={(e) => updateProperty(idx, { key: e.target.value })}
                              placeholder="key"
                              className="w-40 px-2 py-0.5 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-white dark:bg-[#1F201B] text-[11px] font-mono outline-none focus:border-[#282C25] dark:focus:border-[#EDEAE2]"
                            />
                          </div>

                          {(prop.property_type === 'select' ||
                            prop.property_type === 'multiselect') && (
                            <SelectOptionsEditor
                              propertyIndex={idx}
                              options={prop.options || []}
                              onChange={(newOptions) =>
                                updateProperty(idx, { options: newOptions })
                              }
                            />
                          )}
                        </div>
                      ))}
                    </div>
                  )}

                  {activeTab === 'template' && (
                    <div className="space-y-4">
                      <div>
                        <label className="block text-[11px] font-semibold text-[#5A5852] dark:text-[#9E9C93] mb-1">
                          Default Markdown Skeleton
                        </label>
                        <p className="text-[10px] text-[#8C8A82] dark:text-[#6E6D66] mb-2">
                          Every new <span className="font-semibold">{draft.display_name || 'object'}</span>{' '}
                          starts with this body. Heading structure, checklists, and callouts are
                          preserved verbatim.
                        </p>
                        <textarea
                          data-testid="textarea-default-template"
                          value={draft.default_template_md}
                          onChange={(e) => updateDraft({ default_template_md: e.target.value })}
                          rows={10}
                          placeholder={'## Section\n\n- [ ] First step'}
                          className="w-full px-3 py-2 rounded-xl border border-[#E8E5DC] dark:border-[#2C2E27] bg-[#FAF8F3] dark:bg-[#161714] text-xs font-mono leading-relaxed text-[#1E201C] dark:text-[#EDEAE2] outline-none resize-y focus:border-[#282C25] dark:focus:border-[#EDEAE2]"
                        />
                      </div>

                      <div className="rounded-xl border border-[#E8E5DC] dark:border-[#2C2E27] bg-[#FAF8F3] dark:bg-[#161714] p-3">
                        <div className="flex items-center gap-1.5 mb-2">
                          <Sparkles className="w-3.5 h-3.5 text-[#8C6D1F] dark:text-[#D4A738]" />
                          <span className="text-[11px] font-bold text-[#6E6D66] dark:text-[#9E9C93] uppercase tracking-wider">
                            Live Preview
                          </span>
                        </div>
                        <div
                          data-testid="template-live-preview"
                          className="text-xs text-[#3A3B36] dark:text-[#D4D1C7] whitespace-pre-wrap font-sans"
                        >
                          {draft.default_template_md || (
                            <span className="italic text-[#A09D94]">
                              Empty template — a bare title skeleton will be used.
                            </span>
                          )}
                        </div>
                      </div>

                      <div className="rounded-xl border border-[#E8E5DC] dark:border-[#2C2E27] p-3 space-y-2">
                        <span className="text-[11px] font-bold text-[#6E6D66] dark:text-[#9E9C93] uppercase tracking-wider">
                          Named Templates ({templates.length})
                        </span>
                        {templates.length > 0 && (
                          <div className="flex flex-wrap gap-1.5">
                            {templates.map((t) => (
                              <span
                                key={t.id}
                                data-testid={`template-chip-${t.id}`}
                                className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] border border-[#E8E5DC] dark:border-[#2C2E27] bg-[#FAF8F3] dark:bg-[#161714]"
                              >
                                <span>{t.icon || '📄'}</span>
                                <span>{t.name}</span>
                                {t.is_default && (
                                  <span className="text-[9px] px-1 rounded bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310]">
                                    default
                                  </span>
                                )}
                              </span>
                            ))}
                          </div>
                        )}
                        <div className="flex items-center gap-1.5">
                          <input
                            type="text"
                            data-testid="input-template-name"
                            value={templateName}
                            onChange={(e) => setTemplateName(e.target.value)}
                            placeholder="Save current body as template..."
                            className="flex-1 px-2 py-1 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-[#FAF8F3] dark:bg-[#161714] text-xs outline-none focus:border-[#282C25] dark:focus:border-[#EDEAE2]"
                          />
                          <button
                            type="button"
                            data-testid="btn-save-as-template"
                            onClick={handleSaveAsTemplate}
                            disabled={!templateName.trim()}
                            className="inline-flex items-center gap-1 px-2.5 py-1 rounded-lg text-[11px] font-semibold bg-[#EFECE4] dark:bg-[#252820] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] disabled:opacity-40 transition-colors cursor-pointer"
                          >
                            <Save className="w-3 h-3" />
                            Save
                          </button>
                        </div>
                      </div>
                    </div>
                  )}
                </div>



                {/* Footer */}
                <div className="px-6 py-3 border-t border-[#ECE9E0] dark:border-[#2C2E27] bg-[#FAF8F3] dark:bg-[#161714] flex items-center justify-between shrink-0">
                  <div className="flex items-center gap-2 min-w-0">
                    {errorMessage && (
                      <span
                        data-testid="object-type-error"
                        className="inline-flex items-center gap-1.5 text-[11px] text-red-600 dark:text-red-400"
                      >
                        <AlertCircle className="w-3.5 h-3.5 shrink-0" />
                        <span className="truncate">{errorMessage}</span>
                      </span>
                    )}
                    {savedFlash && !errorMessage && (
                      <span
                        data-testid="object-type-saved"
                        className="inline-flex items-center gap-1.5 text-[11px] text-emerald-700 dark:text-emerald-400 font-semibold"
                      >
                        <Check className="w-3.5 h-3.5" />
                        {savedFlash}
                      </span>
                    )}
                  </div>

                  <div className="flex items-center gap-2">
                    {!isNew && !draft.is_system && (
                      <button
                        type="button"
                        data-testid="btn-delete-object-type"
                        onClick={handleDeleteType}
                        className="inline-flex items-center gap-1.5 px-3 py-2 rounded-xl text-xs font-medium text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/30 transition-colors cursor-pointer"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                        Delete Type
                      </button>
                    )}
                    <button
                      type="button"
                      onClick={onClose}
                      className="px-4 py-2 rounded-xl text-xs font-medium text-[#6E6D66] dark:text-[#9E9C93] hover:bg-[#EFECE4] dark:hover:bg-[#252820] transition-colors cursor-pointer"
                    >
                      Close
                    </button>
                    <button
                      type="button"
                      data-testid="btn-save-object-type"
                      onClick={handleSave}
                      disabled={isSaving}
                      className="px-5 py-2 rounded-xl text-xs font-semibold bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] hover:opacity-90 transition-opacity cursor-pointer shadow-xs disabled:opacity-50"
                    >
                      {isSaving ? 'Saving...' : isNew ? 'Create Object Type' : 'Save Changes'}
                    </button>
                  </div>
                </div>
              </>
            ) : (
              <div className="flex-1 flex items-center justify-center text-xs text-[#8C8A82] dark:text-[#6E6D66]">
                Select an object type or create a new one.
              </div>
            )}
          </section>
        </div>
      </div>
    </div>
  );
};

export default ObjectTypeManagerModal;
