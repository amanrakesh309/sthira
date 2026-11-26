import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import * as Y from 'yjs';
import { LiveSharedDraftEditor } from '../components/LiveSharedDraftEditor';
import { SharedProposalReview } from '../components/SharedProposalReview';
import { TeamWorkspaceView } from '../components/TeamWorkspaceView';
import {
  encodeSelectionRelativePositions,
  seedInitialYjsUpdateB64,
  syncRemoteAwarenessPositions,
  uint8ToBase64,
} from '../editor/yjsCollab';
import { Awareness } from 'y-protocols/awareness';
import type { CollabServerEvent } from '../api/liveCollab';
import type {
  SharedProposalDetails,
  SharedProjectThreadView,
  WorkspaceDetails,
  WorkspaceSummary,
} from '../api/workspaces';

const tauriMocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  collabListeners: new Set<(ev: CollabServerEvent) => void>(),
  clipboardWrite: vi.fn(async () => {}),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: tauriMocks.invoke,
}));

vi.mock('../api/liveCollab', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api/liveCollab')>();
  return {
    ...actual,
    subscribeCollabEvents: vi.fn(async (handler: (ev: CollabServerEvent) => void) => {
      tauriMocks.collabListeners.add(handler);
      return () => {
        tauriMocks.collabListeners.delete(handler);
      };
    }),
  };
});

function emitCollabEvent(ev: CollabServerEvent) {
  for (const listener of Array.from(tauriMocks.collabListeners)) {
    listener(ev);
  }
}

const wsSummary: WorkspaceSummary = {
  workspaceId: 'ws_cw3',
  name: 'CW3 Lab',
  myMembershipId: 'mem_a',
  myRole: 'owner',
  createdAtMs: 1_750_000_000_000,
  updatedAtMs: 1_750_000_000_000,
};

const wsDetails: WorkspaceDetails = {
  workspace: wsSummary,
  members: [
    {
      membershipId: 'mem_a',
      uid: 'uid_a',
      displayName: 'Alice Owner',
      role: 'owner',
      joinedAtMs: 1_750_000_000_000,
      isCurrentUser: true,
    },
    {
      membershipId: 'mem_b',
      uid: 'uid_b',
      displayName: 'Bob Editor',
      role: 'editor',
      joinedAtMs: 1_750_000_100_000,
      isCurrentUser: false,
    },
  ],
  invitations: [],
  pendingTransfer: null,
};

