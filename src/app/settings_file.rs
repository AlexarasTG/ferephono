use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::Path;

pub(super) fn load<T: DeserializeOwned + Default>(path: &Path, label: &str) -> Result<T, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
        Err(e) => return Err(format!("Unable to read {label}: {e}")),
    };
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid {label}: {e}"))
}

pub(super) fn save<T: Serialize>(value: &T, path: &Path, label: &str) -> Result<(), String> {
    let directory = path.parent().ok_or("Settings directory is missing")?;
    std::fs::create_dir_all(directory)
        .map_err(|e| format!("Unable to create settings directory: {e}"))?;
    let bytes = serde_json::to_vec(value).map_err(|e| format!("Unable to encode {label}: {e}"))?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes).map_err(|e| format!("Unable to save {label}: {e}"))?;
    std::fs::rename(&temporary, path).map_err(|e| format!("Unable to replace {label}: {e}"))
}
