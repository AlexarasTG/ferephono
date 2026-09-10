use std::process::Command;
use std::sync::{Arc, Mutex};

use av_foundation::capture_device::{
    AVAuthorizationStatusAuthorized, AVAuthorizationStatusNotDetermined, AVCaptureDevice,
};
use av_foundation::media_format::AVMediaTypeAudio;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionSnapshot {
    pub microphone_granted: bool,
    pub input_monitoring_granted: bool,
    pub accessibility_granted: bool,
}

impl PermissionSnapshot {
    pub fn allows_dictation(self) -> bool {
        self.microphone_granted && self.input_monitoring_granted && self.accessibility_granted
    }
}

#[derive(Clone, Copy)]
struct MacPermissionManager;

impl MacPermissionManager {
    pub fn snapshot(&self) -> PermissionSnapshot {
        PermissionSnapshot {
            microphone_granted: AVCaptureDevice::authorization_status_for_media_type(unsafe {
                AVMediaTypeAudio
            }) == AVAuthorizationStatusAuthorized,
            input_monitoring_granted: unsafe { preflight_listen_event_access() },
            accessibility_granted:
                macos_accessibility_client::accessibility::application_is_trusted(),
        }
    }

    pub async fn request_microphone_access(&self) -> bool {
        let status =
            AVCaptureDevice::authorization_status_for_media_type(unsafe { AVMediaTypeAudio });
        if status == AVAuthorizationStatusAuthorized {
            return true;
        }
        if status != AVAuthorizationStatusNotDetermined {
            return false;
        }

        let (sender, receiver) = tokio::sync::oneshot::channel();
        let sender = Arc::new(Mutex::new(Some(sender)));
        AVCaptureDevice::request_access_for_media_type(
            unsafe { AVMediaTypeAudio },
            move |granted| {
                if let Some(sender) = sender.lock().ok().and_then(|mut sender| sender.take()) {
                    let _ = sender.send(granted.as_bool());
                }
            },
        );
        receiver.await.unwrap_or(false)
    }

    pub fn prompt_for_accessibility_access(&self) {
        macos_accessibility_client::accessibility::application_is_trusted_with_prompt();
    }

    pub fn request_input_monitoring_access(&self) {
        unsafe {
            request_listen_event_access();
        }
    }

    pub fn open_microphone_settings_if_prompt_unavailable(&self) {
        let status =
            AVCaptureDevice::authorization_status_for_media_type(unsafe { AVMediaTypeAudio });
        if status == AVAuthorizationStatusAuthorized || status == AVAuthorizationStatusNotDetermined
        {
            return;
        }

        if let Err(error) = Command::new("/usr/bin/open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
            .status()
        {
            log::error!("unable to open Microphone privacy settings: {error}");
        }
    }
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    #[link_name = "CGPreflightListenEventAccess"]
    fn preflight_listen_event_access() -> bool;
    #[link_name = "CGRequestListenEventAccess"]
    fn request_listen_event_access() -> bool;
}

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
    fn all_permissions_are_required_for_the_full_flow() {
        assert!(
            PermissionSnapshot {
                microphone_granted: true,
                input_monitoring_granted: true,
                accessibility_granted: true,
            }
            .allows_dictation()
        );

        for permissions in [
            PermissionSnapshot {
                microphone_granted: false,
                input_monitoring_granted: true,
                accessibility_granted: true,
            },
            PermissionSnapshot {
                microphone_granted: true,
                input_monitoring_granted: false,
                accessibility_granted: true,
            },
            PermissionSnapshot {
                microphone_granted: true,
                input_monitoring_granted: true,
                accessibility_granted: false,
            },
        ] {
            assert!(!permissions.allows_dictation());
        }
    }
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
