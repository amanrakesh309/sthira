import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { StrictMode, useState } from 'react';
import ObjectTypeBuilderModal from '../components/objects/ObjectTypeBuilderModal';
import { FocusView } from '../components/FocusView';
import { App } from '../App';
import { editorMarkdown, setEditorMarkdown } from './editor-helpers';

const mockInvoke = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}));

describe('Wave B UI Regressions (B3 Dialogs, Tokens & Shortcuts)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    sessionStorage.clear();
    mockInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'cmd_get_home_state':
          return {
            projects: [],
            recent_items: [],
            loose_captures: [],
          };
        case 'cmd_list_object_types':
          return [];
        case 'cmd_get_provider_catalog':
          return [];
        case 'cmd_get_search_providers':
          return [];
        case 'cmd_get_vault_session_key':
          return 'wave-b-session';
        case 'cmd_get_workspace_tree':
          return { projects: [], orphaned_artifacts: [] };
        case 'cmd_filter_existing_artifacts':
          return [];
        case 'cmd_list_scheduled_topics':
          return [];
        case 'cmd_search_all':
          return [];
        default:
          return null;
      }
    });
  });

  it('ObjectTypeBuilderModal enforces modal dialog semantics, theme tokens, select contrast, Tab trap, and Escape focus restoration (F-B5-020, F-B5-021)', async () => {
    const user = userEvent.setup();

    function BuilderHarness() {
      const [open, setOpen] = useState(false);
      return (
        <div className="sthira-shell dark" data-theme="dark">
          <button
            type="button"
            data-testid="outside-builder-opener"
            onClick={() => setOpen(true)}
          >
            New Object Type
          </button>
          <ObjectTypeBuilderModal
            isOpen={open}
            onClose={() => setOpen(false)}
            onTypeCreated={vi.fn()}
          />
          <button type="button" data-testid="outside-trailing">Trailing</button>
        </div>
      );
    }

    render(<BuilderHarness />);

    const opener = screen.getByTestId('outside-builder-opener');
    opener.focus();
    await user.click(opener);

    const dialog = await screen.findByTestId('object-type-builder-modal');
    expect(dialog).toHaveAttribute('role', 'dialog');
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    expect(dialog).toHaveAttribute('aria-labelledby', 'object-type-builder-title');
    expect(dialog).toHaveAccessibleName('New Object Type');
    expect(dialog.className).toContain('bg-[var(--sthira-surface)]');
    expect(dialog.className).toContain('text-[var(--sthira-text)]');
    expect(dialog.className).not.toContain('bg-[#FFFFFF]');

    // Initial focus goes into display name input inside modal
    const nameInput = screen.getByTestId('input-object-type-name');
    expect(nameInput).toHaveFocus();

    // Accessible close button label
    const closeBtn = screen.getByTestId('btn-close-object-type-builder');
    expect(closeBtn).toHaveAccessibleName('Close object type builder');

    // <select> and <option> use theme tokens + inherited colorScheme for dark-mode readability
    const typeSelect = screen.getByTestId('select-property-type') as HTMLSelectElement;
    expect(typeSelect.className).toContain('bg-[var(--sthira-surface)]');
    expect(typeSelect.className).toContain('text-[var(--sthira-text)]');
    expect(typeSelect.style.colorScheme).toBe('inherit');
    const options = Array.from(typeSelect.querySelectorAll('option'));
    expect(options.length).toBeGreaterThan(0);
    for (const opt of options) {
      expect(opt.className).toContain('bg-[var(--sthira-surface)]');
      expect(opt.className).toContain('text-[var(--sthira-text)]');
    }

    // Tab wraps cleanly between last control and first control without escaping to outside buttons
    const submitBtn = screen.getByTestId('btn-submit-object-type');
    submitBtn.focus();
    await user.tab();
    expect(closeBtn).toHaveFocus();
    await user.tab({ shift: true });
    expect(submitBtn).toHaveFocus();

    // Escape closes modal and restores focus to opener
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByTestId('object-type-builder-modal')).not.toBeInTheDocument();
    });
    expect(opener).toHaveFocus();
  });

  it('App suppresses Ctrl+K when another modal dialog is open and restores Search shortcut once closed (F-B5-018)', async () => {
    const user = userEvent.setup();
    render(<App />);

    const newTypeBtn = await screen.findByTestId('btn-new-custom-object-type');
    await user.click(newTypeBtn);

    expect(await screen.findByTestId('object-type-builder-modal')).toBeInTheDocument();

    // Pressing Ctrl+K while ObjectTypeBuilderModal owns focus must NOT open SearchModal on top
    await user.keyboard('{Control>}k{/Control}');
    expect(screen.queryByTestId('search-workspace-dialog')).not.toBeInTheDocument();
    expect(screen.getByTestId('object-type-builder-modal')).toBeInTheDocument();

    // Dismiss ObjectTypeBuilderModal via Escape
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByTestId('object-type-builder-modal')).not.toBeInTheDocument();
    });

    // Now Ctrl+K opens SearchModal cleanly, and Escape dismisses it
    await user.keyboard('{Control>}k{/Control}');
    expect(await screen.findByTestId('search-workspace-dialog')).toBeInTheDocument();
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByTestId('search-workspace-dialog')).not.toBeInTheDocument();
    });
  });

  it('FocusView mounts UndoToast in workspace slot inside .focus-document-column and isolates role=alert banners (F-B5-013, F-B5-014)', async () => {
    const user = userEvent.setup();
    let conversationState = {
      mode: 'document' as const,
      initial_prompt: '',
      model_id: 'm1',
      model_name: 'Model 1',
      turns: [
        {
          id: 'turn-1',
          user_text: 'Expand intro',
          reply: 'Added details.',
          error: null,
          basis_content_md: '# Doc Title\n\nOriginal paragraph.',
          proposed_content_md: '# Doc Title\n\nExpanded paragraph.',
          proposal_state: 'pending' as 'pending' | 'applied' | 'dismissed' | 'reverted',
          change_set_id: null as string | null,
        },
      ],
    };
    let failSave = false;

    mockInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'cmd_get_artifact_context':
          return {
            artifact: {
              id: 'art-b5',
              artifact_type: 'note',
              authorship: 'user',
              title: 'Doc Title',
              content_md: '# Doc Title\n\nOriginal paragraph.',
              content_hash: 'h1',
              version: 1,
              is_unfinished_draft: false,
              created_at: 1,
              updated_at: 1,
            },
            epistemic_nodes: [],
            pending_proposals: [],
            project_id: null,
            is_unfinished_draft: false,
          };
        case 'cmd_get_document_conversation':
          return conversationState;
        case 'cmd_get_provider_catalog':
          return [
            {
              id: 'prov-1',
              display_name: 'Local',
              provider_type: 'ollama',
              base_url: 'http://127.0.0.1:11434',
              has_api_key: false,
              is_enabled: true,
              models: [
                {
                  id: 'm1',
                  provider_id: 'prov-1',
                  model_identifier: 'llama3.2:latest',
                  display_name: 'Model 1',
                  context_window: 8192,
                  supports_structured_output: true,
                  is_enabled: true,
                  is_default: true,
                },
              ],
            },
          ];
        case 'cmd_list_object_types':
          return [];
        case 'cmd_get_object_instance':
          return null;
        case 'cmd_decide_document_edit':
          conversationState = {
            ...conversationState,
            turns: [{ ...conversationState.turns[0], proposal_state: 'applied', change_set_id: 'cs-b5-1' }],
          };
          return {
            id: 'art-b5',
            artifact_type: 'note',
            authorship: 'user',
            title: 'Doc Title',
            content_md: '# Doc Title\n\nExpanded paragraph.',
            content_hash: 'h2',
            version: 2,
            is_unfinished_draft: false,
            created_at: 1,
            updated_at: 2,
          };
        case 'cmd_save_artifact':
          if (failSave) throw new Error('disk write refused');
          return {
            artifact: {
              id: 'art-b5',
              artifact_type: 'note',
              authorship: 'user',
              title: 'Doc Title',
              content_md: '# Doc Title\n\nNewer dirty paragraph.',
              content_hash: 'h3',
              version: 3,
              is_unfinished_draft: false,
              created_at: 1,
              updated_at: 3,
            },
            mirror_synced: true,
            mirror_sync_error: null,
            unchanged: false,
            repaired_mirror_only: false,
          };
        default:
          return null;
      }
    });

    const { unmount } = render(<FocusView artifactId="art-b5" onBack={vi.fn()} onOpenCanvas={vi.fn()} />);
    const editor = await screen.findByRole('textbox', { name: 'Document content' });

    await user.click(await screen.findByRole('button', { name: 'Conversation' }));
    await user.click(await screen.findByRole('button', { name: 'Review suggested edit' }));
    await user.click(screen.getByRole('button', { name: 'Apply edit' }));

    const toast = await screen.findByTestId('undo-toast');
    expect(toast).toHaveAttribute('data-placement', 'workspace');
    expect(toast.className).toContain('focus-undo-slot');
    expect(toast.className).toContain('flex-wrap');
    expect(toast.className).not.toContain('fixed bottom-6 right-6');
    expect(toast.closest('.focus-document-column')).not.toBeNull();
    expect(toast.closest('#document-chat')).toBeNull();
    const toastStatus = within(toast).getByRole('status');
    expect(toastStatus).toHaveTextContent('Conversation edit applied');
    expect(toastStatus.querySelector('button')).toBeNull();
    const toastUndoBtn = within(toast).getByTestId('undo-toast-btn');
    const toastCloseBtn = within(toast).getByRole('button', { name: 'Close' });
    expect(toastUndoBtn.parentElement).toBe(toastCloseBtn.parentElement);

    // Dirty the editor before clicking Undo -> isolated alert must expose text without button controls
    setEditorMarkdown(editor, '# Doc Title\n\nNewer dirty paragraph.');
    await user.click(screen.getByTestId('undo-toast-btn'));

    const actionAlert = await screen.findByRole('alert');
    expect(actionAlert).toHaveTextContent('Save or resolve your newer changes before undoing this edit.');
    expect(actionAlert.querySelector('button')).toBeNull();
    expect(actionAlert.tagName).toBe('P');
    expect(screen.getByRole('button', { name: 'Dismiss' })).toBeInTheDocument();

    // Also verify manual save error banner isolates alert text from Retry save button
    failSave = true;
    await user.click(screen.getByTestId('btn-save-artifact'));
    await waitFor(() => {
      const alerts = screen.getAllByRole('alert');
      const saveAlert = alerts.find((el) => el.textContent?.includes('Couldn’t save your changes'));
      expect(saveAlert).toBeDefined();
      expect(saveAlert?.querySelector('button')).toBeNull();
    });

    unmount();

    // Also verify document load unavailable alert excludes buttons and SVG icons
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'cmd_get_artifact_context') {
        throw new Error('Artifact not found in database');
      }
      return null;
    });
    render(<FocusView artifactId="missing-doc" onBack={vi.fn()} onOpenCanvas={vi.fn()} />);
    const unavailableAlert = await screen.findByRole('alert');
    expect(unavailableAlert).toHaveTextContent('Document unavailable');
    expect(unavailableAlert.querySelector('button')).toBeNull();
    expect(unavailableAlert.querySelector('svg')).toBeNull();
  });
});

