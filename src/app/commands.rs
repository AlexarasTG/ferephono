use std::sync::Arc;

use tauri::{Emitter, State};

use super::{AppState, FilenameMatchingSnapshot, StatusSnapshot};
use crate::platform::PermissionSnapshot;
use floating_whisprer::post_processing::IndexStatus;

#[tauri::command]
pub(super) fn report_overlay_initialization(error: Option<String>) {
    match error {
        Some(error) => log::error!("dictation overlay initialization failed: {error}"),
        None => log::info!("dictation overlay initialized; state listener ready"),
    }
}

#[tauri::command]
pub(super) fn get_microphone_level(backend: State<'_, Arc<AppState>>) -> f32 {
    backend.microphone_level.rms()
}

#[tauri::command]
pub(super) fn get_status(backend: State<'_, Arc<AppState>>) -> Result<StatusSnapshot, String> {
    Ok(StatusSnapshot {
        state: backend
            .current_state
            .read()
            .map_err(|_| "Unable to read the current application state.".to_owned())?
            .clone(),
        permissions: backend.permissions.snapshot(),
        filename_matching: matching_snapshot(&backend)?,
    })
}

#[tauri::command]
pub(super) async fn request_permissions(
    app: tauri::AppHandle,
    backend: State<'_, Arc<AppState>>,
) -> Result<PermissionSnapshot, String> {
    let permissions = backend.permissions.request_missing().await;
    backend
        .permissions
        .open_microphone_settings_if_prompt_unavailable();
    let _ = app.emit("permissions-changed", permissions);
    Ok(permissions)
}

#[tauri::command]
pub(super) async fn clear_error(backend: State<'_, Arc<AppState>>) -> Result<(), String> {
    backend.session.lock().await.clear_error();
    Ok(())
}

#[tauri::command]
pub(super) fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

fn matching_snapshot(backend: &AppState) -> Result<FilenameMatchingSnapshot, String> {
    let settings = backend
        .filename_settings
        .lock()
        .map_err(|_| "Unable to read filename matching settings")?;
    let error = backend
        .filename_settings_error
        .lock()
        .map_err(|_| "Unable to read settings error")?;
    Ok(FilenameMatchingSnapshot {
        enabled: settings.enabled,
        folder: settings.folder.clone(),
        minimum_similarity_percent: settings.minimum_similarity_percent,
        default_minimum_similarity_percent:
            floating_whisprer::post_processing::DEFAULT_MINIMUM_SIMILARITY_TO_REPLACE,
        status: error
            .clone()
            .map_or_else(|| backend.filename_index.status(), IndexStatus::Error),
    })
}

fn update_matching_settings(
    backend: &AppState,
    update: impl FnOnce(
        &mut super::filename_matching_settings::FilenameMatchingSettings,
    ) -> Result<(), String>,
) -> Result<(), String> {
    let mut settings = backend
        .filename_settings
        .lock()
        .map_err(|_| "Unable to update filename matching settings")?;
    let mut updated = settings.clone();
    update(&mut updated)?;
    updated.save(&backend.filename_settings_path)?;
    if updated.enabled != settings.enabled || updated.folder != settings.folder {
        backend.filename_index.configure(if updated.enabled {
            updated.folder.clone()
        } else {
            None
        });
    }
    backend.minimum_similarity_percent.store(
        updated.minimum_similarity_percent,
        std::sync::atomic::Ordering::Relaxed,
    );
    *settings = updated;
    *backend
        .filename_settings_error
        .lock()
        .map_err(|_| "Unable to clear settings error")? = None;
    log::debug!(
        "filename matching settings updated: enabled={}",
        settings.enabled
    );
    Ok(())
}

#[tauri::command]
pub(super) async fn set_filename_matching_enabled(
    backend: State<'_, Arc<AppState>>,
    enabled: bool,
) -> Result<(), String> {
    let backend = backend.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        update_matching_settings(&backend, |settings| {
            if enabled && settings.folder.is_none() {
                return Err("Choose a folder before enabling filename matching".into());
            }
            settings.enabled = enabled;
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("Unable to update filename matching: {e}"))?
}

#[tauri::command]
pub(super) async fn choose_matching_folder(
    app: tauri::AppHandle,
    backend: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    crate::platform::choose_folder(&app, move |folder| {
        let _ = sender.send(folder);
    })?;
    let folder = receiver
        .await
        .map_err(|e| format!("Folder selection failed: {e}"))?;
    let Some(folder) = folder else {
        return Ok(());
    };
    let backend = backend.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        update_matching_settings(&backend, |settings| {
            settings.folder = Some(folder);
            Ok(())
        })
    })
    .await
    .map_err(|e| format!("Unable to update selected folder: {e}"))?
}

#[tauri::command]
pub(super) async fn set_filename_matching_similarity(
    backend: State<'_, Arc<AppState>>,
    minimum_similarity_percent: u8,
) -> Result<(), String> {
    let backend = backend.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        update_matching_settings(&backend, |settings| {
            settings.minimum_similarity_percent = minimum_similarity_percent;
            settings.validate()
        })
    })
    .await
    .map_err(|e| format!("Unable to save matching sensitivity: {e}"))?
}
