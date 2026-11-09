import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
const tauriWindow = window as unknown as { __TAURI_INTERNALS__?: unknown };
const original = tauriWindow.__TAURI_INTERNALS__;

beforeEach(() => { vi.resetModules(); invoke.mockReset(); });
afterEach(() => { tauriWindow.__TAURI_INTERNALS__ = original; vi.resetModules(); });

describe('Unconfigured provider catalogs', () => {
  it('has no sample providers or models in a fresh browser preview', async () => {
    delete tauriWindow.__TAURI_INTERNALS__;
    const api = await import('../api/ipc');
    expect(api.isTauri).toBe(false);
    expect(await api.getProviderCatalog()).toEqual([]);
    expect(await api.listAvailableModels()).toEqual([]);
    expect(invoke).not.toHaveBeenCalled();
  });

  it('uses the desktop catalog exactly as returned without adding a default connection', async () => {
    tauriWindow.__TAURI_INTERNALS__ = {};
    invoke.mockResolvedValue([]);
    const api = await import('../api/ipc');
    expect(await api.getProviderCatalog()).toEqual([]);
    expect(await api.listAvailableModels()).toEqual([]);
    expect(invoke.mock.calls.map(call => call[0])).toEqual(['cmd_get_provider_catalog', 'cmd_list_available_models']);
  });
});
