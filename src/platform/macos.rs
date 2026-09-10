mod dictation_overlay;
mod hotkey;
mod permissions;
mod popover;
pub use dictation_overlay::update as update_dictation_overlay;

use std::sync::Arc;
use std::sync::mpsc;

use hotkey::RightOptionListener;
use tauri::Manager;

pub use hotkey::HotkeyEvent;
pub use permissions::{PermissionManager, PermissionSnapshot, permission_guidance};

pub fn configure_app(app: &mut tauri::App) -> Result<(), String> {
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    app.manage(Arc::new(popover::PopoverPresenter::new()?));
    app.manage(
        dictation_overlay::DictationOverlay::new(app.handle())
            .map_err(|error| error.to_string())?,
    );
    Ok(())
}

pub fn start_hotkey_listener(sender: mpsc::Sender<HotkeyEvent>) -> Result<(), String> {
    RightOptionListener::start(sender)
}

pub fn toggle_status_ui(
    app: &tauri::AppHandle,
    tray: &tauri::tray::TrayIcon,
) -> Result<(), String> {
    popover::toggle(app, tray)
}

pub fn show_status_ui(app: &tauri::AppHandle) -> Result<(), String> {
    let tray = app
        .tray_by_id("main")
        .ok_or_else(|| "The main tray icon was not created.".to_owned())?;
    popover::show(app, &tray)
}
