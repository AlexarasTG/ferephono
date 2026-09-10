use std::sync::Arc;

use tauri::{Emitter, State};

use super::{DesktopState, StatusSnapshot};
use crate::platform::PermissionSnapshot;

#[tauri::command]
pub(super) fn report_overlay_initialization(error: Option<String>) {
    match error {
        Some(error) => log::error!("dictation overlay initialization failed: {error}"),
        None => log::info!("dictation overlay initialized; state listener ready"),
    }
}

#[tauri::command]
pub(super) fn get_input_level(backend: State<'_, Arc<DesktopState>>) -> f32 {
    backend.input_level.rms()
}

#[tauri::command]
pub(super) fn get_status(backend: State<'_, Arc<DesktopState>>) -> Result<StatusSnapshot, String> {
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
pub(super) async fn request_permissions(
    app: tauri::AppHandle,
    backend: State<'_, Arc<DesktopState>>,
) -> Result<PermissionSnapshot, String> {
    let permissions = backend.permissions.request_missing().await;
    backend
        .permissions
        .open_microphone_settings_if_prompt_unavailable();
    let _ = app.emit("permissions-changed", permissions);
    Ok(permissions)
}

#[tauri::command]
pub(super) async fn clear_error(backend: State<'_, Arc<DesktopState>>) -> Result<(), String> {
    backend.session.lock().await.clear_error();
    Ok(())
}

#[tauri::command]
pub(super) fn quit(app: tauri::AppHandle) {
    app.exit(0);
}
