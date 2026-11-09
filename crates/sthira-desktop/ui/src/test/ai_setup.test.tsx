import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { AIModelsSettings } from '../components/AIModelsSettings';
import { connectionLocation, modelContextGuidance, modelGuidance } from '../components/modelGuidance';
import type { ProviderEntry, ProviderModelEntry } from '../types/sthira';

const api = vi.hoisted(() => ({
  discoverProviderModels: vi.fn(), saveProvider: vi.fn(), upsertModel: vi.fn(),
  deleteModel: vi.fn(), deleteProvider: vi.fn(), setDefaultModel: vi.fn(),
  toggleModel: vi.fn(), updateModelSearchCapability: vi.fn(),
  openAIAppDownload: vi.fn(),
}));
vi.mock('../api/ipc', () => api);
const model: ProviderModelEntry = {
  id: 'local:deepseek-r1', provider_id: 'local', model_identifier: 'deepseek-r1:8b',
  display_name: 'DeepSeek R1', context_window: 32768, supports_structured_output: true,
  is_enabled: true, is_default: true, web_search_capability: 'engine_augmented_only',
};
const provider: ProviderEntry = { id: 'local', display_name: 'My server', provider_type: 'ollama', base_url: 'http://localhost:11434', has_api_key: false, is_enabled: true, models: [model] };
const reload = vi.fn().mockResolvedValue(undefined);
function setup(providers: ProviderEntry[] = []) {
  render(<AIModelsSettings providers={providers} loading={false} reload={reload} />);
}
function openSetup() { fireEvent.click(screen.getByRole('button', { name: /Set up AI|Add connection/ })); }

beforeEach(() => { vi.clearAllMocks(); api.saveProvider.mockResolvedValue(undefined); api.discoverProviderModels.mockResolvedValue([model]); });

