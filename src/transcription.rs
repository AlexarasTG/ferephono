use std::path::{Path, PathBuf};
use std::process::Stdio;

use async_trait::async_trait;
use log::{debug, info};
use tokio::process::Command;

use crate::TranscriptionEngine;

pub struct WhisperCliEngine {
    executable: PathBuf,
    model: PathBuf,
}

impl WhisperCliEngine {
    pub fn new(executable: impl Into<PathBuf>, model: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            model: model.into(),
        }
    }
}

#[async_trait]
impl TranscriptionEngine for WhisperCliEngine {
    async fn transcribe(&self, audio_file: &Path) -> Result<String, String> {
        let started_at = std::time::Instant::now();
        debug!("starting whisper-cli for {}", audio_file.display());
        let mut command = Command::new(&self.executable);
        command
            .args([
                "--file",
                path_as_str(audio_file)?,
                "--model",
                path_as_str(&self.model)?,
                "--language",
                "en",
                "--no-timestamps",
                "--no-prints",
                "--no-gpu",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let output = command
            .output()
            .await
            .map_err(|error| format!("Unable to start whisper-cli: {error}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        info!(
            "whisper-cli finished: status={}, duration_ms={}",
            output.status,
            started_at.elapsed().as_millis()
        );
        if !stderr.is_empty() {
            debug!("whisper-cli stderr: {stderr}");
        }

        if !output.status.success() {
            return Err(if stderr.is_empty() {
                format!("whisper-cli exited with status {}.", output.status)
            } else {
                stderr
            });
        }

        Ok(stdout)
    }
}

fn path_as_str(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("Path is not valid UTF-8: {}", path.display()))
}
