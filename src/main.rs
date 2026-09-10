use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock, mpsc};

use floating_whisprer::{
    AppState, Application, CpalAudioRecorder, DirectTextInserter, WhisperCliEngine,
};
use serde::Serialize;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, State};
use tokio::sync::Mutex;

mod platform;

use platform::{HotkeyEvent, PermissionManager, PermissionSnapshot};

type ConcreteApplication = Application<CpalAudioRecorder, WhisperCliEngine, DirectTextInserter>;

struct Backend {
    application: Mutex<ConcreteApplication>,
    current_state: Arc<RwLock<AppState>>,
    permissions: PermissionManager,
    input_level: Arc<AtomicU32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusSnapshot {
    state: AppState,
    permissions: PermissionSnapshot,
}

#[tauri::command]
fn report_overlay_initialization(error: Option<String>) {
    match error {
        Some(error) => log::error!("dictation overlay initialization failed: {error}"),
        None => log::info!("dictation overlay initialized; state listener ready"),
    }
}

#[tauri::command]
fn get_input_level(backend: State<'_, Arc<Backend>>) -> f32 {
    f32::from_bits(backend.input_level.load(Ordering::Relaxed))
}

#[tauri::command]
fn get_status(backend: State<'_, Arc<Backend>>) -> Result<StatusSnapshot, String> {
    Ok(StatusSnapshot {
        state: backend
            .current_state
            .read()
            .map_err(|_| "Unable to read the current application state.".to_owned())?
            .clone(),
        permissions: backend.permissions.snapshot(),
    })
}

#[tauri::command]
async fn request_permissions(
    app: tauri::AppHandle,
    backend: State<'_, Arc<Backend>>,
) -> Result<PermissionSnapshot, String> {
    let permissions = backend.permissions.request_missing().await;
    backend
        .permissions
        .open_microphone_settings_if_prompt_unavailable();
    let _ = app.emit("permissions-changed", permissions);
    Ok(permissions)
}

#[tauri::command]
fn refresh_permissions(
    app: tauri::AppHandle,
    backend: State<'_, Arc<Backend>>,
) -> PermissionSnapshot {
    let permissions = backend.permissions.snapshot();
    let _ = app.emit("permissions-changed", permissions);
    permissions
}

#[tauri::command]
async fn clear_error(backend: State<'_, Arc<Backend>>) -> Result<(), String> {
    backend.application.lock().await.clear_error();
    Ok(())
}

#[tauri::command]
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main_window(app);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug)
                .level_for("enigo", log::LevelFilter::Info)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_input_level,
            report_overlay_initialization,
            request_permissions,
            refresh_permissions,
            clear_error,
            quit
        ])
        .setup(|app| {
            platform::configure_app(app)?;

            let app_handle = app.handle().clone();
            let observer_handle = app_handle.clone();
            let current_state = Arc::new(RwLock::new(AppState::Idle));
            let observer_state = current_state.clone();
            let runtime_directory = runtime_directory(app.handle())?;
            let input_level = Arc::new(AtomicU32::new(0));
            let application = Application::new(
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
            let backend = Arc::new(Backend {
                application: Mutex::new(application),
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

            let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;

            TrayIconBuilder::with_id("main")
                .icon(tray_icon)
                .icon_as_template(true)
                .tooltip("Floating Whisprer")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        let backend = app.state::<Arc<Backend>>();
                        let permissions = backend.permissions.snapshot();
                        let _ = app.emit("permissions-changed", permissions);

                        if let Err(error) = platform::toggle_status_ui(app, tray) {
                            log::error!("unable to toggle status UI: {error}");
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Floating Whisprer");
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Err(error) = platform::show_status_ui(app) {
        log::error!("unable to show status UI: {error}");
    }
}

fn install_hotkey_handler(
    app_handle: tauri::AppHandle,
    backend: Arc<Backend>,
) -> Result<(), String> {
    let (sender, receiver) = mpsc::channel();
    platform::start_hotkey_listener(sender)?;
    std::thread::Builder::new()
        .name("floating-whisprer-coordinator".to_owned())
        .spawn(move || {
            while let Ok(event) = receiver.recv() {
                let app_handle = app_handle.clone();
                let backend = backend.clone();
                tauri::async_runtime::spawn(async move {
                    handle_hotkey_event(&app_handle, &backend, event).await;
                });
            }
        })
        .map(|_| ())
        .map_err(|error| format!("Unable to start hotkey coordinator: {error}"))
}

async fn handle_hotkey_event(
    app_handle: &tauri::AppHandle,
    backend: &Arc<Backend>,
    event: HotkeyEvent,
) {
    match event {
        HotkeyEvent::Pressed => {
            let permissions = backend.permissions.snapshot();
            if !permissions.ready_for_end_to_end_flow() {
                let permissions = backend.permissions.request_missing().await;
                let _ = app_handle.emit("permissions-changed", permissions);
                if !permissions.ready_for_end_to_end_flow() {
                    backend
                        .application
                        .lock()
                        .await
                        .report_error(platform::permission_guidance(permissions));
                    return;
                }
            }
            backend.application.lock().await.handle_hotkey_pressed();
        }
        HotkeyEvent::Released => {
            backend
                .application
                .lock()
                .await
                .handle_hotkey_released()
                .await;
        }
    }
}

fn runtime_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let resource_directory = app
        .path()
        .resource_dir()
        .map_err(|error| format!("Unable to locate application resources: {error}"))?;
    let bundled_runtime = resource_directory.join("runtime");

    if runtime_is_complete(&bundled_runtime) {
        return Ok(bundled_runtime);
    }

    #[cfg(debug_assertions)]
    {
        let development_runtime = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime");
        if runtime_is_complete(&development_runtime) {
            return Ok(development_runtime);
        }

        Err(format!(
            "Whisper runtime is missing. Checked {} and {}. Run `npm run prepare:runtime`.",
            bundled_runtime.display(),
            development_runtime.display()
        ))
    }

    #[cfg(not(debug_assertions))]
    Err(format!(
        "Bundled Whisper runtime is missing or incomplete at {}. Rebuild the app after running `npm run prepare:runtime`.",
        bundled_runtime.display()
    ))
}

fn runtime_is_complete(directory: &std::path::Path) -> bool {
    directory.join("whisper-cli").is_file() && directory.join("ggml-base.en.bin").is_file()
}

#[cfg(test)]
mod tests {
    use super::runtime_is_complete;

    #[test]
    fn runtime_requires_both_assets() {
        let directory = std::env::temp_dir().join(format!(
            "floating-whisprer-runtime-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();

        assert!(!runtime_is_complete(&directory));
        std::fs::write(directory.join("whisper-cli"), []).unwrap();
        assert!(!runtime_is_complete(&directory));
        std::fs::write(directory.join("ggml-base.en.bin"), []).unwrap();
        assert!(runtime_is_complete(&directory));

        std::fs::remove_dir_all(directory).unwrap();
    }
}
