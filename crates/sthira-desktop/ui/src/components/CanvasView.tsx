import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  ArrowLeft,
  Maximize2,
  FileText,
  Type,
  Plus,
  Search,
} from 'lucide-react';
import { canvasOps, revertChangeSet } from '../api/ipc';
import { UndoToast } from './UndoToast';
import type { CanvasDataView, CanvasItemView, ArtifactLinkView, SearchResultItem } from '../types/sthira';
import '../styles/spatial-canvas.css';

interface CanvasViewProps {
  canvasId?: string;
  projectId?: string;
  onBack: () => void;
  onOpenArtifact?: (artifactId: string, canvas: CanvasDataView) => void;
}

export const CanvasView: React.FC<CanvasViewProps> = ({
  canvasId,
  projectId,
  onBack,
  onOpenArtifact,
}) => {
  const [canvas, setCanvas] = useState<CanvasDataView | null>(null);
  const [items, setItems] = useState<CanvasItemView[]>([]);
  const [links, setLinks] = useState<ArtifactLinkView[]>([]);
  const [undoToast, setUndoToast] = useState<{ message: string; changeSetId: string } | null>(null);

  // Dragging state
  const [draggingItemId, setDraggingItemId] = useState<string | null>(null);
  const [dragOffset, setDragOffset] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [pages, setPages] = useState<SearchResultItem[]>([]);
  const [pagesState, setPagesState] = useState<'loading' | 'ready' | 'error'>('loading');
  const [pickerReload, setPickerReload] = useState(0);
  const [loading, setLoading] = useState(true);
  const viewport = useRef<HTMLDivElement>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  const pickerButton = useRef<HTMLButtonElement>(null);
  const mounted = useRef(false);
  const canvasLoad = useRef(0);
  const mutationInProgress = useRef(false);
  const pickerVisible = !!canvas && (!items.length || pickerOpen);
  const loadedCanvasId = canvas?.id;

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  useEffect(() => {
    if (pickerOpen) searchInput.current?.focus();
  }, [pickerOpen]);

  const loadCanvas = useCallback(async () => {
    const request = ++canvasLoad.current;
    try {
      const resp = await canvasOps({
        action: 'GetCanvas',
        payload: { canvas_id: canvasId || null, project_id: projectId || null },
      });
      if (request !== canvasLoad.current) return;
      if (!resp.canvas) throw new Error('The canvas is unavailable.');
      setCanvas(resp.canvas);
      setItems(resp.canvas.items);
      setLinks(resp.canvas.links);
    } catch {
      if (request === canvasLoad.current) setError('Could not load this canvas. Try again.');
    } finally {
      if (request === canvasLoad.current) setLoading(false);
    }
  }, [canvasId, projectId]);

  useEffect(() => {
    setCanvas(null);
    setItems([]);
    setLinks([]);
    setError('');
    setLoading(true);
    void loadCanvas();
    return () => { canvasLoad.current += 1; };
  }, [loadCanvas]);

  useEffect(() => {
    if (!pickerVisible || !loadedCanvasId) return;
    let cancelled = false;
    setPagesState('loading');
    const timer = window.setTimeout(async () => {
      try {
        const response = await canvasOps({ action: 'ListPages', payload: { canvas_id: loadedCanvasId, query } });
        if (!cancelled) {
          setPages(response.pages ?? []);
          setPagesState('ready');
        }
      } catch {
        if (!cancelled) setPagesState('error');
      }
    }, query ? 180 : 0);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [pickerVisible, loadedCanvasId, query, pickerReload]);

  const handleMouseDown = (e: React.MouseEvent, item: CanvasItemView) => {
    if (busy || (e.target as HTMLElement).closest('button')) return;
    e.stopPropagation();
    setDraggingItemId(item.item_id);
    setDragOffset({
      x: e.clientX - item.x,
      y: e.clientY - item.y,
    });
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    if (!draggingItemId) return;
    const newX = Math.max(20, e.clientX - dragOffset.x);
    const newY = Math.max(20, e.clientY - dragOffset.y);

    setItems((prev) =>
      prev.map((item) =>
        item.item_id === draggingItemId ? { ...item, x: newX, y: newY } : item
      )
    );
  };

  const handleMouseUp = async () => {
    if (!draggingItemId || !canvas) {
      setDraggingItemId(null);
      return;
    }

    const currentItem = items.find((i) => i.item_id === draggingItemId);
    setDraggingItemId(null);

    if (currentItem) {
      try {
        // Save spatial position only - strict decoupling: never mutates semantic links
        await canvasOps({
          action: 'MoveItem',
          payload: {
            canvas_id: canvas.id,
            item_id: currentItem.item_id,
            x: currentItem.x,
            y: currentItem.y,
          },
        });
      } catch {
        setError('Could not save this card position. The last saved position was reloaded.');
        void loadCanvas();
      }
    }
  };

  const nextPosition = () => {
    const columns = Math.max(1, Math.floor(((viewport.current?.clientWidth || 640) - 24) / 304));
    for (let index = 0; ; index++) {
      const x = 24 + (index % columns) * 304;
      const y = 24 + Math.floor(index / columns) * 164;
      if (!items.some(item => x < item.x + (item.w ?? 280) + 8 && x + 288 > item.x
        && y < item.y + (item.h ?? 140) + 8 && y + 148 > item.y)) return { x, y };
    }
  };

  const changeCards = async (action: 'CreateNote' | 'PlacePage', pageId?: string, placeholderId?: string) => {
    if (!canvas || mutationInProgress.current) return;
    mutationInProgress.current = true;
    setBusy(true);
    setError('');
    try {
      const response = await canvasOps({
        action,
        payload: {
          canvas_id: canvas.id,
          ...(action === 'CreateNote' ? { placeholder_id: placeholderId ?? null } : { artifact_id: pageId }),
          ...nextPosition(),
        },
      });
      if (!mounted.current) return;
      if (!response.success || (action === 'CreateNote' && !response.artifact_id)) {
        throw new Error('The canvas action could not be completed.');
      }
      setPickerOpen(false);
      if (action === 'PlacePage') {
        await loadCanvas();
      } else if (response.artifact_id && onOpenArtifact) {
        onOpenArtifact(response.artifact_id, canvas);
      } else {
        await loadCanvas();
      }
    } catch (err) {
      if (mounted.current) setError(err instanceof Error ? err.message : String(err));
    } finally {
      mutationInProgress.current = false;
      if (mounted.current) {
        setBusy(false);
        if (action === 'PlacePage') requestAnimationFrame(() => pickerButton.current?.focus());
      }
    }
  };

  const handleBatchAutoAlign = async () => {
    if (!canvas || busy || items.length === 0) return;
    // Auto align cards in neat grid
    const updatedPositions = items.map((item, idx) => ({
      item_id: item.item_id,
      x: 80 + (idx % 3) * 340,
      y: 100 + Math.floor(idx / 3) * 240,
    }));

    try {
      const resp = await canvasOps({
        action: 'BatchUpdatePositions',
        payload: {
          canvas_id: canvas.id,
          positions: updatedPositions,
        },
      });

      setItems((prev) =>
        prev.map((item) => {
          const match = updatedPositions.find((p) => p.item_id === item.item_id);
          return match ? { ...item, x: match.x, y: match.y } : item;
        })
      );

      if (resp.change_set_id) {
        setUndoToast({
          message: 'Canvas layout reorganized',
          changeSetId: resp.change_set_id,
        });
      }
    } catch {
      setError('Could not save the new layout.');
    }
  };

  const handleUndo = async () => {
    if (!undoToast) return;
    try {
      await revertChangeSet(undoToast.changeSetId);
      setUndoToast(null);
      loadCanvas();
    } catch {
      setError('Could not undo the layout change.');
    }
  };

  return (
    <div
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      className="spatial-canvas canvas-dot-grid bg-[var(--sthira-canvas)] text-[var(--sthira-text)]"
    >
      {/* Top Header Bar */}
      <header className="spatial-canvas-header bg-[var(--sthira-surface-raised)] border-b border-[var(--sthira-border)]">
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={onBack}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-[var(--sthira-surface-muted)] hover:bg-[var(--sthira-border)] text-xs font-semibold text-[var(--sthira-text)] transition-colors"
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            Back
          </button>
          <div className="h-4 w-px bg-[var(--sthira-border)]" />
          <div>
            <h2 className="text-sm font-bold text-[var(--sthira-text)]">
              {canvas?.name || 'Spatial canvas'}
            </h2>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2.5">
          <button
            type="button"
            onClick={() => void changeCards('CreateNote')}
            disabled={!canvas || busy}
            className="sthira-button-primary"
          >
            <Type className="w-3.5 h-3.5" />
            Add Note
          </button>
          <button ref={pickerButton} type="button" className="sthira-button-secondary" disabled={!canvas || busy}
            aria-expanded={pickerVisible} aria-controls={pickerVisible ? 'canvas-page-picker' : undefined}
            onClick={() => {
              if (!items.length) searchInput.current?.focus();
              else { setQuery(''); setPickerOpen(open => !open); }
            }}>
            <Plus size={15} aria-hidden="true" />
            Add existing page
          </button>

          <button
            type="button"
            onClick={handleBatchAutoAlign}
            disabled={!items.length || busy}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-[var(--sthira-surface-raised)] border border-[var(--sthira-border-strong)] hover:bg-[var(--sthira-surface-muted)] text-xs font-medium text-[var(--sthira-text)] transition-colors"
          >
            <Maximize2 className="w-3.5 h-3.5" />
            Auto-Align
          </button>
        </div>
      </header>

      {error && <div className="spatial-canvas-status" role="alert">{error}{!canvas && (
        <button type="button" className="sthira-button-secondary" onClick={() => { setError(''); setLoading(true); void loadCanvas(); }}>Retry</button>
      )}</div>}
      {loading && <p className="spatial-canvas-status" role="status">Loading canvas…</p>}
      {pickerVisible && (
        <section id="canvas-page-picker" className="canvas-page-picker" aria-label="Choose a saved page">
          <h3>Choose a saved page</h3>
          <label className="canvas-page-search">
            <Search size={16} aria-hidden="true" />
            <input ref={searchInput} aria-label="Search saved pages" value={query} onChange={event => setQuery(event.target.value)} placeholder="Search saved pages…" />
          </label>
          {pagesState === 'loading' && <p role="status">Loading saved pages…</p>}
          {pagesState === 'error' && <p role="alert">Could not load saved pages. <button type="button" onClick={() => setPickerReload(n => n + 1)}>Retry</button></p>}
          {pagesState === 'ready' && (pages.length ? (
            <ul>
              {pages.map(page => <li key={page.id}>
                <button type="button" aria-label={`Add ${page.title} to canvas`} disabled={busy} onClick={() => void changeCards('PlacePage', page.id)}>
                  <FileText size={15} aria-hidden="true" /><span>{page.title}</span><Plus size={15} aria-hidden="true" />
                </button>
              </li>)}
            </ul>
          ) : <p>{query.trim() ? 'No matching pages to add.' : 'No saved pages to add. Use Add Note to create one.'}</p>)}
        </section>
      )}
      <div ref={viewport} className="spatial-canvas-viewport">
        <div className="spatial-canvas-board" style={{
          minWidth: `max(100%, ${items.reduce((size, item) => Math.max(size, item.x + (item.w ?? 280) + 24), 0)}px)`,
          minHeight: `max(100%, ${items.reduce((size, item) => Math.max(size, item.y + (item.h ?? 140) + 24), 0)}px)`,
        }}>
        {/* Render Semantic Links as SVG connector lines */}
        <svg className="absolute inset-0 w-full h-full pointer-events-none z-0">
          {links.map((link, idx) => {
            const src = items.find((i) => i.artifact_id === link.source_artifact_id || i.item_id === link.source_artifact_id);
            const dst = items.find((i) => i.artifact_id === link.target_artifact_id || i.item_id === link.target_artifact_id);
            if (!src || !dst) return null;

            const x1 = src.x + (src.w ?? 280) / 2;
            const y1 = src.y + (src.h ?? 140) / 2;
            const x2 = dst.x + (dst.w ?? 280) / 2;
            const y2 = dst.y + (dst.h ?? 140) / 2;

            return (
              <g key={idx}>
                <line
                  x1={x1}
                  y1={y1}
                  x2={x2}
                  y2={y2}
                  stroke="var(--sthira-primary)"
                  strokeWidth="2"
                  strokeDasharray="4 4"
                  opacity="0.75"
                />
                <rect
                  x={(x1 + x2) / 2 - 40}
                  y={(y1 + y2) / 2 - 12}
                  width="80"
                  height="22"
                  rx="6"
                  fill="var(--sthira-surface-raised)"
                  stroke="var(--sthira-border)"
                />
                <text
                  x={(x1 + x2) / 2}
                  y={(y1 + y2) / 2 + 4}
                  textAnchor="middle"
                  fontSize="11"
                  fontWeight="600"
                  fill="var(--sthira-text)"
                >
                  {link.relationship_type}
                </text>
              </g>
            );
          })}
        </svg>

        {/* Render Canvas Items */}
        {items.map((item) => (
          <div
            key={item.item_id}
            onMouseDown={(e) => handleMouseDown(e, item)}
            style={{
              position: 'absolute',
              left: `${item.x}px`,
              top: `${item.y}px`,
              width: item.w ? `${item.w}px` : '280px',
              minHeight: item.h ? `${item.h}px` : '140px',
              zIndex: draggingItemId === item.item_id ? 50 : item.z || 10,
            }}
            className={`bg-[var(--sthira-surface-raised)] border border-[var(--sthira-border)] rounded-2xl p-4 shadow-sm hover:shadow-md transition-shadow cursor-grab active:cursor-grabbing flex flex-col justify-between ${
              draggingItemId === item.item_id ? 'ring-2 ring-[var(--sthira-primary)] shadow-xl' : ''
            }`}
          >
            <div>
              <div className="flex items-center justify-between pb-2 mb-2 border-b border-[var(--sthira-border)]">
                <span className="text-[11px] font-bold uppercase tracking-wider text-[var(--sthira-text-muted)] flex items-center gap-1">
                  {item.item_kind === 'artifact' ? (
                    <>
                      <FileText className="w-3.5 h-3.5 text-[var(--sthira-primary)]" />
                      Document
                    </>
                  ) : (
                    <>
                      <Type className="w-3.5 h-3.5 text-[var(--sthira-text-muted)]" />
                      Thought Note
                    </>
                  )}
                </span>
                {item.artifact_id && onOpenArtifact && (
                  <button
                    type="button"
                    aria-label={`Open ${item.title || 'Untitled page'}`}
                    onClick={(e) => {
                      e.stopPropagation();
                      if (canvas) onOpenArtifact(item.artifact_id!, canvas);
                    }}
                    className="text-[12px] font-semibold text-[var(--sthira-primary)] hover:underline"
                  >
                    Open
                  </button>
                )}
                {item.item_kind === 'text' && !item.artifact_id && (
                  <button type="button" disabled={busy} className="text-[12px] font-semibold text-[var(--sthira-primary)] hover:underline"
                    onClick={() => void changeCards('CreateNote', undefined, item.item_id)}>Open as note</button>
                )}
              </div>

              <h4 className="font-bold text-[var(--sthira-text)] text-sm mb-1.5">
                {item.title || 'Untitled Card'}
              </h4>
              {item.excerpt && <p className="text-xs text-[var(--sthira-text-muted)] leading-relaxed line-clamp-3">{item.excerpt}</p>}
            </div>
          </div>
        ))}
        </div>
      </div>

      {/* Undo Toast */}
      {undoToast && (
        <UndoToast
          message={undoToast.message}
          onUndo={handleUndo}
          onClose={() => setUndoToast(null)}
        />
      )}
    </div>
  );
};
