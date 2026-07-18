mod application;
mod audio;
mod hotkey;
mod insertion;
#[cfg(target_os = "macos")]
mod permissions;
mod post_processing;
mod transcription;

pub use application::{AppState, Application, AudioRecorder, TextInserter, TranscriptionEngine};
pub use audio::CpalAudioRecorder;
pub use hotkey::{HotkeyEvent, RightOptionHotkeyManager};
pub use insertion::DirectTextInserter;
#[cfg(target_os = "macos")]
pub use permissions::{MacPermissionManager, PermissionSnapshot};
pub use post_processing::TranscriptPostProcessor;
pub use transcription::WhisperCliEngine;
