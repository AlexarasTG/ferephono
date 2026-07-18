use std::path::PathBuf;
use std::sync::{Arc, mpsc};

use floating_whisprer::{
    AppState, Application, CpalAudioRecorder, DirectTextInserter, HotkeyEvent,
    MacPermissionManager, PermissionSnapshot, RightOptionHotkeyManager, WhisperCliEngine,
};
use serde::Serialize;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, PhysicalPosition, State, WindowEvent};
use tokio::sync::Mutex;

type ConcreteApplication = Application<CpalAudioRecorder, WhisperCliEngine, DirectTextInserter>;

struct Backend {
    application: Mutex<ConcreteApplication>,
    permissions: MacPermissionManager,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusSnapshot {
    state: AppState,
    permissions: PermissionSnapshot,
}

#[tauri::command]
async fn get_status(backend: State<'_, Arc<Backend>>) -> Result<StatusSnapshot, String> {
    Ok(StatusSnapshot {
        state: backend.application.lock().await.state().clone(),
        permissions: backend.permissions.snapshot(),
    })
}

#[tauri::command]
async fn request_permissions(
    app: tauri::AppHandle,
    backend: State<'_, Arc<Backend>>,
) -> Result<PermissionSnapshot, String> {
    let permissions = request_missing_permissions(&backend).await;
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
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_status,
            request_permissions,
            refresh_permissions,
            clear_error,
            quit
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let app_handle = app.handle().clone();
            let observer_handle = app_handle.clone();
            let runtime_directory = runtime_directory(app.handle());
            let application = Application::new(
                CpalAudioRecorder::new(),
                WhisperCliEngine::new(
                    runtime_directory.join("whisper-cli"),
                    runtime_directory.join("ggml-base.en.bin"),
                ),
                DirectTextInserter,
            )
            .with_state_observer(move |state| {
                let _ = observer_handle.emit("app-state-changed", state);
            });
            let backend = Arc::new(Backend {
                application: Mutex::new(application),
                permissions: MacPermissionManager,
            });
            app.manage(backend.clone());

            install_hotkey_handler(app_handle.clone(), backend.clone())?;
            tauri::async_runtime::spawn({
                let app_handle = app_handle.clone();
                let backend = backend.clone();
                async move {
                    let permissions = request_missing_permissions(&backend).await;
                    let _ = app_handle.emit("permissions-changed", permissions);
                }
            });

            let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;

            TrayIconBuilder::new()
                .icon(tray_icon)
                .icon_as_template(true)
                .tooltip("Floating Whisprer")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        rect,
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        let Some(window) = app.get_webview_window("main") else {
                            return;
                        };

                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            if let Ok(window_size) = window.outer_size() {
                                let scale_factor = window.scale_factor().unwrap_or(1.0);
                                let tray_position = rect.position.to_physical::<f64>(scale_factor);
                                let tray_size = rect.size.to_physical::<f64>(scale_factor);
                                let x = tray_position.x + tray_size.width / 2.0
                                    - f64::from(window_size.width) / 2.0;
                                let y = tray_position.y + tray_size.height;
                                let _ = window.set_position(PhysicalPosition::new(x, y));
                            }
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::Focused(false) => {
                let _ = window.hide();
            }
            WindowEvent::Focused(true) => {
                let app = window.app_handle();
                let backend = app.state::<Arc<Backend>>();
                let _ = app.emit("permissions-changed", backend.permissions.snapshot());
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("failed to run Floating Whisprer");
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn install_hotkey_handler(
    app_handle: tauri::AppHandle,
    backend: Arc<Backend>,
) -> Result<(), String> {
    let (sender, receiver) = mpsc::channel();
    RightOptionHotkeyManager::start(sender)?;
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
                let permissions = request_missing_permissions(backend).await;
                let _ = app_handle.emit("permissions-changed", permissions);
                if !permissions.ready_for_end_to_end_flow() {
                    backend
                        .application
                        .lock()
                        .await
                        .report_error(permission_guidance(permissions));
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

async fn request_missing_permissions(backend: &Backend) -> PermissionSnapshot {
    let mut permissions = backend.permissions.snapshot();
    if !permissions.microphone_granted {
        backend.permissions.request_microphone_access().await;
        permissions = backend.permissions.snapshot();
    }
    if permissions.microphone_granted && !permissions.accessibility_granted {
        backend.permissions.prompt_for_accessibility_access();
        permissions = backend.permissions.snapshot();
    }
    permissions
}

fn permission_guidance(permissions: PermissionSnapshot) -> &'static str {
    match (
        permissions.microphone_granted,
        permissions.accessibility_granted,
    ) {
        (false, false) => {
            "Grant microphone and accessibility permissions, then press Right Option again."
        }
        (false, true) => "Grant microphone permission, then press Right Option again.",
        (true, false) => "Grant accessibility permission, then press Right Option again.",
        (true, true) => "Permissions updated. Press Right Option again to start dictation.",
    }
}

fn runtime_directory(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .resource_dir()
        .ok()
        .map(|directory| directory.join("runtime"))
        .filter(|directory| directory.exists())
        .unwrap_or_else(|| PathBuf::from("runtime"))
}
