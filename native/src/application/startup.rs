use super::*;

#[cfg(all(test, desktop))]
#[path = "tests/startup.rs"]
mod tests;

fn open_main_window<R: tauri::Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    // The frontend immediately invokes get_startup_state. Tauri's automatic
    // windows load before our setup hook, racing workspace initialization. [@tauriSetupOrdering2026]
    // Keep the configured window deferred and register state before loading it.
    if app.try_state::<Arc<Application>>().is_none() {
        return Err(std::io::Error::other("Application state must precede the main window").into());
    }
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "main")
        .ok_or_else(|| std::io::Error::other("Main window configuration is missing"))?;
    let window = tauri::WebviewWindowBuilder::from_config(app, config)?.build()?;
    #[cfg(all(desktop, feature = "workflow-capture"))]
    if crate::ai::transport::workflow::is_replay() {
        // Preserve replay's cursor isolation after the deferred window exists.
        window.set_ignore_cursor_events(true)?;
    }
    #[cfg(not(all(desktop, feature = "workflow-capture")))]
    let _ = window;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(not(feature = "workflow-capture"))]
    assert!(
        std::env::var_os("SKELLYSPEAK_WORKFLOW_DIR").is_none(),
        "Workflow recording requires a capture-enabled executable; no workspace was opened"
    );
    let context = tauri::generate_context!();
    #[cfg(feature = "workflow-capture")]
    let context = super::workflow::configure(context)
        .expect("Workflow configuration failed; the ordinary workspace was not opened");
    let builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(diagnostics::sharing::plugin());
    #[cfg(target_os = "ios")]
    let builder = builder.plugin(tauri_plugin_diagnostic_sharing::init());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(tauri_plugin_deep_link::init());
    #[cfg(target_os = "ios")]
    let builder = builder.plugin(crate::ai::hosted::ios_callback_plugin());
    #[cfg(all(desktop, not(feature = "workflow-capture")))]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    #[cfg(all(desktop, feature = "workflow-capture"))]
    let builder = if crate::ai::transport::workflow::is_replay() {
        builder
    } else {
        builder.plugin(tauri_plugin_updater::Builder::new().build())
    };
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_process::init())
        .on_menu_event(|app, event| {
            let action = if event.id() == "reading-size-increase" {
                Some("increase")
            } else if event.id() == "reading-size-decrease" {
                Some("decrease")
            } else if event.id() == "reading-size-reset" {
                Some("reset")
            } else {
                None
            };
            if let Some(action) = action
                && let Err(error) = app.emit("reading-size-action", action)
            {
                eprintln!("Unable to deliver reading-size menu action: {error}");
            }
        });
    builder
        .setup(|app| {
            #[cfg(desktop)]
            {
                let app_menu = SubmenuBuilder::new(app, "SkellySpeak")
                    .about(None)
                    .services()
                    .separator()
                    .hide()
                    .hide_others()
                    .separator()
                    .quit()
                    .build()?;
                let edit_menu = SubmenuBuilder::new(app, "Edit")
                    .undo()
                    .redo()
                    .separator()
                    .cut()
                    .copy()
                    .paste()
                    .select_all()
                    .build()?;
                let view_menu = SubmenuBuilder::new(app, "View")
                    .item(
                        &MenuItemBuilder::with_id("reading-size-increase", "Increase Font Size")
                            .accelerator("CmdOrCtrl+=")
                            .build(app)?,
                    )
                    .item(
                        &MenuItemBuilder::with_id("reading-size-decrease", "Decrease Font Size")
                            .accelerator("CmdOrCtrl+-")
                            .build(app)?,
                    )
                    .item(
                        &MenuItemBuilder::with_id("reading-size-reset", "Reset Font Size")
                            .accelerator("CmdOrCtrl+0")
                            .build(app)?,
                    )
                    .build()?;
                let window_menu = SubmenuBuilder::new(app, "Window")
                    .minimize()
                    .close_window()
                    .build()?;
                app.set_menu(
                    MenuBuilder::new(app)
                        .items(&[&app_menu, &edit_menu, &view_menu, &window_menu])
                        .build()?,
                )?;
            }
            let directory = app.path().app_data_dir()?;
            #[cfg(feature = "workflow-capture")]
            super::workflow::prepare_workspace(&directory)?;
            store::prepare_private_directory(&directory)?;
            // A previous reset may have left the log directory to clear. That has to
            // happen before the log sink below opens a file inside it.
            // Android/iOS use a subdirectory of app data: it is private, writable,
            // and available before the webview starts. Desktop retains the platform
            // log directory (or the development repository sink) in diagnostics.
            #[cfg(any(target_os = "android", target_os = "ios"))]
            let diagnostics_root = directory.join("logs");
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            let diagnostics_root = app.path().app_log_dir()?;
            let cleanup = factory_reset::finish_pending(
                &directory,
                &diagnostics::configured_root(&diagnostics_root)?,
            )
            .err();
            diagnostics::initialize(&diagnostics_root)?;
            let state = Application::start(&directory.join(store::WORKSPACE_FILE), cleanup);
            // A refused workspace has nothing to clean or prepare; the window still
            // opens, the reason reaches the screen, and the reset stays reachable.
            if state.refusal().is_none() {
                state.recover_credential_cleanup_with(credentials::remove)?;
                state.lock()?.prepare_chat()?;
            }
            app.manage(state.clone());
            tauri::async_runtime::spawn(scheduler(state.clone(), app.handle().clone()));
            open_main_window(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            super::workflow::workflow_control,
            commands::drill::start_drill_session,
            commands::drill::end_drill_session,
            commands::drill::enter_drill_visit,
            commands::drill::leave_drill_visit,
            commands::drill::get_drill_sessions,
            commands::drill::get_drill_storage,
            commands::drill::set_drill_storage,
            commands::drill_generation::conversation_drill_candidates,
            commands::drill_generation::get_drill_generation_activity,
            commands::drill_generation::begin_drill_preview,
            commands::drill_generation::cancel_drill_preview,
            commands::drill_generation::preview_drill_items,
            commands::drill_generation::get_drill_preview,
            commands::drill_generation::accept_drill_items,
            commands::drill_generation::discard_drill_preview,
            commands::drill::create_drill_item,
            commands::drill::get_practice_sets,
            commands::drill::preview_practice_set,
            commands::drill::accept_practice_phrases,
            commands::drill::get_drill_items,
            commands::drill::get_last_drill_item,
            commands::drill::drill_attempts,
            commands::drill::delete_drill_item,
            commands::drill::delete_drill_attempt,
            commands::drill::clear_drill_attempts,
            commands::drill::get_drill_attempt_audio,
            commands::drill::inspect_drill_audio,
            commands::reading::begin_reading,
            commands::reading::run_reading,
            commands::skill_guides::get_skill_guide,
            commands::reading::get_cached_reading_audio,
            commands::reading::cancel_reading,
            commands::reading::get_reading_activity,
            commands::reading::get_saved_gloss_sources,
            updater::get_update_channel,
            updater::latest_github_release,
            commands::workspace::read_speech_audio,
            commands::workspace::inspect_message_speech,
            commands::cache::get_inference_cache_settings,
            commands::cache::save_inference_cache_settings,
            diagnostics::record_frontend_diagnostic,
            diagnostics::sharing::share_diagnostic_logs,
            diagnostics::sharing::save_diagnostic_logs,
            diagnostics::read_frontend_diagnostics,
            crate::speech::recording::microphone::list_microphones,
            crate::speech::recording::microphone::get_microphone,
            crate::speech::recording::microphone::save_microphone,
            crate::speech::recording::microphone_test::microphone_test_start,
            crate::speech::recording::microphone_test::microphone_test_samples,
            crate::speech::recording::microphone_test::microphone_test_stop,
            voice::mic_start,
            crate::speech::recording::continuous::mic_listen_start,
            crate::speech::recording::continuous::mic_listen_push,
            crate::speech::recording::continuous::mic_listen_status,
            crate::speech::recording::continuous::mic_listen_spectrogram,
            crate::speech::recording::continuous::mic_listen_stop,
            crate::speech::recording::continuous::mic_listen_discard,
            crate::speech::recording::continuous::mic_listen_tune,
            voice::mic_wave,
            voice::mic_spectrogram,
            voice::mic_push,
            voice::mic_cancel,
            voice::mic_transcribe,
            voice::mic_retry_transcription,
            voice::get_transcription_result,
            factory_reset::factory_reset,
            factory_reset::export_workspace,
            commands::workspace::get_startup_state,
            commands::workspace::retry_credential_cleanup,
            commands::workspace::get_snapshot,
            commands::workspace::preferred_languages,
            commands::workspace::preview_conversation_prompt,
            commands::workspace::inspect_language,
            commands::workspace::execute_command,
            access::get_access_settings,
            access::save_access_settings,
            access::check_access,
            commands::local_server::local_server_available,
            commands::local_server::connect_local_server,
            commands::local_server::open_local_admin,
            commands::connections::get_connection,
            commands::connections::save_models,
            commands::workspace::watch_conversation,
            commands::workspace::get_message_history,
            commands::partners::begin_persona_generation,
            commands::partners::run_persona_generation,
            commands::partners::cancel_persona_generation,
            commands::hosted::hosted_sign_in,
            commands::hosted::hosted_account,
            commands::hosted::hosted_diagnostics,
            commands::hosted::hosted_sign_out,
            commands::hosted::cancel_sign_in,
            commands::connections::select_route,
            commands::workspace::get_profile,
            commands::partners::get_persona_generation_activity,
            progression::get_skill_evidence,
            learner_state::get_learner_state,
            learner_state::get_learner_profile,
            rewards::claim_reward_events,
            rewards::skill_level_events::initialize_skill_level_events,
            rewards::skill_level_events::claim_skill_level_events,
            crate::learning::effort::get_effort_progress,
            crate::learning::streaks::commands::refresh_streaks,
            crate::learning::effort::bot::record_bot_inspection,
            crate::learning::effort::report::get_effort_report,
            crate::learning::effort::claim_effort_awards,
            learner_state::export_learner_state,
            learner_state::save_learner_state,
            conversation_export::view_conversation_yaml,
            conversation_export::save_conversation_yaml,
            reward_settings::get_reward_settings,
            reward_settings::get_playback_rate,
            reward_settings::save_playback_rate,
            reward_settings::save_reward_settings,
            progression::save_skill_profile,
            progression::get_practice_overview,
            progression::get_language_totals,
            commands::workspace::open_ai_window,
            commands::workspace::get_practice_view,
            commands::workspace::set_practice_view,
            commands::workspace::list_turn_history,
            commands::workspace::read_graph_history,
            commands::workspace::read_native_graph_attempt,
            content::read_original_content,
            content::list_original_content,
            content::recording_original_content,
            commands::workspace::list_native_workspace_runs,
            commands::workspace::read_native_run_history,
            commands::workspace::read_native_workspace_graph,
            commands::workspace::ai_window_state,
            commands::workspace::dock_ai_window,
            commands::workspace::set_ai_view_selection,
            commands::workspace::get_ai_view_selection,
        ])
        .run(context)
        .expect("SkellySpeak could not start; no reset or fallback was performed");
}