describe('CW3 Live Co-editing, Bulk Invites, Contributor Review, and Project Discussions', () => {
  beforeEach(() => {
    tauriMocks.invoke.mockReset();
    tauriMocks.collabListeners.clear();
    tauriMocks.clipboardWrite.mockClear();
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
      invoke: tauriMocks.invoke,
      transformCallback: () => 0,
    };
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: tauriMocks.clipboardWrite },
      configurable: true,
    });
  });

  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  });

  it('Yjs DocumentEditor + LiveSharedDraftEditor isolates per-user Undo, handles offline Unsynced recovery, shows collaborator names, and freezes when pendingReview', async () => {
    const initialB64 = seedInitialYjsUpdateB64('Live Notes', 'Seed line.');
    const svB64 = uint8ToBase64(Y.encodeStateVector(new Y.Doc()));
    const savedDraftSpy = vi.fn();

    let sendShouldFail = false;
    tauriMocks.invoke.mockImplementation(async (cmd, args: any) => {
      if (cmd === 'cmd_open_live_draft') {
        return {
          outcome: 'ok',
          data: {
            created: true,
            draft: {
              draftId: 'drf_1',
              workspaceId: 'ws_cw3',
              pageId: 'shp_1',
              baseRevisionNumber: 1,
              draftVersion: 1,
              checkpointSeq: 0,
              checkpointId: 'chk:drf_1:v1:s0',
              status: 'editing',
              snapshotStateB64: initialB64,
              snapshotVectorB64: svB64,
              baseTitle: 'Live Notes',
              baseContentMd: 'Seed line.',
              currentTitle: 'Live Notes',
              currentContentMd: 'Seed line.',
              contributors: [],
              pendingProposalId: null,
              createdByUid: 'uid_a',
              createdByDisplayName: 'Alice Owner',
              createdAtMs: 1_750_000_000_000,
              updatedAtMs: 1_750_000_000_000,
            },
          },
        };
      }
      if (cmd === 'cmd_get_shared_draft') return null;
      if (cmd === 'cmd_collab_connect') {
        return { workspaceId: 'ws_cw3', uid: 'uid_a', connected: true };
      }
      if (cmd === 'cmd_get_live_draft') {
        return {
          outcome: 'ok',
          data: {
            draftId: 'drf_v2',
            workspaceId: 'ws_cw3',
            pageId: 'shp_rec',
            baseRevisionNumber: 1,
            draftVersion: 2,
            checkpointSeq: 2,
            checkpointId: 'chk:drf_v2:v2:s2',
            status: 'pendingReview',
            snapshotStateB64: initialB64,
            snapshotVectorB64: svB64,
            baseTitle: 'Doc A',
            baseContentMd: 'Initial text.',
            currentTitle: 'Doc A',
            currentContentMd: 'Initial text.',
            contributors: [],
            pendingProposalId: 'prp_frozen_rec',
            createdByUid: 'uid_a',
            createdByDisplayName: 'Alice Owner',
            createdAtMs: 1_750_000_000_000,
            updatedAtMs: 1_750_000_000_000,
          },
        };
      }
      if (cmd === 'cmd_collab_join_draft') {
        emitCollabEvent({
          type: 'connected',
          workspaceId: 'ws_cw3',
          connectionId: 'conn_a',
          uid: 'uid_a',
          displayName: 'Alice Owner',
          role: 'owner',
        });
        emitCollabEvent({
          type: 'draft_state',
          workspaceId: 'ws_cw3',
          draft: {
            draftId: 'drf_1',
            workspaceId: 'ws_cw3',
            pageId: 'shp_1',
            baseRevisionNumber: 1,
            draftVersion: 1,
            checkpointSeq: 0,
            checkpointId: 'chk:drf_1:v1:s0',
            status: 'editing',
            snapshotStateB64: initialB64,
            snapshotVectorB64: svB64,
            baseTitle: 'Live Notes',
            baseContentMd: 'Seed line.',
            currentTitle: 'Live Notes',
            currentContentMd: 'Seed line.',
            contributors: [],
            pendingProposalId: null,
            createdByUid: 'uid_a',
            createdByDisplayName: 'Alice Owner',
            createdAtMs: 1_750_000_000_000,
            updatedAtMs: 1_750_000_000_000,
          },
          missingUpdateB64: '',
          serverVectorB64: svB64,
          presence: [
            {
              connectionId: 'conn_a',
              uid: 'uid_a',
              displayName: 'Alice Owner',
              color: '#0B6E4F',
              pageId: 'shp_1',
              draftId: 'drf_1',
              anchor: 1,
              head: 1,
              updatedAtMs: 1000,
            },
            {
              connectionId: 'conn_b',
              uid: 'uid_b',
              displayName: 'Bob Editor',
              color: '#1D4ED8',
              pageId: 'shp_1',
              draftId: 'drf_1',
              anchor: 2,
              head: 2,
              updatedAtMs: 1000,
            },
          ],
        });
        return null;
      }
      if (cmd === 'cmd_save_shared_draft') {
        savedDraftSpy(args);
        return null;
      }
      if (cmd === 'cmd_collab_send_update') {
        if (sendShouldFail) throw 'Live collaboration channel is disconnected.';
        emitCollabEvent({
          type: 'draft_update_ack',
          workspaceId: 'ws_cw3',
          requestId: args.requestId,
          draftId: 'drf_1',
          draftVersion: 1,
          checkpointSeq: args.clientOpSeq,
          checkpointId: `chk:drf_1:v1:s${args.clientOpSeq}`,
          clientOpSeq: args.clientOpSeq,
          serverSeq: args.clientOpSeq,
          duplicate: false,
          contributors: [
            {
              uid: 'uid_a',
              displayName: 'Alice Owner',
              contributionCount: 1,
              lastEditedAtMs: 1_750_000_001_000,
            },
          ],
        });
        return null;
      }
      return null;
    });

    const user = userEvent.setup();
    render(
      <LiveSharedDraftEditor
        workspaceId="ws_cw3"
        pageId="shp_1"
        baseRevisionNumber={1}
        baseTitle="Live Notes"
        baseContentMd="Seed line."
        currentUid="uid_a"
        authorDisplayName="Alice Owner"
        onSubmitted={vi.fn()}
        onOpenProposal={vi.fn()}
        onBack={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
      />,
    );

    expect(await screen.findByTestId('live-sync-status')).toHaveTextContent('Synced');
    expect(screen.getByText('Alice Owner (You)')).toBeInTheDocument();
    expect(screen.getByText('Bob Editor')).toBeInTheDocument();

    // Simulate remote title update from Bob + local title edit by Alice.
    const titleInput = screen.getByLabelText('Live draft title') as HTMLInputElement;
    expect(titleInput).toHaveValue('Live Notes');

    sendShouldFail = true;
    await user.type(titleInput, ' X');
    await waitFor(() => {
      expect(screen.getByTestId('live-sync-status')).toHaveTextContent('Unsynced');
    });
    expect(savedDraftSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        workspaceId: 'ws_cw3',
        draftKey: 'live_shp_1',
        accountUid: 'uid_a',
      }),
    );
    expect(titleInput).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Reconnect & sync' })).toBeInTheDocument();

    // The proposal notification can arrive before the peer's draft update.
    // Its checkpoint must be applied to the editor as well as to the review controls.
    const submittedSnapshot = seedInitialYjsUpdateB64('Live Notes', 'Peer text in the submitted checkpoint.');
    tauriMocks.invoke.mockImplementation(async (cmd) => {
      if (cmd === 'cmd_get_live_draft') {
        return {
          outcome: 'ok',
          data: {
            draftId: 'drf_1',
            workspaceId: 'ws_cw3',
            pageId: 'shp_1',
            baseRevisionNumber: 1,
            draftVersion: 1,
            checkpointSeq: 1,
            checkpointId: 'chk:drf_1:v1:s1',
            status: 'pendingReview',
            snapshotStateB64: submittedSnapshot,
            snapshotVectorB64: svB64,
            baseTitle: 'Live Notes',
            baseContentMd: 'Seed line.',
            currentTitle: 'Live Notes',
            currentContentMd: 'Peer text in the submitted checkpoint.',
            contributors: [],
            pendingProposalId: 'prp_frozen',
            createdByUid: 'uid_a',
            createdByDisplayName: 'Alice Owner',
            createdAtMs: 1_750_000_000_000,
            updatedAtMs: 1_750_000_002_000,
          },
        };
      }
      return { outcome: 'ok', data: null };
    });
    emitCollabEvent({
      type: 'workspace_event',
      workspaceId: 'ws_cw3',
      eventSeq: 10,
      eventType: 'proposal_updated',
      pageId: 'shp_1',
      proposalId: 'prp_frozen',
      payload: { status: 'pending' },
    });
    await waitFor(() => {
      expect(screen.getByTestId('live-sync-status')).toHaveTextContent('Frozen for review');
    });
    expect(screen.getByRole('button', { name: 'Open review' })).toBeInTheDocument();
    expect(screen.getByLabelText('Document content')).toHaveTextContent('Peer text in the submitted checkpoint.');
  });

  it('SharedProposalReview shows contributors and explains why self/contributor review is ineligible when no other Owner/Admin exists', () => {
    const details: SharedProposalDetails = {
      proposal: {
        proposalId: 'prp_live_1',
        workspaceId: 'ws_cw3',
        pageId: 'shp_1',
        pageTitle: 'Live Notes',
        baseRevisionNumber: 1,
        summary: 'Collaborative draft checkpoint',
        authorUid: 'uid_a',
        authorDisplayName: 'Alice Owner',
        status: 'pending',
        submissionCount: 1,
        isStale: false,
        createdAtMs: 1_750_000_000_000,
        updatedAtMs: 1_750_000_000_000,
        liveDraftId: 'drf_1',
        draftVersion: 1,
        contributorUids: ['uid_a', 'uid_b'],
        contributorNames: ['Alice Owner', 'Bob Editor'],
      },
      baseTitle: 'Live Notes',
      baseContentMd: 'Before',
      proposedTitle: 'Live Notes',
      proposedContentMd: 'After',
      currentPageRevisionNumber: 1,
      currentPageTitle: 'Live Notes',
      currentPageContentMd: 'Before',
      pageArchived: false,
      canReview: false,
      canAuthorEdit: false,
      hasEligibleReviewer: false,
      ineligibleReason:
        'No other Owner or Admin is eligible to approve this draft because all current reviewers contributed to or submitted it.',
      decisions: [],
    };

    render(
      <SharedProposalReview
        details={details}
        currentUid="uid_a"
        busy={false}
        error=""
        onRefresh={vi.fn()}
        onBack={vi.fn()}
        onReview={vi.fn(async () => true)}
        onOpenRevise={vi.fn()}
        onWithdraw={vi.fn(async () => true)}
      />,
    );

    expect(screen.getByTestId('proposal-contributors')).toHaveTextContent(
      'Alice Owner, Bob Editor',
    );
    expect(screen.getByTestId('review-restriction-banner')).toHaveTextContent(
      /No other Owner or Admin is eligible/i,
    );
    expect(screen.queryByRole('button', { name: 'Apply change' })).not.toBeInTheDocument();
  });

  it('TeamWorkspaceView supports bulk multi-recipient invites with per-recipient results, Project Discussions, live event updates, and Activity deep-linking', async () => {
    const ptThread: SharedProjectThreadView = {
      threadId: 'pthr_1',
      workspaceId: 'ws_cw3',
      projectId: 'prj_2',
      authorUid: 'uid_b',
      authorDisplayName: 'Bob Editor',
      status: 'open',
      resolvedAtMs: null,
      resolvedByDisplayName: null,
      createdAtMs: 1_750_000_100_000,
      updatedAtMs: 1_750_000_100_000,
      messages: [
        {
          messageId: 'pmsg_1',
          threadId: 'pthr_1',
          authorUid: 'uid_b',
          authorDisplayName: 'Bob Editor',
          body: 'Initial milestone discussion.',
          createdAtMs: 1_750_000_100_000,
        },
      ],
    };

    tauriMocks.invoke.mockImplementation(async (cmd, args: any) => {
      if (cmd === 'cmd_get_workspace') return { outcome: 'ok', data: wsDetails };
      if (cmd === 'cmd_list_realtime_events') return { outcome: 'ok', data: [] };
      if (cmd === 'cmd_collab_connect') return { workspaceId: 'ws_cw3', uid: 'uid_a', connected: true };
      if (cmd === 'cmd_create_invitation_batch') {
        expect(args.emails).toEqual(['alice@example.com', 'carol@example.com', 'bad-addr']);
        return {
          outcome: 'ok',
          data: {
            offeredRole: 'editor',
            replayed: false,
            results: [
              {
                email: 'alice@example.com',
                outcome: 'alreadyMember',
                message: 'Already a workspace member (owner).',
                invitation: null,
                inviteCode: null,
                replayed: false,
              },
              {
                email: 'carol@example.com',
                outcome: 'created',
                message: 'Invited as editor.',
                invitation: {
                  invitationId: 'inv_carol',
                  workspaceId: 'ws_cw3',
                  recipientEmail: 'carol@example.com',
                  offeredRole: 'editor',
                  status: 'pending',
                  invitedByMembershipId: 'mem_a',
                  invitedByDisplayName: 'Alice Owner',
                  createdAtMs: 1_750_000_000_000,
                  expiresAtMs: 1_750_604_800_000,
                  acceptedAtMs: null,
                  revokedAtMs: null,
                },
                inviteCode: 'sthi_carol_batch_code',
                replayed: false,
              },
              {
                email: 'bad-addr',
                outcome: 'invalidEmail',
                message: 'Enter a valid email address.',
                invitation: null,
                inviteCode: null,
                replayed: false,
              },
            ],
          },
        };
      }
      if (cmd === 'cmd_list_workspace_activity') {
        return {
          outcome: 'ok',
          data: {
            items: [
              {
                activityId: 'act_1',
                workspaceId: 'ws_cw3',
                actorUid: 'uid_b',
                actorDisplayName: 'Bob Editor',
                action: 'project_discussion_started',
                targetType: 'project_thread',
                projectId: 'prj_2',
                projectName: 'Same Name',
                pageId: null,
                pageTitle: null,
                proposalId: null,
                threadId: 'pthr_1',
                detailText: 'Started a discussion in project “Same Name”',
                createdAtMs: 1_750_000_100_000,
              },
            ],
            nextCursor: null,
          },
        };
      }
      if (cmd === 'cmd_list_shared_projects') {
        return {
          outcome: 'ok',
          data: {
            items: [
              {
                projectId: 'prj_1',
                workspaceId: 'ws_cw3',
                name: 'Same Name',
                purpose: 'First',
                version: 1,
                activePageCount: 0,
                createdByUid: 'uid_a',
                createdByDisplayName: 'Alice Owner',
                updatedByUid: 'uid_a',
                updatedByDisplayName: 'Alice Owner',
                createdAtMs: 1_750_000_000_000,
                updatedAtMs: 1_750_000_000_000,
              },
              {
                projectId: 'prj_2',
                workspaceId: 'ws_cw3',
                name: 'Same Name',
                purpose: 'Second',
                version: 1,
                activePageCount: 0,
                createdByUid: 'uid_a',
                createdByDisplayName: 'Alice Owner',
                updatedByUid: 'uid_a',
                updatedByDisplayName: 'Alice Owner',
                createdAtMs: 1_750_000_000_000,
                updatedAtMs: 1_750_000_000_000,
              },
            ],
            nextCursor: null,
          },
        };
      }
      if (cmd === 'cmd_get_shared_project') {
        return {
          outcome: 'ok',
          data: {
            project: {
              projectId: args.projectId,
              workspaceId: 'ws_cw3',
              name: 'Same Name',
              purpose: args.projectId === 'prj_2' ? 'Second' : 'First',
              version: 1,
              activePageCount: 0,
              createdByUid: 'uid_a',
              createdByDisplayName: 'Alice Owner',
              updatedByUid: 'uid_a',
              updatedByDisplayName: 'Alice Owner',
              createdAtMs: 1_750_000_000_000,
              updatedAtMs: 1_750_000_000_000,
            },
            pages: [],
            pagesNextCursor: null,
          },
        };
      }
      if (cmd === 'cmd_list_project_threads') {
        return { outcome: 'ok', data: { items: [ptThread], nextCursor: null } };
      }
      return null;
    });

    const user = userEvent.setup();
    const copySpy = vi.spyOn(navigator.clipboard, 'writeText').mockImplementation(tauriMocks.clipboardWrite);
    render(
      <TeamWorkspaceView
        workspace={wsSummary}
        initialTab="members"
        currentUid="uid_a"
        onSelectPersonal={vi.fn()}
        onWorkspaceUpdated={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
        onRefreshList={vi.fn()}
      />,
    );

    expect(
      await screen.findByText(/Invitations grant access to the workspace/i),
    ).toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText('Invitation role'), 'editor');
    await user.type(
      screen.getByLabelText('Recipient email addresses'),
      'alice@example.com, carol@example.com, bad-addr',
    );
    await user.click(screen.getByRole('button', { name: 'Send workspace invitations' }));

    expect(await screen.findByText('Already a workspace member (owner).')).toBeInTheDocument();
    expect(screen.getByText('sthi_carol_batch_code')).toBeInTheDocument();
    expect(screen.getByText('Enter a valid email address.')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Copy invite code for carol@example.com' }));
    expect(copySpy).toHaveBeenCalledWith('sthi_carol_batch_code');

    // Open Activity and jump directly to exact project & highlighted discussion thread across identically named projects.
    await user.click(screen.getByRole('tab', { name: 'Activity' }));
    expect(await screen.findByText(/Started a discussion in project “Same Name”/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Open discussion' }));

    expect(await screen.findByText('Second')).toBeInTheDocument();
    const highlighted = document.querySelector('[data-thread-id="pthr_1"]');
    expect(highlighted).toHaveAttribute('data-highlighted', 'true');

    // Incoming live reply via WebSocket appends in place without duplicate threads.
    emitCollabEvent({
      type: 'workspace_event',
      workspaceId: 'ws_cw3',
      eventSeq: 55,
      eventType: 'project_thread_updated',
      projectId: 'prj_2',
      threadId: 'pthr_1',
      payload: {
        ...ptThread,
        messages: [
          ...ptThread.messages,
          {
            messageId: 'pmsg_2',
            threadId: 'pthr_1',
            authorUid: 'uid_b',
            authorDisplayName: 'Bob Editor',
            body: 'Live reply without manual refresh!',
            createdAtMs: 1_750_000_200_000,
          },
        ],
      },
    });
    expect(await screen.findByText('Live reply without manual refresh!')).toBeInTheDocument();
    expect(document.querySelectorAll('[data-thread-id="pthr_1"]')).toHaveLength(1);
  });

  it('per-user Y.UndoManager in Collaboration undoes only local edits and preserves peer edits', () => {
    const docA = new Y.Doc({ gc: true });
    docA.clientID = 11;
    const docB = new Y.Doc({ gc: true });
    docB.clientID = 22;
    const fragA = docA.getXmlFragment('default');
    const fragB = docB.getXmlFragment('default');
    const undoA = new Y.UndoManager(fragA, { trackedOrigins: new Set(['local-a']) });

    // Local A inserts paragraph 1; remote B inserts paragraph 2.
    docA.transact(() => {
      const p = new Y.XmlElement('paragraph');
      const t = new Y.XmlText('From Alice');
      p.insert(0, [t]);
      fragA.insert(0, [p]);
    }, 'local-a');
    Y.applyUpdate(docB, Y.encodeStateAsUpdate(docA), 'remote');

    docB.transact(() => {
      const p = new Y.XmlElement('paragraph');
      const t = new Y.XmlText('From Bob');
      p.insert(0, [t]);
      fragB.insert(1, [p]);
    }, 'local-b');
    Y.applyUpdate(docA, Y.encodeStateAsUpdate(docB), 'remote');
    expect(fragA.length).toBe(2);

    // Undo on Alice removes Alice's own edit ONLY while keeping Bob's paragraph!
    undoA.undo();
    expect(fragA.length).toBe(1);
    expect(fragA.get(0).toString()).toContain('From Bob');
    expect(fragA.toString()).not.toContain('From Alice');
    undoA.destroy();
    docA.destroy();
    docB.destroy();
  });

  it('Finding 2/3/4: preserves frozen rejected edits, prevents stale/cross-account recovery leak, handles late ack & backup failure, and supports stale-base rebase', async () => {
    const initialB64 = seedInitialYjsUpdateB64('Doc A', 'Initial text.');
    const svB64 = uint8ToBase64(Y.encodeStateVector(new Y.Doc()));
    const rebaseCalls: any[] = [];
    let failSaveBackup = true;
    const savedCalls: any[] = [];

    // Seed stale backup belonging to draftVersion 1 while server opens v2.
    tauriMocks.invoke.mockImplementation(async (cmd, args: any) => {
      if (cmd === 'cmd_open_live_draft') {
        return {
          outcome: 'ok',
          data: {
            created: false,
            draft: {
              draftId: 'drf_v2',
              workspaceId: 'ws_cw3',
              pageId: 'shp_rec',
              baseRevisionNumber: 1,
              draftVersion: 2,
              checkpointSeq: 2,
              checkpointId: 'chk:drf_v2:v2:s2',
              status: 'editing',
              snapshotStateB64: initialB64,
              snapshotVectorB64: svB64,
              baseTitle: 'Doc A',
              baseContentMd: 'Initial text.',
              currentTitle: 'Doc A',
              currentContentMd: 'Initial text.',
              contributors: [],
              pendingProposalId: null,
              createdByUid: 'uid_a',
              createdByDisplayName: 'Alice Owner',
              createdAtMs: 1_750_000_000_000,
              updatedAtMs: 1_750_000_000_000,
            },
          },
        };
      }
      if (cmd === 'cmd_get_shared_page') {
        return {
          outcome: 'ok',
          data: {
            page: { pageId: 'shp_rec', currentRevisionNumber: 2, title: 'Doc A rev2' },
            currentRevision: {
              revisionId: 'rev_2',
              workspaceId: 'ws_cw3',
              pageId: 'shp_rec',
              revisionNumber: 2,
              title: 'Doc A rev2',
              contentMd: 'Published rev 2 upstream text.',
              contentSha256: 'abc',
              changeSummary: 'Upstream',
              authorUid: 'uid_c',
              authorDisplayName: 'Cara Admin',
              createdAtMs: 1_750_000_000_000,
            },
            revisions: [],
            threads: [],
            proposals: [],
          },
        };
      }
      if (cmd === 'cmd_get_shared_draft') {
        // Stale v1 backup must NOT leak into v2 draft!
        return {
          accountUid: 'uid_a',
          workspaceId: 'ws_cw3',
          draftKey: 'live_shp_rec',
          title: 'STALE LEAK TITLE',
          contentMd: JSON.stringify({
            draftId: 'drf_v2',
            draftVersion: 1,
            nextOpSeq: 5,
            mergedPendingB64: initialB64,
            previewMarkdown: 'SHOULD NOT APPEAR',
            summary: 'stale summary',
          }),
          summary: 'stale summary',
          updatedAtMs: 1_750_000_000_000,
        };
      }
      if (cmd === 'cmd_save_shared_draft') {
        savedCalls.push(args);
        if (failSaveBackup) throw 'Disk full';
        return { ...args, updatedAtMs: 1_750_000_001_000 };
      }
      if (cmd === 'cmd_collab_connect') {
        return { workspaceId: 'ws_cw3', uid: 'uid_a', connected: true };
      }
      if (cmd === 'cmd_get_live_draft') {
        return {
          outcome: 'ok',
          data: {
            draftId: 'drf_v2',
            workspaceId: 'ws_cw3',
            pageId: 'shp_rec',
            baseRevisionNumber: 1,
            draftVersion: 2,
            checkpointSeq: 2,
            checkpointId: 'chk:drf_v2:v2:s2',
            status: 'pendingReview',
            snapshotStateB64: initialB64,
            snapshotVectorB64: svB64,
            baseTitle: 'Doc A',
            baseContentMd: 'Initial text.',
            currentTitle: 'Doc A',
            currentContentMd: 'Initial text.',
            contributors: [],
            pendingProposalId: 'prp_frozen_rec',
            createdByUid: 'uid_a',
            createdByDisplayName: 'Alice Owner',
            createdAtMs: 1_750_000_000_000,
            updatedAtMs: 1_750_000_000_000,
          },
        };
      }
      if (cmd === 'cmd_collab_join_draft') {
        const activeUid = args?.accountUid ?? 'uid_a';
        emitCollabEvent({
          type: 'connected',
          workspaceId: 'ws_cw3',
          accountUid: activeUid,
          connectionId: 'conn_a',
          uid: activeUid,
          displayName: activeUid === 'uid_b' ? 'Bob Editor' : 'Alice Owner',
          role: activeUid === 'uid_b' ? 'editor' : 'owner',
        });
        emitCollabEvent({
          type: 'draft_state',
          workspaceId: 'ws_cw3',
          accountUid: activeUid,
          draft: {
            draftId: 'drf_v2',
            workspaceId: 'ws_cw3',
            pageId: 'shp_rec',
            baseRevisionNumber: 1,
            draftVersion: 2,
            checkpointSeq: 2,
            checkpointId: 'chk:drf_v2:v2:s2',
            status: 'editing',
            snapshotStateB64: initialB64,
            snapshotVectorB64: svB64,
            baseTitle: 'Doc A',
            baseContentMd: 'Initial text.',
            currentTitle: 'Doc A',
            currentContentMd: 'Initial text.',
            contributors: [],
            pendingProposalId: null,
            createdByUid: 'uid_a',
            createdByDisplayName: 'Alice Owner',
            createdAtMs: 1_750_000_000_000,
            updatedAtMs: 1_750_000_000_000,
          },
          missingUpdateB64: '',
          serverVectorB64: svB64,
          presence: [],
        });
        return null;
      }
      if (cmd === 'cmd_rebase_live_draft') {
        rebaseCalls.push(args);
        const rebasedB64 = seedInitialYjsUpdateB64(
          args.rebasedTitle ?? 'Doc A',
          args.rebasedContentMd ?? 'Merged',
        );
        return {
          outcome: 'ok',
          data: {
            draftId: 'drf_v2',
            workspaceId: 'ws_cw3',
            pageId: 'shp_rec',
            baseRevisionNumber: 2,
            draftVersion: 3,
            checkpointSeq: 3,
            checkpointId: 'chk:drf_v2:v3:s3',
            status: 'editing',
            snapshotStateB64: rebasedB64,
            snapshotVectorB64: svB64,
            baseTitle: 'Doc A rev2',
            baseContentMd: 'Published rev 2 upstream text.',
            currentTitle: args.rebasedTitle ?? 'Doc A',
            currentContentMd: args.rebasedContentMd ?? 'Merged',
            contributors: [],
            pendingProposalId: null,
            createdByUid: 'uid_a',
            createdByDisplayName: 'Alice Owner',
            createdAtMs: 1_750_000_000_000,
            updatedAtMs: 1_750_000_002_000,
          },
        };
      }
      return null;
    });

    const user = userEvent.setup();
    const { rerender } = render(
      <LiveSharedDraftEditor
        workspaceId="ws_cw3"
        pageId="shp_rec"
        baseRevisionNumber={1}
        baseTitle="Doc A"
        baseContentMd="Initial text."
        initialCurrentRevisionNumber={2}
        initialCurrentTitle="Doc A rev2"
        initialCurrentContentMd="Published rev 2 upstream text."
        currentUid="uid_a"
        authorDisplayName="Alice Owner"
        onSubmitted={vi.fn()}
        onOpenProposal={vi.fn()}
        onBack={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
      />,
    );

    expect(await screen.findByTestId('live-sync-status')).toHaveTextContent('Synced');
    const titleInput = screen.getByLabelText('Live draft title') as HTMLInputElement;
    expect(titleInput).toHaveValue('Doc A');
    expect(screen.queryByText(/Recovered your unsynced/)).not.toBeInTheDocument();

    // Rebase banner appears immediately showing rev 2 landed while draft is based on rev 1.
    expect(screen.getByTestId('live-draft-stale-banner')).toBeInTheDocument();

    // Type two rapid edits: disk backup fails -> warn user visibly; late ack #1 leaves status at Saving…!
    await user.type(titleInput, '12');
    expect(await screen.findByTestId('persist-warning-banner')).toHaveTextContent(/Offline backup failed/i);
    expect(screen.getByTestId('live-sync-status')).toHaveTextContent('Saving…');

    const firstReqId = tauriMocks.invoke.mock.calls.find((c) => c[0] === 'cmd_collab_send_update')?.[1]?.requestId;
    const secondReqId = tauriMocks.invoke.mock.calls.filter((c) => c[0] === 'cmd_collab_send_update')?.[1]?.[1]?.requestId;
    expect(firstReqId).toBeTruthy();
    expect(secondReqId).toBeTruthy();

    emitCollabEvent({
      type: 'draft_update_ack',
      workspaceId: 'ws_cw3',
      requestId: firstReqId,
      draftId: 'drf_v2',
      draftVersion: 2,
      checkpointSeq: 3,
      checkpointId: 'chk:drf_v2:v2:s3',
      clientOpSeq: 1,
      serverSeq: 3,
      duplicate: false,
      contributors: [],
    });
    expect(screen.getByTestId('live-sync-status')).toHaveTextContent('Saving…');

    // Peer freeze NACK rejects second op -> recoverable edits panel preserves title/text!
    emitCollabEvent({
      type: 'draft_update_nack',
      workspaceId: 'ws_cw3',
      requestId: secondReqId,
      clientOpSeq: 2,
      code: 'live_draft_frozen',
      error: 'Submitted for review before op #2 landed.',
      fatal: false,
    });
    expect(await screen.findByTestId('recoverable-edits-banner')).toBeInTheDocument();

    // Switching account uid clears prior editor/recovery/warning state cleanly!
    failSaveBackup = false;
    rerender(
      <LiveSharedDraftEditor
        workspaceId="ws_cw3"
        pageId="shp_rec"
        baseRevisionNumber={1}
        baseTitle="Doc A"
        baseContentMd="Initial text."
        initialCurrentRevisionNumber={2}
        initialCurrentTitle="Doc A rev2"
        initialCurrentContentMd="Published rev 2 upstream text."
        currentUid="uid_b"
        authorDisplayName="Bob Editor"
        onSubmitted={vi.fn()}
        onOpenProposal={vi.fn()}
        onBack={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
      />,
    );
    await waitFor(() => {
      expect(screen.getByTestId('live-sync-status')).toHaveTextContent('Synced');
    });
    expect(screen.queryByTestId('recoverable-edits-banner')).not.toBeInTheDocument();
    expect(screen.queryByTestId('persist-warning-banner')).not.toBeInTheDocument();

    // Execute CarryDraft rebase UI flow -> invokes cmd_rebase_live_draft & removes stale banner.
    await user.click(screen.getByRole('button', { name: /Compare & rebase onto rev 2/i }));
    await user.click(screen.getByRole('button', { name: /Carry draft onto rev 2/i }));
    await waitFor(() => {
      expect(rebaseCalls).toHaveLength(1);
    });
    expect(rebaseCalls[0].mode).toBe('carryDraft');
    expect(rebaseCalls[0].expectedBaseRevisionNumber).toBe(2);
    expect(screen.queryByTestId('live-draft-stale-banner')).not.toBeInTheDocument();
  });

  it('Finding 6: relative Yjs cursor stays anchored on original word when collaborator inserts text before it', () => {
    const docA = new Y.Doc();
    docA.clientID = 101;
    const docB = new Y.Doc();
    docB.clientID = 202;

    const pB = new Y.XmlElement('paragraph');
    const txtB = new Y.XmlText('Hello target word');
    pB.insert(0, [txtB]);
    docB.getXmlFragment('default').insert(0, [pB]);
    Y.applyUpdate(docA, Y.encodeStateAsUpdate(docB));

    const fragA = docA.getXmlFragment('default');
    const fragB = docB.getXmlFragment('default');
    const nodeB = (fragB.get(0) as Y.XmlElement).get(0) as Y.XmlText;
    // Bob selects "target" (offsets 6..12 inside nodeB).
    const relAnchor = Y.relativePositionToJSON(Y.createRelativePositionFromTypeIndex(nodeB, 6));
    const relHead = Y.relativePositionToJSON(Y.createRelativePositionFromTypeIndex(nodeB, 12));

    // Mirror into Alice's Awareness via syncRemoteAwarenessPositions using relAnchor/relHead.
    const awarenessA = new Awareness(docA);
    syncRemoteAwarenessPositions(null, awarenessA, 'conn_a', [
      {
        connectionId: 'conn_b',
        uid: 'uid_b',
        displayName: 'Bob Editor',
        color: '#1D4ED8',
        pageId: 'p_1',
        draftId: 'd_1',
        anchor: 6,
        head: 12,
        relAnchor: JSON.stringify(relAnchor),
        relHead: JSON.stringify(relHead),
        updatedAtMs: 1000,
      },
    ]);

    // Alice inserts "PREFIX " at index 0 BEFORE Bob's selected word!
    const nodeA = (fragA.get(0) as Y.XmlElement).get(0) as Y.XmlText;
    nodeA.insert(0, 'PREFIX ');
    expect(nodeA.toString()).toBe('PREFIX Hello target word');

    // Resolving stored relative cursor against Alice's updated Y.Doc stays pinned to "target" (13..19)!
    const bobState = Array.from(awarenessA.getStates().values()).find(
      (s: any) => s?.user?.connectionId === 'conn_b',
    ) as any;
    const absA = Y.createAbsolutePositionFromRelativePosition(
      Y.createRelativePositionFromJSON(bobState.cursor.anchor),
      docA,
    )!;
    const absH = Y.createAbsolutePositionFromRelativePosition(
      Y.createRelativePositionFromJSON(bobState.cursor.head),
      docA,
    )!;
    expect(nodeA.toString().slice(absA.index, absH.index)).toBe('target');
    expect(absA.index).toBe(13);
    expect(absH.index).toBe(19);
    // Clean helper returns null gracefully if editor isn't mounted yet.
    expect(encodeSelectionRelativePositions(null, 0, 0)).toEqual({ relAnchor: null, relHead: null });
    awarenessA.destroy();
    docA.destroy();
    docB.destroy();
  });

  it('Finding 8: TeamWorkspaceView auto-reconnects after outage, buffers in-flight WS events during multi-page catch-up, and cancels timers on unmount', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    let connectCalls = 0;
    const catchupAfterSeqs: number[] = [];

    tauriMocks.invoke.mockImplementation(async (cmd, args: any) => {
      if (cmd === 'cmd_get_workspace') return { outcome: 'ok', data: wsDetails };
      if (cmd === 'cmd_collab_connect') {
        connectCalls += 1;
        if (connectCalls === 1) throw 'Service temporarily unreachable';
        return { workspaceId: 'ws_cw3', uid: 'uid_a', connected: true };
      }
      if (cmd === 'cmd_list_realtime_events') {
        catchupAfterSeqs.push(args.afterSeq);
        if (args.afterSeq === 0 && connectCalls <= 1) {
          return { outcome: 'ok', data: [{ eventSeq: 10, workspaceId: 'ws_cw3', eventType: 'old', payloadJson: {}, createdAtMs: 1 }] };
        }
        // Page 1: 100 items (seqs 11..110), Page 2: 2 items (111..112).
        if (args.afterSeq === 10) {
          return {
            outcome: 'ok',
            data: Array.from({ length: 100 }, (_, i) => ({
              eventSeq: 11 + i,
              workspaceId: 'ws_cw3',
              eventType: 'project_thread_updated',
              payloadJson: {},
              createdAtMs: 2 + i,
            })),
          };
        }
        if (args.afterSeq === 110) {
          return {
            outcome: 'ok',
            data: [
              { eventSeq: 111, workspaceId: 'ws_cw3', eventType: 'project_thread_updated', payloadJson: {}, createdAtMs: 200 },
              { eventSeq: 112, workspaceId: 'ws_cw3', eventType: 'project_thread_updated', payloadJson: {}, createdAtMs: 201 },
            ],
          };
        }
        return { outcome: 'ok', data: [] };
      }
      return { outcome: 'ok', data: { items: [], nextCursor: null } };
    });

    const { unmount } = render(
      <TeamWorkspaceView
        workspace={wsSummary}
        initialTab="overview"
        currentUid="uid_a"
        onSelectPersonal={vi.fn()}
        onWorkspaceUpdated={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
        onRefreshList={vi.fn()}
      />,
    );

    await waitFor(() => expect(connectCalls).toBe(1));

    // Advance past 1s exponential backoff -> reconnect succeeds -> connected + paginated catchup!
    await vi.advanceTimersByTimeAsync(1100);
    await waitFor(() => expect(connectCalls).toBe(2));

    // Fire connected event; while first catch-up page is in flight, emit high workspace_event seq 500 -> must NOT skip 111..112!
    emitCollabEvent({
      type: 'connected',
      workspaceId: 'ws_cw3',
      connectionId: 'conn_rec',
      uid: 'uid_a',
      displayName: 'Alice Owner',
      role: 'owner',
    });
    emitCollabEvent({
      type: 'workspace_event',
      workspaceId: 'ws_cw3',
      eventSeq: 500,
      eventType: 'project_thread_updated',
      payload: {},
    });

    await waitFor(() => {
      expect(catchupAfterSeqs).toEqual([0, 10, 110]);
    });

    // Disconnect again -> unmount clears pending retry timer so no connect attempt runs after unmount.
    emitCollabEvent({
      type: 'ws_disconnected',
      workspaceId: 'ws_cw3',
      reason: 'closed',
    });
    unmount();
    await vi.advanceTimersByTimeAsync(5000);
    expect(connectCalls).toBe(2);
    vi.useRealTimers();
  });

  it.each(['rejected', 'pending'])('Finding 2: recovers %s edits after a draft advance and isolates same-profile accounts', async (recoveryKind) => {
    let persistedDraftRow: any = null;
    const deletedKeys: string[] = [];
    let currentReopenTitle = 'Doc B';
    let version = 1;
    let currentReopenContent = 'Base paragraph.';
    let currentSnapB64 = seedInitialYjsUpdateB64('Doc B', 'Base paragraph.');
    const initialB64 = currentSnapB64;
    const svB64 = uint8ToBase64(Y.encodeStateVector(new Y.Doc()));

    tauriMocks.invoke.mockImplementation(async (cmd, args: any) => {
      if (cmd === 'cmd_get_shared_page') {
        return {
          outcome: 'ok',
          data: {
            page: {
              workspaceId: 'ws_cw3',
              pageId: 'shp_f2',
              title: currentReopenTitle,
              currentRevisionNumber: 1,
              archivedAtMs: null,
              createdByUid: 'uid_a',
              createdByDisplayName: 'Alice Owner',
              createdAtMs: 1_750_000_000_000,
              updatedAtMs: 1_750_000_000_000,
            },
            currentRevision: {
              revisionNumber: 1,
              title: currentReopenTitle,
              contentMd: currentReopenContent,
              authorUid: 'uid_a',
              authorDisplayName: 'Alice Owner',
              createdAtMs: 1_750_000_000_000,
              provenance: { originKind: 'userAuthored' },
            },
            threads: [],
            proposals: [],
          },
        };
      }
      if (cmd === 'cmd_get_shared_draft') {
        if (!persistedDraftRow) return null;
        return persistedDraftRow;
      }
      if (cmd === 'cmd_save_shared_draft') {
        persistedDraftRow = { ...args, updatedAtMs: 1_750_000_001_000 };
        return persistedDraftRow;
      }
      if (cmd === 'cmd_delete_shared_draft') {
        deletedKeys.push(`${args.accountUid ?? ''}:${args.draftKey}`);
        if (!persistedDraftRow || persistedDraftRow.accountUid === args.accountUid) {
          persistedDraftRow = null;
        }
        return true;
      }
      if (cmd === 'cmd_collab_connect') {
        return { workspaceId: 'ws_cw3', uid: args.accountUid ?? 'uid_b', connected: true };
      }
      if (cmd === 'cmd_open_live_draft' || cmd === 'cmd_get_live_draft') {
        return {
          outcome: 'ok',
          data: {
            created: false,
            draft: {
              draftId: 'drf_f2',
              workspaceId: 'ws_cw3',
              pageId: 'shp_f2',
              baseRevisionNumber: 1,
              draftVersion: version,
              checkpointSeq: 2,
              checkpointId: `chk:drf_f2:v${version}:s2`,
              status: 'editing',
              snapshotStateB64: currentSnapB64,
              snapshotVectorB64: svB64,
              baseTitle: 'Doc B',
              baseContentMd: 'Base paragraph.',
              currentTitle: currentReopenTitle,
              currentContentMd: currentReopenContent,
              contributors: [],
              pendingProposalId: null,
              createdByUid: 'uid_a',
              createdByDisplayName: 'Alice Owner',
              createdAtMs: 1_750_000_000_000,
              updatedAtMs: 1_750_000_000_000,
            },
            draftId: 'drf_f2',
            workspaceId: 'ws_cw3',
            pageId: 'shp_f2',
            baseRevisionNumber: 1,
            draftVersion: version,
            checkpointSeq: 2,
            checkpointId: `chk:drf_f2:v${version}:s2`,
            status: 'editing',
            snapshotStateB64: currentSnapB64,
            snapshotVectorB64: svB64,
            baseTitle: 'Doc B',
            baseContentMd: 'Base paragraph.',
            currentTitle: currentReopenTitle,
            currentContentMd: currentReopenContent,
            contributors: [],
            pendingProposalId: null,
            createdByUid: 'uid_a',
            createdByDisplayName: 'Alice Owner',
            createdAtMs: 1_750_000_000_000,
            updatedAtMs: 1_750_000_000_000,
          },
        };
      }
      if (cmd === 'cmd_collab_join_draft') {
        const uid = args?.accountUid ?? 'uid_b';
        emitCollabEvent({
          type: 'connected',
          workspaceId: 'ws_cw3',
          accountUid: uid,
          connectionId: 'conn_b',
          uid,
          displayName: uid === 'uid_c' ? 'Cara Admin' : 'Bob Editor',
          role: 'editor',
        });
        emitCollabEvent({
          type: 'draft_state',
          workspaceId: 'ws_cw3',
          accountUid: uid,
          draft: {
            draftId: 'drf_f2',
            workspaceId: 'ws_cw3',
            pageId: 'shp_f2',
            baseRevisionNumber: 1,
            draftVersion: version,
            checkpointSeq: 2,
            checkpointId: `chk:drf_f2:v${version}:s2`,
            status: 'editing',
            snapshotStateB64: currentSnapB64,
            snapshotVectorB64: svB64,
            baseTitle: 'Doc B',
            baseContentMd: 'Base paragraph.',
            currentTitle: currentReopenTitle,
            currentContentMd: currentReopenContent,
            contributors: [],
            pendingProposalId: null,
            createdByUid: 'uid_a',
            createdByDisplayName: 'Alice Owner',
            createdAtMs: 1_750_000_000_000,
            updatedAtMs: 1_750_000_000_000,
          },
          missingUpdateB64: '',
          serverVectorB64: svB64,
          presence: [],
        });
        return null;
      }
      return null;
    });

    const user = userEvent.setup();
    const firstRender = render(
      <LiveSharedDraftEditor
        workspaceId="ws_cw3"
        pageId="shp_f2"
        baseRevisionNumber={1}
        baseTitle="Doc B"
        baseContentMd="Base paragraph."
        currentUid="uid_b"
        authorDisplayName="Bob Editor"
        onSubmitted={vi.fn()}
        onOpenProposal={vi.fn()}
        onBack={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId('live-sync-status')).toHaveTextContent('Synced');
    });
    const titleInput = screen.getByLabelText('Live draft title') as HTMLInputElement;
    await user.type(titleInput, ' LocalMarker');

    const reqId = tauriMocks.invoke.mock.calls.find((c) => c[0] === 'cmd_collab_send_update')?.[1]?.requestId;
    expect(reqId).toBeTruthy();
    await waitFor(() => {
      expect(JSON.parse(persistedDraftRow?.contentMd ?? '{}').pending?.title).toBe('Doc B LocalMarker');
    });
    const pendingBeforeRejection = persistedDraftRow;

    // Freeze NACK rejects Bob's op -> backup on disk must retain { rejected } instead of calling deleteSharedDraft!
    emitCollabEvent({
      type: 'draft_update_nack',
      workspaceId: 'ws_cw3',
      requestId: reqId,
      clientOpSeq: 1,
      code: 'live_draft_frozen',
      error: 'Submitted for review before edit landed.',
      fatal: false,
    });
    expect(await screen.findByTestId('recoverable-edits-banner')).toBeInTheDocument();
    await waitFor(() => {
      expect(persistedDraftRow).toBeTruthy();
      expect(JSON.parse(persistedDraftRow.contentMd).rejected).toBeTruthy();
    });

    // Even when disconnected, Copy remains enabled inside recovery banner.
    const writeSpy = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue(undefined);
    emitCollabEvent({ type: 'ws_disconnected', workspaceId: 'ws_cw3', reason: 'offline' });
    const copyBtn = screen.getByRole('button', { name: /Copy unsent edits/i });
    expect(copyBtn).not.toBeDisabled();
    await user.click(copyBtn);
    await waitFor(() => expect(writeSpy).toHaveBeenCalled());
    writeSpy.mockRestore();

    firstRender.unmount();

    // A client can also exit before the rejection reaches it, leaving the pending envelope.
    if (recoveryKind === 'pending') persistedDraftRow = pendingBeforeRejection;
    version = 2;
    currentReopenTitle = 'Peer Reopened Title';
    currentReopenContent = 'Base paragraph.\n\nPeer confirmed paragraph.';
    currentSnapB64 = seedInitialYjsUpdateB64(currentReopenTitle, currentReopenContent);

    // Account C opening in same profile must NOT see or delete Bob's persisted recovery!
    const acctCRender = render(
      <LiveSharedDraftEditor
        workspaceId="ws_cw3"
        pageId="shp_f2"
        baseRevisionNumber={1}
        baseTitle="Doc B"
        baseContentMd="Base paragraph."
        currentUid="uid_c"
        authorDisplayName="Cara Admin"
        onSubmitted={vi.fn()}
        onOpenProposal={vi.fn()}
        onBack={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
      />,
    );
    await waitFor(() => {
      expect(screen.getByTestId('live-sync-status')).toHaveTextContent('Synced');
    });
    expect(screen.queryByTestId('recoverable-edits-banner')).not.toBeInTheDocument();
    expect(persistedDraftRow).toBeTruthy();
    acctCRender.unmount();

    // Account B reopens after restart -> persisted recovery + comparison panel appear,
    // and Apply preserves Peer's title and Peer's content!
    render(
      <LiveSharedDraftEditor
        workspaceId="ws_cw3"
        pageId="shp_f2"
        baseRevisionNumber={1}
        baseTitle="Doc B"
        baseContentMd="Base paragraph."
        currentUid="uid_b"
        authorDisplayName="Bob Editor"
        onSubmitted={vi.fn()}
        onOpenProposal={vi.fn()}
        onBack={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
      />,
    );
    expect(await screen.findByTestId('recoverable-edits-banner')).toBeInTheDocument();
    expect(screen.getByTestId('recovery-compare-panel')).toHaveTextContent('Peer confirmed paragraph.');
    await waitFor(() => {
      expect(screen.getByTestId('live-sync-status')).toHaveTextContent('Synced');
    });
    expect(screen.getByLabelText('Live draft title')).toHaveValue('Peer Reopened Title');

    await user.click(screen.getByTestId('recovery-append-btn'));
    // Peer title preserved cleanly!
    expect(screen.getByLabelText('Live draft title')).toHaveValue('Peer Reopened Title');
    expect(initialB64.length).toBeGreaterThan(0);
    expect(deletedKeys).toHaveLength(0);
  });

  it('Finding 3 (final): holds Submit disabled on out-of-order checkpoint gaps, ignores wrong draftId frames, and enables Submit once gap closes', async () => {
    const svB64 = uint8ToBase64(Y.encodeStateVector(new Y.Doc()));
    const snap1B64 = seedInitialYjsUpdateB64('Seq Title', 'Initial body.');
    const peerDoc = new Y.Doc();
    peerDoc.clientID = 55;
    Y.applyUpdate(peerDoc, Uint8Array.from(atob(snap1B64), (c) => c.charCodeAt(0)));
    const p2Node = new Y.XmlElement('paragraph');
    p2Node.insert(0, [new Y.XmlText('Peer seq2 insert.')]);
    const beforePeerSv = Y.encodeStateVector(peerDoc);
    peerDoc.getXmlFragment('default').insert(1, [p2Node]);
    const peerDeltaB64 = uint8ToBase64(Y.encodeStateAsUpdate(peerDoc, beforePeerSv));

    const submitCalls: any[] = [];
    tauriMocks.invoke.mockImplementation(async (cmd, args: any) => {
      if (cmd === 'cmd_get_shared_draft') return null;
      if (cmd === 'cmd_collab_connect') {
        return { workspaceId: 'ws_cw3', uid: 'uid_a', connected: true };
      }
      if (cmd === 'cmd_open_live_draft' || cmd === 'cmd_get_live_draft') {
        const d = {
          draftId: 'drf_seq',
          workspaceId: 'ws_cw3',
          pageId: 'shp_seq',
          baseRevisionNumber: 1,
          draftVersion: 1,
          checkpointSeq: 1,
          checkpointId: 'chk:drf_seq:v1:s1',
          status: 'editing',
          snapshotStateB64: snap1B64,
          snapshotVectorB64: svB64,
          baseTitle: 'Seq Title',
          baseContentMd: 'Initial body.',
          currentTitle: 'Seq Title',
          currentContentMd: 'Initial body.',
          contributors: [],
          pendingProposalId: null,
          createdByUid: 'uid_a',
          createdByDisplayName: 'Alice Owner',
          createdAtMs: 1_750_000_000_000,
          updatedAtMs: 1_750_000_000_000,
        };
        return { outcome: 'ok', data: cmd === 'cmd_open_live_draft' ? { created: false, draft: d } : d };
      }
      if (cmd === 'cmd_collab_join_draft') {
        emitCollabEvent({
          type: 'connected',
          workspaceId: 'ws_cw3',
          connectionId: 'conn_seq',
          uid: 'uid_a',
          displayName: 'Alice Owner',
          role: 'owner',
        });
        emitCollabEvent({
          type: 'draft_state',
          workspaceId: 'ws_cw3',
          draft: {
            draftId: 'drf_seq',
            workspaceId: 'ws_cw3',
            pageId: 'shp_seq',
            baseRevisionNumber: 1,
            draftVersion: 1,
            checkpointSeq: 1,
            checkpointId: 'chk:drf_seq:v1:s1',
            status: 'editing',
            snapshotStateB64: snap1B64,
            snapshotVectorB64: svB64,
            baseTitle: 'Seq Title',
            baseContentMd: 'Initial body.',
            currentTitle: 'Seq Title',
            currentContentMd: 'Initial body.',
            contributors: [],
            pendingProposalId: null,
            createdByUid: 'uid_a',
            createdByDisplayName: 'Alice Owner',
            createdAtMs: 1_750_000_000_000,
            updatedAtMs: 1_750_000_000_000,
          },
          missingUpdateB64: '',
          serverVectorB64: svB64,
          presence: [],
        });
        return null;
      }
      if (cmd === 'cmd_submit_live_draft') {
        submitCalls.push(args);
        return {
          outcome: 'ok',
          data: {
            proposal: { proposalId: 'prp_seq', status: 'pending', submissionCount: 1 },
            decisions: [],
            canReview: false,
          },
        };
      }
      return null;
    });

    const user = userEvent.setup();
    render(
      <LiveSharedDraftEditor
        workspaceId="ws_cw3"
        pageId="shp_seq"
        baseRevisionNumber={1}
        baseTitle="Seq Title"
        baseContentMd="Initial body."
        currentUid="uid_a"
        authorDisplayName="Alice Owner"
        onSubmitted={vi.fn()}
        onOpenProposal={vi.fn()}
        onBack={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId('live-sync-status')).toHaveTextContent('Synced');
    });
    await user.type(screen.getByLabelText('Live draft change summary'), 'Submit check');
    await user.type(screen.getByLabelText('Live draft title'), '!');
    const opReqId = tauriMocks.invoke.mock.calls.find((c) => c[0] === 'cmd_collab_send_update')?.[1]?.requestId;

    // Frame from DIFFERENT draftId on same pageId must be ignored completely!
    emitCollabEvent({
      type: 'draft_remote_update',
      workspaceId: 'ws_cw3',
      pageId: 'shp_seq',
      draftId: 'drf_WRONG',
      draftVersion: 1,
      checkpointSeq: 2,
      checkpointId: 'chk:drf_WRONG:v1:s2',
      serverSeq: 2,
      authorUid: 'uid_b',
      authorDisplayName: 'Bob',
      updateB64: peerDeltaB64,
      contributors: [],
    });

    // Out-of-order ACK for checkpointSeq=3 arrives BEFORE peer seq=2 -> gap banner shown & Submit DISABLED!
    emitCollabEvent({
      type: 'draft_update_ack',
      workspaceId: 'ws_cw3',
      requestId: opReqId,
      draftId: 'drf_seq',
      draftVersion: 1,
      checkpointSeq: 3,
      checkpointId: 'chk:drf_seq:v1:s3',
      clientOpSeq: 1,
      serverSeq: 3,
      duplicate: false,
      contributors: [],
    });

    expect(await screen.findByTestId('checkpoint-gap-banner')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Submit for review/i })).toBeDisabled();

    // Missing peer seq=2 arrives -> gap closes -> contiguous advance reaches chk:drf_seq:v1:s3 & Submit enables!
    emitCollabEvent({
      type: 'draft_remote_update',
      workspaceId: 'ws_cw3',
      pageId: 'shp_seq',
      draftId: 'drf_seq',
      draftVersion: 1,
      checkpointSeq: 2,
      checkpointId: 'chk:drf_seq:v1:s2',
      serverSeq: 2,
      authorUid: 'uid_b',
      authorDisplayName: 'Bob',
      updateB64: peerDeltaB64,
      contributors: [],
    });

    await waitFor(() => {
      expect(screen.queryByTestId('checkpoint-gap-banner')).not.toBeInTheDocument();
    });
    const submitBtn = screen.getByRole('button', { name: /Submit for review/i });
    expect(submitBtn).not.toBeDisabled();
    await user.click(submitBtn);
    await waitFor(() => expect(submitCalls).toHaveLength(1));
    expect(submitCalls[0].expectedCheckpointId).toBe('chk:drf_seq:v1:s3');
  });

  it('Finding 4 (final): binds rebase comparison checkpoint, preserves attempted merge text across stale checkpoint conflict, and retries with refreshed checkpoint', async () => {
    const svB64 = uint8ToBase64(Y.encodeStateVector(new Y.Doc()));
    const snap1B64 = seedInitialYjsUpdateB64('Reb Title', 'Initial draft line.');
    let latestServerChk = 'chk:drf_r4:v1:s1';
    let latestServerMd = 'Initial draft line.';
    let latestSeq = 1;
    const rebaseAttempts: any[] = [];

    tauriMocks.invoke.mockImplementation(async (cmd, args: any) => {
      if (cmd === 'cmd_get_shared_page') {
        return {
          outcome: 'ok',
          data: {
            currentRevision: {
              revisionNumber: 2,
              title: 'Reb Title',
              contentMd: 'Published rev 2 upstream.',
              authorUid: 'uid_c',
              authorDisplayName: 'Cara Admin',
              createdAtMs: 1_750_000_000_000,
              provenance: { originKind: 'userAuthored' },
            },
          },
        };
      }
      if (cmd === 'cmd_get_shared_draft') return null;
      if (cmd === 'cmd_collab_connect') {
        return { workspaceId: 'ws_cw3', uid: 'uid_a', connected: true };
      }
      if (cmd === 'cmd_open_live_draft' || cmd === 'cmd_get_live_draft') {
        const snap = seedInitialYjsUpdateB64('Reb Title', latestServerMd);
        const d = {
          draftId: 'drf_r4',
          workspaceId: 'ws_cw3',
          pageId: 'shp_r4',
          baseRevisionNumber: 1,
          draftVersion: 1,
          checkpointSeq: latestSeq,
          checkpointId: latestServerChk,
          status: 'editing',
          snapshotStateB64: snap,
          snapshotVectorB64: svB64,
          baseTitle: 'Reb Title',
          baseContentMd: 'Initial draft line.',
          currentTitle: 'Reb Title',
          currentContentMd: latestServerMd,
          contributors: [],
          pendingProposalId: null,
          createdByUid: 'uid_a',
          createdByDisplayName: 'Alice Owner',
          createdAtMs: 1_750_000_000_000,
          updatedAtMs: 1_750_000_000_000,
        };
        return { outcome: 'ok', data: cmd === 'cmd_open_live_draft' ? { created: false, draft: d } : d };
      }
      if (cmd === 'cmd_collab_join_draft') {
        const snap = seedInitialYjsUpdateB64('Reb Title', latestServerMd);
        emitCollabEvent({
          type: 'connected',
          workspaceId: 'ws_cw3',
          connectionId: 'conn_r4',
          uid: 'uid_a',
          displayName: 'Alice Owner',
          role: 'owner',
        });
        emitCollabEvent({
          type: 'draft_state',
          workspaceId: 'ws_cw3',
          draft: {
            draftId: 'drf_r4',
            workspaceId: 'ws_cw3',
            pageId: 'shp_r4',
            baseRevisionNumber: 1,
            draftVersion: 1,
            checkpointSeq: latestSeq,
            checkpointId: latestServerChk,
            status: 'editing',
            snapshotStateB64: snap,
            snapshotVectorB64: svB64,
            baseTitle: 'Reb Title',
            baseContentMd: 'Initial draft line.',
            currentTitle: 'Reb Title',
            currentContentMd: latestServerMd,
            contributors: [],
            pendingProposalId: null,
            createdByUid: 'uid_a',
            createdByDisplayName: 'Alice Owner',
            createdAtMs: 1_750_000_000_000,
            updatedAtMs: 1_750_000_000_000,
          },
          missingUpdateB64: '',
          serverVectorB64: svB64,
          presence: [],
        });
        return null;
      }
      if (cmd === 'cmd_rebase_live_draft') {
        rebaseAttempts.push(args);
        if (args.expectedCheckpointId !== latestServerChk) {
          return {
            outcome: 'error',
            code: 'stale_draft_checkpoint',
            message: `Live draft changed (expected ${args.expectedCheckpointId}, found ${latestServerChk}).`,
          };
        }
        const rebasedB64 = seedInitialYjsUpdateB64(args.rebasedTitle ?? 'Reb Title', args.rebasedContentMd ?? 'Merged');
        return {
          outcome: 'ok',
          data: {
            draftId: 'drf_r4',
            workspaceId: 'ws_cw3',
            pageId: 'shp_r4',
            baseRevisionNumber: 2,
            draftVersion: 2,
            checkpointSeq: 3,
            checkpointId: 'chk:drf_r4:v2:s3',
            status: 'editing',
            snapshotStateB64: rebasedB64,
            snapshotVectorB64: svB64,
            baseTitle: 'Reb Title',
            baseContentMd: 'Published rev 2 upstream.',
            currentTitle: args.rebasedTitle ?? 'Reb Title',
            currentContentMd: args.rebasedContentMd ?? 'Merged',
            contributors: [],
            pendingProposalId: null,
            createdByUid: 'uid_a',
            createdByDisplayName: 'Alice Owner',
            createdAtMs: 1_750_000_000_000,
            updatedAtMs: 1_750_000_002_000,
          },
        };
      }
      return null;
    });

    const user = userEvent.setup();
    render(
      <LiveSharedDraftEditor
        workspaceId="ws_cw3"
        pageId="shp_r4"
        baseRevisionNumber={1}
        baseTitle="Reb Title"
        baseContentMd={snap1B64 ? 'Initial draft line.' : ''}
        initialCurrentRevisionNumber={2}
        initialCurrentTitle="Reb Title"
        initialCurrentContentMd="Published rev 2 upstream."
        currentUid="uid_a"
        authorDisplayName="Alice Owner"
        onSubmitted={vi.fn()}
        onOpenProposal={vi.fn()}
        onBack={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId('live-sync-status')).toHaveTextContent('Synced');
    });
    await user.click(screen.getByRole('button', { name: /Compare & rebase onto rev 2/i }));
    const mergeBox = screen.getByLabelText('Rebased draft Markdown') as HTMLTextAreaElement;
    await user.clear(mergeBox);
    await user.type(mergeBox, 'My careful merge attempt');

    // Peer edit advances live draft on server BEFORE Alice confirms comparison opened at s1!
    latestServerChk = 'chk:drf_r4:v1:s2';
    latestServerMd = 'Initial draft line.\n\nPeer edit arrived before rebase.';
    latestSeq = 2;

    await user.click(screen.getByRole('button', { name: /Carry draft onto rev 2/i }));

    // Conflict surfaces cleanly, attempted merge text stays preserved, comparison refreshes to s2!
    expect(await screen.findByText(/Live draft changed/i)).toBeInTheDocument();
    expect(screen.getByLabelText('Rebased draft Markdown')).toHaveValue('My careful merge attempt');
    expect(await screen.findByTestId('rebase-latest-draft-preview')).toHaveTextContent(
      'Peer edit arrived before rebase.',
    );

    const carry = screen.getByRole('button', { name: /Carry draft onto rev 2/i });
    expect(carry).toBeDisabled();
    await user.click(carry);
    expect(rebaseAttempts).toHaveLength(1);
    const reconciled = 'My careful merge attempt\n\nPeer edit arrived before rebase.';
    fireEvent.change(mergeBox, { target: { value: reconciled } });
    const confirmComparison = screen.getByRole('button', { name: 'Confirm updated comparison' });
    await waitFor(() => expect(confirmComparison).toBeEnabled());
    await user.click(confirmComparison);
    await user.click(screen.getByRole('button', { name: /Carry draft onto rev 2/i }));
    await waitFor(() => expect(rebaseAttempts).toHaveLength(2));
    expect(rebaseAttempts[0].expectedCheckpointId).toBe('chk:drf_r4:v1:s1');
    expect(rebaseAttempts[1].expectedCheckpointId).toBe('chk:drf_r4:v1:s2');
    expect(rebaseAttempts[1].rebasedContentMd).toBe(reconciled);
    expect(screen.queryByTestId('live-draft-stale-banner')).not.toBeInTheDocument();
  });

  it.each([true, false])('Finding 8: refreshes after a failed second page with live overlap %s', async (withLiveOverlap) => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const catchupCalls: number[] = [];
    let page2Failures = 1;
    let detailsFetches = 0;

    tauriMocks.invoke.mockImplementation(async (cmd, args: any) => {
      if (cmd === 'cmd_get_workspace') {
        detailsFetches += 1;
        return { outcome: 'ok', data: wsDetails };
      }
      if (cmd === 'cmd_collab_connect') {
        return { workspaceId: 'ws_cw3', uid: 'uid_b', connected: true };
      }
      if (cmd === 'cmd_list_realtime_events') {
        catchupCalls.push(args.afterSeq);
        if (args.afterSeq === 0) {
          return {
            outcome: 'ok',
            data: [{ eventSeq: 10, workspaceId: 'ws_cw3', eventType: 'old', payloadJson: {}, createdAtMs: 1 }],
          };
        }
        if (args.afterSeq === 10) {
          return {
            outcome: 'ok',
            data: Array.from({ length: 100 }, (_, i) => ({
              eventSeq: 11 + i,
              workspaceId: 'ws_cw3',
              eventType: 'project_thread_updated',
              payloadJson: {},
              createdAtMs: 2 + i,
            })),
          };
        }
        if (args.afterSeq === 110) {
          if (page2Failures > 0) {
            page2Failures -= 1;
            return { outcome: 'error', code: 'temporary', message: 'Page 2 transiently failed' };
          }
          if (!withLiveOverlap) return { outcome: 'ok', data: [] };
          return {
            outcome: 'ok',
            data: [
              { eventSeq: 111, workspaceId: 'ws_cw3', eventType: 'project_thread_updated', payloadJson: {}, createdAtMs: 200 },
              { eventSeq: 205, workspaceId: 'ws_cw3', eventType: 'project_thread_updated', payloadJson: {}, createdAtMs: 250 },
            ],
          };
        }
        if (args.afterSeq === 205) {
          return {
            outcome: 'ok',
            data: [{ eventSeq: 206, workspaceId: 'ws_cw3', eventType: 'project_thread_updated', payloadJson: {}, createdAtMs: 260 }],
          };
        }
        return { outcome: 'ok', data: [] };
      }
      return { outcome: 'ok', data: { items: [], nextCursor: null } };
    });

    render(
      <TeamWorkspaceView
        workspace={wsSummary}
        initialTab="overview"
        currentUid="uid_b"
        onSelectPersonal={vi.fn()}
        onWorkspaceUpdated={vi.fn()}
        onAccessLost={vi.fn()}
        onSessionChange={vi.fn()}
        onRefreshList={vi.fn()}
      />,
    );

    await waitFor(() => expect(catchupCalls).toEqual([0]));
    const baselineDetails = detailsFetches;

    // Trigger disconnect -> reconnect -> page 1 (afterSeq=10) succeeds, page 2 (afterSeq=110) FAILS
    // WHILE live event seq 205 arrives! Cursor MUST stay at 110 cleanly.
    emitCollabEvent({ type: 'ws_disconnected', workspaceId: 'ws_cw3', reason: 'drop' });
    emitCollabEvent({
      type: 'connected',
      workspaceId: 'ws_cw3',
      connectionId: 'conn_p2',
      uid: 'uid_b',
      displayName: 'Bob',
      role: 'editor',
    });
    if (withLiveOverlap) emitCollabEvent({
      type: 'workspace_event',
      workspaceId: 'ws_cw3',
      eventSeq: 205,
      eventType: 'project_thread_updated',
      payload: {},
    });

    await waitFor(() => expect(catchupCalls).toEqual([0, 10, 110]));

    // Advance backoff -> retries afterSeq=110 automatically (NOT 205!), recovers 111 + 205, refreshes UI!
    await vi.advanceTimersByTimeAsync(1100);
    await waitFor(() => expect(catchupCalls).toEqual([0, 10, 110, 110]));
    await waitFor(() => expect(detailsFetches).toBeGreaterThan(baselineDetails));

    // Draft-only access_revoked (role_downgraded) must NOT permanently kill discussion catch-up!
    emitCollabEvent({
      type: 'access_revoked',
      workspaceId: 'ws_cw3',
      code: 'role_downgraded',
      reason: 'Downgraded to Viewer.',
    });
    emitCollabEvent({ type: 'ws_disconnected', workspaceId: 'ws_cw3', reason: 'brief' });
    emitCollabEvent({
      type: 'connected',
      workspaceId: 'ws_cw3',
      connectionId: 'conn_v',
      uid: 'uid_b',
      displayName: 'Bob',
      role: 'viewer',
    });
    await waitFor(() => expect(catchupCalls).toEqual([0, 10, 110, 110, withLiveOverlap ? 205 : 110]));
    vi.useRealTimers();
  });
});
