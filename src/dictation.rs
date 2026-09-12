use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use log::{debug, error, info};
use serde::Serialize;

use crate::file_name_formatter;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum DictationState {
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

pub struct DictationSession<R, T, I> {
    state: DictationState,
    recorder: R,
    transcription_engine: T,
    text_inserter: I,
    state_observer: Arc<dyn Fn(&DictationState) + Send + Sync>,
    diagnostic_transcripts_enabled: bool,
}

impl<R, T, I> DictationSession<R, T, I>
where
    R: AudioRecorder,
    T: TranscriptionEngine,
    I: TextInserter,
{
    pub fn new(recorder: R, transcription_engine: T, text_inserter: I) -> Self {
        Self {
            state: DictationState::Idle,
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
        observer: impl Fn(&DictationState) + Send + Sync + 'static,
    ) -> Self {
        self.state_observer = Arc::new(observer);
        self
    }

    pub fn state(&self) -> &DictationState {
        &self.state
    }

    pub fn clear_error(&mut self) {
        if matches!(self.state, DictationState::Error(_)) {
            self.set_state(DictationState::Idle);
        }
    }

    pub fn report_error(&mut self, message: impl Into<String>) {
        self.set_state(DictationState::Error(message.into()));
    }

    pub fn start_recording(&mut self) {
        if !matches!(self.state, DictationState::Idle | DictationState::Error(_)) {
            return;
        }

        match self.recorder.start() {
            Ok(()) => self.set_state(DictationState::Recording),
            Err(message) => self.set_state(DictationState::Error(message)),
        }
    }

    pub async fn finish_recording(&mut self) {
        if self.state != DictationState::Recording {
            return;
        }

        self.set_state(DictationState::Transcribing);
        let recording = match self.recorder.stop().await {
            Ok(path) => TemporaryRecording::new(path),
            Err(message) => {
                self.set_state(DictationState::Error(message));
                return;
            }
        };

        let transcript = match self.transcription_engine.transcribe(recording.path()).await {
            Ok(transcript) => transcript,
            Err(message) => {
                self.set_state(DictationState::Error(message));
                return;
            }
        };
        let transcript = transcript.trim();

        if self.diagnostic_transcripts_enabled {
            debug!("raw transcript: {transcript}");
        }

        if transcript.is_empty() {
            self.set_state(DictationState::Error(
                "No speech was detected in the recording.".to_owned(),
            ));
            return;
        }

        let processed = file_name_formatter::format_spoken_references(transcript);
        if self.diagnostic_transcripts_enabled {
            debug!("processed transcript: {processed}");
        }
        self.set_state(DictationState::Inserting);

        match self.text_inserter.insert(&processed).await {
            Ok(()) => self.set_state(DictationState::Idle),
            Err(message) => self.set_state(DictationState::Error(message)),
        }
    }

    fn set_state(&mut self, state: DictationState) {
        match &state {
            DictationState::Error(message) => {
                error!("application state changed to error: {message}")
            }
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
