import { useLayoutEffect, useState } from 'react';
import type { RefObject } from 'react';

export function useAnchoredPopover(open: boolean, anchor: RefObject<HTMLElement | null>, panel: RefObject<HTMLElement | null>, revision: string) {
  const [position, setPosition] = useState({ top: 0, left: 0 });
  useLayoutEffect(() => {
    if (!open) return;
    const place = () => {
      if (!anchor.current || !panel.current) return;
      const a = anchor.current.getBoundingClientRect();
      const p = panel.current.getBoundingClientRect();
      const next = {
        left: Math.max(8, Math.min(a.right - p.width, window.innerWidth - p.width - 8)),
        top: Math.max(8, a.bottom + p.height + 8 > window.innerHeight ? a.top - p.height - 4 : a.bottom + 4),
      };
      setPosition(previous => previous.top === next.top && previous.left === next.left ? previous : next);
    };
    place();
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(place);
    if (panel.current) observer?.observe(panel.current);
    window.addEventListener('scroll', place, true);
    window.addEventListener('resize', place);
    return () => { observer?.disconnect(); window.removeEventListener('scroll', place, true); window.removeEventListener('resize', place); };
  }, [open, anchor, panel, revision]);
  return position;
}
