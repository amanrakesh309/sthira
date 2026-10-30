import React, { useState } from 'react';
import { X, Plus, Trash2, AlertCircle, ChevronUp, ChevronDown } from 'lucide-react';
import { createObjectType } from '../../api/ipc';
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
    <div className="mt-2.5 pt-2 border-t border-[#E8E5DC] space-y-2">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5 text-[11px] font-semibold text-[#6E6D66]">
          <span className="w-1.5 h-1.5 rounded-full bg-[#8C8A82]" />
          <span>Dropdown Options ({options.length})</span>
        </div>
        <span className="text-[10px] text-[#8C8A82]">
          Press Enter or comma to add
        </span>
      </div>

      {/* Bulleted list of options */}
      <div className="space-y-1.5">
        {options.length === 0 ? (
          <div className="text-[11px] text-[#8C8A82] italic py-1 pl-2">
            No options added yet. Type below and press Enter to add.
          </div>
        ) : (
          options.map((opt, optIdx) => (
            <div
              key={optIdx}
              className="flex items-center gap-1.5 group bg-white px-2 py-1 rounded-lg border border-[#E8E5DC] hover:border-[#D1CDC2] transition-colors"
            >
              <span className="text-[#8C8A82] select-none text-xs font-bold shrink-0">•</span>
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
                className="flex-1 bg-transparent text-xs font-medium text-[#1E201C] outline-none"
              />
              <div className="flex items-center gap-0.5 opacity-60 group-hover:opacity-100 transition-opacity">
                <button
                  type="button"
                  disabled={optIdx === 0}
                  onClick={() => handleMoveOption(optIdx, -1)}
                  data-testid={`btn-move-up-option-${propertyIndex}-${optIdx}`}
                  className="p-0.5 text-[#8C8A82] hover:text-[#1E201C] disabled:opacity-20 cursor-pointer disabled:cursor-not-allowed rounded"
                  title="Move Up"
                >
                  <ChevronUp className="w-3 h-3" />
                </button>
                <button
                  type="button"
                  disabled={optIdx === options.length - 1}
                  onClick={() => handleMoveOption(optIdx, 1)}
                  data-testid={`btn-move-down-option-${propertyIndex}-${optIdx}`}
                  className="p-0.5 text-[#8C8A82] hover:text-[#1E201C] disabled:opacity-20 cursor-pointer disabled:cursor-not-allowed rounded"
                  title="Move Down"
                >
                  <ChevronDown className="w-3 h-3" />
                </button>
                <button
                  type="button"
                  onClick={() => handleRemoveOption(optIdx)}
                  data-testid={`btn-remove-option-${propertyIndex}-${optIdx}`}
                  className="p-0.5 text-[#8C8A82] hover:text-red-600 rounded transition-colors cursor-pointer"
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
          <div className="flex-1 flex items-center gap-1.5 bg-white px-2 py-1 rounded-lg border border-dashed border-[#D1CDC2] focus-within:border-[#282C25] focus-within:border-solid transition-all">
            <Plus className="w-3 h-3 text-[#8C8A82] shrink-0" />
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
              className="flex-1 bg-transparent text-xs text-[#1E201C] placeholder:text-[#8C8A82] outline-none"
            />
          </div>
          <button
            type="button"
            onClick={handleAddOption}
            data-testid={`btn-add-option-${propertyIndex}`}
            className="px-2.5 py-1 rounded-lg bg-[#FAF9F5] hover:bg-[#EFECE4] border border-[#E8E5DC] text-[#282C25] text-xs font-semibold flex items-center gap-1 transition-colors cursor-pointer shrink-0"
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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4 animate-in fade-in duration-150">
      <div className="bg-[#FFFFFF] border border-[#E8E5DC] rounded-2xl shadow-2xl w-full max-w-xl max-h-[90vh] flex flex-col overflow-hidden text-[#1E201C]">
        {/* Header */}
        <div className="px-6 py-4 border-b border-[#E8E5DC] flex items-center justify-between bg-[#F5F3EC]/50">
          <div className="flex items-center gap-2.5">
            <div
              className="w-8 h-8 rounded-xl flex items-center justify-center text-lg shadow-2xs"
              style={{ backgroundColor: `${selectedColor}18`, borderColor: selectedColor }}
            >
              <span>{selectedIcon}</span>
            </div>
            <div>
              <h2 className="text-base font-bold text-[#1E201C]">New Object Type</h2>
              <p className="text-xs text-[#6E6D66]">
                Define a custom entity schema with typed properties
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded-lg text-[#8C8A82] hover:text-[#1E201C] hover:bg-[#EFECE4] transition-colors cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Form Body */}
        <form onSubmit={handleSubmit} className="flex-1 overflow-y-auto p-6 space-y-5 text-xs">
          {error && (
            <div className="p-3 rounded-xl bg-red-50 border border-red-200 text-red-700 flex items-center gap-2 text-xs">
              <AlertCircle className="w-4 h-4 shrink-0" />
              <span>{error}</span>
            </div>
          )}

          {/* Type Name & ID */}
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-[11px] font-bold text-[#6E6D66] uppercase tracking-wider mb-1">
                Display Name
              </label>
              <input
                type="text"
                required
                data-testid="input-object-type-name"
                value={displayName}
                onChange={(e) => handleNameChange(e.target.value)}
                placeholder="e.g. Bug Report, Book, Contact"
                className="w-full px-3 py-2 rounded-xl border border-[#E8E5DC] bg-[#FAF9F5] text-xs font-medium focus:bg-white focus:border-[#282C25] outline-none transition-all"
              />
            </div>
            <div>
              <label className="block text-[11px] font-bold text-[#6E6D66] uppercase tracking-wider mb-1">
                Type Identifier
              </label>
              <input
                type="text"
                required
                data-testid="input-object-type-id"
                value={typeId}
                onChange={(e) => setTypeId(e.target.value)}
                placeholder="e.g. bug_report"
                className="w-full px-3 py-2 rounded-xl border border-[#E8E5DC] bg-[#FAF9F5] text-xs font-mono focus:bg-white focus:border-[#282C25] outline-none transition-all"
              />
            </div>
          </div>

          {/* Icon & Color Selector */}
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-[11px] font-bold text-[#6E6D66] uppercase tracking-wider mb-1.5">
                Icon
              </label>
              <div className="flex flex-wrap gap-1.5 p-2 rounded-xl border border-[#E8E5DC] bg-[#FAF9F5] max-h-24 overflow-y-auto">
                {EMOJI_OPTIONS.map((emoji) => (
                  <button
                    key={emoji}
                    type="button"
                    onClick={() => setSelectedIcon(emoji)}
                    className={`w-7 h-7 rounded-lg flex items-center justify-center text-sm transition-all cursor-pointer ${
                      selectedIcon === emoji
                        ? 'bg-white shadow-xs border border-[#282C25] scale-105'
                        : 'hover:bg-[#EFECE4]'
                    }`}
                  >
                    {emoji}
                  </button>
                ))}
              </div>
            </div>

            <div>
              <label className="block text-[11px] font-bold text-[#6E6D66] uppercase tracking-wider mb-1.5">
                Accent Color
              </label>
              <div className="flex flex-wrap gap-2 p-2.5 rounded-xl border border-[#E8E5DC] bg-[#FAF9F5]">
                {COLOR_OPTIONS.map((color) => (
                  <button
                    key={color}
                    type="button"
                    onClick={() => setSelectedColor(color)}
                    style={{ backgroundColor: color }}
                    className={`w-6 h-6 rounded-full transition-transform cursor-pointer ${
                      selectedColor === color
                        ? 'ring-2 ring-[#282C25] ring-offset-2 scale-110'
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
              <label className="block text-[11px] font-bold text-[#6E6D66] uppercase tracking-wider">
                Custom Properties ({properties.length})
              </label>
              <button
                type="button"
                data-testid="btn-add-property"
                onClick={handleAddProperty}
                className="inline-flex items-center gap-1 text-[11px] font-medium text-[#282C25] hover:bg-[#EFECE4] px-2 py-1 rounded-lg transition-colors cursor-pointer"
              >
                <Plus className="w-3.5 h-3.5" />
                <span>Add Property</span>
              </button>
            </div>

            <div className="space-y-2 max-h-80 overflow-y-auto p-1">
              {properties.length === 0 ? (
                <div className="text-center py-4 text-xs text-[#8C8A82] italic border border-dashed border-[#E8E5DC] rounded-xl">
                  No properties added yet. Click &quot;Add Property&quot; above.
                </div>
              ) : (
                properties.map((prop, idx) => (
                  <div
                    key={idx}
                    className="p-2.5 rounded-xl border border-[#E8E5DC] bg-[#FAF9F5] space-y-2"
                  >
                    <div className="flex items-center gap-2">
                      <input
                        type="text"
                        data-testid="input-property-label"
                        value={prop.label}
                        onChange={(e) => handleUpdateProperty(idx, { label: e.target.value })}
                        placeholder="Label (e.g. Priority)"
                        className="flex-1 px-2 py-1 rounded-lg border border-[#E8E5DC] bg-white text-xs font-medium outline-none focus:border-[#282C25]"
                      />
                      <input
                        type="text"
                        data-testid="input-property-key"
                        value={prop.key}
                        onChange={(e) => handleUpdateProperty(idx, { key: e.target.value })}
                        placeholder="key"
                        className="w-28 px-2 py-1 rounded-lg border border-[#E8E5DC] bg-white text-xs font-mono outline-none focus:border-[#282C25]"
                      />
                      <select
                        value={prop.property_type}
                        data-testid="select-property-type"
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
                        className="w-28 px-2 py-1 rounded-lg border border-[#E8E5DC] bg-white text-xs font-medium outline-none cursor-pointer focus:border-[#282C25]"
                      >
                        <option value="text">Text</option>
                        <option value="number">Number</option>
                        <option value="boolean">Checkbox</option>
                        <option value="select">Select</option>
                        <option value="date">Date</option>
                        <option value="url">URL</option>
                      </select>
                      <button
                        type="button"
                        onClick={() => handleRemoveProperty(idx)}
                        className="p-1 text-[#8C8A82] hover:text-red-600 rounded hover:bg-red-50 transition-colors cursor-pointer"
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
          <div className="pt-2 border-t border-[#E8E5DC] flex items-center justify-end gap-2.5">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 rounded-xl text-xs font-medium text-[#6E6D66] hover:bg-[#EFECE4] transition-colors cursor-pointer"
            >
              Cancel
            </button>
            <button
              type="submit"
              data-testid="btn-submit-object-type"
              disabled={isSubmitting}
              className="px-5 py-2 rounded-xl text-xs font-semibold bg-[#282C25] text-white hover:opacity-90 transition-opacity cursor-pointer shadow-xs disabled:opacity-50"
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
