import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ModelSettingsModal } from '../components/ModelSettingsModal';
import { RightModeRail } from '../components/RightModeRail';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe('Phase 10: Search Providers & Web Evidence Extraction UI Tests', () => {
  const sampleProviders = [
    {
      id: 'duckduckgo',
      display_name: 'DuckDuckGo (Headless HTML)',
      provider_type: 'duckduckgo_scraper',
      endpoint_url: 'https://html.duckduckgo.com/html/',
      api_key: null,
      is_enabled: true,
      is_default: true,
    },
    {
      id: 'searxng',
      display_name: 'SearXNG (Self-Hosted / Local)',
      provider_type: 'searxng',
      endpoint_url: 'http://localhost:8080',
      api_key: null,
      is_enabled: false,
      is_default: false,
    },
    {
      id: 'brave',
      display_name: 'Brave Search API',
      provider_type: 'brave',
      endpoint_url: 'https://api.search.brave.com/res/v1/web/search',
      api_key: null,
      is_enabled: false,
      is_default: false,
    },
  ];

  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation((cmd: string, args: any) => {
      if (cmd === 'cmd_get_provider_catalog') {
        return Promise.resolve([]);
      }
      if (cmd === 'cmd_get_search_providers') {
        return Promise.resolve(sampleProviders);
      }
      if (cmd === 'cmd_test_search_provider') {
        return Promise.resolve([
          {
            title: 'Official Architectural Reference & Standard',
            url: 'https://docs.rs/sthira-core',
            snippet: 'Deterministic state transitions with SQLite reversibility.',
            score: 0.95,
          },
        ]);
      }
      if (cmd === 'cmd_save_search_provider') {
        return Promise.resolve();
      }
      if (cmd === 'cmd_answer_question') {
        return Promise.resolve('Synthesized answer from local knowledge.');
      }
      if (cmd === 'cmd_execute_web_research') {
        return Promise.resolve({
          original_prompt: args?.query,
          generated_queries: [`${args?.query} rust`],
          sources_scraped: 1,
          provider_used: 'DuckDuckGo (Headless HTML)',
          findings: [
            {
              query: `${args?.query} rust`,
              url: 'https://docs.rs/sthira',
              domain: 'docs.rs',
              favicon_url: 'https://www.google.com/s2/favicons?domain=docs.rs&sz=32',
              title: 'Sthira Rust Reference',
              quote: 'All operations respect the zero unsolicited writes invariant.',
              start_offset: 0,
              end_offset: 58,
              markdown_excerpt: '# Sthira\n\nAll operations respect the zero unsolicited writes invariant.',
              verified: true,
            },
          ],
        });
      }
      if (cmd === 'cmd_import_web_source_to_vault') {
        return Promise.resolve('art-source-created-123');
      }
      return Promise.resolve(null);
    });
  });

  it('renders Web Search Providers tab in ModelSettingsModal and runs test search', async () => {
    render(
      <ModelSettingsModal
        isOpen={true}
        onClose={vi.fn()}
        onVaultMutated={vi.fn()}
      />
    );

    // Switch to "Web Search Providers" tab
    const searchTabBtn = screen.getByRole('button', { name: /Web Search Providers/i });
    fireEvent.click(searchTabBtn);

    // Assert provider titles are displayed
    await waitFor(() => {
      expect(screen.getByText(/DuckDuckGo \(Headless HTML\)/i)).toBeInTheDocument();
      expect(screen.getByText(/SearXNG \(Self-Hosted \/ Local\)/i)).toBeInTheDocument();
      expect(screen.getByText(/Brave Search API/i)).toBeInTheDocument();
    });

    // Run test search on DuckDuckGo
    const testBtns = screen.getAllByRole('button', { name: /Test Search/i });
    expect(testBtns.length).toBeGreaterThan(0);
    fireEvent.click(testBtns[0]);

    // Verify test search results are rendered
    await waitFor(() => {
      expect(screen.getByText(/Returned 1 Live Results/i)).toBeInTheDocument();
      expect(screen.getByText(/Official Architectural Reference & Standard/i)).toBeInTheDocument();
    });
  });

  it('renders [Include Web Search] chip in RightModeRail and displays findings with [Import to Vault]', async () => {
    const onMutated = vi.fn();
    render(
      <RightModeRail
        activeProjectId="proj-distributed-consensus"
        activeArtifactId="art-note-1"
        activeDocumentContent="# Notes"
        onWorkspaceMutated={onMutated}
      />
    );

    // Open Research Mode tab using aria-label
    const researchTabIcon = screen.getByLabelText('Research');
    fireEvent.click(researchTabIcon);

    // Verify [Include Web Search] chip is present and active
    const webSearchChip = screen.getByRole('button', { name: /Include Web Search/i });
    expect(webSearchChip).toBeInTheDocument();
    expect(screen.getByText(/Live Search Active/i)).toBeInTheDocument();

    // Fill query
    const input = screen.getByPlaceholderText(/How does speculative decoding scale/i);
    fireEvent.change(input, { target: { value: 'Rust asynchronous runtime invariants' } });

    // Submit inquiry
    const submitBtn = screen.getByRole('button', { name: /Investigate Evidence/i });
    fireEvent.click(submitBtn);

    // Verify web citation card with domain and verified quote
    await waitFor(() => {
      expect(screen.getByText(/Web Citations/i)).toBeInTheDocument();
      expect(screen.getByText(/docs.rs/i)).toBeInTheDocument();
      expect(screen.getByText(/Verified Quote • External Source/i)).toBeInTheDocument();
    });

    // Click [Import to Vault]
    const importBtn = screen.getAllByRole('button', { name: /Import to Vault/i })[0];
    fireEvent.click(importBtn);

    // Verify button changes to "Imported to Vault" and mutates workspace
    await waitFor(() => {
      expect(screen.getAllByText(/Imported to Vault/i).length).toBeGreaterThan(0);
      expect(onMutated).toHaveBeenCalled();
    });

    expect(mockInvoke).toHaveBeenCalledWith(
      'cmd_import_web_source_to_vault',
      expect.objectContaining({
        url: 'https://docs.rs/sthira',
        title: 'Sthira Rust Reference',
      })
    );
  });
});
