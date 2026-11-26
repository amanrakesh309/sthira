import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './ipc';

export type ExternalAgentClient = 'codex' | 'antigravity' | 'claudeCode' | 'cline' | 'kilo' | 'opencode' | 'custom';

export interface AgentConnection {
  id: string;
  client: ExternalAgentClient;
  projectId: string;
  canPropose: boolean;
  registered: boolean;
  verified: boolean;
  manual: boolean;
}

interface AgentConnectionSetup {
  supported: boolean;
  apps: { client: ExternalAgentClient; installed: boolean; supportsConfig: boolean }[];
  connections: AgentConnection[];
}

export function getAgentConnections(): Promise<AgentConnectionSetup> {
  if (!isTauri) return Promise.resolve({ supported: false, apps: [], connections: [] });
  return invoke('cmd_get_agent_connections');
}

export interface AgentConfigSnippet {
  label: string;
  instructions: string;
  json: Record<string, unknown>;
}

export function connectAgent(client: ExternalAgentClient, projectId: string, canPropose: boolean, useConfig: boolean): Promise<string> {
  return invoke('cmd_connect_agent', { client, projectId, canPropose, useConfig });
}

export function getAgentConnectionConfig(id: string): Promise<AgentConfigSnippet[]> {
  return invoke('cmd_get_agent_connection_config', { id });
}

export function testAgentConnection(id: string): Promise<void> {
  return invoke('cmd_test_agent_connection', { id });
}

export function disconnectAgent(id: string): Promise<{ clientEntryRemoved: boolean }> {
  return invoke('cmd_disconnect_agent', { id });
}
