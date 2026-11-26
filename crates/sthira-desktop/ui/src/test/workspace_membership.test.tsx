import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';

const tauriMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: tauriMocks.invoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}));

vi.mock('../api/ipc', async (original) => ({
  ...await original<typeof import('../api/ipc')>(),
  isTauri: true,
}));

import { App } from '../App';
import { WorkspaceSwitcher } from '../components/WorkspaceSwitcher';
import { CreateWorkspaceModal } from '../components/CreateWorkspaceModal';
import { JoinWorkspaceModal } from '../components/JoinWorkspaceModal';
import { TeamWorkspaceView, type TeamWorkspaceTab } from '../components/TeamWorkspaceView';
import type { GoogleSession } from '../components/AccountModal';
import type {
  WorkspaceDetails,
  WorkspaceResponse,
  WorkspaceSummary,
} from '../api/workspaces';

const signedInSession: GoogleSession = {
  status: 'signedIn',
  email: 'alice@example.com',
  currentSessionId: 'sid-a',
  currentDeviceLabel: 'Workstation A',
  devices: [],
};

const wsOneOwner: WorkspaceSummary = {
  workspaceId: 'ws-1',
  name: 'Workspace One',
  myMembershipId: 'mem-a',
  myRole: 'owner',
  createdAtMs: 1_795_000_000_000,
  updatedAtMs: 1_795_000_000_000,
};

const wsOneDetailsOwner: WorkspaceDetails = {
  workspace: wsOneOwner,
  members: [
    {
      membershipId: 'mem-a',
      uid: 'uid-a',
      displayName: 'Alice Owner',
      role: 'owner',
      joinedAtMs: 1_795_000_000_000,
      isCurrentUser: true,
    },
    {
      membershipId: 'mem-b',
      uid: 'uid-b',
      displayName: 'Bob Admin',
      role: 'admin',
      joinedAtMs: 1_795_000_100_000,
      isCurrentUser: false,
    },
    {
      membershipId: 'mem-c',
      uid: 'uid-c',
      displayName: 'Cara Viewer',
      role: 'viewer',
      joinedAtMs: 1_795_000_200_000,
      isCurrentUser: false,
    },
  ],
  invitations: [
    {
      invitationId: 'inv-1',
      workspaceId: 'ws-1',
      recipientEmail: 'dan@example.com',
      offeredRole: 'editor',
      status: 'pending',
      invitedByMembershipId: 'mem-a',
      invitedByDisplayName: 'Alice Owner',
      createdAtMs: 1_795_000_300_000,
      expiresAtMs: 1_795_604_800_000,
    },
  ],
  pendingTransfer: null,
};

