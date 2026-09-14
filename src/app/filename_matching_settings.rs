use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Default, Deserialize, Serialize)]
pub(super) struct FilenameMatchingSettings {
    pub enabled: bool,
    pub folder: Option<PathBuf>,
}

impl FilenameMatchingSettings {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("Unable to read filename matching settings: {e}")),
        };
        let mut settings: Self = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Invalid filename matching settings: {e}"))?;
        if settings.folder.is_none() {
            settings.enabled = false;
        }
        Ok(settings)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let directory = path.parent().ok_or("Settings directory is missing")?;
        std::fs::create_dir_all(directory)
            .map_err(|e| format!("Unable to create settings directory: {e}"))?;
        let bytes =
            serde_json::to_vec(self).map_err(|e| format!("Unable to encode settings: {e}"))?;
        let temporary = path.with_extension("tmp");
        std::fs::write(&temporary, bytes)
            .map_err(|e| format!("Unable to save filename matching settings: {e}"))?;
        std::fs::rename(&temporary, path)
            .map_err(|e| format!("Unable to replace filename matching settings: {e}"))
    }
}
