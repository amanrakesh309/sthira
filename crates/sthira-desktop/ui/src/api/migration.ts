import {invoke} from '@tauri-apps/api/core';import {isTauri} from './ipc';
export interface ImportFile{path:string;content:string|null;problem?:string}
export interface ImportItem{path:string;title:string;status:string;artifact_id:string;source_key:string;source_hash:string;original:string|null;preview_content:string|null;warnings:string[]}
export interface ImportRun{id:string;revision:number;status:string;plan:{source:string;collection:string;project_id:string|null;items:ImportItem[]};receipt:{selected:string[];pages:{artifact_id:string;warnings:string[]}[];mirror_warnings:{artifact_id:string;error:string}[]}|null;created_at:number}
export interface ImportDraft{request_id:string;source:string;collection:string;project_id:string|null;files:ImportFile[]}
function native<T>(cmd:string,args?:Record<string,unknown>):Promise<T>{if(!isTauri)return Promise.reject(new Error('Imports require the desktop app. Browser preview does not save imported files.'));return invoke<T>(cmd,args);}
export const previewMigration=(draft:ImportDraft)=>native<ImportRun>('cmd_preview_migration',{draft});
export const listMigrations=()=>isTauri?native<ImportRun[]>('cmd_list_migrations'):Promise.resolve([]);
export const getMigration=(id:string)=>native<ImportRun>('cmd_get_migration',{id});
export const applyMigration=(run:ImportRun)=>native<ImportRun>('cmd_apply_migration',{id:run.id,revision:run.revision,paths:run.plan.items.filter(i=>i.status==='new').map(i=>i.path)});
export const cancelMigration=(run:ImportRun)=>native<ImportRun>('cmd_cancel_migration',{id:run.id,revision:run.revision});
export const rollbackMigration=(run:ImportRun)=>native<ImportRun>('cmd_rollback_migration',{id:run.id,revision:run.revision});
