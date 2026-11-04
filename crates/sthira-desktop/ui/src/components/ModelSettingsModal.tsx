import React, { useState, useEffect, useMemo } from 'react';
import {
  X,
  Server,
  Eye,
  EyeOff,
  RefreshCw,
  Check,
  Trash2,
  Sparkles,
  Layers,
  Save,
  Plus,
  Globe,
  AlertCircle,
  Search,
  ExternalLink,
  ShieldCheck,
  Sun,
  Sprout,
  Link as LinkIcon,
  ChevronRight,
  MoreHorizontal,
} from 'lucide-react';
import { ThemeToggle } from './ThemeToggle';
import {
  getProviderCatalog,
  saveProvider,
  discoverProviderModels,
  upsertModel,
  deleteModel,
  deleteProvider,
  setDefaultModel,
  toggleModel,
  updateModelSearchCapability,
  clearVaultData,
  seedDemoWorkspace,
  getSearchProviders,
  saveSearchProvider,
  testSearchProvider,
} from '../api/ipc';
import type {
  ProviderEntry,
  ProviderModelEntry,
  SearchProviderEntry,
  WebSearchResultItem,
  WebSearchCapability,
} from '../types/sthira';

interface ModelSettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onVaultMutated: () => void;
  onCatalogChanged?: () => void;
  initialTab?: 'models' | 'search' | 'appearance' | 'vault';
}

interface ProviderPreset {
  id: string;
  display_name: string;
  provider_type: string;
  base_url: string;
  placeholder_key: string;
}

const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    id: 'openrouter',
    display_name: 'OpenRouter',
    provider_type: 'openai_compatible',
    base_url: 'https://openrouter.ai/api/v1',
    placeholder_key: 'sk-or-v1-...',
  },
  {
    id: 'anthropic',
    display_name: 'Anthropic Claude',
    provider_type: 'anthropic',
    base_url: 'https://api.anthropic.com/v1',
    placeholder_key: 'sk-ant-api03-...',
  },
  {
    id: 'groq',
    display_name: 'Groq Cloud',
    provider_type: 'openai_compatible',
    base_url: 'https://api.groq.com/openai/v1',
    placeholder_key: 'gsk_...',
  },
  {
    id: 'deepseek',
    display_name: 'DeepSeek',
    provider_type: 'openai_compatible',
    base_url: 'https://api.deepseek.com/v1',
    placeholder_key: 'sk-...',
  },
  {
    id: 'openai',
    display_name: 'OpenAI',
    provider_type: 'openai_compatible',
    base_url: 'https://api.openai.com/v1',
    placeholder_key: 'sk-proj-...',
  },
  {
    id: 'custom',
    display_name: 'Custom (OpenAI-Compatible)',
    provider_type: 'openai_compatible',
    base_url: 'http://localhost:8000/v1',
    placeholder_key: 'API key (optional)',
  },
];

