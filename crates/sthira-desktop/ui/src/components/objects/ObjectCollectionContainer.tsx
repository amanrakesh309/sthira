import React, { useState, useEffect } from 'react';
import {
  Table as TableIcon,
  Columns,
  LayoutGrid,
  Plus,
  ArrowLeft,
  X,
} from 'lucide-react';
import {
  listObjectsByType,
  saveObjectInstance,
  createTypedObject,
  getWorkspaceTree,
} from '../../api/ipc';
import { ObjectTableView } from './ObjectTableView';
import { ObjectKanbanView } from './ObjectKanbanView';
import { PropertyFieldRenderer } from './PropertyFieldRenderer';
import type { ObjectType, ObjectItem } from '../../types/sthira';

interface ObjectCollectionContainerProps {
  objectType: ObjectType;
  onBack: () => void;
  onOpenArtifact: (artifactId: string, projectId?: string) => void;
  onOpenCanvas?: (canvasId?: string) => void;
}

export const ObjectCollectionContainer: React.FC<ObjectCollectionContainerProps> = ({
  objectType,
  onBack,
  onOpenArtifact,
}) => {
  const [viewMode, setViewMode] = useState<'table' | 'kanban' | 'canvas'>('table');
  const [items, setItems] = useState<ObjectItem[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isCreateOpen, setIsCreateOpen] = useState(false);

  // New object form state
  const [newTitle, setNewTitle] = useState('');
  const [newProperties, setNewProperties] = useState<Record<string, any>>({});
  const [newProjectId, setNewProjectId] = useState<string>('');
  const [workspaceProjects, setWorkspaceProjects] = useState<{ id: string; name: string }[]>([]);
  const [isCreating, setIsCreating] = useState(false);

  const loadItems = async () => {
    setIsLoading(true);
    try {
      const data = await listObjectsByType(objectType.id);
      setItems(data);
    } catch (err) {
      console.error('Failed to load objects for type', objectType.id, err);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadItems();
    // Load projects for quick-attach dropdown
    getWorkspaceTree().then((tree) => {
      setWorkspaceProjects(tree.projects.map((p) => ({ id: p.id, name: p.name })));
    }).catch(console.error);
  }, [objectType.id]);

  // Handle inline property changes
  const handleUpdateProperty = async (
    artifactId: string,
    propertyKey: string,
    newValue: any
  ) => {
    // Optimistic UI update
    setItems((prev) =>
      prev.map((item) => {
        if (item.artifact_id !== artifactId) return item;
        return {
          ...item,
          properties: {
            ...item.properties,
            [propertyKey]: newValue,
          },
          updated_at: Date.now(),
        };
      })
    );

    // Save to backend
    const targetItem = items.find((i) => i.artifact_id === artifactId);
    if (!targetItem) return;

    const updatedProps = {
      ...targetItem.properties,
      [propertyKey]: newValue,
    };

    try {
      await saveObjectInstance({
        artifact_id: artifactId,
        object_type_id: objectType.id,
        properties: updatedProps,
        updated_at: Date.now(),
      });
    } catch (err) {
      console.error('Failed to save object instance update', err);
      // Revert if failed
      loadItems();
    }
  };

  const handleOpenCreateWithDefaults = (initialProps?: Record<string, any>) => {
    setNewTitle('');
    const defaults: Record<string, any> = {};
    for (const p of objectType.schema) {
      if (initialProps && initialProps[p.key] !== undefined) {
        defaults[p.key] = initialProps[p.key];
      } else if (p.property_type === 'boolean') {
        defaults[p.key] = false;
      } else if (p.property_type === 'select' && p.options && p.options.length > 0) {
        defaults[p.key] = p.options[0];
      } else {
        defaults[p.key] = '';
      }
    }
    setNewProperties(defaults);
    setNewProjectId('');
    setIsCreateOpen(true);
  };

  const handleCreateSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newTitle.trim()) return;

    setIsCreating(true);
    try {
      const created = await createTypedObject(
        objectType.id,
        newTitle.trim(),
        newProperties,
        newProjectId || null
      );
      setItems((prev) => [created, ...prev]);
      setIsCreateOpen(false);
    } catch (err) {
      console.error('Failed to create typed object', err);
    } finally {
      setIsCreating(false);
    }
  };

  return (
    <div className="flex flex-col h-full bg-[#F5F3EC] text-[#1E201C] overflow-hidden">
      {/* Top Header Bar */}
      <header className="px-6 py-4 bg-[#F5F3EC] border-b border-[#E8E5DC] flex items-center justify-between shrink-0">
        {/* Left: Back button + Type Identity */}
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={onBack}
            className="p-1.5 rounded-xl text-[#6E6D66] hover:text-[#1E201C] hover:bg-[#EFECE4] transition-colors cursor-pointer"
            title="Back to Home"
          >
            <ArrowLeft className="w-4 h-4" />
          </button>

          <div
            className="w-9 h-9 rounded-xl flex items-center justify-center text-xl shadow-2xs border"
            style={{
              backgroundColor: `${objectType.color}15`,
              borderColor: `${objectType.color}40`,
            }}
          >
            <span>{objectType.icon}</span>
          </div>

          <div>
            <div className="flex items-center gap-2">
              <h1 className="text-lg font-bold tracking-tight text-[#1E201C]">
                {objectType.display_name}s
              </h1>
              <span className="text-xs font-semibold px-2 py-0.5 rounded-full bg-[#EFECE4] text-[#6E6D66] border border-[#E8E5DC]">
                {items.length}
              </span>
            </div>
            <p className="text-[11px] text-[#8C8A82]">
              {objectType.is_system ? 'System Entity Collection' : 'Custom Entity Collection'}
            </p>
          </div>
        </div>

        {/* Right: View Switcher + New Object Button */}
        <div className="flex items-center gap-3">
          {/* View Mode Switcher Pill */}
          <div className="flex items-center p-1 rounded-xl bg-[#EFECE4] border border-[#E8E5DC]">
            <button
              type="button"
              onClick={() => setViewMode('table')}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold transition-all cursor-pointer ${
                viewMode === 'table'
                  ? 'bg-white text-[#1E201C] shadow-2xs'
                  : 'text-[#6E6D66] hover:text-[#1E201C]'
              }`}
            >
              <TableIcon className="w-3.5 h-3.5" />
              <span>Table</span>
            </button>

            <button
              type="button"
              onClick={() => setViewMode('kanban')}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold transition-all cursor-pointer ${
                viewMode === 'kanban'
                  ? 'bg-white text-[#1E201C] shadow-2xs'
                  : 'text-[#6E6D66] hover:text-[#1E201C]'
              }`}
            >
              <Columns className="w-3.5 h-3.5" />
              <span>Kanban</span>
            </button>

            <button
              type="button"
              onClick={() => setViewMode('canvas')}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold transition-all cursor-pointer ${
                viewMode === 'canvas'
                  ? 'bg-white text-[#1E201C] shadow-2xs'
                  : 'text-[#6E6D66] hover:text-[#1E201C]'
              }`}
            >
              <LayoutGrid className="w-3.5 h-3.5" />
              <span>Canvas</span>
            </button>
          </div>

          {/* New Item Button */}
          <button
            type="button"
            onClick={() => handleOpenCreateWithDefaults()}
            className="flex items-center gap-1.5 px-4 py-2 rounded-xl text-xs font-semibold bg-[#282C25] text-white hover:opacity-90 transition-opacity cursor-pointer shadow-xs"
          >
            <Plus className="w-4 h-4" />
            <span>New {objectType.display_name}</span>
          </button>
        </div>
      </header>

      {/* Main Body */}
      <div className="flex-1 overflow-hidden p-6">
        {isLoading ? (
          <div className="flex items-center justify-center h-64 text-xs text-[#8C8A82]">
            Loading collection...
          </div>
        ) : viewMode === 'table' ? (
          <ObjectTableView
            objectType={objectType}
            items={items}
            onUpdateProperty={handleUpdateProperty}
            onOpenItem={(artId, projId) => onOpenArtifact(artId, projId || undefined)}
          />
        ) : viewMode === 'kanban' ? (
          <ObjectKanbanView
            objectType={objectType}
            items={items}
            onUpdateProperty={handleUpdateProperty}
            onOpenItem={(artId, projId) => onOpenArtifact(artId, projId || undefined)}
            onQuickCreate={handleOpenCreateWithDefaults}
          />
        ) : (
          /* Canvas / Visual Card Grid View */
          <div className="h-full overflow-y-auto">
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4 pb-8">
              {items.length === 0 ? (
                <div className="col-span-full py-16 text-center text-xs text-[#8C8A82] italic">
                  No {objectType.display_name.toLowerCase()}s found.
                </div>
              ) : (
                items.map((item) => (
                  <div
                    key={item.artifact_id}
                    onClick={() => onOpenArtifact(item.artifact_id, item.project_id || undefined)}
                    className="p-4 rounded-2xl border border-[#E8E5DC] bg-white hover:border-[#282C25]/40 hover:shadow-xs transition-all cursor-pointer flex flex-col justify-between h-48 group"
                  >
                    <div>
                      <div className="flex items-center justify-between mb-2">
                        <span className="text-xl">{objectType.icon}</span>
                        <span className="text-[10px] text-[#8C8A82]">
                          {new Date(item.updated_at).toLocaleDateString()}
                        </span>
                      </div>
                      <h3 className="font-bold text-sm text-[#1E201C] group-hover:text-[#282C25] line-clamp-2">
                        {item.title || 'Untitled'}
                      </h3>
                    </div>

                    <div className="space-y-1 pt-2 border-t border-[#F5F3EC]">
                      {objectType.schema.slice(0, 3).map((prop) => {
                        const val = item.properties[prop.key];
                        if (val === undefined || val === null || val === '') return null;
                        return (
                          <div
                            key={prop.key}
                            className="flex items-center justify-between text-[11px] text-[#6E6D66]"
                          >
                            <span className="font-medium">{prop.label}:</span>
                            <span className="truncate max-w-[150px] font-semibold text-[#1E201C]">
                              {String(val)}
                            </span>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        )}
      </div>

      {/* Quick Create Drawer / Modal */}
      {isCreateOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-xs p-4">
          <div className="bg-white border border-[#E8E5DC] rounded-2xl shadow-xl w-full max-w-lg overflow-hidden animate-in fade-in duration-150">
            {/* Modal Header */}
            <div className="px-5 py-3.5 border-b border-[#E8E5DC] flex items-center justify-between bg-[#F5F3EC]/50">
              <div className="flex items-center gap-2">
                <span>{objectType.icon}</span>
                <h3 className="font-bold text-sm text-[#1E201C]">
                  New {objectType.display_name}
                </h3>
              </div>
              <button
                type="button"
                onClick={() => setIsCreateOpen(false)}
                className="p-1 rounded-lg text-[#8C8A82] hover:text-[#1E201C] hover:bg-[#EFECE4] transition-colors cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            {/* Modal Form */}
            <form onSubmit={handleCreateSubmit} className="p-5 space-y-4 text-xs">
              <div>
                <label className="block text-[11px] font-bold text-[#6E6D66] uppercase tracking-wider mb-1">
                  Title
                </label>
                <input
                  type="text"
                  required
                  autoFocus
                  value={newTitle}
                  onChange={(e) => setNewTitle(e.target.value)}
                  placeholder={`Enter ${objectType.display_name.toLowerCase()} title...`}
                  className="w-full px-3 py-2 rounded-xl border border-[#E8E5DC] bg-[#FAF9F5] text-xs font-medium focus:bg-white focus:border-[#282C25] outline-none"
                />
              </div>

              {/* Dynamic Property Inputs */}
              {objectType.schema.map((prop) => (
                <div key={prop.key}>
                  <label className="block text-[11px] font-bold text-[#6E6D66] uppercase tracking-wider mb-1">
                    {prop.label}
                  </label>
                  <PropertyFieldRenderer
                    definition={prop}
                    value={newProperties[prop.key]}
                    onChange={(val) =>
                      setNewProperties((prev) => ({ ...prev, [prop.key]: val }))
                    }
                  />
                </div>
              ))}

              {/* Attach to Project (Optional) */}
              <div>
                <label className="block text-[11px] font-bold text-[#6E6D66] uppercase tracking-wider mb-1">
                  Associate with Project (Optional)
                </label>
                <select
                  value={newProjectId}
                  onChange={(e) => setNewProjectId(e.target.value)}
                  className="w-full px-3 py-2 rounded-xl border border-[#E8E5DC] bg-[#FAF9F5] text-xs font-medium outline-none cursor-pointer focus:border-[#282C25]"
                >
                  <option value="">None (Unassociated)</option>
                  {workspaceProjects.map((proj) => (
                    <option key={proj.id} value={proj.id}>
                      {proj.name}
                    </option>
                  ))}
                </select>
              </div>

              {/* Modal Actions */}
              <div className="pt-2 border-t border-[#E8E5DC] flex items-center justify-end gap-2">
                <button
                  type="button"
                  onClick={() => setIsCreateOpen(false)}
                  className="px-4 py-2 rounded-xl text-xs font-medium text-[#6E6D66] hover:bg-[#EFECE4] transition-colors cursor-pointer"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={isCreating || !newTitle.trim()}
                  className="px-5 py-2 rounded-xl text-xs font-semibold bg-[#282C25] text-white hover:opacity-90 transition-opacity cursor-pointer shadow-xs disabled:opacity-50"
                >
                  {isCreating ? 'Creating...' : `Create ${objectType.display_name}`}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};

export default ObjectCollectionContainer;
