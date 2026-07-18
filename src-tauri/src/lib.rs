pub mod accounts;
pub mod action_plan;
pub mod audit;
pub mod auth;
pub mod checks;
mod commands;
pub mod compliance;
pub mod config;
pub mod core;
pub mod data_protection;
pub mod engine;
pub mod enterprise;
pub mod error;
pub mod filing;
pub mod finance;
pub mod fraud;
pub mod identity;
pub mod integrations;
pub mod intelligence;
pub mod mcp;
pub mod organizer;
pub mod performance;
mod platform;
pub mod policy;
pub mod runtime;
pub mod storage;
pub mod telemetry;

#[cfg(test)]
mod test_support;

macro_rules! run_with_commands {
    ($builder:expr_2021 $(, $experimental_command:path)* $(,)?) => {
        $builder
            .invoke_handler(tauri::generate_handler![
                // Stable core: recording, replay, inspection, workflow storage, and permissions.
                commands::start_recording,
                commands::stop_recording,
                commands::replay_workflow,
                commands::approve_routine_replay,
                commands::ghost_guard_audit,
                commands::ghost_guard_audit_compressed,
                commands::routine_policy_plan,
                commands::get_replay_history,
                commands::replay_check_unfinished_run,
                commands::replay_dismiss_unfinished_run,
                commands::replay_undo,
                commands::get_replay_progress,
                commands::dry_run_workflow,
                commands::cancel_replay,
                commands::pause_replay,
                commands::resume_replay,
                commands::is_replay_paused,
                commands::is_replay_running,
                commands::set_playback_speed,
                commands::get_playback_speed,
                commands::inspect_element,
                commands::inspect_element_at_cursor,
                // Local OCR on user-selected images (macOS Vision / Windows OCR).
                commands::run_ocr_on_image,
                // Deterministic ID-document parsing over OCR'd text (no image, no IO).
                commands::parse_id_document,
                commands::save_workflow,
                commands::load_workflow,
                commands::delete_workflow,
                commands::list_workflows,
                commands::get_recorded_events,
                commands::check_accessibility,
                commands::request_accessibility,
                commands::check_input_monitoring,
                commands::request_input_monitoring,
                // Relaunch so macOS re-evaluates permission grants (banner CTA).
                commands::restart_app,
                // Deterministic workflow compression for review timeline.
                commands::compress_workflow,
                // Signed auto-update: a read-only check plus a user-approved install.
                commands::check_for_update,
                commands::install_update,
                // Local auth and at-rest workflow protection.
                commands::auth_status,
                commands::auth_setup,
                commands::auth_unlock,
                commands::auth_lock,
                // Account sign-in (Microsoft/Google OAuth + PKCE): identity only,
                // independent of the local vault password above.
                commands::account_status,
                commands::account_sign_in,
                commands::account_sign_out,
                // Configuration, telemetry, and diagnostics.
                commands::get_config,
                commands::update_config,
                commands::get_telemetry_stats,
                commands::export_telemetry,
                commands::get_performance_summary,
                // Always registered so the frontend can detect whether the
                // experimental surface is compiled in and gate its UI.
                commands::is_experimental_enabled,
                // Ghost Organizer: the wedge product's trust pipeline, end to end.
                // Plan is read-only; execute/undo mutate only inside approved Zones,
                // writing an audit log and undo journal for every run.
                commands::organizer_list_zones,
                commands::organizer_list_folder_rules,
                commands::organizer_default_paths,
                commands::organizer_create_zone,
                commands::organizer_add_folder_rule,
                commands::organizer_set_rule_trust,
                commands::organizer_plan,
                commands::organizer_execute,
                commands::organizer_list_executions,
                commands::organizer_check_unfinished_run,
                commands::organizer_dismiss_unfinished_run,
                commands::organizer_undo,
                commands::organizer_export_audit,
                commands::organizer_verify_signed_report,
                commands::organizer_export_policy_pack,
                commands::organizer_import_policy_pack,
                commands::organizer_time_to_value,
                commands::organizer_verify_audit_chain,
                commands::organizer_issue_mcp_approval_token,
                commands::routine_issue_mcp_approval_token,
                // Ghost 2.0 — unified action plan pipeline
                commands::action_plan_from_zone,
                commands::action_plan_from_events,
                commands::action_plan_demo,
                commands::execute_action_plan,
                commands::execute_routine_action_plan,
                commands::get_execution_receipt,
                commands::undo_action_plan_execution,
                commands::mcp_pairing_status,
                commands::mcp_enable_pairing,
                commands::mcp_disable_pairing,
                commands::mcp_list_pending_approvals,
                // Read-only, audience-aware filing preview + savings estimate.
                // Name-only planning; no filesystem/network access.
                commands::preview_file_filing,
                commands::estimate_filing_savings,
                $($experimental_command,)*
            ])
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    };
}

