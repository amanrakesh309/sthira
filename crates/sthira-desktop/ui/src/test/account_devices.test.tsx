import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../api/ipc', () => ({
  isTauri: true,
}));

import { invoke } from '@tauri-apps/api/core';
import {
  AccountModal,
  type GoogleSession,
  type RevokeDeviceOutcome,
} from '../components/AccountModal';

const twoDevicesSession: GoogleSession = {
  status: 'signedIn',
  email: 'qa-user@example.invalid',
  currentSessionId: 'sid-a',
  currentDeviceLabel: 'Workstation A (Linux · 11aa)',
  devices: [
    {
      sessionId: 'sid-a',
      deviceId: 'dev-a',
      deviceLabel: 'Workstation A (Linux · 11aa)',
      createdAtMs: 1_795_000_000_000,
      lastActiveAtMs: 1_795_001_000_000,
      expiresAtMs: 1_797_592_000_000,
      isCurrent: true,
    },
    {
      sessionId: 'sid-b',
      deviceId: 'dev-b',
      deviceLabel: 'Laptop B (Linux · 22bb)',
      createdAtMs: 1_794_900_000_000,
      lastActiveAtMs: 1_794_950_000_000,
      expiresAtMs: 1_797_492_000_000,
      isCurrent: false,
    },
  ],
};

