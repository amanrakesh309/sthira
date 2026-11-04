import { useEffect, useRef } from 'react';
import type { RefObject } from 'react';

/** Focus and dismissal for the scoped object dialogs; the first input can request autofocus. */
export function useDialogFocus(open: boolean, panel: RefObject<HTMLElement | null>, onClose: () => void) {
  const close = useRef(onClose);
  close.current = onClose;
  const wasOpen = useRef(false);
  const returnFocus = useRef<HTMLElement | null>(null);
  // Capture before descendants commit autofocus, so closing returns to the opener.
  if (open && !wasOpen.current && typeof document !== 'undefined') returnFocus.current = document.activeElement as HTMLElement;
  wasOpen.current = open;
  useEffect(() => {
    if (!open) return;
    const previous = returnFocus.current;
    const elements = () => Array.from(panel.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex="0"]') || []);
    if (!panel.current?.contains(document.activeElement)) elements()[0]?.focus();
    const keys = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close.current(); }
      if (event.key === 'Tab') {
        const controls = elements();
        if (event.shiftKey && document.activeElement === controls[0]) { event.preventDefault(); controls[controls.length - 1]?.focus(); }
        if (!event.shiftKey && document.activeElement === controls[controls.length - 1]) { event.preventDefault(); controls[0]?.focus(); }
      }
    };
    document.addEventListener('keydown', keys);
    return () => { document.removeEventListener('keydown', keys); if (previous?.isConnected) previous.focus(); };
  }, [open, panel]);
}
