import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { Info } from 'lucide-react';
import { useAnchoredPopover } from '../hooks/useAnchoredPopover';
import '../styles/workspace-guide.css';

export function ContextInfo({ title, children }: { title: string; children: ReactNode }) {
  const [open, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const id = useId();
  const position = useAnchoredPopover(open, button, panel, title);

  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !button.current?.contains(event.target) && !panel.current?.contains(event.target)) {
        setOpen(false);
      }
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      button.current?.focus();
    };
    document.addEventListener('pointerdown', outside);
    window.addEventListener('keydown', escape, true);
    return () => {
      document.removeEventListener('pointerdown', outside);
      window.removeEventListener('keydown', escape, true);
    };
  }, [open]);

  return <>
    <button
      className="context-info-button"
      type="button"
      ref={button}
      aria-label={`About ${title}`}
      aria-expanded={open}
      aria-controls={open ? id : undefined}
      aria-describedby={open ? id : undefined}
      onClick={() => setOpen(value => !value)}
      onBlur={() => setOpen(false)}
    >
      <Info size={15} aria-hidden="true" />
    </button>
    {open && createPortal(<div className="context-info-panel" role="note" ref={panel} id={id} style={position}>{children}</div>, document.body)}
  </>;
}
