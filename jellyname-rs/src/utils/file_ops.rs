use std::path::Path;

use color_eyre::eyre::Result;

pub fn rename_file(src: &Path, dst: &Path, dry_run: bool) -> Result<()> {
    if !dry_run {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(src, dst)?;
    }
    Ok(())
}

pub fn supported_extensions() -> &'static [&'static str] {
    &["mkv"]
}

pub fn has_supported_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| supported_extensions().contains(&e))
}
