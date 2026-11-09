import { invoke } from '@tauri-apps/api/core';
import { isTauri, getBrowserConnectionItems } from './ipc';
import type { EntityRef, WorkspaceConnection, WorkspaceEntity, WorkspaceEntityDetail } from '../types/sthira';

export const sameEntity = (a: EntityRef, b: EntityRef) => a.kind === b.kind && a.id === b.id;
export const entityHref = (entity: EntityRef) => `#/sthira/${entity.kind}/${encodeURIComponent(entity.id)}`;
export function parseEntityHref(href?: string): EntityRef | null {
  const match = href?.match(/^(?:#\/sthira\/|sthira:\/\/)(artifact|project)\/([^/?#]+)$/);
  if (!match) return null;
  try {
    const id = decodeURIComponent(match[2]);
    if (!id.trim() || id.length > 1536 || /[\u0000-\u001f\u007f-\u009f/\\?#]/.test(id)) return null;
    return { kind: match[1] as EntityRef['kind'], id };
  } catch { return null; }
}

export function parseReferenceQuery(query:string):EntityRef|null {
  const value=query.trim();
  const unwrapped=value.startsWith('`')&&value.endsWith('`')||value.startsWith('<')&&value.endsWith('>')?value.slice(1,-1):value;
  return parseEntityHref(unwrapped);
}

// Browser preview only. Desktop references are persisted in the vault's SQLite DB.
const previewLinks = new Map<string, WorkspaceConnection>();

export async function searchConnectionTargets(query: string): Promise<WorkspaceEntity[]> {
  if (isTauri) return invoke('cmd_search_connection_targets', { query });
  const reference=parseReferenceQuery(query);
  const items=getBrowserConnectionItems().map(item=>item.entity);
  if(reference)return items.filter(entity=>sameEntity(entity,reference));
  if(/^[0-9a-f-]{36}$/i.test(query.trim()))return items.filter(entity=>entity.id===query.trim());
  return items
    .filter((entity) => `${entity.title} ${entity.type_label}`.toLowerCase().includes(query.trim().toLowerCase())).slice(0, 60);
}

export async function getWorkspaceEntity(entity: EntityRef): Promise<WorkspaceEntityDetail> {
  if (isTauri) return invoke('cmd_get_workspace_entity', { entity });
  const item = getBrowserConnectionItems().find((item) => sameEntity(item.entity, entity));
  if (!item) throw new Error('This item is no longer available.');
  return item;
}

export async function getWorkspaceConnections(entity: EntityRef): Promise<WorkspaceConnection[]> {
  if (isTauri) return invoke('cmd_get_workspace_connections', { entity });
  const entities = getBrowserConnectionItems().map((item) => item.entity);
  return [...previewLinks.values()].filter((link) => sameEntity(link.source, entity) || sameEntity(link.target, entity))
    .flatMap((link) => {
      const source = entities.find((item) => sameEntity(item, link.source));
      const target = entities.find((item) => sameEntity(item, link.target));
      return source && target ? [{ ...link, source, target }] : [];
    });
}

export async function createWorkspaceConnection(source: EntityRef, target: EntityRef): Promise<string> {
  if (isTauri) return invoke('cmd_create_workspace_connection', { source, target });
  if (sameEntity(source, target)) throw new Error('An item cannot link to itself.');
  const existing = [...previewLinks.values()].find((link) => sameEntity(link.source, source) && sameEntity(link.target, target));
  if (existing) return existing.id;
  const [s, t] = await Promise.all([getWorkspaceEntity(source), getWorkspaceEntity(target)]);
  const id = crypto.randomUUID();
  previewLinks.set(id, { id, source: s.entity, target: t.entity, created_at: Date.now() });
  return id;
}

export async function deleteWorkspaceConnection(connectionId: string): Promise<void> {
  if (isTauri) return invoke('cmd_delete_workspace_connection', { connectionId });
  previewLinks.delete(connectionId);
}
