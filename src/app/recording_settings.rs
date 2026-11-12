use serde::{Deserialize, Serialize};
use std::path::Path;

use super::settings_file;

const LABEL: &str = "recording settings";
const MIN_LIMIT_MINUTES: u8 = 1;
const MAX_LIMIT_MINUTES: u8 = 5;
const DEFAULT_LIMIT_MINUTES: u8 = 4;

#[derive(Deserialize, Serialize)]
pub(super) struct RecordingSettings {
    pub limit_minutes: u8,
}

impl Default for RecordingSettings {
    fn default() -> Self {
        Self {
            limit_minutes: DEFAULT_LIMIT_MINUTES,
        }
    }
}

impl RecordingSettings {
    fn validate(&self) -> Result<(), String> {
        if !(MIN_LIMIT_MINUTES..=MAX_LIMIT_MINUTES).contains(&self.limit_minutes) {
            return Err(format!(
                "Recording limit must be between {MIN_LIMIT_MINUTES} and {MAX_LIMIT_MINUTES} minutes."
            ));
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let settings: Self = settings_file::load(path, LABEL)?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        settings_file::save(self, path, LABEL)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_limit_is_between_one_and_five_minutes() {
        assert_eq!(RecordingSettings::default().limit_minutes, 4);
        for limit_minutes in [0, 6, 10, 255] {
            assert!(RecordingSettings { limit_minutes }.validate().is_err());
        }
        for limit_minutes in 1..=5 {
            assert!(RecordingSettings { limit_minutes }.validate().is_ok());
        }
    }

    #[test]
    fn recording_settings_persist_independently_of_filename_settings() {
        let directory = std::env::temp_dir().join(format!(
            "ferephono-recording-settings-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let recording = directory.join("recording.json");
        let filenames = directory.join("filename-matching.json");
        let original = br#"{"enabled":false,"folder":null}"#;
        std::fs::write(&filenames, original).unwrap();
        assert_eq!(
            RecordingSettings::load(&recording).unwrap().limit_minutes,
            4
        );
        RecordingSettings { limit_minutes: 5 }
            .save(&recording)
            .unwrap();
        assert_eq!(
            RecordingSettings::load(&recording).unwrap().limit_minutes,
            5
        );
        assert_eq!(std::fs::read(&filenames).unwrap(), original);
        assert!(
            RecordingSettings { limit_minutes: 6 }
                .save(&recording)
                .is_err()
        );
        assert_eq!(
            RecordingSettings::load(&recording).unwrap().limit_minutes,
            5
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}