#[cfg(feature = "experimental")]
macro_rules! run_experimental_app {
    ($builder:expr) => {
        run_with_commands!(
            $builder,
            commands::analyze_workflow,
            commands::optimize_workflow,
            commands::suggest_workflow_name,
            commands::save_workflow_with_metadata,
            commands::load_workflow_with_metadata,
            commands::generate_workflow_from_prompt,
            commands::analyze_and_tag_workflow,
            commands::save_workflow_with_sidecar,
            commands::replay_with_reliability,
            commands::init_cloud_sync,
            commands::cloud_authenticate,
            commands::cloud_sync_workflows,
            commands::create_workspace,
            // Internal intelligence providers (suggestion-only planning; sends
            // redacted metadata to a remote provider once configured).
            commands::intelligence_provider_status,
            commands::intelligence_set_api_key,
            commands::intelligence_clear_api_key,
            commands::intelligence_test_provider,
            commands::intelligence_propose_plan,
            commands::intelligence_discover_local,
            commands::organizer_intelligence_suggest,
            // Power BI audit-export: grant flow, preview, and push (real
            // network write to a third-party paid service — see
            // docs/power-bi-integration.md).
            commands::power_bi_grant_status,
            commands::power_bi_request_grant,
            commands::power_bi_revoke_grant,
            commands::power_bi_export_preview,
            commands::power_bi_push_audit_export,
            commands::fabric_grant_status,
            commands::fabric_request_grant,
            commands::fabric_revoke_grant,
            commands::fabric_list_workspaces,
            commands::fabric_export_preview,
            commands::fabric_list_lakehouses,
            commands::fabric_push_audit_export,
            commands::fabric_list_inbound_intents,
            commands::fabric_dismiss_inbound_intent,
            commands::fabric_record_inbound_intent,
            commands::fabric_webhook_status,
            commands::fabric_set_webhook_secret,
            commands::google_grant_status,
            commands::google_request_grant,
            commands::google_revoke_grant,
            commands::google_list_buckets,
            commands::google_export_preview,
            commands::google_bind_export_bucket,
            commands::google_push_audit_export,
            commands::mcp_http_server_status,
            commands::mcp_start_http_server,
            commands::mcp_stop_http_server,
            commands::mcp_relay_status,
            commands::mcp_start_relay,
            commands::mcp_stop_relay,
            commands::get_audit_logs,
            commands::get_execution_history,
            commands::get_all_executions,
            commands::get_workflow_analytics,
            commands::replay_with_visual_check,
            commands::capture_baseline_screenshot,
            commands::create_data_source,
            commands::load_variables,
            commands::start_observer,
            commands::stop_observer,
            commands::is_observer_active,
            commands::set_observer_interval,
            commands::observe_events,
            commands::get_proactive_suggestions,
            commands::get_learned_patterns,
            commands::get_app_usage_stats,
            commands::generate_geek_insights,
        );
    };
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(engine::GhostEngine::new());

    // Cloud sync state only exists when the experimental surface is compiled in.
    #[cfg(feature = "experimental")]
    let builder = builder.manage(commands::CloudState::default());

    #[cfg(feature = "experimental")]
    run_experimental_app!(builder);

    #[cfg(not(feature = "experimental"))]
    run_with_commands!(builder);
}
