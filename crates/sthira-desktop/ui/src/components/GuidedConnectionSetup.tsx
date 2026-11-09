import { useEffect, useRef, useState } from 'react';
import { ExternalLink, RefreshCw, Server, Globe, Settings2 } from 'lucide-react';
import { discoverProviderModels, openAIAppDownload, saveProvider } from '../api/ipc';
import type { ProviderEntry, ProviderModelEntry } from '../types/sthira';
import { validConnectionUrl } from './modelGuidance';

const PRESETS = {
  ollama: { name: 'Ollama', type: 'ollama', url: 'http://localhost:11434' },
  lmstudio: { name: 'LM Studio', type: 'openai_compatible', url: 'http://localhost:1234/v1' },
  custom: { name: '', type: 'openai_compatible', url: '' },
  openai: { name: 'OpenAI', type: 'openai_compatible', url: 'https://api.openai.com/v1' },
  anthropic: { name: 'Anthropic', type: 'anthropic', url: 'https://api.anthropic.com/v1' },
  openrouter: { name: 'OpenRouter', type: 'openai_compatible', url: 'https://openrouter.ai/api/v1' },
  groq: { name: 'Groq', type: 'openai_compatible', url: 'https://api.groq.com/openai/v1' },
  deepseek: { name: 'DeepSeek', type: 'openai_compatible', url: 'https://api.deepseek.com/v1' },
};
type Preset = keyof typeof PRESETS;

