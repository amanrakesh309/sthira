import { useCallback, useEffect, useState } from 'react';
import { Cable } from 'lucide-react';
import {
  connectAgent,
  disconnectAgent,
  getAgentConnectionConfig,
  getAgentConnections,
  testAgentConnection,
  type AgentConfigSnippet,
  type ExternalAgentClient,
} from '../api/agentConnections';
import { getWorkspaceTree } from '../api/ipc';
import { ContextInfo } from './ContextInfo';

const appNames: Record<ExternalAgentClient, string> = {
  codex: 'Codex',
  antigravity: 'Antigravity',
  claudeCode: 'Claude Code',
  cline: 'Cline',
  kilo: 'Kilo Code',
  opencode: 'OpenCode',
  custom: 'Other MCP client',
};

export function AgentConnectionsSettings() {
  const [setup, setSetup] = useState<Awaited<ReturnType<typeof getAgentConnections>>>();
  const [projects, setProjects] = useState<{ id: string; name: string }[]>([]);
  const [client, setClient] = useState<ExternalAgentClient | ''>('');
  const [projectId, setProjectId] = useState('');
  const [canPropose, setCanPropose] = useState(false);
  const [useConfig, setUseConfig] = useState(false);
  const [config, setConfig] = useState<{ id: string; snippets: AgentConfigSnippet[] }>();
  const [format, setFormat] = useState(0);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const [error, setError] = useState('');

  const refresh = useCallback(async () => {
    const [nextSetup, tree] = await Promise.all([getAgentConnections(), getWorkspaceTree()]);
    setSetup(nextSetup);
    setProjects(tree.projects);
  }, []);

  useEffect(() => {
    void refresh().catch(() => setError('Could not load agent connections. Reopen this section to retry.'));
  }, [refresh]);

  async function act(task: () => Promise<string>) {
    setBusy(true);
    setError('');
    setNotice('');
    try {
      setNotice(await task());
      await refresh();
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setBusy(false);
    }
  }

  async function showConfig(id: string) {
    const snippets = await getAgentConnectionConfig(id);
    setFormat(0);
    setConfig({ id, snippets });
  }

  const selectedApp = setup?.apps.find(app => app.client === client);
  const manual = Boolean(selectedApp?.supportsConfig && (useConfig || !selectedApp.installed));
  const canConnect = selectedApp?.installed || manual;
  const snippet = config?.snippets[format];
  const existingConnection = setup?.connections.find(connection =>
    connection.client === client && connection.projectId === projectId,
  );

  return (
    <section className="settings-agents" aria-labelledby="settings-agents-title">
      <header className="flex items-center gap-2">
        <h3 id="settings-agents-title" className="settings-section-title">Connect an agent</h3>
        <ContextInfo title="agent connections">Share one personal project with an app on this device. You review suggested changes in Sthira. Disconnecting revokes access and blocks approval of pending suggestions.</ContextInfo>
      </header>
      {error && <p role="alert" className="settings-agents-error">{error}</p>}
      {notice && <p role="status">{notice}</p>}
      {!setup && !error && <p role="status">Loading agent connections…</p>}
      {setup && !setup.supported && <p>Connect agents in the Sthira desktop app on Linux.</p>}
      {setup?.supported && (
        <>
          <form className="settings-agents-form" onSubmit={event => {
            event.preventDefault();
            if (!client || !projectId || !canConnect || existingConnection || busy) return;
            void act(async () => {
              const id = await connectAgent(client, projectId, canPropose, manual);
              if (manual) {
                await showConfig(id);
              } else {
                setConfig(undefined);
              }
              setCanPropose(false);
              if (manual) return 'Configuration ready.';
              return `Setup complete. Open ${appNames[client]} to use Sthira.`;
            });
          }}>
            <label>
              <span>Agent app</span>
              <select value={client} onChange={event => {
                setClient(event.target.value as ExternalAgentClient | '');
                setUseConfig(false);
              }} disabled={busy}>
                <option value="">Choose an app</option>
                {setup.apps.map(app => (
                  <option key={app.client} value={app.client} disabled={!app.installed && !app.supportsConfig}>
                    {appNames[app.client]}{app.installed ? '' : app.supportsConfig ? ' (config file)' : ' (install required)'}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>Project to share</span>
              <select value={projectId} onChange={event => setProjectId(event.target.value)} disabled={busy || projects.length === 0}>
                <option value="">Choose a project</option>
                {projects.map(project => <option key={project.id} value={project.id}>{project.name}</option>)}
              </select>
            </label>
            <label>
              <span>Access</span>
              <select value={canPropose ? 'suggest' : 'read'} onChange={event => setCanPropose(event.target.value === 'suggest')} disabled={busy}>
                <option value="read">Can read</option>
                <option value="suggest">Can read and suggest changes</option>
              </select>
            </label>
            {projects.length === 0 && <p>Create a personal project before connecting an agent.</p>}
            {selectedApp?.installed && selectedApp.supportsConfig && (
              <label className="settings-agents-config-toggle">
                <input type="checkbox" checked={useConfig} onChange={event => setUseConfig(event.target.checked)} disabled={busy} />
                Use a config file
              </label>
            )}
            {existingConnection && <p>{client && appNames[client]} {existingConnection.manual ? 'already has a setup for' : 'is already connected to'} this project.</p>}
            <button className="settings-btn-primary" type="submit" disabled={!canConnect || !projectId || Boolean(existingConnection) || busy}>
              <Cable size={16} aria-hidden="true" /> {manual ? 'Create setup' : 'Connect'}
            </button>
          </form>

          {config && snippet && (
            <section className="settings-agent-config" aria-label="MCP configuration">
              <div className="settings-agents-list-header">
                <h4>Client configuration</h4>
                <div className="settings-agents-actions">
                  <button type="button" className="settings-btn-primary" onClick={async () => {
                    try {
                      await navigator.clipboard.writeText(JSON.stringify(snippet.json, null, 2));
                      setNotice('Configuration copied.');
                    } catch {
                      setNotice('Select the configuration text and copy it.');
                    }
                  }}>Copy configuration</button>
                  <button type="button" className="settings-agent-secondary" onClick={() => setConfig(undefined)}>Close configuration</button>
                </div>
              </div>
              {config.snippets.length > 1 && (
                <label>
                  Client version
                  <select value={format} onChange={event => setFormat(Number(event.target.value))}>
                    {config.snippets.map((entry, index) => <option key={entry.label} value={index}>{entry.label}</option>)}
                  </select>
                </label>
              )}
              <p>{snippet.instructions} Merge it with any existing entries.</p>
              <textarea aria-label="MCP configuration JSON" readOnly rows={7} value={JSON.stringify(snippet.json, null, 2)} />
            </section>
          )}

          <section className="settings-agents-list" aria-labelledby="connected-agents-title">
            <div className="settings-agents-list-header">
              <h4 id="connected-agents-title">Connected agents</h4>
              {setup.connections.length > 0 && (
                <button type="button" className="settings-agent-secondary" onClick={() => void act(async () => '')} disabled={busy}>
                  Refresh status
                </button>
              )}
            </div>
            {setup.connections.length === 0 && <p>No connected agents.</p>}
            <ul>
              {setup.connections.map(connection => {
                const name = appNames[connection.client];
                const project = projects.find(item => item.id === connection.projectId)?.name ?? 'Unavailable project';
                return (
                  <li key={connection.id}>
                    <div>
                      <strong>{name} · {project}</strong>
                      <p>{connection.canPropose ? 'Can read and suggest changes' : 'Can read'}</p>
                      <p>{connection.verified ? 'Verified in the client' : connection.manual ? 'Configuration ready' : connection.registered ? 'Setup complete' : 'Setup interrupted'}</p>
                    </div>
                    <div className="settings-agents-actions">
                      {connection.manual && (
                        <button type="button" className="settings-agent-secondary" aria-label={`Show configuration for ${name} and ${project}`}
                          onClick={() => void act(async () => { await showConfig(connection.id); return ''; })}
                          disabled={busy}>Show configuration</button>
                      )}
                      <button type="button" className="settings-agent-secondary" aria-label={`Test connection for ${name} and ${project}`}
                        onClick={() => void act(async () => {
                          await testAgentConnection(connection.id);
                          return `Sthira can access ${project} through this connection.`;
                        })} disabled={busy}>Test connection</button>
                      <button type="button" className="settings-agent-secondary" aria-label={`Disconnect ${name} from ${project}`}
                        onClick={() => void act(async () => {
                          const result = await disconnectAgent(connection.id);
                          if (config?.id === connection.id) setConfig(undefined);
                          return result.clientEntryRemoved ? `${name} disconnected.`
                            : `${name} access revoked. Remove the old Sthira entry in the client if it still appears.`;
                        })} disabled={busy}>Disconnect</button>
                    </div>
                  </li>
                );
              })}
            </ul>
            {setup.connections.length > 0 && (
              <details>
                <summary>Try a prompt</summary>
                <p>Use the Sthira connector to find notes in my shared project. Tell me what decisions are saved and which notes contain them.</p>
              </details>
            )}
          </section>
        </>
      )}
    </section>
  );
}
