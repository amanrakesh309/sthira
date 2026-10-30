import React, { useState, useEffect, useRef } from 'react';
import { X, Check } from 'lucide-react';
import { PropertyFieldRenderer } from './PropertyFieldRenderer';
import type { ObjectType, PropertyDefinition } from '../../types/sthira';

interface QuickObjectCreatorCardProps {
  objectType: ObjectType;
  position?: { top: number; left: number } | null;
  onSubmit: (title: string, properties: Record<string, any>) => void;
  onClose: () => void;
}

export const QuickObjectCreatorCard: React.FC<QuickObjectCreatorCardProps> = ({
  objectType,
  position,
  onSubmit,
  onClose,
}) => {
  const [title, setTitle] = useState('');
  const [properties, setProperties] = useState<Record<string, any>>({});
  const containerRef = useRef<HTMLDivElement>(null);
  const titleInputRef = useRef<HTMLInputElement>(null);

  // Initialize default properties from schema
  useEffect(() => {
    const initialProps: Record<string, any> = {};
    const schema: PropertyDefinition[] = objectType.schema || (objectType as any).properties || [];
    schema.forEach((prop) => {
      if (prop.default_value !== undefined) {
        initialProps[prop.key] = prop.default_value;
      } else if (prop.property_type === 'boolean') {
        initialProps[prop.key] = false;
      } else if (prop.property_type === 'select' && prop.options && prop.options.length > 0) {
        initialProps[prop.key] = prop.options[0];
      } else {
        initialProps[prop.key] = '';
      }
    });
    setProperties(initialProps);

    setTimeout(() => {
      titleInputRef.current?.focus();
    }, 50);
  }, [objectType]);

  const handleSubmit = (e?: React.FormEvent) => {
    e?.preventDefault();
    const trimmedTitle = title.trim();
    if (!trimmedTitle) {
      titleInputRef.current?.focus();
      return;
    }
    onSubmit(trimmedTitle, properties);
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
        e.preventDefault();
        handleSubmit();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [title, properties]);

  // Click outside to close
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [onClose]);

  const schema: PropertyDefinition[] = objectType.schema || (objectType as any).properties || [];

  return (
    <div
      ref={containerRef}
      id="quick-object-creator-card"
      data-testid="quick-object-creator-card"
      style={
        position
          ? {
              top: `${position.top}px`,
              left: `${Math.min(position.left, window.innerWidth - 380)}px`,
            }
          : undefined
      }
      className="absolute z-50 w-88 bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl shadow-2xl p-4 text-xs font-sans text-[#1E201C] dark:text-[#EDEAE2] animate-in fade-in zoom-in-95 duration-100"
    >
      {/* Header */}
      <div className="flex items-center justify-between pb-3 border-b border-[#ECE9E0] dark:border-[#2C2E27]">
        <div className="flex items-center gap-2">
          <div
            className="w-6 h-6 rounded-md flex items-center justify-center text-sm shadow-2xs"
            style={{ backgroundColor: `${objectType.color || '#F59E0B'}22`, color: objectType.color || '#F59E0B' }}
          >
            {objectType.icon || '📦'}
          </div>
          <div>
            <span className="font-bold text-xs">New {objectType.display_name}</span>
            <span className="ml-1.5 text-[10px] text-[#8C8A82] dark:text-[#6E6D66] font-mono">
              typed object
            </span>
          </div>
        </div>
        <button
          type="button"
          onClick={onClose}
          className="p-1 rounded-md text-[#8C8A82] dark:text-[#6E6D66] hover:bg-[#EFECE4] dark:hover:bg-[#252820] transition-colors cursor-pointer"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Form */}
      <form onSubmit={handleSubmit} className="mt-3 space-y-3">
        {/* Title Input */}
        <div>
          <label className="block text-[11px] font-semibold text-[#5A5852] dark:text-[#9E9C93] mb-1">
            Title <span className="text-red-500">*</span>
          </label>
          <input
            ref={titleInputRef}
            data-testid="input-object-title"
            type="text"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={`Enter ${objectType.display_name} title...`}
            className="w-full px-2.5 py-1.5 rounded-lg bg-[#FAF8F3] dark:bg-[#161714] border border-[#E0DCD3] dark:border-[#2C2E27] text-xs text-[#1E201C] dark:text-[#EDEAE2] outline-none focus:border-[#282C25] dark:focus:border-[#EDEAE2] transition-colors"
          />
        </div>

        {/* Schema Properties */}
        {schema.length > 0 && (
          <div className="space-y-2.5 pt-1">
            {schema.map((prop) => (
              <div key={prop.key} className="flex flex-col gap-1">
                <span className="text-[11px] font-medium text-[#5A5852] dark:text-[#9E9C93]">
                  {prop.label || prop.key}
                  {prop.required && <span className="text-red-500 ml-0.5">*</span>}
                </span>
                <div>
                  <PropertyFieldRenderer
                    definition={prop}
                    value={properties[prop.key]}
                    onChange={(val) =>
                      setProperties((prev) => ({
                        ...prev,
                        [prop.key]: val,
                      }))
                    }
                  />
                </div>
              </div>
            ))}
          </div>
        )}

        {/* Actions */}
        <div className="flex items-center justify-between pt-2 border-t border-[#ECE9E0] dark:border-[#2C2E27]">
          <span className="text-[10px] text-[#A09D94] dark:text-[#6E6D66]">
            Press <kbd className="px-1 py-0.5 rounded bg-[#EFECE4] dark:bg-[#252820] font-mono">⌘↵</kbd> to insert
          </span>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={onClose}
              className="px-2.5 py-1 rounded-lg text-xs text-[#6E6D66] dark:text-[#9E9C93] hover:bg-[#EFECE4] dark:hover:bg-[#252820] transition-colors cursor-pointer"
            >
              Cancel
            </button>
            <button
              type="submit"
              data-testid="btn-create-insert-object"
              className="inline-flex items-center gap-1.5 px-3 py-1 rounded-lg bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] font-semibold text-xs hover:bg-[#3D4238] dark:hover:bg-[#D4D1C7] transition-colors shadow-2xs cursor-pointer"
            >
              <Check className="w-3.5 h-3.5" />
              <span>Create & Insert</span>
            </button>
          </div>
        </div>
      </form>
    </div>
  );
};