describe('CW1 Workspace Membership UI', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('keeps the current selection when older workspace list and detail requests resolve', async () => {
    const secondWorkspace = { ...wsOneOwner, workspaceId: 'ws-2', name: 'Workspace Two' };
    const workspaceList = { outcome: 'ok', data: { workspaces: [wsOneOwner, secondWorkspace], incomingTransfers: [] } };
    const details = { outcome: 'ok', data: wsOneDetailsOwner };
    let listRequests = 0;
    let completeList!: (value: typeof workspaceList) => void;
    let completeDetails!: (value: typeof details) => void;
    tauriMocks.invoke.mockImplementation(async (command, args) => {
      switch (command) {
        case 'cmd_get_google_session': return signedInSession;
        case 'cmd_get_active_workspace': return 'ws-1';
        case 'cmd_list_workspaces':
          if (listRequests++ === 0) return workspaceList;
          return new Promise<typeof workspaceList>((resolve) => { completeList = resolve; });
        case 'cmd_get_workspace':
          if (args.workspaceId === 'ws-1') {
            return new Promise<typeof details>((resolve) => { completeDetails = resolve; });
          }
          return { outcome: 'ok', data: { ...wsOneDetailsOwner, workspace: secondWorkspace } };
        case 'cmd_get_home_state': return { projects: [], recent_items: [], loose_captures: [] };
        case 'cmd_get_sidebar_items': return { projects: [], notes: [], objects: [] };
        case 'cmd_get_workspace_tree': return { projects: [], unassociated_pages: [] };
        case 'cmd_get_object_type_counts': return {};
        default: return [];
      }
    });

    const user = userEvent.setup();
    const welcomeKey = 'sthira.guide.welcome.v1';
    localStorage.setItem(welcomeKey, 'seen');
    try {
      render(<App />);
      await user.click(await screen.findByRole('button', { name: 'Workspace switcher: Workspace One (Owner)' }));
      await user.click(screen.getByRole('menuitem', { name: 'Switch to Workspace Two (Owner)' }));
      expect(await screen.findByRole('heading', { name: 'Workspace Two', level: 1 })).toBeInTheDocument();

      await act(async () => {
        completeList(workspaceList);
        completeDetails(details);
      });
      expect(screen.getByRole('heading', { name: 'Workspace Two', level: 1 })).toBeInTheDocument();
      expect(screen.queryByRole('heading', { name: 'Workspace One' })).not.toBeInTheDocument();
      await user.keyboard('{Control>}k{/Control}');
      expect(screen.queryByTestId('search-workspace-dialog')).not.toBeInTheDocument();
    } finally {
      localStorage.removeItem(welcomeKey);
    }
  });

  it('supports keyboard Escape/focus restore on switcher, preserves form input on Create retry, and switches into new empty workspace', async () => {
    let createAttempts = 0;
    const seenKeys = new Set<string>();
    tauriMocks.invoke.mockImplementation(async (cmd, args: any) => {
      if (cmd === 'cmd_create_workspace') {
        createAttempts += 1;
        seenKeys.add(args.idempotencyKey);
        if (createAttempts === 1) {
          return {
            outcome: 'error',
            code: 'service_unavailable',
            message: 'Session service temporarily unavailable.',
          } satisfies WorkspaceResponse<never>;
        }
        return {
          outcome: 'ok',
          data: { created: true, workspace: wsOneOwner },
        };
      }
      if (cmd === 'cmd_get_workspace') {
        return { outcome: 'ok', data: wsOneDetailsOwner };
      }
      return { outcome: 'ok', data: null };
    });

    function SwitcherHarness() {
      const [active, setActive] = useState<WorkspaceSummary | null>(null);
      const [workspaces, setWorkspaces] = useState<WorkspaceSummary[]>([]);
      const [createOpen, setCreateOpen] = useState(false);
      return (
        <div>
          <WorkspaceSwitcher
            activeWorkspace={active}
            workspaces={workspaces}
            incomingTransfers={[]}
            session={signedInSession}
            onSelectPersonal={() => setActive(null)}
            onSelectWorkspace={(ws) => setActive(ws)}
            onOpenCreate={() => setCreateOpen(true)}
            onOpenJoin={vi.fn()}
            onOpenAccount={vi.fn()}
          />
          <CreateWorkspaceModal
            isOpen={createOpen}
            session={signedInSession}
            onSessionChange={vi.fn()}
            onClose={() => setCreateOpen(false)}
            onOpenAccount={vi.fn()}
            onCreated={(ws) => {
              setCreateOpen(false);
              setWorkspaces([ws]);
              setActive(ws);
            }}
          />
          {active && (
            <TeamWorkspaceView
              workspace={active}
              onSelectPersonal={() => setActive(null)}
              onWorkspaceUpdated={setActive}
              onAccessLost={vi.fn()}
              onSessionChange={vi.fn()}
              onRefreshList={vi.fn()}
            />
          )}
        </div>
      );
    }

    const user = userEvent.setup();
    render(<SwitcherHarness />);

    const trigger = screen.getByTestId('workspace-switcher-trigger');
    await user.click(trigger);
    expect(screen.getByRole('menu', { name: 'Switch workspace' })).toBeInTheDocument();
    await user.keyboard('{ArrowUp}');
    expect(screen.getByRole('menuitem', { name: 'Join workspace' })).toHaveFocus();

    // Escape closes switcher menu and restores focus to trigger.
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();

    await user.click(trigger);
    await user.click(screen.getByRole('menuitem', { name: 'Create workspace' }));

    const dialog = screen.getByRole('dialog', { name: 'Create workspace' });
    const nameInput = within(dialog).getByLabelText('Workspace name');
    await user.type(nameInput, 'Workspace One');
    await user.click(within(dialog).getByRole('button', { name: 'Create workspace' }));

    // Retryable failure preserves typed name and keeps same idempotencyKey for retry.
    expect(await within(dialog).findByRole('alert')).toHaveTextContent(/unavailable/i);
    expect(nameInput).toHaveValue('Workspace One');

    await user.click(within(dialog).getByRole('button', { name: 'Create workspace' }));
    expect(
      await screen.findByRole('heading', { name: 'Workspace One', level: 1 }),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Invite people' })).toBeInTheDocument();
    expect(seenKeys.size).toBe(1);
  });

  it('Join workspace blocks wrong account without revealing workspace details, preserves code, and joins after explicit Accept', async () => {
    let previewCall = 0;
    tauriMocks.invoke.mockImplementation(async (cmd, args: any) => {
      if (cmd === 'cmd_preview_invitation') {
        previewCall += 1;
        expect(args).toEqual({ inviteCode: 'sthi_secret_invite_code' });
        if (previewCall === 1) {
          return {
            outcome: 'error',
            code: 'invitation_wrong_account',
            message: 'This invitation belongs to a different Google account.',
          };
        }
        return {
          outcome: 'ok',
          data: {
            invitationId: 'inv-1',
            workspaceId: 'ws-1',
            workspaceName: 'Workspace One',
            offeredRole: 'viewer',
            expiresAtMs: 1_795_604_800_000,
            alreadyMember: false,
            existingRole: null,
          },
        };
      }
      if (cmd === 'cmd_accept_invitation') {
        expect(args).toEqual({ inviteCode: 'sthi_secret_invite_code' });
        return {
          outcome: 'ok',
          data: {
            alreadyMember: false,
            workspace: { ...wsOneOwner, myRole: 'viewer', myMembershipId: 'mem-c' },
            membership: wsOneDetailsOwner.members[2],
          },
        };
      }
      return null;
    });

    const onJoined = vi.fn();
    const user = userEvent.setup();
    render(
      <JoinWorkspaceModal
        isOpen
        session={signedInSession}
        onSessionChange={vi.fn()}
        onJoined={onJoined}
        onClose={vi.fn()}
        onOpenAccount={vi.fn()}
      />,
    );

    const input = screen.getByLabelText('Invitation code');
    await user.type(input, 'sthi_secret_invite_code');
    await user.click(screen.getByRole('button', { name: 'Continue' }));

    // Wrong account reveals no workspace name/role, keeps invite code, and offers Switch Google account.
    expect(await screen.findByRole('alert')).toHaveTextContent(/different Google account/i);
    expect(screen.queryByText('Workspace One')).not.toBeInTheDocument();
    expect(input).toHaveValue('sthi_secret_invite_code');
    expect(screen.getByRole('button', { name: 'Switch Google account' })).toBeInTheDocument();

    // Retry with invited account reveals preview and requires explicit Accept invitation.
    await user.click(screen.getByRole('button', { name: 'Continue' }));
    expect(await screen.findByText('Workspace One')).toBeInTheDocument();
    expect(screen.getByText('Viewer')).toBeInTheDocument();
    expect(onJoined).not.toHaveBeenCalled();

    await user.click(screen.getByRole('button', { name: 'Accept invitation' }));
    await waitFor(() => {
      expect(onJoined).toHaveBeenCalledWith(
        expect.objectContaining({ workspaceId: 'ws-1', myRole: 'viewer' }),
      );
    });
  });

  it('enforces role controls by matrix, confirms role/remove/transfer actions, and falls back to Personal when removed', async () => {
    const adminWorkspace: WorkspaceSummary = {
      ...wsOneOwner,
      myMembershipId: 'mem-b',
      myRole: 'admin',
    };
    const adminDetails: WorkspaceDetails = {
      workspace: adminWorkspace,
      members: wsOneDetailsOwner.members.map((m) => ({
        ...m,
        isCurrentUser: m.uid === 'uid-b',
      })),
      invitations: wsOneDetailsOwner.invitations,
      pendingTransfer: null,
    };

    let removeCall = 0;
    tauriMocks.invoke.mockImplementation(async (cmd) => {
      if (cmd === 'cmd_get_workspace') {
        return { outcome: 'ok', data: adminDetails };
      }
      if (cmd === 'cmd_remove_member') {
        removeCall += 1;
        return {
          outcome: 'accessLost',
          message: 'You no longer have access to this workspace. Returned to Personal.',
          workspaces: { workspaces: [], incomingTransfers: [] },
        };
      }
      return { outcome: 'ok', data: null };
    });

    const onAccessLost = vi.fn();
    const user = userEvent.setup();
    render(
      <TeamWorkspaceView
        workspace={adminWorkspace}
        initialTab={'members' as TeamWorkspaceTab}
        onSelectPersonal={vi.fn()}
        onWorkspaceUpdated={vi.fn()}
        onAccessLost={onAccessLost}
        onSessionChange={vi.fn()}
        onRefreshList={vi.fn()}
      />,
    );

    // Wait for Members list loaded as Admin.
    expect(await screen.findByText('Alice Owner')).toBeInTheDocument();

    // Admin CANNOT invite Admin/Owner — only Editor & Viewer appear in invitation role select.
    const inviteRoleSelect = screen.getByLabelText('Invitation role') as HTMLSelectElement;
    const inviteOptions = Array.from(inviteRoleSelect.options).map((o) => o.value);
    expect(inviteOptions).toEqual(['editor', 'viewer']);

    // Admin CANNOT change Owner's role, remove Owner, or transfer ownership.
    expect(screen.queryByLabelText('Role for Alice Owner')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Remove Alice Owner' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Transfer ownership' })).not.toBeInTheDocument();

    // Admin CAN change/remove Cara Viewer, gated behind explicit confirmation.
    const caraSelect = screen.getByLabelText('Role for Cara Viewer') as HTMLSelectElement;
    expect(Array.from(caraSelect.options).map((o) => o.value)).toEqual(['editor', 'viewer']);

    await user.click(screen.getByRole('button', { name: 'Remove Cara Viewer' }));
    expect(screen.getByRole('region', { name: 'Confirm workspace action' })).toBeInTheDocument();
    expect(removeCall).toBe(0);

    // Executing an action after caller itself was removed returns AccessLost -> fallback to Personal!
    await user.click(screen.getByRole('button', { name: 'Confirm' }));
    await waitFor(() => {
      expect(onAccessLost).toHaveBeenCalledWith(
        expect.stringContaining('Returned to Personal'),
        expect.objectContaining({ workspaces: [] }),
      );
    });
  });

  it('hides management controls for Viewer, keeps Leave available, and blocks Leave for Owner', async () => {
    const viewerWorkspace: WorkspaceSummary = {
      ...wsOneOwner,
      myMembershipId: 'mem-c',
      myRole: 'viewer',
    };
    const viewerDetails: WorkspaceDetails = {
      workspace: viewerWorkspace,
      members: wsOneDetailsOwner.members.map((m) => ({
        ...m,
        isCurrentUser: m.uid === 'uid-c',
      })),
      invitations: [],
      pendingTransfer: null,
    };

    tauriMocks.invoke.mockImplementation(async (cmd) => {
      if (cmd === 'cmd_get_workspace') return { outcome: 'ok', data: viewerDetails };
      return null;
    });

    const user = userEvent.setup();
    const { rerender } = render(
      <TeamWorkspaceView
        workspace={viewerWorkspace}
        initialTab="members"
        onSelectPersonal={vi.fn()}
        onWorkspaceUpdated={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
        onRefreshList={vi.fn()}
      />,
    );

    expect(await screen.findByText('Alice Owner')).toBeInTheDocument();
    expect(screen.queryByLabelText('Invitations')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Save name' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Leave workspace' })).toBeEnabled();

    // Switch mock to Owner and verify Owner cannot leave until ownership is transferred.
    tauriMocks.invoke.mockImplementation(async (cmd) => {
      if (cmd === 'cmd_get_workspace') return { outcome: 'ok', data: wsOneDetailsOwner };
      return null;
    });
    rerender(
      <TeamWorkspaceView
        workspace={wsOneOwner}
        initialTab="members"
        onSelectPersonal={vi.fn()}
        onWorkspaceUpdated={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
        onRefreshList={vi.fn()}
      />,
    );
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Leave workspace' })).toBeDisabled();
    });
    expect(
      screen.getByText(/Transfer ownership to another member before leaving/i),
    ).toBeInTheDocument();
    await user.click(
      screen.getByRole('button', { name: 'Transfer ownership to Bob Admin' }),
    );
    expect(screen.getByRole('region', { name: 'Confirm workspace action' })).toHaveTextContent(
      /Bob Admin/i,
    );
  });

  it('does not refetch or wipe invite state when parent passes inline callbacks updating workspace state', async () => {
    let getCount = 0;
    tauriMocks.invoke.mockImplementation(async (cmd) => {
      if (cmd === 'cmd_get_workspace') {
        getCount += 1;
        return { outcome: 'ok', data: wsOneDetailsOwner };
      }
      if (cmd === 'cmd_create_invitation_batch') {
        return {
          outcome: 'ok',
          data: {
            offeredRole: 'admin',
            replayed: false,
            results: [
              {
                email: 'teammate@example.com',
                outcome: 'created',
                message: 'Invited as admin.',
                invitation: {
                  invitationId: 'inv_1',
                  workspaceId: 'ws_1',
                  recipientEmail: 'teammate@example.com',
                  offeredRole: 'admin',
                  status: 'pending',
                  invitedByMembershipId: 'mem_a',
                  invitedByDisplayName: 'Alice Owner',
                  createdAtMs: 1_700_000_001_000,
                  expiresAtMs: 1_700_604_801_000,
                  acceptedAtMs: null,
                  revokedAtMs: null,
                },
                inviteCode: 'sthi_regression_code_1',
                replayed: false,
              },
            ],
          },
        };
      }
      return null;
    });

    function ParentHarness() {
      const [ws, setWs] = useState<WorkspaceSummary>(wsOneOwner);
      return (
        <TeamWorkspaceView
          workspace={ws}
          initialTab="members"
          onSelectPersonal={() => {}}
          onWorkspaceUpdated={(updated) => setWs(updated)}
          onAccessLost={() => {}}
          onSessionChange={() => {}}
          onRefreshList={async () => {}}
        />
      );
    };

    const user = userEvent.setup();
    render(<ParentHarness />);
    expect(await screen.findByText('Members (3)')).toBeInTheDocument();
    expect(getCount).toBe(1);

    await user.selectOptions(screen.getByLabelText('Invitation role'), 'admin');
    await user.type(screen.getByLabelText('Recipient email addresses'), 'teammate@example.com');
    await user.click(screen.getByRole('button', { name: 'Send workspace invitations' }));

    expect(await screen.findByTestId('generated-invite-code')).toHaveTextContent(
      'sthi_regression_code_1',
    );
    expect(getCount).toBe(2);
  });
});
