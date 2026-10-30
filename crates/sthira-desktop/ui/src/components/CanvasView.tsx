import React, { useState, useEffect } from 'react';
import {
  ArrowLeft,
  Maximize2,
  FileText,
  Type,
} from 'lucide-react';
import { canvasOps, revertChangeSet } from '../api/ipc';
import { UndoToast } from './UndoToast';
import type { CanvasDataView, CanvasItemView, ArtifactLinkView } from '../types/sthira';

interface CanvasViewProps {
  canvasId?: string;
  projectId?: string;
  onBack: () => void;
  onOpenArtifact?: (artifactId: string) => void;
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

  useEffect(() => {
    loadCanvas();
  }, [canvasId, projectId]);

  const loadCanvas = async () => {
    try {
      const resp = await canvasOps({
        action: 'GetCanvas',
        payload: { canvas_id: canvasId || null, project_id: projectId || null },
      });
      if (resp.canvas) {
        setCanvas(resp.canvas);
        setItems(resp.canvas.items);
        setLinks(resp.canvas.links);
      }
    } catch (err) {
      console.error('Failed to load canvas', err);
    }
  };

  const handleMouseDown = (e: React.MouseEvent, item: CanvasItemView) => {
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
      } catch (err) {
        console.error('Failed to move canvas item', err);
      }
    }
  };

  const handleAddTextCard = async () => {
    if (!canvas) return;
    try {
      await canvasOps({
        action: 'AddItem',
        payload: {
          canvas_id: canvas.id,
          item_kind: 'text',
          artifact_id: null,
          x: 200 + Math.random() * 80,
          y: 200 + Math.random() * 80,
          w: 240,
          h: 120,
        },
      });
      loadCanvas();
    } catch (err) {
      console.error('Failed to add text card', err);
    }
  };

  const handleBatchAutoAlign = async () => {
    if (!canvas || items.length === 0) return;
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
    } catch (err) {
      console.error('Failed to batch update positions', err);
    }
  };

  const handleUndo = async () => {
    if (!undoToast) return;
    try {
      await revertChangeSet(undoToast.changeSetId);
      setUndoToast(null);
      loadCanvas();
    } catch (err) {
      console.error('Failed to undo canvas change set', err);
    }
  };

  return (
    <div
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      className="min-h-screen canvas-dot-grid relative overflow-hidden select-none flex flex-col"
    >
      {/* Top Header Bar */}
      <header className="w-full bg-[#FFFFFF]/90 dark:bg-[#161714]/90 backdrop-blur-md border-b border-[#E8E5DC] dark:border-[#2C2E27] px-6 py-3.5 flex items-center justify-between sticky top-0 z-30">
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={onBack}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-xs font-semibold text-[#1E201C] dark:text-[#EDEAE2] transition-colors"
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            Back
          </button>
          <div className="h-4 w-px bg-[#E8E5DC] dark:bg-[#2C2E27]" />
          <div>
            <h2 className="text-sm font-bold text-[#1E201C] dark:text-[#EDEAE2]">
              {canvas?.name || 'Ideas to investigate'}
            </h2>
            <p className="text-[11px] text-[#6E6D66] dark:text-[#9E9C93]">
              Spatial Canvas · Coordinates decoupled from semantic links
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2.5">
          <button
            type="button"
            onClick={handleAddTextCard}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-xs font-medium text-[#3A3B36] dark:text-[#D4D1C7] transition-colors"
          >
            <Type className="w-3.5 h-3.5" />
            Add Note
          </button>

          <button
            type="button"
            onClick={handleBatchAutoAlign}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-xl bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-xs font-medium text-[#3A3B36] dark:text-[#D4D1C7] transition-colors"
          >
            <Maximize2 className="w-3.5 h-3.5" />
            Auto-Align
          </button>
        </div>
      </header>

      {/* 2D Canvas Surface */}
      <div className="flex-1 relative w-full h-[calc(100vh-60px)] overflow-auto p-8">
        {/* Render Semantic Links as SVG connector lines */}
        <svg className="absolute inset-0 w-full h-full pointer-events-none z-0">
          {links.map((link, idx) => {
            const src = items.find((i) => i.artifact_id === link.source_artifact_id || i.item_id === link.source_artifact_id);
            const dst = items.find((i) => i.artifact_id === link.target_artifact_id || i.item_id === link.target_artifact_id);
            if (!src || !dst) return null;

            const x1 = src.x + 140;
            const y1 = src.y + 60;
            const x2 = dst.x + 140;
            const y2 = dst.y + 60;

            return (
              <g key={idx}>
                <line
                  x1={x1}
                  y1={y1}
                  x2={x2}
                  y2={y2}
                  stroke="#8C6D1F"
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
                  fill="#FFFFFF"
                  stroke="#E8E5DC"
                />
                <text
                  x={(x1 + x2) / 2}
                  y={(y1 + y2) / 2 + 4}
                  textAnchor="middle"
                  fontSize="10"
                  fontWeight="600"
                  fill="#282C25"
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
            className={`bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl p-4 shadow-sm hover:shadow-md transition-shadow cursor-grab active:cursor-grabbing flex flex-col justify-between ${
              draggingItemId === item.item_id ? 'ring-2 ring-[#282C25] dark:ring-[#EDEAE2] shadow-xl' : ''
            }`}
          >
            <div>
              <div className="flex items-center justify-between pb-2 mb-2 border-b border-[#ECE9E0] dark:border-[#2C2E27]">
                <span className="text-[10px] font-bold uppercase tracking-wider text-[#6E6D66] dark:text-[#9E9C93] flex items-center gap-1">
                  {item.item_kind === 'artifact' ? (
                    <>
                      <FileText className="w-3 h-3 text-[#282C25] dark:text-[#EDEAE2]" />
                      Document
                    </>
                  ) : (
                    <>
                      <Type className="w-3 h-3 text-[#6E6D66] dark:text-[#9E9C93]" />
                      Thought Note
                    </>
                  )}
                </span>
                {item.artifact_id && onOpenArtifact && (
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      onOpenArtifact(item.artifact_id!);
                    }}
                    className="text-[11px] font-semibold text-[#282C25] dark:text-[#EDEAE2] hover:underline"
                  >
                    Open →
                  </button>
                )}
              </div>

              <h4 className="font-bold text-[#1E201C] dark:text-[#EDEAE2] text-sm mb-1.5">
                {item.title || 'Untitled Card'}
              </h4>
              <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93] leading-relaxed line-clamp-3">
                {item.excerpt || 'Drag card anywhere on canvas. Spatial movements remain decoupled from semantic links.'}
              </p>
            </div>

            <div className="pt-2 text-[10px] text-[#6E6D66] dark:text-[#9E9C93] flex items-center justify-between">
              <span>x: {Math.round(item.x)}, y: {Math.round(item.y)}</span>
              <span className="font-medium text-[#282C25] dark:text-[#EDEAE2]">Sthira Canvas</span>
            </div>
          </div>
        ))}
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
