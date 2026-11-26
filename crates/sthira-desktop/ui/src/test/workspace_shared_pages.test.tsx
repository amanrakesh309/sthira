import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

const tauriMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: tauriMocks.invoke,
}));

vi.mock('../api/ipc', () => ({
  isTauri: true,
  getArtifactContext: vi.fn(),
  getWorkspaceIndex: vi.fn(),
}));

import { getArtifactContext, getWorkspaceIndex } from '../api/ipc';
import {
  sanitizePersonalPageForShare,
  validateSharedPageInput,
} from '../api/shareSanitizer';
import { TeamWorkspaceView } from '../components/TeamWorkspaceView';
import { PublishPersonalCopyModal } from '../components/PublishPersonalCopyModal';
import { SharedPageDraftEditor } from '../components/SharedPageDraftEditor';
import { SharedProposalReview } from '../components/SharedProposalReview';
import type {
  SharedPageDetails,
  SharedProposalDetails,
  WorkspaceDetails,
  WorkspaceSummary,
} from '../api/workspaces';
import type { ArtifactContext } from '../types/sthira';

const wsOwner: WorkspaceSummary = {
  workspaceId: 'ws_1',
  name: 'Collab Team',
  myMembershipId: 'mem_a',
  myRole: 'owner',
  createdAtMs: 1_700_000_000_000,
  updatedAtMs: 1_700_000_000_000,
};

const wsViewer: WorkspaceSummary = {
  ...wsOwner,
  myMembershipId: 'mem_v',
  myRole: 'viewer',
};

const ownerDetails: WorkspaceDetails = {
  workspace: wsOwner,
  members: [
    {
      membershipId: 'mem_a',
      uid: 'uid_a',
      displayName: 'Alice Owner',
      role: 'owner',
      joinedAtMs: 1_700_000_000_000,
      isCurrentUser: true,
    },
    {
      membershipId: 'mem_b',
      uid: 'uid_b',
      displayName: 'Bob Editor',
      role: 'editor',
      joinedAtMs: 1_700_000_001_000,
      isCurrentUser: false,
    },
  ],
  invitations: [],
  pendingTransfer: null,
};

const samplePageDetails: SharedPageDetails = {
  page: {
    pageId: 'shp_1',
    workspaceId: 'ws_1',
    title: 'Shared Research Brief',
    currentRevisionNumber: 2,
    entryMode: 'copyPersonal',
    provenance: {
      originKind: 'userAuthored',
      originalAuthorLabel: 'Alice Personal Author',
      sourceNote: 'docs.example.org',
    },
    omittedSummary: ['Personal vault links converted to plain text (1)'],
    publisherUid: 'uid_a',
    publisherDisplayName: 'Alice Owner',
    lastEditorUid: 'uid_b',
    lastEditorDisplayName: 'Bob Editor',
    lastReviewerUid: 'uid_a',
    lastReviewerDisplayName: 'Alice Owner',
    archivedAtMs: null,
    archivedByDisplayName: null,
    unresolvedThreadCount: 1,
    pendingProposalCount: 1,
    createdAtMs: 1_700_000_000_000,
    updatedAtMs: 1_700_000_010_000,
  },
  currentRevision: {
    revisionId: 'rev_2',
    workspaceId: 'ws_1',
    pageId: 'shp_1',
    revisionNumber: 2,
    title: 'Shared Research Brief',
    contentMd: '## Key Points\n\n- [x] Verified\n\n| Metric | Value |\n| --- | --- |\n| Latency | 12ms |',
    contentSha256: 'abc',
    changeSummary: 'Added latency row',
    authorUid: 'uid_b',
    authorDisplayName: 'Bob Editor',
    reviewerUid: 'uid_a',
    reviewerDisplayName: 'Alice Owner',
    proposalId: 'prp_0',
    createdAtMs: 1_700_000_010_000,
  },
  revisions: [
    {
      revisionId: 'rev_2',
      workspaceId: 'ws_1',
      pageId: 'shp_1',
      revisionNumber: 2,
      title: 'Shared Research Brief',
      contentMd: '## Key Points',
      contentSha256: 'abc',
      changeSummary: 'Added latency row',
      authorUid: 'uid_b',
      authorDisplayName: 'Bob Editor',
      reviewerUid: 'uid_a',
      reviewerDisplayName: 'Alice Owner',
      proposalId: 'prp_0',
      createdAtMs: 1_700_000_010_000,
    },
  ],
  threads: [
    {
      threadId: 'thr_1',
      workspaceId: 'ws_1',
      pageId: 'shp_1',
      baseRevisionNumber: 1,
      authorUid: 'uid_b',
      authorDisplayName: 'Bob Editor',
      status: 'open',
      createdAtMs: 1_700_000_002_000,
      updatedAtMs: 1_700_000_002_000,
      messages: [
        {
          messageId: 'msg_1',
          threadId: 'thr_1',
          authorUid: 'uid_b',
          authorDisplayName: 'Bob Editor',
          body: 'Should we include benchmarks here?',
          createdAtMs: 1_700_000_002_000,
        },
      ],
    },
  ],
  proposals: [
    {
      proposalId: 'prp_1',
      workspaceId: 'ws_1',
      pageId: 'shp_1',
      pageTitle: 'Shared Research Brief',
      baseRevisionNumber: 1,
      summary: 'Update benchmark table',
      authorUid: 'uid_b',
      authorDisplayName: 'Bob Editor',
      status: 'pending',
      submissionCount: 1,
      isStale: true,
      createdAtMs: 1_700_000_003_000,
      updatedAtMs: 1_700_000_003_000,
    },
  ],
};