export function GuidedConnectionSetup({ providers, onSaved, onCancel }: {
  providers: ProviderEntry[];
  onSaved: () => Promise<void>;
  onCancel: () => void;
}) {
  const [preset, setPreset] = useState<Preset>('ollama');
  const [name, setName] = useState('Ollama');
  const [protocol, setProtocol] = useState('ollama');
  const [url, setUrl] = useState(PRESETS.ollama.url);
  const [key, setKey] = useState('');
  const [models, setModels] = useState<ProviderModelEntry[] | null>(null);
  const [selected, setSelected] = useState('');
  const [busy, setBusy] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const request = useRef(0);
  const [id, setId] = useState(() => `ollama-${crypto.randomUUID()}`);
  useEffect(() => () => { request.current++; }, []);

  const invalidate = () => { request.current++; setBusy(false); setModels(null); setSelected(''); setError(''); };
  const choose = (next: Preset) => {
    invalidate(); setPreset(next); setName(PRESETS[next].name); setUrl(PRESETS[next].url);
    setProtocol(PRESETS[next].type); setKey(''); setId(`${next}-${crypto.randomUUID()}`);
  };
  const check = async () => {
    if (!validConnectionUrl(url.trim())) { setError('Enter an HTTP or HTTPS server address without credentials, a query, or a fragment.'); return; }
    const current = ++request.current;
    setBusy(true); setError(''); setModels(null); setSelected('');
    try {
      const found = await discoverProviderModels(id, protocol, url.trim(), key.trim() || undefined);
      if (request.current !== current) return;
      setModels(found); setSelected(found[0]?.model_identifier || '');
    } catch {
      if (request.current === current) setError('Could not list models. Check the server address, start the app’s API server, and check authentication if enabled. You can retry or save the connection and add a model manually.');
    } finally { if (request.current === current) setBusy(false); }
  };
  const save = async () => {
    if (!name.trim() || !validConnectionUrl(url.trim())) { setError('Enter a connection name and a valid HTTP or HTTPS server address.'); return; }
    setSaving(true); setError('');
    const model = models?.find(item => item.model_identifier === selected);
    const hasDefault = providers.some(provider => provider.models.some(item => item.is_default));
    const entry: ProviderEntry = {
      id, display_name: name.trim(), provider_type: protocol, base_url: url.trim(),
      has_api_key: !!key.trim(), is_enabled: true,
      models: model ? [{ ...model, id: `${id}:${model.model_identifier}`, provider_id: id, is_default: !hasDefault }] : [],
    };
    try { await saveProvider(entry, key.trim() || undefined); await onSaved(); }
    catch { setError('Could not save the connection. Your setup is still here; try again.'); }
    finally { setSaving(false); }
  };
  const openDownload = async (event: React.MouseEvent<HTMLAnchorElement>, app: 'ollama' | 'lmstudio') => {
    event.preventDefault();
    try { await openAIAppDownload(app); }
    catch { setError(`Could not open your browser. Visit ${app === 'ollama' ? 'https://ollama.com/download' : 'https://lmstudio.ai/download'} to install the app.`); }
  };

  return <section className="ai-setup settings-provider-card" aria-labelledby="ai-setup-title">
    <div className="ai-section-heading"><div><h4 id="ai-setup-title">Connect an AI app</h4><p>Sthira connects to an app or service you choose.</p></div>
      <button type="button" className="settings-btn-secondary" disabled={saving} onClick={onCancel}>Set up later</button></div>
    <fieldset disabled={saving} className="ai-setup-fields">
      <legend className="sr-only">Connection setup</legend>
      <div className="ai-app-choices" role="group" aria-label="Choose an AI app">
        {([['ollama', 'Ollama', 'Connect its local server', Server], ['lmstudio', 'LM Studio', 'Connect its local server', Server], ['custom', 'Custom connection', 'Your own compatible server', Settings2]] as const).map(([value, title, description, Icon]) =>
          <button type="button" key={value} aria-label={title} aria-pressed={preset === value} className="ai-app-choice" onClick={() => choose(value)}><Icon size={20} /><strong>{title}</strong><span>{description}</span></button>)}
      </div>
      <label className="ai-cloud-choice"><Globe size={16} /><span>Or use a cloud provider</span><select aria-label="Cloud provider" value={['ollama', 'lmstudio', 'custom'].includes(preset) ? '' : preset} onChange={event => { if (event.target.value) choose(event.target.value as Preset); }}>
        <option value="">Choose a provider…</option>{Object.entries(PRESETS).filter(([value]) => !['ollama', 'lmstudio', 'custom'].includes(value)).map(([value, item]) => <option key={value} value={value}>{item.name}</option>)}
      </select></label>
      <div className="ai-setup-guide">
        <h5>{preset === 'ollama' || preset === 'lmstudio' ? `Get ${PRESETS[preset].name} ready` : 'Get your connection ready'}</h5>
        {preset === 'ollama' ? <><ol><li>Install Ollama if you do not have it.</li><li>Download a text model in Ollama, then keep its server running. On Linux, use <code>ollama serve</code> if needed.</li><li>Check the connection below and choose a model.</li></ol><a href="https://ollama.com/download" onClick={event => openDownload(event, 'ollama')} target="_blank" rel="noreferrer">Get Ollama <ExternalLink size={13} /></a></>
          : preset === 'lmstudio' ? <><ol><li>Install LM Studio if you do not have it.</li><li>Download a text model, then open Developer and turn on Start server.</li><li>Copy its server address below, check the connection, and choose a model.</li></ol><a href="https://lmstudio.ai/download" onClick={event => openDownload(event, 'lmstudio')} target="_blank" rel="noreferrer">Get LM Studio <ExternalLink size={13} /></a></>
          : <p>{preset === 'custom' ? 'Start your server and copy its API base address. For an OpenAI-compatible server, include /v1 when required. Enter an API key only if your server requires one.' : 'Create an API key with your provider and enter it below. Usage is billed by your provider.'}</p>}
        <p className="ai-helper">Sthira does not install apps or download models. You can keep using your notes while you set up AI.</p>
      </div>
      <div className="ai-form-grid">
        <label>Connection name<input value={name} onChange={event => setName(event.target.value)} placeholder="My AI server" /></label>
        {preset === 'custom' && <label>API format<select value={protocol} onChange={event => { invalidate(); setProtocol(event.target.value); }}><option value="openai_compatible">OpenAI-compatible</option><option value="ollama">Ollama</option><option value="anthropic">Anthropic</option></select></label>}
        <label className="ai-field-wide">Server address<input type="url" value={url} onChange={event => { invalidate(); setUrl(event.target.value); }} placeholder="http://localhost:8000/v1" spellCheck={false} /></label>
        <label className="ai-field-wide">API key (if required)<input type="password" autoComplete="off" value={key} onChange={event => { invalidate(); setKey(event.target.value); }} placeholder="Leave empty for servers without authentication" /></label>
      </div>
      <div className="ai-actions"><button type="button" className="settings-btn-secondary" onClick={check} disabled={busy || !url.trim()}><RefreshCw size={15} className={busy ? 'animate-spin' : ''} />{busy ? 'Checking…' : 'Check connection'}</button></div>
      {models !== null && <div className="ai-discovery" role="status">{models.length ? <><p>Server reached · {models.length} {models.length === 1 ? 'model' : 'models'} found. Generation has not been tested.</p><label>Choose a model<select value={selected} onChange={event => setSelected(event.target.value)}>{models.map(model => <option key={model.model_identifier} value={model.model_identifier}>{model.display_name}</option>)}</select></label></> : <p>Server reached, but no models were returned. Download a text model in your AI app, make it available to the server, and check again. You can also save this connection and add a model by its identifier.</p>}</div>}
      {error && <p role="alert" className="ai-error">{error}</p>}
      <div className="ai-actions ai-setup-footer"><span className="ai-helper">{selected ? 'Adds this model. You can add more later.' : 'Saves the address; no model will be assumed available.'}</span><button type="button" className="settings-btn-primary" disabled={saving || busy} onClick={save}>{saving ? 'Saving…' : selected ? 'Use selected model' : 'Save connection'}</button></div>
    </fieldset>
  </section>;
}
