use ferephono::post_processing::DEFAULT_MINIMUM_SIMILARITY_TO_REPLACE;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::settings_file;

const LABEL: &str = "filename matching settings";

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct FilenameMatchingSettings {
    pub enabled: bool,
    pub folder: Option<PathBuf>,
    #[serde(default = "default_minimum_similarity_percent")]
    pub minimum_similarity_percent: u8,
}

fn default_minimum_similarity_percent() -> u8 {
    DEFAULT_MINIMUM_SIMILARITY_TO_REPLACE
}

impl Default for FilenameMatchingSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            folder: None,
            minimum_similarity_percent: DEFAULT_MINIMUM_SIMILARITY_TO_REPLACE,
        }
    }
}

impl FilenameMatchingSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(50..=100).contains(&self.minimum_similarity_percent) {
            return Err("Minimum filename similarity must be between 50% and 100%.".into());
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let mut settings: Self = settings_file::load(path, LABEL)?;
        if settings.folder.is_none() {
            settings.enabled = false;
        }
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
    fn existing_settings_keep_the_default_threshold() {
        let settings: FilenameMatchingSettings =
            serde_json::from_str(r#"{"enabled":true,"folder":"/Users/alex/HomeDevelopment"}"#)
                .unwrap();
        assert_eq!(settings.minimum_similarity_percent, 80);
        assert!(settings.enabled);
    }

    #[test]
    fn custom_threshold_survives_serialization() {
        let settings = FilenameMatchingSettings {
            minimum_similarity_percent: 72,
            ..Default::default()
        };
        let bytes = serde_json::to_vec(&settings).unwrap();
        let restored: FilenameMatchingSettings = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.minimum_similarity_percent, 72);
        assert!(restored.validate().is_ok());
    }

    #[test]
    fn out_of_range_threshold_is_rejected() {
        for percent in [0, 49, 101, 255] {
            let settings = FilenameMatchingSettings {
                minimum_similarity_percent: percent,
                ..Default::default()
            };
            assert!(settings.validate().is_err());
        }
    }
}
