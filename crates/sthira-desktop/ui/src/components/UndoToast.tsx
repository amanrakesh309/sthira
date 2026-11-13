import React from 'react';
import { RotateCcw, X } from 'lucide-react';

interface UndoToastProps {
  message: string;
  onUndo: () => void;
  onClose: () => void;
  placement?: 'viewport' | 'workspace';
}

export const UndoToast: React.FC<UndoToastProps> = ({
  message,
  onUndo,
  onClose,
  placement = 'viewport',
}) => {
  const positionClass =
    placement === 'workspace'
      ? 'focus-undo-slot'
      : 'fixed bottom-6 right-6 z-50';
  return (
    <div
      data-testid="undo-toast"
      data-placement={placement}
      className={`${positionClass} flex flex-wrap items-center gap-x-3 gap-y-2 bg-[#282C25] text-white px-4 py-3 rounded-xl shadow-lg border border-[#3A3B36] animate-in fade-in slide-in-from-bottom-2`}
    >
      <span role="status" className="min-w-0 text-sm font-medium">{message}</span>
      <div className="flex items-center gap-2 shrink-0 ml-auto">
        <button
          type="button"
          data-testid="undo-toast-btn"
          onClick={onUndo}
          className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-white/10 hover:bg-white/20 text-xs font-semibold text-white transition-colors"
        >
          <RotateCcw className="w-3.5 h-3.5" />
          Undo
        </button>
        <button
          type="button"
          onClick={onClose}
          aria-label="Close"
          className="text-[#B8B5AA] hover:text-white p-0.5 rounded transition-colors"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>
    </div>
  );
};
