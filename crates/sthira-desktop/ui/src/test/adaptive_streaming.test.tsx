import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { AdaptiveResponseRenderer } from '../components/AdaptiveResponseRenderer';
import { MilestonePlanDagWidget } from '../components/widgets/MilestonePlanDagWidget';
import { EvidenceComparisonMatrixWidget } from '../components/widgets/EvidenceComparisonMatrixWidget';
import { InteractiveDecisionSelectorWidget } from '../components/widgets/InteractiveDecisionSelectorWidget';
import type { PlanPhase, ComparisonRow, DecisionPoint } from '../types/sthira';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
  Channel: class MockChannel {
    onmessage?: (data: any) => void;
  },
}));

describe('Phase 11: Adaptive Streaming & Generative UI Widget Tests', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation(async (cmd: string, args: any) => {
      if (cmd === 'cmd_create_project') {
        return {
          id: 'proj-123',
          name: args?.name || 'New Project',
          status: 'active',
          created_at: Date.now(),
          last_substantive_at: Date.now(),
        };
      }
      if (cmd === 'cmd_create_page') {
        return {
          id: 'art-decision-1',
          title: args?.title || 'Decision Note',
          artifact_type: 'note',
          authorship: 'user',
          content_md: '',
          content_hash: 'hash-1',
          version: 1,
          created_at: Date.now(),
          updated_at: Date.now(),
          is_unfinished_draft: false,
        };
      }
      if (cmd === 'cmd_save_artifact') {
        return {
          id: args?.id || 'art-decision-1',
          title: args?.title || 'Decision Note',
          content_md: args?.content_md || '',
        };
      }
      if (cmd === 'cmd_revert_change_set') {
        return 'reverted_cs_123';
      }
      if (cmd === 'cmd_canvas_ops') {
        return {
          canvas: { id: 'canvas-123', name: 'Canvas', items: [] },
          success: true,
        };
      }
      return {};
    });
  });

  it('AdaptiveResponseRenderer toggles smoothly between Interactive Widgets and Plain Markdown', async () => {
    const phases: PlanPhase[] = [
      {
        phase_number: 1,
        title: 'Phase 1: Immutable Data Contracts',
        milestones: ['Define SQLite schema', 'Add zero-unsolicited-write invariant'],
        prerequisites: [],
      },
    ];

    const rawText = 'Introductory plan commentary text.\n```widget:plan\n{...}\n```';

    render(
      <AdaptiveResponseRenderer
        rawText={rawText}
        phases={phases}
        status="completed"
        basisFingerprint="e1b2c3d4f5a6b7c8"
        metrics={{ total_tokens: 42, duration_ms: 180, basis_fingerprint: 'e1b2c3d4f5a6b7c8' }}
      />
    );

    // Initial view is Widgets
    expect(screen.getByText(/Progressive Plan DAG/i)).toBeInTheDocument();
    expect(screen.getByText('Phase 1: Immutable Data Contracts')).toBeInTheDocument();
    expect(screen.getByText(/Verified Basis: e1b2c3d4/i)).toBeInTheDocument();

    // Toggle to Plain Markdown mode
    const markdownBtn = screen.getByTitle(/Render formatted plain markdown text/i);
    fireEvent.click(markdownBtn);

    // Assert Plain Markdown container is visible and DAG widget is unmounted
    expect(screen.queryByText(/Progressive Plan DAG/i)).not.toBeInTheDocument();
    expect(screen.getByText(/Introductory plan commentary text/i)).toBeInTheDocument();

    // Toggle back to Widgets mode
    const widgetsBtn = screen.getByTitle(/Render progressive interactive generative UI widgets/i);
    fireEvent.click(widgetsBtn);

    expect(screen.getByText(/Progressive Plan DAG/i)).toBeInTheDocument();
    expect(screen.getByText('Phase 1: Immutable Data Contracts')).toBeInTheDocument();
  });

  it('renders streaming unverified badge while streaming and verified basis when complete', () => {
    const { rerender } = render(
      <AdaptiveResponseRenderer
        rawText="Partial token..."
        status="streaming"
      />
    );

    expect(screen.getByText(/\[Streaming \/ Unverified\]/i)).toBeInTheDocument();

    rerender(
      <AdaptiveResponseRenderer
        rawText="Full token stream finished."
        status="completed"
        basisFingerprint="9988776655443322"
      />
    );

    expect(screen.queryByText(/\[Streaming \/ Unverified\]/i)).not.toBeInTheDocument();
    expect(screen.getByText(/\[Verified Basis: 99887766\]/i)).toBeInTheDocument();
  });

  it('MilestonePlanDagWidget renders milestones checklist and supports 1-click project creation with UndoToast', async () => {
    const phases: PlanPhase[] = [
      {
        phase_number: 1,
        title: 'Phase 1: Adaptive Parser Implementation',
        milestones: ['Build buffer tokenizer', 'Test fail-safe JSON parser'],
        prerequisites: [],
      },
    ];

    const onProjectCreated = vi.fn();

    render(
      <MilestonePlanDagWidget
        phases={phases}
        onProjectCreated={onProjectCreated}
      />
    );

    expect(screen.getByText('Phase 1: Adaptive Parser Implementation')).toBeInTheDocument();
    const milestoneItem = screen.getByText('Build buffer tokenizer');
    expect(milestoneItem).toBeInTheDocument();

    // Click milestone to toggle checkbox
    fireEvent.click(milestoneItem);
    expect(milestoneItem).toHaveClass('line-through');

    // Click [Create as Project]
    const createBtn = screen.getByRole('button', { name: /Create as Project/i });
    fireEvent.click(createBtn);

    await waitFor(() => {
      expect(onProjectCreated).toHaveBeenCalledWith('proj-123', 'cs_project_proj-123');
    });
  });

  it('EvidenceComparisonMatrixWidget renders side-by-side comparison with Warm Stone provenance tags', () => {
    const rows: ComparisonRow[] = [
      {
        criteria: 'IPC Latency',
        aspect_a: 'Tauri v2 typed channel (<0.5ms)',
        aspect_b: 'WebSocket loopback (3.5ms)',
        verdict: 'Tauri channel eliminates TCP handshake latency',
      },
    ];

    render(<EvidenceComparisonMatrixWidget rows={rows} />);

    expect(screen.getByText('IPC Latency')).toBeInTheDocument();
    expect(screen.getByText('Tauri v2 typed channel (<0.5ms)')).toBeInTheDocument();
    expect(screen.getByText('WebSocket loopback (3.5ms)')).toBeInTheDocument();
    expect(screen.getByText('Tauri channel eliminates TCP handshake latency')).toBeInTheDocument();
    expect(screen.getByText('Recorded')).toBeInTheDocument();
    expect(screen.getByText('Source')).toBeInTheDocument();
  });

  it('InteractiveDecisionSelectorWidget records chosen option as a note', async () => {
    const decisions: DecisionPoint[] = [
      {
        id: 'db_mode',
        question: 'Which SQLite journal mode should be configured?',
        options: ['WAL Mode (Write-Ahead Logging)', 'Rollback Journal (DELETE)'],
        recommended: 'WAL Mode (Write-Ahead Logging)',
      },
    ];

    const onDecisionRecorded = vi.fn();

    render(
      <InteractiveDecisionSelectorWidget
        decisions={decisions}
        onDecisionRecorded={onDecisionRecorded}
      />
    );

    expect(screen.getByText('Which SQLite journal mode should be configured?')).toBeInTheDocument();
    expect(screen.getByText('Recommended')).toBeInTheDocument();

    const walOption = screen.getByRole('button', { name: /WAL Mode/i });
    fireEvent.click(walOption);

    await waitFor(() => {
      expect(onDecisionRecorded).toHaveBeenCalledWith(
        'db_mode',
        'WAL Mode (Write-Ahead Logging)',
        'art-decision-1'
      );
    });

    expect(screen.getByText(/Choice recorded as note/i)).toBeInTheDocument();
  });

  it('AdaptiveResponseRenderer displays fallback notice badge when malformed JSON was encountered', () => {
    render(
      <AdaptiveResponseRenderer
        rawText="```widget:plan { malformed json } ```"
        fallbackNotices={['Plan JSON parse error: expected ident at line 1']}
      />
    );

    expect(screen.getByText('Degraded to Markdown')).toBeInTheDocument();
  });
});
