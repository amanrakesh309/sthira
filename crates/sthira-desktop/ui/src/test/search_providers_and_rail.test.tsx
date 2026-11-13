import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ModelSettingsModal } from '../components/ModelSettingsModal';


const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe('Phase 10: Search Providers UI Tests', () => {
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
    const searchTabBtn = screen.getByRole('button', { name: /^Web Search$/i });
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
      expect(screen.getByText(/Test Search checks this search provider only/)).toBeInTheDocument();
      expect(screen.getByText(/Official Architectural Reference & Standard/i)).toBeInTheDocument();
    });
  });

  it('renders truthful empty result notice when test search returns zero results', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'cmd_get_provider_catalog') return Promise.resolve([]);
      if (cmd === 'cmd_get_search_providers') return Promise.resolve(sampleProviders);
      if (cmd === 'cmd_test_search_provider') return Promise.resolve([]); // zero results
      return Promise.resolve(null);
    });

    render(
      <ModelSettingsModal
        isOpen={true}
        onClose={vi.fn()}
        onVaultMutated={vi.fn()}
      />
    );

    const searchTabBtn = screen.getByRole('button', { name: /^Web Search$/i });
    fireEvent.click(searchTabBtn);

    await waitFor(() => {
      expect(screen.getByText(/DuckDuckGo \(Headless HTML\)/i)).toBeInTheDocument();
    });

    const testBtns = screen.getAllByRole('button', { name: /Test Search/i });
    fireEvent.click(testBtns[0]);

    await waitFor(() => {
      expect(screen.getByText(/Test search returned 0 results from DuckDuckGo/i)).toBeInTheDocument();
      expect(screen.getByText(/No results for this test query\. Try another query\./i)).toBeInTheDocument();
    });
  });

  it('displays structured error when test search fails with anti-bot challenge or rate limit', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'cmd_get_provider_catalog') return Promise.resolve([]);
      if (cmd === 'cmd_get_search_providers') return Promise.resolve(sampleProviders);
      if (cmd === 'cmd_test_search_provider') {
        return Promise.reject("Anti-bot challenge detected on 'duckduckgo': verification required");
      }
      return Promise.resolve(null);
    });

    render(
      <ModelSettingsModal
        isOpen={true}
        onClose={vi.fn()}
        onVaultMutated={vi.fn()}
      />
    );

    const searchTabBtn = screen.getByRole('button', { name: /^Web Search$/i });
    fireEvent.click(searchTabBtn);

    await waitFor(() => {
      expect(screen.getByText(/DuckDuckGo \(Headless HTML\)/i)).toBeInTheDocument();
    });

    const testBtns = screen.getAllByRole('button', { name: /Test Search/i });
    fireEvent.click(testBtns[0]);

    await waitFor(() => {
      const errorElements = screen.getAllByText(
        /Anti-bot challenge detected on 'duckduckgo': verification required/i
      );
      expect(errorElements.length).toBeGreaterThanOrEqual(1);
    });
  });

  it('ignores stale test search completions when modal is closed before response returns', async () => {
    let resolveSearch: (val: any) => void;
    const pendingSearch = new Promise((resolve) => {
      resolveSearch = resolve;
    });

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'cmd_get_provider_catalog') return Promise.resolve([]);
      if (cmd === 'cmd_get_search_providers') return Promise.resolve(sampleProviders);
      if (cmd === 'cmd_test_search_provider') return pendingSearch;
      return Promise.resolve(null);
    });

    const { rerender } = render(
      <ModelSettingsModal
        isOpen={true}
        onClose={vi.fn()}
        onVaultMutated={vi.fn()}
      />
    );

    const searchTabBtn = screen.getByRole('button', { name: /^Web Search$/i });
    fireEvent.click(searchTabBtn);

    await waitFor(() => {
      expect(screen.getByText(/DuckDuckGo \(Headless HTML\)/i)).toBeInTheDocument();
    });

    const testBtns = screen.getAllByRole('button', { name: /Test Search/i });
    fireEvent.click(testBtns[0]);

    // Close the modal while the test search is still pending
    rerender(
      <ModelSettingsModal
        isOpen={false}
        onClose={vi.fn()}
        onVaultMutated={vi.fn()}
      />
    );

    // Resolve the late response
    resolveSearch!([
      {
        title: 'Late Completed Result',
        url: 'https://example.com/late',
        snippet: 'Late.',
        score: 1.0,
      },
    ]);

    // Give microtasks time to execute
    await new Promise((r) => setTimeout(r, 50));

    // When modal reopens, stale result must not be shown
    rerender(
      <ModelSettingsModal
        isOpen={true}
        onClose={vi.fn()}
        onVaultMutated={vi.fn()}
      />
    );

    const searchTabBtn2 = screen.getByRole('button', { name: /^Web Search$/i });
    fireEvent.click(searchTabBtn2);

    await waitFor(() => {
      expect(screen.getByText(/DuckDuckGo \(Headless HTML\)/i)).toBeInTheDocument();
    });

    expect(screen.queryByText(/Late Completed Result/i)).not.toBeInTheDocument();
  });
});

