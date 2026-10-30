import React, { useState, useEffect } from 'react';
import { AlertTriangle, Trash2, Folder, FileText } from 'lucide-react';
import { inspectProjectDeletion, deleteProject } from '../api/ipc';
import type { ProjectDeletionImpact } from '../types/sthira';

interface ProjectDeletionModalProps {
  isOpen: boolean;
  projectId: string | null;
  onClose: () => void;
  onDeleted: () => void;
}

export const ProjectDeletionModal: React.FC<ProjectDeletionModalProps> = ({
  isOpen,
  projectId,
  onClose,
  onDeleted,
}) => {
  const [impact, setImpact] = useState<ProjectDeletionImpact | null>(null);
  const [cascadeExclusive, setCascadeExclusive] = useState<boolean>(false); // Safe default: keep pages as loose
  const [isLoading, setIsLoading] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen && projectId) {
      setIsLoading(true);
      setError(null);
      setCascadeExclusive(false);
      inspectProjectDeletion(projectId)
        .then((data) => setImpact(data))
        .catch((err) => {
          console.error('Failed to inspect deletion impact', err);
          setError('Could not inspect project impact');
        })
        .finally(() => setIsLoading(false));
    } else {
      setImpact(null);
    }
  }, [isOpen, projectId]);

  if (!isOpen || !projectId) return null;

  const handleConfirm = async () => {
    setIsDeleting(true);
    setError(null);
    try {
      await deleteProject(projectId, cascadeExclusive);
      onDeleted();
      onClose();
    } catch (err: any) {
      console.error('Failed to delete project', err);
      setError(err?.toString() || 'Failed to delete project');
    } finally {
      setIsDeleting(false);
    }
  };

  const exclusiveCount = impact?.exclusive_artifacts.length || 0;
  const sharedCount = impact?.shared_artifacts.length || 0;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4 animate-in fade-in duration-150">
      <div className="bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl shadow-2xl w-full max-w-lg overflow-hidden text-[#1E201C] dark:text-[#EDEAE2]">
        {/* Header */}
        <div className="px-6 py-4 border-b border-[#E8E5DC] dark:border-[#2C2E27] flex items-center justify-between bg-[#F5F3EC]/50 dark:bg-[#161714]/60">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-xl bg-red-50 dark:bg-red-950/40 border border-red-200 dark:border-red-900/40 flex items-center justify-center text-red-600 dark:text-red-400">
              <AlertTriangle className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-base font-bold text-[#1E201C] dark:text-[#EDEAE2]">Delete Project</h2>
              <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93]">
                Review deletion impact for &ldquo;{impact?.project_name || 'Project'}&rdquo;
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-1 rounded-lg text-[#8C8A82] dark:text-[#6E6D66] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] hover:bg-[#EFECE4] dark:hover:bg-[#252820] transition-colors cursor-pointer"
          >
            ✕
          </button>
        </div>

        {/* Content */}
        <div className="p-6 space-y-5 text-xs">
          {error && (
            <div className="p-3 rounded-xl bg-red-50 dark:bg-red-950/40 border border-red-200 dark:border-red-900/40 text-red-700 dark:text-red-300">
              {error}
            </div>
          )}

          {isLoading ? (
            <div className="py-8 text-center text-[#8C8A82] dark:text-[#6E6D66]">
              Inspecting project pages and relationships...
            </div>
          ) : (
            <>
              {/* Impact Banner */}
              <div className="p-4 rounded-xl bg-[#FAF9F5] dark:bg-[#161714] border border-[#E8E5DC] dark:border-[#2C2E27] space-y-2">
                <div className="text-xs font-semibold text-[#1E201C] dark:text-[#EDEAE2]">
                  This project contains:
                </div>
                <div className="grid grid-cols-2 gap-2 text-xs">
                  <div className="p-2.5 rounded-lg bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] flex items-center gap-2">
                    <FileText className="w-4 h-4 text-[#8C8A82] dark:text-[#6E6D66]" />
                    <span>
                      <strong className="text-[#1E201C] dark:text-[#EDEAE2]">{exclusiveCount}</strong> exclusive {exclusiveCount === 1 ? 'page' : 'pages'}
                    </span>
                  </div>
                  <div className="p-2.5 rounded-lg bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] flex items-center gap-2">
                    <Folder className="w-4 h-4 text-[#8C8A82] dark:text-[#6E6D66]" />
                    <span>
                      <strong className="text-[#1E201C] dark:text-[#EDEAE2]">{sharedCount}</strong> shared {sharedCount === 1 ? 'page' : 'pages'}
                    </span>
                  </div>
                </div>
              </div>

              {/* Radio Choices */}
              <div className="space-y-2.5">
                <label className="block text-[11px] font-bold text-[#6E6D66] dark:text-[#9E9C93] uppercase tracking-wider">
                  Select Deletion Strategy
                </label>

                {/* Safe Default */}
                <div
                  onClick={() => setCascadeExclusive(false)}
                  className={`p-3.5 rounded-xl border transition-all cursor-pointer flex items-start gap-3 ${
                    !cascadeExclusive
                      ? 'border-[#282C25] dark:border-[#EDEAE2] bg-[#F5F3EC]/70 dark:bg-[#22241F] shadow-xs'
                      : 'border-[#E8E5DC] dark:border-[#2C2E27] bg-white dark:bg-[#1C1D19] hover:bg-[#FAF9F5] dark:hover:bg-[#252820]'
                  }`}
                >
                  <input
                    type="radio"
                    name="cascadeChoice"
                    data-testid="radio-keep-unassociated"
                    checked={!cascadeExclusive}
                    onChange={() => setCascadeExclusive(false)}
                    className="mt-0.5 cursor-pointer accent-[#282C25] dark:accent-[#EDEAE2]"
                  />
                  <div className="flex-1 text-xs">
                    <div className="font-semibold text-[#1E201C] dark:text-[#EDEAE2] flex items-center gap-1.5">
                      <span>Keep all pages as unassociated notes</span>
                      <span className="text-[10px] font-bold px-1.5 py-0.5 rounded bg-[#E3EBE3] dark:bg-[#182618] text-[#2D5A27] dark:text-[#88D080]">
                        Safe Default
                      </span>
                    </div>
                    <p className="text-[#6E6D66] dark:text-[#9E9C93] text-[11px] mt-0.5">
                      The project container is removed. Its {exclusiveCount} exclusive {exclusiveCount === 1 ? 'page remains' : 'pages remain'} safe and accessible under Unassociated Notes.
                    </p>
                  </div>
                </div>

                {/* Cascade Option */}
                <div
                  onClick={() => setCascadeExclusive(true)}
                  className={`p-3.5 rounded-xl border transition-all cursor-pointer flex items-start gap-3 ${
                    cascadeExclusive
                      ? 'border-red-400 dark:border-red-600 bg-red-50/50 dark:bg-red-950/20 shadow-xs'
                      : 'border-[#E8E5DC] dark:border-[#2C2E27] bg-white dark:bg-[#1C1D19] hover:bg-[#FAF9F5] dark:hover:bg-[#252820]'
                  }`}
                >
                  <input
                    type="radio"
                    name="cascadeChoice"
                    data-testid="radio-cascade-exclusive"
                    checked={cascadeExclusive}
                    onChange={() => setCascadeExclusive(true)}
                    className="mt-0.5 cursor-pointer accent-red-600"
                  />
                  <div className="flex-1 text-xs">
                    <div className="font-semibold text-red-700 dark:text-red-400">
                      Delete project and its {exclusiveCount} exclusive {exclusiveCount === 1 ? 'page' : 'pages'}
                    </div>
                    <p className="text-[#6E6D66] dark:text-[#9E9C93] text-[11px] mt-0.5">
                      Permanently deletes exclusive content. Any shared pages ({sharedCount}) will remain intact in their other projects.
                    </p>
                  </div>
                </div>
              </div>
            </>
          )}
        </div>

        {/* Footer */}
        <div className="px-6 py-4 border-t border-[#E8E5DC] dark:border-[#2C2E27] bg-[#FAF9F5] dark:bg-[#161714] flex items-center justify-end gap-2.5">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 rounded-xl text-xs font-medium text-[#6E6D66] dark:text-[#9E9C93] hover:bg-[#EFECE4] dark:hover:bg-[#252820] transition-colors cursor-pointer"
          >
            Cancel
          </button>
          <button
            type="button"
            data-testid="btn-confirm-delete-project"
            disabled={isDeleting || isLoading}
            onClick={handleConfirm}
            className="px-5 py-2 rounded-xl text-xs font-semibold bg-red-600 text-white hover:bg-red-700 transition-colors cursor-pointer shadow-xs disabled:opacity-50 inline-flex items-center gap-1.5"
          >
            <Trash2 className="w-3.5 h-3.5" />
            <span>{isDeleting ? 'Deleting...' : 'Delete Project'}</span>
          </button>
        </div>
      </div>
    </div>
  );
};

export default ProjectDeletionModal;
