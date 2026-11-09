import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './ipc';
export async function getWorkspaceSessionKey():Promise<string> {
  if (!isTauri) return 'preview';
  const value=await invoke<string>('cmd_get_workspace_session_key');
  if(typeof value!=='string'||!value)throw new Error('Workspace session is unavailable.');
  return value;
}
