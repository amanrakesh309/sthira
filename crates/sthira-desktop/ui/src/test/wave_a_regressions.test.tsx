import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { FocusView } from '../components/FocusView';
import { App } from '../App';
import * as ipc from '../api/ipc';
import type {
  Artifact,
  ArtifactContext,
  ProjectOverviewData,
  SaveArtifactOutcome,
  SearchResultItem,
} from '../types/sthira';
import { setEditorMarkdown, editorMarkdown } from './editor-helpers';

const mockInvoke = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}));

interface FixtureProject {
  id: string;
  name: string;
  purpose: string;
}

interface FixtureArtifact {
  artifact: Artifact;
  projectIds: string[];
}

describe('Wave A End-to-End UI Regressions (A1, A3, A5)', () => {
  let sessionKey: string;
  let projects: Map<string, FixtureProject>;
  let artifacts: Map<string, FixtureArtifact>;
  let recentItems: Array<{
    id: string;
    title: string;
    item_type: string;
    subtitle: string;
    project_id: string | null;
    artifact_id: string;
    timestamp: number;
  }>;
  let saveCalls: Array<{
    artifactId: string;
    contentMd: string;
    title: string | null;
    projectId: string | null;
  }>;
  let deriveCalls: Array<{
    artifactId: string;
    derivationType: string;
    targetProjectId: string | null;
  }>;
  let customSaveHandler:
    | ((args: {
        artifactId: string;
        contentMd: string;
        title: string | null;
        projectId: string | null;
      }) => Promise<SaveArtifactOutcome>)
    | null;
  let customDeriveHandler:
    | ((args: {
        artifactId: string;
        derivationType: string;
        targetProjectId: string | null;
      }) => Promise<string>)
    | null;
  let customRefineHandler: ((args: any) => Promise<string>) | null;
  let customCancelHandler: ((args: any) => Promise<boolean>) | null;
  let customFilterHandler: ((artifactIds: string[]) => Promise<string[]>) | null;

  const makeArtifact = (
    id: string,
    title: string,
    content_md: string,
    overrides: Partial<Artifact> = {}
  ): Artifact => ({
    id,
    artifact_type: 'note',
    authorship: 'user',
    title,
    content_md,
    source_url: null,
    content_hash: `hash-${id}-1`,
    file_path: `${id}.md`,
    last_synced_hash: `hash-${id}-1`,
    version: 1,
    is_unfinished_draft: false,
    deleted_at: null,
    created_at: 1_700_000_000_000,
    updated_at: 1_700_000_000_000,
    derived_from_artifact_id: null,
    derivation_type: null,
    ...overrides,
  });

  const buildContext = (entry: FixtureArtifact): ArtifactContext => ({
    artifact: structuredClone(entry.artifact),
    epistemic_nodes: [],
    pending_proposals: [],
    project_id: entry.projectIds.length === 1 ? entry.projectIds[0] : null,
    project_ids: [...entry.projectIds],
    is_unfinished_draft: false,
    derived_from_title: entry.artifact.derived_from_artifact_id
      ? artifacts.get(entry.artifact.derived_from_artifact_id)?.artifact.title ?? null
      : null,
  });

  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    sessionStorage.clear();

    sessionKey = 'ns-vault-1';
    projects = new Map([
      ['proj-a', { id: 'proj-a', name: 'Project Alpha', purpose: 'Alpha workspace' }],
      ['proj-b', { id: 'proj-b', name: 'Project Beta', purpose: 'Beta workspace' }],
    ]);
    artifacts = new Map([
      [
        'art-alpha',
        {
          artifact: makeArtifact(
            'art-alpha',
            'Alpha Brief',
            '# Alpha Brief\n\nAlpha body content.'
          ),
          projectIds: ['proj-a'],
        },
      ],
      [
        'art-beta',
        {
          artifact: makeArtifact(
            'art-beta',
            'Beta Brief',
            '# Beta Brief\n\nBeta body content.'
          ),
          projectIds: ['proj-b'],
        },
      ],
      [
        'art-shared',
        {
          artifact: makeArtifact(
            'art-shared',
            'Shared Architecture',
            '# Shared Architecture\n\nShared notes across Alpha and Beta.'
          ),
          projectIds: ['proj-a', 'proj-b'],
        },
      ],
    ]);
    recentItems = [
      {
        id: 'rec-shared',
        title: 'Shared Architecture',
        item_type: 'Document',
        subtitle: 'Project Alpha',
        project_id: 'proj-a',
        artifact_id: 'art-shared',
        timestamp: 1_700_000_000_500,
      },
      {
        id: 'rec-alpha',
        title: 'Alpha Brief',
        item_type: 'Document',
        subtitle: 'Project Alpha',
        project_id: 'proj-a',
        artifact_id: 'art-alpha',
        timestamp: 1_700_000_000_400,
      },
    ];
    saveCalls = [];
    deriveCalls = [];
    customSaveHandler = null;
    customDeriveHandler = null;
    customRefineHandler = null;
    customCancelHandler = null;
    customFilterHandler = null;

    mockInvoke.mockImplementation(async (cmd: string, args: any = {}) => {
      switch (cmd) {
        case 'cmd_get_workspace_session_key':
          return sessionKey;
        case 'cmd_filter_existing_artifacts': {
          const ids: string[] = args.artifactIds || args.artifact_ids || [];
          if (customFilterHandler) return await customFilterHandler(ids);
          return ids.filter((id) => artifacts.has(id));
        }
        case 'cmd_get_home_state':
          return {
            projects: [...projects.values()].map((p) => ({
              id: p.id,
              name: p.name,
              purpose: p.purpose,
              last_substantive_at: 1_700_000_000_000,
              item_count: [...artifacts.values()].filter((a) => a.projectIds.includes(p.id)).length,
              category: 'Active',
              icon_type: 'folder',
            })),
            recent_items: [...recentItems],
            loose_captures: [],
          };
        case 'cmd_get_workspace_tree':
          return {
            projects: [...projects.values()].map((p) => ({
              id: p.id,
              name: p.name,
              purpose: p.purpose,
              pages: [...artifacts.entries()]
                .filter(([, entry]) => entry.projectIds.includes(p.id))
                .map(([id, entry]) => ({
                  id,
                  title: entry.artifact.title || 'Untitled Note',
                  updated_at: entry.artifact.updated_at,
                })),
              canvases: [],
              updated_at: 1_700_000_000_000,
            })),
            unassociated_pages: [...artifacts.entries()]
              .filter(([, entry]) => entry.projectIds.length === 0)
              .map(([id, entry]) => ({
                id,
                title: entry.artifact.title || 'Untitled Note',
                updated_at: entry.artifact.updated_at,
              })),
          };
        case 'cmd_get_sidebar_items':
          return {
            projects: [...projects.values()].map((p) => ({
              id: p.id,
              title: p.name,
              item_type: 'project',
              is_pinned: false,
              last_accessed_at: 1_700_000_000_000,
              updated_at: 1_700_000_000_000,
            })),
            notes: [...artifacts.entries()].map(([id, entry]) => ({
              id,
              title: entry.artifact.title || 'Untitled Note',
              item_type: 'note',
              is_pinned: false,
              last_accessed_at: entry.artifact.updated_at,
              updated_at: entry.artifact.updated_at,
              project_id: entry.projectIds.length === 1 ? entry.projectIds[0] : null,
              project_name:
                entry.projectIds.length === 1
                  ? projects.get(entry.projectIds[0])?.name || null
                  : null,
            })),
            objects: [],
          };
        case 'cmd_list_object_types':
        case 'cmd_get_type_templates':
        case 'cmd_list_synthesis_templates':
        case 'cmd_list_available_models':
        case 'cmd_get_workspace_connections':
        case 'cmd_list_search_providers':
          return [];
        case 'cmd_get_object_type_counts':
          return {};
        case 'cmd_get_object_instance':
        case 'cmd_record_access':
          return null;
        case 'cmd_get_workflow_settings':
          return {
            research: { enabled: true, model_id: 'ollama:llama3.2', templates_enabled: true },
            plan: { enabled: true, model_id: 'ollama:llama3.2', templates_enabled: true },
            draft: { enabled: true, model_id: 'ollama:llama3.2', templates_enabled: true },
            chat: { enabled: true, model_id: 'ollama:llama3.2', templates_enabled: true },
            resume: { enabled: true, model_id: 'ollama:llama3.2', templates_enabled: true },
          };
        case 'cmd_get_provider_catalog':
          return [
            {
              id: 'ollama',
              display_name: 'Ollama Local',
              provider_type: 'ollama',
              base_url: 'http://localhost:11434',
              is_enabled: true,
              models: [
                {
                  id: 'ollama:llama3.2',
                  provider_id: 'ollama',
                  model_identifier: 'llama3.2:latest',
                  display_name: 'Llama 3.2',
                  context_window: 8192,
                  supports_structured_output: true,
                  is_enabled: true,
                  is_default: true,
                  web_search_capability: 'engine_augmented_only',
                },
              ],
            },
          ];
        case 'cmd_get_project_history':
          return {
            project_id: args.projectId,
            narrative_segments: [],
            milestones: [],
          };
        case 'cmd_get_resume_brief':
          return {
            project_id: args.projectId,
            level: 1,
            purpose: 'Workspace test',
            recent_progress: [],
            conflicting_decisions: [],
            active_working_breadcrumb: '',
            continuation_point: '',
            primary_continuation_artifact_id: null,
            pane_hint: { collapse: { reason: 'active_session' } },
          };
        case 'cmd_get_project_overview': {
          const pid: string = args.projectId;
          const p = projects.get(pid);
          if (!p) throw new Error(`Project ${pid} not found`);
          const data: ProjectOverviewData = {
            project: {
              id: p.id,
              name: p.name,
              purpose: p.purpose,
              status: 'active',
              created_at: 1_700_000_000_000,
              last_substantive_at: 1_700_000_000_000,
            },
            items: [...artifacts.entries()]
              .filter(([, entry]) => entry.projectIds.includes(pid))
              .map(([id, entry]) => ({
                id,
                title: entry.artifact.title || 'Untitled Note',
                kind: 'artifact',
                item_type: 'note',
                description: entry.artifact.content_md,
                project_id: pid,
                object_type_id: null,
                object_type_name: null,
                icon: null,
                color: null,
                roles: ['member'],
                created_at: entry.artifact.created_at,
                updated_at: entry.artifact.updated_at,
                is_draft: false,
                item_count: null,
              })),
          };
          return data;
        }
        case 'cmd_get_workspace_entity': {
          const entity = args.entity as { kind: string; id: string };
          if (entity.kind === 'project') {
            const p = projects.get(entity.id);
            if (!p) throw new Error(`Project ${entity.id} not found`);
            return {
              entity: {
                kind: 'project',
                id: p.id,
                title: p.name,
                type_label: 'Project',
                subtitle: p.purpose,
              },
              project_name: p.name,
              connections_count: 0,
              member_ids: [],
            };
          }
          const entry = artifacts.get(entity.id);
          if (!entry) throw new Error(`Artifact ${entity.id} not found`);
          return {
            entity: {
              kind: 'artifact',
              id: entry.artifact.id,
              title: entry.artifact.title || 'Untitled Note',
              type_label: 'Page',
              subtitle: null,
            },
            project_name: null,
            connections_count: 0,
            member_ids: [],
          };
        }
        case 'cmd_get_artifact_context': {
          const aid: string = args.artifactId;
          const entry = artifacts.get(aid);
          if (!entry) throw new Error(`Artifact ${aid} not found`);
          return buildContext(entry);
        }
        case 'cmd_get_document_conversation':
          return {
            mode: 'document',
            initial_prompt: '',
            model_id: 'ollama:llama3.2',
            model_name: 'Llama 3.2',
            turns: [],
          };
        case 'cmd_search_all': {
          const q = String(args.query || '').toLowerCase();
          const results: SearchResultItem[] = [];
          for (const [id, entry] of artifacts.entries()) {
            const title = entry.artifact.title || '';
            if (
              !q ||
              title.toLowerCase().includes(q) ||
              entry.artifact.content_md.toLowerCase().includes(q)
            ) {
              results.push({
                id,
                title,
                kind: 'document',
                project_id: entry.projectIds.length === 1 ? entry.projectIds[0] : null,
                excerpt: entry.artifact.content_md.slice(0, 80),
              });
            }
          }
          return results;
        }
        case 'cmd_save_artifact': {
          const call = {
            artifactId: String(args.artifactId || args.artifact_id || ''),
            contentMd: String(args.contentMd ?? args.content_md ?? ''),
            title: (args.title ?? null) as string | null,
            projectId: (args.projectId ?? args.project_id ?? null) as string | null,
          };
          saveCalls.push(call);
          if (customSaveHandler) return await customSaveHandler(call);
          const existing = artifacts.get(call.artifactId);
          const nextHash = `hash-${call.artifactId}-${(existing?.artifact.version || 0) + 1}`;
          const nextArt = makeArtifact(
            call.artifactId || 'new-art',
            call.title || existing?.artifact.title || 'Untitled Note',
            call.contentMd,
            {
              version: (existing?.artifact.version || 0) + 1,
              content_hash: nextHash,
              last_synced_hash: nextHash,
              updated_at: Date.now(),
              derived_from_artifact_id: existing?.artifact.derived_from_artifact_id ?? null,
              derivation_type: existing?.artifact.derivation_type ?? null,
            }
          );
          const nextProjects = existing
            ? [...new Set([...existing.projectIds, ...(call.projectId ? [call.projectId] : [])])]
            : call.projectId
              ? [call.projectId]
              : [];
          artifacts.set(nextArt.id, { artifact: nextArt, projectIds: nextProjects });
          return {
            artifact: structuredClone(nextArt),
            mirror_synced: true,
            mirror_sync_error: null,
            unchanged: false,
            repaired_mirror_only: false,
          } satisfies SaveArtifactOutcome;
        }
        case 'cmd_refine_artifact': {
          if (customRefineHandler) return await customRefineHandler(args);
          return '# Alpha Brief\n\nRefined output content.';
        }
        case 'cmd_cancel_request': {
          if (customCancelHandler) return await customCancelHandler(args);
          return true;
        }
        case 'cmd_derive_artifact': {
          const call = {
            artifactId: String(
              args.sourceArtifactId ||
                args.source_artifact_id ||
                args.artifactId ||
                args.artifact_id ||
                ''
            ),
            derivationType: String(args.derivationType || args.derivation_type || ''),
            targetProjectId: (args.projectId ??
              args.project_id ??
              args.targetProjectId ??
              args.target_project_id ??
              null) as string | null,
          };
          deriveCalls.push(call);
          const childId = customDeriveHandler
            ? await customDeriveHandler(call)
            : `derived-${deriveCalls.length}`;
          if (!artifacts.has(childId)) {
            artifacts.set(childId, {
              artifact: makeArtifact(
                childId,
                'Derived Execution Plan',
                '# Derived Execution Plan\n\nSynthesized milestones.',
                {
                  authorship: 'ai_generated',
                  derived_from_artifact_id: call.artifactId,
                  derivation_type: call.derivationType,
                }
              ),
              projectIds: call.targetProjectId ? [call.targetProjectId] : [],
            });
          }
          return childId;
        }
        case 'cmd_clear_vault_data': {
          sessionKey = 'ns-vault-reset-2';
          projects.clear();
          artifacts.clear();
          recentItems = [];
          return null;
        }
        default:
          return null;
      }
    });
  });

  // -------------------------------------------------------------------------
  // A1: Mirror failure warning, Retry mirror sync, reopen & CRLF / indentation
  // -------------------------------------------------------------------------
  it('A1: surfaces mirror warning on post-commit sync failure, repairs via Retry without new edits, and resurfaces mismatch on reopen', async () => {
    const user = userEvent.setup();
    let failMirror = true;
    customSaveHandler = async (call) => {
      const existing = artifacts.get(call.artifactId)!;
      const newHash = 'hash-art-alpha-2';
      const updated: Artifact = {
        ...existing.artifact,
        content_md: call.contentMd,
        title: call.title || existing.artifact.title,
        version: existing.artifact.version + 1,
        content_hash: newHash,
        last_synced_hash: failMirror ? 'hash-art-alpha-1' : newHash,
      };
      artifacts.set(call.artifactId, { ...existing, artifact: updated });
      return {
        artifact: structuredClone(updated),
        mirror_synced: !failMirror,
        mirror_sync_error: failMirror ? 'destination is a directory' : null,
        unchanged: false,
        repaired_mirror_only: !failMirror,
      };
    };

    const view = render(
      <FocusView
        projectId="proj-a"
        artifactId="art-alpha"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    const editor = await screen.findByRole('textbox', { name: 'Document content' });
    setEditorMarkdown(editor, '# Alpha Brief\n\nSaved to SQLite despite mirror failure.');
    await user.click(screen.getByTestId('btn-save-artifact'));

    const banner = await screen.findByTestId('banner-mirror-warning');
    expect(banner).toHaveTextContent('destination is a directory');
    expect(screen.getByTestId('btn-save-artifact')).toHaveTextContent('Saved (mirror warning)');

    // Reopening a fresh FocusView instance with last_synced_hash != content_hash
    // still surfaces the persisted mirror warning immediately.
    view.unmount();
    const reopened = render(
      <FocusView
        projectId="proj-a"
        artifactId="art-alpha"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );
    await screen.findByTestId('banner-mirror-warning');
    expect(screen.getByTestId('btn-save-artifact')).toHaveTextContent('Saved (mirror warning)');

    // Repair mirror without changing editor text
    failMirror = false;
    await user.click(screen.getByTestId('btn-retry-mirror-sync'));
    await waitFor(() => {
      expect(screen.queryByTestId('banner-mirror-warning')).not.toBeInTheDocument();
    });
    expect(screen.getByTestId('btn-save-artifact')).not.toHaveTextContent('mirror warning');
    reopened.unmount();
  });

  it('A1: treats CRLF/trailing newline differences as clean while preserving leading indentation', async () => {
    const user = userEvent.setup();
    artifacts.set('art-alpha', {
      artifact: makeArtifact(
        'art-alpha',
        'Alpha Brief',
        '# Alpha Brief\r\n\r\n    indented line\r\n'
      ),
      projectIds: ['proj-a'],
    });
    const onBack = vi.fn();

    render(
      <FocusView
        projectId="proj-a"
        artifactId="art-alpha"
        onBack={onBack}
        onOpenCanvas={vi.fn()}
      />
    );

    const editor = await screen.findByRole('textbox', { name: 'Document content' });
    expect(screen.getByTestId('btn-save-artifact')).toHaveTextContent('Auto-saved');

    await user.click(screen.getByRole('button', { name: 'Home' }));
    expect(onBack).toHaveBeenCalledTimes(1);
    // CRLF/trailing newline equivalence must NOT trigger a redundant save.
    expect(saveCalls).toHaveLength(0);

    // Leading spaces change must be detected as a real modification when edited.
    setEditorMarkdown(editor, '# Alpha Brief\n\nindented line');
    await waitFor(() => {
      expect(screen.getByTestId('btn-save-artifact')).toHaveTextContent('Unsaved changes');
    });
  });

  it('A1: clean Refine updates baseline so reload does not create an extra user save, and Keep propagates mirror warning', async () => {
    const user = userEvent.setup();
    customRefineHandler = async () => {
      const entry = artifacts.get('art-alpha')!;
      const updated = {
        ...entry.artifact,
        content_md: '# Alpha Brief\n\nRefined model body.',
        authorship: 'ai_generated' as const,
        version: 2,
      };
      artifacts.set('art-alpha', { ...entry, artifact: updated });
      return updated.content_md;
    };

    render(
      <FocusView
        projectId="proj-a"
        artifactId="art-alpha"
        onBack={vi.fn()}
        onOpenCanvas={vi.fn()}
      />
    );

    await user.click(await screen.findByTestId('btn-refine-artifact'));
    await user.type(await screen.findByTestId('textarea-critique-prompt'), 'Improve clarity');
    saveCalls.length = 0;
    await user.click(screen.getByTestId('btn-run-refinement'));

    await waitFor(() => {
      expect(screen.queryByTestId('refine-artifact-modal')).not.toBeInTheDocument();
    });
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
      'Refined model body.'
    );

    // Wait past debounce window to ensure neither loadProjectData nor autosave dispatches a bogus user save.
    await new Promise((r) => setTimeout(r, 750));
    const spuriousUserSaves = saveCalls.filter((c) => c.contentMd.includes('Refined model body.'));
    expect(spuriousUserSaves).toHaveLength(0);

    // Conflict Keep path with mirror sync warning
    let resolveLateRefine!: (value: string) => void;
    customRefineHandler = () => new Promise((resolve) => (resolveLateRefine = resolve));
    customCancelHandler = async () => false;

    await user.click(screen.getByTestId('btn-refine-artifact'));
    await user.type(await screen.findByTestId('textarea-critique-prompt'), 'Conflict case');
    await user.click(screen.getByTestId('btn-run-refinement'));
    await user.click(await screen.findByTestId('btn-cancel-refine-modal'));

    setEditorMarkdown(
      screen.getByRole('textbox', { name: 'Document content' }),
      '# Alpha Brief\n\nUser conflict draft to keep.'
    );
    resolveLateRefine('# Alpha Brief\n\nModel committed version.');

    await screen.findByTestId('banner-refinement-conflict');
    customSaveHandler = async (call) => {
      const entry = artifacts.get(call.artifactId)!;
      const updated: Artifact = {
        ...entry.artifact,
        content_md: call.contentMd,
        content_hash: 'hash-keep-2',
        last_synced_hash: 'hash-keep-1',
      };
      artifacts.set(call.artifactId, { ...entry, artifact: updated });
      return {
        artifact: structuredClone(updated),
        mirror_synced: false,
        mirror_sync_error: 'read-only fs',
        unchanged: false,
        repaired_mirror_only: false,
      };
    };

    await user.click(screen.getByTestId('btn-dismiss-pending-refinement'));
    await waitFor(() => {
      expect(screen.queryByTestId('banner-refinement-conflict')).not.toBeInTheDocument();
    });
    expect(await screen.findByTestId('banner-mirror-warning')).toHaveTextContent('read-only fs');
  });

  // -------------------------------------------------------------------------
  // A3 (F-B5-005): Real App parent navigation guard during/after derive
  // -------------------------------------------------------------------------
  it('A3 (F-B5-005): blocks navigation while derive is in flight, then saves newer mid-derive edits before opening derived child in real App', async () => {
    const user = userEvent.setup();
    let resolveDerive!: (childId: string) => void;
    customDeriveHandler = () => new Promise<string>((resolve) => (resolveDerive = resolve));

    const view = render(<App />);

    // Open Alpha Brief from Home's Pick up where you left off list
    await user.click((await screen.findAllByRole('button', { name: /Alpha Brief/i }))[0]);
    await waitFor(() => expect(screen.getByTestId('input-artifact-title')).toHaveValue('Alpha Brief'));

    // Start derive on Alpha
    await user.click(screen.getByTestId('btn-derive-menu'));
    await user.click(screen.getByTestId('btn-derive-plan'));

    await waitFor(() => expect(deriveCalls).toHaveLength(1));

    // Attempt to leave via LeftSidebar Home while derive is pending -> blocked by App's real guard
    await user.click(screen.getByTestId('nav-home'));
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Wait for the current page change to finish before switching.'
    );
    expect(screen.getByTestId('input-artifact-title')).toHaveValue('Alpha Brief');

    // Type newer edits on source page WHILE derivation is still running
    const editor = screen.getByRole('textbox', { name: 'Document content' });
    setEditorMarkdown(editor, '# Alpha Brief\n\nEdited mid-derive must be saved before child opens.');
    saveCalls.length = 0;

    // Resolve derive -> source newer edit saves first, then App navigates smoothly to child!
    await act(async () => {
      resolveDerive('derived-plan-child');
    });

    await waitFor(() => {
      expect(screen.getByTestId('input-artifact-title')).toHaveValue('Derived Execution Plan');
    });
    expect(
      saveCalls.some(
        (c) =>
          c.artifactId === 'art-alpha' &&
          c.contentMd.includes('Edited mid-derive must be saved before child opens.')
      )
    ).toBe(true);
    expect(screen.getByTestId('badge-lineage-derived-from')).toHaveTextContent('Alpha Brief');
    view.unmount();
  });

  it('A3 (F-B5-005): aborts child navigation and keeps newer source edits open if post-derive save fails', async () => {
    const user = userEvent.setup();
    let resolveDerive!: (childId: string) => void;
    customDeriveHandler = () => new Promise<string>((resolve) => (resolveDerive = resolve));

    const view = render(<App />);
    await user.click((await screen.findAllByRole('button', { name: /Alpha Brief/i }))[0]);
    await waitFor(() => expect(screen.getByTestId('input-artifact-title')).toHaveValue('Alpha Brief'));

    await user.click(screen.getByTestId('btn-derive-menu'));
    await user.click(screen.getByTestId('btn-derive-plan'));
    await waitFor(() => expect(deriveCalls).toHaveLength(1));

    setEditorMarkdown(
      screen.getByRole('textbox', { name: 'Document content' }),
      '# Alpha Brief\n\nCritical mid-derive draft that must not vanish.'
    );
    customSaveHandler = async () => {
      throw new Error('Disk write error');
    };

    await act(async () => {
      resolveDerive('derived-plan-child');
    });

    expect(
      await screen.findByText('Couldn’t save your changes. Try saving again.')
    ).toBeInTheDocument();
    expect(screen.getByTestId('input-artifact-title')).toHaveValue('Alpha Brief');
    expect(editorMarkdown(screen.getByRole('textbox', { name: 'Document content' }))).toContain(
      'Critical mid-derive draft that must not vanish.'
    );
    view.unmount();
  });

  // -------------------------------------------------------------------------
  // A3 (F-B5-006): Shared vs Project vs Single-Parent placement through App
  // -------------------------------------------------------------------------
  it('A3 (F-B5-006): Search open of shared note derives standalone even when recent_items carries proj-a; Project opens bind to explicit project; single-parent inherits cleanly', async () => {
    const user = userEvent.setup();
    const view = render(<App />);

    // 1. Open Search from LeftSidebar and open shared note by identity alone
    await user.click(await screen.findByText('Search workspace...'));
    const searchInput = await screen.findByPlaceholderText(/Search/i);
    await user.type(searchInput, 'Shared Architecture');
    await user.click(await screen.findByRole('option', { name: 'Open Shared Architecture' }));

    await waitFor(() => {
      expect(screen.getByTestId('input-artifact-title')).toHaveValue('Shared Architecture');
    });
    // Must show Pages breadcrumb, NOT Project Alpha (even though recent_items has project_id='proj-a')
    expect(screen.getByTitle('Browse standalone pages')).toHaveTextContent('Pages');

    await user.click(screen.getByTestId('btn-derive-menu'));
    await user.click(screen.getByTestId('btn-derive-plan'));
    await waitFor(() => expect(deriveCalls).toHaveLength(1));
    expect(deriveCalls[0].targetProjectId).toBeNull();
    await waitFor(() => {
      expect(screen.getByTestId('input-artifact-title')).toHaveValue('Derived Execution Plan');
    });
    expect(screen.getByTitle('Browse standalone pages')).toHaveTextContent('Pages');

    // 2. Open Project Beta Overview and open the SAME shared note -> derives into proj-b
    await user.click(screen.getByTestId('nav-home'));
    await user.click(await screen.findByRole('button', { name: 'Open project: Project Beta' }));
    await user.click(await screen.findByRole('button', { name: 'Open Shared Architecture' }));
    await waitFor(() => {
      expect(screen.getByTitle('Open project: Project Beta')).toHaveTextContent('Project Beta');
    });
    await user.click(screen.getByTestId('btn-derive-menu'));
    await user.click(screen.getByTestId('btn-derive-ideas'));
    await waitFor(() => expect(deriveCalls).toHaveLength(2));
    expect(deriveCalls[1].targetProjectId).toBe('proj-b');

    // 3. Single-parent note ('art-beta' in proj-b only) opened via Search inherits proj-b unambiguously
    await user.click(screen.getByText('Search workspace...'));
    await user.type(await screen.findByPlaceholderText(/Search/i), 'Beta Brief');
    await user.click(await screen.findByRole('option', { name: 'Open Beta Brief' }));
    await waitFor(() => {
      expect(screen.getByTestId('input-artifact-title')).toHaveValue('Beta Brief');
    });
    await user.click(screen.getByTestId('btn-derive-menu'));
    await user.click(screen.getByTestId('btn-derive-tasks'));
    await waitFor(() => expect(deriveCalls).toHaveLength(3));
    expect(deriveCalls[2].targetProjectId).toBe('proj-b');
    view.unmount();
  });

  // -------------------------------------------------------------------------
  // A5 (F-B5-003): Startup tab pruning, vault reset cleanup, unavailable UI
  // -------------------------------------------------------------------------
  it('A5 (F-B5-003): prunes missing tabs on startup, recovers from deleted open document, and wipes tab/chat state on vault reset without resurrecting old doc', async () => {
    const user = userEvent.setup();
    localStorage.setItem(
      'sthira.tabs.v1:ns-vault-1',
      JSON.stringify({
        tabs: [
          {
            id: 'art-deleted',
            title: 'Deleted Stale Tab',
            projectId: 'proj-a',
            returnProjectId: null,
            scrollTop: 0,
          },
          {
            id: 'art-alpha',
            title: 'Alpha Brief',
            projectId: 'proj-a',
            returnProjectId: null,
            scrollTop: 0,
          },
        ],
        activeId: 'art-deleted',
        resume: true,
      })
    );
    sessionStorage.setItem('sthira.chat-draft:ns-vault-1:art-alpha', 'unsent chat draft');
    const unrelatedTabs = JSON.stringify({
      tabs: [
        {
          id: 'other-doc',
          title: 'Other Vault Doc',
          projectId: null,
          returnProjectId: null,
          scrollTop: 0,
        },
      ],
      activeId: 'other-doc',
      resume: true,
    });
    localStorage.setItem('sthira.tabs.v1:ns-other-vault', unrelatedTabs);
    sessionStorage.setItem('sthira.chat-draft:ns-other-vault:other-doc', 'other vault draft');

    const view1 = render(<App />);

    // Startup pruning strips 'art-deleted' and restores 'art-alpha'
    await waitFor(() => {
      expect(screen.getByTestId('input-artifact-title')).toHaveValue('Alpha Brief');
    });
    expect(screen.queryByRole('tab', { name: /Deleted Stale Tab/i })).not.toBeInTheDocument();

    // Dirty edit on Alpha before vault reset
    setEditorMarkdown(
      screen.getByRole('textbox', { name: 'Document content' }),
      '# Alpha Brief\n\nDirty before vault reset.'
    );
    saveCalls.length = 0;

    // Trigger Vault Reset via canonical ipc.clearVaultData()
    await act(async () => {
      await ipc.clearVaultData();
    });

    // Returns to Home, clears only current vault tab/chat storage, preserves unrelated vault, and does NOT resurrect dirty old note
    await waitFor(() => expect(screen.getByText('Workspace / Home')).toBeInTheDocument());
    expect(localStorage.getItem('sthira.tabs.v1:ns-vault-1')).toBeNull();
    expect(sessionStorage.getItem('sthira.chat-draft:ns-vault-1:art-alpha')).toBeNull();
    expect(localStorage.getItem('sthira.tabs.v1:ns-other-vault')).toBe(unrelatedTabs);
    expect(sessionStorage.getItem('sthira.chat-draft:ns-other-vault:other-doc')).toBe(
      'other vault draft'
    );
    expect(saveCalls).toHaveLength(0);
    view1.unmount();

    // Direct navigation to an artifact deleted between search and open -> Document unavailable recovery
    artifacts.set('art-vanishing', {
      artifact: makeArtifact('art-vanishing', 'Vanishing Note', '# Vanishing\n\nBody.'),
      projectIds: [],
    });
    const view2 = render(<App />);
    await user.click(await screen.findByText('Search workspace...'));
    const searchBox = await screen.findByPlaceholderText(/Search/i);
    await user.type(searchBox, 'Vanishing Note');
    await screen.findByText('Vanishing Note');
    artifacts.delete('art-vanishing');
    await user.click(screen.getByText('Vanishing Note'));

    expect(await screen.findByTestId('document-unavailable-state')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Document unavailable' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Close tab' }));
    await waitFor(() => expect(screen.queryByTestId('document-unavailable-state')).not.toBeInTheDocument());
    view2.unmount();
  });

  it('A5 (R1/R2): reset in full App preserves unrelated vault storage and blocks in-flight pre-reset tab restoration without timing sleeps', async () => {
    const unrelatedTabs = JSON.stringify({
      tabs: [
        {
          id: 'other-doc',
          title: 'Other Vault Doc',
          projectId: null,
          returnProjectId: null,
          scrollTop: 0,
        },
      ],
      activeId: 'other-doc',
      resume: true,
    });
    localStorage.setItem('sthira.tabs.v1:ns-other-vault', unrelatedTabs);
    sessionStorage.setItem('sthira.chat-draft:ns-other-vault:other-doc', 'other vault draft');
    localStorage.setItem(
      'sthira.tabs.v1:ns-vault-1',
      JSON.stringify({
        tabs: [
          {
            id: 'art-alpha',
            title: 'Alpha Brief',
            projectId: 'proj-a',
            returnProjectId: null,
            scrollTop: 0,
          },
        ],
        activeId: 'art-alpha',
        resume: true,
      })
    );
    sessionStorage.setItem('sthira.chat-draft:ns-vault-1:art-alpha', 'current draft to remove');

    let releaseValidation!: (ids: string[]) => void;
    let validationStarted!: () => void;
    const validationReady = new Promise<void>((resolve) => {
      validationStarted = resolve;
    });
    customFilterHandler = () => {
      validationStarted();
      return new Promise<string[]>((resolve) => {
        releaseValidation = resolve;
      });
    };

    const view = render(<App />);
    await validationReady;

    await act(async () => {
      await ipc.clearVaultData();
    });
    await waitFor(() => expect(screen.getByText('Workspace / Home')).toBeInTheDocument());

    // Resolve delayed pre-reset validation response after reset
    await act(async () => {
      releaseValidation(['art-alpha']);
    });

    expect(screen.getByText('Workspace / Home')).toBeInTheDocument();
    expect(screen.queryByTestId('input-artifact-title')).not.toBeInTheDocument();
    expect(screen.queryByRole('tab', { name: /Alpha Brief/i })).not.toBeInTheDocument();
    expect(localStorage.getItem('sthira.tabs.v1:ns-vault-1')).toBeNull();
    expect(sessionStorage.getItem('sthira.chat-draft:ns-vault-1:art-alpha')).toBeNull();
    expect(localStorage.getItem('sthira.tabs.v1:ns-other-vault')).toBe(unrelatedTabs);
    expect(sessionStorage.getItem('sthira.chat-draft:ns-other-vault:other-doc')).toBe(
      'other vault draft'
    );
    await waitFor(() => {
      const replacement = localStorage.getItem('sthira.tabs.v1:ns-vault-reset-2');
      expect(replacement).not.toBeNull();
      expect(JSON.parse(replacement!).tabs).toEqual([]);
    });
    view.unmount();
  });
});
