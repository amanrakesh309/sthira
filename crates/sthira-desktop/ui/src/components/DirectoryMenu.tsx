import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { ReactNode } from 'react';

export interface DirectoryMenuItem {
  id: string;
  label: string;
  icon?: ReactNode;
  testId?: string;
  danger?: boolean;
  onSelect: () => void;
}

/** Small row menu, portaled so table edges never clip its actions. */
export function DirectoryMenu({ label, icon, items, testId, disabled = false }: {
  label: string;
  icon: ReactNode;
  items: DirectoryMenuItem[];
  testId?: string;
  disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState({ top: 0, left: 0 });
  const trigger = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const menuId = useId();
  const close = (restoreFocus = true) => {
    setOpen(false);
    if (restoreFocus) trigger.current?.focus();
  };

  useLayoutEffect(() => {
    if (!open) return;
    const place = () => {
      if (!trigger.current || !panel.current) return;
      const anchor = trigger.current.getBoundingClientRect();
      const box = panel.current.getBoundingClientRect();
      setPosition({
        left: Math.max(8, Math.min(anchor.right - box.width, window.innerWidth - box.width - 8)),
        top: Math.max(8, anchor.bottom + box.height + 8 > window.innerHeight ? anchor.top - box.height - 4 : anchor.bottom + 4),
      });
    };
    place();
    panel.current?.querySelector<HTMLButtonElement>('button')?.focus();
    window.addEventListener('resize', place);
    window.addEventListener('scroll', place, true);
    return () => { window.removeEventListener('resize', place); window.removeEventListener('scroll', place, true); };
  }, [open, items.length]);

  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (!trigger.current?.contains(event.target as Node) && !panel.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  }, [open]);

  return <>
    <button ref={trigger} type="button" className="directory-icon-button" aria-label={label} title={label}
      data-testid={testId} aria-haspopup="menu" aria-expanded={open} aria-controls={open ? menuId : undefined}
      disabled={disabled} onClick={(event) => { event.stopPropagation(); setOpen(!open); }}
      onKeyDown={(event) => {
        event.stopPropagation();
        if (event.key === 'ArrowDown') { event.preventDefault(); setOpen(true); }
      }}>{icon}</button>
    {open && createPortal(<div ref={panel} id={menuId} role="menu" aria-label={label}
      className="directory-popover" style={position} onClick={(event) => event.stopPropagation()}
      onKeyDown={(event) => {
        event.stopPropagation();
        const buttons = Array.from(panel.current?.querySelectorAll<HTMLButtonElement>('button') || []);
        const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
        if (event.key === 'Escape') { event.preventDefault(); close(); }
        if (event.key === 'Tab') { close(); return; }
        if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
          event.preventDefault();
          const next = event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1
            : (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length;
          buttons[next]?.focus();
        }
      }}>
      {items.length === 0 ? <p>No projects available.</p> : items.map(item => <button type="button" role="menuitem"
        key={item.id} data-testid={item.testId} className={item.danger ? 'directory-menu-danger' : undefined}
        onClick={() => { close(); item.onSelect(); }}>{item.icon}<span>{item.label}</span></button>)}
    </div>, document.body)}
  </>;
}
