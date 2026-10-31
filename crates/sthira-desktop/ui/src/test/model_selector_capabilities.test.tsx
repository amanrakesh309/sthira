import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ModelSelector, WebSearchCapabilityBadge } from '../components/ModelSelector';
import { ExplorePlanView } from '../views/ExplorePlanView';
import { UniversalInputBar } from '../components/UniversalInputBar';
import { ModelSettingsModal } from '../components/ModelSettingsModal';
import type { ProviderModelEntry, ProviderEntry } from '../types/sthira';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
  Channel: class {
    onmessage?: (data: any) => void;
  },
}));

vi.mock('../api/ipc', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api/ipc')>();
  return {
    ...actual,
    onScheduledTopicReady: vi.fn(async () => () => {}),
  };
});

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const MOCK_MODELS: ProviderModelEntry[] = [
  {
    id: 'm-native',
    provider_id: 'perplexity',
    model_identifier: 'sonar-pro',
    display_name: 'Perplexity Sonar Pro',
    context_window: 128000,
    supports_structured_output: true,
    is_enabled: true,
    is_default: false,
    web_search_capability: 'native',
  },
  {
    id: 'm-agentic',
    provider_id: 'anthropic',
    model_identifier: 'claude-3-7-sonnet',
    display_name: 'Claude 3.7 Sonnet',
    context_window: 200000,
    supports_structured_output: true,
    is_enabled: true,
    is_default: true,
    web_search_capability: 'agentic_tool_call',
  },
  {
    id: 'm-augmented',
    provider_id: 'ollama',
    model_identifier: 'llama3.2:latest',
    display_name: 'Ollama Llama 3.2',
    context_window: 8192,
    supports_structured_output: true,
    is_enabled: true,
    is_default: false,
    web_search_capability: 'engine_augmented_only',
  },
  {
    id: 'm-unsupported',
    provider_id: 'local',
    model_identifier: 'bge-m3-dense',
    display_name: 'BGE-M3 Dense Offline',
    context_window: 4096,
    supports_structured_output: false,
    is_enabled: true,
    is_default: false,
    web_search_capability: 'unsupported',
  },
];

const MOCK_PROVIDERS: ProviderEntry[] = [
  {
    id: 'prov-1',
    provider_type: 'anthropic',
    display_name: 'Anthropic Claude',
    base_url: 'https://api.anthropic.com/v1',
    has_api_key: true,
    is_enabled: true,
    models: MOCK_MODELS,
  },
];

