use color_eyre::eyre::Result;

pub struct MkvMetadata {
    pub title: Option<String>,
    pub resolution: Option<(u64, u64)>,
}

pub fn read_mkv(path: &std::path::Path) -> Result<MkvMetadata> {
    let mkv = matroska::open(path).map_err(|e| {
        color_eyre::eyre::eyre!("Failed to open MKV {path:?}: {e}")
    })?;

    let title = mkv.info.title.clone();

    let resolution = mkv
        .video_tracks()
        .next()
        .and_then(|t| match &t.settings {
            matroska::Settings::Video(v) => Some((v.pixel_width, v.pixel_height)),
            _ => None,
        });

    Ok(MkvMetadata { title, resolution })
}
