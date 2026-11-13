import React, { useState, useEffect, useRef } from 'react';
import {
  X,
  Server,
  RefreshCw,
  Check,
  Sparkles,
  Layers,
  Eye,
  EyeOff,
  Search,
  Save,
  Trash2,
  Globe,
  AlertCircle,
  ExternalLink,
  ShieldCheck,
  Sun,
  Sprout,
  ArrowLeft,
} from 'lucide-react';
import { AIModelsSettings } from './AIModelsSettings';
import { WorkflowSettingsPanel } from './WorkflowSettingsPanel';
import { ThemeToggle } from './ThemeToggle';
import {
  getProviderCatalog,
  clearVaultData,
  seedDemoWorkspace,
  getSearchProviders,
  saveSearchProvider,
  testSearchProvider,
} from '../api/ipc';
import type {
  ProviderEntry,
  SearchProviderEntry,
  WebSearchResultItem,
} from '../types/sthira';

export type SettingsSection = 'models' | 'workflows' | 'search' | 'appearance' | 'vault';

interface ModelSettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  onVaultMutated: () => void;
  onCatalogChanged?: () => void;
  initialTab?: SettingsSection;
}

export const ModelSettingsModal: React.FC<ModelSettingsModalProps> = ({
  isOpen,
  onClose,
  onVaultMutated,
  onCatalogChanged,
  initialTab,
}) => {
  const dialogRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  const mountedRef = useRef(true);
  const noticeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const errorTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      if (noticeTimerRef.current) { clearTimeout(noticeTimerRef.current); noticeTimerRef.current = null; }
      if (errorTimerRef.current) { clearTimeout(errorTimerRef.current); errorTimerRef.current = null; }
    };
  }, []);
  const showTimedNotice = (text: string, duration = 3000) => {
    if (!mountedRef.current) return;
    setNotice(text);
    if (noticeTimerRef.current) clearTimeout(noticeTimerRef.current);
    noticeTimerRef.current = setTimeout(() => {
      noticeTimerRef.current = null;
      if (mountedRef.current) setNotice(null);
    }, duration);
  };
  const showTimedErrorNotice = (text: string, duration = 4000) => {
    if (!mountedRef.current) return;
    setErrorNotice(text);
    if (errorTimerRef.current) clearTimeout(errorTimerRef.current);
    errorTimerRef.current = setTimeout(() => {
      errorTimerRef.current = null;
      if (mountedRef.current) setErrorNotice(null);
    }, duration);
  };
  const [providers, setProviders] = useState<ProviderEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [errorNotice, setErrorNotice] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<SettingsSection>(() => {
    if (initialTab) return initialTab;
    if (typeof window !== 'undefined') {
      const params = new URLSearchParams(window.location.search);
      const tabParam = params.get('tab') || params.get('settings_tab');
      if (tabParam === 'workflows' || tabParam === 'search' || tabParam === 'appearance' || tabParam === 'vault') {
        return tabParam;
      }
    }
    return 'models';
  });

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
  const testRequestSeqRef = useRef<Record<string, number>>({});

  useEffect(() => { if (isOpen && initialTab) setActiveTab(initialTab); }, [initialTab, isOpen]);

  const fetchCatalog = async () => {
    try {
      setLoading(true);
      const catalog = await getProviderCatalog();
      setProviders(catalog || []);
      window.dispatchEvent(new Event('sthira:catalog-changed'));
    } catch (err: any) {
      setErrorNotice('Could not load AI connections. Close and reopen Settings to retry.');
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
      showTimedNotice(`${provider.display_name} ${updated.is_enabled ? 'enabled' : 'disabled'}.`, 3000);
      await fetchSearchProvidersList();
    } catch (err: any) {
      showTimedErrorNotice(`Failed to update provider status: ${err?.message || err}`, 4000);
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
      showTimedNotice(`${provider.display_name} set as primary web search provider.`, 3000);
      await fetchSearchProvidersList();
    } catch (err: any) {
      showTimedErrorNotice(`Failed to set default search provider: ${err?.message || err}`, 4000);
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
      showTimedNotice(`Updated settings for ${provider.display_name}.`, 3000);
      await fetchSearchProvidersList();
    } catch (err: any) {
      showTimedErrorNotice(`Failed to save provider settings: ${err?.message || err}`, 4000);
    }
  };

  const handleRunSearchTest = async (provider: SearchProviderEntry) => {
    const nextSeq = (testRequestSeqRef.current[provider.id] || 0) + 1;
    testRequestSeqRef.current[provider.id] = nextSeq;

    const query = testQueryMap[provider.id]?.trim() || 'rust async traits';
    try {
      setIsTestingMap((prev) => ({ ...prev, [provider.id]: true }));
      setTestErrorMap((prev) => ({ ...prev, [provider.id]: null }));
      const results = await testSearchProvider(provider.id, query);
      if (testRequestSeqRef.current[provider.id] !== nextSeq) {
        return;
      }
      setTestResultsMap((prev) => ({ ...prev, [provider.id]: results }));
      if (results.length > 0) {
        showTimedNotice(
          `Test search completed: received ${results.length} results from ${provider.display_name}. This checks search only; model generation was not tested.`,
          3000,
        );
      } else {
        showTimedNotice(
          `Test search returned 0 results from ${provider.display_name}. This checks search only; model generation was not tested.`,
          3000,
        );
      }
    } catch (err: any) {
      if (testRequestSeqRef.current[provider.id] !== nextSeq) {
        return;
      }
      const msg = typeof err === 'string' ? err : err?.message || 'Search request failed';
      setTestErrorMap((prev) => ({ ...prev, [provider.id]: msg }));
      showTimedErrorNotice(`Test search error (${provider.display_name}): ${msg}`, 5000);
    } finally {
      if (testRequestSeqRef.current[provider.id] === nextSeq) {
        setIsTestingMap((prev) => ({ ...prev, [provider.id]: false }));
      }
    }
  };

  useEffect(() => {
    if (isOpen) {
      fetchCatalog();
      fetchSearchProvidersList();
    } else {
      // Invalidate any in-flight test searches when modal is closed
      Object.keys(testRequestSeqRef.current).forEach((k) => {
        testRequestSeqRef.current[k] += 1;
      });
      setIsTestingMap({});
    }
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialogRef.current?.querySelector<HTMLButtonElement>('.settings-close-btn')?.focus();
    const keyboard = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); closeRef.current(); return; }
      if (event.key !== 'Tab') return;
      const controls = Array.from(dialogRef.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], summary, [tabindex="0"]') || []).filter(element => {
        if (element.closest('[hidden]')) return false;
        const closed = element.closest('details:not([open])');
        return !closed || element === closed.querySelector('summary');
      });
      const first = controls[0], last = controls.at(-1);
      if (event.shiftKey && (document.activeElement === first || !dialogRef.current?.contains(document.activeElement))) { event.preventDefault(); last?.focus(); }
      else if (!event.shiftKey && (document.activeElement === last || !dialogRef.current?.contains(document.activeElement))) { event.preventDefault(); first?.focus(); }
    };
    document.addEventListener('keydown', keyboard, true);
    return () => { document.removeEventListener('keydown', keyboard, true); if (previous?.isConnected) previous.focus(); };
  }, [isOpen]);

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

  if (!isOpen) return null;

  return (
    <div
      className="settings-modal-backdrop"
      data-testid="model-settings-modal"
    >
      <div ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby="workspace-settings-title" className="settings-modal-dialog animate-in fade-in zoom-in-95 duration-150">
        {/* 1. Header with Brand Badge & Amber Rule */}
        <div className="settings-header">
          <div className="settings-header-left">
            <button
              type="button"
              onClick={onClose}
              aria-label="Back to workspace"
              data-testid="settings-back-btn"
              className="settings-back-btn"
            >
              <ArrowLeft className="w-4 h-4" />
              <span>Back to workspace</span>
            </button>
            <div className="settings-brand-badge">
              <div className="settings-brand-icon">
                <Sprout className="w-4 h-4" />
              </div>
              <span className="settings-brand-name">Sthira</span>
            </div>
            <div className="settings-title-section">
              <div className="settings-amber-rule" />
              <h2 id="workspace-settings-title" className="settings-title">Settings</h2>
              <p className="settings-subtitle">
                Make room for the way you work.
              </p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <div className="settings-key-hint hidden sm:flex items-center gap-1.5 px-2 py-1 rounded-md bg-sthira-surface-muted border border-sthira-border text-[11px] text-sthira-text-muted font-mono">
              <kbd>Esc</kbd>
              <span>to exit</span>
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
          <nav className="settings-navigation" aria-label="Settings categories">
            <p className="settings-navigation-label">Workspace</p>
            {([
              ['models', 'AI & Models', Server], ['workflows', 'Workflows', Sparkles],
              ['search', 'Web Search', Globe], ['appearance', 'Appearance', Sun], ['vault', 'Workspace & Data', Layers],
            ] as const).map(([id, label, Icon]) => <button key={id} type="button" data-settings-section={id} aria-current={activeTab === id ? 'page' : undefined} className={activeTab === id ? 'active' : ''} onClick={() => setActiveTab(id)}><Icon size={17} /><span>{label}</span></button>)}
            <p className="settings-navigation-note">Your notes stay available, with or without AI.</p>
          </nav>

          <div className="settings-content-col">
            {activeTab === 'workflows' && <WorkflowSettingsPanel providers={providers} onNavigateTab={(tab) => setActiveTab(tab as SettingsSection)} />}
            {activeTab === 'models' && <AIModelsSettings providers={providers} loading={loading} reload={fetchCatalog} onCatalogChanged={onCatalogChanged} />}

          {/* 4. WEB SEARCH PROVIDERS TAB */}
          {activeTab === 'search' && (
            <div className="space-y-6 animate-in fade-in duration-100">
              <div className="settings-section-header">
                <h3 className="settings-section-title">Web Search Providers</h3>
                <p className="settings-section-sub">
                  Test Search checks this search provider only. Research also needs a working AI model.
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

                    {testResults && testResults.length === 0 && <p className="text-xs text-sthira-text-muted" role="status">No results for this test query. Try another query.</p>}
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
              <h3 className="settings-section-title">Appearance</h3>
              <p className="settings-section-sub">
                Choose a theme or follow your system appearance.
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

          </div>
        )}

        {/* 6. VAULT & STORAGE TAB */}
        {activeTab === 'vault' && (
          <div className="space-y-6 animate-in fade-in duration-100">
            <div className="settings-section-header">
              <h3 className="settings-section-title">Workspace &amp; Data</h3>
              <p className="settings-section-sub">
                Your notes and workspace data are stored locally in your vault.
              </p>
            </div>

            <div className="space-y-4">
<details className="settings-danger-zone"><summary>Delete workspace data</summary>              <div className="flex items-center justify-between p-4 bg-red-50/50 dark:bg-red-950/20 border border-red-200 dark:border-red-900/50 rounded-2xl shadow-xs">
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
              </div></details>
              {import.meta.env.DEV && <details className="settings-developer-tools"><summary>Developer tools</summary>              <div className="flex items-center justify-between p-4 bg-sthira-surface border border-sthira-border rounded-2xl shadow-xs">
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

</details>}
            </div>
          </div>
        )}
      </div>
    </div>

        <footer className="settings-footer">
          <p>Workflow preferences save automatically. Connection changes use their Save button.</p>
          <button type="button" onClick={onClose} className="settings-btn-primary" data-testid="btn-save-settings">Done</button>
        </footer>
      </div>
    </div>
  );
};

export default ModelSettingsModal;
