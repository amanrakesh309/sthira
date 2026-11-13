import React, { useState, useRef } from 'react';
import { X, Plus, Trash2, AlertCircle, ChevronUp, ChevronDown } from 'lucide-react';
import { createObjectType } from '../../api/ipc';
import { useDialogFocus } from '../../hooks/useDialogFocus';
import type { ObjectType, PropertyDefinition, PropertyType } from '../../types/sthira';

interface ObjectTypeBuilderModalProps {
  isOpen: boolean;
  onClose: () => void;
  onTypeCreated: (newType: ObjectType) => void;
}

const EMOJI_OPTIONS = ['💡', '☑️', '⚖️', '🔗', '🏷️', '🎯', '📑', '👤', '🐞', '📚', '📌', '🚀', '💬', '🧪', '📦', '⭐'];
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

interface SelectPropertyOptionsEditorProps {
  propertyIndex: number;
  options: string[];
  onChange: (options: string[]) => void;
}

const SelectPropertyOptionsEditor: React.FC<SelectPropertyOptionsEditorProps> = ({
  propertyIndex,
  options,
  onChange,
}) => {
  const [newOptionText, setNewOptionText] = useState('');

  const handleAddOption = () => {
    const trimmed = newOptionText.trim();
    if (!trimmed) return;

    // Split on commas if multiple were entered/pasted
    const parts = trimmed
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean);

    if (parts.length === 0) return;

    onChange([...options, ...parts]);
    setNewOptionText('');
  };

  const handleAddInputChange = (val: string) => {
    // If user types a comma or pastes a comma-separated list, auto-split and append
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
    <div className="mt-2.5 pt-2 border-t border-[var(--sthira-border)] space-y-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5 text-[11px] font-semibold text-[var(--sthira-text-muted)]">
          <span className="w-1.5 h-1.5 rounded-full bg-[var(--sthira-text-subtle)]" />
          <span>Dropdown Options ({options.length})</span>
        </div>
        <span className="text-[10px] text-[var(--sthira-text-subtle)]">
          Press Enter or comma to add
        </span>
      </div>

      {/* Bulleted list of options */}
      <div className="space-y-1.5">
        {options.length === 0 ? (
          <div className="text-[11px] text-[var(--sthira-text-subtle)] italic py-1 pl-2">
            No options added yet. Type below and press Enter to add.
          </div>
        ) : (
          options.map((opt, optIdx) => (
            <div
              key={optIdx}
              className="flex items-center gap-1.5 group bg-[var(--sthira-surface)] px-2 py-1 rounded-lg border border-[var(--sthira-border)] hover:border-[var(--sthira-border-strong)] transition-colors"
            >
              <span className="text-[var(--sthira-text-subtle)] select-none text-xs font-bold shrink-0">•</span>
              <input
                type="text"
                value={opt}
                data-testid={`input-option-${propertyIndex}-${optIdx}`}
                onChange={(e) => handleEditOption(optIdx, e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault();
                  }
                }}
                placeholder={`Option ${optIdx + 1}`}
                className="flex-1 bg-transparent text-xs font-medium text-[var(--sthira-text)] placeholder:text-[var(--sthira-text-muted)] outline-none"
              />
              <div className="flex items-center gap-0.5 opacity-75 group-hover:opacity-100 transition-opacity">
                <button
                  type="button"
                  disabled={optIdx === 0}
                  onClick={() => handleMoveOption(optIdx, -1)}
                  data-testid={`btn-move-up-option-${propertyIndex}-${optIdx}`}
                  className="p-0.5 text-[var(--sthira-text-subtle)] hover:text-[var(--sthira-text)] disabled:opacity-25 cursor-pointer disabled:cursor-not-allowed rounded"
                  aria-label={`Move option ${optIdx + 1} up`}
                  title="Move Up"
                >
                  <ChevronUp className="w-3 h-3" />
                </button>
                <button
                  type="button"
                  disabled={optIdx === options.length - 1}
                  onClick={() => handleMoveOption(optIdx, 1)}
                  data-testid={`btn-move-down-option-${propertyIndex}-${optIdx}`}
                  className="p-0.5 text-[var(--sthira-text-subtle)] hover:text-[var(--sthira-text)] disabled:opacity-25 cursor-pointer disabled:cursor-not-allowed rounded"
                  aria-label={`Move option ${optIdx + 1} down`}
                  title="Move Down"
                >
                  <ChevronDown className="w-3 h-3" />
                </button>
                <button
                  type="button"
                  onClick={() => handleRemoveOption(optIdx)}
                  data-testid={`btn-remove-option-${propertyIndex}-${optIdx}`}
                  className="p-0.5 text-[var(--sthira-text-subtle)] hover:text-[var(--sthira-danger)] rounded transition-colors cursor-pointer"
                  aria-label={`Remove option ${optIdx + 1}`}
                  title="Remove option"
                >
                  <X className="w-3 h-3" />
                </button>
              </div>
            </div>
          ))
        )}

        {/* Add new option input row */}
        <div className="flex items-center gap-1.5 pt-0.5">
          <div className="flex-1 flex items-center gap-1.5 bg-[var(--sthira-surface)] px-2 py-1 rounded-lg border border-dashed border-[var(--sthira-border-strong)] focus-within:border-[var(--sthira-primary)] focus-within:border-solid transition-all">
            <Plus className="w-3 h-3 text-[var(--sthira-text-subtle)] shrink-0" />
            <input
              type="text"
              value={newOptionText}
              data-testid={`input-new-option-${propertyIndex}`}
              onChange={(e) => handleAddInputChange(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  handleAddOption();
                }
              }}
              placeholder="Add option (press Enter or comma to split)..."
              className="flex-1 bg-transparent text-xs text-[var(--sthira-text)] placeholder:text-[var(--sthira-text-muted)] outline-none"
            />
          </div>
          <button
            type="button"
            onClick={handleAddOption}
            data-testid={`btn-add-option-${propertyIndex}`}
            className="px-2.5 py-1 rounded-lg bg-[var(--sthira-canvas)] hover:bg-[var(--sthira-surface-muted)] border border-[var(--sthira-border)] text-[var(--sthira-text)] text-xs font-semibold flex items-center gap-1 transition-colors cursor-pointer shrink-0"
          >
            <Plus className="w-3 h-3" />
            <span>Add</span>
          </button>
        </div>
      </div>
    </div>
  );
};

