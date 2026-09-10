use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use floating_whisprer::{
    AudioRecorder, DictationSession, DictationState, TextInserter, TranscriptionEngine,
};

struct FakeRecorder {
    start_count: Arc<Mutex<usize>>,
    recording_path: PathBuf,
    start_error: Option<String>,
    stop_error: Option<String>,
}

#[async_trait]
impl AudioRecorder for FakeRecorder {
    fn start(&mut self) -> Result<(), String> {
        *self.start_count.lock().unwrap() += 1;
        self.start_error.clone().map_or(Ok(()), Err)
    }

    async fn stop(&mut self) -> Result<PathBuf, String> {
        self.stop_error
            .clone()
            .map_or_else(|| Ok(self.recording_path.clone()), Err)
    }
}

struct FakeTranscriptionEngine {
    result: Result<String, String>,
}

#[async_trait]
impl TranscriptionEngine for FakeTranscriptionEngine {
    async fn transcribe(&self, _audio_file: &Path) -> Result<String, String> {
        self.result.clone()
    }
}

struct FakeTextInserter {
    inserted: Arc<Mutex<Vec<String>>>,
    error: Option<String>,
}

#[async_trait]
impl TextInserter for FakeTextInserter {
    async fn insert(&self, text: &str) -> Result<(), String> {
        self.inserted.lock().unwrap().push(text.to_owned());
        self.error.clone().map_or(Ok(()), Err)
    }
}

#[tokio::test]
async fn completes_the_full_dictation_flow_and_deletes_the_recording() {
    let recording_path = temporary_recording("happy-path");
    fs::write(&recording_path, b"audio").unwrap();
    let start_count = Arc::new(Mutex::new(0));
    let inserted = Arc::new(Mutex::new(Vec::new()));
    let mut session = session(
        recording_path.clone(),
        start_count.clone(),
        Ok("Please update the developer dictionary.swift file".to_owned()),
        inserted.clone(),
        None,
    );

    session.start_recording();
    assert_eq!(session.state(), &DictationState::Recording);
    session.finish_recording().await;

    assert_eq!(session.state(), &DictationState::Idle);
    assert_eq!(*start_count.lock().unwrap(), 1);
    assert_eq!(
        *inserted.lock().unwrap(),
        vec!["Please update the @DeveloperDictionary.swift file"]
    );
    assert!(!recording_path.exists());
}

#[tokio::test]
async fn ignores_duplicate_presses_and_release_while_idle() {
    let start_count = Arc::new(Mutex::new(0));
    let mut session = session(
        temporary_recording("ignored-events"),
        start_count.clone(),
        Ok("hello".to_owned()),
        Arc::new(Mutex::new(Vec::new())),
        None,
    );

    session.finish_recording().await;
    session.start_recording();
    session.start_recording();

    assert_eq!(*start_count.lock().unwrap(), 1);
    assert_eq!(session.state(), &DictationState::Recording);
}

#[tokio::test]
async fn enters_error_state_when_recording_cannot_start_and_can_clear_it() {
    let recording_path = temporary_recording("start-error");
    let recorder = FakeRecorder {
        start_count: Arc::new(Mutex::new(0)),
        recording_path,
        start_error: Some("microphone unavailable".to_owned()),
        stop_error: None,
    };
    let mut session = DictationSession::new(
        recorder,
        FakeTranscriptionEngine {
            result: Ok(String::new()),
        },
        FakeTextInserter {
            inserted: Arc::new(Mutex::new(Vec::new())),
            error: None,
        },
    );

    session.start_recording();
    assert_eq!(
        session.state(),
        &DictationState::Error("microphone unavailable".to_owned())
    );

    session.clear_error();
    assert_eq!(session.state(), &DictationState::Idle);
}

#[tokio::test]
async fn deletes_recording_and_recovers_to_error_after_transcription_failure() {
    let recording_path = temporary_recording("transcription-error");
    fs::write(&recording_path, b"audio").unwrap();
    let mut session = session(
        recording_path.clone(),
        Arc::new(Mutex::new(0)),
        Err("whisper failed".to_owned()),
        Arc::new(Mutex::new(Vec::new())),
        None,
    );

    session.start_recording();
    session.finish_recording().await;

    assert_eq!(
        session.state(),
        &DictationState::Error("whisper failed".to_owned())
    );
    assert!(!recording_path.exists());
}

#[tokio::test]
async fn treats_an_empty_transcript_as_an_error() {
    let recording_path = temporary_recording("empty-transcript");
    fs::write(&recording_path, b"audio").unwrap();
    let mut session = session(
        recording_path.clone(),
        Arc::new(Mutex::new(0)),
        Ok("   \n".to_owned()),
        Arc::new(Mutex::new(Vec::new())),
        None,
    );

    session.start_recording();
    session.finish_recording().await;

    assert_eq!(
        session.state(),
        &DictationState::Error("No speech was detected in the recording.".to_owned())
    );
    assert!(!recording_path.exists());
}

#[tokio::test]
async fn deletes_recording_and_recovers_to_error_after_insertion_failure() {
    let recording_path = temporary_recording("insertion-error");
    fs::write(&recording_path, b"audio").unwrap();
    let mut session = session(
        recording_path.clone(),
        Arc::new(Mutex::new(0)),
        Ok("hello".to_owned()),
        Arc::new(Mutex::new(Vec::new())),
        Some("paste failed".to_owned()),
    );

    session.start_recording();
    session.finish_recording().await;

    assert_eq!(
        session.state(),
        &DictationState::Error("paste failed".to_owned())
    );
    assert!(!recording_path.exists());
}

fn session(
    recording_path: PathBuf,
    start_count: Arc<Mutex<usize>>,
    transcription_result: Result<String, String>,
    inserted: Arc<Mutex<Vec<String>>>,
    insertion_error: Option<String>,
) -> DictationSession<FakeRecorder, FakeTranscriptionEngine, FakeTextInserter> {
    DictationSession::new(
        FakeRecorder {
            start_count,
            recording_path,
            start_error: None,
            stop_error: None,
        },
        FakeTranscriptionEngine {
            result: transcription_result,
        },
        FakeTextInserter {
            inserted,
            error: insertion_error,
        },
    )
}

fn temporary_recording(test_name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "floating-whisprer-{test_name}-{}.wav",
        std::process::id()
    ))
}