export const ModelSettingsModal: React.FC<ModelSettingsModalProps> = ({
  isOpen,
  onClose,
  onVaultMutated,
  onCatalogChanged,
  initialTab,
}) => {
  const [providers, setProviders] = useState<ProviderEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [fetchingProviderId, setFetchingProviderId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [errorNotice, setErrorNotice] = useState<string | null>(null);
  const [showKeys, setShowKeys] = useState<Record<string, boolean>>({});
  const [keyInputs, setKeyInputs] = useState<Record<string, string>>({});
  const [urlInputs, setUrlInputs] = useState<Record<string, string>>({});

  const [activeTab, setActiveTab] = useState<'models' | 'search' | 'appearance' | 'vault'>(() => {
    if (initialTab) return initialTab;
    if (typeof window !== 'undefined') {
      const params = new URLSearchParams(window.location.search);
      const tabParam = params.get('tab') || params.get('settings_tab');
      if (tabParam === 'search' || tabParam === 'appearance' || tabParam === 'vault') {
        return tabParam;
      }
    }
    return 'models';
  });

  // Local model search & capability filter
  const [modelSearchQuery, setModelSearchQuery] = useState('');
  const [selectedCapabilityFilter, setSelectedCapabilityFilter] = useState<string>('all');

  // Search Providers State
  const [searchProviders, setSearchProviders] = useState<SearchProviderEntry[]>([]);
  const [, setLoadingSearchProviders] = useState(false);
  const [searchUrlInputs, setSearchUrlInputs] = useState<Record<string, string>>({});
  const [searchKeyInputs, setSearchKeyInputs] = useState<Record<string, string>>({});
  const [showSearchKeys, setShowSearchKeys] = useState<Record<string, boolean>>({});
  const [testQueryMap, setTestQueryMap] = useState<Record<string, string>>({});
  const [testResultsMap, setTestResultsMap] = useState<Record<string, WebSearchResultItem[]>>({});
  const [isTestingMap, setIsTestingMap] = useState<Record<string, boolean>>({});
  const [testErrorMap, setTestErrorMap] = useState<Record<string, string | null>>({});

  // Discovered models per connected provider
  const [discoveredModelsMap, setDiscoveredModelsMap] = useState<Record<string, ProviderModelEntry[]>>({});
  const [selectedDiscoveredModelMap, setSelectedDiscoveredModelMap] = useState<Record<string, string>>({});

  // Inline "Add Custom Model" form state per provider
  const [addingModelForProvider, setAddingModelForProvider] = useState<string | null>(null);
  const [newModelId, setNewModelId] = useState('');
  const [newModelDisplayName, setNewModelDisplayName] = useState('');

  // "Add Provider" modal / drawer state
  const [isAddingProvider, setIsAddingProvider] = useState(false);
  const [selectedPresetId, setSelectedPresetId] = useState<string>('openrouter');
  const [customProviderName, setCustomProviderName] = useState('OpenRouter');
  const [customProviderUrl, setCustomProviderUrl] = useState('https://openrouter.ai/api/v1');
  const [customProviderKey, setCustomProviderKey] = useState('');
  const [discoveredForNewProvider, setDiscoveredForNewProvider] = useState<ProviderModelEntry[]>([]);
  const [selectedModelForNewProvider, setSelectedModelForNewProvider] = useState<string>('');
  const [isFetchingNewProvider, setIsFetchingNewProvider] = useState(false);

  const fetchCatalog = async () => {
    try {
      setLoading(true);
      const catalog = await getProviderCatalog();
      setProviders(catalog || []);
      const urls: Record<string, string> = {};
      (catalog || []).forEach((p) => {
        urls[p.id] = p.base_url;
      });
      setUrlInputs(urls);
    } catch (err: any) {
      console.error('Failed to load provider catalog', err);
    } finally {
      setLoading(false);
    }
  };

  const fetchSearchProvidersList = async () => {
    try {
      setLoadingSearchProviders(true);
      const list = await getSearchProviders();
      setSearchProviders(list || []);
      const urls: Record<string, string> = {};
      const queries: Record<string, string> = {};
      (list || []).forEach((p) => {
        urls[p.id] = p.endpoint_url;
        queries[p.id] = 'rust async traits';
      });
      setSearchUrlInputs(urls);
      setTestQueryMap(queries);
    } catch (err: any) {
      console.error('Failed to load search providers', err);
    } finally {
      setLoadingSearchProviders(false);
    }
  };

  const handleToggleSearchProviderEnabled = async (provider: SearchProviderEntry) => {
    try {
      const updated: SearchProviderEntry = {
        ...provider,
        is_enabled: !provider.is_enabled,
      };
      await saveSearchProvider(updated);
      setNotice(`${provider.display_name} ${updated.is_enabled ? 'enabled' : 'disabled'}.`);
      await fetchSearchProvidersList();
      setTimeout(() => setNotice(null), 3000);
    } catch (err: any) {
      setErrorNotice(`Failed to update provider status: ${err?.message || err}`);
      setTimeout(() => setErrorNotice(null), 4000);
    }
  };

  const handleSetDefaultSearchProvider = async (provider: SearchProviderEntry) => {
    try {
      const updated: SearchProviderEntry = {
        ...provider,
        is_enabled: true,
        is_default: true,
      };
      await saveSearchProvider(updated);
      setNotice(`${provider.display_name} set as primary web search provider.`);
      await fetchSearchProvidersList();
      setTimeout(() => setNotice(null), 3000);
    } catch (err: any) {
      setErrorNotice(`Failed to set default search provider: ${err?.message || err}`);
      setTimeout(() => setErrorNotice(null), 4000);
    }
  };

  const handleSaveSearchProviderSettings = async (provider: SearchProviderEntry) => {
    try {
      const updatedUrl = searchUrlInputs[provider.id] || provider.endpoint_url;
      const apiKeyVal = searchKeyInputs[provider.id];
      const updated: SearchProviderEntry = {
        ...provider,
        endpoint_url: updatedUrl,
        api_key: apiKeyVal !== undefined ? (apiKeyVal.trim() === '' ? null : apiKeyVal.trim()) : provider.api_key,
      };
      await saveSearchProvider(updated);
      setNotice(`Updated settings for ${provider.display_name}.`);
      await fetchSearchProvidersList();
      setTimeout(() => setNotice(null), 3000);
    } catch (err: any) {
      setErrorNotice(`Failed to save provider settings: ${err?.message || err}`);
      setTimeout(() => setErrorNotice(null), 4000);
    }
  };

  const handleRunSearchTest = async (provider: SearchProviderEntry) => {
    const query = testQueryMap[provider.id]?.trim() || 'rust async traits';
    try {
      setIsTestingMap((prev) => ({ ...prev, [provider.id]: true }));
      setTestErrorMap((prev) => ({ ...prev, [provider.id]: null }));
      const results = await testSearchProvider(provider.id, query);
      setTestResultsMap((prev) => ({ ...prev, [provider.id]: results }));
      setNotice(`Test search completed: received ${results.length} results from ${provider.display_name}.`);
      setTimeout(() => setNotice(null), 3000);
    } catch (err: any) {
      const msg = typeof err === 'string' ? err : err?.message || 'Search request failed';
      setTestErrorMap((prev) => ({ ...prev, [provider.id]: msg }));
      setErrorNotice(`Test search error (${provider.display_name}): ${msg}`);
      setTimeout(() => setErrorNotice(null), 5000);
    } finally {
      setIsTestingMap((prev) => ({ ...prev, [provider.id]: false }));
    }
  };

  useEffect(() => {
    if (isOpen) {
      fetchCatalog();
      fetchSearchProvidersList();
    }
  }, [isOpen]);

  const totalRegisteredModels = useMemo(() => {
    return providers.reduce((sum, p) => sum + (p.models?.length || 0), 0);
  }, [providers]);

  const handleToggleKeyVisibility = (providerId: string) => {
    setShowKeys((prev) => ({ ...prev, [providerId]: !prev[providerId] }));
  };

  const handleSaveProviderSettings = async (provider: ProviderEntry) => {
    try {
      const updatedUrl = urlInputs[provider.id] || provider.base_url;
      const apiKeyVal = keyInputs[provider.id];
      const updated: ProviderEntry = {
        ...provider,
        base_url: updatedUrl,
        has_api_key: apiKeyVal && apiKeyVal.trim().length > 0 ? true : provider.has_api_key,
      };

      await saveProvider(updated, apiKeyVal && apiKeyVal.trim().length > 0 ? apiKeyVal.trim() : undefined);
      setNotice(`Saved settings for ${provider.display_name}`);
      setErrorNotice(null);
      await fetchCatalog();
      if (onCatalogChanged) onCatalogChanged();
    } catch (err: any) {
      setErrorNotice(`Save failed: ${err?.message || err}`);
    }
  };

  const handleFetchModels = async (provider: ProviderEntry) => {
    setFetchingProviderId(provider.id);
    setNotice(null);
    setErrorNotice(null);
    try {
      const endpoint = urlInputs[provider.id] || provider.base_url;
      const apiKey = keyInputs[provider.id];

      if (endpoint !== provider.base_url || apiKey) {
        await saveProvider(
          { ...provider, base_url: endpoint, has_api_key: !!apiKey || provider.has_api_key },
          apiKey && apiKey.trim().length > 0 ? apiKey.trim() : undefined
        );
      }

      const discovered = await discoverProviderModels(
        provider.id,
        provider.provider_type,
        endpoint,
        apiKey && apiKey.trim().length > 0 ? apiKey.trim() : undefined
      );

      setDiscoveredModelsMap((prev) => ({ ...prev, [provider.id]: discovered }));
      if (discovered.length > 0) {
        setSelectedDiscoveredModelMap((prev) => ({ ...prev, [provider.id]: discovered[0].model_identifier }));
        setNotice(`Retrieved ${discovered.length} model(s) from ${provider.display_name}. Choose a model from the dropdown to add it.`);
      } else {
        setNotice(`No models returned from ${provider.display_name}.`);
      }
    } catch (err: any) {
      setErrorNotice(`Fetch failed from ${provider.display_name}: ${err?.message || err}`);
    } finally {
      setFetchingProviderId(null);
    }
  };

  const handleAddSelectedDiscoveredModel = async (providerId: string) => {
    const selectedIdentifier = selectedDiscoveredModelMap[providerId];
    const available = discoveredModelsMap[providerId] || [];
    const modelToAdd = available.find((m) => m.model_identifier === selectedIdentifier);
    if (!modelToAdd) return;

    try {
      await upsertModel(modelToAdd);
      setNotice(`Added '${modelToAdd.display_name}' to ${providerId}`);
      await fetchCatalog();
      if (onCatalogChanged) onCatalogChanged();
    } catch (err: any) {
      setErrorNotice(`Failed to add model: ${err?.message || err}`);
    }
  };

  const handleAddAllDiscoveredModels = async (providerId: string) => {
    const available = discoveredModelsMap[providerId] || [];
    if (available.length === 0) return;
    try {
      for (const m of available) {
        await upsertModel(m);
      }
      setNotice(`Added ${available.length} models to ${providerId}`);
      await fetchCatalog();
      if (onCatalogChanged) onCatalogChanged();
    } catch (err: any) {
      setErrorNotice(`Failed to batch add models: ${err?.message || err}`);
    }
  };

  const handleAddCustomModel = async (providerId: string) => {
    if (!newModelId.trim()) {
      setErrorNotice('Model identifier cannot be empty');
      return;
    }
    const newEntry: ProviderModelEntry = {
      id: `${providerId}-${newModelId.trim().toLowerCase().replace(/[^a-z0-9]/g, '-')}`,
      provider_id: providerId,
      model_identifier: newModelId.trim(),
      display_name: newModelDisplayName.trim() || newModelId.trim(),
      context_window: 131072,
      supports_structured_output: true,
      is_enabled: true,
      is_default: false,
      web_search_capability: 'engine_augmented_only',
    };

    try {
      await upsertModel(newEntry);
      setNotice(`Added custom model '${newEntry.display_name}'`);
      setAddingModelForProvider(null);
      setNewModelId('');
      setNewModelDisplayName('');
      await fetchCatalog();
      if (onCatalogChanged) onCatalogChanged();
    } catch (err: any) {
      setErrorNotice(`Failed to add custom model: ${err?.message || err}`);
    }
  };

  const handleDiscoverForNewProvider = async () => {
    if (!customProviderUrl) return;
    setIsFetchingNewProvider(true);
    setNotice(null);
    setErrorNotice(null);
    try {
      const discovered = await discoverProviderModels(
        selectedPresetId,
        'openai_compatible',
        customProviderUrl,
        customProviderKey.trim() || undefined
      );
      setDiscoveredForNewProvider(discovered);
      if (discovered.length > 0) {
        setSelectedModelForNewProvider(discovered[0].model_identifier);
        setNotice(`Found ${discovered.length} available models`);
      } else {
        setNotice('Connection succeeded, but no models were returned.');
      }
    } catch (err: any) {
      setErrorNotice(`Discovery failed: ${err?.message || err}`);
    } finally {
      setIsFetchingNewProvider(false);
    }
  };

  const handleSaveNewProvider = async () => {
    if (!customProviderName.trim()) {
      setErrorNotice('Provider name is required');
      return;
    }
    try {
      const pId = selectedPresetId === 'custom'
        ? `custom-${Date.now()}`
        : selectedPresetId;

      const newProvider: ProviderEntry = {
        id: pId,
        provider_type: 'openai_compatible',
        display_name: customProviderName.trim(),
        base_url: customProviderUrl.trim(),
        has_api_key: customProviderKey.trim().length > 0,
        is_enabled: true,
        models: [],
      };

      await saveProvider(newProvider, customProviderKey.trim() || undefined);

      if (selectedModelForNewProvider) {
        const found = discoveredForNewProvider.find(
          (m) => m.model_identifier === selectedModelForNewProvider
        );
        if (found) {
          await upsertModel({ ...found, provider_id: pId, is_default: true });
        }
      }

      setNotice(`Connected provider '${newProvider.display_name}'`);
      setIsAddingProvider(false);
      setCustomProviderKey('');
      setDiscoveredForNewProvider([]);
      setSelectedModelForNewProvider('');
      await fetchCatalog();
      if (onCatalogChanged) onCatalogChanged();
    } catch (err: any) {
      setErrorNotice(`Failed to connect provider: ${err?.message || err}`);
    }
  };

  const handleDeleteModel = async (modelId: string, modelName: string) => {
    if (!confirm(`Are you sure you want to remove '${modelName}'?`)) return;
    try {
      await deleteModel(modelId);
      setNotice(`Removed model '${modelName}'`);
      await fetchCatalog();
      if (onCatalogChanged) onCatalogChanged();
    } catch (err: any) {
      setErrorNotice(`Delete failed: ${err?.message || err}`);
    }
  };

  const handleDeleteProvider = async (providerId: string, providerName: string) => {
    if (!confirm(`Remove provider '${providerName}' and all its registered models?`)) return;
    try {
      await deleteProvider(providerId);
      setNotice(`Removed provider '${providerName}'`);
      await fetchCatalog();
      if (onCatalogChanged) onCatalogChanged();
    } catch (err: any) {
      setErrorNotice(`Provider removal failed: ${err?.message || err}`);
    }
  };

  const handleSetDefault = async (modelId: string, modelName: string) => {
    try {
      await setDefaultModel(modelId);
      setNotice(`'${modelName}' set as default workspace model`);
      await fetchCatalog();
      if (onCatalogChanged) onCatalogChanged();
    } catch (err: any) {
      setErrorNotice(`Failed to set default: ${err?.message || err}`);
    }
  };

  const handleToggleModelEnabled = async (modelId: string, currentEnabled: boolean) => {
    try {
      await toggleModel(modelId, !currentEnabled);
      await fetchCatalog();
      if (onCatalogChanged) onCatalogChanged();
    } catch (err: any) {
      setErrorNotice(`Failed to toggle model: ${err?.message || err}`);
    }
  };

  const handleUpdateCapability = async (modelId: string, capability: WebSearchCapability) => {
    try {
      await updateModelSearchCapability(modelId, capability);
      setProviders((prev) =>
        prev.map((prov) => ({
          ...prov,
          models: prov.models.map((m) =>
            m.id === modelId ? { ...m, web_search_capability: capability } : m
          ),
        }))
      );
      if (onCatalogChanged) onCatalogChanged();
      setNotice(`Updated web search support to ${capability}`);
    } catch (err: any) {
      setErrorNotice(`Failed to update capability: ${err?.message || err}`);
    }
  };

  const handleResetVault = async () => {
    if (!confirm('Are you sure you want to clear all projects, notes, and epistemic claims?')) {
      return;
    }
    setLoading(true);
    try {
      await clearVaultData();
      setNotice('Vault cleared to clean start.');
      onVaultMutated();
    } catch (err: any) {
      setErrorNotice(`Error: ${err?.message || err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleLoadDemo = async () => {
    setLoading(true);
    try {
      await seedDemoWorkspace();
      setNotice('Demo workspace seeded successfully.');
      onVaultMutated();
    } catch (err: any) {
      setErrorNotice(`Error: ${err?.message || err}`);
    } finally {
      setLoading(false);
    }
  };

  const getProviderIcon = (providerType: string, providerId: string) => {
    if (providerId === 'ollama' || providerType === 'ollama') return '🦙';
    if (providerId === 'anthropic' || providerType === 'anthropic') return '⚡';
    if (providerId === 'openrouter') return '🌐';
    if (providerId === 'groq') return '🚀';
    if (providerId === 'deepseek') return '🐋';
    return '☁️';
  };

  // Helper for model capability badge
  const getModelTypeBadge = (m: ProviderModelEntry): { text: string; color: 'mint' | 'peach' | 'slate' | 'amber' } => {
    const id = (m.model_identifier || m.id || '').toLowerCase();
    if (id.includes('1b') || id.includes('3b') || id.includes('flash') || id.includes('mini') || id.includes('light')) {
      return { text: '⚡ Fast & Lightweight', color: 'mint' };
    }
    if (id.includes('70b') || id.includes('120b') || id.includes('pro') || id.includes('claude-3') || id.includes('gpt-4') || id.includes('41b')) {
      return { text: '✨ High Capability', color: 'peach' };
    }
    if (id.includes('llama3.2') || id.includes('8b') || id.includes('efficient')) {
      return { text: '📖 Efficient & Reliable', color: 'slate' };
    }
    return { text: '✨ General Purpose', color: 'amber' };
  };

  // Filtered models for current search/capability dropdown
  const filterModels = (models: ProviderModelEntry[]) => {
    return models.filter((m) => {
      const query = modelSearchQuery.trim().toLowerCase();
      const matchesSearch = query
        ? m.display_name.toLowerCase().includes(query) ||
          m.model_identifier.toLowerCase().includes(query)
        : true;

      if (!matchesSearch) return false;

      if (selectedCapabilityFilter === 'all') return true;
      if (selectedCapabilityFilter === 'search') {
        return m.web_search_capability && m.web_search_capability !== 'unsupported';
      }
      if (selectedCapabilityFilter === 'fast') {
        return getModelTypeBadge(m).text.includes('Fast');
      }
      if (selectedCapabilityFilter === 'high') {
        return getModelTypeBadge(m).text.includes('High');
      }
      return true;
    });
  };

  const guidanceInfo = useMemo(() => {
    switch (activeTab) {
      case 'search':
        return {
          icon: <Globe className="w-4 h-4" />,
          title: 'Ground your research with live web context.',
          desc: 'Configure search engines and headless scrapers to give models access to up-to-date web pages and citations.',
          footer: `${searchProviders.length} ${searchProviders.length === 1 ? 'provider' : 'providers'} · ${searchProviders.filter((p) => p.is_enabled).length} active`,
        };
      case 'appearance':
        return {
          icon: <Sun className="w-4 h-4" />,
          title: 'Craft your contemplative workspace.',
          desc: 'Choose visual themes and color balance designed for deep reading, minimal glare, and epistemic clarity.',
          footer: 'Warm Stone · Deep Obsidian · System',
        };
      case 'vault':
        return {
          icon: <Layers className="w-4 h-4" />,
          title: 'Local-first storage & atomic safety.',
          desc: 'SQLite database transactions adhere strictly to the 7-Layer Reversibility Protocol with atomic rollback guarantees.',
          footer: '7-Layer Protocol · SQLite Engine',
        };
      case 'models':
      default:
        return {
          icon: <Layers className="w-4 h-4" />,
          title: 'Your AI workspace, your way.',
          desc: 'Connect the model providers you use, choose your models, and set up a default for everyday work.',
          footer: `${providers.length} ${providers.length === 1 ? 'provider' : 'providers'} · ${totalRegisteredModels} registered ${totalRegisteredModels === 1 ? 'model' : 'models'}`,
        };
    }
  }, [activeTab, providers.length, totalRegisteredModels, searchProviders]);

  if (!isOpen) return null;

  return (
    <div
      className="settings-modal-backdrop"
      data-testid="model-settings-modal"
    >
      <div className="settings-modal-dialog animate-in fade-in zoom-in-95 duration-150">
        {/* 1. Header with Brand Badge & Amber Rule */}
        <div className="settings-header">
          {/* Subtle misty mountain wash in top right */}
          <div className="settings-header-art" aria-hidden="true">
            <svg viewBox="0 0 540 90" fill="none" preserveAspectRatio="none" className="w-full h-full opacity-35 dark:opacity-15">
              <path d="M0 90 Q90 55 180 68 T360 45 T540 58 L540 90 Z" fill="#9FB096" />
              <path d="M100 90 Q200 35 300 48 T540 25 L540 90 Z" fill="#667A58" />
              <path d="M200 90 Q320 15 420 25 T540 10 L540 90 Z" fill="#49583D" />
            </svg>
          </div>

          <div className="settings-header-left">
            <div className="settings-brand-badge">
              <div className="settings-brand-icon">
                <Sprout className="w-4 h-4" />
              </div>
              <span className="settings-brand-name">Sthira</span>
            </div>
            <div className="settings-title-section">
              <div className="settings-amber-rule" />
              <h2 className="settings-title">Model &amp; Workspace Settings</h2>
              <p className="settings-subtitle">
                Configure your models, tools, and workspace experience.
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close Settings"
            className="settings-close-btn"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* 2. Horizontal Navigation Tabs */}
        <div className="settings-tabs-row">
          <button
            type="button"
            onClick={() => setActiveTab('models')}
            className={`settings-tab-pill ${activeTab === 'models' ? 'active' : ''}`}
          >
            <Server className="w-3.5 h-3.5" />
            <span>Connected Providers &amp; Models</span>
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('search')}
            className={`settings-tab-pill ${activeTab === 'search' ? 'active' : ''}`}
          >
            <Globe className="w-3.5 h-3.5" />
            <span>Web Search Providers</span>
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('appearance')}
            className={`settings-tab-pill ${activeTab === 'appearance' ? 'active' : ''}`}
          >
            <Sun className="w-3.5 h-3.5" />
            <span>Appearance</span>
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('vault')}
            className={`settings-tab-pill ${activeTab === 'vault' ? 'active' : ''}`}
          >
            <Layers className="w-3.5 h-3.5" />
            <span>Vault &amp; Storage</span>
          </button>
        </div>

        {/* Notifications */}
        {notice && (
          <div className="mx-8 mt-4 p-3 bg-[#E3EBE3] dark:bg-[#1C2B1B] border border-[#CCDCCC] dark:border-[#2D452B] text-[#2D5A27] dark:text-[#879E78] rounded-xl flex items-center gap-2 font-medium text-xs animate-in fade-in">
            <Check className="w-4 h-4 shrink-0" />
            <span>{notice}</span>
          </div>
        )}

        {errorNotice && (
          <div className="mx-8 mt-4 p-3 bg-[#FDF2F2] dark:bg-[#2B1717] border border-[#F8D7DA] dark:border-[#4A2020] text-[#9E2A2B] dark:text-[#F87171] rounded-xl flex items-center gap-2 font-medium text-xs animate-in fade-in">
            <AlertCircle className="w-4 h-4 shrink-0" />
            <span>{errorNotice}</span>
          </div>
        )}

        {/* 3. Main Modal Body (2-Column Grid Across All Tabs) */}
        <div className="settings-body-grid">
          {/* Left Guidance Column */}
          <div className="settings-guidance-col">
            <div className="settings-guidance-card">
              {/* Subtle mountain wash in guidance card */}
              <div className="settings-card-art" aria-hidden="true">
                <svg viewBox="0 0 240 70" fill="none" preserveAspectRatio="none" className="w-full h-full opacity-25 dark:opacity-10">
                  <path d="M0 70 Q60 40 120 50 T240 30 L240 70 Z" fill="#667A58" />
                  <path d="M80 70 Q140 20 200 25 T240 15 L240 70 Z" fill="#49583D" />
                </svg>
              </div>

              <div className="settings-guidance-icon">
                {guidanceInfo.icon}
              </div>
              <h3 className="settings-guidance-title">{guidanceInfo.title}</h3>
              <p className="settings-guidance-desc">
                {guidanceInfo.desc}
              </p>
              <div className="mt-3 pt-3 border-t border-sthira-border/50 text-[11px] text-sthira-text-muted">
                {guidanceInfo.footer}
              </div>
            </div>

            <div className="settings-quick-setup">
              <div className="settings-quick-label">Quick setup</div>
              <button
                type="button"
                onClick={() => setActiveTab('models')}
                className={`settings-quick-row ${activeTab === 'models' ? 'active' : ''}`}
                data-testid="quick-setup-models"
              >
                <div className="settings-quick-row-left">
                  <Server className="w-4 h-4 text-sthira-text-muted" />
                  <div>
                    <span className="settings-quick-title">Connected Providers</span>
                    <span className="settings-quick-sub">Manage AI models &amp; keys</span>
                  </div>
                </div>
                <ChevronRight className="w-4 h-4 text-sthira-text-muted" />
              </button>

              <button
                type="button"
                onClick={() => setActiveTab('search')}
                className={`settings-quick-row ${activeTab === 'search' ? 'active' : ''}`}
                data-testid="quick-setup-search"
              >
                <div className="settings-quick-row-left">
                  <Globe className="w-4 h-4 text-sthira-text-muted" />
                  <div>
                    <span className="settings-quick-title">Search Tools</span>
                    <span className="settings-quick-sub">Add and configure search tools</span>
                  </div>
                </div>
                <ChevronRight className="w-4 h-4 text-sthira-text-muted" />
              </button>

              <button
                type="button"
                onClick={() => setActiveTab('appearance')}
                className={`settings-quick-row ${activeTab === 'appearance' ? 'active' : ''}`}
                data-testid="quick-setup-appearance"
              >
                <div className="settings-quick-row-left">
                  <Sun className="w-4 h-4 text-sthira-text-muted" />
                  <div>
                    <span className="settings-quick-title">Theme &amp; Display</span>
                    <span className="settings-quick-sub">Themes, look &amp; feel, and more</span>
                  </div>
                </div>
                <ChevronRight className="w-4 h-4 text-sthira-text-muted" />
              </button>

              <button
                type="button"
                onClick={() => setActiveTab('vault')}
                className={`settings-quick-row ${activeTab === 'vault' ? 'active' : ''}`}
                data-testid="quick-setup-vault"
              >
                <div className="settings-quick-row-left">
                  <Layers className="w-4 h-4 text-sthira-text-muted" />
                  <div>
                    <span className="settings-quick-title">Storage &amp; Vault</span>
                    <span className="settings-quick-sub">Set up knowledge, files, and sync</span>
                  </div>
                </div>
                <ChevronRight className="w-4 h-4 text-sthira-text-muted" />
              </button>
            </div>

            {/* Botanical Leaf Art */}
            <div className="settings-botanical-art hidden sm:flex" aria-hidden="true">
              <svg viewBox="0 0 160 90" fill="none" className="w-28 h-auto opacity-35 dark:opacity-15 text-sthira-primary">
                <path d="M20 85 C40 55 80 40 140 20 C100 45 70 75 60 88" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                <path d="M55 52 C75 35 110 32 135 22 C105 40 85 55 75 68" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
                <path d="M35 70 C50 55 75 50 95 45 C75 60 60 72 50 82" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
              </svg>
            </div>
          </div>

          {/* Right Content Column */}
          <div className="settings-content-col">
            {activeTab === 'models' && (
              <div className="space-y-6 animate-in fade-in duration-100">
                <div className="settings-section-header">
                  <h3 className="settings-section-title">Connected Providers &amp; Models</h3>
                  <p className="settings-section-sub">
                    Add model providers, manage available models, and choose a default for your workspace.
                  </p>
                </div>

              {/* Add New Provider Card Form */}
              {isAddingProvider && (
                <div className="border-2 border-sthira-primary rounded-2xl p-5 bg-sthira-surface space-y-4 shadow-sm animate-in fade-in">
                  <div className="flex items-center justify-between">
                    <span className="font-bold text-sm text-sthira-text flex items-center gap-2">
                      <Globe className="w-4 h-4 text-sthira-primary" />
                      Connect Compute Provider (OmniRouter)
                    </span>
                    <button
                      type="button"
                      onClick={() => setIsAddingProvider(false)}
                      className="p-1 rounded-lg text-sthira-text-muted hover:text-sthira-text"
                    >
                      <X className="w-4 h-4" />
                    </button>
                  </div>

                  <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                    <div>
                      <label className="block text-[11px] font-semibold text-sthira-text-muted mb-1">
                        Select Provider Preset
                      </label>
                      <select
                        value={selectedPresetId}
                        onChange={(e) => {
                          const pId = e.target.value;
                          setSelectedPresetId(pId);
                          const preset = PROVIDER_PRESETS.find((p) => p.id === pId);
                          if (preset) {
                            setCustomProviderName(preset.display_name);
                            setCustomProviderUrl(preset.base_url);
                          }
                        }}
                        className="w-full px-3 py-1.5 bg-sthira-surface border border-sthira-border rounded-lg text-xs text-sthira-text focus:outline-none focus:border-sthira-primary"
                      >
                        {PROVIDER_PRESETS.map((p) => (
                          <option key={p.id} value={p.id}>
                            {p.display_name}
                          </option>
                        ))}
                      </select>
                    </div>

                    <div>
                      <label className="block text-[11px] font-semibold text-sthira-text-muted mb-1">
                        Display Name
                      </label>
                      <input
                        type="text"
                        placeholder="e.g. OpenRouter / Groq / LM Studio"
                        value={customProviderName}
                        onChange={(e) => setCustomProviderName(e.target.value)}
                        className="w-full px-3 py-1.5 bg-sthira-surface border border-sthira-border rounded-lg text-xs text-sthira-text focus:outline-none focus:border-sthira-primary"
                      />
                    </div>

                    <div className="md:col-span-2">
                      <label className="block text-[11px] font-semibold text-sthira-text-muted mb-1">
                        Base Endpoint URL
                      </label>
                      <input
                        type="text"
                        placeholder="https://openrouter.ai/api/v1"
                        value={customProviderUrl}
                        onChange={(e) => setCustomProviderUrl(e.target.value)}
                        className="w-full px-3 py-1.5 bg-sthira-surface border border-sthira-border rounded-lg text-xs font-mono text-sthira-text focus:outline-none focus:border-sthira-primary"
                      />
                    </div>

                    <div className="md:col-span-2">
                      <label className="block text-[11px] font-semibold text-sthira-text-muted mb-1">
                        API Key (Optional for Local / Ollama)
                      </label>
                      <input
                        type="password"
                        placeholder={
                          PROVIDER_PRESETS.find((p) => p.id === selectedPresetId)?.placeholder_key ||
                          'API key...'
                        }
                        value={customProviderKey}
                        onChange={(e) => setCustomProviderKey(e.target.value)}
                        className="w-full px-3 py-1.5 bg-sthira-surface border border-sthira-border rounded-lg text-xs font-mono text-sthira-text focus:outline-none focus:border-sthira-primary"
                      />
                    </div>
                  </div>

                  <div className="flex items-center justify-between pt-2 border-t border-sthira-border">
                    <button
                      type="button"
                      onClick={handleDiscoverForNewProvider}
                      disabled={isFetchingNewProvider}
                      className="px-3 py-1.5 rounded-xl border border-sthira-border bg-sthira-surface hover:bg-sthira-surface-muted text-xs font-semibold text-sthira-text flex items-center gap-1.5 transition-colors cursor-pointer"
                    >
                      {isFetchingNewProvider ? (
                        <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                      ) : (
                        <Sparkles className="w-3.5 h-3.5" />
                      )}
                      <span>Test &amp; Fetch Models</span>
                    </button>

                    <button
                      type="button"
                      onClick={handleSaveNewProvider}
                      className="settings-btn-primary"
                    >
                      <Save className="w-3.5 h-3.5" />
                      <span>Save Provider</span>
                    </button>
                  </div>

                  {discoveredForNewProvider.length > 0 && (
                    <div className="pt-2 border-t border-sthira-border">
                      <label className="block text-[11px] font-semibold text-sthira-text-muted mb-1">
                        Select Default Model to Seed
                      </label>
                      <select
                        value={selectedModelForNewProvider}
                        onChange={(e) => setSelectedModelForNewProvider(e.target.value)}
                        className="w-full px-3 py-1.5 bg-sthira-surface border border-sthira-border rounded-lg text-xs text-sthira-text focus:outline-none focus:border-sthira-primary"
                      >
                        {discoveredForNewProvider.map((m) => (
                          <option key={m.model_identifier} value={m.model_identifier}>
                            {m.display_name} ({m.model_identifier})
                          </option>
                        ))}
                      </select>
                    </div>
                  )}
                </div>
              )}

              {/* Connected Provider Cards List */}
              {loading && providers.length === 0 ? (
                <div className="p-8 text-center text-xs text-sthira-text-muted">
                  Loading providers catalog...
                </div>
              ) : providers.length === 0 ? (
                <div className="p-8 rounded-2xl border border-sthira-border bg-sthira-surface text-center space-y-2">
                  <Server className="w-8 h-8 mx-auto text-sthira-text-muted" />
                  <h4 className="font-bold text-sm text-sthira-text">No Model Providers Connected</h4>
                  <p className="text-xs text-sthira-text-muted max-w-sm mx-auto">
                    Connect local models via Ollama or cloud providers like OpenRouter and Anthropic to power research.
                  </p>
                </div>
              ) : (
                <div className="space-y-4">
                  {providers.map((provider) => {
                    const isFetching = fetchingProviderId === provider.id;
                    const isKeyVisible = !!showKeys[provider.id];
                    const discovered = discoveredModelsMap[provider.id] || [];
                    const isConnected = provider.models.length > 0;
                    const displayedModels = filterModels(provider.models);

                    return (
                      <div
                        key={provider.id}
                        className="settings-provider-card"
                      >
                        {/* Provider Header */}
                        <div className="settings-provider-header">
                          <div className="settings-provider-meta">
                            <div className="settings-provider-avatar">
                              {getProviderIcon(provider.provider_type, provider.id)}
                            </div>
                            <div>
                              <div className="settings-provider-name-row">
                                <span className="settings-provider-name">{provider.display_name}</span>
                                <span className="settings-provider-count-badge">
                                  {provider.models.length} {provider.models.length === 1 ? 'model' : 'models'}
                                </span>
                              </div>
                              <p className="settings-provider-desc">
                                {provider.id === 'ollama'
                                  ? 'Run open models on your own machine with Ollama.'
                                  : `Cloud inference via ${provider.display_name}.`}
                              </p>
                            </div>
                          </div>

                          <div className="flex items-center gap-2">
                            <span
                              className={`settings-conn-badge ${
                                isConnected ? 'connected' : 'unconnected'
                              }`}
                            >
                              {isConnected ? (
                                <>
                                  <Check className="w-3 h-3" />
                                  <span>Connected</span>
                                </>
                              ) : (
                                <span>Not Connected</span>
                              )}
                            </span>
                            <button
                              type="button"
                              onClick={() => handleDeleteProvider(provider.id, provider.display_name)}
                              className="p-1 rounded text-sthira-text-muted hover:text-sthira-danger cursor-pointer"
                              title="Remove Provider"
                            >
                              <MoreHorizontal className="w-4 h-4" />
                            </button>
                          </div>
                        </div>

                        {/* Endpoint URL Row */}
                        <div>
                          <label className="settings-endpoint-label">Endpoint URL</label>
                          <div className="settings-endpoint-box">
                            <div className="relative flex-1 min-w-[240px]">
                              <LinkIcon className="w-3.5 h-3.5 absolute left-3 top-1/2 -translate-y-1/2 text-sthira-text-muted" />
                              <input
                                type="text"
                                value={urlInputs[provider.id] ?? provider.base_url}
                                onChange={(e) =>
                                  setUrlInputs((prev) => ({
                                    ...prev,
                                    [provider.id]: e.target.value,
                                  }))
                                }
                                placeholder="http://localhost:11434"
                                className="settings-endpoint-input pl-8 w-full"
                              />
                            </div>

                            <button
                              type="button"
                              onClick={() => handleFetchModels(provider)}
                              disabled={isFetching}
                              className="settings-btn-secondary"
                            >
                              {isFetching ? (
                                <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                              ) : (
                                <RefreshCw className="w-3.5 h-3.5" />
                              )}
                              <span>Test Connection</span>
                            </button>

                            <button
                              type="button"
                              onClick={() => handleSaveProviderSettings(provider)}
                              className="settings-btn-secondary"
                            >
                              <Save className="w-3.5 h-3.5" />
                              <span>Save</span>
                            </button>
                          </div>
                        </div>

                        {/* Optional API Key Input */}
                        {provider.provider_type !== 'ollama' && (
                          <div className="grid grid-cols-1 md:grid-cols-2 gap-3 pt-2 border-t border-sthira-border">
                            <div>
                              <label className="block text-[11px] font-semibold text-sthira-text-muted mb-1">
                                API Key {provider.has_api_key ? '(Configured)' : '(Required)'}
                              </label>
                              <div className="relative">
                                <input
                                  type={isKeyVisible ? 'text' : 'password'}
                                  placeholder={provider.has_api_key ? '••••••••••••••••' : 'Enter API Key...'}
                                  value={keyInputs[provider.id] || ''}
                                  onChange={(e) =>
                                    setKeyInputs((prev) => ({
                                      ...prev,
                                      [provider.id]: e.target.value,
                                    }))
                                  }
                                  className="w-full pl-3 pr-8 py-1.5 rounded-lg bg-sthira-surface-muted border border-sthira-border text-xs font-mono text-sthira-text focus:bg-sthira-surface focus:outline-none"
                                />
                                <button
                                  type="button"
                                  onClick={() => handleToggleKeyVisibility(provider.id)}
                                  className="absolute right-2 top-2 text-sthira-text-muted hover:text-sthira-text cursor-pointer"
                                >
                                  {isKeyVisible ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                                </button>
                              </div>
                            </div>

                            <div className="flex items-end">
                              <button
                                type="button"
                                onClick={() =>
                                  setAddingModelForProvider(
                                    addingModelForProvider === provider.id ? null : provider.id
                                  )
                                }
                                className="px-3 py-1.5 rounded-lg border border-dashed border-sthira-border hover:bg-sthira-surface-muted text-xs text-sthira-text-muted hover:text-sthira-text flex items-center gap-1.5 transition-colors cursor-pointer w-full justify-center"
                              >
                                <Plus className="w-3.5 h-3.5" />
                                <span>Add Custom Model ID</span>
                              </button>
                            </div>
                          </div>
                        )}

                        {/* Discovered Models Dropdown Bar */}
                        {discovered.length > 0 && (
                          <div className="p-3 bg-sthira-surface-muted border border-sthira-border rounded-xl space-y-2">
                            <span className="text-[11px] font-semibold text-sthira-text">
                              Discovered {discovered.length} Available Models:
                            </span>
                            <div className="flex items-center gap-2 flex-wrap">
                              <select
                                value={selectedDiscoveredModelMap[provider.id] || ''}
                                onChange={(e) =>
                                  setSelectedDiscoveredModelMap((prev) => ({
                                    ...prev,
                                    [provider.id]: e.target.value,
                                  }))
                                }
                                className="flex-1 min-w-[200px] px-2.5 py-1.5 bg-sthira-surface border border-sthira-border rounded-lg text-xs text-sthira-text focus:outline-none"
                              >
                                {discovered.map((m) => (
                                  <option key={m.model_identifier} value={m.model_identifier}>
                                    {m.display_name} ({m.model_identifier})
                                  </option>
                                ))}
                              </select>
                              <button
                                type="button"
                                onClick={() => handleAddSelectedDiscoveredModel(provider.id)}
                                className="settings-btn-primary"
                              >
                                <Plus className="w-3.5 h-3.5" />
                                <span>Add Model</span>
                              </button>
                              <button
                                type="button"
                                onClick={() => handleAddAllDiscoveredModels(provider.id)}
                                className="settings-btn-secondary"
                              >
                                <span>Add All ({discovered.length})</span>
                              </button>
                            </div>
                          </div>
                        )}

                        {/* Inline Custom Model Entry Form */}
                        {addingModelForProvider === provider.id && (
                          <div className="p-3 bg-sthira-surface-muted border border-sthira-border rounded-xl space-y-2 animate-in fade-in">
                            <span className="text-xs font-semibold text-sthira-text">Register Custom Model ID</span>
                            <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                              <input
                                type="text"
                                placeholder="Identifier: e.g. mistral-small-latest"
                                value={newModelId}
                                onChange={(e) => setNewModelId(e.target.value)}
                                className="px-2.5 py-1 rounded bg-sthira-surface border border-sthira-border text-xs text-sthira-text"
                              />
                              <input
                                type="text"
                                placeholder="Display Name: e.g. Mistral Small"
                                value={newModelDisplayName}
                                onChange={(e) => setNewModelDisplayName(e.target.value)}
                                className="px-2.5 py-1 rounded bg-sthira-surface border border-sthira-border text-xs text-sthira-text"
                              />
                            </div>
                            <div className="flex justify-end gap-2 pt-1">
                              <button
                                type="button"
                                onClick={() => setAddingModelForProvider(null)}
                                className="px-3 py-1 text-xs text-sthira-text-muted hover:text-sthira-text cursor-pointer"
                              >
                                Cancel
                              </button>
                              <button
                                type="button"
                                onClick={() => handleAddCustomModel(provider.id)}
                                className="settings-btn-primary"
                              >
                                Save Model
                              </button>
                            </div>
                          </div>
                        )}

                        {/* Available Models Section */}
                        <div className="settings-models-header">
                          <div className="settings-models-title-row">
                            <span className="settings-models-title">Available Models</span>
                            <span className="settings-provider-count-badge">
                              {provider.models.length}
                            </span>
                          </div>

                          <div className="settings-models-controls">
                            <div className="relative">
                              <Search className="w-3 h-3 absolute left-2.5 top-1/2 -translate-y-1/2 text-sthira-text-muted" />
                              <input
                                type="text"
                                value={modelSearchQuery}
                                onChange={(e) => setModelSearchQuery(e.target.value)}
                                placeholder="Search models..."
                                className="settings-model-search"
                              />
                            </div>

                            <select
                              value={selectedCapabilityFilter}
                              onChange={(e) => setSelectedCapabilityFilter(e.target.value)}
                              className="settings-select-capability"
                            >
                              <option value="all">All Capabilities</option>
                              <option value="search">Web Search Ready</option>
                              <option value="fast">Fast &amp; Lightweight</option>
                              <option value="high">High Capability</option>
                            </select>
                          </div>
                        </div>

                        {provider.models.length === 0 ? (
                          <div className="p-4 bg-sthira-surface-muted border border-sthira-border rounded-xl text-sthira-text-muted text-center space-y-1">
                            <p className="italic">No models fetched yet for {provider.display_name}.</p>
                            <p className="text-[11px]">
                              Click <strong>Test Connection</strong> to dynamically query available models from this endpoint.
                            </p>
                          </div>
                        ) : displayedModels.length === 0 ? (
                          <div className="p-4 bg-sthira-surface-muted border border-sthira-border rounded-xl text-sthira-text-muted text-center text-xs">
                            No models match your current filter.
                          </div>
                        ) : (
                          <div className="space-y-2">
                            {displayedModels.map((m) => {
                              const { text: typeText, color: typeColor } = getModelTypeBadge(m);
                              const hasSearch = m.web_search_capability && m.web_search_capability !== 'unsupported';
                              const ctxKb = Math.round(m.context_window / 1024);

                              return (
                                <div
                                  key={m.id}
                                  className={`settings-model-row ${m.is_default ? 'is-default' : ''}`}
                                >
                                  <div className="settings-model-left">
                                    <button
                                      type="button"
                                      onClick={() => handleSetDefault(m.id, m.display_name)}
                                      className={`settings-radio-btn ${m.is_default ? 'checked' : ''}`}
                                      title={m.is_default ? 'Current Default Model' : 'Set as Default Model'}
                                    >
                                      {m.is_default && <div className="settings-radio-inner" />}
                                    </button>

                                    <div className="settings-model-meta">
                                      <span className="settings-model-name truncate">
                                        {m.display_name}
                                      </span>
                                      <span className="settings-model-sub truncate">
                                        {m.model_identifier}
                                      </span>
                                    </div>
                                  </div>

                                  <div className="settings-model-chips">
                                    <span className={`settings-chip ${typeColor}`}>
                                      {typeText}
                                    </span>

                                    {/* Capability selection dropdown for explicit testing and configuration */}
                                    <select
                                      value={m.web_search_capability || 'engine_augmented_only'}
                                      onChange={(e) =>
                                        handleUpdateCapability(
                                          m.id,
                                          e.target.value as WebSearchCapability
                                        )
                                      }
                                      className="settings-capability-select"
                                      data-testid={`select-capability-${m.id}`}
                                      aria-label={`Web Search Support for ${m.display_name}`}
                                    >
                                      <option value="agentic_tool_call">Tool Calling</option>
                                      <option value="native">Native Grounding</option>
                                      <option value="engine_augmented_only">Web Scrape Injected</option>
                                      <option value="unsupported">Offline Only</option>
                                    </select>

                                    <span className="settings-chip">
                                      {hasSearch ? (
                                        <span className="flex items-center gap-1 text-emerald-700 dark:text-emerald-400">
                                          <Globe className="w-3 h-3" />
                                          <span>Web Search</span>
                                        </span>
                                      ) : (
                                        <span className="flex items-center gap-1 text-sthira-text-muted">
                                          <span>⊗ No Web Search</span>
                                        </span>
                                      )}
                                    </span>

                                    <span className="settings-chip">
                                      <Layers className="w-3 h-3" />
                                      <span>{ctxKb}K context</span>
                                    </span>

                                    {m.is_default && (
                                      <span className="settings-chip default-badge">
                                        Default
                                      </span>
                                    )}

                                    <button
                                      type="button"
                                      onClick={() => handleToggleModelEnabled(m.id, m.is_enabled)}
                                      className={`px-2 py-0.5 rounded text-[10px] font-semibold cursor-pointer transition-colors ${
                                        m.is_enabled
                                          ? 'bg-[#E3EBE3] text-[#2D5A27] dark:bg-[#1C2B1B] dark:text-[#879E78]'
                                          : 'bg-sthira-surface-muted text-sthira-text-muted'
                                      }`}
                                      title={m.is_enabled ? 'Disable model' : 'Enable model'}
                                    >
                                      {m.is_enabled ? 'Enabled' : 'Disabled'}
                                    </button>

                                    <button
                                      type="button"
                                      onClick={() => handleDeleteModel(m.id, m.display_name)}
                                      className="text-sthira-text-muted hover:text-sthira-danger p-1 rounded cursor-pointer"
                                      title="Delete model"
                                    >
                                      <Trash2 className="w-3 h-3" />
                                    </button>
                                  </div>
                                </div>
                              );
                            })}
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}

              {/* Add Another Provider Bar */}
              {!isAddingProvider && (
                <div
                  onClick={() => setIsAddingProvider(true)}
                  className="settings-add-provider-bar"
                  role="button"
                  tabIndex={0}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      setIsAddingProvider(true);
                    }
                  }}
                >
                  <div className="settings-add-provider-btn">
                    <Plus className="w-3.5 h-3.5" />
                    <span>Add Another Provider</span>
                  </div>
                  <span className="settings-add-provider-desc">
                    Connect OpenAI, Anthropic, Gemini, local models, and more.
                  </span>
                </div>
              )}
            </div>
          )}

          {/* 4. WEB SEARCH PROVIDERS TAB */}
          {activeTab === 'search' && (
            <div className="space-y-6 animate-in fade-in duration-100">
              <div className="settings-section-header">
                <h3 className="settings-section-title">Web Search Providers</h3>
                <p className="settings-section-sub">
                  Configure external search engines and headless scrapers for research evidence grounding.
                </p>
              </div>

            <div className="space-y-4">
              {searchProviders.map((provider) => {
                const isTesting = !!isTestingMap[provider.id];
                const testError = testErrorMap[provider.id];
                const testResults = testResultsMap[provider.id];
                const isKeyVisible = !!showSearchKeys[provider.id];
                const needsApiKey = provider.provider_type === 'brave' || provider.provider_type === 'tavily';

                return (
                  <div
                    key={provider.id}
                    className="p-5 rounded-2xl border border-sthira-border bg-sthira-surface space-y-4 shadow-xs"
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div>
                        <div className="flex items-center gap-2">
                          <span className="font-bold text-sm text-sthira-text">{provider.display_name}</span>
                          {provider.is_default && (
                            <span className="px-2 py-0.5 rounded-full bg-sthira-primary text-sthira-on-primary text-[10px] font-bold">
                              Primary
                            </span>
                          )}
                          <span className="px-2 py-0.5 rounded-full bg-sthira-surface-muted text-sthira-text-muted text-[10px] font-mono">
                            {provider.provider_type}
                          </span>
                        </div>
                        <p className="text-xs text-sthira-text-muted mt-0.5">
                          {provider.provider_type === 'duckduckgo_scraper'
                            ? 'Headless zero-token HTML scraper. No API key required.'
                            : provider.provider_type === 'searxng'
                            ? 'Local metasearch engine proxy. Privacy-first, zero third-party leakage.'
                            : 'Direct cloud web search API.'}
                        </p>
                      </div>

                      <div className="flex items-center gap-2">
                        {!provider.is_default && provider.is_enabled && (
                          <button
                            type="button"
                            onClick={() => handleSetDefaultSearchProvider(provider)}
                            className="px-2.5 py-1 text-xs rounded-xl border border-sthira-border bg-sthira-surface hover:bg-sthira-surface-muted text-sthira-text transition-colors cursor-pointer"
                          >
                            Set Primary
                          </button>
                        )}
                        <button
                          type="button"
                          onClick={() => handleToggleSearchProviderEnabled(provider)}
                          className={`px-3 py-1 rounded-xl text-xs font-semibold cursor-pointer transition-colors ${
                            provider.is_enabled
                              ? 'bg-[#E3EBE3] text-[#2D5A27] dark:bg-[#1C2B1B] dark:text-[#879E78]'
                              : 'bg-sthira-surface-muted text-sthira-text-muted'
                          }`}
                        >
                          {provider.is_enabled ? 'Active' : 'Disabled'}
                        </button>
                      </div>
                    </div>

                    <div className="grid grid-cols-1 md:grid-cols-2 gap-3 pt-3 border-t border-sthira-border">
                      <div>
                        <label className="block text-[11px] font-semibold text-sthira-text-muted mb-1">
                          Endpoint URL
                        </label>
                        <input
                          type="text"
                          value={searchUrlInputs[provider.id] ?? provider.endpoint_url}
                          onChange={(e) =>
                            setSearchUrlInputs((prev) => ({
                              ...prev,
                              [provider.id]: e.target.value,
                            }))
                          }
                          className="w-full px-3 py-1.5 rounded-xl bg-sthira-surface-muted border border-sthira-border text-xs text-sthira-text focus:bg-sthira-surface focus:outline-none"
                        />
                      </div>

                      <div>
                        <label className="block text-[11px] font-semibold text-sthira-text-muted mb-1">
                          API Key {needsApiKey ? '(Required)' : '(Optional / None)'}
                        </label>
                        <div className="relative">
                          <input
                            type={isKeyVisible ? 'text' : 'password'}
                            value={
                              searchKeyInputs[provider.id] !== undefined
                                ? searchKeyInputs[provider.id]
                                : provider.api_key ?? ''
                            }
                            placeholder={needsApiKey ? 'Enter API Key...' : 'None required'}
                            onChange={(e) =>
                              setSearchKeyInputs((prev) => ({
                                ...prev,
                                [provider.id]: e.target.value,
                              }))
                            }
                            className="w-full pl-3 pr-8 py-1.5 rounded-xl bg-sthira-surface-muted border border-sthira-border text-xs text-sthira-text focus:bg-sthira-surface focus:outline-none"
                          />
                          <button
                            type="button"
                            onClick={() =>
                              setShowSearchKeys((prev) => ({
                                ...prev,
                                [provider.id]: !prev[provider.id],
                              }))
                            }
                            className="absolute right-2 top-2 text-sthira-text-muted hover:text-sthira-text cursor-pointer"
                          >
                            {isKeyVisible ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                          </button>
                        </div>
                      </div>
                    </div>

                    <div className="pt-3 border-t border-sthira-border flex flex-wrap items-center justify-between gap-3">
                      <div className="flex items-center gap-2 flex-1 max-w-md">
                        <input
                          type="text"
                          placeholder="Test query (e.g. rust async traits)..."
                          value={testQueryMap[provider.id] ?? 'rust async traits'}
                          onChange={(e) =>
                            setTestQueryMap((prev) => ({
                              ...prev,
                              [provider.id]: e.target.value,
                            }))
                          }
                          className="px-3 py-1.5 text-xs rounded-xl bg-sthira-surface border border-sthira-border text-sthira-text flex-1 focus:outline-none"
                        />
                        <button
                          type="button"
                          onClick={() => handleRunSearchTest(provider)}
                          disabled={isTesting}
                          className="settings-btn-primary shrink-0"
                        >
                          {isTesting ? (
                            <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                          ) : (
                            <Search className="w-3.5 h-3.5" />
                          )}
                          <span>Test Search</span>
                        </button>
                      </div>

                      <button
                        type="button"
                        onClick={() => handleSaveSearchProviderSettings(provider)}
                        className="settings-btn-secondary"
                      >
                        <Save className="w-3.5 h-3.5" />
                        <span>Save Settings</span>
                      </button>
                    </div>

                    {testError && (
                      <div className="p-3 rounded-xl bg-red-50 dark:bg-red-950/40 border border-red-200 dark:border-red-900 text-red-700 dark:text-red-300 text-xs flex items-center gap-2">
                        <AlertCircle className="w-3.5 h-3.5 shrink-0" />
                        <span>{testError}</span>
                      </div>
                    )}

                    {testResults && testResults.length > 0 && (
                      <div className="p-4 bg-sthira-surface-muted border border-sthira-border rounded-xl space-y-3">
                        <div className="flex items-center justify-between text-xs font-semibold text-sthira-text">
                          <span className="flex items-center gap-1.5 text-emerald-700 dark:text-emerald-400">
                            <ShieldCheck className="w-4 h-4" />
                            Returned {testResults.length} Live Results
                          </span>
                          <button
                            type="button"
                            onClick={() =>
                              setTestResultsMap((prev) => {
                                const next = { ...prev };
                                delete next[provider.id];
                                return next;
                              })
                            }
                            className="text-sthira-text-muted hover:text-sthira-text text-xs cursor-pointer"
                          >
                            Dismiss
                          </button>
                        </div>
                        <div className="space-y-2">
                          {testResults.slice(0, 3).map((item, idx) => (
                            <div
                              key={idx}
                              className="p-3 rounded-xl bg-sthira-surface border border-sthira-border text-xs space-y-1"
                            >
                              <div className="flex items-center justify-between">
                                <span className="font-semibold text-sthira-text line-clamp-1">
                                  {item.title}
                                </span>
                                {item.score !== undefined && (
                                  <span className="text-[10px] text-sthira-text-muted">
                                    score: {item.score.toFixed(2)}
                                  </span>
                                )}
                              </div>
                              <p className="text-sthira-text-muted text-[11px] line-clamp-2 leading-relaxed">
                                {item.snippet}
                              </p>
                              <a
                                href={item.url}
                                target="_blank"
                                rel="noreferrer"
                                className="text-[11px] text-blue-600 dark:text-blue-400 hover:underline flex items-center gap-1 truncate"
                              >
                                <ExternalLink className="w-2.5 h-2.5 shrink-0" />
                                <span className="truncate">{item.url}</span>
                              </a>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* 5. APPEARANCE TAB */}
        {activeTab === 'appearance' && (
          <div className="space-y-6 animate-in fade-in duration-100">
            <div className="settings-section-header">
              <h3 className="settings-section-title">Theme &amp; Visual Appearance</h3>
              <p className="settings-section-sub">
                Customize workspace color temperature, reading contrast, and system theme persistence.
              </p>
            </div>

            <div className="p-5 rounded-2xl bg-sthira-surface border border-sthira-border flex flex-col sm:flex-row sm:items-center justify-between gap-4 shadow-xs">
              <div>
                <span className="font-semibold text-sthira-text text-sm block mb-1">
                  Interface Theme
                </span>
                <p className="text-xs text-sthira-text-muted max-w-md">
                  Choose between tactile Warm Stone (Light), glare-free Deep Obsidian (Dark), or automatically follow your operating system appearance.
                </p>
              </div>
              <div className="shrink-0">
                <ThemeToggle variant="selector" />
              </div>
            </div>

            <div className="p-5 rounded-2xl bg-sthira-surface border border-sthira-border space-y-3 text-xs shadow-xs">
              <span className="font-semibold text-sthira-text">
                Aesthetic Design Invariants
              </span>
              <ul className="list-disc list-inside space-y-1.5 text-sthira-text-muted">
                <li>
                  <strong>Warm Stone (Light):</strong> Calming cream paper tone (
                  <code className="font-mono text-[11px]">#FAF8F3</code>) optimized for daytime contemplation.
                </li>
                <li>
                  <strong>Deep Obsidian (Dark):</strong> Low blue-light obsidian tone (
                  <code className="font-mono text-[11px]">#161714</code>) preserving epistemic clarity.
                </li>
                <li>
                  <strong>System:</strong> Automatically synchronizes with your OS dark mode toggle.
                </li>
              </ul>
            </div>
          </div>
        )}

        {/* 6. VAULT & STORAGE TAB */}
        {activeTab === 'vault' && (
          <div className="space-y-6 animate-in fade-in duration-100">
            <div className="settings-section-header">
              <h3 className="settings-section-title">Vault Invariants &amp; Local Storage</h3>
              <p className="settings-section-sub">
                SQLite database transactions adhere strictly to the 7-Layer Reversibility Protocol with atomic rollback guarantees.
              </p>
            </div>

            <div className="space-y-4">
              <div className="flex items-center justify-between p-4 bg-sthira-surface border border-sthira-border rounded-2xl shadow-xs">
                <div>
                  <span className="font-semibold text-sthira-text text-sm block">Load Benchmark Workspace</span>
                  <p className="text-xs text-sthira-text-muted">
                    Seed the sample research workspace and verified epistemic claims for testing.
                  </p>
                </div>
                <button
                  type="button"
                  onClick={handleLoadDemo}
                  disabled={loading}
                  className="settings-btn-secondary"
                >
                  <Sparkles className="w-3.5 h-3.5 text-sthira-primary" />
                  <span>Seed Demo</span>
                </button>
              </div>

              <div className="flex items-center justify-between p-4 bg-red-50/50 dark:bg-red-950/20 border border-red-200 dark:border-red-900/50 rounded-2xl shadow-xs">
                <div>
                  <span className="font-semibold text-red-700 dark:text-red-300 text-sm block">Clear Vault Data</span>
                  <p className="text-xs text-sthira-text-muted">
                    Wipes all local projects, notes, and epistemic claims for a pristine clean start.
                  </p>
                </div>
                <button
                  type="button"
                  onClick={handleResetVault}
                  disabled={loading}
                  className="px-4 py-2 rounded-xl bg-red-100 hover:bg-red-200 dark:bg-red-900/60 dark:hover:bg-red-900 text-red-700 dark:text-red-200 font-semibold flex items-center gap-1.5 transition-colors cursor-pointer text-xs"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                  <span>Reset Vault</span>
                </button>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>

    {/* 7. Modal Footer with Cancel & Save Changes */}
        <div className="settings-footer">
          <button
            type="button"
            onClick={onClose}
            className="settings-btn-secondary"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={onClose}
            className="settings-btn-primary"
            data-testid="btn-save-settings"
          >
            <Check className="w-3.5 h-3.5" />
            <span>Save Changes</span>
          </button>
        </div>
      </div>
    </div>
  );
};

export default ModelSettingsModal;
