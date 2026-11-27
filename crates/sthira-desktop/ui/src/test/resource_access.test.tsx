import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SharedAccessDialog } from '../components/SharedAccessDialog';

const api = vi.hoisted(() => ({
  getResourceAccess: vi.fn(),
  listAccessMembers: vi.fn(),
  listSharedProjects: vi.fn(),
  previewResourceAccess: vi.fn(),
  commitResourceAccess: vi.fn(),
}));

vi.mock('../api/workspaces', async (original) => ({
  ...(await original<typeof import('../api/workspaces')>()),
  getResourceAccess: api.getResourceAccess,
  listAccessMembers: api.listAccessMembers,
  listSharedProjects: api.listSharedProjects,
  previewResourceAccess: api.previewResourceAccess,
  commitResourceAccess: api.commitResourceAccess,
}));

const ok = (data: unknown) => ({ outcome: 'ok', data });

const policy = {
  resourceKind: 'page',
  resourceId: 'page-a',
  accessMode: 'workspace',
  accessProjectId: null,
  accessProjectName: null,
  accessVersion: 1,
  audience: [],
  canManage: true,
  administrativeAccess: true,
  administrativeDisclosure:
    'Owners and Admins can always open restricted projects and pages, their history, and full workspace backups. Restricting a resource does not hide it from workspace administrators.',
  followingPages: [],
};

const members = [
  { membershipId: 'mem-b', uid: 'uid-b', displayName: 'Bea Member', role: 'editor' },
  { membershipId: 'mem-c', uid: 'uid-c', displayName: 'Cal Member', role: 'viewer' },
];

const preview = {
  previewId: 'sha_1',
  resourceKind: 'page',
  resourceId: 'page-a',
  currentMode: 'Workspace access',
  proposedMode: 'Selected members',
  currentAudience: [],
  proposedAudience: [members[0]],
  audienceChange: { added: [members[0]], removed: [] },
  followProjectId: null,
  followProjectName: null,
  affectedPages: [],
  conversionPageIds: [],
  administrativeDisclosure: policy.administrativeDisclosure,
  sharesHistory:
    "Sharing includes the page's readable revision history, discussions, proposals and other existing history, not only the current body.",
  expansionRequiresConfirmation: false,
  expiresAtMs: 1,
  baseAccessVersion: 1,
};

function mount() {
  const onCommitted = vi.fn();
  const onClose = vi.fn();
  render(
    <SharedAccessDialog
      workspaceId="ws"
      resourceKind="page"
      resourceId="page-a"
      resourceName="Restricted brief"
      onClose={onClose}
      onCommitted={onCommitted}
      onAccessLost={vi.fn()}
      onSessionChange={vi.fn()}
    />,
  );
  return { onCommitted, onClose };
}

describe('Shared resource access dialog', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    api.getResourceAccess.mockResolvedValue(ok(policy));
    api.listAccessMembers.mockResolvedValue(ok(members));
    api.listSharedProjects.mockResolvedValue(ok({items: [], nextCursor: null}));
    api.previewResourceAccess.mockResolvedValue(ok(preview));
    api.commitResourceAccess.mockResolvedValue(
      ok({ committed: true, policy: { ...policy, accessMode: 'selected', audience: [members[0]] }, changedPageIds: [] }),
    );
  });

  it('discloses administrative access and previews an explicit audience change', async () => {
    mount();
    expect(await screen.findByText(/Owners and Admins can always open restricted/)).toBeVisible();
    fireEvent.click(await screen.findByRole('radio', { name: /Selected members/ }));
    fireEvent.click(screen.getByRole('checkbox', { name: /Bea Member/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Preview change' }));
    await screen.findByText('Workspace access → Selected members');
    expect(api.previewResourceAccess).toHaveBeenCalledWith('ws', 'page', 'page-a', {
      proposedMode: 'selected',
      selectedMembershipIds: ['mem-b'],
      followProjectId: null,
      convertPageIds: [],
      confirmExpansion: false,
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save sharing' }));
    await waitFor(() => expect(api.commitResourceAccess).toHaveBeenCalledTimes(1));
  });

  it('requires explicit confirmation before widening an audience', async () => {
    api.getResourceAccess.mockResolvedValue(
      ok({ ...policy, accessMode: 'selected', audience: [members[0]] }),
    );
    api.previewResourceAccess.mockResolvedValue(
      ok({ ...preview, expansionRequiresConfirmation: true }),
    );
    mount();
    fireEvent.click(await screen.findByRole('radio', { name: /Workspace access/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Preview change' }));
    await screen.findByText('Workspace access → Selected members');
    fireEvent.click(screen.getByRole('button', { name: 'Save sharing' }));
    expect(await screen.findByText(/Confirm that this change shares/)).toBeVisible();
    expect(api.commitResourceAccess).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('checkbox', {name: /I understand/}));
    fireEvent.click(screen.getByRole('button', {name: 'Save sharing'}));
    await waitFor(() => expect(api.commitResourceAccess).toHaveBeenCalledWith('ws', 'page', 'page-a', 'sha_1', expect.any(String), true));
  });
  it('loads every page of projects for the access-following selector', async () => {
    api.listSharedProjects.mockResolvedValueOnce(ok({items:[{projectId:'pr-1',name:'First project'}],nextCursor:'next'}))
      .mockResolvedValueOnce(ok({items:[{projectId:'pr-2',name:'Second project'}],nextCursor:null}));
    mount();
    fireEvent.click(await screen.findByRole('radio', {name:/Access follows a project/}));
    fireEvent.change(screen.getByRole('combobox'), {target:{value:'pr-2'}});
    fireEvent.click(screen.getByRole('button',{name:'Preview change'}));
    await waitFor(() => expect(api.previewResourceAccess).toHaveBeenCalledWith('ws','page','page-a',expect.objectContaining({followProjectId:'pr-2'})));
    expect(api.listSharedProjects).toHaveBeenLastCalledWith('ws',false,null,'next',100);
  });

  it('keeps the same commit key after an uncertain response and uses a new key for a fresh preview', async () => {
    api.commitResourceAccess.mockRejectedValueOnce(new Error('response lost'));
    const view = mount();
    fireEvent.click(await screen.findByRole('button',{name:'Preview change'}));
    fireEvent.click(await screen.findByRole('button',{name:'Save sharing'}));
    await screen.findByText(/result could not be confirmed/);
    const key = api.commitResourceAccess.mock.calls[0][4];
    fireEvent.click(screen.getByRole('button',{name:'Save sharing'}));
    await waitFor(() => expect(view.onCommitted).toHaveBeenCalledOnce());
    expect(api.commitResourceAccess.mock.calls[1][4]).toBe(key);
    fireEvent.click(screen.getByRole('button',{name:'Preview change'}));
    fireEvent.click(await screen.findByRole('button',{name:'Save sharing'}));
    await waitFor(() => expect(api.commitResourceAccess).toHaveBeenCalledTimes(3));
    expect(api.commitResourceAccess.mock.calls[2][4]).not.toBe(key);
  });

});
