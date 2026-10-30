import React, { useState, useEffect, useCallback } from 'react';
import { ChevronDown, ChevronUp, Loader2, Check, Layers } from 'lucide-react';
import { PropertyFieldRenderer } from './PropertyFieldRenderer';
import { saveObjectInstance } from '../../api/ipc';
import type { ObjectType, ObjectInstance, PropertyDefinition } from '../../types/sthira';

interface DocumentPropertyHeaderProps {
  artifactId: string;
  objectType: ObjectType;
  instance: ObjectInstance;
  onInstanceUpdated?: (instance: ObjectInstance) => void;
}

/**
 * Phase 16: Notion-style interactive property tray rendered directly below the page
 * title in `FocusView` whenever the open artifact is an `object_instance`.
 *
 * Every chip is inline-editable and persists immediately in the background via
 * `cmd_save_object_instance` (`saveObjectInstance`), with a quiet save indicator.
 */
export const DocumentPropertyHeader: React.FC<DocumentPropertyHeaderProps> = ({
  artifactId,
  objectType,
  instance,
  onInstanceUpdated,
}) => {
  const [properties, setProperties] = useState<Record<string, any>>(instance.properties || {});
  const [isCollapsed, setIsCollapsed] = useState(false);
  const [saveState, setSaveState] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle');

  useEffect(() => {
    setProperties(instance.properties || {});
  }, [instance]);

  const schema: PropertyDefinition[] =
    objectType.schema || (objectType as any).properties || [];

  const persist = useCallback(
    async (next: Record<string, any>) => {
      const updated: ObjectInstance = {
        ...instance,
        artifact_id: artifactId || instance.artifact_id,
        object_type_id: objectType.id,
        properties: next,
        updated_at: Date.now(),
      };
      onInstanceUpdated?.(updated);

      try {
        setSaveState('saving');
        await saveObjectInstance(updated);
        setSaveState('saved');
        window.dispatchEvent(new CustomEvent('sthira:refresh_objects'));
        setTimeout(() => setSaveState((prev) => (prev === 'saved' ? 'idle' : prev)), 1500);
      } catch (err) {
        console.error('Failed to save object instance property', err);
        setSaveState('error');
      }
    },
    [artifactId, instance, objectType.id, onInstanceUpdated],
  );

  const handleChange = (key: string, value: any) => {
    const next = { ...properties, [key]: value };
    setProperties(next);
    persist(next);
  };

  // Ad-hoc properties with no formal schema still render as read-only chips.
  const adHocEntries = Object.entries(properties).filter(
    ([k]) => !schema.some((p) => p.key === k),
  );


  return (
    <div
      data-testid="document-property-header"
      data-object-type={objectType.id}
      className="mb-4 rounded-xl border border-[#E8E5DC] dark:border-[#2C2E27] bg-[#FAF9F6] dark:bg-[#181915] px-3.5 py-2.5 text-xs"
    >
      {/* Header Row: Type Badge + Collapse Toggle + Save Indicator */}
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          <span
            data-testid="property-header-type-badge"
            className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md font-semibold text-[11px] border shrink-0"
            style={{
              backgroundColor: `${objectType.color || '#6366F1'}18`,
              borderColor: `${objectType.color || '#6366F1'}40`,
              color: objectType.color || '#6366F1',
            }}
          >
            <span>{objectType.icon || '📄'}</span>
            <span>{objectType.display_name}</span>
          </span>

          {objectType.description && (
            <span className="truncate text-[11px] text-[#8C8A82] dark:text-[#6E6D66]">
              {objectType.description}
            </span>
          )}
        </div>

        <div className="flex items-center gap-2 shrink-0">
          {saveState === 'saving' && (
            <span
              data-testid="property-save-indicator"
              className="inline-flex items-center gap-1 text-[10px] text-[#8C8A82] dark:text-[#6E6D66]"
            >
              <Loader2 className="w-3 h-3 animate-spin" />
              saving
            </span>
          )}
          {saveState === 'saved' && (
            <span className="inline-flex items-center gap-1 text-[10px] text-emerald-700 dark:text-emerald-400 font-semibold">
              <Check className="w-3 h-3" />
              saved
            </span>
          )}
          {saveState === 'error' && (
            <span className="text-[10px] text-red-600 dark:text-red-400">save failed</span>
          )}

          <button
            type="button"
            data-testid="btn-toggle-property-tray"
            onClick={() => setIsCollapsed((prev) => !prev)}
            className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded-md text-[10px] text-[#6E6D66] dark:text-[#9E9C93] hover:bg-[#EFECE4] dark:hover:bg-[#252820] transition-colors cursor-pointer"
            title={isCollapsed ? 'Show properties' : 'Hide properties'}
          >
            <Layers className="w-3 h-3" />
            {isCollapsed ? <ChevronDown className="w-3 h-3" /> : <ChevronUp className="w-3 h-3" />}
          </button>
        </div>
      </div>

      {/* Interactive Property Chips */}
      {!isCollapsed && (
        <div
          data-testid="property-chip-tray"
          className="mt-2.5 pt-2.5 border-t border-[#ECE9E0] dark:border-[#262822] flex flex-wrap items-center gap-2"
        >
          {schema.length === 0 && adHocEntries.length === 0 && (
            <span className="text-[11px] text-[#A09D94] dark:text-[#6E6D66] italic">
              This object type has no properties defined yet.
            </span>
          )}

          {schema.map((prop) => (
            <div
              key={prop.key}
              data-testid={`property-chip-${prop.key}`}
              className="inline-flex items-center gap-1.5 px-2 py-1 rounded-lg bg-white dark:bg-[#1F201B] border border-[#E0DCD3] dark:border-[#2C2E27]"
            >
              <span className="text-[10px] font-semibold text-[#6E6D66] dark:text-[#9E9C93] uppercase tracking-wider">
                {prop.label || prop.key}
                {prop.required && <span className="text-red-500 ml-0.5">*</span>}
              </span>
              <PropertyFieldRenderer
                definition={prop}
                value={properties[prop.key]}
                compact
                onChange={(val) => handleChange(prop.key, val)}
              />
            </div>
          ))}

          {adHocEntries.map(([k, v]) => (
            <span
              key={k}
              className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md bg-white dark:bg-[#1F201B] border border-[#E0DCD3] dark:border-[#2C2E27] text-[11px]"
            >
              <span className="font-semibold text-[#6E6D66] dark:text-[#9E9C93]">{k}:</span>
              <span className="text-[#1E201C] dark:text-[#EDEAE2]">{String(v)}</span>
            </span>
          ))}
        </div>
      )}
    </div>
  );
};

export default DocumentPropertyHeader;
