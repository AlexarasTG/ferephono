#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::{
    HotkeyEvent, PermissionManager, PermissionSnapshot, configure_app, install_status_ui,
    permission_guidance, show_status_ui, start_hotkey_listener, update_dictation_overlay,
};
