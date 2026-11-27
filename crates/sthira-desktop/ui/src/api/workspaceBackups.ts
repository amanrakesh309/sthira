import { invoke } from '@tauri-apps/api/core';
import type { WorkspaceResponse } from './workspaces';
export interface BackupSummary {
  backupId: string; sourceName: string; createdAtMs: number; createdByDisplayName: string;
  bytes: number; checksumSha256: string; counts: Record<string, number>;
}
export interface RestorePreview {
  ticketId: string; workspaceId: string; targetName: string; sourceName: string;
  checksumSha256: string; counts: Record<string, number>; expiresAtMs: number; memberCount: number;
  legacyAccessReviewRequired?: boolean; accessSummary?: Record<string, number>;
}
export interface Restoration {
  receipt: { restoreId: string; workspaceId: string; sourceName: string; restoredAtMs: number;
    restoredByDisplayName: string; checksumSha256: string; counts: Record<string, number> };
  history: RecoveryHistory[] | null;
}
export interface RecoveryHistory {
  sourceName: string; capturedAtMs: number; originChecksum: string | null;
  pageMap: Record<string, string>; projectMap: Record<string, string>;
  records: {table: string; rows: Record<string, string | number | null>[]}[];
}
export function workspaceBackups<T>(workspaceId: string, accountUid: string, action: string, input: unknown = {}): Promise<WorkspaceResponse<T>> {
  return invoke('cmd_workspace_backups', {workspaceId, accountUid, action, input});
}
