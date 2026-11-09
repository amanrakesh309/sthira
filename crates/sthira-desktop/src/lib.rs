pub mod ai_setup;
pub mod resume;
pub mod naming;
pub mod workspace_session;
pub mod resume_pack;
pub mod conversation;
pub mod workflow_settings;
pub mod commands;
pub mod connections;
pub mod projects;
pub mod events;
pub mod scheduler;
pub mod seed;
pub mod state;

pub use commands::*;
pub use connections::*;
pub use projects::*;
pub use events::spawn_event_bridge;
pub use scheduler::{spawn_scheduled_topic_scheduler, ScheduledTopicReadyEvent};
pub use seed::{clean_duplicate_models_on_startup, seed_demo_workspace_if_empty};
pub use state::AppState;

pub fn register_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        ai_setup::cmd_open_ai_app_download,
        resume::cmd_get_resume_settings,
        resume::cmd_get_resume_model,
        naming::cmd_suggest_document_titles,
        naming::cmd_apply_document_title,
        workspace_session::cmd_get_workspace_session_key,
        resume::cmd_save_resume_settings,
        resume::cmd_generate_resume_brief,
        resume_pack::cmd_get_resume_pack_status,
        resume_pack::cmd_install_resume_pack,
        resume_pack::cmd_remove_resume_pack,
        workflow_settings::cmd_get_workflow_settings,
        workflow_settings::cmd_save_workflow_setting,
        conversation::cmd_get_document_conversation,
        conversation::cmd_select_document_conversation_model,
        conversation::cmd_discuss_document,
        conversation::cmd_decide_document_edit,
        conversation::cmd_undo_document_edit,
        projects::cmd_get_project_overview,
        connections::cmd_get_workspace_entity,
        connections::cmd_search_connection_targets,
        connections::cmd_get_workspace_connections,
        connections::cmd_create_workspace_connection,
        connections::cmd_delete_workspace_connection,
        commands::cmd_route_input,
        commands::cmd_apply_plan,
        commands::cmd_get_home_state,
        commands::cmd_get_workspace_tree,
        commands::cmd_create_project,
        commands::cmd_create_page,
        commands::cmd_get_resume_brief,
        commands::cmd_get_resume_pane_hint,
        commands::cmd_get_project_history,
        commands::cmd_get_artifact_context,
        commands::cmd_save_artifact,
        commands::cmd_save_generated_artifact,
        commands::cmd_decide_proposal,
        commands::cmd_revert_change_set,
        commands::cmd_promote_to_project,
        commands::cmd_canvas_ops,
        commands::cmd_create_guided_project,
        commands::cmd_clear_vault_data,
        commands::cmd_seed_demo_workspace,
        commands::cmd_search_all,
        commands::cmd_synthesize_and_create_research,
        commands::cmd_answer_question,
        commands::cmd_get_provider_catalog,
        commands::cmd_list_available_models,
        commands::cmd_update_model_search_capability,
        commands::cmd_save_provider,
        commands::cmd_discover_ollama_models,
        commands::cmd_discover_provider_models,
        commands::cmd_fetch_provider_models,
        commands::cmd_upsert_model,
        commands::cmd_delete_model,
        commands::cmd_delete_provider,
        commands::cmd_set_default_model,
        commands::cmd_toggle_model,
        commands::cmd_route_input_steered,
        commands::cmd_execute_plan_action,
        commands::cmd_list_object_types,
        commands::cmd_create_object_type,
        commands::cmd_list_objects_by_type,
        commands::cmd_save_object_instance,
        commands::cmd_get_object_type_counts,
        commands::cmd_create_typed_object,
        commands::cmd_update_object_type,
        commands::cmd_delete_object_type,
        commands::cmd_get_type_templates,
        commands::cmd_save_object_template,
        commands::cmd_instantiate_object_from_template,
        commands::cmd_get_object_instance,
        commands::cmd_inspect_project_deletion,
        commands::cmd_delete_project,
        commands::cmd_delete_artifact,
        commands::cmd_get_knowledge_graph,
        commands::cmd_duplicate_artifact,
        commands::cmd_add_artifact_to_project,
        commands::cmd_link_artifact_to_project,
        commands::cmd_move_artifact_to_project,
        commands::cmd_create_project_with_artifact,
        commands::cmd_convert_artifact_to_object,
        commands::cmd_detach_artifact_from_project,
        commands::cmd_duplicate_project,
        commands::cmd_rename_project,
        commands::cmd_rename_artifact,
        commands::cmd_get_search_providers,
        commands::cmd_save_search_provider,
        commands::cmd_test_search_provider,
        commands::cmd_import_web_source_to_vault,
        commands::cmd_execute_web_research,
        commands::cmd_create_research_page,
        commands::cmd_run_research_into_page,
        commands::cmd_stream_adaptive_run,
        commands::cmd_get_sidebar_items,
        commands::cmd_toggle_pin,
        commands::cmd_get_workspace_index,
        commands::cmd_record_access,
        commands::cmd_list_scheduled_topics,
        commands::cmd_create_scheduled_topic,
        commands::cmd_trigger_scheduled_topic_run,
        commands::cmd_toggle_scheduled_topic_active,
        commands::cmd_delete_scheduled_topic,
        commands::cmd_refine_artifact,
        commands::cmd_derive_artifact,
        commands::cmd_cancel_request,
        commands::cmd_list_synthesis_templates,
    ])
}

