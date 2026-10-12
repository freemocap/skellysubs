use super::*;
use crate::application::conversation_reads::read_conversation_since;

/// Ordered operating-system preferences, read without changing device settings.
#[tauri::command]
pub(in crate::application) fn preferred_languages() -> Vec<String> {
    sys_locale::get_locales().collect()
}

/// Why the workspace could not be opened, or null when it opened. This is the one
/// command that answers before any store exists, so the window can always report
/// a refusal instead of failing every call with a generic error.
#[tauri::command]
pub(in crate::application) fn get_startup_state(
    state: tauri::State<'_, Arc<Application>>,
) -> StartupState {
    state.startup_state()
}

#[tauri::command]
pub(in crate::application) async fn retry_credential_cleanup(
    state: tauri::State<'_, Arc<Application>>,
) -> Result<StartupState> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.recover_credential_cleanup_with(credentials::remove)
    })
    .await
    .map_err(|cause| {
        crate::diagnostics::failures::join(&cause, "workspace.rs_worker", internal())
    })?
}

#[tauri::command]
pub(in crate::application) async fn get_snapshot(
    state: tauri::State<'_, Arc<Application>>,
) -> Result<Snapshot> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.lock()?.snapshot())
        .await
        .map_err(|cause| {
            crate::diagnostics::failures::join(&cause, "activity_read_worker", internal())
        })?
}

#[tauri::command]
pub(in crate::application) fn execute_command(
    state: tauri::State<'_, Arc<Application>>,
    command: Command,
) -> Result<Receipt> {
    let result = state.execute_command(command);
    state.scheduler_wake.notify_waiters();
    state.conversation_changed.notify_waiters();
    result
}

#[tauri::command]
pub(in crate::application) fn get_message_history(
    state: tauri::State<'_, Arc<Application>>,
    conversation_id: String,
    message_id: String,
) -> Result<crate::conversations::message_history::MessageHistory> {
    state.lock()?.message_history(&conversation_id, &message_id)
}

#[tauri::command]
pub(in crate::application) async fn read_speech_audio(
    state: tauri::State<'_, Arc<Application>>,
    session_id: String,
    operation_id: String,
    sample_offset: Option<u32>,
    execution_id: Option<String>,
) -> Result<model::SpeechAudioState> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = state.lock()?;
        if session_id != store.session_id {
            return Err(AppError::new(
                ErrorCode::SessionExpired,
                "The application session changed. Refresh before continuing.",
            ));
        }
        let audio = store.speech_audio(&operation_id)?;
        let source_execution = store.speech_stream_execution(&operation_id)?;
        if !matches!(audio, model::SpeechAudioState::Unavailable { .. })
            && execution_id
                .as_ref()
                .is_some_and(|previous| source_execution.as_ref() != Some(previous))
        {
            return Err(AppError::new(
                ErrorCode::Conflict,
                "Speech execution changed during playback.",
            ));
        }
        if let model::SpeechAudioState::Pending { message_id, .. } = &audio
            && let Some(id) = source_execution
        {
            let streams = state
                .speech_streams
                .lock()
                .map_err(|_| crate::diagnostics::failures::poisoned(internal()))?;
            let offset = sample_offset.unwrap_or(0);
            if let Some(pcm) = streams.read(&id, offset as usize)? {
                use base64::Engine;
                let pcm = pcm.to_vec();
                let alignment = streams.alignment(&id);
                drop(streams);
                drop(store);
                return Ok(model::SpeechAudioState::Streaming {
                    operation_id,
                    message_id: message_id.clone(),
                    execution_id: id.clone(),
                    sample_offset: offset,
                    alignment,
                    audio_base64: base64::engine::general_purpose::STANDARD.encode(pcm),
                });
            }
            if execution_id.is_some() {
                return Err(AppError::new(
                    ErrorCode::Conflict,
                    "Speech stream delivery expired.",
                ));
            }
        }
        Ok(audio)
    })
    .await
    .map_err(|cause| crate::diagnostics::failures::join(&cause, "speech_audio_read", internal()))?
}

