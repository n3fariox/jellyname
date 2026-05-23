use color_eyre::eyre::Result;

pub struct MkvMetadata {
    pub title: Option<String>,
}

pub fn read_mkv(path: &std::path::Path) -> Result<MkvMetadata> {
    let mkv = matroska::open(path).map_err(|e| {
        color_eyre::eyre::eyre!("Failed to open MKV {path:?}: {e}")
    })?;

    let title = mkv.info.title.clone();

    Ok(MkvMetadata { title })
}
