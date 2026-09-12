#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::{
    HotkeyEvent, PermissionManager, PermissionSnapshot, configure_app, install_status_menu,
    permission_guidance, show_status_menu, start_hotkey_listener, update_dictation_overlay,
};
