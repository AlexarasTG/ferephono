mod dictation_overlay;
mod hotkey;
mod permissions;
mod status_menu;
pub use dictation_overlay::update as update_dictation_overlay;

use std::sync::mpsc;

use hotkey::RightOptionListener;
use tauri::Manager;

pub use hotkey::HotkeyEvent;
pub use permissions::{PermissionManager, PermissionSnapshot, permission_guidance};

pub fn configure_app(app: &mut tauri::App) -> Result<(), String> {
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    app.manage(
        dictation_overlay::DictationOverlay::new(app.handle())
            .map_err(|error| error.to_string())?,
    );
    Ok(())
}

pub fn start_hotkey_listener(sender: mpsc::Sender<HotkeyEvent>) -> Result<(), String> {
    RightOptionListener::start(sender)
}

pub fn install_status_menu(
    app: &tauri::AppHandle,
    tray: &tauri::tray::TrayIcon,
) -> Result<(), String> {
    status_menu::install(app, tray)
}

pub fn show_status_menu(app: &tauri::AppHandle) -> Result<(), String> {
    let tray = app
        .tray_by_id("main")
        .ok_or("The main tray icon was not created.")?;
    status_menu::show(&tray)
}