describe('Corrective Wave B Follow-up UI Regressions (WB-R3 & WB-R4)', () => {
  const CATALOG = [
    {
      id: 'prov-1',
      display_name: 'Local',
      provider_type: 'ollama',
      base_url: 'http://127.0.0.1:11434',
      has_api_key: false,
      is_enabled: true,
      models: [
        {
          id: 'm1',
          provider_id: 'prov-1',
          model_identifier: 'llama3.2:latest',
          display_name: 'Model 1',
          context_window: 8192,
          supports_structured_output: true,
          is_enabled: true,
          is_default: true,
        },
      ],
    },
  ];

  function deferred<T>() {
    let resolve!: (value: T) => void;
    let reject!: (reason?: unknown) => void;
    const promise = new Promise<T>((res, rej) => {
      resolve = res;
      reject = rej;
    });
    return { promise, resolve, reject };
  }

  beforeEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
    localStorage.clear();
    sessionStorage.clear();
  });

  it('WB-R3: releases busy lock and reconciles restored document when same-artifact reload overlaps deferred Undo success, failure, and conversation Undo', async () => {
    const user = userEvent.setup();
    const originalMd = '# Recovery Spec\n\nOriginal verified paragraph.';
    const refinedMd = '# Recovery Spec\n\nRefined verified paragraph.';
    const convAppliedMd = '# Recovery Spec\n\nConversation edited paragraph.';
    let dbState = {
      id: 'art-wbr3',
      artifact_type: 'note',
      authorship: 'user',
      title: 'Recovery Spec',
      content_md: originalMd,
      content_hash: 'h-orig',
      version: 1,
      is_unfinished_draft: false,
      created_at: 1,
      updated_at: 1,
      file_path: 'Notes/recovery-spec.md',
      last_synced_hash: 'h-orig',
    };
    let conversationState = {
      mode: 'document' as const,
      initial_prompt: '',
      model_id: 'm1',
      model_name: 'Model 1',
      turns: [
        {
          id: 'turn-conv-1',
          user_text: 'Tighten wording',
          reply: 'Proposed edit.',
          error: null,
          basis_content_md: originalMd,
          proposed_content_md: convAppliedMd,
          proposal_state: 'pending' as 'pending' | 'applied' | 'dismissed' | 'reverted',
          change_set_id: null as string | null,
        },
      ],
    };

    let undoRefineDef = deferred<unknown>();
    let staleReloadContextDef: ReturnType<typeof deferred<unknown>> | null = null;
    let undoConvDef = deferred<unknown>();

    mockInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'cmd_get_artifact_context':
          if (staleReloadContextDef) {
            const p = staleReloadContextDef.promise;
            staleReloadContextDef = null;
            return p;
          }
          return {
            artifact: { ...dbState },
            epistemic_nodes: [],
            pending_proposals: [],
            project_id: 'proj-1',
            is_unfinished_draft: false,
          };
        case 'cmd_get_project_history':
          return { project_id: 'proj-1', items: [] };
        case 'cmd_get_document_conversation':
          return conversationState;
        case 'cmd_get_provider_catalog':
          return CATALOG;
        case 'cmd_list_object_types':
          return [];
        case 'cmd_get_object_instance':
          return null;
        case 'cmd_get_vault_session_key':
          return 'wbr3-session';
        case 'cmd_refine_artifact':
          dbState = {
            ...dbState,
            content_md: refinedMd,
            content_hash: 'h-refined',
            version: 2,
            last_synced_hash: 'h-refined',
          };
          return {
            artifact_id: 'art-wbr3',
            content_md: refinedMd,
            content_hash: 'h-refined',
            version: 2,
            change_set_id: 'cs-ref-1',
          };
        case 'cmd_undo_refinement':
          return undoRefineDef.promise;
        case 'cmd_decide_document_edit':
          conversationState = {
            ...conversationState,
            turns: [{ ...conversationState.turns[0], proposal_state: 'applied', change_set_id: 'cs-conv-1' }],
          };
          dbState = {
            ...dbState,
            content_md: convAppliedMd,
            content_hash: 'h-conv',
            version: 4,
            last_synced_hash: 'h-conv',
          };
          return { ...dbState };
        case 'cmd_undo_document_edit':
          return undoConvDef.promise;
        case 'cmd_save_artifact':
          return {
            artifact: { ...dbState },
            mirror_synced: true,
            mirror_sync_error: null,
            unchanged: true,
            repaired_mirror_only: false,
          };
        default:
          return null;
      }
    });

    let leaveGuard: (() => Promise<boolean>) | null = null;
    const props = {
      projectId: 'proj-1',
      artifactId: 'art-wbr3',
      onBack: vi.fn(),
      onOpenCanvas: vi.fn(),
      registerBeforeLeave: (guard: (() => Promise<boolean>) | null) => {
        leaveGuard = guard;
      },
    };
    const view = render(<FocusView {...props} />);
    await waitFor(() => expect(screen.getByTestId('input-artifact-title')).toHaveValue('Recovery Spec'));

    // Case 1: Same-artifact reload during failed refinement Undo -> unlocks cleanly & preserves toast
    await user.click(screen.getByTestId('btn-refine-artifact'));
    await user.type(screen.getByTestId('textarea-critique-prompt'), 'Refine spec');
    await user.click(screen.getByTestId('btn-run-refinement'));
    await screen.findByTestId('undo-toast');

    await user.click(screen.getByTestId('undo-toast-btn'));
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: 'Document content' })).toHaveAttribute('contenteditable', 'false')
    );

    // Reload same artifact with stale v2 context arriving before failure
    view.rerender(
      <FocusView
        {...props}
        documentRun={{
          artifactId: 'art-wbr3',
          refreshToken: 1,
          status: 'done',
          kind: 'research',
          message: 'Refresh 1',
          query: 'spec',
          streamedMarkdown: '',
        }}
      />
    );
    await waitFor(() =>
      expect(mockInvoke.mock.calls.filter(([c]) => c === 'cmd_get_artifact_context').length).toBeGreaterThanOrEqual(2)
    );

    await act(async () => {
      undoRefineDef.reject('Temporary lock busy');
    });
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: 'Document content' })).toHaveAttribute('contenteditable', 'true')
    );
    expect(screen.getByTestId('btn-save-artifact')).toBeEnabled();
    expect(screen.getByTestId('undo-toast')).toBeInTheDocument();
    expect(await screen.findByText(/Temporary lock busy/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Dismiss' }));

    // Case 2: Same-artifact reload during successful refinement Undo where stale context resolves AFTER Undo!
    undoRefineDef = deferred<unknown>();
    const releaseStale = deferred<unknown>();
    staleReloadContextDef = releaseStale;
    await user.click(screen.getByTestId('undo-toast-btn'));
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: 'Document content' })).toHaveAttribute('contenteditable', 'false')
    );

    // Trigger same-artifact reload whose getArtifactContext stalls on stale v2 snapshot
    view.rerender(
      <FocusView
        {...props}
        documentRun={{
          artifactId: 'art-wbr3',
          refreshToken: 2,
          status: 'done',
          kind: 'research',
          message: 'Refresh 2',
          query: 'spec',
          streamedMarkdown: '',
        }}
      />
    );

    // Commit Undo v3 BEFORE stale reload context resolves
    const staleSnapshotCtx = {
      artifact: { ...dbState },
      epistemic_nodes: [],
      pending_proposals: [],
      project_id: 'proj-1',
      is_unfinished_draft: false,
    };
    dbState = {
      ...dbState,
      content_md: originalMd,
      content_hash: 'h-orig',
      version: 3,
      last_synced_hash: 'h-orig',
    };
    await act(async () => {
      undoRefineDef.resolve({
        artifact: { ...dbState },
        mirror_synced: true,
        mirror_sync_error: null,
        unchanged: false,
        repaired_mirror_only: false,
      });
    });

    await waitFor(() => {
      expect(screen.queryByTestId('undo-toast')).not.toBeInTheDocument();
    });
    // Now resolve the stale v2 getArtifactContext -> loader finishes and must NOT clobber restored v3 body!
    await act(async () => {
      releaseStale.resolve(staleSnapshotCtx);
    });
    await waitFor(() => {
      expect(screen.getByRole('textbox', { name: 'Document content' })).toHaveAttribute('contenteditable', 'true');
      expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
        'Original verified paragraph.'
      );
    });
    expect(screen.getByTestId('btn-save-artifact')).toBeEnabled();
    expect(await leaveGuard!()).toBe(true);

    // Case 3: Conversation edit Undo + same-artifact reload
    await user.click(screen.getByRole('button', { name: 'Conversation' }));
    await user.click(await screen.findByRole('button', { name: 'Review suggested edit' }));
    await user.click(screen.getByRole('button', { name: 'Apply edit' }));
    await screen.findByTestId('undo-toast');

    undoConvDef = deferred<unknown>();
    await user.click(screen.getByTestId('undo-toast-btn'));
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: 'Document content' })).toHaveAttribute('contenteditable', 'false')
    );

    view.rerender(
      <FocusView
        {...props}
        documentRun={{
          artifactId: 'art-wbr3',
          refreshToken: 3,
          status: 'done',
          kind: 'research',
          message: 'Refresh 3',
          query: 'spec',
          streamedMarkdown: '',
        }}
      />
    );

    dbState = {
      ...dbState,
      content_md: originalMd,
      content_hash: 'h-orig',
      version: 5,
      last_synced_hash: 'h-orig',
    };
    conversationState = {
      ...conversationState,
      turns: [{ ...conversationState.turns[0], proposal_state: 'reverted', change_set_id: 'cs-conv-1' }],
    };
    await act(async () => {
      undoConvDef.resolve({ ...dbState });
    });
    await waitFor(() => {
      expect(screen.getByRole('textbox', { name: 'Document content' })).toHaveAttribute('contenteditable', 'true');
      expect(screen.queryByTestId('undo-toast')).not.toBeInTheDocument();
      expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
        'Original verified paragraph.'
      );
    });
  });

  it('WB-R3: isolates cross-document navigation, overlapping newer locks, and mirror-warning Undo outcomes', async () => {
    const user = userEvent.setup();
    const doc1Orig = '# Alpha\n\nAlpha original.';
    const doc1Ref = '# Alpha\n\nAlpha refined.';
    const doc2Orig = '# Beta\n\nBeta original.';
    const doc2Edited = '# Beta\n\nBeta conversation edit.';

    let art1 = {
      id: 'art-alpha',
      artifact_type: 'note',
      authorship: 'user',
      title: 'Alpha',
      content_md: doc1Orig,
      content_hash: 'a1',
      version: 1,
      is_unfinished_draft: false,
      created_at: 1,
      updated_at: 1,
      file_path: 'Notes/alpha.md',
      last_synced_hash: 'a1',
    };
    const art2 = {
      id: 'art-beta',
      artifact_type: 'note',
      authorship: 'user',
      title: 'Beta',
      content_md: doc2Orig,
      content_hash: 'b1',
      version: 1,
      is_unfinished_draft: false,
      created_at: 1,
      updated_at: 1,
      file_path: 'Notes/beta.md',
      last_synced_hash: 'b1',
    };
    const betaChat = {
      mode: 'document' as const,
      initial_prompt: '',
      model_id: 'm1',
      model_name: 'Model 1',
      turns: [
        {
          id: 'beta-turn-1',
          user_text: 'Improve Beta',
          reply: 'Suggestion ready.',
          error: null,
          basis_content_md: doc2Orig,
          proposed_content_md: doc2Edited,
          proposal_state: 'pending' as 'pending' | 'applied' | 'dismissed' | 'reverted',
          change_set_id: null as string | null,
        },
      ],
    };

    let deferredAlphaUndo = deferred<unknown>();
    let deferredBetaApply = deferred<unknown>();
    let failMirrorNext = false;

    mockInvoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      switch (cmd) {
        case 'cmd_get_artifact_context': {
          const id = (args?.artifactId ?? args?.artifact_id) as string;
          return {
            artifact: id === 'art-beta' ? { ...art2 } : { ...art1 },
            epistemic_nodes: [],
            pending_proposals: [],
            project_id: 'proj-1',
            is_unfinished_draft: false,
          };
        }
        case 'cmd_get_project_history':
          return { project_id: 'proj-1', items: [] };
        case 'cmd_get_document_conversation': {
          const id = (args?.artifactId ?? args?.artifact_id) as string;
          return id === 'art-beta'
            ? betaChat
            : { mode: 'document', initial_prompt: '', model_id: 'm1', model_name: 'Model 1', turns: [] };
        }
        case 'cmd_get_provider_catalog':
          return CATALOG;
        case 'cmd_list_object_types':
          return [];
        case 'cmd_get_object_instance':
          return null;
        case 'cmd_get_vault_session_key':
          return 'wbr3-iso-session';
        case 'cmd_refine_artifact':
          art1 = { ...art1, content_md: doc1Ref, content_hash: 'a2', version: 2, last_synced_hash: 'a2' };
          return {
            artifact_id: 'art-alpha',
            content_md: doc1Ref,
            content_hash: 'a2',
            version: 2,
            change_set_id: 'cs-alpha-1',
          };
        case 'cmd_undo_refinement':
          return deferredAlphaUndo.promise;
        case 'cmd_decide_document_edit':
          return deferredBetaApply.promise;
        case 'cmd_save_artifact': {
          const id = (args?.id ?? args?.artifactId ?? args?.artifact_id) as string;
          const targetArt = id === 'art-beta' ? art2 : art1;
          if (failMirrorNext) {
            return {
              artifact: { ...targetArt, last_synced_hash: null },
              mirror_synced: false,
              mirror_sync_error: 'Permission denied',
              unchanged: false,
              repaired_mirror_only: false,
            };
          }
          return {
            artifact: { ...targetArt, last_synced_hash: targetArt.content_hash },
            mirror_synced: true,
            mirror_sync_error: null,
            unchanged: false,
            repaired_mirror_only: true,
          };
        }
        default:
          return null;
      }
    });

    const view = render(
      <FocusView projectId="proj-1" artifactId="art-alpha" onBack={vi.fn()} onOpenCanvas={vi.fn()} />
    );
    await waitFor(() => expect(screen.getByTestId('input-artifact-title')).toHaveValue('Alpha'));

    // Start Undo on Alpha, then navigate to Beta mid-flight
    await user.click(screen.getByTestId('btn-refine-artifact'));
    await user.type(screen.getByTestId('textarea-critique-prompt'), 'Refine Alpha');
    await user.click(screen.getByTestId('btn-run-refinement'));
    await screen.findByTestId('undo-toast');
    await user.click(screen.getByTestId('undo-toast-btn'));
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: 'Document content' })).toHaveAttribute('contenteditable', 'false')
    );

    // Cross-doc nav -> Beta must immediately be unlocked
    view.rerender(<FocusView projectId="proj-1" artifactId="art-beta" onBack={vi.fn()} onOpenCanvas={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByTestId('input-artifact-title')).toHaveValue('Beta');
      expect(screen.getByRole('textbox', { name: 'Document content' })).toHaveAttribute('contenteditable', 'true');
    });

    // Acquire newer apply lock on Beta via DocumentChatPanel while Alpha's Undo is STILL in flight
    await user.click(screen.getByRole('button', { name: 'Conversation' }));
    await user.click(await screen.findByRole('button', { name: 'Review suggested edit' }));
    await user.click(screen.getByRole('button', { name: 'Apply edit' }));
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: 'Document content' })).toHaveAttribute('contenteditable', 'false')
    );

    // Late Alpha Undo settles -> MUST NOT unlock Beta's active lock or mutate Beta's title/body
    await act(async () => {
      deferredAlphaUndo.resolve({
        artifact: { ...art1, content_md: doc1Orig, content_hash: 'a1', version: 3 },
        mirror_synced: true,
        mirror_sync_error: null,
        unchanged: false,
        repaired_mirror_only: false,
      });
    });
    expect(screen.getByTestId('input-artifact-title')).toHaveValue('Beta');
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain('Beta original.');
    expect(screen.getByRole('textbox', { name: 'Document content' })).toHaveAttribute('contenteditable', 'false');

    await act(async () => {
      deferredBetaApply.resolve({ ...art2, content_md: doc2Edited, content_hash: 'b2', version: 2 });
    });
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: 'Document content' })).toHaveAttribute('contenteditable', 'true')
    );

    // Mirror-warning Undo outcome on Alpha -> surfaces Retry mirror sync banner & removes undo-toast
    view.rerender(<FocusView projectId="proj-1" artifactId="art-alpha" onBack={vi.fn()} onOpenCanvas={vi.fn()} />);
    await waitFor(() => expect(screen.getByTestId('input-artifact-title')).toHaveValue('Alpha'));
    deferredAlphaUndo = deferred<unknown>();
    await user.click(screen.getByTestId('btn-refine-artifact'));
    await user.type(screen.getByTestId('textarea-critique-prompt'), 'Refine Alpha 2');
    await user.click(screen.getByTestId('btn-run-refinement'));
    await screen.findByTestId('undo-toast');
    await user.click(screen.getByTestId('undo-toast-btn'));

    art1 = { ...art1, content_md: doc1Orig, content_hash: 'a1', version: 4, last_synced_hash: 'stale-hash' };
    await act(async () => {
      deferredAlphaUndo.resolve({
        artifact: { ...art1 },
        mirror_synced: false,
        mirror_sync_error: 'EACCES: read-only mirror',
        unchanged: false,
        repaired_mirror_only: false,
      });
    });

    const mirrorBanner = await screen.findByTestId('banner-mirror-warning');
    expect(mirrorBanner).toHaveTextContent('EACCES: read-only mirror');
    expect(screen.getByTestId('btn-retry-mirror-sync')).toBeInTheDocument();
    expect(screen.queryByTestId('undo-toast')).not.toBeInTheDocument();

    failMirrorNext = false;
    await user.click(screen.getByTestId('btn-retry-mirror-sync'));
    await waitFor(() => expect(screen.queryByTestId('banner-mirror-warning')).not.toBeInTheDocument());
  });

  it('WB-R4: prevents post-unmount, cross-document, vault-reset, failed, mirror-warning, and overlapping save feedback/timers while keeping durable backend save', async () => {
    const baseArt = {
      id: 'art-save-1',
      artifact_type: 'note',
      authorship: 'user',
      title: 'Save Lifecycle',
      content_md: '# Save Lifecycle\n\nInitial text.',
      content_hash: 's1',
      version: 1,
      is_unfinished_draft: false,
      created_at: 1,
      updated_at: 1,
    };
    const art2 = {
      ...baseArt,
      id: 'art-save-2',
      title: 'Second Doc',
      content_md: '# Second Doc\n\nSecond doc text.',
    };

    let saveDef = deferred<unknown>();
    let refreshCtxDef: ReturnType<typeof deferred<unknown>> | null = null;

    const installMock = () => {
      mockInvoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
        switch (cmd) {
          case 'cmd_get_artifact_context': {
            const id = (args?.artifactId ?? args?.artifact_id) as string;
            if (refreshCtxDef) {
              const p = refreshCtxDef.promise;
              refreshCtxDef = null;
              return p;
            }
            return {
              artifact: id === 'art-save-2' ? art2 : baseArt,
              epistemic_nodes: [],
              pending_proposals: [],
              project_id: 'proj-1',
              is_unfinished_draft: false,
            };
          }
          case 'cmd_get_project_history':
            return { project_id: 'proj-1', items: [] };
          case 'cmd_get_document_conversation':
            return null;
          case 'cmd_get_provider_catalog':
            return CATALOG;
          case 'cmd_list_object_types':
            return [];
          case 'cmd_get_object_instance':
            return null;
          case 'cmd_save_artifact':
            return saveDef.promise;
          default:
            return null;
        }
      });
    };

    installMock();
    const timeoutSpy = vi.spyOn(globalThis, 'setTimeout');

    // 1. Late save settling after unmount + StrictMode compatibility
    const onSave1 = vi.fn();
    const view1 = render(
      <StrictMode>
        <FocusView projectId="proj-1" artifactId="art-save-1" onBack={vi.fn()} onOpenCanvas={vi.fn()} onSave={onSave1} />
      </StrictMode>
    );
    await waitFor(() => expect(screen.getByTestId('btn-save-artifact')).toBeEnabled());
    fireEvent.click(screen.getByTestId('btn-save-artifact'));
    await waitFor(() =>
      expect(mockInvoke.mock.calls.filter(([c]) => c === 'cmd_save_artifact').length).toBeGreaterThanOrEqual(1)
    );
    view1.unmount();

    const timersBeforeLateSettle = timeoutSpy.mock.calls.filter((a) => a[1] === 2500).length;
    await act(async () => {
      saveDef.resolve({
        artifact: { ...baseArt, version: 2 },
        mirror_synced: true,
        mirror_sync_error: null,
        unchanged: false,
        repaired_mirror_only: false,
      });
    });
    expect(onSave1).not.toHaveBeenCalled();
    expect(timeoutSpy.mock.calls.filter((a) => a[1] === 2500).length).toBe(timersBeforeLateSettle);

    // 2. Unmount during post-save getArtifactContext refresh (both resolve and reject paths)
    for (const rejectRefresh of [false, true]) {
      saveDef = deferred<unknown>();
      refreshCtxDef = null;
      const onSaveRefresh = vi.fn();
      const v = render(
        <FocusView
          projectId="proj-1"
          artifactId="art-save-1"
          onBack={vi.fn()}
          onOpenCanvas={vi.fn()}
          onSave={onSaveRefresh}
        />
      );
      await waitFor(() => expect(screen.getByTestId('btn-save-artifact')).toBeEnabled());
      const pendingRefresh = deferred<unknown>();
      refreshCtxDef = pendingRefresh;
      fireEvent.click(screen.getByTestId('btn-save-artifact'));
      await waitFor(() =>
        expect(mockInvoke.mock.calls.some(([c]) => c === 'cmd_save_artifact')).toBe(true)
      );
      await act(async () => {
        saveDef.resolve({
          artifact: { ...baseArt, version: 2 },
          mirror_synced: true,
          mirror_sync_error: null,
          unchanged: false,
          repaired_mirror_only: false,
        });
      });
      v.unmount();
      const tBefore = timeoutSpy.mock.calls.filter((a) => a[1] === 2500).length;
      await act(async () => {
        if (rejectRefresh) pendingRefresh.reject(new Error('late refresh fail'));
        else pendingRefresh.resolve({ artifact: { ...baseArt, version: 2 }, epistemic_nodes: [], pending_proposals: [], project_id: 'proj-1' });
      });
      expect(onSaveRefresh).not.toHaveBeenCalled();
      expect(timeoutSpy.mock.calls.filter((a) => a[1] === 2500).length).toBe(tBefore);
    }

    // 3. Doc switch mid-save + vault-reset mid-save
    saveDef = deferred<unknown>();
    refreshCtxDef = null;
    const onSaveSwitch = vi.fn();
    const viewSwitch = render(
      <FocusView projectId="proj-1" artifactId="art-save-1" onBack={vi.fn()} onOpenCanvas={vi.fn()} onSave={onSaveSwitch} />
    );
    await waitFor(() => expect(screen.getByTestId('btn-save-artifact')).toBeEnabled());
    fireEvent.click(screen.getByTestId('btn-save-artifact'));
    viewSwitch.rerender(
      <FocusView projectId="proj-1" artifactId="art-save-2" onBack={vi.fn()} onOpenCanvas={vi.fn()} onSave={onSaveSwitch} />
    );
    await waitFor(() => expect(screen.getByTestId('input-artifact-title')).toHaveValue('Second Doc'));
    const tBeforeSwitchSettle = timeoutSpy.mock.calls.filter((a) => a[1] === 2500).length;
    await act(async () => {
      saveDef.resolve({
        artifact: { ...baseArt, version: 3 },
        mirror_synced: true,
        mirror_sync_error: null,
        unchanged: false,
        repaired_mirror_only: false,
      });
    });
    expect(onSaveSwitch).not.toHaveBeenCalled();
    expect(timeoutSpy.mock.calls.filter((a) => a[1] === 2500).length).toBe(tBeforeSwitchSettle);
    expect(screen.getByTestId('input-artifact-title')).toHaveValue('Second Doc');

    // Vault reset mid-save
    saveDef = deferred<unknown>();
    fireEvent.click(screen.getByTestId('btn-save-artifact'));
    act(() => {
      window.dispatchEvent(new CustomEvent('sthira:vault-reset'));
    });
    const tBeforeResetSettle = timeoutSpy.mock.calls.filter((a) => a[1] === 2500).length;
    await act(async () => {
      saveDef.resolve({
        artifact: { ...art2, version: 4 },
        mirror_synced: true,
        mirror_sync_error: null,
        unchanged: false,
        repaired_mirror_only: false,
      });
    });
    expect(onSaveSwitch).not.toHaveBeenCalled();
    expect(timeoutSpy.mock.calls.filter((a) => a[1] === 2500).length).toBe(tBeforeResetSettle);
    viewSwitch.unmount();

    // 4. Mirror-warning outcome and save error NEVER schedule the 2500ms "Saved" timer
    saveDef = deferred<unknown>();
    const onSaveWarn = vi.fn();
    const viewWarn = render(
      <FocusView projectId="proj-1" artifactId="art-save-1" onBack={vi.fn()} onOpenCanvas={vi.fn()} onSave={onSaveWarn} />
    );
    await waitFor(() => expect(screen.getByTestId('btn-save-artifact')).toBeEnabled());
    const tBeforeWarn = timeoutSpy.mock.calls.filter((a) => a[1] === 2500).length;
    fireEvent.click(screen.getByTestId('btn-save-artifact'));
    await act(async () => {
      saveDef.resolve({
        artifact: { ...baseArt, version: 5, last_synced_hash: null },
        mirror_synced: false,
        mirror_sync_error: 'disk read-only',
        unchanged: false,
        repaired_mirror_only: false,
      });
    });
    await waitFor(() => expect(screen.getByTestId('btn-save-artifact')).toHaveTextContent('Saved (mirror warning)'));
    expect(onSaveWarn).toHaveBeenCalledTimes(1);
    expect(timeoutSpy.mock.calls.filter((a) => a[1] === 2500).length).toBe(tBeforeWarn);

    saveDef = deferred<unknown>();
    fireEvent.click(screen.getByTestId('btn-save-artifact'));
    await act(async () => {
      saveDef.reject(new Error('SQLite busy'));
    });
    await waitFor(() => expect(screen.getByTestId('btn-save-artifact')).toHaveTextContent('Save failed'));
    expect(onSaveWarn).toHaveBeenCalledTimes(1);
    expect(timeoutSpy.mock.calls.filter((a) => a[1] === 2500).length).toBe(tBeforeWarn);
    viewWarn.unmount();

    // 5. Armed timer cleared on unmount + consecutive save timer isolation (using fake timers)
    timeoutSpy.mockRestore();
    vi.useFakeTimers();
    const clearSpy = vi.spyOn(globalThis, 'clearTimeout');
    let saveVer = 10;
    mockInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'cmd_get_artifact_context':
          return {
            artifact: { ...baseArt, version: saveVer },
            epistemic_nodes: [],
            pending_proposals: [],
            project_id: 'proj-1',
            is_unfinished_draft: false,
          };
        case 'cmd_get_project_history':
          return { project_id: 'proj-1', items: [] };
        case 'cmd_get_document_conversation':
          return null;
        case 'cmd_get_provider_catalog':
          return CATALOG;
        case 'cmd_list_object_types':
          return [];
        case 'cmd_get_object_instance':
          return null;
        case 'cmd_save_artifact':
          saveVer += 1;
          return {
            artifact: { ...baseArt, version: saveVer },
            mirror_synced: true,
            mirror_sync_error: null,
            unchanged: false,
            repaired_mirror_only: false,
          };
        default:
          return null;
      }
    });

    const viewTimer = render(
      <FocusView projectId="proj-1" artifactId="art-save-1" onBack={vi.fn()} onOpenCanvas={vi.fn()} onSave={vi.fn()} />
    );
    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });
    expect(screen.getByTestId('btn-save-artifact')).toBeEnabled();

    // Save #1 at t=0
    await act(async () => {
      fireEvent.click(screen.getByTestId('btn-save-artifact'));
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(screen.getByTestId('btn-save-artifact')).toHaveTextContent('Saved');

    // Advance 2000ms (<2500ms) and trigger Save #2
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    await act(async () => {
      fireEvent.click(screen.getByTestId('btn-save-artifact'));
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(screen.getByTestId('btn-save-artifact')).toHaveTextContent('Saved');

    // Advance 600ms -> Save #1's timer would have fired at 2500ms, but Save #2 MUST still show "Saved"
    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });
    expect(screen.getByTestId('btn-save-artifact')).toHaveTextContent('Saved');

    // Advance past Save #2's 2500ms window -> resets cleanly
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(screen.getByTestId('btn-save-artifact')).toHaveTextContent('Auto-saved');

    // Trigger Save #3 and unmount while 2500ms timer is armed -> clearTimeout cancels it
    await act(async () => {
      fireEvent.click(screen.getByTestId('btn-save-artifact'));
      await vi.advanceTimersByTimeAsync(1);
    });
    const clearsBeforeUnmount = clearSpy.mock.calls.length;
    viewTimer.unmount();
    expect(clearSpy.mock.calls.length).toBeGreaterThan(clearsBeforeUnmount);
    clearSpy.mockRestore();
    vi.useRealTimers();
  });
});
