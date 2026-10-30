import React, { useState, useMemo } from 'react';
import { Plus, ExternalLink } from 'lucide-react';
import type { ObjectType, ObjectItem, PropertyDefinition } from '../../types/sthira';

interface ObjectKanbanViewProps {
  objectType: ObjectType;
  items: ObjectItem[];
  onUpdateProperty: (artifactId: string, propertyKey: string, newValue: any) => void;
  onOpenItem: (artifactId: string, projectId?: string | null) => void;
  onQuickCreate?: (initialProperties: Record<string, any>) => void;
}

export const ObjectKanbanView: React.FC<ObjectKanbanViewProps> = ({
  objectType,
  items,
  onUpdateProperty,
  onOpenItem,
  onQuickCreate,
}) => {
  // Find all select properties that can group the Kanban board
  const selectProperties = useMemo(() => {
    return objectType.schema.filter((p) => p.property_type === 'select');
  }, [objectType.schema]);

  const [activeGroupKey, setActiveGroupKey] = useState<string>(() => {
    return selectProperties[0]?.key || '';
  });

  // Dragging state
  const [draggedArtifactId, setDraggedArtifactId] = useState<string | null>(null);
  const [activeDropColumn, setActiveDropColumn] = useState<string | null>(null);

  const groupProperty: PropertyDefinition | undefined = useMemo(() => {
    return selectProperties.find((p) => p.key === activeGroupKey) || selectProperties[0];
  }, [selectProperties, activeGroupKey]);

  // Options for columns
  const columns = useMemo(() => {
    if (!groupProperty || !groupProperty.options || groupProperty.options.length === 0) {
      return ['all'];
    }
    return [...groupProperty.options, 'unassigned'];
  }, [groupProperty]);

  // Group items into columns
  const columnItems = useMemo(() => {
    const map: Record<string, ObjectItem[]> = {};
    for (const col of columns) {
      map[col] = [];
    }

    if (!groupProperty) {
      map['all'] = items;
      return map;
    }

    for (const item of items) {
      const val = item.properties[groupProperty.key];
      if (val && map[val]) {
        map[val].push(item);
      } else {
        if (map['unassigned']) {
          map['unassigned'].push(item);
        } else if (columns[0]) {
          map[columns[0]].push(item);
        }
      }
    }

    return map;
  }, [items, groupProperty, columns]);

  const getColumnColor = (col: string) => {
    const lower = col.toLowerCase();
    if (['spark', 'exploring', 'low', 'unread'].includes(lower)) return '#F59E0B'; // Amber
    if (['validated', 'high', 'processed', 'done'].includes(lower)) return '#10B981'; // Emerald
    if (['dormant', 'medium', 'reading'].includes(lower)) return '#3B82F6'; // Blue
    return '#8C8A82';
  };

  const handleDragStart = (e: React.DragEvent, artifactId: string) => {
    e.dataTransfer.setData('text/plain', artifactId);
    setDraggedArtifactId(artifactId);
  };

  const handleDragOver = (e: React.DragEvent, col: string) => {
    e.preventDefault();
    if (activeDropColumn !== col) {
      setActiveDropColumn(col);
    }
  };

  const handleDrop = (e: React.DragEvent, col: string) => {
    e.preventDefault();
    setActiveDropColumn(null);
    setDraggedArtifactId(null);
    const artifactId = e.dataTransfer.getData('text/plain');
    if (!artifactId || !groupProperty) return;

    const targetVal = col === 'unassigned' ? '' : col;
    onUpdateProperty(artifactId, groupProperty.key, targetVal);
  };

  if (selectProperties.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center p-12 text-center text-xs text-[#8C8A82] bg-white rounded-2xl border border-[#E8E5DC]">
        <p className="font-semibold text-sm text-[#1E201C] mb-1">
          No Categorical Groupings Available
        </p>
        <p className="max-w-md">
          To display a Kanban board, the &quot;{objectType.display_name}&quot; schema needs at least
          one select property (e.g. status, priority, stage).
        </p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full space-y-3">
      {/* Grouping switcher if multiple select properties exist */}
      {selectProperties.length > 1 && (
        <div className="flex items-center gap-2 px-1">
          <span className="text-xs font-semibold text-[#6E6D66]">Group By:</span>
          <select
            value={groupProperty?.key}
            onChange={(e) => setActiveGroupKey(e.target.value)}
            className="text-xs px-2.5 py-1 rounded-lg border border-[#E8E5DC] bg-white text-[#1E201C] font-medium outline-none cursor-pointer focus:border-[#282C25]"
          >
            {selectProperties.map((p) => (
              <option key={p.key} value={p.key}>
                {p.label}
              </option>
            ))}
          </select>
        </div>
      )}

      {/* Kanban Board Horizontal Columns Container */}
      <div className="flex-1 overflow-x-auto flex gap-4 pb-4 items-start select-none">
        {columns.map((col) => {
          const colItems = columnItems[col] || [];
          const isOver = activeDropColumn === col;
          const colColor = getColumnColor(col);
          const formattedTitle = col === 'unassigned' ? 'Unassigned' : col.charAt(0).toUpperCase() + col.slice(1);

          return (
            <div
              key={col}
              onDragOver={(e) => handleDragOver(e, col)}
              onDragLeave={() => setActiveDropColumn(null)}
              onDrop={(e) => handleDrop(e, col)}
              className={`w-72 shrink-0 flex flex-col max-h-full rounded-2xl border transition-colors ${
                isOver
                  ? 'border-[#282C25] bg-[#EFECE4]/80 ring-2 ring-[#282C25]/20'
                  : 'border-[#E8E5DC] bg-[#FAF9F5]'
              }`}
            >
              {/* Column Header */}
              <div className="p-3 border-b border-[#E8E5DC] flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <div
                    className="w-2.5 h-2.5 rounded-full shrink-0"
                    style={{ backgroundColor: colColor }}
                  />
                  <h3 className="text-xs font-bold text-[#1E201C] tracking-tight">
                    {formattedTitle}
                  </h3>
                  <span className="text-[10px] font-semibold text-[#6E6D66] bg-[#EFECE4] px-1.5 py-0.5 rounded-full border border-[#E8E5DC]">
                    {colItems.length}
                  </span>
                </div>

                {onQuickCreate && col !== 'unassigned' && groupProperty && (
                  <button
                    type="button"
                    onClick={() => onQuickCreate({ [groupProperty.key]: col })}
                    className="p-1 rounded text-[#8C8A82] hover:text-[#1E201C] hover:bg-[#EFECE4] transition-colors cursor-pointer"
                    title={`Add ${objectType.display_name} in ${formattedTitle}`}
                  >
                    <Plus className="w-3.5 h-3.5" />
                  </button>
                )}
              </div>

              {/* Cards List */}
              <div className="flex-1 overflow-y-auto p-2.5 space-y-2.5 min-h-[150px]">
                {colItems.length === 0 ? (
                  <div className="py-8 text-center text-[11px] text-[#8C8A82] italic border border-dashed border-[#E8E5DC]/80 rounded-xl">
                    Drop items here
                  </div>
                ) : (
                  colItems.map((item) => (
                    <div
                      key={item.artifact_id}
                      draggable={true}
                      onDragStart={(e) => handleDragStart(e, item.artifact_id)}
                      className={`p-3 rounded-xl border border-[#E8E5DC] bg-white shadow-2xs hover:shadow-xs transition-all cursor-grab active:cursor-grabbing group ${
                        draggedArtifactId === item.artifact_id ? 'opacity-40' : ''
                      }`}
                    >
                      {/* Top Bar: Icon + Title + Open Link */}
                      <div className="flex items-start justify-between gap-1.5 mb-1.5">
                        <div
                          onClick={() => onOpenItem(item.artifact_id, item.project_id)}
                          className="font-semibold text-xs text-[#1E201C] hover:underline cursor-pointer flex-1 line-clamp-2"
                        >
                          {objectType.icon} {item.title || 'Untitled'}
                        </div>
                        <button
                          type="button"
                          onClick={() => onOpenItem(item.artifact_id, item.project_id)}
                          className="opacity-0 group-hover:opacity-100 p-0.5 rounded text-[#8C8A82] hover:text-[#1E201C] transition-opacity cursor-pointer shrink-0"
                          title="Open in Focus View"
                        >
                          <ExternalLink className="w-3 h-3" />
                        </button>
                      </div>

                      {/* Other Properties Preview */}
                      <div className="space-y-1 pt-1 border-t border-[#F5F3EC]">
                        {objectType.schema
                          .filter((p) => p.key !== groupProperty?.key)
                          .slice(0, 3)
                          .map((prop) => {
                            const val = item.properties[prop.key];
                            if (val === undefined || val === null || val === '') return null;
                            return (
                              <div
                                key={prop.key}
                                className="flex items-center justify-between text-[10px] text-[#6E6D66]"
                              >
                                <span className="font-medium">{prop.label}:</span>
                                <span className="truncate max-w-[140px] text-[#1E201C]">
                                  {String(val)}
                                </span>
                              </div>
                            );
                          })}
                      </div>

                      {/* Move Column Shortcut for touch/accessibility */}
                      {columns.length > 1 && groupProperty && (
                        <div className="mt-2 pt-1.5 border-t border-[#F5F3EC] flex items-center justify-between">
                          <span className="text-[9px] text-[#8C8A82] uppercase tracking-wider font-semibold">
                            Move:
                          </span>
                          <select
                            value={item.properties[groupProperty.key] || 'unassigned'}
                            onChange={(e) =>
                              onUpdateProperty(
                                item.artifact_id,
                                groupProperty.key,
                                e.target.value === 'unassigned' ? '' : e.target.value
                              )
                            }
                            className="text-[10px] px-1.5 py-0.5 rounded border border-[#E8E5DC] bg-[#FAF9F5] text-[#3A3B36] outline-none cursor-pointer"
                          >
                            {columns.map((c) => (
                              <option key={c} value={c}>
                                {c === 'unassigned' ? 'Unassigned' : c}
                              </option>
                            ))}
                          </select>
                        </div>
                      )}
                    </div>
                  ))
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};

export default ObjectKanbanView;
