import { invoke } from '@tauri-apps/api/core';
import type { WorkspaceResponse } from './workspaces';

export interface SharedAgentConnection {
  connectionId: string; workspaceId: string; projectId: string; projectName: string;
  memberUid: string; memberDisplayName: string; deviceLabel: string;
  label: string; clientName: string; permission: 'read' | 'suggest'; generation: number;
  pages: { pageId: string; title: string; currentRevisionNumber: number; available: boolean }[];
  revokedAtMs: number | null; canRunOnDevice: boolean; localStatus?: string; initiatedByCurrentUser?: boolean;
}
export interface SharedAgentProposal {
  proposalId: string; kind: 'edit' | 'newPage'; workspaceId: string;
  projectId: string | null; projectName: string | null; targetPageId: string | null;
  targetTitle: string; baseRevisionNumber: number | null; proposedTitle: string;
  proposedContentMd: string; summary: string; status: string; submissionCount: number;
  authorUid: string; authorDisplayName: string; connectionId: string | null;
  connectionLabel: string | null; agentLabel: string; modelLabel: string; providerLabel: string;
  modelTrust: string; sources: { pageId: string; pageTitle: string; revisionNumber: number; contentSha256: string }[];
  isStale: boolean; staleReason: string | null; reviewEligible: boolean; revocationState: string;
  appliedPageId: string | null; appliedRevisionNumber: number | null;
  reviewerDisplayName: string | null; reviewerNote?: string | null; initiatedByCurrentUser?: boolean;
}
export interface SharedNativeRun {
  id: string; workspaceId: string; connectionId: string; kind: 'answer' | 'edit' | 'newPage';
  goal: string; modelId: string; modelLabel: string; providerLabel: string;
  status: string; stage: string; answer: string | null; proposalId: string | null; error: string | null;
  sources: { pageId: string; title: string; revisionNumber: number; contentSha256: string }[];
}
export interface SharedConfig { label: string; instructions: string; json?: unknown; text?: string }
export function sharedAgents<T>(workspaceId: string, action: string, input: unknown = {}): Promise<WorkspaceResponse<T>> {
  return invoke('cmd_shared_agents', { workspaceId, action, input });
}
