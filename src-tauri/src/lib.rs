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
pub mod organizer;
pub mod performance;
mod platform;
pub mod policy;
pub mod storage;
pub mod telemetry;

macro_rules! run_with_commands {
    ($builder:expr $(, $experimental_command:path)* $(,)?) => {
        $builder
            .invoke_handler(tauri::generate_handler![
                // Stable core: recording, replay, inspection, workflow storage, and permissions.
                commands::start_recording,
                commands::stop_recording,
                commands::replay_workflow,
                commands::ghost_guard_audit,
                commands::ghost_guard_audit_compressed,
                commands::get_replay_history,
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
