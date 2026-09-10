use std::path::PathBuf;

use tauri::Manager;

pub(super) fn runtime_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let resource_directory = app
        .path()
        .resource_dir()
        .map_err(|error| format!("Unable to locate application resources: {error}"))?;
    let bundled_runtime = resource_directory.join("runtime");

    if runtime_is_complete(&bundled_runtime) {
        return Ok(bundled_runtime);
    }

    #[cfg(debug_assertions)]
    {
        let development_runtime = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime");
        if runtime_is_complete(&development_runtime) {
            return Ok(development_runtime);
        }

        Err(format!(
            "Whisper runtime is missing. Checked {} and {}. Run `npm run prepare:runtime`.",
            bundled_runtime.display(),
            development_runtime.display()
        ))
    }

    #[cfg(not(debug_assertions))]
    Err(format!(
        "Bundled Whisper runtime is missing or incomplete at {}. Rebuild the app after running `npm run prepare:runtime`.",
        bundled_runtime.display()
    ))
}

fn runtime_is_complete(directory: &std::path::Path) -> bool {
    directory.join("whisper-cli").is_file() && directory.join("ggml-base.en.bin").is_file()
}

#[cfg(test)]
mod tests {
    use super::runtime_is_complete;

    #[test]
    fn runtime_requires_both_assets() {
        let directory = std::env::temp_dir().join(format!(
            "floating-whisprer-runtime-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();

        assert!(!runtime_is_complete(&directory));
        std::fs::write(directory.join("whisper-cli"), []).unwrap();
        assert!(!runtime_is_complete(&directory));
        std::fs::write(directory.join("ggml-base.en.bin"), []).unwrap();
        assert!(runtime_is_complete(&directory));

        std::fs::remove_dir_all(directory).unwrap();
    }
}
