import React, { useState, useEffect, useRef, useCallback } from 'react';
import {
  MoreHorizontal,
  Link2,
  Package,
  FolderPlus,
  Undo2,
  Copy,
  Tag,
  FileText,
  Link,
  Trash2,
  ChevronRight,
  Search,
  Loader2,
  Check,
  ExternalLink,
  AlertTriangle,
} from 'lucide-react';
import {
  getWorkspaceTree,
  listObjectTypes,
  duplicateArtifact,
  linkArtifactToProject,
  moveArtifactToProject,
  createProjectWithArtifact,
  convertArtifactToObject,
  detachArtifactFromProject,
  deleteArtifact,
} from '../api/ipc';
import type { ObjectType } from '../types/sthira';

interface PageActionMenuProps {
  artifactId: string;
  artifactTitle?: string;
  /** Markdown body, used by `Copy Markdown`. */
  contentMd?: string;
  /** Current project. `null`/`undefined` means the note is standalone. */
  projectId?: string | null;
  /** Pre-loaded object types; fetched on demand when omitted. */
  objectTypes?: ObjectType[];
  variant?: 'toolbar' | 'row';
  align?: 'left' | 'right';
  triggerTestId?: string;
  /** When provided, a `Rename` item is rendered (the parent owns inline-rename state). */
  onRename?: () => void;
  /**
   * When provided, `Link to Project...` delegates to the parent (e.g. LeftSidebar's
   * existing `AddToProjectModal`) instead of rendering the built-in picker.
   */
  onLinkProjectRequested?: () => void;
  onChanged?: () => void;
  onOpenArtifact?: (artifactId: string, projectId?: string | null) => void;
  onDeleted?: () => void;
  onProjectCreated?: (projectId: string) => void;
  onConverted?: (artifactId: string, objectTypeId: string) => void;
}

type MenuView = 'root' | 'convert' | 'picker' | 'create-project' | 'confirm-delete';

/** Best-effort clipboard write that also works under jsdom / missing permissions. */
const copyToClipboard = async (text: string): Promise<boolean> => {
  try {
    if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    /* fall through to the legacy path */
  }
  try {
    const ta = document.createElement('textarea');
    ta.value = text;
    ta.style.position = 'fixed';
    ta.style.opacity = '0';
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand ? document.execCommand('copy') : false;
    document.body.removeChild(ta);
    return ok;
  } catch {
    return false;
  }
};

