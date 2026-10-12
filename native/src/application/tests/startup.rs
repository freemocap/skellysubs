use tauri::Manager;

fn run_setup(mut app: tauri::App<tauri::test::MockRuntime>) {
    // MockRuntime does not implement request_exit, so run_return cannot stop.
    // One iteration runs Tauri's actual setup without a deprecated busy loop.
    #[allow(deprecated)]
    app.run_iteration(|_, _| {});
}

#[test]
fn configured_webviews_do_not_start_before_application_setup() {
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    *context.config_mut() = serde_json::from_str(include_str!("../../../tauri.conf.json")).unwrap();
    let app = tauri::test::mock_builder()
        .setup(|app| {
            assert!(
                app.webview_windows().is_empty(),
                "The frontend must not issue commands before application state is managed"
            );
            Ok(())
        })
        .build(context)
        .unwrap();
    run_setup(app);
}

#[test]
fn main_window_requires_state_and_its_first_startup_command_succeeds() {
    check_first_startup(false);
}

#[test]
fn refused_workspace_still_opens_a_window_and_reports_its_reason() {
    check_first_startup(true);
}

fn check_first_startup(refused: bool) {
    let directory = tempfile::tempdir().unwrap();
    let workspace = directory.path().join("startup.sqlite3");
    if refused {
        std::fs::write(&workspace, b"not a database").unwrap();
    }
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    *context.config_mut() = serde_json::from_str(include_str!("../../../tauri.conf.json")).unwrap();
    let app = tauri::test::mock_builder()
        .invoke_handler(tauri::generate_handler![
            crate::application::commands::workspace::get_startup_state
        ])
        .setup(move |app| {
            assert!(super::open_main_window(app).is_err());
            assert!(app.get_webview_window("main").is_none());
            let state = crate::application::Application::start(&workspace, None);
            assert_eq!(state.refusal().is_some(), refused);
            app.manage(state);
            super::open_main_window(app)?;
            let window = app.get_webview_window("main").unwrap();
            let response = tauri::test::get_ipc_response(
                &window,
                tauri::webview::InvokeRequest {
                    cmd: "get_startup_state".into(),
                    callback: tauri::ipc::CallbackFn(0),
                    error: tauri::ipc::CallbackFn(1),
                    // Use the configured webview origin: Tauri's local URL
                    // differs across platforms and development/release builds.
                    url: window.url().unwrap(),
                    body: tauri::ipc::InvokeBody::Json(serde_json::json!({})),
                    headers: Default::default(),
                    invoke_key: tauri::test::INVOKE_KEY.into(),
                },
            )
            .expect("The first startup IPC must see the managed state")
            .deserialize::<serde_json::Value>()
            .unwrap();
            assert_eq!(!response["refusal"].is_null(), refused);
            Ok(())
        })
        .build(context)
        .unwrap();
    run_setup(app);
}
