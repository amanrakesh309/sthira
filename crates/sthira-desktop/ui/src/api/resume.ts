import { invoke } from '@tauri-apps/api/core';
import { isTauri, getResumeBrief } from './ipc';
import { WORKFLOW_SETTINGS_CHANGED } from './workflowSettings';
import type { ResumeBrief } from '../types/sthira';

export interface ResumePreferences {
  model_mode: 'workspace' | 'local' | 'factual'; model_id: string | null; local_fallback: boolean;
  include_decisions: boolean; include_tasks: boolean; include_questions: boolean;
  include_recent_changes: boolean; include_excerpts: boolean; max_sources: number;
}
export interface ResumeSettings { enabled: boolean; preferences: ResumePreferences }
export interface ResumePackStatus { label: string; ready: boolean; downloading: boolean; supported: boolean; download_bytes: number; notice: string | null }
export const RESUME_SETTINGS_CHANGED = 'sthira:resume-settings-changed';
export const defaultResumeSettings = (): ResumeSettings => ({ enabled: false, preferences: { model_mode: 'workspace', model_id: null, local_fallback: true, include_decisions: true, include_tasks: true, include_questions: true, include_recent_changes: true, include_excerpts: true, max_sources: 8 } });
export async function getResumeSettings(): Promise<ResumeSettings> {
  if (isTauri) {
    const result = await invoke<ResumeSettings>('cmd_get_resume_settings');
    if (!result?.preferences) throw new Error('Could not load Resume settings.');
    return result;
  }
  return JSON.parse(localStorage.getItem('sthira-preview-resume-v1') || 'null') || defaultResumeSettings();
}
export async function saveResumeSettings(settings: ResumeSettings): Promise<void> {
  if (isTauri) await invoke('cmd_save_resume_settings', { settings });
  else localStorage.setItem('sthira-preview-resume-v1', JSON.stringify(settings));
  window.dispatchEvent(new Event(RESUME_SETTINGS_CHANGED));
  window.dispatchEvent(new CustomEvent(WORKFLOW_SETTINGS_CHANGED, { detail: { workflow: 'resume', setting: { enabled: settings.enabled, model_id: settings.preferences.model_id, templates_enabled: false } } }));
}
export async function getResumePackStatus(): Promise<ResumePackStatus> {
  if (isTauri) {
    const value = await invoke<ResumePackStatus>('cmd_get_resume_pack_status');
    if (!value?.label) throw new Error('Could not inspect the local Resume pack.');
    return value;
  }
  return { label: 'SmolLM2-360M · local preview', ready: false, downloading: false, supported: false, download_bytes: 409456950, notice: 'The local model download is available in the desktop app.' };
}
export async function installResumePack(): Promise<ResumePackStatus> { return invoke('cmd_install_resume_pack'); }
export async function removeResumePack(): Promise<ResumePackStatus> { return invoke('cmd_remove_resume_pack'); }
export async function generateResumeBrief(projectId: string): Promise<ResumeBrief> {
  if (isTauri) return invoke('cmd_generate_resume_brief', { projectId });
  return getResumeBrief(projectId, 1);
}

export interface ResumeModelView { id: string | null; name: string | null; provider_name: string | null; available: boolean; notice: string | null }
export async function getResumeModel(forNaming = false): Promise<ResumeModelView> {
  if (isTauri) return invoke('cmd_get_resume_model', { forNaming });
  const settings = await getResumeSettings();
  const { getProviderCatalog } = await import('./ipc');
  const providers = (await getProviderCatalog()).filter(provider => provider.is_enabled);
  const candidates = providers.flatMap(provider => provider.models.filter(model => model.is_enabled).map(model => ({ model, provider })));
  const selected = candidates.find(({model}) => settings.preferences.model_id ? model.id === settings.preferences.model_id : model.is_default);
  if ((!settings.enabled && !forNaming) || settings.preferences.model_mode === 'factual' || !selected || settings.preferences.model_mode === 'local') return { id:null,name:null,provider_name:null,available:false,notice:'Choose a connected model, or use the local pack in the desktop app.' };
  return { id:selected.model.id,name:selected.model.display_name,provider_name:selected.provider.display_name,available:true,notice:null };
}
