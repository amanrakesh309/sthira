import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { RichDocumentRenderer } from '../components/RichDocumentRenderer';
import { FocusView } from '../components/FocusView';
import * as ipc from '../api/ipc';

// Mock IPC for FocusView integration
vi.mock('../api/ipc', () => ({
  getArtifactContext: vi.fn(),
  getResumeBrief: vi.fn(),
  getProjectHistory: vi.fn(),
  getProviderCatalog: vi.fn(),
  listObjectTypes: vi.fn(),
  saveArtifact: vi.fn(),
  createTypedObject: vi.fn(),
  saveObjectInstance: vi.fn(),
  decideProposal: vi.fn(),
  revertChangeSet: vi.fn(),
  streamAdaptiveRun: vi.fn(),
}));

describe('RichDocumentRenderer & Component-Decorated Markdown Hydration', () => {
  let writeTextSpy: any;

  beforeEach(() => {
    vi.clearAllMocks();
    if (!navigator.clipboard) {
      Object.defineProperty(navigator, 'clipboard', {
        value: { writeText: vi.fn().mockResolvedValue(undefined) },
        writable: true,
        configurable: true,
      });
      writeTextSpy = (navigator.clipboard as any).writeText;
    } else {
      writeTextSpy = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue(undefined);
    }
  });

  // -------------------------------------------------------------------------
  // 1. Interactive Comparison Table Override
  // -------------------------------------------------------------------------
  it('renders interactive comparison table with Warm Stone styling, real-time filter, and copy table button', async () => {
    const tableMarkdown = `# Architecture Comparison

| Engine Architecture | Read Latency | Memory Footprint | Verdict |
| :--- | :--- | :--- | :--- |
| SQLite WAL Engine | < 1ms | 12 MB | Immediate sub-millisecond local response |
| Remote Cloud REST | 185ms | 94 MB | Network latency jitter on cold return |
| In-Memory Cache | 0.2ms | 64 MB | Ephemeral with volatile loss risk |
`;

    render(<RichDocumentRenderer content={tableMarkdown} />);

    // Assert container and header
    const tableContainer = screen.getByTestId('interactive-comparison-table-container');
    expect(tableContainer).toBeInTheDocument();
    expect(screen.getByText('Comparison Matrix')).toBeInTheDocument();

    // Assert table headers and all rows exist
    expect(screen.getByText('Engine Architecture')).toBeInTheDocument();
    expect(screen.getByText('SQLite WAL Engine')).toBeInTheDocument();
    expect(screen.getByText('Remote Cloud REST')).toBeInTheDocument();
    expect(screen.getByText('In-Memory Cache')).toBeInTheDocument();

    // Test real-time filter
    const filterInput = screen.getByPlaceholderText('Filter rows...');
    fireEvent.change(filterInput, { target: { value: 'SQLite' } });

    // SQLite row should be visible, others filtered out
    expect(screen.getByText('SQLite WAL Engine')).toBeInTheDocument();
    expect(screen.queryByText('Remote Cloud REST')).toBeNull();
    expect(screen.queryByText('In-Memory Cache')).toBeNull();

    // Clear filter
    fireEvent.change(filterInput, { target: { value: '' } });
    expect(screen.getByText('Remote Cloud REST')).toBeInTheDocument();

    // Test Copy Table button
    const copyBtn = screen.getByTestId('btn-copy-table');
    expect(copyBtn).toBeInTheDocument();
    fireEvent.click(copyBtn);

    expect(writeTextSpy).toHaveBeenCalled();
    await waitFor(() => {
      expect(screen.getByText('Copied!')).toBeInTheDocument();
    });
  });

  // -------------------------------------------------------------------------
  // 2. Interactive Milestone Checklist Override
  // -------------------------------------------------------------------------
  it('renders interactive task checklist and toggles markdown - [ ] to - [x] on click', async () => {
    const onUpdateContent = vi.fn();
    const checklistMarkdown = `# Project Deployment Plan

## Phase 1: Storage Layer
- [ ] Define invariant SQLite schema migrations
- [x] Implement append-only changeset log
- [ ] Verify two-way vault reconciliation
`;

    render(
      <RichDocumentRenderer
        content={checklistMarkdown}
        onUpdateContent={onUpdateContent}
      />
    );

    // Verify checklist checkboxes
    const checkboxes = screen.getAllByRole('checkbox') as HTMLInputElement[];
    expect(checkboxes.length).toBe(3);
    expect(checkboxes[0].checked).toBe(false);
    expect(checkboxes[1].checked).toBe(true);
    expect(checkboxes[2].checked).toBe(false);

    // Click the first unchecked checkbox
    fireEvent.click(checkboxes[0]);

    // Assert onUpdateContent called with toggled checklist item
    expect(onUpdateContent).toHaveBeenCalledTimes(1);
    const updatedContent = onUpdateContent.mock.calls[0][0];
    expect(updatedContent).toContain('- [x] Define invariant SQLite schema migrations');
  });

  // -------------------------------------------------------------------------
  // 3. Interactive Epistemic Decision Callout Override
  // -------------------------------------------------------------------------
  it('renders decision blockquotes as styled callout cards with epistemic badge and action chip', async () => {
    const onRecordDecision = vi.fn();
    const decisionMarkdown = `# System Architecture Notes

> **Decision:** Use SQLite WAL mode to guarantee zero concurrency locks and sub-millisecond local reads.

Some additional context notes below.
`;

    render(
      <RichDocumentRenderer
        content={decisionMarkdown}
        onRecordDecision={onRecordDecision}
      />
    );

    // Assert decision callout container is rendered
    const callout = screen.getByTestId('interactive-epistemic-decision-callout');
    expect(callout).toBeInTheDocument();

    // Assert epistemic badge and label
    expect(screen.getByText('Recorded')).toBeInTheDocument();
    expect(screen.getByText('Architectural Decision')).toBeInTheDocument();
    expect(screen.getByText(/Use SQLite WAL mode to guarantee zero concurrency locks/)).toBeInTheDocument();

    // Assert action button exists and is clickable
    const actionBtn = screen.getByTestId('btn-add-to-project-decisions');
    expect(actionBtn).toBeInTheDocument();
    fireEvent.click(actionBtn);

    expect(onRecordDecision).toHaveBeenCalledTimes(1);
    expect(onRecordDecision).toHaveBeenCalledWith(expect.stringContaining('Use SQLite WAL mode'));
  });

  // -------------------------------------------------------------------------
  // 4. Embedded Object Directive Override
  // -------------------------------------------------------------------------
  it('renders embedded object directive as interactive EmbeddedObjectCard', () => {
    const objectMarkdown = `# Research Notes

\`\`\`sthira-object
{"artifact_id":"art-decision-42","object_type_id":"decision","title":"Adopt Sub-millisecond Local Engine","properties":{"status":"Recorded","rationale":"Ensures zero round-trip latency"}}
\`\`\`
`;

    const systemDecisionType = {
      id: 'decision',
      name: 'decision',
      display_name: 'Decision',
      icon: '⚖️',
      color: '#10B981',
      is_system: true,
      property_schema: [
        { key: 'status', label: 'Status', property_type: 'select' as const, required: true },
        { key: 'rationale', label: 'Rationale', property_type: 'text' as const },
      ],
      schema: [],
      created_at: 1000,
      updated_at: 1000,
    } as any;

    render(
      <RichDocumentRenderer
        content={objectMarkdown}
        objectTypes={[systemDecisionType]}
      />
    );

    expect(screen.getByTestId('embedded-object-art-decision-42')).toBeInTheDocument();
    expect(screen.getByText('Adopt Sub-millisecond Local Engine')).toBeInTheDocument();
    expect(screen.getByText('Recorded')).toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // 5. FocusView Binary Segmented Control & Cmd/Ctrl + E Shortcut
  // -------------------------------------------------------------------------
  it('FocusView toggles cleanly between Edit and Preview via segmented control and Cmd+E shortcut', async () => {
    vi.mocked(ipc.getArtifactContext).mockResolvedValue({
      artifact: {
        id: 'art-arch-1',
        artifact_type: 'note',
        authorship: 'user',
        title: 'Distributed System Blueprint',
        content_md: `# Distributed System Blueprint\n\n| Module | Status |\n| :--- | :--- |\n| Router | Complete |\n\n- [ ] Deploy locally`,
        source_url: null,
        content_hash: 'hash-1',
        version: 1,
        file_path: null,
        last_synced_hash: null,
        is_unfinished_draft: false,
        deleted_at: null,
        created_at: 1000,
        updated_at: 1000,
      },
      provenance: [],
      pending_proposals: [],
      epistemic_nodes: [],
    } as any);
    vi.mocked(ipc.getResumeBrief).mockResolvedValue({
      project_id: 'proj-dist',
      level: 1,
      text: 'Summary',
      created_at: 1000,
    } as any);
    vi.mocked(ipc.getProjectHistory).mockResolvedValue({
      sessions: [],
      stats: { total_sessions: 0, total_artifacts: 1, total_epistemic_nodes: 0, first_active_at: 1000, last_active_at: 1000 },
      pending_proposals_count: 0,
    } as any);
    vi.mocked(ipc.listObjectTypes).mockResolvedValue([]);
    vi.mocked(ipc.getProviderCatalog).mockResolvedValue([]);

    render(
      <FocusView
        artifactId="art-arch-1"
        projectId="proj-dist"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    // Initial state: Edit mode with textarea
    await waitFor(() => {
      expect(screen.getByTestId('textarea-editor')).toBeInTheDocument();
    });

    // 1. Click Preview mode segmented button
    const previewBtn = screen.getByTestId('btn-toggle-preview-mode');
    fireEvent.click(previewBtn);

    // Assert Preview mode canvas is active, textarea is gone
    expect(screen.queryByTestId('textarea-editor')).toBeNull();
    expect(screen.getByTestId('rich-preview-canvas')).toBeInTheDocument();
    expect(screen.getByTestId('interactive-comparison-table-container')).toBeInTheDocument();

    // 2. Click Edit mode segmented button
    const editBtn = screen.getByTestId('btn-toggle-edit-mode');
    fireEvent.click(editBtn);

    // Assert Edit mode canvas is restored
    expect(screen.getByTestId('textarea-editor')).toBeInTheDocument();
    expect(screen.queryByTestId('rich-preview-canvas')).toBeNull();

    // 3. Test Cmd + E keyboard shortcut
    fireEvent.keyDown(window, { key: 'e', metaKey: true });

    // Should toggle to Preview mode
    await waitFor(() => {
      expect(screen.getByTestId('rich-preview-canvas')).toBeInTheDocument();
      expect(screen.queryByTestId('textarea-editor')).toBeNull();
    });

    // 4. Test Ctrl + E keyboard shortcut (Windows/Linux)
    fireEvent.keyDown(window, { key: 'e', ctrlKey: true });

    // Should toggle back to Edit mode
    await waitFor(() => {
      expect(screen.getByTestId('textarea-editor')).toBeInTheDocument();
      expect(screen.queryByTestId('rich-preview-canvas')).toBeNull();
    });
  });
});
