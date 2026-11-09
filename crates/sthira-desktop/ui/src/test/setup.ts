import '@testing-library/jest-dom/vitest';
import { vi, beforeEach } from 'vitest';
beforeEach(() => { localStorage.removeItem('sthira.tabs.v1:preview'); });

// Existing interaction suites represent an explicitly configured workspace.
// Workflow settings/gating suites unmock this hook and test the fresh/off state,
// persistence, updates and failures through the real hook and API boundary.
vi.mock('../hooks/useWorkflowSettings', () => {
  const settings = Object.fromEntries(['research', 'plan', 'draft', 'chat', 'resume'].map(id => [id, { enabled: true, model_id: null, templates_enabled: true }]));
  return { useWorkflowSettings: () => ({ settings, loading: false, error: null, retry: vi.fn() }) };
});

// ProseMirror measures selections; jsdom supplies no layout geometry.
const testRect = () => ({ x: 0, y: 0, width: 0, height: 0, top: 0, right: 0, bottom: 0, left: 0, toJSON: () => ({}) }) as DOMRect;
Range.prototype.getBoundingClientRect = testRect;
Range.prototype.getClientRects = () => ({ length: 0, item: () => null, [Symbol.iterator]: function* () {} }) as DOMRectList;
if (!document.elementFromPoint) document.elementFromPoint = () => null;
if (!HTMLElement.prototype.scrollIntoView) HTMLElement.prototype.scrollIntoView = () => {};

// Enable isTauri flag in ipc.ts
(window as unknown as { __TAURI_INTERNALS__: Record<string, unknown> }).__TAURI_INTERNALS__ = {};

// Mock HTMLCanvasElement.prototype.getContext for 2D canvas testing in jsdom
HTMLCanvasElement.prototype.getContext = function () {
  return {
    save: () => {},
    restore: () => {},
    scale: () => {},
    translate: () => {},
    clearRect: () => {},
    beginPath: () => {},
    arc: () => {},
    fill: () => {},
    stroke: () => {},
    moveTo: () => {},
    lineTo: () => {},
    strokeRect: () => {},
    fillRect: () => {},
    fillText: () => {},
    measureText: (text: string) => ({ width: (text || '').length * 6 }),
    setLineDash: () => {},
  } as unknown as CanvasRenderingContext2D;
} as unknown as typeof HTMLCanvasElement.prototype.getContext;

