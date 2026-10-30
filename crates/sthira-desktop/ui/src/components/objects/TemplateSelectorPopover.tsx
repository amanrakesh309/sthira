import React, { useState, useEffect, useRef } from 'react';
import { X, FileText, Square, Sparkles } from 'lucide-react';
import { getTypeTemplates } from '../../api/ipc';
import type { ObjectType, ObjectTemplate } from '../../types/sthira';

interface TemplateSelectorPopoverProps {
  objectType: ObjectType;
  position?: { top: number; left: number } | null;
  /** Receives the chosen template id, or `null` for a blank (title-only) body. */
  onSelect: (templateId: string | null) => void;
  onClose: () => void;
}

/**
 * Phase 16: Clean template selection popover shown when a typed object is created and
 * more than one template exists for its type.
 *
 * Always offers `[Default Template]`, `[Blank]`, and every named custom preset.
 */
export const TemplateSelectorPopover: React.FC<TemplateSelectorPopoverProps> = ({
  objectType,
  position,
  onSelect,
  onClose,
}) => {
  const [templates, setTemplates] = useState<ObjectTemplate[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const list = await getTypeTemplates(objectType.id);
        if (!cancelled) setTemplates(list);
      } catch (err) {
        console.error('Failed to load templates for selector', err);
        if (!cancelled) setTemplates([]);
      } finally {
        if (!cancelled) setIsLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [objectType.id]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

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

  const defaultTemplate = templates.find((t) => t.is_default) || templates[0] || null;
  const customTemplates = templates.filter((t) => !defaultTemplate || t.id !== defaultTemplate.id);

  return (
    <div
      ref={containerRef}
      id="template-selector-popover"
      data-testid="template-selector-popover"
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
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-base shrink-0">{objectType.icon || '📄'}</span>
          <div className="min-w-0">
            <div className="font-bold text-xs truncate">New {objectType.display_name}</div>
            <div className="text-[10px] text-[#8C8A82] dark:text-[#6E6D66]">
              Choose a starting template
            </div>
          </div>
        </div>
        <button
          type="button"
          data-testid="btn-close-template-selector"
          onClick={onClose}
          className="p-1 rounded-md text-[#8C8A82] dark:text-[#6E6D66] hover:bg-[#EFECE4] dark:hover:bg-[#252820] transition-colors cursor-pointer"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Template Options */}
      <div className="mt-3 space-y-1.5">
        {isLoading ? (
          <div className="text-[11px] text-[#8C8A82] dark:text-[#6E6D66] italic py-2">
            Loading templates...
          </div>
        ) : (
          <>
            {defaultTemplate && (
              <button
                type="button"
                data-testid="template-option-default"
                onClick={() => onSelect(defaultTemplate.id)}
                className="w-full text-left px-2.5 py-2 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] hover:bg-[#FAF8F3] dark:hover:bg-[#22241F] transition-colors cursor-pointer flex items-start gap-2"
              >
                <FileText className="w-3.5 h-3.5 mt-0.5 text-[#282C25] dark:text-[#EDEAE2] shrink-0" />
                <span className="min-w-0">
                  <span className="flex items-center gap-1.5">
                    <span className="font-semibold text-xs truncate">{defaultTemplate.name}</span>
                    <span className="text-[9px] px-1 py-0.2 rounded bg-[#EFECE4] dark:bg-[#252820] text-[#6E6D66] dark:text-[#9E9C93] font-semibold uppercase">
                      default
                    </span>
                  </span>
                  <span className="block text-[10px] text-[#8C8A82] dark:text-[#6E6D66] truncate font-mono mt-0.5">
                    {(defaultTemplate.body_template_md || '(empty body)').slice(0, 60)}
                  </span>
                </span>
              </button>
            )}

            <button
              type="button"
              data-testid="template-option-blank"
              onClick={() => onSelect(null)}
              className="w-full text-left px-2.5 py-2 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] hover:bg-[#FAF8F3] dark:hover:bg-[#22241F] transition-colors cursor-pointer flex items-start gap-2"
            >
              <Square className="w-3.5 h-3.5 mt-0.5 text-[#8C8A82] shrink-0" />
              <span className="min-w-0">
                <span className="font-semibold text-xs block">Blank</span>
                <span className="block text-[10px] text-[#8C8A82] dark:text-[#6E6D66]">
                  Start with an empty title-only page.
                </span>
              </span>
            </button>

            {customTemplates.map((t) => (
              <button
                key={t.id}
                type="button"
                data-testid={`template-option-${t.id}`}
                onClick={() => onSelect(t.id)}
                className="w-full text-left px-2.5 py-2 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] hover:bg-[#FAF8F3] dark:hover:bg-[#22241F] transition-colors cursor-pointer flex items-start gap-2"
              >
                <span className="mt-0.5 shrink-0">{t.icon || '📄'}</span>
                <span className="min-w-0">
                  <span className="font-semibold text-xs block truncate">{t.name}</span>
                  <span className="block text-[10px] text-[#8C8A82] dark:text-[#6E6D66] truncate font-mono">
                    {(t.body_template_md || '(empty body)').slice(0, 60)}
                  </span>
                </span>
              </button>
            ))}

            {!defaultTemplate && (
              <div className="inline-flex items-center gap-1.5 text-[10px] text-[#A09D94] dark:text-[#6E6D66] px-1 pt-1">
                <Sparkles className="w-3 h-3" />
                No named templates yet — a bare skeleton will be used.
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
};

export default TemplateSelectorPopover;

