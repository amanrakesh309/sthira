import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useDocumentTabs } from '../hooks/useDocumentTabs';

const mocks = vi.hoisted(() => ({
  key: vi.fn(),
  existing: vi.fn(),
}));

vi.mock('../api/workspaceSession', () => ({
  getWorkspaceSessionKey: mocks.key,
}));

vi.mock('../api/ipc', () => ({
  filterExistingArtifacts: mocks.existing,
}));

const previous = {
  tabs: [
    {
      id: 'old-page',
      title: 'Previous vault page',
      projectId: null,
      returnProjectId: null,
      scrollTop: 0,
    },
  ],
  activeId: 'old-page',
  resume: true,
};

describe('useDocumentTabs vault reset isolation & epoch ownership (R1, R2)', () => {
  beforeEach(() => {
    localStorage.clear();
    sessionStorage.clear();
    mocks.key.mockReset().mockResolvedValue('current-vault');
    mocks.existing.mockReset().mockResolvedValue(['old-page']);
  });

  it('R1: reset clears only current vault tabs and chat drafts while preserving other vaults', async () => {
    const otherTabs = JSON.stringify(previous);
    localStorage.setItem('sthira.tabs.v1:current-vault', otherTabs);
    sessionStorage.setItem('sthira.chat-draft:current-vault:old-page', 'Current draft to clear');
    localStorage.setItem('sthira.tabs.v1:other-vault', otherTabs);
    sessionStorage.setItem('sthira.chat-draft:other-vault:old-page', 'Unsent draft in another vault');

    const hook = renderHook(() => useDocumentTabs(vi.fn()));
    await waitFor(() => expect(hook.result.current.namespace).toBe('current-vault'));
    expect(hook.result.current.state.tabs).toHaveLength(1);

    mocks.key.mockResolvedValue('replacement-vault');
    await act(async () => {
      window.dispatchEvent(new Event('sthira:vault-reset'));
    });
    await waitFor(() => expect(hook.result.current.namespace).toBe('replacement-vault'));

    expect(localStorage.getItem('sthira.tabs.v1:current-vault')).toBeNull();
    expect(sessionStorage.getItem('sthira.chat-draft:current-vault:old-page')).toBeNull();
    expect(localStorage.getItem('sthira.tabs.v1:other-vault')).toBe(otherTabs);
    expect(sessionStorage.getItem('sthira.chat-draft:other-vault:old-page')).toBe(
      'Unsent draft in another vault'
    );
    expect(hook.result.current.state.tabs).toEqual([]);
  });

  it('R1/R2: delayed initial session-key resolution after reset cannot overwrite replacement vault or resurrect old tabs', async () => {
    let releaseInitialKey!: (key: string) => void;
    let releaseResetKey!: (key: string) => void;
    mocks.key
      .mockImplementationOnce(() => new Promise<string>((resolve) => { releaseInitialKey = resolve; }))
      .mockImplementationOnce(() => new Promise<string>((resolve) => { releaseResetKey = resolve; }));

    const otherTabs = JSON.stringify(previous);
    localStorage.setItem('sthira.tabs.v1:current-vault', otherTabs);
    sessionStorage.setItem('sthira.chat-draft:current-vault:old-page', 'Old unsent draft');
    localStorage.setItem('sthira.tabs.v1:other-vault', otherTabs);
    sessionStorage.setItem('sthira.chat-draft:other-vault:old-page', 'Other vault unsent draft');

    const onRestore = vi.fn();
    const hook = renderHook(() => useDocumentTabs(onRestore));
    await waitFor(() => expect(mocks.key).toHaveBeenCalledTimes(1));

    // Reset happens before initial getWorkspaceSessionKey() ever resolved
    await act(async () => {
      window.dispatchEvent(new Event('sthira:vault-reset'));
    });
    await waitFor(() => expect(mocks.key).toHaveBeenCalledTimes(2));

    // Now the stale initial session-key promise resolves first
    await act(async () => {
      releaseInitialKey('current-vault');
    });

    expect(hook.result.current.namespace).toBe('');
    expect(hook.result.current.state.tabs).toEqual([]);
    expect(onRestore).not.toHaveBeenCalled();
    expect(localStorage.getItem('sthira.tabs.v1:current-vault')).toBeNull();
    expect(sessionStorage.getItem('sthira.chat-draft:current-vault:old-page')).toBeNull();
    expect(localStorage.getItem('sthira.tabs.v1:other-vault')).toBe(otherTabs);
    expect(sessionStorage.getItem('sthira.chat-draft:other-vault:old-page')).toBe(
      'Other vault unsent draft'
    );

    // Then the replacement vault key resolves
    await act(async () => {
      releaseResetKey('replacement-vault');
    });
    await waitFor(() => expect(hook.result.current.namespace).toBe('replacement-vault'));
    expect(hook.result.current.state.tabs).toEqual([]);
    expect(JSON.parse(localStorage.getItem('sthira.tabs.v1:replacement-vault')!).tabs).toEqual([]);
  });

  it('R2: reset invalidates in-flight pre-reset existence validation without timing sleeps', async () => {
    let releaseExisting!: (ids: string[]) => void;
    mocks.existing.mockReturnValue(
      new Promise<string[]>((resolve) => {
        releaseExisting = resolve;
      })
    );
    const otherTabs = JSON.stringify(previous);
    localStorage.setItem('sthira.tabs.v1:current-vault', JSON.stringify(previous));
    sessionStorage.setItem('sthira.chat-draft:current-vault:old-page', 'Old draft');
    localStorage.setItem('sthira.tabs.v1:other-vault', otherTabs);

    const onRestore = vi.fn();
    const hook = renderHook(() => useDocumentTabs(onRestore));
    await waitFor(() => expect(mocks.existing).toHaveBeenCalledWith(['old-page']));

    mocks.key.mockResolvedValue('replacement-vault');
    await act(async () => {
      window.dispatchEvent(new Event('sthira:vault-reset'));
    });
    await waitFor(() => expect(hook.result.current.namespace).toBe('replacement-vault'));

    // Release delayed pre-reset validation response
    await act(async () => {
      releaseExisting(['old-page']);
    });

    expect(hook.result.current.namespace).toBe('replacement-vault');
    expect(hook.result.current.state.tabs).toEqual([]);
    expect(onRestore).not.toHaveBeenCalledWith(previous);
    expect(JSON.parse(localStorage.getItem('sthira.tabs.v1:replacement-vault')!).tabs).toEqual([]);
    expect(localStorage.getItem('sthira.tabs.v1:current-vault')).toBeNull();
    expect(sessionStorage.getItem('sthira.chat-draft:current-vault:old-page')).toBeNull();
    expect(localStorage.getItem('sthira.tabs.v1:other-vault')).toBe(otherTabs);
  });

  it('R2: superseded delayed reset key cannot overwrite a newer reset session key', async () => {
    let releaseReset1!: (key: string) => void;
    let releaseReset2!: (key: string) => void;
    const hook = renderHook(() => useDocumentTabs(vi.fn()));
    await waitFor(() => expect(hook.result.current.namespace).toBe('current-vault'));

    mocks.key
      .mockImplementationOnce(() => new Promise<string>((resolve) => { releaseReset1 = resolve; }))
      .mockImplementationOnce(() => new Promise<string>((resolve) => { releaseReset2 = resolve; }));

    await act(async () => {
      window.dispatchEvent(new Event('sthira:vault-reset'));
      window.dispatchEvent(new Event('sthira:vault-reset'));
    });

    await act(async () => {
      releaseReset2('vault-after-second-reset');
    });
    await waitFor(() => expect(hook.result.current.namespace).toBe('vault-after-second-reset'));

    await act(async () => {
      releaseReset1('stale-first-reset-vault');
    });

    expect(hook.result.current.namespace).toBe('vault-after-second-reset');
    expect(localStorage.getItem('sthira.tabs.v1:stale-first-reset-vault')).toBeNull();
  });
});
