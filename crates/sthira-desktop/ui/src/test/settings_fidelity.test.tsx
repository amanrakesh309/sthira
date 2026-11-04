import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ModelSettingsModal } from '../components/ModelSettingsModal';
import { ThemeProvider } from '../context/ThemeContext';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe('Model & Workspace Settings Visual Fidelity and Functionality', () => {
  const sampleProviders = [
    {
      id: 'ollama',
      display_name: 'Ollama (Local)',
      provider_type: 'ollama',
      base_url: 'http://localhost:11434',
      has_api_key: false,
      is_enabled: true,
      models: [
        {
          id: 'model-1',
          provider_id: 'ollama',
          model_identifier: 'llama3:latest',
          display_name: 'Llama 3 8B',
          is_default: true,
          is_enabled: true,
          context_window: 8192,
          web_search_capability: 'browser_fallback',
        },
        {
          id: 'model-2',
          provider_id: 'ollama',
          model_identifier: 'mistral:latest',
          display_name: 'Mistral 7B',
          is_default: false,
          is_enabled: true,
          context_window: 32768,
          web_search_capability: 'unsupported',
        },
      ],
    },
    {
      id: 'openrouter',
      display_name: 'OpenRouter',
      provider_type: 'openrouter',
      base_url: 'https://openrouter.ai/api/v1',
      has_api_key: true,
      is_enabled: true,
      models: [],
    },
  ];

  const sampleSearchProviders = [
    {
      id: 'duckduckgo',
      display_name: 'DuckDuckGo (Headless HTML)',
      provider_type: 'duckduckgo_scraper',
      endpoint_url: 'https://html.duckduckgo.com/html/',
      api_key: null,
      is_enabled: true,
      is_default: true,
    },
  ];

  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'cmd_get_provider_catalog') {
        return Promise.resolve(sampleProviders);
      }
      if (cmd === 'cmd_get_search_providers') {
        return Promise.resolve(sampleSearchProviders);
      }
      if (cmd === 'cmd_set_default_model') {
        return Promise.resolve();
      }
      if (cmd === 'cmd_save_provider') {
        return Promise.resolve();
      }
      return Promise.resolve(null);
    });
  });

  it('renders modal with editorial Fraunces heading, Sthira brand badge, and amber rule', async () => {
    render(
      <ThemeProvider>
        <ModelSettingsModal isOpen={true} onClose={vi.fn()} onVaultMutated={vi.fn()} />
      </ThemeProvider>
    );

    // Brand badge
    expect(screen.getByText('Sthira')).toBeInTheDocument();

    // Fraunces serif heading
    const heading = screen.getByRole('heading', { level: 2, name: /Model & Workspace Settings/i });
    expect(heading).toBeInTheDocument();
    expect(heading).toHaveClass('settings-title');

    // Subtitle
    expect(
      screen.getByText(/Configure your models, tools, and workspace experience/i)
    ).toBeInTheDocument();

    // Close button
    const closeBtn = screen.getByRole('button', { name: /Close Settings/i });
    expect(closeBtn).toBeInTheDocument();
    expect(closeBtn).toHaveClass('settings-close-btn');
  });

  it('renders horizontal navigation tab pills and switches active tabs', async () => {
    render(
      <ThemeProvider>
        <ModelSettingsModal isOpen={true} onClose={vi.fn()} onVaultMutated={vi.fn()} />
      </ThemeProvider>
    );

    const tabsRow = document.querySelector('.settings-tabs-row');
    expect(tabsRow).toBeInTheDocument();

    // Verify all 4 tab pills exist
    const modelsTab = screen.getByRole('button', { name: /Connected Providers & Models/i });
    const searchTab = screen.getByRole('button', { name: /Web Search Providers/i });
    const appearanceTab = screen.getByRole('button', { name: /Appearance/i });
    const vaultTab = screen.getByRole('button', { name: /Vault & Storage/i });

    expect(modelsTab).toHaveClass('active');
    expect(searchTab).not.toHaveClass('active');

    // Switch to Appearance tab
    fireEvent.click(appearanceTab);
    expect(appearanceTab).toHaveClass('active');
    expect(modelsTab).not.toHaveClass('active');
    expect(screen.getAllByText(/Warm Stone/i).length).toBeGreaterThan(0);

    // Switch to Vault & Storage tab
    fireEvent.click(vaultTab);
    expect(vaultTab).toHaveClass('active');
    expect(screen.getByText(/Vault Invariants & Local Storage/i)).toBeInTheDocument();
  });

  it('renders 2-column layout with guidance card, quick setup navigation, and provider card', async () => {
    render(
      <ThemeProvider>
        <ModelSettingsModal isOpen={true} onClose={vi.fn()} onVaultMutated={vi.fn()} />
      </ThemeProvider>
    );

    // Left guidance column
    expect(screen.getByText('Your AI workspace, your way.')).toBeInTheDocument();
    expect(
      screen.getByText(/Connect the model providers you use, choose your models, and set up a default/i)
    ).toBeInTheDocument();

    // Quick setup shortcuts
    expect(screen.getByTestId('quick-setup-search')).toBeInTheDocument();
    expect(screen.getByTestId('quick-setup-appearance')).toBeInTheDocument();
    expect(screen.getByTestId('quick-setup-vault')).toBeInTheDocument();

    // Click quick setup shortcut to navigate to search
    fireEvent.click(screen.getByTestId('quick-setup-search'));
    const searchTab = screen.getByRole('button', { name: /Web Search Providers/i });
    expect(searchTab).toHaveClass('active');

    // Switch back to models tab
    fireEvent.click(screen.getByRole('button', { name: /Connected Providers & Models/i }));

    // Right column: Provider cards
    await waitFor(() => {
      expect(screen.getByText('Ollama (Local)')).toBeInTheDocument();
      expect(screen.getByText('OpenRouter')).toBeInTheDocument();
    });

    // Connection states: Ollama has 2 models (Connected), OpenRouter has 0 models (Not Connected)
    expect(screen.getByText('Connected')).toBeInTheDocument();
    expect(screen.getByText('Not Connected')).toBeInTheDocument();
  });

  it('supports model filtering, capability display, and setting default model', async () => {
    render(
      <ThemeProvider>
        <ModelSettingsModal isOpen={true} onClose={vi.fn()} onVaultMutated={vi.fn()} />
      </ThemeProvider>
    );

    await waitFor(() => {
      expect(screen.getByText('Llama 3 8B')).toBeInTheDocument();
      expect(screen.getByText('Mistral 7B')).toBeInTheDocument();
    });

    // Verify context window badges
    expect(screen.getByText('8K context')).toBeInTheDocument();
    expect(screen.getByText('32K context')).toBeInTheDocument();

    // Verify default badge
    expect(screen.getByText('Default')).toBeInTheDocument();

    // Test model search filtering
    const searchInputs = screen.getAllByPlaceholderText('Search models...');
    const searchInput = searchInputs[0];
    fireEvent.change(searchInput, { target: { value: 'llama' } });

    expect(screen.getByText('Llama 3 8B')).toBeInTheDocument();
    expect(screen.queryByText('Mistral 7B')).not.toBeInTheDocument();

    // Clear search
    fireEvent.change(searchInput, { target: { value: '' } });
    expect(screen.getByText('Mistral 7B')).toBeInTheDocument();

    // Set Mistral 7B as default model
    const radioButtons = document.querySelectorAll('.settings-radio-btn');
    expect(radioButtons.length).toBeGreaterThanOrEqual(2);
    fireEvent.click(radioButtons[1]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('cmd_set_default_model', {
        modelId: 'model-2',
      });
    });
  });

  it('footer preserves Cancel and Save buttons and calls onClose', () => {
    const handleClose = vi.fn();
    render(
      <ThemeProvider>
        <ModelSettingsModal isOpen={true} onClose={handleClose} onVaultMutated={vi.fn()} />
      </ThemeProvider>
    );

    const cancelBtn = screen.getByRole('button', { name: /Cancel/i });
    expect(cancelBtn).toBeInTheDocument();

    const saveBtn = screen.getByRole('button', { name: /Save Changes/i });
    expect(saveBtn).toBeInTheDocument();

    fireEvent.click(cancelBtn);
    expect(handleClose).toHaveBeenCalledTimes(1);
  });
});