describe('AccountModal Devices & Session Revocation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('lists active devices with current device marked and requires named confirmation before revoking another device', async () => {
    const afterRevoke: GoogleSession = {
      ...twoDevicesSession,
      devices: [twoDevicesSession.devices[0]],
    };
    vi.mocked(invoke).mockImplementation(async (cmd, args) => {
      if (cmd === 'cmd_get_google_session') return twoDevicesSession;
      if (cmd === 'cmd_revoke_device') {
        expect(args).toEqual({ sessionId: 'sid-b' });
        return {
          outcome: 'revoked',
          alreadyRevoked: false,
          session: afterRevoke,
        } satisfies RevokeDeviceOutcome;
      }
      return null;
    });

    const user = userEvent.setup();
    render(<AccountModal isOpen onClose={vi.fn()} />);

    expect(await screen.findByText('Signed in as qa-user@example.invalid')).toBeInTheDocument();
    expect(screen.getByText('Workstation A (Linux · 11aa)')).toBeInTheDocument();
    expect(screen.getByText('Current device')).toBeInTheDocument();
    expect(screen.getByText('Laptop B (Linux · 22bb)')).toBeInTheDocument();

    const revokeBtns = screen.getAllByRole('button', { name: /Sign out device/ });
    expect(revokeBtns).toHaveLength(1);
    await user.click(revokeBtns[0]);

    const confirmRegion = screen.getByRole('region', {
      name: 'Confirm sign out for Laptop B (Linux · 22bb)',
    });
    expect(confirmRegion).toHaveTextContent('Laptop B (Linux · 22bb)');
    expect(invoke).not.toHaveBeenCalledWith('cmd_revoke_device', expect.anything());

    await user.click(screen.getByRole('button', { name: 'Confirm sign out' }));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('cmd_revoke_device', { sessionId: 'sid-b' });
    });
    expect(screen.getByText('Signed out “Laptop B (Linux · 22bb)”.')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Sign out device/ })).not.toBeInTheDocument();
  });

  it('handles stale replaced device session without showing false success and requires confirming the refreshed session', async () => {
    const refreshedWithReplacement: GoogleSession = {
      ...twoDevicesSession,
      devices: [
        twoDevicesSession.devices[0],
        {
          ...twoDevicesSession.devices[1],
          sessionId: 'sid-b2',
          lastActiveAtMs: 1_795_002_000_000,
        },
      ],
    };
    const afterConfirmedSecond: GoogleSession = {
      ...twoDevicesSession,
      devices: [twoDevicesSession.devices[0]],
    };
    let call = 0;
    vi.mocked(invoke).mockImplementation(async (cmd, args) => {
      if (cmd === 'cmd_get_google_session') return twoDevicesSession;
      if (cmd === 'cmd_revoke_device') {
        call += 1;
        if (call === 1) {
          expect(args).toEqual({ sessionId: 'sid-b' });
          return {
            outcome: 'staleReplaced',
            message:
              'This device signed in again with a newer session. Review the refreshed Devices list and confirm if you still want to sign it out.',
            replacementSessionId: 'sid-b2',
            session: refreshedWithReplacement,
          } satisfies RevokeDeviceOutcome;
        }
        expect(args).toEqual({ sessionId: 'sid-b2' });
        return {
          outcome: 'revoked',
          alreadyRevoked: false,
          session: afterConfirmedSecond,
        } satisfies RevokeDeviceOutcome;
      }
      return null;
    });

    const user = userEvent.setup();
    render(<AccountModal isOpen onClose={vi.fn()} />);

    await user.click(await screen.findByRole('button', { name: 'Sign out device Laptop B (Linux · 22bb)' }));
    await user.click(screen.getByRole('button', { name: 'Confirm sign out' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/signed in again with a newer session/i);
    expect(screen.queryByText(/Signed out “Laptop B/)).not.toBeInTheDocument();
    // Refreshed entry must still require an explicit new confirmation before calling sid-b2
    expect(screen.getByRole('button', { name: 'Sign out device Laptop B (Linux · 22bb)' })).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledTimes(2); // get_google_session + stale revoke only

    await user.click(screen.getByRole('button', { name: 'Sign out device Laptop B (Linux · 22bb)' }));
    await user.click(screen.getByRole('button', { name: 'Confirm sign out' }));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('cmd_revoke_device', { sessionId: 'sid-b2' });
    });
    expect(screen.getByText('Signed out “Laptop B (Linux · 22bb)”.')).toBeInTheDocument();
  });

  it('handles missing device target honestly without reporting Signed out', async () => {
    const refreshedOnlyA: GoogleSession = {
      ...twoDevicesSession,
      devices: [twoDevicesSession.devices[0]],
    };
    vi.mocked(invoke).mockImplementation(async (cmd) => {
      if (cmd === 'cmd_get_google_session') return twoDevicesSession;
      if (cmd === 'cmd_revoke_device') {
        return {
          outcome: 'missing',
          message: 'That device session was no longer active. The Devices list has been refreshed.',
          session: refreshedOnlyA,
        } satisfies RevokeDeviceOutcome;
      }
      return null;
    });

    const user = userEvent.setup();
    render(<AccountModal isOpen onClose={vi.fn()} />);

    await user.click(await screen.findByRole('button', { name: 'Sign out device Laptop B (Linux · 22bb)' }));
    await user.click(screen.getByRole('button', { name: 'Confirm sign out' }));

    expect(await screen.findByText(/no longer active/i)).toBeInTheDocument();
    expect(screen.queryByText(/Signed out “/)).not.toBeInTheDocument();
  });

  it('shows revoked and invalid states distinctly on restart and plain signed-out state afterward without auto-starting Google', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({ status: 'signedOut', reason: 'revoked' } satisfies GoogleSession)
      .mockResolvedValueOnce({ status: 'signedOut', reason: 'invalid' } satisfies GoogleSession)
      .mockResolvedValueOnce({ status: 'signedOut' } satisfies GoogleSession);

    const { unmount } = render(<AccountModal isOpen onClose={vi.fn()} />);
    expect(await screen.findByRole('alert')).toHaveTextContent('This device was signed out.');
    expect(screen.getByRole('button', { name: 'Continue with Google' })).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith('cmd_google_sign_in', expect.anything());
    expect(invoke).not.toHaveBeenCalledWith('cmd_google_sign_in');
    unmount();

    const { unmount: unmount2 } = render(<AccountModal isOpen onClose={vi.fn()} />);
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'This device session is invalid. Sign in again with Google.',
    );
    unmount2();

    render(<AccountModal isOpen onClose={vi.fn()} />);
    expect(await screen.findByRole('button', { name: 'Continue with Google' })).toBeInTheDocument();
    expect(screen.queryByText('This device was signed out.')).not.toBeInTheDocument();
  });

  it('retains unverified state when service is unreachable or 401 is ambiguous and retries on Retry and online event', async () => {
    const unverified: GoogleSession = {
      status: 'unverified',
      message:
        'Saved session can’t be verified right now because the session service is unreachable. Your personal vault still works offline.',
    };
    vi.mocked(invoke)
      .mockResolvedValueOnce(unverified)
      .mockResolvedValueOnce(unverified)
      .mockResolvedValueOnce({ status: 'signedOut', reason: 'revoked' } satisfies GoogleSession);

    const user = userEvent.setup();
    render(<AccountModal isOpen onClose={vi.fn()} />);

    expect(await screen.findByRole('alert')).toHaveTextContent(/can’t be verified/i);
    expect(screen.getByRole('button', { name: 'Sign out' })).toBeInTheDocument();
    expect(screen.queryByText('Devices')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Retry verification' }));
    expect(await screen.findByRole('alert')).toHaveTextContent(/can’t be verified/i);

    window.dispatchEvent(new Event('online'));
    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent('This device was signed out.');
    });
  });

  it('transitions to Unverified on ambiguous 401 during Sign out device and restores Devices on Retry verification without starting Google sign-in', async () => {
    const unverifiedMessage =
      'Saved session could not be verified because the session service returned an unexpected authentication response. Your saved session was kept; try again.';
    let getSessionCalls = 0;
    vi.mocked(invoke).mockImplementation(async (cmd) => {
      if (cmd === 'cmd_get_google_session') {
        getSessionCalls += 1;
        return twoDevicesSession;
      }
      if (cmd === 'cmd_revoke_device') {
        return {
          outcome: 'unverified',
          session: {
            status: 'unverified',
            message: unverifiedMessage,
          },
        } satisfies RevokeDeviceOutcome;
      }
      return null;
    });

    const user = userEvent.setup();
    render(<AccountModal isOpen onClose={vi.fn()} />);

    await user.click(await screen.findByRole('button', { name: 'Sign out device Laptop B (Linux · 22bb)' }));
    expect(
      screen.getByRole('region', { name: 'Confirm sign out for Laptop B (Linux · 22bb)' }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Confirm sign out' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(unverifiedMessage);
    expect(screen.queryByText(/Signed out “Laptop B/)).not.toBeInTheDocument();
    expect(
      screen.queryByRole('region', { name: 'Confirm sign out for Laptop B (Linux · 22bb)' }),
    ).not.toBeInTheDocument();
    expect(screen.queryByRole('list', { name: 'Signed-in devices' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Refresh' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Sign out device/ })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Retry verification' })).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith('cmd_google_sign_in', expect.anything());
    expect(invoke).not.toHaveBeenCalledWith('cmd_google_sign_in');

    await user.click(screen.getByRole('button', { name: 'Retry verification' }));

    expect(await screen.findByRole('list', { name: 'Signed-in devices' })).toBeInTheDocument();
    expect(screen.getByText('Workstation A (Linux · 11aa)')).toBeInTheDocument();
    expect(screen.getByText('Laptop B (Linux · 22bb)')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Sign out device Laptop B (Linux · 22bb)' })).toBeInTheDocument();
    expect(
      screen.queryByRole('region', { name: 'Confirm sign out for Laptop B (Linux · 22bb)' }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/Signed out “Laptop B/)).not.toBeInTheDocument();
    expect(getSessionCalls).toBe(2);
    expect(invoke).not.toHaveBeenCalledWith('cmd_google_sign_in', expect.anything());
    expect(invoke).not.toHaveBeenCalledWith('cmd_google_sign_in');
  });

  it('keeps confirmed revoked, expired, and invalid caller termination on Sign out device', async () => {
    for (const [reason, expectedCopy] of [
      ['revoked', 'This device was signed out.'],
      ['expired', 'Your saved session expired. Sign in again with Google.'],
      ['invalid', 'This device session is invalid. Sign in again with Google.'],
    ] as const) {
      vi.clearAllMocks();
      vi.mocked(invoke).mockImplementation(async (cmd) => {
        if (cmd === 'cmd_get_google_session') return twoDevicesSession;
        if (cmd === 'cmd_revoke_device') {
          return {
            outcome: 'callerInvalid',
            session: { status: 'signedOut', reason },
          } satisfies RevokeDeviceOutcome;
        }
        return null;
      });

      const user = userEvent.setup();
      const { unmount } = render(<AccountModal isOpen onClose={vi.fn()} />);

      await user.click(
        await screen.findByRole('button', { name: 'Sign out device Laptop B (Linux · 22bb)' }),
      );
      await user.click(screen.getByRole('button', { name: 'Confirm sign out' }));

      expect(await screen.findByRole('alert')).toHaveTextContent(expectedCopy);
      expect(screen.getByRole('button', { name: 'Continue with Google' })).toBeInTheDocument();
      expect(screen.queryByText(/Signed out “Laptop B/)).not.toBeInTheDocument();
      unmount();
    }
  });
});
