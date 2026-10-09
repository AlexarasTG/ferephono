mod commands;
mod filename_matching_settings;
mod hotkey_handler;
mod status_menu_setup;
mod whisper_assets;

use std::path::PathBuf;
use std::sync::atomic::AtomicU8;
use std::sync::{Arc, RwLock};

use ferephono::post_processing::{FileNameIndex, IndexStatus, PostProcessor};

use ferephono::{
    CpalAudioRecorder, DictationSession, DictationState, DirectTextInserter, MicrophoneLevel,
    WhisperCliEngine,
};
use serde::Serialize;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

use crate::platform::{self, PermissionManager, PermissionSnapshot};
use filename_matching_settings::FilenameMatchingSettings;
use hotkey_handler::install_hotkey_handler;

type AppDictationSession =
    DictationSession<CpalAudioRecorder, WhisperCliEngine, DirectTextInserter>;

struct AppState {
    session: Mutex<AppDictationSession>,
    current_state: Arc<RwLock<DictationState>>,
    permissions: PermissionManager,
    microphone_level: MicrophoneLevel,
    filename_index: Arc<FileNameIndex>,
    minimum_similarity_percent: Arc<AtomicU8>,
    filename_settings: std::sync::Mutex<FilenameMatchingSettings>,
    filename_settings_path: PathBuf,
    filename_settings_error: std::sync::Mutex<Option<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusSnapshot {
    state: DictationState,
    permissions: PermissionSnapshot,
    filename_matching: FilenameMatchingSnapshot,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FilenameMatchingSnapshot {
    enabled: bool,
    folder: Option<PathBuf>,
    status: IndexStatus,
    minimum_similarity_percent: u8,
    default_minimum_similarity_percent: u8,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_status_menu(app);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug)
                .level_for("enigo", log::LevelFilter::Info)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::choose_matching_folder,
            commands::set_filename_matching_enabled,
            commands::set_filename_matching_similarity,
            commands::get_microphone_level,
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
            let whisper_assets = whisper_assets::locate_assets(app.handle())?;
            let filename_settings_path =
                app.path().app_config_dir()?.join("filename-matching.json");
            let (filename_settings, filename_settings_error) =
                match FilenameMatchingSettings::load(&filename_settings_path) {
                    Ok(settings) => (settings, None),
                    Err(error) => {
                        log::error!("{error}");
                        (FilenameMatchingSettings::default(), Some(error))
                    }
                };
            let minimum_similarity_percent =
                Arc::new(AtomicU8::new(filename_settings.minimum_similarity_percent));
            let filename_index = Arc::new(FileNameIndex::default());
            if filename_settings.enabled {
                filename_index.configure(filename_settings.folder.clone());
            }
            let microphone_level = MicrophoneLevel::default();
            let session = DictationSession::new(
                CpalAudioRecorder::with_microphone_level(microphone_level.clone()),
                WhisperCliEngine::new(
                    whisper_assets.join("whisper-cli"),
                    whisper_assets.join("ggml-base.en.bin"),
                ),
                DirectTextInserter,
            )
            .with_post_processor(PostProcessor::new(
                filename_index.clone(),
                minimum_similarity_percent.clone(),
            ))
            .with_state_observer(move |state| {
                if let Ok(mut current_state) = observer_state.write() {
                    *current_state = state.clone();
                }
                let _ = observer_handle.emit("app-state-changed", state);
                platform::update_dictation_overlay(&observer_handle, state);
            });
            let backend = Arc::new(AppState {
                session: Mutex::new(session),
                current_state,
                permissions: PermissionManager::new(),
                microphone_level,
                filename_index,
                minimum_similarity_percent,
                filename_settings: std::sync::Mutex::new(filename_settings),
                filename_settings_path,
                filename_settings_error: std::sync::Mutex::new(filename_settings_error),
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

            status_menu_setup::install(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Ferephono");
}

fn show_status_menu(app: &tauri::AppHandle) {
    if let Err(error) = platform::show_status_menu(app) {
        log::error!("unable to show status menu: {error}");
    }
}