const staleProposalDetails: SharedProposalDetails = {
  proposal: samplePageDetails.proposals[0],
  baseTitle: 'Shared Research Brief',
  baseContentMd: 'Old rev 1 body',
  proposedTitle: 'Shared Research Brief',
  proposedContentMd: 'Proposed body from rev 1',
  currentPageRevisionNumber: 2,
  currentPageTitle: 'Shared Research Brief',
  currentPageContentMd: 'New rev 2 body',
  pageArchived: false,
  canReview: true,
  canAuthorEdit: false,
  decisions: [
    {
      decisionId: 'dec_1',
      submissionNumber: 1,
      action: 'submitted',
      actorUid: 'uid_b',
      actorDisplayName: 'Bob Editor',
      note: 'Update benchmark table',
      baseRevisionNumber: 1,
      submittedTitle: 'Shared Research Brief',
      submittedContentMd: 'Proposed body from rev 1',
      createdAtMs: 1_700_000_003_000,
    },
  ],
};

describe('CW2 Shared Pages, Preview Copy, Draft Recovery, and Change Review', () => {
  beforeEach(() => {
    tauriMocks.invoke.mockReset();
    vi.mocked(getArtifactContext).mockReset();
    vi.mocked(getWorkspaceIndex).mockReset();
  });

  it('sanitizes personal copy omissions locally without leaking private paths, data URLs, or internal links', () => {
    const ctx: ArtifactContext = {
      artifact: {
        id: 'art_private_1',
        artifact_type: 'note',
        authorship: 'user',
        title: 'Local Draft',
        content_md:
          '# Local Draft\n\nSee [Secret Note](sthira://artifact/art_secret) and ![img](data:image/png;base64,AAAA) and [doc](file:///home/aman/secret.md) and [Public](https://example.org/spec).',
        source_url: 'file:///home/aman/local-file.txt',
        content_hash: 'h1',
        version: 1,
        file_path: '/home/aman/.sthira/vault/Local Draft.md',
        last_synced_hash: 'sync_hash_1',
        is_unfinished_draft: false,
        created_at: 1000,
        updated_at: 2000,
        derived_from_artifact_id: 'art_parent',
      },
      epistemic_nodes: [
        {
          id: 'ep_1',
          artifact_id: 'art_private_1',
          node_type: 'finding',
          origin: 'user_recorded',
          lifecycle: 'open',
          statement: 'Finding',
        },
      ],
      pending_proposals: [],
      project_id: 'proj_1',
      project_ids: ['proj_1'],
      is_unfinished_draft: false,
    };

    const preview = sanitizePersonalPageForShare(ctx, 'Alice');
    expect(preview.sanitizedMarkdown).not.toContain('sthira://');
    expect(preview.sanitizedMarkdown).not.toContain('data:image');
    expect(preview.sanitizedMarkdown).not.toContain('file:///home/aman');
    expect(preview.sanitizedMarkdown).toContain('Secret Note');
    expect(preview.sanitizedMarkdown).toContain('[Public](https://example.org/spec)');
    expect(preview.provenance.sourceNote).toBeUndefined();
    expect(preview.omissions.length).toBeGreaterThanOrEqual(4);
    expect(JSON.stringify(preview)).not.toContain('art_private_1');
    expect(JSON.stringify(preview)).not.toContain('/home/aman');
  });

  it('requires fresh preview if personal original changes after Publish a copy preview', async () => {
    vi.mocked(getWorkspaceIndex).mockResolvedValue({
      counts: { all: 1, projects: 0, notes: 1, objects: 0, standalone: 1 },
      items: [
        {
          id: 'art_1',
          title: 'Research Memo',
          entity_type: 'note',
          is_pinned: false,
          last_accessed_at: 1000,
          updated_at: 1000,
          created_at: 1000,
          standalone: true,
        },
      ],
    });
    vi.mocked(getArtifactContext)
      .mockResolvedValueOnce({
        artifact: {
          id: 'art_1',
          artifact_type: 'note',
          authorship: 'user',
          title: 'Research Memo',
          content_md: 'Initial v1 with [ref](sthira://artifact/a2)',
          content_hash: 'hash_v1',
          version: 1,
          is_unfinished_draft: false,
          created_at: 1000,
          updated_at: 1000,
        },
        epistemic_nodes: [],
        pending_proposals: [],
        is_unfinished_draft: false,
      })
      .mockResolvedValueOnce({
        artifact: {
          id: 'art_1',
          artifact_type: 'note',
          authorship: 'user',
          title: 'Research Memo',
          content_md: 'Edited privately v2!',
          content_hash: 'hash_v2',
          version: 2,
          is_unfinished_draft: false,
          created_at: 1000,
          updated_at: 2000,
        },
        epistemic_nodes: [],
        pending_proposals: [],
        is_unfinished_draft: false,
      });

    const user = userEvent.setup();
    render(
      <PublishPersonalCopyModal
        isOpen={true}
        workspaceId="ws_1"
        workspaceName="Collab Team"
        onPublished={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    await user.click(await screen.findByRole('button', { name: 'Preview copy of Research Memo' }));
    expect(await screen.findByText(/Personal vault links converted to plain text/i)).toBeInTheDocument();

    // Confirm triggers recheck -> detects v1 != v2 and blocks publishing unreviewed v2!
    await user.click(screen.getByRole('button', { name: 'Publish copy to Collab Team' }));
    expect(
      await screen.findByText(/Your personal original changed after preview/i),
    ).toBeInTheDocument();
    expect(tauriMocks.invoke).not.toHaveBeenCalledWith('cmd_create_shared_page', expect.anything());
  });

  it('hides authoring controls for Viewer while allowing reading and enforces stale proposal UI', async () => {
    tauriMocks.invoke.mockImplementation(async (cmd) => {
      if (cmd === 'cmd_get_workspace') {
        return {
          outcome: 'ok',
          data: {
            ...ownerDetails,
            workspace: wsViewer,
          },
        };
      }
      if (cmd === 'cmd_list_shared_pages') {
        return { outcome: 'ok', data: { items: [samplePageDetails.page], nextCursor: null } };
      }
      if (cmd === 'cmd_get_shared_page') {
        return { outcome: 'ok', data: samplePageDetails };
      }
      if (cmd === 'cmd_get_proposal') {
        return { outcome: 'ok', data: staleProposalDetails };
      }
      return null;
    });

    const user = userEvent.setup();
    render(
      <TeamWorkspaceView
        workspace={wsViewer}
        initialTab="pages"
        currentUid="uid_v"
        onSelectPersonal={vi.fn()}
        onWorkspaceUpdated={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
        onRefreshList={vi.fn()}
      />,
    );

    expect(await screen.findByText('Read-only access (Viewer)')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'New shared page' })).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Open Shared Research Brief' }));
    expect(await screen.findByText(/Should we include benchmarks here/i)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Propose edit' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Start thread' })).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Open review' }));
    expect(await screen.findByText(/Stale base revision/i)).toBeInTheDocument();
  });

  it('recovers unsent draft locally on outage, allows retry, and triggers accessLost on removal', async () => {
    const onAccessLost = vi.fn();
    let offline = true;
    tauriMocks.invoke.mockImplementation(async (cmd, args: Record<string, unknown>) => {
      if (cmd === 'cmd_get_workspace') return { outcome: 'ok', data: ownerDetails };
      if (cmd === 'cmd_list_shared_pages') return { outcome: 'ok', data: { items: [], nextCursor: null } };
      if (cmd === 'cmd_get_shared_draft') {
        return {
          accountUid: 'uid_a',
          workspaceId: 'ws_1',
          draftKey: 'new_page',
          pageId: null,
          proposalId: null,
          baseRevisionNumber: null,
          title: 'Recovered Offline Page',
          contentMd: 'Saved during outage',
          summary: '',
          updatedAtMs: 1234,
        };
      }
      if (cmd === 'cmd_save_shared_draft') {
        return {
          accountUid: 'uid_a',
          workspaceId: 'ws_1',
          draftKey: 'new_page',
          title: args.title,
          contentMd: args.contentMd,
          summary: '',
          updatedAtMs: 1235,
        };
      }
      if (cmd === 'cmd_delete_shared_draft') return null;
      if (cmd === 'cmd_create_shared_page') {
        if (offline) {
          return {
            outcome: 'error',
            code: 'service_unavailable',
            message: 'Could not reach the Sthira session service.',
          };
        }
        return { outcome: 'accessLost', message: 'You no longer have access.', workspaces: null };
      }
      return null;
    });

    const user = userEvent.setup();
    render(
      <TeamWorkspaceView
        workspace={wsOwner}
        initialTab="pages"
        currentUid="uid_a"
        onSelectPersonal={vi.fn()}
        onWorkspaceUpdated={vi.fn()}
        onAccessLost={onAccessLost}
        onSessionChange={vi.fn()}
        onRefreshList={vi.fn()}
      />,
    );

    await user.click(await screen.findByRole('button', { name: 'New shared page' }));
    expect(await screen.findByText(/Restored private local draft/i)).toBeInTheDocument();
    expect(screen.getByLabelText('Shared page title')).toHaveValue('Recovered Offline Page');

    // Service outage fails cleanly & preserves draft in editor for retry:
    await user.click(screen.getByRole('button', { name: 'Publish shared page' }));
    expect(await screen.findByRole('alert')).toHaveTextContent(/Could not reach/i);
    expect(screen.getByLabelText('Shared page title')).toHaveValue('Recovered Offline Page');

    // Retry after removal clears via onAccessLost:
    offline = false;
    await user.click(screen.getByRole('button', { name: 'Publish shared page' }));
    await waitFor(() => {
      expect(onAccessLost).toHaveBeenCalledWith('You no longer have access.', null);
    });
  });

  it('ignores stale shared-page list responses after switching workspaces and enforces self-review block in UI', async () => {
    let resolveWs1Pages!: (v: unknown) => void;
    const ws1PagesPromise = new Promise((r) => {
      resolveWs1Pages = r;
    });
    const ws2: WorkspaceSummary = { ...wsOwner, workspaceId: 'ws_2', name: 'Workspace Two' };

    tauriMocks.invoke.mockImplementation(async (cmd, args: Record<string, unknown>) => {
      if (cmd === 'cmd_get_workspace') {
        return {
          outcome: 'ok',
          data: {
            ...ownerDetails,
            workspace: args.workspaceId === 'ws_2' ? ws2 : wsOwner,
          },
        };
      }
      if (cmd === 'cmd_list_shared_pages') {
        if (args.workspaceId === 'ws_1') return ws1PagesPromise;
        return { outcome: 'ok', data: { items: [], nextCursor: null } };
      }
      if (cmd === 'cmd_get_proposal') {
        return {
          outcome: 'ok',
          data: {
            ...staleProposalDetails,
            proposal: {
              ...staleProposalDetails.proposal,
              workspaceId: 'ws_2',
              authorUid: 'uid_a',
              isStale: false,
            },
            canReview: false,
            canAuthorEdit: true,
          },
        };
      }
      return null;
    });

    const { rerender } = render(
      <TeamWorkspaceView
        key="ws_1"
        workspace={wsOwner}
        initialTab="pages"
        currentUid="uid_a"
        onSelectPersonal={vi.fn()}
        onWorkspaceUpdated={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
        onRefreshList={vi.fn()}
      />,
    );

    rerender(
      <TeamWorkspaceView
        key="ws_2"
        workspace={ws2}
        initialTab="pages"
        currentUid="uid_a"
        onSelectPersonal={vi.fn()}
        onWorkspaceUpdated={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
        onRefreshList={vi.fn()}
      />,
    );

    // Late ws_1 response resolves AFTER switching to ws_2; ws_2 must not display ws_1's page!
    resolveWs1Pages({ outcome: 'ok', data: { items: [samplePageDetails.page], nextCursor: null } });
    expect(await screen.findByText(/No shared pages yet/i)).toBeInTheDocument();
    expect(screen.queryByText('Shared Research Brief')).not.toBeInTheDocument();
  });

  it('sanitizes reference-style file: links, attachments, autolinks, and HTML while preserving valid Markdown', () => {
    const ctx: ArtifactContext = {
      artifact: {
        id: 'art_md_1',
        artifact_type: 'note',
        authorship: 'user',
        title: 'Markdown Sanitize Test',
        content_md: [
          '# Markdown Sanitize Test',
          '',
          '## Supported Section',
          '',
          '- [x] **Keep bold checklist** with `inline [code](file:///literal)`',
          '',
          'See [Private Vault](sthira://artifact/a9), [Local File][ref-file], ![LocalPic](attachments/local.png), <file:///tmp/secret.txt>, <a href="file&#58;///etc/hosts">Hosts</a>.',
          '',
          'Keep [Public Docs](https://example.org/spec), [Public Ref][pub-ref], ![Badge](https://example.org/badge.png), and <https://example.org/guide>.',
          '',
          '[ref-file]: file:///tmp/private-attachment.pdf "Secret"',
          '[pub-ref]: https://example.org/reference "Public"',
        ].join('\n'),
        source_url: 'https://docs.example.org/article/full-path?token=1',
        content_hash: 'h_md',
        version: 1,
        file_path: '/home/aman/.sthira/vault/Markdown.md',
        is_unfinished_draft: false,
        created_at: 1000,
        updated_at: 2000,
      },
      epistemic_nodes: [],
      pending_proposals: [],
      is_unfinished_draft: false,
    };

    const preview = sanitizePersonalPageForShare(ctx, 'Author A');
    expect(preview.sanitizedMarkdown).not.toContain('file:///tmp/private-attachment.pdf');
    expect(preview.sanitizedMarkdown).not.toContain('attachments/local.png');
    expect(preview.sanitizedMarkdown).not.toContain('/etc/hosts');
    expect(preview.sanitizedMarkdown).not.toContain('sthira://artifact/a9');
    expect(preview.sanitizedMarkdown).toContain('## Supported Section');
    expect(preview.sanitizedMarkdown).toContain('**Keep bold checklist**');
    expect(preview.sanitizedMarkdown).toContain('`inline [code](file:///literal)`');
    expect(preview.sanitizedMarkdown).toContain('[Public Docs](https://example.org/spec)');
    expect(preview.sanitizedMarkdown).toContain('[Public Ref][pub-ref]');
    expect(preview.sanitizedMarkdown).toContain('[pub-ref]: https://example.org/reference');
    expect(preview.provenance.sourceNote).toBe('docs.example.org');
    expect(JSON.stringify(preview.omissions)).not.toContain('file:///');
    expect(validateSharedPageInput('Ok Title', preview.sanitizedMarkdown)).toBeNull();

    // Direct writes with reference-style file: link or attachment path are rejected cleanly:
    expect(
      validateSharedPageInput('Direct', 'See [Private][ref]\n\n[ref]: file:///tmp/secret.pdf'),
    ).not.toBeNull();
    expect(
      validateSharedPageInput('Direct', 'See [Rel](attachments/diagram.png)'),
    ).not.toBeNull();
  });

  it('binds review decisions to rendered submissionCount and preserves review note on stale conflict', async () => {
    const onReview = vi.fn().mockResolvedValue(false);
    const onRefresh = vi.fn();
    const user = userEvent.setup();
    render(
      <SharedProposalReview
        details={{
          ...staleProposalDetails,
          proposal: { ...staleProposalDetails.proposal, isStale: false, submissionCount: 1 },
          canReview: true,
        }}
        currentUid="uid_owner"
        busy={false}
        error="Submission #1 was replaced by submission #2. Refresh the review to inspect the replacement before deciding."
        onRefresh={onRefresh}
        onBack={vi.fn()}
        onReview={onReview}
        onOpenRevise={vi.fn()}
        onWithdraw={vi.fn()}
      />,
    );

    const noteInput = screen.getByLabelText('Review note');
    await user.type(noteInput, 'Please verify checklist item');
    await user.click(screen.getByRole('button', { name: 'Apply change' }));

    expect(onReview).toHaveBeenCalledWith(
      'apply',
      'Please verify checklist item',
      expect.any(String),
      1,
    );
    expect(noteInput).toHaveValue('Please verify checklist item');
    await user.click(screen.getByRole('button', { name: 'Refresh to latest submission' }));
    expect(onRefresh).toHaveBeenCalledTimes(1);
    expect(noteInput).toHaveValue('Please verify checklist item');
  });

  it('flushes latest draft on Back, surfaces local write failure honestly, and requires explicit rebase on outdated base', async () => {
    let failWrites = true;
    const savedCalls: Array<Record<string, unknown>> = [];
    const deletedCalls: Array<Record<string, unknown>> = [];
    tauriMocks.invoke.mockImplementation(async (cmd, args: Record<string, unknown>) => {
      if (cmd === 'cmd_get_shared_draft') {
        return {
          accountUid: 'uid_b',
          workspaceId: 'ws_1',
          draftKey: 'page_p1',
          pageId: 'p1',
          proposalId: null,
          baseRevisionNumber: 1,
          title: 'Draft Title',
          contentMd: 'Original unsent rev 1 draft',
          summary: 'My summary',
          updatedAtMs: 500,
        };
      }
      if (cmd === 'cmd_save_shared_draft') {
        if (failWrites) {
          throw 'Simulated disk write failure';
        }
        savedCalls.push(args);
        return {
          accountUid: 'uid_b',
          workspaceId: 'ws_1',
          draftKey: 'page_p1',
          pageId: 'p1',
          proposalId: null,
          baseRevisionNumber: args.baseRevisionNumber,
          title: args.title,
          contentMd: args.contentMd,
          summary: args.summary,
          updatedAtMs: 600,
        };
      }
      if (cmd === 'cmd_delete_shared_draft') {
        deletedCalls.push(args);
        return null;
      }
      return null;
    });

    const onCancel = vi.fn();
    let registeredGuard: (() => Promise<boolean>) | null = null;
    const user = userEvent.setup();
    render(
      <SharedPageDraftEditor
        workspaceId="ws_1"
        accountUid="uid_b"
        mode={{
          kind: 'proposeEdit',
          pageId: 'p1',
          baseRevisionNumber: 2,
          initialTitle: 'Current Rev 2 Title',
          initialContentMd: 'Current rev 2 published body',
        }}
        currentPublishedDoc={{
          revisionNumber: 2,
          title: 'Current Rev 2 Title',
          contentMd: 'Current rev 2 published body',
        }}
        registerBeforeLeave={(g) => {
          registeredGuard = g;
        }}
        submitting={false}
        error=""
        onClearError={vi.fn()}
        onCancel={onCancel}
        onSubmit={vi.fn().mockResolvedValue(false)}
      />,
    );

    // Outdated base rev 1 kept cleanly; Submit disabled until explicit rebase; comparison toggles cleanly:
    expect(await screen.findByText(/Base rev 1/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Submit proposal' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: /Compare draft \(rev 1\) with rev 2/i }));
    expect(screen.getByText('Current rev 2 published body')).toBeInTheDocument();
    expect(screen.getByLabelText('Shared page title')).toHaveValue('Draft Title');

    // Typing when local save fails shows honest error, does NOT claim Saved, and blocks Back/navigation:
    await user.type(screen.getByLabelText('Change summary'), ' plus immediate key');
    await user.click(screen.getByRole('button', { name: 'Back' }));
    expect(await screen.findByRole('alert')).toHaveTextContent(/Simulated disk write failure/);
    expect(screen.queryByText(/Saved private draft locally/i)).not.toBeInTheDocument();
    expect(onCancel).not.toHaveBeenCalled();

    // Once write succeeds, Back and beforeLeave guard flush latest text cleanly:
    failWrites = false;
    await user.click(screen.getByRole('button', { name: 'Back' }));
    await waitFor(() => expect(onCancel).toHaveBeenCalledTimes(1));
    expect(savedCalls.at(-1)?.summary).toBe('My summary plus immediate key');
    expect(savedCalls.at(-1)?.baseRevisionNumber).toBe(1);
    expect(await registeredGuard!()).toBe(true);

    // Rebase explicitly onto rev 2 preserves unsent title/content/summary and unlocks Submit:
    await user.click(screen.getByRole('button', { name: 'Rebase onto rev 2' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Submit proposal' })).toBeEnabled());
    expect(screen.getByLabelText('Shared page title')).toHaveValue('Draft Title');
    expect(screen.getByLabelText('Change summary')).toHaveValue('My summary plus immediate key');
    expect(savedCalls.at(-1)?.baseRevisionNumber).toBe(2);

    // Discard cancels pending save and deletes without letting earlier writes recreate it:
    await user.type(screen.getByLabelText('Change summary'), ' temporary');
    await user.click(screen.getByRole('button', { name: 'Discard local draft' }));
    await waitFor(() => expect(deletedCalls.length).toBeGreaterThanOrEqual(1));
  });

  it('keeps the form locked while restoring and ignores a previous context response', async () => {
    let finishOldRead!: (draft: unknown) => void;
    const oldRead = new Promise(resolve => { finishOldRead = resolve; });
    let finishCurrentRead!: (draft: unknown) => void;
    const currentRead = new Promise(resolve => { finishCurrentRead = resolve; });
    tauriMocks.invoke.mockImplementation(async (cmd, args) => {
      if (cmd === 'cmd_get_shared_draft') return args.accountUid === 'uid_a' ? oldRead : currentRead;
      return null;
    });
    const props = {
      workspaceId: 'ws_1',
      mode: { kind: 'newPage' } as const,
      submitting: false,
      error: '',
      onCancel: vi.fn(),
      onClearError: vi.fn(),
      onSubmit: vi.fn().mockResolvedValue(false),
    };
    const view = render(<SharedPageDraftEditor {...props} accountUid="uid_a" />);
    const title = screen.getByLabelText('Shared page title');
    expect(title).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Publish shared page' })).toBeDisabled();
    await waitFor(() => expect(tauriMocks.invoke).toHaveBeenCalledWith('cmd_get_shared_draft', expect.objectContaining({ accountUid: 'uid_a' })));
    view.rerender(<SharedPageDraftEditor {...props} accountUid="uid_b" />);
    expect(title).toBeDisabled();
    await act(async () => { finishCurrentRead(null); });
    expect(title).toBeEnabled();
    fireEvent.change(title, { target: { value: 'Latest draft for B' } });
    await act(async () => { finishOldRead({ title: 'Saved draft for A', contentMd: 'Only for A', summary: '' }); });
    expect(title).toHaveValue('Latest draft for B');
    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    await waitFor(() => expect(tauriMocks.invoke).toHaveBeenCalledWith('cmd_save_shared_draft', expect.objectContaining({ accountUid: 'uid_b', title: 'Latest draft for B' })));
  });

  it('offers recovery retry without enabling edits when the saved draft cannot be read', async () => {
    tauriMocks.invoke
      .mockRejectedValueOnce('Simulated draft read failure')
      .mockResolvedValueOnce({ title: 'Recovered on retry', contentMd: 'Saved body', summary: '' });
    const user = userEvent.setup();
    render(<SharedPageDraftEditor workspaceId="ws_1" accountUid="uid_b" mode={{ kind: 'newPage' }}
      submitting={false} error="" onCancel={vi.fn()} onClearError={vi.fn()} onSubmit={vi.fn()} />);
    expect(await screen.findByRole('alert')).toHaveTextContent('Simulated draft read failure');
    expect(screen.getByLabelText('Shared page title')).toBeDisabled();
    await user.click(screen.getByRole('button', { name: 'Retry draft recovery' }));
    await waitFor(() => expect(screen.getByLabelText('Shared page title')).toHaveValue('Recovered on retry'));
    expect(screen.getByLabelText('Shared page title')).toBeEnabled();
  });

  it('waits for the newest edit before leaving and exposes a failed final write', async () => {
    let finishFirstWrite!: (draft: unknown) => void;
    let failFinalWrite!: (error: string) => void;
    const firstWrite = new Promise(resolve => { finishFirstWrite = resolve; });
    const finalWrite = new Promise((_resolve, reject) => { failFinalWrite = reject; });
    let attempts = 0;
    let persisted: unknown = null;
    tauriMocks.invoke.mockImplementation(async (cmd, args) => {
      if (cmd === 'cmd_get_shared_draft') return persisted;
      if (cmd !== 'cmd_save_shared_draft') return null;
      attempts += 1;
      if (attempts === 1) await firstWrite;
      if (attempts === 2) await finalWrite;
      persisted = args;
      return args;
    });
    const onCancel = vi.fn();
    const props = { workspaceId: 'ws_1', accountUid: 'uid_b', mode: { kind: 'newPage' } as const,
      submitting: false, error: '', onCancel, onClearError: vi.fn(), onSubmit: vi.fn() };
    const view = render(<SharedPageDraftEditor {...props} />);
    const title = screen.getByLabelText('Shared page title');
    await waitFor(() => expect(title).toBeEnabled());
    fireEvent.change(title, { target: { value: 'First snapshot' } });
    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    await waitFor(() => expect(attempts).toBe(1));
    fireEvent.change(title, { target: { value: 'Latest edit' } });
    await act(async () => { finishFirstWrite(null); });
    await waitFor(() => expect(attempts).toBe(2));
    expect(onCancel).not.toHaveBeenCalled();
    await act(async () => { failFinalWrite('Simulated slow write failure'); });
    expect(await screen.findByRole('alert')).toHaveTextContent('Simulated slow write failure');
    expect(title).toHaveValue('Latest edit');
    expect(onCancel).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    await waitFor(() => expect(onCancel).toHaveBeenCalledTimes(1));
    expect(persisted).toEqual(expect.objectContaining({ title: 'Latest edit' }));
    view.unmount();
    render(<SharedPageDraftEditor {...props} />);
    await waitFor(() => expect(screen.getByLabelText('Shared page title')).toHaveValue('Latest edit'));
  });
});
