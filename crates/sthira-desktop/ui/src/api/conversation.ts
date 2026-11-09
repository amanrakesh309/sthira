import { invoke } from '@tauri-apps/api/core';
import { isTauri, getArtifactContext } from './ipc';
import type { Artifact } from '../types/sthira';
export interface ConversationTurn {
  id: string; user_text: string; reply: string | null; error: string | null;
  basis_content_md: string; proposed_content_md: string | null;
  model_id?: string | null; model_name?: string | null; provider_id?: string | null; provider_name?: string | null;
  proposal_state: 'pending' | 'applied' | 'dismissed' | 'reverted'; change_set_id: string | null;
}
export interface Conversation { mode: 'plan' | 'research' | 'document'; initial_prompt: string; model_id?: string | null; model_name?: string | null; turns: ConversationTurn[] }
const preview = new Map<string, Conversation>();
export async function getDocumentConversation(artifactId: string, mode?: 'plan' | 'research' | 'document', initialPrompt?: string, modelId?: string | null): Promise<Conversation | null> {
  if (isTauri) {
    const value = await invoke<Conversation | null>('cmd_get_document_conversation', { artifactId, mode: mode || null, initialPrompt: initialPrompt || null, modelId: modelId || null });
    return value && (value.mode === 'plan' || value.mode === 'research' || value.mode === 'document') && Array.isArray(value.turns) ? value : null;
  }
  if (!preview.has(artifactId) && mode) preview.set(artifactId, { mode, initial_prompt: initialPrompt || '', turns: [] });
  return preview.get(artifactId) || null;
}
export async function discussDocument(artifactId: string, text: string, modelId: string | undefined, proposeEdit: boolean): Promise<Conversation> {
  if (isTauri) return invoke('cmd_discuss_document', { artifactId, text, modelId: modelId || null, proposeEdit });
  const chat = preview.get(artifactId);
  if (!chat) throw new Error('Conversation unavailable.');
  const context = await getArtifactContext(artifactId);
  const next: Conversation = { ...chat, turns: [...chat.turns, { id: crypto.randomUUID(), user_text: text,
    reply: 'Browser preview: live discussion requires the desktop app and a configured model. Your document has not changed.',
    error: null, basis_content_md: context.artifact.content_md, proposed_content_md: null, proposal_state: 'pending', change_set_id: null }] };
  preview.set(artifactId, next); return next;
}
export async function decideDocumentEdit(artifactId: string, turnId: string, apply: boolean): Promise<Artifact> {
  if (isTauri) return invoke('cmd_decide_document_edit', { artifactId, turnId, apply });
  throw new Error('Applying model suggestions requires the desktop app.');
}

export async function undoDocumentEdit(artifactId: string, changeSetId: string): Promise<Artifact> {
  if (isTauri) return invoke('cmd_undo_document_edit', { artifactId, changeSetId });
  throw new Error('Undoing model edits requires the desktop app.');
}

export async function selectDocumentConversationModel(artifactId: string, modelId: string): Promise<Conversation> {
  if (isTauri) return invoke('cmd_select_document_conversation_model', { artifactId, modelId });
  const chat = preview.get(artifactId);
  if (!chat) throw new Error('Conversation unavailable.');
  const { getProviderCatalog } = await import('./ipc');
  const catalog = await getProviderCatalog();
  const model = catalog.filter(provider => provider.is_enabled).flatMap(provider => provider.models).find(model => model.id === modelId && model.is_enabled);
  if (!model) throw new Error('The selected model is unavailable. Select an enabled model.');
  const next = { ...chat, model_id: model.id, model_name: model.display_name || model.model_identifier };
  preview.set(artifactId, next); return next;
}
