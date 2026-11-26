import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { isTauri } from './ipc';
import type {
  LiveDraftContributorView,
  LiveDraftDiffResponse,
  WorkspaceRole,
} from './workspaces';

export interface CollaboratorPresence {
  connectionId: string;
  uid: string;
  displayName: string;
  color: string;
  pageId: string;
  draftId: string;
  anchor?: number | null;
  head?: number | null;
  relAnchor?: string | null;
  relHead?: string | null;
  updatedAtMs: number;
}

export type CollabServerEvent =
  | {
      type: 'connected';
      workspaceId: string;
      accountUid?: string;
      connectionId: string;
      uid: string;
      displayName: string;
      role: WorkspaceRole;
    }
  | ({
      type: 'draft_state';
      workspaceId: string;
      accountUid?: string;
      presence: CollaboratorPresence[];
    } & LiveDraftDiffResponse)
  | {
      type: 'draft_update_ack';
      workspaceId: string;
      accountUid?: string;
      requestId: string;
      draftId: string;
      draftVersion: number;
      checkpointSeq: number;
      checkpointId: string;
      clientOpSeq: number;
      serverSeq: number;
      duplicate: boolean;
      contributors: LiveDraftContributorView[];
    }
  | {
      type: 'draft_update_nack';
      workspaceId: string;
      accountUid?: string;
      requestId: string;
      clientOpSeq: number;
      code: string;
      error: string;
      fatal: boolean;
    }
  | {
      type: 'draft_remote_update';
      workspaceId: string;
      accountUid?: string;
      pageId: string;
      draftId: string;
      draftVersion: number;
      checkpointSeq: number;
      checkpointId: string;
      serverSeq: number;
      authorUid: string;
      authorDisplayName: string;
      updateB64: string;
      contributors: LiveDraftContributorView[];
    }
  | {
      type: 'draft_rebased';
      workspaceId: string;
      accountUid?: string;
      draft: import('./workspaces').SharedLiveDraftView;
    }
  | {
      type: 'presence_snapshot';
      workspaceId: string;
      accountUid?: string;
      pageId: string;
      draftId: string;
      participants: CollaboratorPresence[];
    }
  | {
      type: 'presence_notice';
      workspaceId: string;
      accountUid?: string;
      pageId: string;
      draftId: string;
      kind: string;
      connectionId: string;
      uid: string;
      displayName: string;
    }
  | {
      type: 'workspace_event';
      workspaceId: string;
      accountUid?: string;
      eventSeq: number;
      eventType: string;
      projectId?: string | null;
      pageId?: string | null;
      threadId?: string | null;
      proposalId?: string | null;
      draftId?: string | null;
      payload: unknown;
    }
  | {
      type: 'access_revoked';
      workspaceId: string;
      accountUid?: string;
      code: string;
      reason: string;
    }
  | {
      type: 'pong';
      workspaceId: string;
      accountUid?: string;
      serverTimeMs: number;
    }
  | {
      type: 'ws_disconnected';
      workspaceId: string;
      accountUid?: string;
      reason: string;
    }
  | {
      type: 'error';
      workspaceId: string;
      accountUid?: string;
      code: string;
      error: string;
    };

export interface CollabConnectResult {
  workspaceId: string;
  uid: string;
  connected: boolean;
}

export async function collabConnect(
  workspaceId: string,
  accountUid?: string | null,
): Promise<CollabConnectResult | null> {
  if (!isTauri) return null;
  return invoke<CollabConnectResult>('cmd_collab_connect', {
    workspaceId,
    accountUid: accountUid ?? null,
  });
}

export async function collabDisconnect(): Promise<void> {
  if (!isTauri) return;
  await invoke('cmd_collab_disconnect');
}

export async function collabJoinDraft(params: {
  workspaceId: string;
  pageId: string;
  clientStateVectorB64?: string | null;
  initialUpdateB64?: string | null;
  expectedBaseRevisionNumber?: number | null;
  accountUid?: string | null;
}): Promise<void> {
  if (!isTauri) return;
  await invoke('cmd_collab_join_draft', {
    workspaceId: params.workspaceId,
    pageId: params.pageId,
    clientStateVectorB64: params.clientStateVectorB64 ?? null,
    initialUpdateB64: params.initialUpdateB64 ?? null,
    expectedBaseRevisionNumber: params.expectedBaseRevisionNumber ?? null,
    accountUid: params.accountUid ?? null,
  });
}

export async function collabLeaveDraft(
  workspaceId: string,
  pageId: string,
  accountUid?: string | null,
): Promise<void> {
  if (!isTauri) return;
  await invoke('cmd_collab_leave_draft', {
    workspaceId,
    pageId,
    accountUid: accountUid ?? null,
  });
}

export async function collabSendUpdate(params: {
  workspaceId: string;
  requestId: string;
  pageId: string;
  draftId: string;
  draftVersion: number;
  clientOpSeq: number;
  updateB64: string;
  accountUid?: string | null;
}): Promise<void> {
  if (!isTauri) return;
  await invoke('cmd_collab_send_update', {
    workspaceId: params.workspaceId,
    requestId: params.requestId,
    pageId: params.pageId,
    draftId: params.draftId,
    draftVersion: params.draftVersion,
    clientOpSeq: params.clientOpSeq,
    updateB64: params.updateB64,
    accountUid: params.accountUid ?? null,
  });
}

export async function collabSendAwareness(params: {
  workspaceId: string;
  pageId: string;
  draftId: string;
  anchor?: number | null;
  head?: number | null;
  relAnchor?: string | null;
  relHead?: string | null;
  accountUid?: string | null;
}): Promise<void> {
  if (!isTauri) return;
  await invoke('cmd_collab_send_awareness', {
    workspaceId: params.workspaceId,
    pageId: params.pageId,
    draftId: params.draftId,
    anchor: params.anchor ?? null,
    head: params.head ?? null,
    relAnchor: params.relAnchor ?? null,
    relHead: params.relHead ?? null,
    accountUid: params.accountUid ?? null,
  });
}

export async function collabPing(
  workspaceId: string,
  accountUid?: string | null,
): Promise<void> {
  if (!isTauri) return;
  await invoke('cmd_collab_ping', {
    workspaceId,
    accountUid: accountUid ?? null,
  });
}

export async function subscribeCollabEvents(
  handler: (event: CollabServerEvent) => void,
): Promise<UnlistenFn> {
  const internals = (
    typeof window !== 'undefined'
      ? (window as unknown as { __TAURI_INTERNALS__?: { transformCallback?: unknown } })
          .__TAURI_INTERNALS__
      : undefined
  );
  if (!isTauri || typeof internals?.transformCallback !== 'function') {
    return () => {};
  }
  return listen<CollabServerEvent>('sthira://collab-event', (event) => {
    handler(event.payload);
  });
}
