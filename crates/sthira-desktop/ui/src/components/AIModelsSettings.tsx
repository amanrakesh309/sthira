import { useEffect, useRef, useState } from 'react';
import { Check, Info, Plus, RefreshCw, Server } from 'lucide-react';
import {
  deleteModel, deleteProvider, discoverProviderModels, saveProvider, setDefaultModel,
  toggleModel, updateModelSearchCapability, upsertModel,
} from '../api/ipc';
import type { ProviderEntry, ProviderModelEntry, WebSearchCapability } from '../types/sthira';
import { GuidedConnectionSetup } from './GuidedConnectionSetup';
import { connectionLocation, modelContextGuidance, validConnectionUrl } from './modelGuidance';

type Run = (action: () => Promise<void>, success?: string) => Promise<boolean>;

function ModelRow({ model, provider, run }: { model: ProviderModelEntry; provider: ProviderEntry; run: Run }) {
  const [expanded, setExpanded] = useState(false);
  const detailId = `model-details-${model.id}`;
  return <article className={`ai-model ${model.is_default ? 'is-default' : ''}`}>
    <div className="ai-model-summary">
      <button type="button" className={`settings-radio-btn ${model.is_default ? 'checked' : ''}`} aria-label={model.is_default ? `${model.display_name} is the workspace default` : `Use ${model.display_name} as workspace default`} aria-pressed={model.is_default} disabled={!model.is_enabled || !provider.is_enabled} onClick={() => run(() => setDefaultModel(model.id), 'Workspace default updated.')}>
        {model.is_default && <Check size={15} />}
      </button>
      <div className="ai-model-name"><strong>{model.display_name}</strong><span>{connectionLocation(provider, model)}{!model.is_enabled ? ' · Disabled' : ''}</span></div>
      {model.is_default && <span className="settings-chip default-badge">Default</span>}
      <button type="button" className="ai-info-button" aria-label={`Information about ${model.display_name}`} aria-expanded={expanded} aria-controls={detailId} onClick={() => setExpanded(value => !value)}><Info size={18} /></button>
    </div>
    {expanded && <div className="ai-model-details" id={detailId}>
      <div className="ai-guidance"><h5>Working with this model</h5><p>{modelContextGuidance(model)}</p><p className="ai-helper">Context capacity does not measure answer quality or planning ability. Model evaluations have not been run.</p></div>
      <dl className="ai-technical-details">
        <div><dt>Model identifier</dt><dd><code>{model.model_identifier}</code></dd></div>
        <div><dt>Connection</dt><dd>{provider.display_name} · {provider.provider_type}</dd></div>
        <div><dt>Context window</dt><dd>{model.context_window > 0 ? `${model.context_window.toLocaleString('en-US')} tokens (catalog value)` : 'Not reported by the provider'}</dd></div>
        <div><dt>Structured output</dt><dd>{model.supports_structured_output ? 'Enabled in catalog' : 'Not enabled in catalog'}</dd></div>
        <div><dt>Web grounding</dt><dd>{({ native: 'Native grounding', agentic_tool_call: 'Tool calling', engine_augmented_only: 'Sources supplied by Sthira', unsupported: 'Not enabled' })[model.web_search_capability || 'engine_augmented_only']}</dd></div>
      </dl>
      <p className="ai-helper">Context is how much text a model can work with at once. Catalog values may be estimates; the context configured in your AI app can be smaller. A local connection can forward requests to a cloud service. Search availability is configured separately in Web Search.</p>
      <details className="ai-model-controls"><summary>Model controls</summary>
        <label>Web search support<select data-testid={`select-capability-${model.id}`} aria-label={`Web Search Support for ${model.display_name}`} value={model.web_search_capability || 'engine_augmented_only'} onChange={event => run(() => updateModelSearchCapability(model.id, event.target.value as WebSearchCapability))}>
          <option value="native">Native grounding</option><option value="agentic_tool_call">Tool calling</option><option value="engine_augmented_only">Sources supplied by Sthira</option><option value="unsupported">Not enabled</option>
        </select></label>
        <div className="ai-actions"><button type="button" className="settings-btn-secondary" onClick={() => run(() => toggleModel(model.id, !model.is_enabled))}>{model.is_enabled ? 'Disable model' : 'Enable model'}</button>
          <button type="button" className="settings-btn-secondary ai-danger" onClick={() => { if (confirm(`Remove '${model.display_name}' from Sthira? This does not delete it from your AI app.`)) void run(() => deleteModel(model.id), 'Model removed from Sthira.'); }}>Remove model</button></div>
      </details>
    </div>}
  </article>;
}