describe('Guided external AI setup', () => {
  it('starts without assuming apps or models and allows setup later', () => {
    setup(); expect(screen.getByText('Set up AI when you’re ready')).toBeInTheDocument();
    expect(api.discoverProviderModels).not.toHaveBeenCalled(); expect(api.saveProvider).not.toHaveBeenCalled();
    openSetup(); expect(screen.getByText(/Install Ollama if/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Set up later' }));
    expect(screen.getByRole('button', { name: 'Set up AI' })).toBeInTheDocument(); expect(api.saveProvider).not.toHaveBeenCalled();
  });

  it('opens the chosen app’s official download page through the desktop command', async () => {
    api.openAIAppDownload.mockResolvedValue(undefined);
    setup(); openSetup(); fireEvent.click(screen.getByRole('link', { name: 'Get Ollama' }));
    expect(api.openAIAppDownload).toHaveBeenCalledWith('ollama');
    fireEvent.click(screen.getByRole('button', { name: 'LM Studio' }));
    fireEvent.click(screen.getByRole('link', { name: 'Get LM Studio' }));
    expect(api.openAIAppDownload).toHaveBeenCalledWith('lmstudio');
  });

  it.each([
    ['Ollama', 'ollama', 'http://localhost:11434'],
    ['LM Studio', 'openai_compatible', 'http://localhost:1234/v1'],
  ])('checks %s with its protocol and atomically saves its selected model', async (app, protocol, url) => {
    setup(); openSetup(); fireEvent.click(screen.getByRole('button', { name: app }));
    fireEvent.click(screen.getByRole('button', { name: 'Check connection' }));
    expect(await screen.findByLabelText('Choose a model')).toBeInTheDocument();
    expect(api.discoverProviderModels).toHaveBeenCalledWith(expect.any(String), protocol, url, undefined);
    expect(api.saveProvider).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Use selected model' }));
    await waitFor(() => expect(api.saveProvider).toHaveBeenCalled());
    const entry = api.saveProvider.mock.calls[0][0];
    expect(entry.provider_type).toBe(protocol); expect(entry.base_url).toBe(url);
    expect(entry.models[0]).toMatchObject({ provider_id: entry.id, id: `${entry.id}:deepseek-r1:8b`, is_default: true });
    expect(api.upsertModel).not.toHaveBeenCalled();
  });

  it('preserves an existing workspace default when adding a connection', async () => {
    setup([provider]); openSetup(); fireEvent.click(within(screen.getByRole('region', { name: 'Connect an AI app' })).getByRole('button', { name: 'Check connection' }));
    await screen.findByLabelText('Choose a model'); fireEvent.click(screen.getByRole('button', { name: 'Use selected model' }));
    await waitFor(() => expect(api.saveProvider).toHaveBeenCalled());
    expect(api.saveProvider.mock.calls[0][0].models[0].is_default).toBe(false);
  });

  it('supports a custom address, optional key and selected API format', async () => {
    setup(); openSetup(); fireEvent.click(screen.getByRole('button', { name: /^Custom connection/ }));
    fireEvent.change(screen.getByLabelText('Connection name'), { target: { value: 'My remote runtime' } });
    fireEvent.change(screen.getByLabelText('Server address'), { target: { value: 'http://192.168.1.10:11434' } });
    fireEvent.change(screen.getByLabelText('API format'), { target: { value: 'ollama' } });
    fireEvent.change(screen.getByLabelText('API key (if required)'), { target: { value: 'fixture-secret' } });
    fireEvent.click(screen.getByRole('button', { name: 'Check connection' })); await screen.findByLabelText('Choose a model');
    expect(api.discoverProviderModels).toHaveBeenCalledWith(expect.any(String), 'ollama', 'http://192.168.1.10:11434', 'fixture-secret');
    expect(screen.getByLabelText('API key (if required)')).toHaveAttribute('type', 'password');
    fireEvent.click(screen.getByRole('button', { name: 'Use selected model' }));
    await waitFor(() => expect(api.saveProvider).toHaveBeenCalledWith(expect.objectContaining({ display_name: 'My remote runtime', provider_type: 'ollama', has_api_key: true }), 'fixture-secret'));
  });

  it('does not treat an unreachable server as proof the app is missing', async () => {
    api.discoverProviderModels.mockRejectedValue(new Error('offline with private response'));
    setup(); openSetup(); fireEvent.click(screen.getByRole('button', { name: 'Check connection' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('start the app’s API server');
    expect(screen.queryByText(/private response/)).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Choose a model')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Save connection' }));
    await waitFor(() => expect(api.saveProvider).toHaveBeenCalledWith(expect.objectContaining({ models: [] }), undefined));
  });

  it('handles an empty server model list without inventing a default', async () => {
    api.discoverProviderModels.mockResolvedValue([]); setup(); openSetup();
    fireEvent.click(screen.getByRole('button', { name: 'Check connection' }));
    expect(await screen.findByText(/Server reached, but no models/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Use selected model' })).not.toBeInTheDocument();
  });

  it('discards stale checks when switching apps and invalidates results on address edits', async () => {
    let resolve!: (models: ProviderModelEntry[]) => void;
    api.discoverProviderModels.mockReturnValueOnce(new Promise(done => { resolve = done; }));
    setup(); openSetup(); fireEvent.click(screen.getByRole('button', { name: 'Check connection' }));
    fireEvent.click(screen.getByRole('button', { name: 'LM Studio' }));
    resolve([model]); await waitFor(() => expect(screen.queryByLabelText('Choose a model')).not.toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: 'Check connection' })); await screen.findByLabelText('Choose a model');
    fireEvent.change(screen.getByLabelText('Server address'), { target: { value: 'http://localhost:9999/v1' } });
    expect(screen.queryByLabelText('Choose a model')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Save connection' }));
    await waitFor(() => expect(api.saveProvider).toHaveBeenCalledWith(expect.objectContaining({ models: [], base_url: 'http://localhost:9999/v1' }), undefined));
  });

  it('keeps setup input after a failed save and prevents invalid URLs', async () => {
    api.saveProvider.mockRejectedValue(new Error('disk full')); setup(); openSetup();
    fireEvent.change(screen.getByLabelText('Server address'), { target: { value: 'file:///tmp/server' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save connection' }));
    expect(api.saveProvider).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText('Server address'), { target: { value: 'http://localhost:11434' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save connection' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Your setup is still here');
    expect(screen.getByLabelText('Server address')).toHaveValue('http://localhost:11434');
  });
});

describe('Model choices and details', () => {
  it('keeps recommendations out of model rows and explains context only inside Information', () => {
    setup([provider]); expect(screen.queryByText(/Try for/)).not.toBeInTheDocument();
    expect(screen.queryByText('Context window')).not.toBeInTheDocument();
    expect(screen.queryByText('Working with this model')).not.toBeInTheDocument();
    expect(screen.getByText('Not checked')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Information about DeepSeek R1' }));
    expect(screen.getByText('32,768 tokens (catalog value)')).toBeInTheDocument();
    expect(screen.getByText(/Context capacity does not measure answer quality/)).toBeInTheDocument();
    expect(screen.getByText(/offers room for document excerpts/)).toBeInTheDocument();
    expect(screen.queryByText(/Try for/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Information about DeepSeek R1' }));
    expect(screen.queryByText('Context window')).not.toBeInTheDocument();
  });

  it('provides cautious context guidance without making quality claims', () => {
    expect(modelContextGuidance({ ...model, context_window: 0 })).toContain('has not reported');
    expect(modelContextGuidance({ ...model, context_window: 8192 })).toContain('shorter excerpts');
    expect(modelContextGuidance({ ...model, context_window: 32768 })).toContain('conversation history');
    expect(modelContextGuidance({ ...model, context_window: 131072 })).toContain('effective limit depends on the configuration');
  });

  it('checks an existing connection without saving edits or overwriting model choices', async () => {
    setup([provider]); fireEvent.click(screen.getByText('Connection settings & models'));
    fireEvent.click(screen.getByRole('button', { name: 'Check connection' }));
    await screen.findByText('Server reached'); expect(api.saveProvider).not.toHaveBeenCalled(); expect(api.upsertModel).not.toHaveBeenCalled();
    expect(screen.getByText('All returned models have been added.')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Server address'), { target: { value: 'http://localhost:2222' } });
    expect(screen.getByText('Not checked')).toBeInTheDocument();
    expect(screen.queryByText('All returned models have been added.')).not.toBeInTheDocument();
  });

  it('retains enable and search override controls under information', async () => {
    setup([provider]); fireEvent.click(screen.getByRole('button', { name: 'Information about DeepSeek R1' }));
    fireEvent.click(screen.getByText('Model controls'));
    fireEvent.change(screen.getByLabelText('Web Search Support for DeepSeek R1'), { target: { value: 'unsupported' } });
    await waitFor(() => expect(api.updateModelSearchCapability).toHaveBeenCalledWith(model.id, 'unsupported'));
    fireEvent.click(screen.getByRole('button', { name: 'Disable model' }));
    await waitFor(() => expect(api.toggleModel).toHaveBeenCalledWith(model.id, false));
  });

  it('does not offer disabled models as defaults and does not infer readiness from saved entries', () => {
    setup([{ ...provider, models: [{ ...model, is_enabled: false }] }]);
    expect(screen.getByRole('button', { name: /is the workspace default/ })).toBeDisabled();
    expect(screen.getByText('Not checked')).toBeInTheDocument();
  });

  it('can manually add a model without inventing technical specifications', async () => {
    setup([{ ...provider, models: [] }]); fireEvent.click(screen.getByText('Connection settings & models'));
    fireEvent.click(screen.getByText('Add a model by identifier'));
    fireEvent.change(screen.getByLabelText('Model identifier'), { target: { value: 'custom-text-model' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add model' }));
    await waitFor(() => expect(api.upsertModel).toHaveBeenCalledWith(expect.objectContaining({ id: 'local:custom-text-model', context_window: 0, is_default: false, supports_structured_output: false })));
  });

  it('shows a model search only for larger catalogs', () => {
    setup([{ ...provider, models: Array.from({ length: 7 }, (_, i) => ({ ...model, id: `m${i}`, display_name: `Model ${i}` })) }]);
    fireEvent.change(screen.getByLabelText('Find a model'), { target: { value: 'Model 3' } });
    expect(within(screen.getByRole('region', { name: 'My server connection' })).getAllByRole('article')).toHaveLength(1);
  });

  it('uses cautious hints for unknown and specialist models and distinguishes cloud models', () => {
    expect(modelGuidance({ ...model, model_identifier: 'unknown', display_name: 'Unknown' }).label).toBe('Explore this model');
    expect(modelGuidance({ ...model, model_identifier: 'bge-m3', display_name: 'BGE' }).label).toBe('Specialist model');
    expect(modelGuidance({ ...model, model_identifier: 'sonar-pro', display_name: 'Sonar', web_search_capability: 'native' }).label).toBe('Try for research');
    expect(connectionLocation(provider, { ...model, model_identifier: 'gemma4:31b-cloud' })).toBe('Cloud model');
    expect(connectionLocation(provider, model)).toBe('Via local app');
    expect(connectionLocation({ ...provider, base_url: 'https://my-server.example' }, model)).toBe('Remote connection');
  });
});
