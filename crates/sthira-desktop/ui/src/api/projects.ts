import { invoke } from '@tauri-apps/api/core';
import { getBrowserProjectItems, getBrowserProjectOverview, isTauri } from './ipc';
import { getWorkspaceConnections, sameEntity } from './connections';
import type { ProjectOverviewData, ProjectItemRole } from '../types/sthira';

export async function getProjectOverview(projectId: string): Promise<ProjectOverviewData> {
  if (isTauri) return invoke('cmd_get_project_overview', { projectId });
  const overview = getBrowserProjectOverview(projectId);
  const candidates = getBrowserProjectItems();
  const source = { kind: 'project' as const, id: projectId };
  const links = await getWorkspaceConnections(source);
  for (const link of links) {
    const outgoing = sameEntity(link.source, source);
    const target = outgoing ? link.target : link.source;
    const item = candidates.find((item) => item.kind === target.kind && item.id === target.id);
    if (!item) continue;
    const role: ProjectItemRole = outgoing ? 'reference' : 'backlink';
    const existing = overview.items.find((value) => value.kind === item.kind && value.id === item.id);
    if (existing) { if (!existing.roles.includes(role)) existing.roles.push(role); }
    else overview.items.push({ ...item, roles: [role] });
  }
  return overview;
}