/// Inspect the exact audio already delivered to playback; never consume delivery twice.
#[tauri::command]
pub(in crate::application) async fn inspect_message_speech(
    state: tauri::State<'_, Arc<Application>>,
    session_id: String,
    operation_id: String,
    attempt_id: String,
    audio_base64: String,
    speech_alignment: Option<crate::speech::alignment::SpeechAlignment>,
) -> Result<crate::speech::analysis::audio_inspection::AudioInspection> {
    let audio = crate::speech::alignment::SpeechAudio {
        audio_base64,
        alignment: speech_alignment,
    };
    if audio.audio_base64.len() > 36 * 1024 * 1024 {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Speech audio exceeds inspection limits.",
        ));
    }
    let owner = {
        let store = state.lock()?;
        if session_id != store.session_id {
            return Err(AppError::new(
                ErrorCode::SessionExpired,
                "The audio workspace changed.",
            ));
        }
        store.delivered_speech_owner(&operation_id, &attempt_id, &audio)?
    };
    let wav = audio.wav()?;
    let mut inspection = state.inner().inspect_audio(wav, attempt_id, owner).await?;
    let timing = audio
        .alignment
        .as_ref()
        .and_then(|a| a.words(inspection.duration));
    crate::speech::analysis::audio_inspection::attach_words(&mut inspection, timing.as_ref());
    Ok(inspection)
}

#[tauri::command]
pub(in crate::application) fn get_profile(
    state: tauri::State<'_, Arc<Application>>,
) -> Result<ProfileSnapshot> {
    state.lock()?.profile()
}

#[tauri::command]
pub(in crate::application) async fn open_ai_window(app: tauri::AppHandle) -> Result<()> {
    if let Some(window) = app.get_webview_window("ai") {
        window.show().map_err(|cause| {
            crate::diagnostics::failures::platform(&cause, "window_show", &[], internal())
        })?;
        window.set_focus().map_err(|cause| {
            crate::diagnostics::failures::platform(&cause, "window_focus", &[], internal())
        })?;
    } else {
        let window = tauri::WebviewWindowBuilder::new(
            &app,
            "ai",
            tauri::WebviewUrl::App("index.html?view=ai".into()),
        )
        .title("SkellySpeak · AI activity")
        .inner_size(1000.0, 700.0)
        .min_inner_size(380.0, 400.0)
        .build()
        .map_err(|cause| {
            crate::diagnostics::failures::platform(
                &cause,
                "window_open",
                &[],
                AppError::new(ErrorCode::Internal, "Could not open the AI window."),
            )
        })?;
        // The main window re-reads the window state on this hint; it never
        // trusts the event alone, so a missed event cannot leave it stale.
        let handle = app.clone();
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                let _ = handle.emit_to("main", "ai-window-changed", ());
            }
        });
    }
    let _ = app.emit_to("main", "ai-window-changed", ());
    Ok(())
}

/// Whether this build can pop the AI View out, and whether its window exists.
/// The authoritative answer the main window reconciles against, after reloads
/// and missed events alike.
#[tauri::command]
pub(in crate::application) fn ai_window_state(
    app: tauri::AppHandle,
) -> crate::model::AiWindowState {
    crate::model::AiWindowState {
        supported: cfg!(desktop),
        open: app.get_webview_window("ai").is_some(),
    }
}

/// Pop the AI View back into the main window: tell the main window to show
/// its panel, then close the separate window.
#[tauri::command]
pub(in crate::application) async fn dock_ai_window(app: tauri::AppHandle) -> Result<()> {
    app.emit_to("main", "ai-view-docked", ()).map_err(|_| {
        AppError::new(
            ErrorCode::Internal,
            "Could not return the AI View to the main window.",
        )
    })?;
    if let Some(window) = app.get_webview_window("ai") {
        window.close().map_err(|cause| {
            crate::diagnostics::failures::platform(
                &cause,
                "window_close",
                &[],
                AppError::new(ErrorCode::Internal, "Could not close the AI window."),
            )
        })?;
    }
    Ok(())
}

#[tauri::command]
pub(in crate::application) fn set_ai_view_selection(
    state: tauri::State<'_, Arc<Application>>,
    selection: Option<crate::model::AiViewSelection>,
) -> Result<()> {
    *state
        .ai_view_selection
        .lock()
        .map_err(|_| crate::diagnostics::failures::poisoned(internal()))? = selection;
    Ok(())
}

#[tauri::command]
pub(in crate::application) fn get_ai_view_selection(
    state: tauri::State<'_, Arc<Application>>,
) -> Result<Option<crate::model::AiViewSelection>> {
    Ok(state
        .ai_view_selection
        .lock()
        .map_err(|_| crate::diagnostics::failures::poisoned(internal()))?
        .clone())
}