export const PageActionMenu: React.FC<PageActionMenuProps> = ({
  artifactId,
  artifactTitle,
  contentMd,
  projectId,
  objectTypes,
  variant = 'row',
  align = 'right',
  triggerTestId,
  onRename,
  onLinkProjectRequested,
  onChanged,
  onOpenArtifact,
  onDeleted,
  onProjectCreated,
  onConverted,
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const [view, setView] = useState<MenuView>('root');
  const [pickerMode, setPickerMode] = useState<'link' | 'move'>('link');
  const [projects, setProjects] = useState<{ id: string; name: string }[]>([]);
  const [availableTypes, setAvailableTypes] = useState<ObjectType[]>(objectTypes || []);
  const [projectSearch, setProjectSearch] = useState('');
  const [newProjectName, setNewProjectName] = useState('');
  const [isBusy, setIsBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<{ message: string; openId?: string } | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const isStandalone = !projectId;

  const close = useCallback(() => {
    setIsOpen(false);
    setView('root');
    setError(null);
    setProjectSearch('');
    setNewProjectName('');
  }, []);

  // Lazy-load projects + object types the first time the menu opens.
  useEffect(() => {
    if (!isOpen) return;
    let cancelled = false;
    (async () => {
      try {
        const [tree, types] = await Promise.all([getWorkspaceTree(), listObjectTypes()]);
        if (cancelled) return;
        setProjects(tree.projects.map((p) => ({ id: p.id, name: p.name })));
        if (!objectTypes) setAvailableTypes(types);
      } catch (err) {
        console.warn('PageActionMenu: failed to load projects/types', err);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [isOpen, objectTypes]);

  useEffect(() => {
    if (!isOpen) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        close();
      }
    };
    const onClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) close();
    };
    window.addEventListener('keydown', onKeyDown);
    document.addEventListener('mousedown', onClickOutside);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      document.removeEventListener('mousedown', onClickOutside);
    };
  }, [isOpen, close]);

  const afterMutation = (message: string, openId?: string) => {
    setToast({ message, openId });
    onChanged?.();
    window.dispatchEvent(new CustomEvent('sthira:refresh_objects'));
    close();
  };

  const run = async (fn: () => Promise<void>) => {
    setIsBusy(true);
    setError(null);
    try {
      await fn();
    } catch (err: any) {
      console.error('Page action failed', err);
      setError(err?.toString() || 'Action failed');
    } finally {
      setIsBusy(false);
    }
  };

  const filteredProjects = projects.filter((p) =>
    p.name.toLowerCase().includes(projectSearch.trim().toLowerCase())
  );

  const itemClass =
    'w-full flex items-center gap-2.5 px-3 py-2 text-left text-xs text-[#3A3B36] dark:text-[#EDEAE2] hover:bg-[#FAF9F5] dark:hover:bg-[#252820] hover:text-[#1E201C] dark:hover:text-white transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed';

  const renderRoot = () => (
    <>
      {/* Group 1: Project Management */}
      <div className="py-1">
        <div className="px-3 py-1 text-[10px] font-bold text-[#8C8A82] dark:text-[#7A786F] uppercase tracking-wider">
          Project
        </div>
        <button
          type="button"
          data-testid="page-action-link"
          disabled={isBusy}
          onClick={() => {
            if (onLinkProjectRequested) {
              onLinkProjectRequested();
              close();
              return;
            }
            setPickerMode('link');
            setView('picker');
          }}
          className={itemClass}
        >
          <Link2 className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E] shrink-0" />
          <span>🔗 Link to Project...</span>
        </button>

        <button
          type="button"
          data-testid="page-action-move"
          disabled={isBusy}
          onClick={() => {
            setPickerMode('move');
            setView('picker');
          }}
          className={itemClass}
        >
          <Package className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E] shrink-0" />
          <span>📦 Move to Project...</span>
        </button>

        <button
          type="button"
          data-testid="page-action-create-project"
          disabled={isBusy}
          onClick={() => setView('create-project')}
          className={itemClass}
        >
          <FolderPlus className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E] shrink-0" />
          <span>➕ Create Project with this Note</span>
        </button>

        {!isStandalone && (
          <button
            type="button"
            data-testid="page-action-detach"
            disabled={isBusy}
            onClick={() =>
              run(async () => {
                await detachArtifactFromProject(artifactId, projectId!);
                afterMutation('Detached — moved to Standalone Notes');
              })
            }
            className={itemClass}
          >
            <Undo2 className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E] shrink-0" />
            <span>↩️ Detach from Project</span>
          </button>
        )}
      </div>

      <div className="h-px bg-[#E8E5DC] dark:bg-[#2C2E27]" />

      {/* Group 2: Duplication & Transform */}
      <div className="py-1">
        <div className="px-3 py-1 text-[10px] font-bold text-[#8C8A82] dark:text-[#7A786F] uppercase tracking-wider">
          Duplicate &amp; Transform
        </div>
        <button
          type="button"
          data-testid="page-action-duplicate"
          disabled={isBusy}
          onClick={() =>
            run(async () => {
              const newId = await duplicateArtifact(artifactId, null, projectId || null);
              afterMutation('Note duplicated', newId);
            })
          }
          className={itemClass}
        >
          <Copy className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E] shrink-0" />
          <span>📋 Duplicate Note</span>
        </button>

        <button
          type="button"
          data-testid="page-action-convert"
          disabled={isBusy}
          onClick={() => setView('convert')}
          className={`${itemClass} justify-between`}
        >
          <span className="flex items-center gap-2.5">
            <Tag className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E] shrink-0" />
            <span>🏷️ Convert to Object Type</span>
          </span>
          <ChevronRight className="w-3 h-3 text-[#8C8A82]" />
        </button>

        {onRename && (
          <button
            type="button"
            data-testid="page-action-rename"
            disabled={isBusy}
            onClick={() => {
              onRename();
              close();
            }}
            className={itemClass}
          >
            <FileText className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E] shrink-0" />
            <span>✏️ Rename</span>
          </button>
        )}
      </div>


      <div className="h-px bg-[#E8E5DC] dark:bg-[#2C2E27]" />

      {/* Group 3: Clipboard & Links */}
      <div className="py-1">
        <button
          type="button"
          data-testid="page-action-copy-markdown"
          disabled={isBusy}
          onClick={() =>
            run(async () => {
              const ok = await copyToClipboard(contentMd || '');
              setToast({
                message: ok
                  ? 'Markdown copied to clipboard'
                  : 'Copy failed — clipboard unavailable',
              });
              close();
            })
          }
          className={itemClass}
        >
          <FileText className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E] shrink-0" />
          <span>📄 Copy Markdown</span>
        </button>

        <button
          type="button"
          data-testid="page-action-copy-link"
          disabled={isBusy}
          onClick={() =>
            run(async () => {
              const ok = await copyToClipboard(`sthira://artifact/${artifactId}`);
              setToast({
                message: ok ? 'Note link copied' : 'Copy failed — clipboard unavailable',
              });
              close();
            })
          }
          className={itemClass}
        >
          <Link className="w-3.5 h-3.5 text-[#8C8A82] dark:text-[#A8A69E] shrink-0" />
          <span>🔗 Copy Note Link</span>
        </button>
      </div>

      <div className="h-px bg-[#E8E5DC] dark:bg-[#2C2E27]" />

      {/* Group 4: Destructive */}
      <div className="py-1">
        <button
          type="button"
          data-testid="page-action-delete"
          disabled={isBusy}
          onClick={() => setView('confirm-delete')}
          className="w-full flex items-center gap-2.5 px-3 py-2 text-left text-xs text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/30 transition-colors cursor-pointer"
        >
          <Trash2 className="w-3.5 h-3.5 shrink-0" />
          <span>🗑️ Delete Page...</span>
        </button>
      </div>
    </>
  );


  const renderConvert = () => (
    <div className="py-1">
      <div className="px-3 py-1 text-[10px] font-bold text-[#8C8A82] dark:text-[#7A786F] uppercase tracking-wider">
        Convert to Object Type
      </div>
      {availableTypes.length === 0 ? (
        <div className="px-3 py-2 text-[11px] text-[#8C8A82] italic">
          No object types registered.
        </div>
      ) : (
        availableTypes.map((t) => (
          <button
            key={t.id}
            type="button"
            data-testid={`page-action-convert-${t.id}`}
            disabled={isBusy}
            onClick={() =>
              run(async () => {
                await convertArtifactToObject(artifactId, t.id);
                onConverted?.(artifactId, t.id);
                afterMutation(`Converted to ${t.display_name}`);
              })
            }
            className={itemClass}
          >
            <span className="shrink-0">{t.icon || '📦'}</span>
            <span className="truncate">{t.display_name}</span>
          </button>
        ))
      )}
      <div className="h-px bg-[#E8E5DC] dark:bg-[#2C2E27] my-1" />
      <button
        type="button"
        data-testid="page-action-back"
        onClick={() => setView('root')}
        className="w-full flex items-center gap-2.5 px-3 py-2 text-left text-xs text-[#6E6D66] dark:text-[#9E9C93] hover:bg-[#FAF9F5] dark:hover:bg-[#252820] cursor-pointer"
      >
        <ChevronRight className="w-3.5 h-3.5 rotate-180 shrink-0" />
        <span>Back</span>
      </button>
    </div>
  );

  const renderPicker = () => (
    <div className="py-1">
      <div className="px-3 py-1.5 text-[10px] font-bold text-[#8C8A82] dark:text-[#7A786F] uppercase tracking-wider">
        {pickerMode === 'move' ? 'Move to Project' : 'Link to Project'}
      </div>
      <div className="px-2.5 pb-1.5">
        <div className="relative">
          <Search className="w-3 h-3 absolute left-2.5 top-1/2 -translate-y-1/2 text-[#8C8A82]" />
          <input
            type="text"
            autoFocus
            data-testid="page-action-project-search"
            value={projectSearch}
            onChange={(e) => setProjectSearch(e.target.value)}
            placeholder="Search projects..."
            className="w-full pl-7 pr-2 py-1.5 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-[#FAF8F3] dark:bg-[#161714] text-xs outline-none focus:border-[#282C25] dark:focus:border-[#EDEAE2]"
          />
        </div>
      </div>

      <div className="max-h-52 overflow-y-auto">
        {filteredProjects.length === 0 ? (
          <div className="px-3 py-2 text-[11px] text-[#8C8A82] italic">
            {projects.length === 0 ? 'No projects exist yet' : 'No matching projects'}
          </div>
        ) : (
          filteredProjects.map((p) => (
            <button
              key={p.id}
              type="button"
              data-testid={`page-action-project-${p.id}`}
              disabled={isBusy || (pickerMode === 'move' && p.id === projectId)}
              onClick={() =>
                run(async () => {
                  if (pickerMode === 'move') {
                    await moveArtifactToProject(artifactId, projectId || null, p.id);
                    afterMutation(`Moved to ${p.name}`);
                  } else {
                    await linkArtifactToProject(artifactId, p.id);
                    afterMutation(`Linked to ${p.name}`);
                  }
                })
              }
              className={itemClass}
            >
              <span className="shrink-0">📁</span>
              <span className="truncate">{p.name}</span>
              {pickerMode === 'move' && p.id === projectId && (
                <span className="ml-auto text-[9px] uppercase text-[#8C8A82]">current</span>
              )}
            </button>
          ))
        )}
      </div>

      <div className="h-px bg-[#E8E5DC] dark:bg-[#2C2E27] my-1" />
      <button
        type="button"
        data-testid="page-action-back"
        onClick={() => setView('root')}
        className="w-full flex items-center gap-2.5 px-3 py-2 text-left text-xs text-[#6E6D66] dark:text-[#9E9C93] hover:bg-[#FAF9F5] dark:hover:bg-[#252820] cursor-pointer"
      >
        <ChevronRight className="w-3.5 h-3.5 rotate-180 shrink-0" />
        <span>Back</span>
      </button>
    </div>
  );


  const renderCreateProject = () => (
    <div className="py-1">
      <div className="px-3 py-1.5 text-[10px] font-bold text-[#8C8A82] dark:text-[#7A786F] uppercase tracking-wider">
        New Project from Note
      </div>
      <div className="px-2.5 pb-2 space-y-2">
        <input
          type="text"
          autoFocus
          data-testid="page-action-create-project-input"
          value={newProjectName}
          onChange={(e) => setNewProjectName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && newProjectName.trim() && !isBusy) {
              e.preventDefault();
              run(async () => {
                const name = newProjectName.trim();
                const newId = await createProjectWithArtifact(name, artifactId);
                onProjectCreated?.(newId);
                afterMutation(`Project "${name}" created`);
              });
            }
          }}
          placeholder="Project name..."
          className="w-full px-2.5 py-1.5 rounded-lg border border-[#E8E5DC] dark:border-[#2C2E27] bg-[#FAF8F3] dark:bg-[#161714] text-xs outline-none focus:border-[#282C25] dark:focus:border-[#EDEAE2]"
        />
        <div className="flex items-center gap-1.5">
          <button
            type="button"
            data-testid="page-action-create-project-submit"
            disabled={isBusy || !newProjectName.trim()}
            onClick={() =>
              run(async () => {
                const name = newProjectName.trim();
                const newId = await createProjectWithArtifact(name, artifactId);
                onProjectCreated?.(newId);
                afterMutation(`Project "${name}" created`);
              })
            }
            className="flex-1 inline-flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-lg bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] text-xs font-semibold hover:opacity-90 transition-opacity disabled:opacity-40 cursor-pointer"
          >
            <Check className="w-3 h-3" />
            Create &amp; Open
          </button>
          <button
            type="button"
            data-testid="page-action-back"
            onClick={() => setView('root')}
            className="px-3 py-1.5 rounded-lg text-xs text-[#6E6D66] dark:text-[#9E9C93] hover:bg-[#FAF9F5] dark:hover:bg-[#252820] cursor-pointer"
          >
            Back
          </button>
        </div>
      </div>
    </div>
  );

  const renderConfirmDelete = () => (
    <div className="p-3 space-y-3" data-testid="page-action-delete-confirm">
      <div className="flex items-start gap-2.5">
        <AlertTriangle className="w-4 h-4 text-red-600 dark:text-red-400 shrink-0 mt-0.5" />
        <div className="space-y-1">
          <p className="text-xs font-bold text-[#1E201C] dark:text-[#EDEAE2]">
            Delete &ldquo;{artifactTitle || 'this page'}&rdquo;?
          </p>
          <p className="text-[11px] text-[#6E6D66] dark:text-[#9E9C93] leading-relaxed">
            {isStandalone
              ? 'This standalone note will be permanently removed from the vault.'
              : 'If this note also lives in another project it is detached from this one only; otherwise it is permanently removed. Epistemic history is preserved either way.'}
          </p>
        </div>
      </div>
      <div className="flex items-center gap-2">
        <button
          type="button"
          data-testid="page-action-confirm-delete"
          disabled={isBusy}
          onClick={() =>
            run(async () => {
              await deleteArtifact(artifactId, projectId || undefined);
              onDeleted?.();
              onChanged?.();
              window.dispatchEvent(new CustomEvent('sthira:refresh_objects'));
              close();
            })
          }
          className="flex-1 inline-flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-lg bg-red-600 hover:bg-red-700 text-white text-xs font-semibold transition-colors disabled:opacity-50 cursor-pointer"
        >
          <Trash2 className="w-3 h-3" />
          Delete
        </button>
        <button
          type="button"
          data-testid="page-action-cancel-delete"
          onClick={() => setView('root')}
          className="px-3 py-1.5 rounded-lg text-xs text-[#6E6D66] dark:text-[#9E9C93] hover:bg-[#FAF9F5] dark:hover:bg-[#252820] cursor-pointer"
        >
          Cancel
        </button>
      </div>
    </div>
  );


  return (
    <div
      ref={containerRef}
      className="relative"
      onClick={(e) => e.stopPropagation()}
      data-testid={`page-action-menu-${artifactId}`}
    >
      {/* Three-dots trigger */}
      <button
        type="button"
        data-testid={triggerTestId || 'page-action-menu-trigger'}
        onClick={() => (isOpen ? close() : setIsOpen(true))}
        aria-haspopup="menu"
        aria-expanded={isOpen}
        aria-label="Page actions"
        title="Page actions"
        className={
          variant === 'toolbar'
            ? 'inline-flex items-center gap-1 px-2 py-1.5 rounded-lg bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-[#3A3B36] dark:text-[#D4D1C7] transition-colors cursor-pointer'
            : 'p-1.5 rounded-lg text-[#6E6D66] dark:text-[#A8A69E] hover:bg-[#EFECE4] dark:hover:bg-[#22241F] hover:text-[#1E201C] dark:hover:text-white transition-colors cursor-pointer'
        }
      >
        <MoreHorizontal className="w-3.5 h-3.5" />
        {variant === 'toolbar' && <span className="text-[11px] font-semibold">•••</span>}
      </button>

      {/* Dropdown */}
      {isOpen && (
        <div
          role="menu"
          data-testid="page-action-dropdown"
          className={`absolute z-50 mt-1 w-64 bg-white dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl shadow-2xl overflow-hidden text-xs font-sans animate-in fade-in zoom-in-95 duration-100 ${
            align === 'right' ? 'right-0' : 'left-0'
          }`}
        >
          {isBusy && (
            <div className="flex items-center gap-1.5 px-3 py-1.5 bg-[#FAF8F3] dark:bg-[#161714] border-b border-[#ECE9E0] dark:border-[#2C2E27] text-[10px] text-[#8C8A82]">
              <Loader2 className="w-3 h-3 animate-spin" />
              <span>Working...</span>
            </div>
          )}
          {error && (
            <div className="px-3 py-1.5 bg-red-50 dark:bg-red-950/30 border-b border-red-100 dark:border-red-900/40 text-[10px] text-red-600 dark:text-red-400">
              {error}
            </div>
          )}

          {view === 'root' && renderRoot()}
          {view === 'convert' && renderConvert()}
          {view === 'picker' && renderPicker()}
          {view === 'create-project' && renderCreateProject()}
          {view === 'confirm-delete' && renderConfirmDelete()}
        </div>
      )}

      {/* Result toast with [Open] */}
      {toast && (
        <div
          data-testid="page-action-toast"
          className={`fixed bottom-6 right-6 z-50 flex items-center gap-3 bg-[#282C25] text-white px-4 py-3 rounded-xl shadow-lg border border-[#3A3B36] animate-in fade-in slide-in-from-bottom-2 ${
            variant === 'row' ? 'pointer-events-auto' : ''
          }`}
        >
          <span className="text-xs font-medium">{toast.message}</span>
          {toast.openId && onOpenArtifact && (
            <button
              type="button"
              data-testid="page-action-toast-open"
              onClick={() => {
                onOpenArtifact(toast.openId!, projectId || null);
                setToast(null);
              }}
              className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-white/10 hover:bg-white/20 text-xs font-semibold text-white transition-colors cursor-pointer"
            >
              <ExternalLink className="w-3.5 h-3.5" />
              Open
            </button>
          )}
          <button
            type="button"
            data-testid="page-action-toast-close"
            onClick={() => setToast(null)}
            aria-label="Dismiss"
            className="text-[#6E6D66] hover:text-white p-0.5 rounded transition-colors cursor-pointer"
          >
            ✕
          </button>
        </div>
      )}
    </div>
  );
};

export default PageActionMenu;

