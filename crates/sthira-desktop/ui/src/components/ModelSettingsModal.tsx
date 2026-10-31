import React, { useState, useEffect } from 'react';
import {
  SlidersHorizontal,
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
import { WebSearchCapabilityBadge } from './ModelSelector';

interface ModelSettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onVaultMutated: () => void;
  onCatalogChanged?: () => void;
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
}) => {
  const [providers, setProviders] = useState<ProviderEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [fetchingProviderId, setFetchingProviderId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [errorNotice, setErrorNotice] = useState<string | null>(null);
  const [showKeys, setShowKeys] = useState<Record<string, boolean>>({});
  const [keyInputs, setKeyInputs] = useState<Record<string, string>>({});
  const [urlInputs, setUrlInputs] = useState<Record<string, string>>({});
  const [activeTab, setActiveTab] = useState<'models' | 'search' | 'appearance' | 'vault'>('models');

  // Search Providers State
  const [searchProviders, setSearchProviders] = useState<SearchProviderEntry[]>([]);
  const [loadingSearchProviders, setLoadingSearchProviders] = useState(false);
  const [searchUrlInputs, setSearchUrlInputs] = useState<Record<string, string>>({});
  const [searchKeyInputs, setSearchKeyInputs] = useState<Record<string, string>>({});
  const [showSearchKeys, setShowSearchKeys] = useState<Record<string, boolean>>({});
  const [testQueryMap, setTestQueryMap] = useState<Record<string, string>>({});
  const [testResultsMap, setTestResultsMap] = useState<Record<string, WebSearchResultItem[]>>({});
  const [isTestingMap, setIsTestingMap] = useState<Record<string, boolean>>({});
  const [testErrorMap, setTestErrorMap] = useState<Record<string, string | null>>({});

  // Discovered models per connected provider: providerId -> ProviderModelEntry[]
  const [discoveredModelsMap, setDiscoveredModelsMap] = useState<Record<string, ProviderModelEntry[]>>({});
  // Selected model identifier from the discovered dropdown per connected provider: providerId -> modelIdentifier
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
      setProviders(catalog);
      const urls: Record<string, string> = {};
      catalog.forEach((p) => {
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
      setSearchProviders(list);
      const urls: Record<string, string> = {};
      const queries: Record<string, string> = {};
      list.forEach((p) => {
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

  if (!isOpen) return null;

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

  // Dynamic Model Fetching (OmniRouter Style): discover models into dropdown selection
  const handleFetchModels = async (provider: ProviderEntry) => {
    setFetchingProviderId(provider.id);
    setNotice(null);
    setErrorNotice(null);
    try {
      const endpoint = urlInputs[provider.id] || provider.base_url;
      const apiKey = keyInputs[provider.id];

      // Save provider settings first if changed
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
      setNotice(`Added all ${available.length} models to ${providerId}`);
      await fetchCatalog();
      if (onCatalogChanged) onCatalogChanged();
    } catch (err: any) {
      setErrorNotice(`Failed to add models: ${err?.message || err}`);
    }
  };

  const handleFetchModelsForNewProvider = async () => {
    const preset = PROVIDER_PRESETS.find((p) => p.id === selectedPresetId);
    const pId = selectedPresetId === 'custom' ? `custom-${Date.now()}` : selectedPresetId;
    const baseUrl = customProviderUrl.trim() || (preset ? preset.base_url : 'http://localhost:8000/v1');
    const provType = preset ? preset.provider_type : 'openai_compatible';
    const apiKey = customProviderKey.trim() || undefined;

    setIsFetchingNewProvider(true);
    setErrorNotice(null);
    setNotice(null);
    try {
      const discovered = await discoverProviderModels(pId, provType, baseUrl, apiKey);
      setDiscoveredForNewProvider(discovered);
      if (discovered.length > 0) {
        setSelectedModelForNewProvider(discovered[0].model_identifier);
        setNotice(`Discovered ${discovered.length} model(s). Select which model to connect below.`);
      } else {
        setNotice('No models returned. You can still save and add models manually.');
      }
    } catch (err: any) {
      setErrorNotice(`Fetch failed: ${err?.message || err}`);
    } finally {
      setIsFetchingNewProvider(false);
    }
  };

  const handleAddCustomModel = async (providerId: string) => {
    if (!newModelId.trim()) return;
    try {
      const modelEntry: ProviderModelEntry = {
        id: `${providerId}:${newModelId.trim()}`,
        provider_id: providerId,
        model_identifier: newModelId.trim(),
        display_name: newModelDisplayName.trim() || newModelId.trim(),
        context_window: 128000,
        supports_structured_output: true,
        is_enabled: true,
        is_default: false,
      };

      await upsertModel(modelEntry);
      setNotice(`Added model '${modelEntry.display_name}'`);
      setNewModelId('');
      setNewModelDisplayName('');
      setAddingModelForProvider(null);
      await fetchCatalog();
      if (onCatalogChanged) onCatalogChanged();
    } catch (err: any) {
      setErrorNotice(`Failed to add model: ${err?.message || err}`);
    }
  };

  const handleDeleteModel = async (modelId: string, modelName: string) => {
    if (!confirm(`Delete model '${modelName}'?`)) return;
    try {
      await deleteModel(modelId);
      setNotice(`Deleted model '${modelName}'`);
      await fetchCatalog();
      if (onCatalogChanged) onCatalogChanged();
    } catch (err: any) {
      setErrorNotice(`Failed to delete model: ${err?.message || err}`);
    }
  };

  const handleDeleteProvider = async (providerId: string, providerName: string) => {
    if (!confirm(`Remove provider '${providerName}' and all its models?`)) return;
    try {
      await deleteProvider(providerId);
      setNotice(`Removed provider '${providerName}'`);
      await fetchCatalog();
      if (onCatalogChanged) onCatalogChanged();
    } catch (err: any) {
      setErrorNotice(`Failed to delete provider: ${err?.message || err}`);
    }
  };

  const handleCreateNewProvider = async () => {
    const preset = PROVIDER_PRESETS.find((p) => p.id === selectedPresetId);
    const pId = selectedPresetId === 'custom'
      ? `custom-${Date.now()}`
      : selectedPresetId;
    const displayName = customProviderName.trim() || (preset ? preset.display_name : 'Custom Provider');
    const baseUrl = customProviderUrl.trim() || (preset ? preset.base_url : 'http://localhost:8000/v1');
    const provType = preset ? preset.provider_type : 'openai_compatible';

    try {
      const selectedModelObj = discoveredForNewProvider.find(
        (m) => m.model_identifier === selectedModelForNewProvider
      );

      const modelsToSeed: ProviderModelEntry[] = selectedModelObj
        ? [{ ...selectedModelObj, provider_id: pId, id: `${pId}:${selectedModelObj.model_identifier}` }]
        : [];

      const newEntry: ProviderEntry = {
        id: pId,
        display_name: displayName,
        provider_type: provType,
        base_url: baseUrl,
        has_api_key: customProviderKey.trim().length > 0,
        is_enabled: true,
        models: modelsToSeed,
      };

      await saveProvider(newEntry, customProviderKey.trim().length > 0 ? customProviderKey.trim() : undefined);

      for (const m of modelsToSeed) {
        await upsertModel(m);
      }

      setNotice(`Connected provider '${displayName}'! Added to connected providers list.`);
      setIsAddingProvider(false);
      setCustomProviderName('OpenRouter');
      setCustomProviderUrl('https://openrouter.ai/api/v1');
      setCustomProviderKey('');
      setDiscoveredForNewProvider([]);
      setSelectedModelForNewProvider('');

      await fetchCatalog();
      if (onCatalogChanged) onCatalogChanged();
    } catch (err: any) {
      setErrorNotice(`Failed to create provider: ${err?.message || err}`);
    }
  };

  const handleSetDefault = async (modelId: string, modelName: string) => {
    try {
      await setDefaultModel(modelId);
      setNotice(`Default model set to ${modelName}`);
      setErrorNotice(null);
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

  // Provider icon helper
  const getProviderIcon = (providerType: string, providerId: string) => {
    if (providerId === 'ollama' || providerType === 'ollama') return '🦙';
    if (providerId === 'anthropic' || providerType === 'anthropic') return '⚡';
    if (providerId === 'openrouter') return '🌐';
    if (providerId === 'groq') return '🚀';
    if (providerId === 'deepseek') return '🐋';
    return '☁️';
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/40 backdrop-blur-sm animate-in fade-in duration-150">
      <div className="w-full max-w-2xl bg-[#FFFFFF] dark:bg-[#1C1D19] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
        {/* Header */}
        <div className="px-6 py-4 border-b border-[#ECE9E0] dark:border-[#2C2E27] flex items-center justify-between bg-[#FAF8F3] dark:bg-[#161714]">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] flex items-center justify-center shadow-sm">
              <SlidersHorizontal className="w-4 h-4" />
            </div>
            <div>
              <h3 className="text-base font-bold text-[#1E201C] dark:text-[#EDEAE2]">OmniRouter Compute Fabric</h3>
              <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93]">
                Connect any model provider, paste API keys, and configure appearance
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close Settings"
            className="p-1.5 rounded-lg text-[#6E6D66] dark:text-[#9E9C93] hover:text-[#1E201C] dark:hover:text-[#EDEAE2] hover:bg-[#EFECE4] dark:hover:bg-[#252820] transition-colors cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Tab Switcher */}
        <div className="px-6 pt-3 flex items-center justify-between border-b border-[#ECE9E0] dark:border-[#2C2E27] bg-[#FFFFFF] dark:bg-[#1C1D19] text-xs font-semibold text-[#6E6D66] dark:text-[#9E9C93]">
          <div className="flex gap-4">
            <button
              type="button"
              onClick={() => setActiveTab('models')}
              className={`pb-2.5 flex items-center gap-2 border-b-2 transition-colors cursor-pointer ${
                activeTab === 'models'
                  ? 'border-[#282C25] dark:border-[#EDEAE2] text-[#1E201C] dark:text-[#EDEAE2]'
                  : 'border-transparent hover:text-[#1E201C] dark:hover:text-[#EDEAE2]'
              }`}
            >
              <Server className="w-3.5 h-3.5" />
              Connected Providers &amp; Models
            </button>
            <button
              type="button"
              onClick={() => setActiveTab('search')}
              className={`pb-2.5 flex items-center gap-2 border-b-2 transition-colors cursor-pointer ${
                activeTab === 'search'
                  ? 'border-[#282C25] dark:border-[#EDEAE2] text-[#1E201C] dark:text-[#EDEAE2]'
                  : 'border-transparent hover:text-[#1E201C] dark:hover:text-[#EDEAE2]'
              }`}
            >
              <Globe className="w-3.5 h-3.5" />
              Web Search Providers
            </button>
            <button
              type="button"
              onClick={() => setActiveTab('appearance')}
              className={`pb-2.5 flex items-center gap-2 border-b-2 transition-colors cursor-pointer ${
                activeTab === 'appearance'
                  ? 'border-[#282C25] dark:border-[#EDEAE2] text-[#1E201C] dark:text-[#EDEAE2]'
                  : 'border-transparent hover:text-[#1E201C] dark:hover:text-[#EDEAE2]'
              }`}
            >
              <Sun className="w-3.5 h-3.5" />
              Appearance
            </button>
            <button
              type="button"
              onClick={() => setActiveTab('vault')}
              className={`pb-2.5 flex items-center gap-2 border-b-2 transition-colors cursor-pointer ${
                activeTab === 'vault'
                  ? 'border-[#282C25] dark:border-[#EDEAE2] text-[#1E201C] dark:text-[#EDEAE2]'
                  : 'border-transparent hover:text-[#1E201C] dark:hover:text-[#EDEAE2]'
              }`}
            >
              <Layers className="w-3.5 h-3.5" />
              Vault &amp; Storage
            </button>
          </div>

          {activeTab === 'models' && !isAddingProvider && (
            <button
              type="button"
              onClick={() => setIsAddingProvider(true)}
              className="pb-2.5 text-xs text-[#282C25] dark:text-[#EDEAE2] font-semibold flex items-center gap-1 hover:opacity-80 transition-opacity cursor-pointer"
            >
              <Plus className="w-3.5 h-3.5" />
              <span>Connect Provider</span>
            </button>
          )}
        </div>

        {/* Content */}
        <div className="p-6 space-y-6 overflow-y-auto flex-1 text-xs">
          {/* Notifications */}
          {notice && (
            <div className="p-3 bg-[#E3EBE3] border border-[#CCDCCC] text-[#2D5A27] rounded-xl flex items-center gap-2 font-medium animate-in fade-in">
              <Check className="w-4 h-4 shrink-0" />
              <span>{notice}</span>
            </div>
          )}

          {errorNotice && (
            <div className="p-3 bg-[#FDF2F2] border border-[#F8D7DA] text-[#9E2A2B] rounded-xl flex items-center gap-2 font-medium animate-in fade-in">
              <AlertCircle className="w-4 h-4 shrink-0" />
              <span>{errorNotice}</span>
            </div>
          )}

          {activeTab === 'models' && (
            <div className="space-y-6">
              {/* Add New Provider Card */}
              {isAddingProvider && (
                <div className="border-2 border-[#282C25] rounded-xl p-4 bg-[#FAF8F3] space-y-3.5 shadow-sm animate-in fade-in">
                  <div className="flex items-center justify-between">
                    <span className="font-bold text-sm text-[#1E201C] flex items-center gap-2">
                      <Globe className="w-4 h-4" />
                      Connect Compute Provider (OmniRouter)
                    </span>
                    <button
                      type="button"
                      onClick={() => setIsAddingProvider(false)}
                      className="p-1 rounded-lg text-[#6E6D66] hover:text-[#1E201C] hover:bg-[#EFECE4]"
                    >
                      <X className="w-4 h-4" />
                    </button>
                  </div>

                  <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                    <div>
                      <label className="block text-[11px] font-semibold text-[#6E6D66] mb-1">
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
                        className="w-full px-3 py-1.5 bg-[#FFFFFF] border border-[#E8E5DC] rounded-lg text-xs text-[#1E201C] focus:outline-none focus:border-[#282C25]"
                      >
                        {PROVIDER_PRESETS.map((p) => (
                          <option key={p.id} value={p.id}>
                            {p.display_name}
                          </option>
                        ))}
                      </select>
                    </div>

                    <div>
                      <label className="block text-[11px] font-semibold text-[#6E6D66] mb-1">
                        Display Name
                      </label>
                      <input
                        type="text"
                        placeholder="e.g. OpenRouter / Groq / LM Studio"
                        value={customProviderName}
                        onChange={(e) => setCustomProviderName(e.target.value)}
                        className="w-full px-3 py-1.5 bg-[#FFFFFF] border border-[#E8E5DC] rounded-lg text-xs text-[#1E201C] focus:outline-none focus:border-[#282C25]"
                      />
                    </div>

                    <div className="md:col-span-2">
                      <label className="block text-[11px] font-semibold text-[#6E6D66] mb-1">
                        Base Endpoint URL
                      </label>
                      <input
                        type="text"
                        placeholder="https://openrouter.ai/api/v1"
                        value={customProviderUrl}
                        onChange={(e) => setCustomProviderUrl(e.target.value)}
                        className="w-full px-3 py-1.5 bg-[#FFFFFF] border border-[#E8E5DC] rounded-lg text-xs font-mono text-[#1E201C] focus:outline-none focus:border-[#282C25]"
                      />
                    </div>

                    <div className="md:col-span-2">
                      <div className="flex items-center justify-between mb-1">
                        <label className="block text-[11px] font-semibold text-[#6E6D66]">
                          API Key (Stored locally in encrypted SQLite)
                        </label>
                        <button
                          type="button"
                          onClick={handleFetchModelsForNewProvider}
                          disabled={isFetchingNewProvider}
                          className="text-[11px] text-[#282C25] font-semibold underline hover:text-black flex items-center gap-1 cursor-pointer disabled:opacity-40"
                        >
                          <RefreshCw className={`w-3 h-3 ${isFetchingNewProvider ? 'animate-spin' : ''}`} />
                          <span>{isFetchingNewProvider ? 'Fetching...' : 'Fetch Models via API'}</span>
                        </button>
                      </div>
                      <input
                        type="password"
                        placeholder="Paste your API key here..."
                        value={customProviderKey}
                        onChange={(e) => setCustomProviderKey(e.target.value)}
                        className="w-full px-3 py-1.5 bg-[#FFFFFF] border border-[#E8E5DC] rounded-lg text-xs font-mono text-[#1E201C] focus:outline-none focus:border-[#282C25]"
                      />
                    </div>

                    {/* Discovered Models Dropdown for New Provider */}
                    {discoveredForNewProvider.length > 0 && (
                      <div className="md:col-span-2 p-3 bg-white border border-[#E8E5DC] rounded-xl space-y-1.5 animate-in fade-in">
                        <label className="block text-[11px] font-bold text-[#1E201C]">
                          Select Primary Model ({discoveredForNewProvider.length} models retrieved):
                        </label>
                        <select
                          value={selectedModelForNewProvider}
                          onChange={(e) => setSelectedModelForNewProvider(e.target.value)}
                          className="w-full px-3 py-1.5 bg-[#FAF8F3] border border-[#E8E5DC] rounded-lg text-xs font-mono text-[#1E201C] focus:outline-none focus:border-[#282C25]"
                        >
                          {discoveredForNewProvider.map((m) => (
                            <option key={m.model_identifier} value={m.model_identifier}>
                              {m.display_name} ({m.model_identifier}) - {(m.context_window / 1024).toFixed(0)}k ctx
                            </option>
                          ))}
                        </select>
                      </div>
                    )}
                  </div>

                  <div className="flex justify-end gap-2 pt-2">
                    <button
                      type="button"
                      onClick={() => setIsAddingProvider(false)}
                      className="px-3 py-1.5 rounded-lg border border-[#E8E5DC] bg-white text-[#6E6D66] hover:bg-[#EFECE4]"
                    >
                      Cancel
                    </button>
                    <button
                      type="button"
                      onClick={handleCreateNewProvider}
                      className="px-3.5 py-1.5 rounded-lg bg-[#282C25] text-white font-medium hover:bg-[#3D4238] flex items-center gap-1.5 cursor-pointer"
                    >
                      <Check className="w-3.5 h-3.5" />
                      <span>Connect Provider</span>
                    </button>
                  </div>
                </div>
              )}

              {/* Provider Cards */}
              {providers.map((provider) => {
                const isOllama = provider.provider_type === 'ollama';
                const hasKey = provider.has_api_key;
                const isKeyVisible = !!showKeys[provider.id];
                const isFetching = fetchingProviderId === provider.id;

                return (
                  <div
                    key={provider.id}
                    className="border border-[#E8E5DC] rounded-xl p-4 bg-[#FAF8F3] space-y-3.5 transition-all"
                  >
                    {/* Provider Header */}
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2.5">
                        <span className="text-lg">
                          {getProviderIcon(provider.provider_type, provider.id)}
                        </span>
                        <div>
                          <div className="flex items-center gap-2">
                            <h4 className="font-bold text-sm text-[#1E201C]">
                              {provider.display_name}
                            </h4>
                            <span className="px-1.5 py-0.5 rounded bg-[#EFECE4] text-[#6E6D66] text-[10px] font-mono">
                              {provider.provider_type}
                            </span>
                          </div>
                          <span className="text-[10px] text-[#6E6D66]">
                            {provider.models.length} model(s) registered
                          </span>
                        </div>
                      </div>

                      {/* OmniRouter Dynamic Model Fetch Button */}
                      <div className="flex items-center gap-2">
                        <button
                          type="button"
                          onClick={() => handleFetchModels(provider)}
                          disabled={isFetching}
                          className="px-3 py-1.5 rounded-lg bg-[#282C25] text-white hover:bg-[#3D4238] font-semibold flex items-center gap-1.5 transition-colors cursor-pointer disabled:opacity-50 shadow-xs"
                          title="Connect to provider and fetch available models dynamically"
                        >
                          <RefreshCw className={`w-3.5 h-3.5 ${isFetching ? 'animate-spin' : ''}`} />
                          <span>{isFetching ? 'Fetching Models...' : 'Fetch Models'}</span>
                        </button>

                        {provider.id !== 'ollama' && (
                          <button
                            type="button"
                            onClick={() => handleDeleteProvider(provider.id, provider.display_name)}
                            className="p-1.5 rounded-lg text-[#8C8A82] hover:text-[#9E2A2B] hover:bg-[#F8EAEA] transition-colors"
                            title="Delete provider"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                          </button>
                        )}
                      </div>
                    </div>

                    {/* Endpoint & Key Configuration */}
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-3 pt-1">
                      <div>
                        <label className="block text-[11px] font-semibold text-[#6E6D66] mb-1">
                          Base Endpoint URL
                        </label>
                        <input
                          type="text"
                          value={urlInputs[provider.id] ?? provider.base_url}
                          onChange={(e) =>
                            setUrlInputs((prev) => ({ ...prev, [provider.id]: e.target.value }))
                          }
                          className="w-full px-3 py-1.5 bg-[#FFFFFF] border border-[#E8E5DC] rounded-lg text-xs font-mono text-[#1E201C] focus:outline-none focus:border-[#282C25]"
                        />
                      </div>

                      {!isOllama && (
                        <div>
                          <div className="flex items-center justify-between mb-1">
                            <label className="block text-[11px] font-semibold text-[#6E6D66]">
                              API Key {hasKey && <span className="text-emerald-700 font-normal">(Configured)</span>}
                            </label>
                            <button
                              type="button"
                              onClick={() => handleToggleKeyVisibility(provider.id)}
                              className="text-[#6E6D66] hover:text-[#1E201C] cursor-pointer"
                              title="Toggle Visibility"
                            >
                              {isKeyVisible ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                            </button>
                          </div>
                          <div className="relative">
                            <input
                              type={isKeyVisible ? 'text' : 'password'}
                              placeholder={hasKey ? '••••••••••••••••' : 'Enter API Key...'}
                              value={keyInputs[provider.id] ?? ''}
                              onChange={(e) =>
                                setKeyInputs((prev) => ({ ...prev, [provider.id]: e.target.value }))
                              }
                              className="w-full px-3 py-1.5 bg-[#FFFFFF] border border-[#E8E5DC] rounded-lg text-xs font-mono text-[#1E201C] focus:outline-none focus:border-[#282C25]"
                            />
                          </div>
                        </div>
                      )}
                    </div>

                    {/* Discovered Models Dropdown from Live Fetch */}
                    {discoveredModelsMap[provider.id] && discoveredModelsMap[provider.id].length > 0 && (
                      <div className="p-3 bg-[#FFFFFF] border-2 border-[#282C25] rounded-xl space-y-2 animate-in fade-in shadow-xs">
                        <div className="flex items-center justify-between">
                          <span className="text-[11px] font-bold text-[#1E201C] flex items-center gap-1.5">
                            <Sparkles className="w-3.5 h-3.5 text-[#282C25]" />
                            Select Model from {provider.display_name} ({discoveredModelsMap[provider.id].length} fetched)
                          </span>
                          <button
                            type="button"
                            onClick={() => handleAddAllDiscoveredModels(provider.id)}
                            className="text-[10px] text-[#282C25] font-semibold underline hover:text-black cursor-pointer"
                          >
                            Add All Models ({discoveredModelsMap[provider.id].length}) →
                          </button>
                        </div>

                        <div className="flex items-center gap-2">
                          <select
                            value={selectedDiscoveredModelMap[provider.id] || ''}
                            onChange={(e) =>
                              setSelectedDiscoveredModelMap((prev) => ({
                                ...prev,
                                [provider.id]: e.target.value,
                              }))
                            }
                            className="flex-1 px-3 py-1.5 bg-[#FAF8F3] border border-[#E8E5DC] rounded-lg text-xs font-mono text-[#1E201C] focus:outline-none focus:border-[#282C25]"
                          >
                            {discoveredModelsMap[provider.id].map((m) => (
                              <option key={m.model_identifier} value={m.model_identifier}>
                                {m.display_name} ({m.model_identifier}) - {(m.context_window / 1024).toFixed(0)}k ctx
                              </option>
                            ))}
                          </select>

                          <button
                            type="button"
                            onClick={() => handleAddSelectedDiscoveredModel(provider.id)}
                            className="px-3.5 py-1.5 rounded-lg bg-[#282C25] text-white text-xs font-medium hover:bg-[#3D4238] cursor-pointer shrink-0"
                          >
                            + Add Model
                          </button>
                        </div>
                      </div>
                    )}

                    {/* Action Bar */}
                    <div className="flex items-center justify-between pt-1">
                      <button
                        type="button"
                        onClick={() =>
                          setAddingModelForProvider(
                            addingModelForProvider === provider.id ? null : provider.id
                          )
                        }
                        className="text-[11px] text-[#282C25] font-semibold hover:underline flex items-center gap-1 cursor-pointer"
                      >
                        <Plus className="w-3 h-3" />
                        <span>Add Custom Model</span>
                      </button>

                      <button
                        type="button"
                        onClick={() => handleSaveProviderSettings(provider)}
                        className="px-3 py-1 rounded-lg bg-[#EFECE4] hover:bg-[#E5E1D6] text-[#1E201C] font-medium flex items-center gap-1.5 transition-colors cursor-pointer"
                      >
                        <Save className="w-3 h-3" />
                        <span>Save Endpoint / Key</span>
                      </button>
                    </div>

                    {/* Inline Add Model Input Form */}
                    {addingModelForProvider === provider.id && (
                      <div className="p-3 bg-[#FFFFFF] border border-[#E8E5DC] rounded-xl space-y-2 animate-in fade-in">
                        <div className="text-[11px] font-bold text-[#1E201C]">
                          Add Model to {provider.display_name}
                        </div>
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                          <input
                            type="text"
                            placeholder="Model identifier (e.g. meta-llama/llama-3.3-70b-instruct)"
                            value={newModelId}
                            onChange={(e) => setNewModelId(e.target.value)}
                            className="px-2.5 py-1 bg-[#FAF8F3] border border-[#E8E5DC] rounded-lg text-xs font-mono"
                          />
                          <input
                            type="text"
                            placeholder="Display name (optional)"
                            value={newModelDisplayName}
                            onChange={(e) => setNewModelDisplayName(e.target.value)}
                            className="px-2.5 py-1 bg-[#FAF8F3] border border-[#E8E5DC] rounded-lg text-xs"
                          />
                        </div>
                        <div className="flex justify-end gap-2 pt-1">
                          <button
                            type="button"
                            onClick={() => setAddingModelForProvider(null)}
                            className="px-2.5 py-1 text-xs text-[#6E6D66]"
                          >
                            Cancel
                          </button>
                          <button
                            type="button"
                            onClick={() => handleAddCustomModel(provider.id)}
                            className="px-3 py-1 rounded-lg bg-[#282C25] text-white text-xs font-medium"
                          >
                            Save Model
                          </button>
                        </div>
                      </div>
                    )}

                    {/* Model List */}
                    <div className="pt-2 border-t border-[#ECE9E0] space-y-2">
                      <div className="flex items-center justify-between text-[11px] font-bold text-[#3A3B36] uppercase tracking-wider">
                        <span>Available Models ({provider.models.length})</span>
                        <span className="text-[10px] text-[#8C8A82] font-normal hidden sm:inline">
                          Model • Web Search Support • Status
                        </span>
                      </div>

                      {provider.models.length === 0 ? (
                        <div className="p-4 bg-[#FFFFFF] border border-[#ECE9E0] rounded-lg text-[#6E6D66] text-center space-y-1">
                          <p className="italic">No models fetched yet for {provider.display_name}.</p>
                          <p className="text-[11px] text-[#8C8A82]">
                            Enter your API key above and click <strong>Fetch Models</strong> to dynamically load models from this provider.
                          </p>
                        </div>
                      ) : (
                        <div className="space-y-1.5 max-h-60 overflow-y-auto pr-1">
                          {provider.models.map((m) => (
                            <div
                              key={m.id}
                              className={`p-2.5 rounded-lg border transition-all flex items-center justify-between ${
                                m.is_default
                                  ? 'bg-[#FFFFFF] border-[#282C25] shadow-xs'
                                  : 'bg-[#FFFFFF] border-[#E8E5DC]'
                              }`}
                            >
                              <div className="flex items-center gap-2.5">
                                <button
                                  type="button"
                                  onClick={() => handleSetDefault(m.id, m.display_name)}
                                  className={`w-4 h-4 rounded-full border flex items-center justify-center cursor-pointer transition-colors ${
                                    m.is_default
                                      ? 'border-[#282C25] bg-[#282C25] text-white'
                                      : 'border-[#CCC8BE] hover:border-[#282C25]'
                                  }`}
                                  title={m.is_default ? 'Current Default Model' : 'Set as Default Model'}
                                >
                                  {m.is_default && <div className="w-1.5 h-1.5 rounded-full bg-white" />}
                                </button>

                                <div className="truncate">
                                  <div className="flex items-center gap-2">
                                    <span className="font-semibold text-[#1E201C] truncate">{m.display_name}</span>
                                    {m.is_default && (
                                      <span className="px-1.5 py-0.5 rounded bg-[#282C25] text-white text-[9px] font-bold uppercase shrink-0">
                                        Default
                                      </span>
                                    )}
                                  </div>
                                  <div className="flex items-center gap-2 text-[10px] text-[#6E6D66] font-mono mt-0.5 truncate">
                                    <span className="truncate">{m.model_identifier}</span>
                                    <span>•</span>
                                    <span>{(m.context_window / 1024).toFixed(0)}k ctx</span>
                                  </div>
                                </div>
                              </div>

                              <div className="flex items-center gap-2.5 shrink-0">
                                {/* Web Search Support Column & Dropdown */}
                                <div
                                  className="flex items-center gap-1.5"
                                  title="Web search execution capability for this model"
                                >
                                  <span className="text-[10px] text-[#6E6D66] font-medium hidden md:inline">
                                    Search:
                                  </span>
                                  <select
                                    value={m.web_search_capability || 'engine_augmented_only'}
                                    onChange={(e) =>
                                      handleUpdateCapability(
                                        m.id,
                                        e.target.value as WebSearchCapability
                                      )
                                    }
                                    className="text-[10px] py-1 px-1.5 rounded-md border border-[#E8E5DC] bg-[#FAF8F5] text-[#1E201C] cursor-pointer outline-none font-medium"
                                    data-testid={`select-capability-${m.id}`}
                                    aria-label={`Web Search Support for ${m.display_name}`}
                                  >
                                    <option value="agentic_tool_call">Tool Calling</option>
                                    <option value="native">Native Grounding</option>
                                    <option value="engine_augmented_only">Web Scrape Injected</option>
                                    <option value="unsupported">Offline Only</option>
                                  </select>
                                  <WebSearchCapabilityBadge capability={m.web_search_capability} />
                                </div>

                                <button
                                  type="button"
                                  onClick={() => handleToggleModelEnabled(m.id, m.is_enabled)}
                                  className={`px-2 py-0.5 rounded text-[10px] font-semibold cursor-pointer transition-colors ${
                                    m.is_enabled
                                      ? 'bg-[#E3EBE3] text-[#2D5A27] hover:bg-[#D5E2D5]'
                                      : 'bg-[#F0EEE8] text-[#8C8A82] hover:bg-[#E5E1D6]'
                                  }`}
                                >
                                  {m.is_enabled ? 'Enabled' : 'Disabled'}
                                </button>
                                <button
                                  type="button"
                                  onClick={() => handleDeleteModel(m.id, m.display_name)}
                                  className="text-[#8C8A82] hover:text-[#9E2A2B] p-1 rounded"
                                  title="Delete model"
                                >
                                  <Trash2 className="w-3 h-3" />
                                </button>
                              </div>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}

          {activeTab === 'search' && (
            <div className="space-y-6">
              <div className="p-3 bg-[#FAF8F3] border border-[#E8E5DC] rounded-xl flex items-start gap-2.5">
                <Globe className="w-4 h-4 shrink-0 text-[#282C25] mt-0.5" />
                <div className="space-y-1">
                  <div className="font-semibold text-[#1E201C] text-xs">
                    Pluggable Web Research &amp; Fact Gathering Providers
                  </div>
                  <p className="text-[11px] text-[#6E6D66] leading-relaxed">
                    Configure local and cloud search engines for multi-query evidence gathering.
                    Queries scrape live web pages into Markdown, verify citations with verbatim quotes, and protect your workspace with zero unsolicited writes.
                  </p>
                </div>
              </div>

              {loadingSearchProviders && searchProviders.length === 0 ? (
                <div className="py-12 flex flex-col items-center justify-center text-center space-y-2">
                  <RefreshCw className="w-5 h-5 animate-spin text-[#8C8A82]" />
                  <span className="text-xs text-[#8C8A82]">Loading search providers...</span>
                </div>
              ) : (
                <div className="space-y-4">
                  {searchProviders.map((provider) => {
                    const isTesting = !!isTestingMap[provider.id];
                    const testResults = testResultsMap[provider.id];
                    const testError = testErrorMap[provider.id];
                    const isKeyVisible = !!showSearchKeys[provider.id];
                    const needsApiKey = provider.provider_type === 'brave' || provider.provider_type === 'tavily';

                    return (
                      <div
                        key={provider.id}
                        className={`border rounded-xl p-4 transition-all ${
                          provider.is_default
                            ? 'border-[#282C25] bg-[#FFFFFF] shadow-xs'
                            : 'border-[#E8E5DC] bg-[#FAF8F3]'
                        }`}
                      >
                        {/* Header */}
                        <div className="flex items-center justify-between pb-3 border-b border-[#ECE9E0]">
                          <div className="flex items-center gap-2.5">
                            <div className="p-1.5 rounded-lg bg-[#EFECE4] text-[#1E201C]">
                              <Globe className="w-4 h-4" />
                            </div>
                            <div>
                              <div className="flex items-center gap-2">
                                <span className="font-bold text-[#1E201C]">{provider.display_name}</span>
                                {provider.is_default && (
                                  <span className="px-2 py-0.5 rounded-full text-[10px] font-bold bg-[#282C25] text-white">
                                    Primary Provider
                                  </span>
                                )}
                                <span className="px-2 py-0.5 rounded-full text-[10px] font-medium bg-[#EFECE4] text-[#6E6D66]">
                                  {provider.provider_type === 'duckduckgo_scraper'
                                    ? 'Headless HTML (No Key)'
                                    : provider.provider_type === 'searxng'
                                    ? 'SearXNG JSON API'
                                    : provider.provider_type === 'brave'
                                    ? 'Brave REST API'
                                    : 'Tavily API'}
                                </span>
                              </div>
                              <span className="text-[11px] text-[#6E6D66]">
                                {provider.endpoint_url}
                              </span>
                            </div>
                          </div>

                          <div className="flex items-center gap-2">
                            <button
                              type="button"
                              onClick={() => handleToggleSearchProviderEnabled(provider)}
                              className={`px-2.5 py-1 text-[11px] font-semibold rounded-lg border transition-colors cursor-pointer ${
                                provider.is_enabled
                                  ? 'bg-[#E3EBE3] text-[#2D5A27] border-[#CCDCCC]'
                                  : 'bg-[#F5F3EC] text-[#8C8A82] border-[#E8E5DC]'
                              }`}
                            >
                              {provider.is_enabled ? 'Enabled' : 'Disabled'}
                            </button>

                            {!provider.is_default && (
                              <button
                                type="button"
                                onClick={() => handleSetDefaultSearchProvider(provider)}
                                className="px-2.5 py-1 text-[11px] font-semibold rounded-lg bg-[#EFECE4] hover:bg-[#E5E1D6] text-[#1E201C] transition-colors cursor-pointer"
                              >
                                Make Primary
                              </button>
                            )}
                          </div>
                        </div>

                        {/* Configuration Fields */}
                        <div className="pt-3 grid grid-cols-1 md:grid-cols-2 gap-3 text-xs">
                          <div>
                            <label className="block text-[11px] font-semibold text-[#6E6D66] mb-1">
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
                              className="w-full px-2.5 py-1.5 rounded-lg bg-[#F5F3EC] border border-[#E8E5DC] text-[#1E201C] focus:bg-white focus:outline-none"
                            />
                          </div>

                          <div>
                            <label className="block text-[11px] font-semibold text-[#6E6D66] mb-1">
                              API Key {needsApiKey ? '(Required)' : '(Optional / None)'}
                            </label>
                            <div className="relative">
                              <input
                                type={isKeyVisible ? 'text' : 'password'}
                                value={
                                  searchKeyInputs[provider.id] !== undefined
                                    ? searchKeyInputs[provider.id]
                                    : (provider.api_key ?? '')
                                }
                                placeholder={needsApiKey ? 'Enter API Key...' : 'None required'}
                                onChange={(e) =>
                                  setSearchKeyInputs((prev) => ({
                                    ...prev,
                                    [provider.id]: e.target.value,
                                  }))
                                }
                                className="w-full pl-2.5 pr-8 py-1.5 rounded-lg bg-[#F5F3EC] border border-[#E8E5DC] text-[#1E201C] focus:bg-white focus:outline-none"
                              />
                              <button
                                type="button"
                                onClick={() =>
                                  setShowSearchKeys((prev) => ({
                                    ...prev,
                                    [provider.id]: !prev[provider.id],
                                  }))
                                }
                                className="absolute right-2 top-2 text-[#8C8A82] hover:text-[#1E201C]"
                              >
                                {isKeyVisible ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                              </button>
                            </div>
                          </div>
                        </div>

                        {/* Save & Test bar */}
                        <div className="mt-3 pt-3 border-t border-[#ECE9E0] flex flex-wrap items-center justify-between gap-2">
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
                              className="px-2.5 py-1 text-xs rounded-lg bg-white border border-[#E8E5DC] text-[#1E201C] flex-1 focus:outline-none"
                            />
                            <button
                              type="button"
                              onClick={() => handleRunSearchTest(provider)}
                              disabled={isTesting}
                              className="px-3 py-1 text-xs rounded-lg bg-[#282C25] hover:opacity-90 disabled:opacity-50 text-white font-medium flex items-center gap-1.5 transition-opacity cursor-pointer shrink-0"
                            >
                              {isTesting ? (
                                <RefreshCw className="w-3 h-3 animate-spin" />
                              ) : (
                                <Search className="w-3 h-3" />
                              )}
                              <span>Test Search</span>
                            </button>
                          </div>

                          <button
                            type="button"
                            onClick={() => handleSaveSearchProviderSettings(provider)}
                            className="px-3 py-1 text-xs rounded-lg bg-[#EFECE4] hover:bg-[#E5E1D6] text-[#1E201C] font-semibold flex items-center gap-1.5 transition-colors cursor-pointer"
                          >
                            <Save className="w-3 h-3" />
                            <span>Save Settings</span>
                          </button>
                        </div>

                        {/* Test Results Display */}
                        {testError && (
                          <div className="mt-3 p-2.5 rounded-lg bg-[#FDF2F2] border border-[#F8D7DA] text-[#9E2A2B] text-[11px] flex items-center gap-1.5">
                            <AlertCircle className="w-3.5 h-3.5 shrink-0" />
                            <span>{testError}</span>
                          </div>
                        )}

                        {testResults && testResults.length > 0 && (
                          <div className="mt-3 p-3 bg-white border border-[#E8E5DC] rounded-xl space-y-2">
                            <div className="flex items-center justify-between text-[11px] font-semibold text-[#6E6D66]">
                              <span className="flex items-center gap-1 text-[#2D5A27]">
                                <ShieldCheck className="w-3.5 h-3.5 text-emerald-600" />
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
                                className="hover:text-[#1E201C]"
                              >
                                Dismiss
                              </button>
                            </div>
                            <div className="space-y-2">
                              {testResults.slice(0, 3).map((item, idx) => (
                                <div
                                  key={idx}
                                  className="p-2 rounded-lg bg-[#FAF8F3] border border-[#ECE9E0] text-[11px] space-y-1"
                                >
                                  <div className="flex items-center justify-between">
                                    <span className="font-semibold text-[#1E201C] line-clamp-1">
                                      {item.title}
                                    </span>
                                    {item.score !== undefined && (
                                      <span className="text-[10px] text-[#8C8A82]">
                                        score: {item.score.toFixed(2)}
                                      </span>
                                    )}
                                  </div>
                                  <p className="text-[#6E6D66] line-clamp-2 leading-relaxed">
                                    {item.snippet}
                                  </p>
                                  <a
                                    href={item.url}
                                    target="_blank"
                                    rel="noreferrer"
                                    className="text-[10px] text-blue-600 hover:underline flex items-center gap-1 truncate"
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
              )}
            </div>
          )}

          {/* APPEARANCE TAB */}
          {activeTab === 'appearance' && (
            <div className="space-y-6 animate-in fade-in">
              <div className="flex items-center gap-2 pb-2 border-b border-[#ECE9E0] dark:border-[#2C2E27]">
                <Sun className="w-4 h-4 text-[#282C25] dark:text-[#EDEAE2]" />
                <h4 className="font-bold text-sm text-[#1E201C] dark:text-[#EDEAE2]">
                  Theme &amp; Visual Appearance
                </h4>
              </div>

              <div className="p-4 rounded-xl bg-[#FAF8F3] dark:bg-[#161714] border border-[#E8E5DC] dark:border-[#2C2E27] flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                <div>
                  <span className="font-semibold text-[#1E201C] dark:text-[#EDEAE2] text-sm block mb-0.5">
                    Interface Theme
                  </span>
                  <p className="text-xs text-[#6E6D66] dark:text-[#9E9C93] max-w-md">
                    Choose between tactile Warm Stone (Light), glare-free Deep Obsidian Stone (Dark), or automatically match your system color scheme.
                  </p>
                </div>
                <div className="shrink-0">
                  <ThemeToggle variant="selector" />
                </div>
              </div>

              <div className="p-4 rounded-xl bg-[#FAF8F3] dark:bg-[#161714] border border-[#E8E5DC] dark:border-[#2C2E27] space-y-2 text-xs">
                <span className="font-semibold text-[#1E201C] dark:text-[#EDEAE2]">
                  Aesthetic Design Invariants
                </span>
                <ul className="list-disc list-inside space-y-1 text-[#6E6D66] dark:text-[#9E9C93]">
                  <li><strong>Warm Stone (Light):</strong> Calming cream paper tone (<code className="font-mono text-[11px]">#FAF8F3</code>) optimized for daytime reading.</li>
                  <li><strong>Deep Obsidian (Dark):</strong> Low blue-light obsidian tone (<code className="font-mono text-[11px]">#121310</code>) preserving epistemic badges and matrix clarity.</li>
                  <li><strong>System:</strong> Reactively updates theme when your OS toggles light or dark mode.</li>
                </ul>
              </div>
            </div>
          )}

          {/* VAULT STORAGE TAB */}
          {activeTab === 'vault' && (
            <div className="space-y-4">
              <h4 className="font-bold text-sm text-[#1E201C] dark:text-[#EDEAE2] flex items-center gap-2">
                <Layers className="w-4 h-4 text-[#282C25] dark:text-[#EDEAE2]" />
                Vault Invariants &amp; Local Storage
              </h4>
              <p className="text-[#6E6D66] dark:text-[#9E9C93] leading-relaxed">
                All SQLite database writes adhere strictly to the 7-Layer Reversibility Protocol.
                Every operation records reversible change sets with exact deterministic rollback inverses.
              </p>

              <div className="pt-4 border-t border-[#ECE9E0] dark:border-[#2C2E27] flex flex-col gap-3">
                <div className="flex items-center justify-between p-3.5 bg-[#FAF8F3] dark:bg-[#161714] border border-[#E8E5DC] dark:border-[#2C2E27] rounded-xl">
                  <div>
                    <span className="font-semibold text-[#1E201C] dark:text-[#EDEAE2]">Load Benchmark Workspace</span>
                    <p className="text-[11px] text-[#6E6D66] dark:text-[#9E9C93]">Seed the sample research workspace and verified epistemic claims</p>
                  </div>
                  <button
                    type="button"
                    onClick={handleLoadDemo}
                    disabled={loading}
                    className="px-3 py-1.5 rounded-xl bg-[#EFECE4] dark:bg-[#22241F] hover:bg-[#E5E1D6] dark:hover:bg-[#2C2E27] text-[#1E201C] dark:text-[#EDEAE2] font-semibold flex items-center gap-1.5 transition-colors cursor-pointer"
                  >
                    <Sparkles className="w-3.5 h-3.5" />
                    <span>Seed Demo</span>
                  </button>
                </div>

                <div className="flex items-center justify-between p-3.5 bg-[#FFF8F8] dark:bg-[#2A1616] border border-[#F0D5D5] dark:border-[#4A2020] rounded-xl">
                  <div>
                    <span className="font-semibold text-[#9E2A2B] dark:text-[#F87171]">Clear Vault Data</span>
                    <p className="text-[11px] text-[#6E6D66] dark:text-[#9E9C93]">Wipes all projects, notes, and epistemic claims for a pristine clean start</p>
                  </div>
                  <button
                    type="button"
                    onClick={handleResetVault}
                    disabled={loading}
                    className="px-3 py-1.5 rounded-xl bg-[#F8EAEA] dark:bg-[#3D1A1A] hover:bg-[#F0D5D5] dark:hover:bg-[#4D2020] text-[#9E2A2B] dark:text-[#F87171] font-semibold flex items-center gap-1.5 transition-colors cursor-pointer"
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                    <span>Reset Vault</span>
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="px-6 py-3 border-t border-[#ECE9E0] dark:border-[#2C2E27] bg-[#FAF8F3] dark:bg-[#161714] flex items-center justify-between text-xs text-[#6E6D66] dark:text-[#9E9C93]">
          <span>OmniRouter Compute Fabric • Dynamic Fetch &amp; Connect</span>
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-1.5 rounded-xl bg-[#282C25] dark:bg-[#EDEAE2] text-white dark:text-[#121310] font-medium hover:bg-[#3D4238] dark:hover:bg-[#D4D1C7] transition-colors cursor-pointer"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
};