export const ObjectTypeBuilderModal: React.FC<ObjectTypeBuilderModalProps> = ({
  isOpen,
  onClose,
  onTypeCreated,
}) => {
  const [displayName, setDisplayName] = useState('');
  const [typeId, setTypeId] = useState('');
  const [selectedIcon, setSelectedIcon] = useState('📦');
  const [selectedColor, setSelectedColor] = useState('#6366F1');
  const [properties, setProperties] = useState<PropertyDefinition[]>([
    {
      key: 'status',
      label: 'Status',
      property_type: 'select',
      options: ['draft', 'active', 'archived'],
      required: false,
    },
  ]);
  const [error, setError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);
  const nameInputRef = useRef<HTMLInputElement>(null);

  useDialogFocus(isOpen, dialogRef, () => {
    if (!isSubmitting) onClose();
  }, { initialFocus: nameInputRef });

  if (!isOpen) return null;

  const handleNameChange = (val: string) => {
    setDisplayName(val);
    const slug = val
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9]+/g, '_')
      .replace(/^_+|_+$/g, '');
    setTypeId(slug);
  };

  const handleAddProperty = () => {
    const newIdx = properties.length + 1;
    setProperties([
      ...properties,
      {
        key: `field_${newIdx}`,
        label: `Field ${newIdx}`,
        property_type: 'text',
        required: false,
      },
    ]);
  };

  const handleRemoveProperty = (idx: number) => {
    setProperties(properties.filter((_, i) => i !== idx));
  };

  const handleUpdateProperty = (idx: number, patch: Partial<PropertyDefinition>) => {
    setProperties(
      properties.map((p, i) => {
        if (i !== idx) return p;
        const updated = { ...p, ...patch };
        // If label changes, suggest key
        if (patch.label && !patch.key) {
          updated.key = patch.label
            .toLowerCase()
            .trim()
            .replace(/[^a-z0-9]+/g, '_');
        }
        return updated;
      })
    );
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    const trimmedName = displayName.trim();
    const trimmedId = typeId.trim();

    if (!trimmedName || !trimmedId) {
      setError('Display name and Type ID are required.');
      return;
    }

    // Validate property keys are unique and non-empty
    const keys = new Set<string>();
    for (const prop of properties) {
      const k = prop.key.trim();
      if (!k) {
        setError('All property keys must be non-empty.');
        return;
      }
      if (keys.has(k)) {
        setError(`Duplicate property key: "${k}". Keys must be unique.`);
        return;
      }
      keys.add(k);

      if (prop.property_type === 'select') {
        const cleanOpts = (prop.options || []).map((o) => o.trim()).filter(Boolean);
        if (cleanOpts.length === 0) {
          setError(`Select property "${prop.label || prop.key}" must have at least one option.`);
          return;
        }
      }
    }

    setIsSubmitting(true);
    try {
      const cleanProperties = properties.map((prop) => {
        if (prop.property_type === 'select') {
          return {
            ...prop,
            options: (prop.options || []).map((o) => o.trim()).filter(Boolean),
          };
        }
        return prop;
      });

      const newType: ObjectType = {
        id: trimmedId,
        display_name: trimmedName,
        icon: selectedIcon,
        color: selectedColor,
        schema: cleanProperties,
        is_system: false,
        created_at: Date.now(),
      };

      await createObjectType(newType);
      onTypeCreated?.(newType);
      onClose?.();
    } catch (err: any) {
      setError(err?.toString() || 'Failed to create object type');
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div
      role="presentation"
      onMouseDown={(e) => { if (e.target === e.currentTarget && !isSubmitting) onClose(); }}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4 animate-in fade-in duration-150"
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="object-type-builder-title"
        data-testid="object-type-builder-modal"
        className="bg-[var(--sthira-surface)] border border-[var(--sthira-border)] rounded-2xl shadow-2xl w-full max-w-xl max-h-[90vh] flex flex-col overflow-hidden text-[var(--sthira-text)]"
      >
        {/* Header */}
        <div className="px-6 py-4 border-b border-[var(--sthira-border)] flex items-center justify-between bg-[var(--sthira-surface-muted)]">
          <div className="flex items-center gap-2.5">
            <div
              className="w-8 h-8 rounded-xl flex items-center justify-center text-lg shadow-2xs"
              style={{ backgroundColor: `${selectedColor}18`, borderColor: selectedColor }}
            >
              <span>{selectedIcon}</span>
            </div>
            <div>
              <h2 id="object-type-builder-title" className="text-base font-bold text-[var(--sthira-text)]">
                New Object Type
              </h2>
              <p className="text-xs text-[var(--sthira-text-muted)]">
                Define a custom entity schema with typed properties
              </p>
            </div>
          </div>
          <button
            type="button"
            aria-label="Close object type builder"
            data-testid="btn-close-object-type-builder"
            disabled={isSubmitting}
            onClick={onClose}
            className="p-1 rounded-lg text-[var(--sthira-text-subtle)] hover:text-[var(--sthira-text)] hover:bg-[var(--sthira-canvas)] transition-colors cursor-pointer disabled:opacity-50"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Form Body */}
        <form onSubmit={handleSubmit} className="flex-1 overflow-y-auto p-6 space-y-5 text-xs">
          {error && (
            <div className="p-3 rounded-xl bg-red-500/10 border border-[var(--sthira-danger)] text-[var(--sthira-danger)] flex items-center gap-2 text-xs">
              <AlertCircle aria-hidden="true" className="w-4 h-4 shrink-0" />
              <p role="alert">{error}</p>
            </div>
          )}

          {/* Type Name & ID */}
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label htmlFor="builder-type-name" className="block text-[11px] font-bold text-[var(--sthira-text-muted)] uppercase tracking-wider mb-1">
                Display Name
              </label>
              <input
                ref={nameInputRef}
                id="builder-type-name"
                type="text"
                required
                data-testid="input-object-type-name"
                value={displayName}
                onChange={(e) => handleNameChange(e.target.value)}
                placeholder="e.g. Bug Report, Book, Contact"
                className="w-full px-3 py-2 rounded-xl border border-[var(--sthira-border-strong)] bg-[var(--sthira-canvas)] text-xs font-medium text-[var(--sthira-text)] placeholder:text-[var(--sthira-text-muted)] focus:bg-[var(--sthira-surface)] focus:border-[var(--sthira-primary)] outline-none transition-all"
              />
            </div>
            <div>
              <label htmlFor="builder-type-id" className="block text-[11px] font-bold text-[var(--sthira-text-muted)] uppercase tracking-wider mb-1">
                Type Identifier
              </label>
              <input
                id="builder-type-id"
                type="text"
                required
                data-testid="input-object-type-id"
                value={typeId}
                onChange={(e) => setTypeId(e.target.value)}
                placeholder="e.g. bug_report"
                className="w-full px-3 py-2 rounded-xl border border-[var(--sthira-border-strong)] bg-[var(--sthira-canvas)] text-xs font-mono text-[var(--sthira-text)] placeholder:text-[var(--sthira-text-muted)] focus:bg-[var(--sthira-surface)] focus:border-[var(--sthira-primary)] outline-none transition-all"
              />
            </div>
          </div>

          {/* Icon & Color Selector */}
          <div className="grid grid-cols-2 gap-4">
            <div>
              <span className="block text-[11px] font-bold text-[var(--sthira-text-muted)] uppercase tracking-wider mb-1.5">
                Icon
              </span>
              <div className="flex flex-wrap gap-1.5 p-2 rounded-xl border border-[var(--sthira-border)] bg-[var(--sthira-canvas)] max-h-24 overflow-y-auto">
                {EMOJI_OPTIONS.map((emoji) => (
                  <button
                    key={emoji}
                    type="button"
                    aria-label={`Select icon ${emoji}`}
                    aria-pressed={selectedIcon === emoji}
                    onClick={() => setSelectedIcon(emoji)}
                    className={`w-7 h-7 rounded-lg flex items-center justify-center text-sm transition-all cursor-pointer ${
                      selectedIcon === emoji
                        ? 'bg-[var(--sthira-surface)] shadow-xs border border-[var(--sthira-primary)] scale-105'
                        : 'hover:bg-[var(--sthira-surface-muted)]'
                    }`}
                  >
                    {emoji}
                  </button>
                ))}
              </div>
            </div>

            <div>
              <span className="block text-[11px] font-bold text-[var(--sthira-text-muted)] uppercase tracking-wider mb-1.5">
                Accent Color
              </span>
              <div className="flex flex-wrap gap-2 p-2.5 rounded-xl border border-[var(--sthira-border)] bg-[var(--sthira-canvas)]">
                {COLOR_OPTIONS.map((color) => (
                  <button
                    key={color}
                    type="button"
                    aria-label={`Select color ${color}`}
                    aria-pressed={selectedColor === color}
                    onClick={() => setSelectedColor(color)}
                    style={{ backgroundColor: color }}
                    className={`w-6 h-6 rounded-full transition-transform cursor-pointer ${
                      selectedColor === color
                        ? 'ring-2 ring-[var(--sthira-primary)] ring-offset-2 ring-offset-[var(--sthira-canvas)] scale-110'
                        : 'hover:opacity-80'
                    }`}
                  />
                ))}
              </div>
            </div>
          </div>

          {/* Property Schema Builder */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <span className="block text-[11px] font-bold text-[var(--sthira-text-muted)] uppercase tracking-wider">
                Custom Properties ({properties.length})
              </span>
              <button
                type="button"
                data-testid="btn-add-property"
                onClick={handleAddProperty}
                className="inline-flex items-center gap-1 text-[11px] font-medium text-[var(--sthira-text)] hover:bg-[var(--sthira-surface-muted)] px-2 py-1 rounded-lg transition-colors cursor-pointer"
              >
                <Plus className="w-3.5 h-3.5" />
                <span>Add Property</span>
              </button>
            </div>

            <div className="space-y-2 max-h-80 overflow-y-auto p-1">
              {properties.length === 0 ? (
                <div className="text-center py-4 text-xs text-[var(--sthira-text-subtle)] italic border border-dashed border-[var(--sthira-border)] rounded-xl">
                  No properties added yet. Click &quot;Add Property&quot; above.
                </div>
              ) : (
                properties.map((prop, idx) => (
                  <div
                    key={idx}
                    className="p-2.5 rounded-xl border border-[var(--sthira-border)] bg-[var(--sthira-canvas)] space-y-2"
                  >
                    <div className="flex items-center gap-2">
                      <input
                        type="text"
                        aria-label={`Property ${idx + 1} label`}
                        data-testid="input-property-label"
                        value={prop.label}
                        onChange={(e) => handleUpdateProperty(idx, { label: e.target.value })}
                        placeholder="Label (e.g. Priority)"
                        className="flex-1 px-2 py-1 rounded-lg border border-[var(--sthira-border-strong)] bg-[var(--sthira-surface)] text-xs font-medium text-[var(--sthira-text)] placeholder:text-[var(--sthira-text-muted)] outline-none focus:border-[var(--sthira-primary)]"
                      />
                      <input
                        type="text"
                        aria-label={`Property ${idx + 1} key`}
                        data-testid="input-property-key"
                        value={prop.key}
                        onChange={(e) => handleUpdateProperty(idx, { key: e.target.value })}
                        placeholder="key"
                        className="w-28 px-2 py-1 rounded-lg border border-[var(--sthira-border-strong)] bg-[var(--sthira-surface)] text-xs font-mono text-[var(--sthira-text)] placeholder:text-[var(--sthira-text-muted)] outline-none focus:border-[var(--sthira-primary)]"
                      />
                      <select
                        aria-label={`Property ${idx + 1} type`}
                        value={prop.property_type}
                        data-testid="select-property-type"
                        style={{ colorScheme: 'inherit' }}
                        onChange={(e) => {
                          const newType = e.target.value as PropertyType;
                          handleUpdateProperty(idx, {
                            property_type: newType,
                            options:
                              newType === 'select'
                                ? prop.options && prop.options.length > 0
                                  ? prop.options
                                  : ['Option 1', 'Option 2']
                                : prop.options,
                          });
                        }}
                        className="w-28 px-2 py-1 rounded-lg border border-[var(--sthira-border-strong)] bg-[var(--sthira-surface)] text-[var(--sthira-text)] text-xs font-medium outline-none cursor-pointer focus:border-[var(--sthira-primary)]"
                      >
                        <option className="bg-[var(--sthira-surface)] text-[var(--sthira-text)]" value="text">Text</option>
                        <option className="bg-[var(--sthira-surface)] text-[var(--sthira-text)]" value="number">Number</option>
                        <option className="bg-[var(--sthira-surface)] text-[var(--sthira-text)]" value="boolean">Checkbox</option>
                        <option className="bg-[var(--sthira-surface)] text-[var(--sthira-text)]" value="select">Select</option>
                        <option className="bg-[var(--sthira-surface)] text-[var(--sthira-text)]" value="date">Date</option>
                        <option className="bg-[var(--sthira-surface)] text-[var(--sthira-text)]" value="url">URL</option>
                      </select>
                      <button
                        type="button"
                        aria-label={`Remove property ${idx + 1}`}
                        onClick={() => handleRemoveProperty(idx)}
                        className="p-1 text-[var(--sthira-text-subtle)] hover:text-[var(--sthira-danger)] rounded hover:bg-[var(--sthira-surface-muted)] transition-colors cursor-pointer"
                        title="Remove property"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>

                    {prop.property_type === 'select' && (
                      <SelectPropertyOptionsEditor
                        propertyIndex={idx}
                        options={prop.options || []}
                        onChange={(newOptions) =>
                          handleUpdateProperty(idx, { options: newOptions })
                        }
                      />
                    )}
                  </div>
                ))
              )}
            </div>
          </div>

          {/* Footer Buttons */}
          <div className="pt-2 border-t border-[var(--sthira-border)] flex items-center justify-end gap-2.5">
            <button
              type="button"
              disabled={isSubmitting}
              onClick={onClose}
              className="sthira-button-secondary text-xs cursor-pointer"
            >
              Cancel
            </button>
            <button
              type="submit"
              data-testid="btn-submit-object-type"
              disabled={isSubmitting}
              className="sthira-button-primary text-xs cursor-pointer shadow-xs"
            >
              {isSubmitting ? 'Creating...' : 'Create Object Type'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};

export default ObjectTypeBuilderModal;
