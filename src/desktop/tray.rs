use std::sync::Arc;

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

use super::DesktopState;
use crate::platform;

pub(super) fn install(app: &tauri::App) -> tauri::Result<()> {
    let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../../icons/tray.png"))?;

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
                let backend = app.state::<Arc<DesktopState>>();
                let permissions = backend.permissions.snapshot();
                let _ = app.emit("permissions-changed", permissions);

                if let Err(error) = platform::toggle_status_ui(app, tray) {
                    log::error!("unable to toggle status UI: {error}");
                }
            }
        })
        .build(app)?;
    Ok(())
}
