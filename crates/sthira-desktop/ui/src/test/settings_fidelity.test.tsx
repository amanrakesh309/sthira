import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ModelSettingsModal } from '../components/ModelSettingsModal';
import { App } from '../App';
import { ThemeProvider } from '../context/ThemeContext';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
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
    const heading = screen.getByRole('heading', { level: 2, name: /Settings/i });
    expect(heading).toBeInTheDocument();
    expect(heading).toHaveClass('settings-title');

    // Subtitle
    expect(
      screen.getByText(/Make room for the way you work/i)
    ).toBeInTheDocument();

    // Close button
    const closeBtn = screen.getByRole('button', { name: /Close Settings/i });
    expect(closeBtn).toBeInTheDocument();
    expect(closeBtn).toHaveClass('settings-close-btn');
  });

  it('uses one navigation for models, workflows, search, appearance and data', async () => {
    render(<ThemeProvider><ModelSettingsModal isOpen onClose={vi.fn()} onVaultMutated={vi.fn()} /></ThemeProvider>);
    const navigation = screen.getByRole('navigation', { name: 'Settings categories' });
    expect(navigation.querySelectorAll('button')).toHaveLength(5);
    expect(document.querySelector('.settings-tabs-row')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Appearance' }));
    expect(screen.getByRole('heading', { name: 'Appearance' })).toBeInTheDocument();
    expect(screen.getByTestId('theme-option-system')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Web Search' }));
    expect(screen.getByRole('heading', { name: 'Web Search Providers' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'AI & Models' }));
    expect(await screen.findByText('Ollama (Local)')).toBeInTheDocument();
    expect(screen.getAllByText('Not checked')).toHaveLength(2);
  });

  it('keeps model details on demand and supports setting the workspace default', async () => {
    render(<ThemeProvider><ModelSettingsModal isOpen onClose={vi.fn()} onVaultMutated={vi.fn()} /></ThemeProvider>);
    await screen.findByText('Llama 3 8B');
    expect(screen.queryByText('Context window')).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText('Search models…')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Information about Llama 3 8B' }));
    expect(screen.getByText('8,192 tokens (catalog value)')).toBeInTheDocument();
    expect(screen.getByText('Default')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Use Mistral 7B as workspace default' }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('cmd_set_default_model', { modelId: 'model-2' }));
  });

  it('offers Done and explains which settings save immediately', () => {
    const close = vi.fn();
    render(<ThemeProvider><ModelSettingsModal isOpen onClose={close} onVaultMutated={vi.fn()} /></ThemeProvider>);
    expect(screen.queryByRole('button', { name: 'Cancel' })).not.toBeInTheDocument();
    expect(screen.getByText(/Workflow preferences save automatically/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(close).toHaveBeenCalledOnce();
  });

  it('clicking Settings in LeftSidebar opens ModelSettingsModal without blanking page, and can be closed', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'cmd_get_home_state') {
        return Promise.resolve({ projects: [], recent_items: [], loose_captures: [] });
      }
      if (cmd === 'cmd_get_workspace_tree') {
        return Promise.resolve({ projects: [], unassociated_pages: [] });
      }
      if (cmd === 'cmd_get_sidebar_items') {
        return Promise.resolve({ projects: [], notes: [], objects: [] });
      }
      if (cmd === 'cmd_list_object_types') return Promise.resolve([]);
      if (cmd === 'cmd_get_provider_catalog') {
        return Promise.resolve(sampleProviders);
      }
      if (cmd === 'cmd_get_search_providers') {
        return Promise.resolve(sampleSearchProviders);
      }
      return Promise.resolve(null);
    });

    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <App />
      </ThemeProvider>
    );

    // Initial state: modal is NOT in the document
    expect(screen.queryByTestId('model-settings-modal')).not.toBeInTheDocument();

    // Find and click the Settings button in LeftSidebar
    const sidebarSettingsBtn = screen.getByRole('button', { name: /^settings$/i });
    expect(sidebarSettingsBtn).toBeInTheDocument();
    await user.click(sidebarSettingsBtn);

    // Modal renders successfully with title and tabs without crashing or blanking page
    await waitFor(() => {
      expect(screen.getByTestId('model-settings-modal')).toBeInTheDocument();
      expect(screen.getByRole('heading', { level: 2, name: /Settings/i })).toBeInTheDocument();
    });

    // Close button dismisses the modal
    const closeBtn = screen.getByRole('button', { name: /Close Settings/i });
    await user.click(closeBtn);

    await waitFor(() => {
      expect(screen.queryByTestId('model-settings-modal')).not.toBeInTheDocument();
    });

    // Main workspace remains intact and rendered
    expect(screen.getByTestId('center-workspace')).toBeInTheDocument();
  });

  it('clicking Model Settings chip on HomeView opens modal cleanly', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'cmd_get_home_state') {
        return Promise.resolve({ projects: [], recent_items: [], loose_captures: [] });
      }
      if (cmd === 'cmd_get_workspace_tree') {
        return Promise.resolve({ projects: [], unassociated_pages: [] });
      }
      if (cmd === 'cmd_get_sidebar_items') {
        return Promise.resolve({ projects: [], notes: [], objects: [] });
      }
      if (cmd === 'cmd_list_object_types') return Promise.resolve([]);
      if (cmd === 'cmd_get_provider_catalog') {
        return Promise.resolve(sampleProviders);
      }
      if (cmd === 'cmd_get_search_providers') {
        return Promise.resolve(sampleSearchProviders);
      }
      return Promise.resolve(null);
    });

    const user = userEvent.setup();
    render(
      <ThemeProvider>
        <App />
      </ThemeProvider>
    );

    const homeSettingsBtn = screen.getByTestId('home-model-chip');
    expect(homeSettingsBtn).toBeInTheDocument();
    await user.click(homeSettingsBtn);

    await waitFor(() => {
      expect(screen.getByTestId('model-settings-modal')).toBeInTheDocument();
    });
  });

  it('keeps navigation available across sections and contains destructive actions', () => {
    render(<ThemeProvider><ModelSettingsModal isOpen onClose={vi.fn()} onVaultMutated={vi.fn()} /></ThemeProvider>);
    for (const name of ['Workflows', 'Web Search', 'Appearance', 'Workspace & Data', 'AI & Models']) {
      fireEvent.click(screen.getByRole('button', { name }));
      expect(screen.getByRole('navigation', { name: 'Settings categories' }).querySelectorAll('button')).toHaveLength(5);
    }
    fireEvent.click(screen.getByRole('button', { name: 'Workspace & Data' }));
    expect(screen.getByText('Delete workspace data').closest('details')).not.toHaveAttribute('open');
    expect(screen.queryByText('Aesthetic Design Invariants')).not.toBeInTheDocument();
  });
});
