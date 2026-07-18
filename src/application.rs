use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use log::{debug, error, info};
use serde::Serialize;

use crate::TranscriptPostProcessor;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum AppState {
    Idle,
    Recording,
    Transcribing,
    Inserting,
    Error(String),
}

#[async_trait]
pub trait AudioRecorder: Send {
    fn start(&mut self) -> Result<(), String>;
    async fn stop(&mut self) -> Result<PathBuf, String>;
}

#[async_trait]
pub trait TranscriptionEngine: Send + Sync {
    async fn transcribe(&self, audio_file: &Path) -> Result<String, String>;
}

#[async_trait]
pub trait TextInserter: Send + Sync {
    async fn insert(&self, text: &str) -> Result<(), String>;
}

pub struct Application<R, T, I> {
    state: AppState,
    recorder: R,
    transcription_engine: T,
    text_inserter: I,
    state_observer: Arc<dyn Fn(&AppState) + Send + Sync>,
    diagnostic_transcripts_enabled: bool,
}

impl<R, T, I> Application<R, T, I>
where
    R: AudioRecorder,
    T: TranscriptionEngine,
    I: TextInserter,
{
    pub fn new(recorder: R, transcription_engine: T, text_inserter: I) -> Self {
        Self {
            state: AppState::Idle,
            recorder,
            transcription_engine,
            text_inserter,
            state_observer: Arc::new(|_| {}),
            diagnostic_transcripts_enabled: std::env::var_os("FLOATING_WHISPRER_LOG_TRANSCRIPTS")
                .is_some(),
        }
    }

    pub fn with_state_observer(
        mut self,
        observer: impl Fn(&AppState) + Send + Sync + 'static,
    ) -> Self {
        self.state_observer = Arc::new(observer);
        self
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn clear_error(&mut self) {
        if matches!(self.state, AppState::Error(_)) {
            self.set_state(AppState::Idle);
        }
    }

    pub fn report_error(&mut self, message: impl Into<String>) {
        self.set_state(AppState::Error(message.into()));
    }

    pub fn handle_hotkey_pressed(&mut self) {
        if !matches!(self.state, AppState::Idle | AppState::Error(_)) {
            return;
        }

        match self.recorder.start() {
            Ok(()) => self.set_state(AppState::Recording),
            Err(message) => self.set_state(AppState::Error(message)),
        }
    }

    pub async fn handle_hotkey_released(&mut self) {
        if self.state != AppState::Recording {
            return;
        }

        self.set_state(AppState::Transcribing);
        let recording = match self.recorder.stop().await {
            Ok(path) => TemporaryRecording::new(path),
            Err(message) => {
                self.set_state(AppState::Error(message));
                return;
            }
        };

        let transcript = match self.transcription_engine.transcribe(recording.path()).await {
            Ok(transcript) => transcript,
            Err(message) => {
                self.set_state(AppState::Error(message));
                return;
            }
        };
        let transcript = transcript.trim();

        if self.diagnostic_transcripts_enabled {
            debug!("raw transcript: {transcript}");
        }

        if transcript.is_empty() {
            self.set_state(AppState::Error(
                "No speech was detected in the recording.".to_owned(),
            ));
            return;
        }

        let processed = TranscriptPostProcessor::process(transcript);
        if self.diagnostic_transcripts_enabled {
            debug!("processed transcript: {processed}");
        }
        self.set_state(AppState::Inserting);

        match self.text_inserter.insert(&processed).await {
            Ok(()) => self.set_state(AppState::Idle),
            Err(message) => self.set_state(AppState::Error(message)),
        }
    }

    fn set_state(&mut self, state: AppState) {
        match &state {
            AppState::Error(message) => error!("application state changed to error: {message}"),
            _ => info!("application state changed to {state:?}"),
        }
        self.state = state;
        (self.state_observer)(&self.state);
    }
}

struct TemporaryRecording {
    path: PathBuf,
}

impl TemporaryRecording {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryRecording {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