describe('Explicit Web Search Capability Detection & Badging', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation(async (cmd: string, _args: any) => {
      if (cmd === 'cmd_get_workspace_tree') {
        return { projects: [{ id: 'p1', name: 'Alpha Platform' }], orphaned_artifacts: [] };
      }
      if (cmd === 'cmd_get_provider_catalog') {
        return MOCK_PROVIDERS;
      }
      if (cmd === 'cmd_get_workspace_index') {
        return { counts: {}, items: [] };
      }
      if (cmd === 'cmd_list_scheduled_topics') {
        return [];
      }
      if (cmd === 'cmd_list_available_models') {
        return MOCK_MODELS.map((m) => ({
          id: m.id,
          provider_id: m.provider_id,
          provider_name: 'Anthropic Claude',
          provider_type: 'anthropic',
          model_identifier: m.model_identifier,
          display_name: m.display_name,
          context_window: m.context_window,
          supports_structured_output: m.supports_structured_output,
          is_enabled: m.is_enabled,
          is_default: m.is_default,
          web_search_capability: m.web_search_capability || 'agentic_tool_call',
        }));
      }
      if (cmd === 'cmd_update_model_search_capability') {
        return null;
      }
      if (cmd === 'cmd_get_search_providers') {
        return [];
      }
      return null;
    });
  });

  it('renders distinct capability badges with correct color schemes and labels', () => {
    const { rerender } = render(<WebSearchCapabilityBadge capability="native" />);
    const nativeBadge = screen.getByTestId('badge-capability-native');
    expect(nativeBadge).toBeInTheDocument();
    expect(nativeBadge.textContent).toContain('🌐 Native Search');
    expect(nativeBadge.className).toContain('text-cyan-700');

    rerender(<WebSearchCapabilityBadge capability="agentic_tool_call" />);
    const agenticBadge = screen.getByTestId('badge-capability-agentic_tool_call');
    expect(agenticBadge).toBeInTheDocument();
    expect(agenticBadge.textContent).toContain('🌐 Web Search Ready');
    expect(agenticBadge.className).toContain('text-emerald-700');

    rerender(<WebSearchCapabilityBadge capability="engine_augmented_only" />);
    const augmentedBadge = screen.getByTestId('badge-capability-engine_augmented_only');
    expect(augmentedBadge).toBeInTheDocument();
    expect(augmentedBadge.textContent).toContain('📄 Web Context Injected');

    rerender(<WebSearchCapabilityBadge capability="unsupported" />);
    const unsupportedBadge = screen.getByTestId('badge-capability-unsupported');
    expect(unsupportedBadge).toBeInTheDocument();
    expect(unsupportedBadge.textContent).toContain('🚫 Offline Only');
    expect(unsupportedBadge.className).toContain('text-stone-500');
  });

  it('ModelSelector dropdown renders all models with their respective capability badges', async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();

    render(
      <ModelSelector
        models={MOCK_MODELS}
        selectedModelId="m-agentic"
        onSelectModel={onSelect}
        dataTestId="test-model-selector"
      />
    );

    // Active selector chip shows active model and badge
    const chip = screen.getByTestId('test-model-selector');
    expect(chip).toBeInTheDocument();
    expect(chip.textContent).toContain('Claude 3.7 Sonnet');
    expect(screen.getByTestId('badge-capability-agentic_tool_call')).toBeInTheDocument();

    // Click to open dropdown
    await user.click(chip);

    // Verify all 4 models and their badges appear in dropdown
    expect(screen.getByTestId('model-option-m-native')).toBeInTheDocument();
    expect(screen.getByTestId('model-option-m-agentic')).toBeInTheDocument();
    expect(screen.getByTestId('model-option-m-augmented')).toBeInTheDocument();
    expect(screen.getByTestId('model-option-m-unsupported')).toBeInTheDocument();

    // Select native model
    await user.click(screen.getByTestId('model-option-m-native'));
    expect(onSelect).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'm-native', web_search_capability: 'native' })
    );
  });

  it('ExplorePlanView: shows quiet hint when EngineAugmentedOnly and disables search toggle when Unsupported', async () => {
    const user = userEvent.setup();

    render(<ExplorePlanView initialTab="research" />);

    // Wait for catalog to load
    await waitFor(() => {
      expect(screen.getByTestId('select-research-model')).toBeInTheDocument();
    });

    // Default model is m-agentic (Claude 3.7 Sonnet) -> Web search toggle is enabled
    const toggleSearch = screen.getByTestId('toggle-web-search');
    expect(toggleSearch).not.toHaveClass('cursor-not-allowed');
    expect(screen.queryByTestId('web-search-engine-augmented-hint')).not.toBeInTheDocument();
    expect(screen.queryByTestId('web-search-unsupported-hint')).not.toBeInTheDocument();

    // Open model selector and switch to Ollama Llama 3.2 (EngineAugmentedOnly)
    await user.click(screen.getByTestId('select-research-model'));
    await user.click(screen.getByTestId('model-option-m-augmented'));

    // Quiet hint icon should now appear with expected tooltip
    await waitFor(() => {
      const hint = screen.getByTestId('web-search-engine-augmented-hint');
      expect(hint).toBeInTheDocument();
      expect(hint.getAttribute('title')).toContain(
        "Sthira will automatically query SearXNG/Brave and inject scraped web pages into this model's prompt."
      );
    });

    // Open model selector and switch to BGE-M3 (Unsupported)
    await user.click(screen.getByTestId('select-research-model'));
    await user.click(screen.getByTestId('model-option-m-unsupported'));

    // Toggle should now be disabled/unsupported and show offline indicator
    await waitFor(() => {
      const unsupportedHint = screen.getByTestId('web-search-unsupported-hint');
      expect(unsupportedHint).toBeInTheDocument();
      expect(unsupportedHint.textContent).toContain('(Offline only)');
      expect(screen.getByTestId('search-mode-auto')).toBeDisabled();
      expect(screen.getByTestId('search-mode-always')).toBeDisabled();
    });
  });

  it('UniversalInputBar: integrates web search toggle with capability hint and disables for unsupported model', async () => {
    const user = userEvent.setup();
    const onStartResearch = vi.fn().mockResolvedValue(null);

    render(
      <UniversalInputBar
        onStartResearch={onStartResearch}
      />
    );

    // Wait for catalog to load default model (Claude 3.7 Sonnet - AgenticToolCall)
    await waitFor(() => {
      expect(screen.getByTestId('toggle-web-search')).toBeInTheDocument();
    });

    const webSearchBtn = screen.getByTestId('toggle-web-search');
    expect(webSearchBtn).not.toBeDisabled();

    // Open model dropdown and select Unsupported model
    const modelChip = screen.getByTitle('Switch Compute Model & Provider');
    await user.click(modelChip);

    const unsupportedOption = screen.getByText('BGE-M3 Dense Offline');
    await user.click(unsupportedOption);

    // Web search toggle should now be disabled
    await waitFor(() => {
      expect(screen.getByTestId('toggle-web-search')).toBeDisabled();
      expect(screen.getByTestId('web-search-unsupported-hint')).toBeInTheDocument();
    });
  });

  it('ModelSettingsModal: renders Web Search Support column and triggers IPC capability update', async () => {
    const user = userEvent.setup();

    render(
      <ModelSettingsModal
        isOpen={true}
        onClose={vi.fn()}
        onVaultMutated={vi.fn()}
      />
    );

    // Wait for models to load
    await waitFor(() => {
      expect(screen.getByTestId('select-capability-m-augmented')).toBeInTheDocument();
    });

    const selectEl = screen.getByTestId('select-capability-m-augmented') as HTMLSelectElement;
    expect(selectEl.value).toBe('engine_augmented_only');

    // Change Ollama Llama 3.2 capability to agentic_tool_call
    await user.selectOptions(selectEl, 'agentic_tool_call');

    // Verify cmd_update_model_search_capability is called
    expect(mockInvoke).toHaveBeenCalledWith('cmd_update_model_search_capability', {
      modelId: 'm-augmented',
      capability: 'agentic_tool_call',
    });
  });
});
