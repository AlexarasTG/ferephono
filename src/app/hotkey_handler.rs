use std::sync::{Arc, mpsc};
use std::time::Duration;

use ferephono::DictationState;
use tauri::Emitter;

use super::AppState;
use crate::platform::{self, HotkeyEvent};

pub(super) fn install_hotkey_handler(
    app_handle: tauri::AppHandle,
    backend: Arc<AppState>,
) -> Result<(), String> {
    let (sender, receiver) = mpsc::channel();
    platform::start_hotkey_listener(sender)?;
    std::thread::Builder::new()
        .name("ferephono-hotkey-handler".to_owned())
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
        .map_err(|error| format!("Unable to start hotkey handler: {error}"))
}

async fn handle_hotkey_event(
    app_handle: &tauri::AppHandle,
    backend: &Arc<AppState>,
    event: HotkeyEvent,
) {
    if event != HotkeyEvent::Pressed {
        return;
    }
    // Drop presses while the session is busy (transcribing, permission prompt, timeout stop):
    // queueing them could start an unintended recording.
    let Ok(mut session) = backend.session.try_lock() else {
        return;
    };
    match session.state() {
        DictationState::Recording => {
            // The timeout task cannot be mid-stop here: it would be holding the session lock.
            if let Some(timeout) = backend
                .recording_timeout
                .lock()
                .ok()
                .and_then(|mut t| t.take())
            {
                timeout.abort();
            }
            session.finish_recording().await;
        }
        DictationState::Idle | DictationState::Error(_) => {
            let permissions = backend.permissions.snapshot();
            if !permissions.allows_dictation() {
                let permissions = backend.permissions.request_missing().await;
                let _ = app_handle.emit("permissions-changed", permissions);
                if !permissions.allows_dictation() {
                    session.report_error(platform::permission_guidance(permissions));
                    return;
                }
            }
            let minutes = match backend.recording_settings.lock() {
                Ok(settings) => settings.limit_minutes,
                Err(_) => {
                    session.report_error("Unable to read recording limit");
                    return;
                }
            };
            session.start_recording(Duration::from_secs(u64::from(minutes) * 60));
            if let Some(deadline) = session.recording_deadline() {
                let task_backend = backend.clone();
                let timeout = tauri::async_runtime::spawn(async move {
                    tokio::time::sleep_until(deadline).await;
                    let mut session = task_backend.session.lock().await;
                    // Deadline guard: a stale timeout must never stop a newer recording.
                    session.finish_expired_recording(deadline).await;
                });
                if let Ok(mut slot) = backend.recording_timeout.lock() {
                    *slot = Some(timeout);
                }
            }
        }
        _ => {}
    }
}
