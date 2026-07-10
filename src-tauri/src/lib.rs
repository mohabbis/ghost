pub mod audit;
pub mod auth;
mod commands;
pub mod config;
pub mod core;
pub mod engine;
pub mod error;
pub mod filing;
pub mod organizer;
pub mod performance;
mod platform;
pub mod policy;
pub mod storage;
pub mod telemetry;

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

    builder
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
            // experimental surface below is compiled in and gate its UI.
            commands::is_experimental_enabled,
            // Experimental surfaces, compiled only under `--features experimental`.
            // A stock build does not register or expose these; the frontend hides
            // the experimental panel when `is_experimental_enabled` returns false.
            #[cfg(feature = "experimental")]
            commands::analyze_workflow,
            #[cfg(feature = "experimental")]
            commands::optimize_workflow,
            #[cfg(feature = "experimental")]
            commands::suggest_workflow_name,
            #[cfg(feature = "experimental")]
            commands::save_workflow_with_metadata,
            #[cfg(feature = "experimental")]
            commands::load_workflow_with_metadata,
            #[cfg(feature = "experimental")]
            commands::generate_workflow_from_prompt,
            #[cfg(feature = "experimental")]
            commands::analyze_and_tag_workflow,
            #[cfg(feature = "experimental")]
            commands::save_workflow_with_sidecar,
            #[cfg(feature = "experimental")]
            commands::replay_with_reliability,
            #[cfg(feature = "experimental")]
            commands::init_cloud_sync,
            #[cfg(feature = "experimental")]
            commands::cloud_authenticate,
            #[cfg(feature = "experimental")]
            commands::cloud_sync_workflows,
            #[cfg(feature = "experimental")]
            commands::create_workspace,
            #[cfg(feature = "experimental")]
            commands::get_audit_logs,
            #[cfg(feature = "experimental")]
            commands::get_execution_history,
            #[cfg(feature = "experimental")]
            commands::get_all_executions,
            #[cfg(feature = "experimental")]
            commands::get_workflow_analytics,
            #[cfg(feature = "experimental")]
            commands::replay_with_visual_check,
            #[cfg(feature = "experimental")]
            commands::capture_baseline_screenshot,
            #[cfg(feature = "experimental")]
            commands::create_data_source,
            #[cfg(feature = "experimental")]
            commands::load_variables,
            #[cfg(feature = "experimental")]
            commands::start_observer,
            #[cfg(feature = "experimental")]
            commands::stop_observer,
            #[cfg(feature = "experimental")]
            commands::is_observer_active,
            #[cfg(feature = "experimental")]
            commands::set_observer_interval,
            #[cfg(feature = "experimental")]
            commands::observe_events,
            #[cfg(feature = "experimental")]
            commands::get_proactive_suggestions,
            #[cfg(feature = "experimental")]
            commands::get_learned_patterns,
            #[cfg(feature = "experimental")]
            commands::get_app_usage_stats,
            #[cfg(feature = "experimental")]
            commands::generate_geek_insights,
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
            commands::organizer_undo,
            commands::organizer_export_audit,
            commands::organizer_time_to_value,
            commands::organizer_verify_audit_chain,
            // Read-only, audience-aware filing preview + savings estimate.
            // Name-only planning; no filesystem/network access.
            commands::preview_file_filing,
            commands::estimate_filing_savings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
