import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { CanvasView } from '../components/CanvasView';
import * as ipc from '../api/ipc';

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
});