/// Older turns of one conversation, keyed by turn so the AI View's history
/// reaches every recorded turn.
#[tauri::command]
pub(in crate::application) async fn list_turn_history(
    state: tauri::State<'_, Arc<Application>>,
    conversation_id: String,
    before: Option<String>,
    limit: u32,
) -> Result<crate::model::TurnHistoryPage> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .lock()?
            .turn_history(&conversation_id, before.as_deref(), limit)
    })
    .await
    .map_err(|cause| {
        crate::diagnostics::failures::join(&cause, "activity_read_worker", internal())
    })?
}

#[tauri::command]
pub(in crate::application) async fn read_graph_history(
    state: tauri::State<'_, Arc<Application>>,
    conversation_id: String,
    run_id: String,
    before: Option<String>,
    limit: u32,
) -> Result<crate::ai::graph::RunHistory> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let prepared = {
            let store = state.lock()?;
            let engine: String = store.connection.query_row(
                "SELECT r.engine_id FROM graph_conversation_runs r JOIN turns t ON t.id=r.turn_id WHERE r.run_id=?1 AND t.conversation_id=?2",
                rusqlite::params![run_id, conversation_id], |r| r.get(0))?;
            crate::ai::inspection::prepare(&store.connection, &engine, &run_id)?
        };
        let key = serde_json::to_string(&(prepared.cache_identity()?, &run_id, &before, limit))?;
        state.inspect_cached(key, || prepared.history(&run_id, before.as_deref(), limit))
    })
    .await
    .map_err(|cause| {
        crate::diagnostics::failures::join(&cause, "activity_read_worker", internal())
    })?
}

/// Conditional full snapshots combine observation and hydration without an event gap.
#[tauri::command]
pub(in crate::application) async fn watch_conversation(
    state: tauri::State<'_, Arc<Application>>,
    conversation_id: String,
    after_revision: i32,
    before: Option<i32>,
) -> Result<tauri::ipc::Response> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    loop {
        let changed = state.conversation_changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        if let Some(snapshot) = read_conversation_since(
            state.inner().clone(),
            conversation_id.clone(),
            before,
            after_revision,
            tokio::time::Instant::now() >= deadline,
        )
        .await?
        {
            return Ok(snapshot.response());
        }
        tokio::select! {
            _ = changed => {},
            _ = tokio::time::sleep(Duration::from_millis(150)) => {},
        }
    }
}

/// Inspect bundled teaching content without mutating learner settings.
#[tauri::command]
pub(in crate::application) fn inspect_language(
    state: tauri::State<'_, Arc<Application>>,
    language: String,
    variety: Option<String>,
    explanation: String,
    explanation_variety: Option<String>,
) -> Result<crate::configuration::LanguageInspection> {
    Ok(state.lock()?.config.inspect_language(
        &language,
        variety.as_deref(),
        &explanation,
        explanation_variety.as_deref(),
    )?)
}

/// Pure local preview using the same parser and renderer as accepted turns.
#[tauri::command]
pub(in crate::application) fn preview_conversation_prompt(
    state: tauri::State<'_, Arc<Application>>,
    conversation_id: String,
    configuration: Option<crate::conversations::direction::ConversationStartConfig>,
    yaml: Option<String>,
) -> Result<crate::conversations::direction::PromptPreview> {
    let configuration=match (configuration,yaml) {
        (Some(value),None)=>value,
        (None,Some(text)) if text.len()<=16000 => serde_yaml_ng::from_str(&text).map_err(|cause|crate::diagnostics::failures::yaml(&cause,"configuration_yaml",AppError::new(ErrorCode::Validation,"Invalid configuration YAML. Check required fields, values, duplicate keys and indentation.")))?,
        _=>return Err(AppError::new(ErrorCode::Validation,"Supply a configuration or at most 16 KB of YAML.")),
    };
    let store = state.lock()?;
    let snapshot = store.snapshot()?;
    let mut preview = crate::conversations::conversation_prompt::preview(
        &store.config,
        &snapshot,
        &conversation_id,
        &configuration,
    )?;
    let mut conversation = snapshot
        .conversations
        .iter()
        .find(|c| c.id == conversation_id)
        .cloned()
        .ok_or_else(|| AppError::new(ErrorCode::NotFound, "Conversation not found."))?;
    conversation.settings = crate::conversations::direction::settings(
        &store.config,
        &conversation.language_id,
        &conversation.settings,
        &configuration,
    )?;
    let recommendation = crate::learning::recommendations::capture(
        &store.connection,
        &store.config,
        &snapshot.session_id,
        &conversation,
        &configuration.direction,
    )?;
    crate::learning::recommendations::append_prompt(
        &mut preview.system_prompt,
        &store.config,
        recommendation.as_ref(),
    )?;
    preview.coach_focus = recommendation
        .as_ref()
        .map(|value| {
            let selection: crate::learning::recommendations::Recommendation =
                serde_json::from_value(value["selection"].clone())?;
            let skill: crate::learning::practice_assessment::SkillPrompt =
                serde_json::from_value(value["skill"].clone())?;
            Ok::<_, AppError>(crate::conversations::direction::CoachFocusPreview {
                skill_id: skill.id,
                name: skill.name,
                mode: selection.selected,
                experience: selection.skill.experience,
                effort: selection.skill.effort,
            })
        })
        .transpose()?;
    Ok(preview)
}

