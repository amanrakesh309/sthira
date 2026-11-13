import { useEffect, useRef } from 'react';
import type { RefObject } from 'react';

export interface DialogFocusOptions {
  /** Extra containers (such as portaled popovers owned by this dialog) included in focus & Tab wrapping. */
  extraContainers?: () => (HTMLElement | null)[];
  /** Element to focus when the dialog opens if focus is not already inside it. */
  initialFocus?: RefObject<HTMLElement | null>;
  /** Explicit opener reference to restore focus to on close when still connected. */
  restoreFocusTarget?: RefObject<HTMLElement | null>;
  /** Optional predicate controlling whether closing should restore focus to the opener. */
  shouldRestoreFocus?: () => boolean;
}

const FOCUSABLE_SELECTOR =
  'button:not(:disabled), input:not(:disabled):not([type="hidden"]), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex]:not([tabindex="-1"])';

function collectFocusable(containers: (HTMLElement | null)[]): HTMLElement[] {
  const seen = new Set<HTMLElement>();
  const result: HTMLElement[] = [];
  for (const container of containers) {
    if (!container) continue;
    for (const el of container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)) {
      if (seen.has(el)) continue;
      if (el.hasAttribute('disabled') || el.tabIndex < 0) continue;
      if (el.closest('[hidden], [inert], [aria-hidden="true"]')) continue;
      const details = el.closest('details');
      if (details && !details.open && !el.closest('summary')) continue;
      const style = typeof window !== 'undefined' ? window.getComputedStyle(el) : null;
      if (style && (style.display === 'none' || style.visibility === 'hidden')) continue;
      seen.add(el);
      result.push(el);
    }
  }
  return result;
}

/** Focus, Tab trapping, and Escape dismissal for dialogs; captures the opener before children autofocus. */
export function useDialogFocus(
  open: boolean,
  panel: RefObject<HTMLElement | null>,
  onClose: () => void,
  options?: DialogFocusOptions
) {
  const close = useRef(onClose);
  close.current = onClose;
  const optsRef = useRef(options);
  optsRef.current = options;
  const wasOpen = useRef(false);
  const returnFocus = useRef<HTMLElement | null>(null);
  // Capture before descendants commit autofocus, so closing returns to the opener.
  if (open && !wasOpen.current && typeof document !== 'undefined') returnFocus.current = document.activeElement as HTMLElement;
  wasOpen.current = open;
  useEffect(() => {
    if (!open) return;
    const previous = returnFocus.current;
    const allContainers = () => [panel.current, ...(optsRef.current?.extraContainers?.() || [])];
    const elements = () => collectFocusable(allContainers());
    const insideContainers = (target: Element | null) =>
      Boolean(target && allContainers().some((c) => c && c.contains(target)));

    if (!insideContainers(document.activeElement)) {
      const target = optsRef.current?.initialFocus?.current || elements()[0];
      target?.focus();
    }
    const keys = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        close.current();
        return;
      }
      if (event.key === 'Tab') {
        const controls = elements();
        if (controls.length === 0) {
          event.preventDefault();
          return;
        }
        const active = document.activeElement as HTMLElement | null;
        if (!active || !insideContainers(active)) {
          event.preventDefault();
          (event.shiftKey ? controls[controls.length - 1] : controls[0])?.focus();
          return;
        }
        if (event.shiftKey && active === controls[0]) {
          event.preventDefault();
          controls[controls.length - 1]?.focus();
        } else if (!event.shiftKey && active === controls[controls.length - 1]) {
          event.preventDefault();
          controls[0]?.focus();
        }
      }
    };
    document.addEventListener('keydown', keys);
    return () => {
      document.removeEventListener('keydown', keys);
      if (optsRef.current?.shouldRestoreFocus && !optsRef.current.shouldRestoreFocus()) return;
      const explicit = optsRef.current?.restoreFocusTarget?.current;
      const target = explicit && explicit.isConnected ? explicit : previous;
      if (target?.isConnected && typeof target.focus === 'function') target.focus();
    };
  }, [open, panel]);
}