function ConnectionCard({ provider, run }: { provider: ProviderEntry; run: Run }) {
  const [url, setUrl] = useState(provider.base_url);
  const [key, setKey] = useState('');
  const [models, setModels] = useState<ProviderModelEntry[] | null>(null);
  const [selected, setSelected] = useState('');
  const [status, setStatus] = useState<'unchecked' | 'checking' | 'reached' | 'error'>('unchecked');
  const [error, setError] = useState('');
  const [query, setQuery] = useState('');
  const [manualId, setManualId] = useState('');
  const request = useRef(0);
  const savedUrl = useRef(provider.base_url);
  useEffect(() => () => { request.current++; }, []);
  useEffect(() => {
    if (provider.base_url !== savedUrl.current) {
      savedUrl.current = provider.base_url;
      setUrl(provider.base_url); setModels(null); setStatus('unchecked'); request.current++;
    }
  }, [provider.base_url]);
  const invalidate = () => { request.current++; setStatus('unchecked'); setModels(null); setError(''); };
  const check = async () => {
    if (!validConnectionUrl(url.trim())) { setError('Enter a valid HTTP or HTTPS server address without embedded credentials, a query, or a fragment.'); return; }
    const current = ++request.current;
    setStatus('checking'); setModels(null); setError('');
    try {
      const found = await discoverProviderModels(provider.id, provider.provider_type, url.trim(), key.trim() || undefined);
      if (request.current !== current) return;
      setModels(found); setSelected(found.find(item => !provider.models.some(saved => saved.model_identifier === item.model_identifier))?.model_identifier || ''); setStatus('reached');
    } catch {
      if (request.current === current) { setStatus('error'); setError('Could not list models. Check the address, start the app’s API server, and check its authentication. Retry when ready, or add a model manually if your server does not support listing.'); }
    }
  };
  const dirty = url.trim() !== provider.base_url || !!key.trim();
  const save = async () => {
    if (!validConnectionUrl(url.trim())) { setError('Enter a valid HTTP or HTTPS server address.'); return; }
    if (await run(() => saveProvider({ ...provider, models: [], base_url: url.trim(), has_api_key: !!key.trim() || provider.has_api_key }, key.trim() || undefined), 'Connection settings saved.')) { setKey(''); invalidate(); }
  };
  const add = async (model: ProviderModelEntry) => {
    if (await run(() => upsertModel({ ...model, id: `${provider.id}:${model.model_identifier}`, provider_id: provider.id, is_default: false }), 'Model added.')) setManualId('');
  };
  const available = models?.filter(item => !provider.models.some(saved => saved.model_identifier === item.model_identifier)) || [];
  const visible = provider.models.filter(model => `${model.display_name} ${model.model_identifier}`.toLowerCase().includes(query.toLowerCase()));
  return <section className="settings-provider-card ai-connection" aria-label={`${provider.display_name} connection`}>
    <div className="ai-section-heading"><div className="ai-connection-name"><Server size={20} /><div><h4>{provider.display_name}</h4><p>{provider.models.length} {provider.models.length === 1 ? 'model' : 'models'} added{!provider.is_enabled ? ' · Connection disabled' : ''}</p></div></div>
      <span className={`ai-connection-status ${status === 'error' ? 'ai-error' : ''}`} role="status">{({ unchecked: 'Not checked', checking: 'Checking…', reached: 'Server reached', error: 'Check failed' })[status]}</span></div>
    <details className="ai-connection-settings"><summary>Connection settings &amp; models</summary>
      <div className="ai-form-grid"><label className="ai-field-wide">Server address<input type="url" value={url} spellCheck={false} onChange={event => { invalidate(); setUrl(event.target.value); }} /></label>
        <label className="ai-field-wide">API key (if required)<input type="password" autoComplete="off" value={key} onChange={event => { invalidate(); setKey(event.target.value); }} placeholder={provider.has_api_key ? 'Saved key retained unless replaced' : 'Leave empty if not required'} /></label></div>
      {provider.has_api_key && url.trim() !== provider.base_url && !key.trim() && <p className="ai-helper">Re-enter your key to check a different server address. The saved key is only used to check the saved address.</p>}
      <div className="ai-actions"><button type="button" className="settings-btn-secondary" disabled={status === 'checking'} onClick={check}><RefreshCw size={15} className={status === 'checking' ? 'animate-spin' : ''} />Check connection</button>
        <button type="button" className="settings-btn-secondary" disabled={!dirty} onClick={save}>Save connection settings</button></div>
      {error && <p className="ai-error" role="alert">{error}</p>}
      {models !== null && <div className="ai-discovery"><p>{models.length ? `${models.length} models returned. Generation has not been tested.` : 'Server reached, but no models were returned. Download a text model in your AI app, make it available to the server, then check again.'}</p>
        {dirty && <p className="ai-helper">Save connection changes before adding models.</p>}
        {available.length > 0 ? <div className="ai-actions"><label>Model to add<select value={selected} onChange={event => setSelected(event.target.value)}>{available.map(model => <option key={model.model_identifier} value={model.model_identifier}>{model.display_name}</option>)}</select></label><button type="button" className="settings-btn-primary" disabled={dirty || !selected} onClick={() => { const model = available.find(item => item.model_identifier === selected); if (model) void add(model); }}>Add model</button></div> : models.length > 0 && <p>All returned models have been added.</p>}</div>}
      <details className="ai-manual-model"><summary>Add a model by identifier</summary><p className="ai-helper">Use this if your server cannot list models. The identifier must match a model your server can run.</p>
        <div className="ai-actions"><label>Model identifier<input value={manualId} onChange={event => setManualId(event.target.value)} placeholder="Exact model identifier" /></label><button type="button" className="settings-btn-secondary" disabled={dirty || !manualId.trim() || provider.models.some(model => model.model_identifier === manualId.trim())} onClick={() => add({ id: '', provider_id: provider.id, model_identifier: manualId.trim(), display_name: manualId.trim(), context_window: 0, supports_structured_output: false, is_default: false, is_enabled: true, web_search_capability: 'engine_augmented_only' })}>Add model</button></div>
      </details>
      <button type="button" className="ai-text-button ai-danger" onClick={() => { if (confirm(`Remove '${provider.display_name}' and its models from Sthira? Your AI app and downloaded models will be kept.`)) void run(() => deleteProvider(provider.id), 'Connection removed.'); }}>Remove connection</button>
    </details>
    {provider.models.length > 6 && <label className="ai-model-search">Find a model<input type="search" placeholder="Search models…" value={query} onChange={event => setQuery(event.target.value)} /></label>}
    {!provider.models.length ? <div className="ai-empty-models"><p>No models added yet.</p><p>Open Connection settings &amp; models to check your server and add one.</p></div> : visible.length ? <div className="ai-model-list">{visible.map(model => <ModelRow key={model.id} model={model} provider={provider} run={run} />)}</div> : <p className="ai-helper">No models match your search.</p>}
  </section>;
}

