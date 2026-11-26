import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { CanvasView } from '../components/CanvasView';
import * as ipc from '../api/ipc';
import type { CanvasDataView } from '../types/sthira';

vi.mock('../api/ipc', async (original) => ({
  ...await original<typeof ipc>(),
  canvasOps: vi.fn(),
  revertChangeSet: vi.fn(),
}));

describe('CanvasView Spatial Surface', () => {
  const mockCanvasData = {
    id: 'canvas-1',
    project_id: 'proj-1',
    name: 'System Concept Map',
    items: [
      {
        item_id: 'item-1',
        item_kind: 'artifact' as const,
        artifact_id: 'art-1',
        title: 'Architecture Spec',
        excerpt: 'Core system components and traits.',
        x: 100,
        y: 150,
        w: 280,
        h: 140,
        z: 1,
      },
      {
        item_id: 'item-2',
        item_kind: 'text' as const,
        artifact_id: null,
        title: 'Caching Strategy',
        excerpt: 'Investigate caching layer.',
        x: 450,
        y: 150,
        w: 280,
        h: 140,
        z: 2,
      },
    ],
    links: [
      {
        source_artifact_id: 'item-1',
        target_artifact_id: 'item-2',
        relationship_type: 'references' as const,
        origin: 'user_recorded' as const,
      },
    ],
  };

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.canvasOps).mockResolvedValue({
      success: true,
      canvas: mockCanvasData,
      change_set_id: 'changeset-align-1',
    });
    vi.mocked(ipc.revertChangeSet).mockResolvedValue('cs-revert-1');
  });

  it('renders canvas header, cards, and SVG connector lines with semantic tokens', async () => {
    render(<CanvasView canvasId="canvas-1" projectId="proj-1" onBack={vi.fn()} />);

    expect(await screen.findByText('System Concept Map')).toBeInTheDocument();
    expect(screen.getByText('Architecture Spec')).toBeInTheDocument();
    expect(screen.getByText('Caching Strategy')).toBeInTheDocument();
    expect(screen.getByText('references')).toBeInTheDocument();
  });

  it('supports Auto-Align and provides an Undo affordance that reverts the layout change set', async () => {
    const user = userEvent.setup();
    render(<CanvasView canvasId="canvas-1" projectId="proj-1" onBack={vi.fn()} />);

    await screen.findByText('System Concept Map');
    const autoAlignBtn = screen.getByRole('button', { name: 'Auto-Align' });
    await user.click(autoAlignBtn);

    expect(ipc.canvasOps).toHaveBeenCalledWith({
      action: 'BatchUpdatePositions',
      payload: expect.objectContaining({
        canvas_id: 'canvas-1',
        positions: expect.any(Array),
      }),
    });

    const undoBtn = await screen.findByRole('button', { name: /Undo/i });
    expect(undoBtn).toBeInTheDocument();

    await user.click(undoBtn);
    expect(ipc.revertChangeSet).toHaveBeenCalledWith('changeset-align-1');
  });

  it('offers saved pages on an empty canvas, then opens a placed card', async () => {
    const user = userEvent.setup();
    const onOpenArtifact = vi.fn();
    const empty = { ...mockCanvasData, items: [], links: [] };
    const placed = { ...empty, items: [mockCanvasData.items[0]] };
    let canvas: CanvasDataView = empty;
    vi.mocked(ipc.canvasOps).mockImplementation(async request => {
      switch (request.action) {
        case 'GetCanvas': return { success: true, canvas };
        case 'ListPages': return { success: true, pages: [{ id: 'art-1', title: 'Architecture Spec', excerpt: '', kind: 'document' }] };
        case 'PlacePage':
          canvas = placed;
          return { success: true };
        default: throw new Error('Unexpected canvas action');
      }
    });
    render(<CanvasView canvasId="canvas-1" onBack={vi.fn()} onOpenArtifact={onOpenArtifact} />);

    expect(await screen.findByRole('region', { name: 'Choose a saved page' })).toBeVisible();
    await user.click(screen.getByRole('button', { name: 'Add existing page' }));
    expect(screen.getByRole('textbox', { name: 'Search saved pages' })).toHaveFocus();
    await user.click(await screen.findByRole('button', { name: 'Add Architecture Spec to canvas' }));
    expect(ipc.canvasOps).toHaveBeenCalledWith({ action: 'PlacePage', payload: { canvas_id: 'canvas-1', artifact_id: 'art-1', x: 24, y: 24 } });
    expect(onOpenArtifact).not.toHaveBeenCalled();

    await user.click(await screen.findByRole('button', { name: 'Open Architecture Spec' }));
    expect(onOpenArtifact).toHaveBeenCalledWith('art-1', placed);
  });

  it.each([
    { button: 'Add Note', placeholderId: null },
    { button: 'Open as note', placeholderId: 'item-2' },
  ])('$button opens the real saved page returned by the native command', async ({ button, placeholderId }) => {
    const user = userEvent.setup();
    const onOpenArtifact = vi.fn();
    vi.mocked(ipc.canvasOps).mockImplementation(async request => request.action === 'CreateNote'
      ? { success: true, artifact_id: 'created-a' }
      : { success: true, canvas: mockCanvasData });
    render(<CanvasView canvasId="canvas-1" onBack={vi.fn()} onOpenArtifact={onOpenArtifact} />);

    await screen.findByRole('heading', { name: 'System Concept Map' });
    await user.click(screen.getByRole('button', { name: button }));
    expect(ipc.canvasOps).toHaveBeenCalledWith({ action: 'CreateNote', payload: expect.objectContaining({ canvas_id: 'canvas-1', placeholder_id: placeholderId }) });
    expect(onOpenArtifact).toHaveBeenCalledWith('created-a', mockCanvasData);
  });
});
