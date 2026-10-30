import '@testing-library/jest-dom/vitest';

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

