import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './ipc';
export interface NamingChoice { model_id: string | null; local: boolean }
export interface TitleSuggestions { artifact_id:string; basis_content_hash:string; basis_title:string|null; titles:string[]; model_name:string; provider_name:string }
export async function suggestDocumentTitles(artifactId:string,choice:NamingChoice):Promise<TitleSuggestions> {
  if (!isTauri) throw new Error('Title suggestions need the desktop app and a configured model.');
  return invoke('cmd_suggest_document_titles',{artifactId,choice});
}

export async function applyDocumentTitle(artifactId:string,title:string,basisHash:string,basisTitle:string|null):Promise<import('../types/sthira').Artifact> {
  if(!isTauri)throw new Error('Applying a suggested title requires the desktop app.');
  return invoke('cmd_apply_document_title',{artifactId,title,basisHash,basisTitle});
}
