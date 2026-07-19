mod hotkey;
mod permissions;
mod popover;

use std::sync::mpsc;

use hotkey::RightOptionHotkeyManager;
use permissions::MacPermissionManager;

pub use hotkey::HotkeyEvent;
pub use permissions::PermissionSnapshot;

#[derive(Clone, Copy)]
pub struct PermissionManager {
    inner: MacPermissionManager,
}

impl PermissionManager {
    pub fn new() -> Self {
        Self {
            inner: MacPermissionManager,
        }
    }

    pub fn snapshot(&self) -> PermissionSnapshot {
        self.inner.snapshot()
    }

    pub async fn request_missing(&self) -> PermissionSnapshot {
        let mut permissions = self.snapshot();
        if !permissions.microphone_granted {
            self.inner.request_microphone_access().await;
            permissions = self.snapshot();
        }
        if permissions.microphone_granted && !permissions.input_monitoring_granted {
            self.inner.request_input_monitoring_access();
            permissions = self.snapshot();
        }
        if permissions.microphone_granted
            && permissions.input_monitoring_granted
            && !permissions.accessibility_granted
        {
            self.inner.prompt_for_accessibility_access();
            permissions = self.snapshot();
        }
        permissions
    }

    pub fn open_microphone_settings_if_prompt_unavailable(&self) {
        self.inner.open_microphone_settings_if_prompt_unavailable();
    }
}

pub fn configure_app(app: &mut tauri::App) {
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

pub fn start_hotkey_listener(sender: mpsc::Sender<HotkeyEvent>) -> Result<(), String> {
    RightOptionHotkeyManager::start(sender)
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

pub fn permission_guidance(permissions: PermissionSnapshot) -> String {
    let mut missing = Vec::new();
    if !permissions.microphone_granted {
        missing.push("Microphone");
    }
    if !permissions.input_monitoring_granted {
        missing.push("Input Monitoring");
    }
    if !permissions.accessibility_granted {
        missing.push("Accessibility");
    }

    format!(
        "Grant access to: {}. Then press Right Option again.",
        missing.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_guidance_names_all_missing_permissions() {
        let permissions = PermissionSnapshot {
            microphone_granted: false,
            input_monitoring_granted: false,
            accessibility_granted: false,
        };

        assert_eq!(
            permission_guidance(permissions),
            "Grant access to: Microphone, Input Monitoring, Accessibility. Then press Right Option again."
        );
    }
}
