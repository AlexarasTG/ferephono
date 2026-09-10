#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::{
    HotkeyEvent, PermissionManager, PermissionSnapshot, configure_app, permission_guidance,
    show_status_ui, start_hotkey_listener, toggle_status_ui, update_dictation_overlay,
};
