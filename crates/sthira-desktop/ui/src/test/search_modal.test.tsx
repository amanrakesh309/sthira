import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { SearchModal } from '../components/SearchModal';
import * as ipc from '../api/ipc';

vi.mock('../api/ipc', async (original) => ({
  ...await original<typeof ipc>(),
  searchAll: vi.fn(),
}));

describe('SearchModal Global Search (NAV-05)', () => {
  const mockResults = [
    {
      id: 'proj-1',
      kind: 'project' as const,
      title: 'Knowledge Platform',
      excerpt: 'Build local knowledge workspace',
    },
    {
      id: 'art-1',
      kind: 'artifact' as const,
      title: 'Architecture Spec',
      excerpt: 'System components and traits',
    },
  ];

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.searchAll).mockResolvedValue(mockResults);
  });

  it('renders with dialog semantics and searches vault on input', async () => {
    const user = userEvent.setup();
    const handleClose = vi.fn();
    const handleOpenProject = vi.fn();
    const handleOpenArtifact = vi.fn();

    render(
      <SearchModal
        isOpen={true}
        onClose={handleClose}
        onOpenProject={handleOpenProject}
        onOpenArtifact={handleOpenArtifact}
      />
    );

    const dialog = screen.getByRole('dialog', { name: 'Search workspace' });
    expect(dialog).toBeInTheDocument();
    expect(dialog).toHaveAttribute('aria-modal', 'true');

    const input = screen.getByRole('textbox', { name: 'Search workspace entities' });
    expect(input).toBeInTheDocument();
    expect(input).toHaveFocus();

    await user.type(input, 'Architecture');

    await waitFor(() => {
      expect(ipc.searchAll).toHaveBeenCalledWith('Architecture');
    });

    expect(await screen.findByText('Architecture Spec')).toBeInTheDocument();
    expect(screen.getByText('Knowledge Platform')).toBeInTheDocument();

    // Clicking an artifact result opens it and closes modal
    const item = screen.getByRole('option', { name: 'Open Architecture Spec' });
    await user.click(item);

    expect(handleOpenArtifact).toHaveBeenCalledWith('art-1');
    expect(handleClose).toHaveBeenCalled();
  });

  it('navigates search results with ArrowDown / ArrowUp and selects with Enter', async () => {
    const user = userEvent.setup();
    const handleClose = vi.fn();
    const handleOpenProject = vi.fn();
    const handleOpenArtifact = vi.fn();

    render(
      <SearchModal
        isOpen={true}
        onClose={handleClose}
        onOpenProject={handleOpenProject}
        onOpenArtifact={handleOpenArtifact}
      />
    );

    const input = screen.getByRole('textbox', { name: 'Search workspace entities' });
    await user.type(input, 'test');

    await waitFor(() => {
      expect(screen.getByText('Knowledge Platform')).toBeInTheDocument();
    });

    const options = screen.getAllByRole('option');
    expect(options).toHaveLength(2);
    // Initially first item is selected (index 0)
    expect(options[0]).toHaveAttribute('aria-selected', 'true');
    expect(options[1]).toHaveAttribute('aria-selected', 'false');

    // Press ArrowDown -> moves to index 1
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    expect(options[0]).toHaveAttribute('aria-selected', 'false');
    expect(options[1]).toHaveAttribute('aria-selected', 'true');

    // Press Enter on second option ('Architecture Spec')
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(handleOpenArtifact).toHaveBeenCalledWith('art-1');
    expect(handleClose).toHaveBeenCalled();
  });

  it('restores focus to the opener element when dismissed via Escape', async () => {
    const user = userEvent.setup();

    function TestApp() {
      const [isOpen, setIsOpen] = useState(false);
      return (
        <div>
          <button
            type="button"
            data-testid="search-opener-btn"
            onClick={() => setIsOpen(true)}
          >
            Open Search
          </button>
          <SearchModal
            isOpen={isOpen}
            onClose={() => setIsOpen(false)}
            onOpenProject={vi.fn()}
            onOpenArtifact={vi.fn()}
          />
        </div>
      );
    }

    render(<TestApp />);

    const opener = screen.getByTestId('search-opener-btn');
    opener.focus();
    expect(opener).toHaveFocus();

    // Click opener button
    await user.click(opener);

    // Modal is open, input is focused
    const input = await screen.findByRole('textbox', { name: 'Search workspace entities' });
    expect(input).toHaveFocus();

    // Press Escape
    fireEvent.keyDown(input, { key: 'Escape' });

    // Modal closes and focus is restored to opener
    await waitFor(() => {
      expect(opener).toHaveFocus();
    });
  });

  it('shows truthful empty state guidance when search yields no matches', async () => {
    vi.mocked(ipc.searchAll).mockResolvedValue([]);
    const user = userEvent.setup();

    render(
      <SearchModal
        isOpen={true}
        onClose={vi.fn()}
        onOpenProject={vi.fn()}
        onOpenArtifact={vi.fn()}
      />
    );

    const input = screen.getByRole('textbox', { name: 'Search workspace entities' });
    await user.type(input, 'NonExistent');

    await waitFor(() => {
      expect(screen.getByText(/No matching projects or notes found for/)).toBeInTheDocument();
    });
  });
});
