use std::sync::{Arc, mpsc};

use tauri::Emitter;

use super::DesktopState;
use crate::platform::{self, HotkeyEvent};

pub(super) fn install_hotkey_handler(
    app_handle: tauri::AppHandle,
    backend: Arc<DesktopState>,
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
    backend: &Arc<DesktopState>,
    event: HotkeyEvent,
) {
    match event {
        HotkeyEvent::Pressed => {
            let permissions = backend.permissions.snapshot();
            if !permissions.allows_dictation() {
                let permissions = backend.permissions.request_missing().await;
                let _ = app_handle.emit("permissions-changed", permissions);
                if !permissions.allows_dictation() {
                    backend
                        .session
                        .lock()
                        .await
                        .report_error(platform::permission_guidance(permissions));
                    return;
                }
            }
            backend.session.lock().await.start_recording();
        }
        HotkeyEvent::Released => {
            backend.session.lock().await.finish_recording().await;
        }
    }
}
