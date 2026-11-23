import { invoke } from '@tauri-apps/api/core';
import { getBrowserConnectionItems, isTauri } from './ipc';
export type MemoryScope = { kind: 'workspace' } | { kind: 'project' | 'session'; id: string };
export type MemoryActor = { kind: 'user' } | { kind: 'agent'; agent_id: string; model: string; provider: string };
export interface MemorySource { artifact_id: string; title: string; content_hash: string; status: 'current' | 'changed' | 'unavailable' }
export interface MemoryEntry { id: string; scope: MemoryScope; scope_available?: boolean; title: string; body: string; author: MemoryActor; sources: MemorySource[]; pinned: boolean; archived: boolean; version: number; created_at: number; updated_at: number }
export interface MemoryQuery { scope?: MemoryScope | null; search: string; include_archived: boolean }
export interface MemoryDraft { scope: MemoryScope; title: string; body: string; source_ids: string[]; pinned: boolean }
export interface MemoryRevision { version: number; operation: string; editor: MemoryActor; snapshot: MemoryEntry; created_at: number }
export interface MemoryContext { entries: MemoryEntry[]; used_chars: number; skipped_stale: number; skipped_budget: number; candidate_limit: number; content_is_untrusted: boolean }
export type MemoryAction =
  | { type: 'list'; query: MemoryQuery }
  | { type: 'read'; id: string }
  | { type: 'history'; id: string }
  | { type: 'create'; draft: MemoryDraft }
  | { type: 'update'; id: string; expected_version: number; title: string; body: string; pinned: boolean }
  | { type: 'archive' | 'restore'; id: string; expected_version: number }
  | { type: 'context'; query: MemoryQuery; max_entries: number; max_chars: number };
export type MemoryResult = { kind: 'entries'; value: MemoryEntry[] } | { kind: 'entry'; value: MemoryEntry } | { kind: 'history'; value: MemoryRevision[] } | { kind: 'context'; value: MemoryContext };
// Deliberately session-only UI preview. Native persistence belongs to the Rust
// action service. This store is never used in a Tauri window.
const entries = new Map<string, MemoryEntry>();
const revisions = new Map<string, MemoryRevision[]>();
if (typeof window !== 'undefined') window.addEventListener('sthira:vault-reset', () => { entries.clear(); revisions.clear(); });
const hash = (value: string) => { let n = 2166136261; for (const c of value) n = Math.imul(n ^ c.charCodeAt(0), 16777619); return `preview-${n >>> 0}`; };
const currentSources = (entry: MemoryEntry): MemoryEntry => ({ ...structuredClone(entry), sources: entry.sources.map(source => {
  const page = getBrowserConnectionItems().find(item => item.entity.kind === 'artifact' && item.entity.id === source.artifact_id);
  return { ...source, status: !page ? 'unavailable' : hash(page.content_md) === source.content_hash ? 'current' : 'changed' };
}) });
const matches = (scope: MemoryScope, filter?: MemoryScope | null) => !filter || (scope.kind === filter.kind && (scope.kind === 'workspace' || ('id' in filter && scope.id === filter.id)));
export async function memoryAction(action: MemoryAction): Promise<MemoryResult> {
  if (isTauri) return invoke<MemoryResult>('cmd_memory_action', { action });
  const queryEntries = (query: MemoryQuery) => [...entries.values()].filter(entry =>
    (query.include_archived || !entry.archived) && matches(entry.scope, query.scope) && `${entry.title}\n${entry.body}`.toLowerCase().includes(query.search.trim().toLowerCase()))
    .sort((a,b) => Number(b.pinned)-Number(a.pinned) || b.updated_at-a.updated_at || a.id.localeCompare(b.id)).slice(0,100).map(currentSources);
  if (action.type === 'list') return { kind: 'entries', value: queryEntries(action.query) };
  if (action.type === 'context') {
    const result: MemoryContext = { entries: [], used_chars: 0, skipped_stale: 0, skipped_budget: 0, candidate_limit: 100, content_is_untrusted: true };
    for (const entry of queryEntries({ ...action.query, include_archived: false })) {
      if (entry.sources.some(source => source.status !== 'current')) { result.skipped_stale++; continue; }
      const size = [...JSON.stringify(entry)].length;
      if (result.entries.length >= Math.min(action.max_entries,20) || result.used_chars+size > Math.min(action.max_chars,24000)) { result.skipped_budget++; continue; }
      result.entries.push(entry); result.used_chars += size;
    }
    return { kind: 'context', value: result };
  }
  if (action.type === 'read' || action.type === 'history') {
    const entry = entries.get(action.id); if (!entry) throw new Error('Memory unavailable.');
    return action.type === 'read' ? { kind: 'entry', value: currentSources(entry) } : { kind: 'history', value: structuredClone(revisions.get(action.id) || []).reverse().slice(0,100) };
  }
  let entry: MemoryEntry;
  if (action.type === 'create') {
    const draft = action.draft;
    const sources = draft.source_ids.map(id => {
      const page = getBrowserConnectionItems().find(item => item.entity.kind === 'artifact' && item.entity.id === id);
      if (!page) throw new Error('A source is missing.');
      return { artifact_id: id, title: page.entity.title, content_hash: hash(page.content_md), status: 'current' as const };
    });
    entry = { id: crypto.randomUUID(), scope: draft.scope, scope_available: true, title: draft.title.trim(), body: draft.body, sources, author: { kind: 'user' }, pinned: draft.pinned, archived: false, version: 1, created_at: Date.now(), updated_at: Date.now() };
  } else {
    const current = entries.get(action.id); if (!current) throw new Error('Memory unavailable.');
    if (current.version !== action.expected_version) throw new Error('Memory changed in another session. Reload before saving.');
    entry = structuredClone(current);
    if (action.type === 'update') { entry.title = action.title.trim(); entry.body = action.body; entry.pinned = action.pinned; }
    else entry.archived = action.type === 'archive';
    entry.version++; entry.updated_at = Date.now();
  }
  if (!entry.title || [...entry.title].length>160 || !entry.body.trim() || [...entry.body].length>16000 || (entry.scope.kind !== 'workspace' && !entry.scope.id.trim())) throw new Error('Enter a title, content, and an available scope.');
  entries.set(entry.id, entry);
  revisions.set(entry.id, [...(revisions.get(entry.id) || []), { version: entry.version, operation: action.type, editor: { kind: 'user' }, snapshot: structuredClone(entry), created_at: entry.updated_at }]);
  return { kind: 'entry', value: currentSources(entry) };
}
