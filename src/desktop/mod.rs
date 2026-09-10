mod commands;
mod dictation_coordinator;
mod runtime_assets;
mod tray;

use std::sync::{Arc, RwLock};

use floating_whisprer::{
    CpalAudioRecorder, DictationSession, DictationState, DirectTextInserter, InputLevel,
    WhisperCliEngine,
};
use serde::Serialize;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

use crate::platform::{self, PermissionManager, PermissionSnapshot};
use dictation_coordinator::install_hotkey_handler;
use runtime_assets::runtime_directory;

type DesktopDictationSession =
    DictationSession<CpalAudioRecorder, WhisperCliEngine, DirectTextInserter>;

struct DesktopState {
    session: Mutex<DesktopDictationSession>,
    current_state: Arc<RwLock<DictationState>>,
    permissions: PermissionManager,
    input_level: InputLevel,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusSnapshot {
    state: DictationState,
    permissions: PermissionSnapshot,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_status_panel(app);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug)
                .level_for("enigo", log::LevelFilter::Info)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_input_level,
            commands::report_overlay_initialization,
            commands::request_permissions,
            commands::clear_error,
            commands::quit
        ])
        .setup(|app| {
            platform::configure_app(app)?;

            let app_handle = app.handle().clone();
            let observer_handle = app_handle.clone();
            let current_state = Arc::new(RwLock::new(DictationState::Idle));
            let observer_state = current_state.clone();
            let runtime_directory = runtime_directory(app.handle())?;
            let input_level = InputLevel::default();
            let session = DictationSession::new(
                CpalAudioRecorder::with_input_level(input_level.clone()),
                WhisperCliEngine::new(
                    runtime_directory.join("whisper-cli"),
                    runtime_directory.join("ggml-base.en.bin"),
                ),
                DirectTextInserter,
            )
            .with_state_observer(move |state| {
                if let Ok(mut current_state) = observer_state.write() {
                    *current_state = state.clone();
                }
                let _ = observer_handle.emit("app-state-changed", state);
                platform::update_dictation_overlay(&observer_handle, state);
            });
            let backend = Arc::new(DesktopState {
                session: Mutex::new(session),
                current_state,
                permissions: PermissionManager::new(),
                input_level,
            });
            app.manage(backend.clone());

            install_hotkey_handler(app_handle.clone(), backend.clone())?;
            tauri::async_runtime::spawn({
                let app_handle = app_handle.clone();
                let backend = backend.clone();
                async move {
                    let permissions = backend.permissions.request_missing().await;
                    let _ = app_handle.emit("permissions-changed", permissions);
                }
            });

            tray::install(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Floating Whisprer");
}

fn show_status_panel(app: &tauri::AppHandle) {
    if let Err(error) = platform::show_status_ui(app) {
        log::error!("unable to show status UI: {error}");
    }
}
