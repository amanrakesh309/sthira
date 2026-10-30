import React, { useState } from 'react';
import { PropertyFieldRenderer } from './PropertyFieldRenderer';
import { saveObjectInstance } from '../../api/ipc';
import type { ObjectType, PropertyDefinition } from '../../types/sthira';

export interface EmbeddedObjectData {
  artifact_id: string;
  object_type_id: string;
  title: string;
  properties: Record<string, any>;
}

interface EmbeddedObjectCardProps {
  data: EmbeddedObjectData;
  objectType?: ObjectType;
  onUpdate?: (updatedData: EmbeddedObjectData) => void;
}

export const EmbeddedObjectCard: React.FC<EmbeddedObjectCardProps> = ({
  data,
  objectType,
  onUpdate,
}) => {
  const [properties, setProperties] = useState<Record<string, any>>(data.properties || {});
  const [isSaving, setIsSaving] = useState(false);

  const schema: PropertyDefinition[] = objectType?.schema || (objectType as any)?.properties || [];

  const handlePropertyChange = async (key: string, val: any) => {
    const updated = {
      ...properties,
      [key]: val,
    };
    setProperties(updated);

    const updatedData: EmbeddedObjectData = {
      ...data,
      properties: updated,
    };

    onUpdate?.(updatedData);

    // Persist immediately to SQLite via cmd_save_object_instance
    try {
      setIsSaving(true);
      await saveObjectInstance({
        artifact_id: data.artifact_id,
        object_type_id: data.object_type_id,
        properties: updated,
        updated_at: Date.now(),
      });
      window.dispatchEvent(new CustomEvent('sthira:refresh_objects'));
    } catch (err) {
      console.error('Failed to save object instance', err);
    } finally {
      setIsSaving(false);
    }
  };

  const displayName = objectType?.display_name || data.object_type_id.charAt(0).toUpperCase() + data.object_type_id.slice(1);
  const icon = objectType?.icon || '📦';
  const color = objectType?.color || '#F59E0B';

  return (
    <div
      data-testid={`embedded-object-${data.artifact_id}`}
      className="my-3 p-3.5 rounded-xl bg-[#FAF9F6] dark:bg-[#181915] border border-[#E8E5DC] dark:border-[#2C2E27] text-xs font-sans shadow-2xs space-y-2.5 transition-all"
    >
      {/* Top Banner: Object Type Badge + Title */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <span
            className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md font-semibold text-[11px] border"
            style={{
              backgroundColor: `${color}18`,
              borderColor: `${color}40`,
              color,
            }}
          >
            <span>{icon}</span>
            <span>{displayName}</span>
          </span>

          <span className="font-bold text-sm text-[#1E201C] dark:text-[#EDEAE2]">
            {data.title}
          </span>
        </div>

        <span className="text-[10px] text-[#A09D94] dark:text-[#6E6D66] font-mono">
          {data.artifact_id.slice(0, 14)}
          {isSaving && ' • saving...'}
        </span>
      </div>

      {/* Property Pills & Controls */}
      {schema.length > 0 ? (
        <div className="flex flex-wrap items-center gap-3 pt-1 border-t border-[#ECE9E0] dark:border-[#262822]">
          {schema.map((prop) => (
            <div
              key={prop.key}
              className="inline-flex items-center gap-1.5 px-2 py-1 rounded-lg bg-white dark:bg-[#1F201B] border border-[#E0DCD3] dark:border-[#2C2E27]"
            >
              <span className="text-[10px] font-semibold text-[#6E6D66] dark:text-[#9E9C93] uppercase tracking-wider">
                {prop.label || prop.key}:
              </span>
              <PropertyFieldRenderer
                definition={prop}
                value={properties[prop.key]}
                compact
                onChange={(val) => handlePropertyChange(prop.key, val)}
              />
            </div>
          ))}
        </div>
      ) : (
        /* Fallback for ad-hoc properties without formal schema */
        Object.keys(properties).length > 0 && (
          <div className="flex flex-wrap items-center gap-2 pt-1 border-t border-[#ECE9E0] dark:border-[#262822]">
            {Object.entries(properties).map(([k, v]) => (
              <span
                key={k}
                className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md bg-white dark:bg-[#1F201B] border border-[#E0DCD3] dark:border-[#2C2E27] text-[11px]"
              >
                <span className="font-semibold text-[#6E6D66] dark:text-[#9E9C93]">{k}:</span>
                <span className="text-[#1E201C] dark:text-[#EDEAE2]">{String(v)}</span>
              </span>
            ))}
          </div>
        )
      )}
    </div>
  );
};
