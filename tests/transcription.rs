#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use floating_whisprer::{TranscriptionEngine, WhisperCliEngine};

#[tokio::test]
async fn invokes_whisper_with_the_parity_arguments() {
    let script = executable_script("arguments", "#!/bin/sh\nprintf '%s\\n' \"$@\"\n");
    let model = PathBuf::from("/tmp/floating-whisprer-model.bin");
    let audio = PathBuf::from("/tmp/floating-whisprer-audio.wav");
    let engine = WhisperCliEngine::new(&script, &model);

    let output = engine.transcribe(&audio).await.unwrap();

    assert_eq!(
        output.lines().collect::<Vec<_>>(),
        vec![
            "--file",
            audio.to_str().unwrap(),
            "--model",
            model.to_str().unwrap(),
            "--language",
            "en",
            "--no-timestamps",
            "--no-prints",
            "--no-gpu",
        ]
    );
    fs::remove_file(script).unwrap();
}

#[tokio::test]
async fn returns_stderr_when_whisper_fails() {
    let script = executable_script("failure", "#!/bin/sh\nprintf 'model failed' >&2\nexit 7\n");
    let engine = WhisperCliEngine::new(&script, "/tmp/model.bin");

    let error = engine
        .transcribe(PathBuf::from("/tmp/audio.wav").as_path())
        .await
        .unwrap_err();

    assert_eq!(error, "model failed");
    fs::remove_file(script).unwrap();
}

fn executable_script(name: &str, contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "floating-whisprer-{name}-{}.sh",
        std::process::id()
    ));
    fs::write(&path, contents).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&path, permissions).unwrap();
    path
}
