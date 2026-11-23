import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './ipc';
import { notifyAgentTasksChanged, type AgentRun } from './workspaceAgent';
export interface HandoffConnection {id:string;grant:{name:string;whole_workspace:boolean;project_id:string|null;workspace_memory:boolean;can_propose:boolean;model:string;provider:string};revoked_at:number|null}
export interface HandoffSource {kind:'page'|'memory';id:string;title:string|null;version:number;authorship?:string;scope?:{kind:string;id?:string}}
export interface Handoff {id:string;source_run_id:string;recipient_connection_id:string;revision:number;created_at:number;closed_at:number|null;snapshot:{title:string;goal:string;summary:string|null;source_carry_forward:string|null;next_steps:string;open_questions:string;source_model:string;source_provider:string;artifact_ids:string[];memory_ids:string[];basis:{sources:HandoffSource[]};run_scope:{project_id:string|null;workspace_memory:boolean}}}
export interface HandoffReview {run_id:string;revision:number;status:string;proposals:{step_id:string;type:string;state:string;artifact_id:string|null;project_id:string|null}[]}
export interface HandoffView {handoff:Handoff;current_basis:{sources?:HandoffSource[];project?:{id:string;name:string}|null;unavailable?:boolean};sources_changed:boolean;sources_available:boolean;context_version:string;source_review:HandoffReview;contributions:{run_id:string;model:string;provider:string;summary:string;next_steps:string;open_questions:string;review:HandoffReview;created_at:number}[];contributions_omitted:boolean;content_is_untrusted:boolean}
export interface HandoffDraft {request_id:string;run_id:string;expected_run_revision:number;recipient_connection_id:string;title:string;next_steps:string;open_questions:string;artifact_ids:string[];memory_ids:string[]}
export interface HandoffContinuation {id:string;request_id:string;expected_context_version:string;text:string;model_id:string}
function native<T>(command:string,args?:Record<string,unknown>):Promise<T>{if(!isTauri)return Promise.reject(new Error('Saved handoffs require the desktop app. Browser preview does not share workspace data.'));return invoke<T>(command,args);}
async function changed<T>(promise:Promise<T>){const result=await promise;notifyAgentTasksChanged();return result;}
export const listHandoffConnections=()=>isTauri?native<HandoffConnection[]>('cmd_list_handoff_connections'):Promise.resolve([]);
export const listAgentHandoffs=(runId:string)=>isTauri?native<Handoff[]>('cmd_list_agent_handoffs',{runId}):Promise.resolve([]);
export const getAgentHandoff=(id:string)=>native<HandoffView>('cmd_get_agent_handoff',{id});
export const saveAgentHandoff=(draft:HandoffDraft)=>changed(native<HandoffView>('cmd_save_agent_handoff',{draft}));
export const closeAgentHandoff=(id:string,expectedRevision:number)=>changed(native<HandoffView>('cmd_close_agent_handoff',{id,expectedRevision}));
export const continueAgentHandoff=(input:HandoffContinuation)=>changed(native<AgentRun>('cmd_continue_agent_handoff',{input}));
