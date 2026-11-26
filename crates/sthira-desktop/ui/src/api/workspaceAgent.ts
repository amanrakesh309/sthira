import { invoke } from '@tauri-apps/api/core';
import { isTauri } from './ipc';
export interface AgentGoal {
  goal: string;
  project_id: string | null;
  workspace_memory: boolean;
  model_id: string;
  allow_web?: boolean;
  research_template_mode?: 'inherit' | 'none' | 'explicit' | null;
  research_template_id?: string | null;
  plan_template_mode?: 'inherit' | 'none' | 'explicit' | null;
  plan_template_id?: string | null;
}
export type AgentAction = {type:'propose_project_create';name:string;purpose:string;reason:string} | {type:'propose_project_purpose';project_id:string;purpose:string;reason:string} | {type:'propose_move';artifact_id:string;from_project_id:string|null;to_project_id:string;reason:string} | {type:'search_projects';query:string} | {type:'read_project';project_id:string;offset?:number} | {type:'read_connections';artifact_id:string;offset?:number} | {type:'propose_membership';artifact_id:string;project_id:string;attached:boolean;reason:string} | {type:'propose_connection';source_id:string;target_id:string;connected:boolean;reason:string} | {type:'search';query:string} | {type:'read_page';artifact_id:string} | {type:'propose_create';title:string;content_md:string} | {type:'propose_edit';artifact_id:string;content_md:string} | {type:'propose_refine';artifact_id:string;content_md:string} | {type:'propose_derive';artifact_id:string;title:string;content_md:string;derivation_type:string} | {type:'propose_titles';artifact_id:string;titles:string[]} | {type:'list_templates';target_mode?:string|null} | {type:'read_template';template_id:string} | {type:'propose_research';query:string;use_web:boolean;template_id?:string|null} | {type:'propose_plan';goal:string;template_id?:string|null;source_artifact_id?:string|null} | {type:'read_memory';id:string} | {type:'propose_memory_create';scope:string;title:string;body:string;source_ids:string[]} | {type:'propose_memory_update';id:string;expected_version:number;title:string;body:string} | {type:'finish';summary:string;handoff:string};
export interface AgentStep { id:string; sequence:number; action:AgentAction; result:Record<string,unknown>; state:string; artifact_id:string|null; turn_id:string|null; change_set_id:string|null; basis_content_md:string|null }
export interface AgentRun extends Omit<AgentGoal,'allow_web'|'research_template_mode'|'research_template_id'|'plan_template_mode'|'plan_template_id'> {id:string;model_name:string;provider_id:string;provider_name:string;status:string;summary:string|null;handoff:string|null;error:string|null;created_at:number;updated_at:number;steps:AgentStep[];thread_id?:string;parent_run_id?:string|null;document_id?:string|null;document_turn_id?:string|null;allow_edits?:boolean;revision?:number;allow_web?:boolean;research_template_mode?:string;research_template_id?:string|null;plan_template_mode?:string;plan_template_id?:string|null;search_provider_id?:string|null;search_provider_name?:string|null;progress_stage?:string|null;progress_message?:string|null}
export const isAgentActive = (run:{status:string}|null) => !!run && ['queued','running'].includes(run.status);
function native<T>(command:string,args?:Record<string,unknown>):Promise<T> {
  if (!isTauri) return Promise.reject(new Error('Agent runs require the desktop app and a configured model. Browser preview does not simulate agent work.'));
  return invoke<T>(command,args);
}
export const listAgentRuns = () => isTauri ? native<AgentRun[]>('cmd_list_agent_runs') : Promise.resolve([]);
export const getAgentRun = (id:string) => native<AgentRun>('cmd_get_agent_run',{id});
export const createAgentRun = (goal:AgentGoal) => changed(native<AgentRun>('cmd_create_agent_run',{goal}));
export const executeAgentRun = (id:string) => changed(native<AgentRun>('cmd_execute_agent_run',{id}));
export const cancelAgentRun = (id:string) => changed(native<AgentRun>('cmd_cancel_agent_run',{id}));
export const reviewAgentStep = (runId:string,stepId:string,apply:boolean,title?:string) => changed(native<AgentRun>('cmd_review_agent_step',{runId,stepId,apply,title:title||null}));
export const undoAgentEdit = (runId:string,stepId:string) => changed(native<AgentRun>('cmd_undo_agent_edit',{runId,stepId}));

export interface AgentTask {id:string;thread_id:string;parent_run_id:string|null;goal:string;project_id:string|null;document_id:string|null;model_name:string;provider_name:string;status:string;error:string|null;updated_at:number;revision:number}
export const AGENT_TASKS_CHANGED='sthira:agent-tasks-changed';
export const OPEN_AGENT_TASKS='sthira:open-agent-tasks';
export function notifyAgentTasksChanged() {window.dispatchEvent(new Event(AGENT_TASKS_CHANGED));}
async function changed<T>(pending:Promise<T>):Promise<T>{const value=await pending;notifyAgentTasksChanged();return value;}
export async function listAgentTasks():Promise<AgentTask[]> {if(!isTauri)return [];const value=await native<AgentTask[]>('cmd_list_agent_tasks');if(!Array.isArray(value))throw new Error('Could not load agent tasks. Retry to reconnect.');return value;}
export const followUpAgentRun=(parentId:string,text:string,modelId?:string)=>changed(native<AgentRun>('cmd_follow_up_agent_run',{parentId,text,modelId:modelId||null}));
export function openAgentTasks(){window.dispatchEvent(new Event(OPEN_AGENT_TASKS));}

export const isOrganizationProposal=(action:AgentAction)=>action.type==='propose_membership'||action.type==='propose_connection';

export const isProjectProposal=(action:AgentAction)=>['propose_project_create','propose_project_purpose','propose_move'].includes(action.type);
