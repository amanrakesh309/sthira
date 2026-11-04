import { afterEach, expect, it, vi } from 'vitest';
const original = (window as any).__TAURI_INTERNALS__;
afterEach(() => { (window as any).__TAURI_INTERNALS__ = original; vi.resetModules(); });
it('does not present fabricated vault records in browser mode', async () => {
  delete (window as any).__TAURI_INTERNALS__;
  vi.resetModules();
  const ipc = await import('../api/ipc');
  expect(await ipc.getWorkspaceIndex('all')).toEqual({ counts: { all: 0, projects: 0, notes: 0, objects: 0, standalone: 0 }, items: [] });
  expect(await ipc.getSidebarItems()).toEqual({ projects: [], notes: [], objects: [] });
  expect(await ipc.getWorkspaceTree()).toEqual({ projects: [], unassociated_pages: [] });
});