/// Application blueprints, available without a selected conversation or AI access.

#[tauri::command]
pub(in crate::application) fn get_practice_view(
    state: tauri::State<'_, Arc<Application>>,
) -> Result<crate::model::PracticeView> {
    state.lock()?.practice_view()
}
#[tauri::command]
pub(in crate::application) fn set_practice_view(
    state: tauri::State<'_, Arc<Application>>,
    view: crate::model::PracticeView,
) -> Result<()> {
    state.lock()?.set_practice_view(view)
}

/// Inspect retained evidence at the selected cut without dispatch or disclosure writes.
#[tauri::command]
pub(in crate::application) async fn read_native_graph_attempt(
    state: tauri::State<'_, Arc<Application>>,
    engine: String,
    run: String,
    revision: String,
    node: String,
    attempt: String,
) -> Result<crate::ai::inspection::NativeAttemptInspection> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let prepared = crate::ai::inspection::prepare(&state.lock()?.connection, &engine, &run)?;
        let key =
            serde_json::to_string(&(prepared.cache_identity()?, &run, &revision, &node, &attempt))?;
        state.inspect_cached(key, || {
            prepared.attempt(&engine, &run, &revision, &node, &attempt)
        })
    })
    .await
    .map_err(|cause| {
        crate::diagnostics::failures::join(&cause, "activity_read_worker", internal())
    })?
}

#[tauri::command]
pub(in crate::application) async fn list_native_workspace_runs(
    state: tauri::State<'_, Arc<Application>>,
    before: Option<String>,
) -> Result<crate::ai::inspection::NativeRunPage> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::ai::inspection::workspace_runs(&state.lock()?.connection, before.as_deref())
    })
    .await
    .map_err(|cause| {
        crate::diagnostics::failures::join(&cause, "activity_read_worker", internal())
    })?
}
#[tauri::command]
pub(in crate::application) async fn read_native_run_history(
    state: tauri::State<'_, Arc<Application>>,
    engine: String,
    run: String,
    before: Option<String>,
) -> Result<crate::ai::graph::RunHistory> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let prepared = crate::ai::inspection::prepare(&state.lock()?.connection, &engine, &run)?;
        let key = serde_json::to_string(&(prepared.cache_identity()?, &run, &before, 32))?;
        state.inspect_cached(key, || prepared.history(&run, before.as_deref(), 32))
    })
    .await
    .map_err(|cause| {
        crate::diagnostics::failures::join(&cause, "activity_read_worker", internal())
    })?
}

#[tauri::command]
pub(in crate::application) async fn read_native_workspace_graph(
    state: tauri::State<'_, Arc<Application>>,
    engine: String,
    run: String,
    after: Option<String>,
) -> Result<Option<crate::ai::graph::InspectionSnapshot>> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = state.lock()?;
        crate::ai::inspection::current(
            &store.connection,
            &store.workspace_graphs,
            &store.graph_runtime,
            &engine,
            &run,
            after.as_deref(),
        )
    })
    .await
    .map_err(|cause| {
        crate::diagnostics::failures::join(&cause, "activity_read_worker", internal())
    })?
}
