import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentConnectionsSettings } from '../components/AgentConnectionsSettings';
import type { AgentConnection } from '../api/agentConnections';

const api = vi.hoisted(() => ({
  getAgentConnections: vi.fn(),
  getAgentConnectionConfig: vi.fn(),
  connectAgent: vi.fn(),
  testAgentConnection: vi.fn(),
  disconnectAgent: vi.fn(),
  getWorkspaceTree: vi.fn(),
}));

vi.mock('../api/agentConnections', () => api);
vi.mock('../api/ipc', () => ({ getWorkspaceTree: api.getWorkspaceTree }));

describe('Agent connections Settings', () => {
  let connections: AgentConnection[];

  beforeEach(() => {
    vi.resetAllMocks();
    connections = [];
    api.getAgentConnections.mockImplementation(async () => ({
      supported: true,
      apps: [
        { client: 'codex', installed: true, supportsConfig: false },
        { client: 'antigravity', installed: true, supportsConfig: false },
        { client: 'cline', installed: true, supportsConfig: true },
        { client: 'opencode', installed: false, supportsConfig: true },
      ],
      connections: [...connections],
    }));
    api.getWorkspaceTree.mockResolvedValue({ projects: [{ id: 'project-one', name: 'Field notes' }] });
  });

  it('defaults to read access and distinguishes local testing from a real client verification', async () => {
    api.connectAgent.mockImplementation(async client => {
      connections.push({ id: 'codex-one', client, projectId: 'project-one', canPropose: false, registered: true, manual: false, verified: false });
    });
    api.testAgentConnection.mockResolvedValue(undefined);
    render(<AgentConnectionsSettings />);

    fireEvent.change(await screen.findByLabelText('Agent app'), { target: { value: 'codex' } });
    fireEvent.change(screen.getByLabelText('Project to share'), { target: { value: 'project-one' } });
    fireEvent.click(screen.getByRole('button', { name: 'Connect' }));

    const row = within(await screen.findByRole('listitem'));
    expect(api.connectAgent).toHaveBeenCalledWith('codex', 'project-one', false, false);
    expect(row.getByText('Setup complete')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Connect' })).toBeDisabled();

    fireEvent.click(row.getByRole('button', { name: 'Test connection for Codex and Field notes' }));
    await screen.findByText('Sthira can access Field notes through this connection.');
    expect(api.testAgentConnection).toHaveBeenCalledWith('codex-one');
    expect(row.queryByText('Verified in the client')).toBeNull();

    connections[0] = { ...connections[0], verified: true };
    await waitFor(() => expect(screen.getByRole('button', { name: 'Refresh status' })).toBeEnabled());
    fireEvent.click(screen.getByRole('button', { name: 'Refresh status' }));
    expect(await row.findByText('Verified in the client')).toBeInTheDocument();
  });

  it('removes only the disconnected agent and reports a client cleanup failure separately', async () => {
    connections = [
      { id: 'codex-one', client: 'codex', projectId: 'project-one', canPropose: true, registered: true, manual: false, verified: true },
      { id: 'antigravity-one', client: 'antigravity', projectId: 'project-one', canPropose: false, registered: true, manual: false, verified: true },
    ];
    api.disconnectAgent.mockImplementation(async (id: string) => {
      connections = connections.filter(connection => connection.id !== id);
      return { clientEntryRemoved: false };
    });
    render(<AgentConnectionsSettings />);

    fireEvent.click(await screen.findByRole('button', { name: 'Disconnect Codex from Field notes' }));
    await screen.findByText('Codex access revoked. Remove the old Sthira entry in the client if it still appears.');
    await waitFor(() => expect(screen.queryByText('Codex · Field notes')).toBeNull());
    expect(screen.getByText('Antigravity · Field notes')).toBeInTheDocument();
    expect(api.disconnectAgent).toHaveBeenCalledExactlyOnceWith('codex-one');
  });
  it('exports a client configuration and reopens it without creating another grant', async () => {
    const command = ['/opt/sthira', '--sthira-agent-mcp', '/tmp/vault', '/tmp/config', 'open-one'];
    const entry = { type: 'local', command };
    api.getAgentConnectionConfig.mockResolvedValue([
      { label: 'OpenCode 1', instructions: 'Add under mcp.', json: { mcp: { sthira: entry } } },
      { label: 'OpenCode 2', instructions: 'Add under mcp.servers.', json: { mcp: { servers: { sthira: entry } } } },
    ]);
    api.connectAgent.mockImplementation(async client => {
      connections.push({
        id: 'open-one', client, projectId: 'project-one', canPropose: false,
        manual: true, registered: false, verified: false,
      });
      return 'open-one';
    });
    render(<AgentConnectionsSettings />);
    fireEvent.change(await screen.findByLabelText('Agent app'), { target: { value: 'opencode' } });
    fireEvent.change(screen.getByLabelText('Project to share'), { target: { value: 'project-one' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create setup' }));

    expect(await screen.findByRole('textbox', { name: 'MCP configuration JSON' }))
      .toHaveValue(JSON.stringify({ mcp: { sthira: entry } }, null, 2));
    expect(api.connectAgent).toHaveBeenCalledWith('opencode', 'project-one', false, true);
    fireEvent.change(screen.getByLabelText('Client version'), { target: { value: '1' } });
    expect(screen.getByRole('textbox', { name: 'MCP configuration JSON' }))
      .toHaveValue(JSON.stringify({ mcp: { servers: { sthira: entry } } }, null, 2));
    fireEvent.click(screen.getByRole('button', { name: 'Close configuration' }));
    const reopen = await screen.findByRole('button', { name: 'Show configuration for OpenCode and Field notes' });
    await waitFor(() => expect(reopen).toBeEnabled());
    fireEvent.click(reopen);
    await screen.findByRole('textbox', { name: 'MCP configuration JSON' });
    expect(api.connectAgent).toHaveBeenCalledTimes(1);
    expect(api.getAgentConnectionConfig).toHaveBeenLastCalledWith('open-one');
  });

});
