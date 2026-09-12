use std::path::PathBuf;

use tauri::Manager;

pub(super) fn locate_assets(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let resource_directory = app
        .path()
        .resource_dir()
        .map_err(|error| format!("Unable to locate application resources: {error}"))?;
    let bundled_assets = resource_directory.join("runtime");

    if assets_are_complete(&bundled_assets) {
        return Ok(bundled_assets);
    }

    #[cfg(debug_assertions)]
    {
        let development_assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime");
        if assets_are_complete(&development_assets) {
            return Ok(development_assets);
        }

        Err(format!(
            "Whisper assets are missing. Checked {} and {}. Run `npm run prepare:runtime`.",
            bundled_assets.display(),
            development_assets.display()
        ))
    }

    #[cfg(not(debug_assertions))]
    Err(format!(
        "Bundled Whisper assets are missing or incomplete at {}. Rebuild the app after running `npm run prepare:runtime`.",
        bundled_assets.display()
    ))
}

fn assets_are_complete(directory: &std::path::Path) -> bool {
    directory.join("whisper-cli").is_file() && directory.join("ggml-base.en.bin").is_file()
}

#[cfg(test)]
mod tests {
    use super::assets_are_complete;

    #[test]
    fn requires_executable_and_model() {
        let directory = std::env::temp_dir().join(format!(
            "floating-whisprer-assets-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();

        assert!(!assets_are_complete(&directory));
        std::fs::write(directory.join("whisper-cli"), []).unwrap();
        assert!(!assets_are_complete(&directory));
        std::fs::write(directory.join("ggml-base.en.bin"), []).unwrap();
        assert!(assets_are_complete(&directory));

        std::fs::remove_dir_all(directory).unwrap();
    }
}
