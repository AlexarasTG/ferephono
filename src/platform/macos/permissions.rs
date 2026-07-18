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
    pub accessibility_granted: bool,
}

impl PermissionSnapshot {
    pub fn ready_for_end_to_end_flow(self) -> bool {
        self.microphone_granted && self.accessibility_granted
    }
}

#[derive(Clone, Copy)]
pub struct MacPermissionManager;

impl MacPermissionManager {
    pub fn snapshot(&self) -> PermissionSnapshot {
        PermissionSnapshot {
            microphone_granted: AVCaptureDevice::authorization_status_for_media_type(unsafe {
                AVMediaTypeAudio
            }) == AVAuthorizationStatusAuthorized,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_permissions_are_required_for_the_full_flow() {
        assert!(
            PermissionSnapshot {
                microphone_granted: true,
                accessibility_granted: true,
            }
            .ready_for_end_to_end_flow()
        );
        assert!(
            !PermissionSnapshot {
                microphone_granted: true,
                accessibility_granted: false,
            }
            .ready_for_end_to_end_flow()
        );
    }
}
