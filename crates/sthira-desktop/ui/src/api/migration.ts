import {invoke} from '@tauri-apps/api/core';
import {isTauri} from './ipc';
export interface ImportFile {path:string;content:string|null;problem?:string}
export interface ImportItem {path:string;title:string;status:string;artifact_id:string;source_key:string;source_hash:string;original:string|null;preview_content:string|null;warnings:string[]}
export interface ImportRun {id:string;revision:number;status:string;plan:{source:string;collection:string;project_id:string|null;items:ImportItem[]};receipt:{selected:string[];pages:{artifact_id:string;warnings:string[]}[];mirror_warnings:{artifact_id:string;error:string}[]}|null;created_at:number}
export interface ImportDraft {request_id:string;source:string;collection:string;project_id:string|null;files:ImportFile[]}
export interface HistoryCursor {created_at:number;id:string}
export interface ImportSummary {id:string;revision:number;status:string;created_at:number;counts:{source:string;collection:string;project_id:string|null;total:number;new:number;duplicate:number;conflict:number;unsupported:number}}
export interface ImportHistory {items:ImportSummary[];next_cursor:HistoryCursor|null}
export interface ImportSelection {id:string;revision:number;paths:string[];basis:string;items:ImportItem[]}
export interface GuidanceInput {request_id:string;import_id:string;revision:number;paths:string[];basis:string;model_id:string;instructions:string;include_excerpts:boolean}
export interface GuidancePlan {summary:string;mappings:{path:string;choice:'import_as_page'|'review_first'|'leave_out';reason:string;suggested_group:string|null}[];cautions:string[]}
export interface MigrationGuidance {id:string;import_id:string;status:string;input:GuidanceInput;model_name:string;provider_name:string;result:GuidancePlan|null;error:string|null;created_at:number}
function native<T>(cmd:string,args?:Record<string,unknown>):Promise<T>{if(!isTauri)return Promise.reject(new Error('Imports require the desktop app. Browser preview does not save imported files.'));return invoke<T>(cmd,args);}
export const previewMigration=(draft:ImportDraft)=>native<ImportRun>('cmd_preview_migration',{draft});
export const listMigrations=(before:HistoryCursor|null=null)=>isTauri?native<ImportHistory>('cmd_list_migrations',{limit:10,before}):Promise.resolve({items:[],next_cursor:null} as ImportHistory);
export const getMigration=(id:string)=>native<ImportRun>('cmd_get_migration',{id});
export const reviewMigrationSelection=(run:ImportRun,paths:string[])=>native<ImportSelection>('cmd_review_migration_selection',{id:run.id,revision:run.revision,paths});
export const applyMigration=(run:ImportRun,review:ImportSelection)=>native<ImportRun>('cmd_apply_migration',{id:run.id,revision:run.revision,paths:review.paths,basis:review.basis});
export const cancelMigration=(run:ImportRun)=>native<ImportRun>('cmd_cancel_migration',{id:run.id,revision:run.revision});
export const rollbackMigration=(run:ImportRun)=>native<ImportRun>('cmd_rollback_migration',{id:run.id,revision:run.revision});
export const startMigrationGuidance=(input:GuidanceInput)=>native<MigrationGuidance>('cmd_start_migration_guidance',{input});
export const getMigrationGuidance=(id:string)=>native<MigrationGuidance>('cmd_get_migration_guidance',{id});
export const latestMigrationGuidance=(importId:string)=>native<MigrationGuidance|null>('cmd_latest_migration_guidance',{importId});
export const cancelMigrationGuidance=(id:string)=>native<MigrationGuidance>('cmd_cancel_migration_guidance',{id});