export function AIModelsSettings({ providers, loading, reload, onCatalogChanged }: {
  providers: ProviderEntry[]; loading: boolean; reload: () => Promise<void>; onCatalogChanged?: () => void;
}) {
  const [adding, setAdding] = useState(false);
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  useEffect(() => { if (!message) return; const timer = setTimeout(() => setMessage(''), 4500); return () => clearTimeout(timer); }, [message]);
  const changed = async () => { await reload(); onCatalogChanged?.(); };
  const run: Run = async (action, success) => {
    setBusy(true); setError('');
    try { await action(); await changed(); if (success) setMessage(success); return true; }
    catch { setError('Could not save this change. Your existing setup is kept; try again.'); return false; }
    finally { setBusy(false); }
  };
  return <div className="ai-models-page">
    <div className="ai-section-heading"><div><h3 className="settings-section-title">AI &amp; Models</h3><p className="settings-section-sub">Choose a model for your workspace. Connect more when you need them.</p></div>
      {!adding && providers.length > 0 && <button type="button" className="settings-btn-secondary" onClick={() => setAdding(true)}><Plus size={16} />Add connection</button>}</div>
    {message && <p className="ai-feedback"><Check size={15} aria-hidden="true" /><span role="status">{message}</span></p>}
    {error && <p className="ai-error" role="alert">{error}</p>}
    {loading && !providers.length ? <p role="status" className="ai-helper">Loading connections…</p> : <>
      {!providers.length && !adding && <div className="ai-empty-state"><span className="ai-empty-icon"><Server size={24} /></span><h4>Set up AI when you’re ready</h4><p>Connect Ollama, LM Studio, or another AI service.<br />Your notes work without an AI connection.</p><button type="button" className="settings-btn-primary" onClick={() => setAdding(true)}>Set up AI</button></div>}
      {adding && <GuidedConnectionSetup providers={providers} onCancel={() => setAdding(false)} onSaved={async () => { await changed(); setAdding(false); setMessage('Connection saved.'); }} />}
      <fieldset disabled={busy} className="ai-connections" aria-label="Saved AI connections">{providers.map(provider => <ConnectionCard key={provider.id} provider={provider} run={run} />)}</fieldset>
      {providers.length > 0 && <p className="ai-helper ai-page-note">Use the information button to see model details and guidance on working with its context limit.</p>}
    </>}
  </div>;
}
