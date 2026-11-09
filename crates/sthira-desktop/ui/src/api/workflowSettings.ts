import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './ipc';
import type { ProviderEntry, ProviderModelEntry } from '../types/sthira';

export const WORKFLOW_IDS = ['research', 'plan', 'draft', 'chat', 'resume'] as const;
export type WorkflowId = typeof WORKFLOW_IDS[number];
export interface WorkflowSetting { enabled: boolean; model_id: string | null; templates_enabled: boolean }
export type WorkflowSettings = Record<WorkflowId, WorkflowSetting>;
export const WORKFLOW_SETTINGS_CHANGED = 'sthira:workflow-settings-changed';
const previewKey = 'sthira-preview-workflows-v1';
export function defaultWorkflowSettings(): WorkflowSettings {
  return Object.fromEntries(WORKFLOW_IDS.map(id => [id, { enabled: false, model_id: null, templates_enabled: false }])) as WorkflowSettings;
}
export async function getWorkflowSettings(): Promise<WorkflowSettings> {
  const value = isTauri ? await invoke<WorkflowSettings>('cmd_get_workflow_settings')
    : JSON.parse(localStorage.getItem(previewKey) || '{}');
  if (!value || typeof value !== 'object') throw new Error('Couldn’t load workflow settings. Reopen the desktop app and try again.');
  const settings = defaultWorkflowSettings();
  for (const id of WORKFLOW_IDS) {
    const entry = value[id];
    if (entry) settings[id] = { enabled: entry.enabled === true, model_id: typeof entry.model_id === 'string' && entry.model_id ? entry.model_id : null, templates_enabled: entry.templates_enabled === true };
  }
  return settings;
}
export async function saveWorkflowSetting(workflow: WorkflowId, setting: WorkflowSetting): Promise<void> {
  if (isTauri) await invoke('cmd_save_workflow_setting', { workflow, setting });
  else {
    const settings = await getWorkflowSettings();
    localStorage.setItem(previewKey, JSON.stringify({ ...settings, [workflow]: setting }));
  }
  window.dispatchEvent(new CustomEvent(WORKFLOW_SETTINGS_CHANGED, { detail: { workflow, setting } }));
}
export function openWorkflowSettings(): void {
  window.dispatchEvent(new CustomEvent('sthira:open-settings', { detail: 'workflows' }));
}

export function getEnabledModels(providers: ProviderEntry[]): ProviderModelEntry[] {
  return (providers || [])
    .filter(provider => Boolean(provider && provider.is_enabled))
    .flatMap(provider => (provider.models || []).filter(model => Boolean(model && model.is_enabled)));
}

export function getWorkspaceDefaultModel(providers: ProviderEntry[]): ProviderModelEntry | null {
  const models = getEnabledModels(providers);
  return models.find(m => m.is_default) || null;
}

export function resolveWorkflowModel(
  setting: WorkflowSetting | undefined,
  providers: ProviderEntry[]
): ProviderModelEntry | null {
  if (!setting) return null;
  const models = getEnabledModels(providers);
  if (setting.model_id) {
    return models.find(m => m.id === setting.model_id) || null;
  }
  return models.find(m => m.is_default) || null;
}

export function isWorkflowReady(
  setting: WorkflowSetting | undefined,
  providers: ProviderEntry[],
  loading?: boolean,
  error?: unknown
): boolean {
  if (loading || Boolean(error)) return false;
  if (!setting || !setting.enabled) return false;
  return resolveWorkflowModel(setting, providers) !== null;
}
