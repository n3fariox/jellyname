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
