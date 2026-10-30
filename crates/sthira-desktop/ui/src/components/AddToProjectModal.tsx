import React, { useState } from 'react';
import { FolderPlus, Folder, Check } from 'lucide-react';
import { addArtifactToProject } from '../api/ipc';
import type { WorkspaceTreeProject } from '../types/sthira';

interface AddToProjectModalProps {
  isOpen: boolean;
  artifactId: string | null;
  artifactTitle?: string;
  projects: WorkspaceTreeProject[];
  onClose: () => void;
  onAdded: () => void;
}

export const AddToProjectModal: React.FC<AddToProjectModalProps> = ({
  isOpen,
  artifactId,
  artifactTitle,
  projects,
  onClose,
  onAdded,
}) => {
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!isOpen || !artifactId) return null;

  const handleConfirm = async () => {
    if (!selectedProjectId || isSubmitting) return;
    setIsSubmitting(true);
    setError(null);
    try {
      await addArtifactToProject(artifactId, selectedProjectId);
      onAdded();
      onClose();
    } catch (err: any) {
      console.error('Failed to add artifact to project', err);
      setError(err?.toString() || 'Failed to add to project');
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4 animate-in fade-in duration-150">
      <div className="bg-[#FFFFFF] border border-[#E8E5DC] rounded-2xl shadow-2xl w-full max-w-md overflow-hidden text-[#1E201C]">
        {/* Header */}
        <div className="px-6 py-4 border-b border-[#E8E5DC] flex items-center justify-between bg-[#F5F3EC]/50">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-xl bg-[#FAF9F5] border border-[#E8E5DC] flex items-center justify-center text-[#282C25]">
              <FolderPlus className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-sm font-bold text-[#1E201C]">Add to Project</h2>
              <p className="text-xs text-[#6E6D66] truncate max-w-xs">
                &ldquo;{artifactTitle || 'Untitled Note'}&rdquo;
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded-lg text-[#8C8A82] hover:text-[#1E201C] hover:bg-[#EFECE4] transition-colors cursor-pointer"
          >
            ✕
          </button>
        </div>

        {/* Project List */}
        <div className="p-6 space-y-3 text-xs">
          {error && (
            <div className="p-2.5 rounded-xl bg-red-50 border border-red-200 text-red-700 text-xs">
              {error}
            </div>
          )}

          <p className="text-[11px] font-bold text-[#6E6D66] uppercase tracking-wider">
            Select Destination Project
          </p>

          {projects.length === 0 ? (
            <p className="text-xs text-[#8C8A82] italic py-4 text-center">
              No projects created yet. Create a project first.
            </p>
          ) : (
            <div className="max-h-60 overflow-y-auto space-y-1.5 pr-1">
              {projects.map((proj) => {
                const isSelected = selectedProjectId === proj.id;
                return (
                  <div
                    key={proj.id}
                    data-testid={`select-project-${proj.id}`}
                    onClick={() => setSelectedProjectId(proj.id)}
                    className={`p-3 rounded-xl border transition-all cursor-pointer flex items-center justify-between ${
                      isSelected
                        ? 'border-[#282C25] bg-[#F5F3EC]/80 shadow-xs'
                        : 'border-[#E8E5DC] bg-white hover:bg-[#FAF9F5]'
                    }`}
                  >
                    <div className="flex items-center gap-2.5 min-w-0">
                      <Folder className="w-4 h-4 text-[#8C6D1F] shrink-0" />
                      <div>
                        <div className="font-semibold text-xs text-[#1E201C] truncate">
                          {proj.name}
                        </div>
                        {proj.purpose && (
                          <div className="text-[10px] text-[#8C8A82] truncate max-w-xs">
                            {proj.purpose}
                          </div>
                        )}
                      </div>
                    </div>
                    {isSelected && <Check className="w-4 h-4 text-[#282C25] shrink-0" />}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="px-6 py-4 border-t border-[#E8E5DC] bg-[#FAF9F5] flex items-center justify-end gap-2.5">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 rounded-xl text-xs font-medium text-[#6E6D66] hover:bg-[#EFECE4] transition-colors cursor-pointer"
          >
            Cancel
          </button>
          <button
            type="button"
            data-testid="btn-confirm-add-to-project"
            disabled={!selectedProjectId || isSubmitting}
            onClick={handleConfirm}
            className="px-5 py-2 rounded-xl text-xs font-semibold bg-[#282C25] text-white hover:opacity-90 transition-opacity cursor-pointer shadow-xs disabled:opacity-40"
          >
            {isSubmitting ? 'Adding...' : 'Add to Project'}
          </button>
        </div>
      </div>
    </div>
  );
};

export default AddToProjectModal;
