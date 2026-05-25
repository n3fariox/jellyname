use std::path::Path;

use color_eyre::eyre::{bail, eyre, Result};

use super::ffmpeg::{build_cmd, get_duration, run_with_progress};
use super::profiles::Profile;

pub async fn convert_file(input: &Path, profile: &Profile) -> Result<ConvertResult> {
    let bak = input.with_file_name(format!(
        "{}.bak",
        input.file_name().unwrap_or_default().to_string_lossy()
    ));

    if bak.exists() {
        bail!(
            "Backup already exists, refusing to overwrite: {:?}",
            bak
        );
    }

    tokio::fs::rename(input, &bak).await?;

    let result = convert_impl(input, &bak, profile).await;

    if let Err(e) = &result {
        // Remove partial/corrupt output first, then restore backup
        if input.exists() {
            let _ = tokio::fs::remove_file(input).await;
        }
        let _ = tokio::fs::rename(&bak, input).await;
        return Err(eyre!("Conversion failed: {e}"));
    }

    result
}

async fn convert_impl(input: &Path, bak: &Path, profile: &Profile) -> Result<ConvertResult> {
    let duration = get_duration(bak).await?;
    let cmd_args = build_cmd(profile, bak, input);

    let filename = input
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string();

    run_with_progress(&cmd_args[1..], duration, &filename).await?;

    let bak_size = tokio::fs::metadata(bak).await.map(|m| m.len()).unwrap_or(0);
    let out_size = tokio::fs::metadata(input)
        .await
        .map(|m| m.len())
        .unwrap_or(0);

    tokio::fs::remove_file(bak).await?;

    let saved = bak_size.saturating_sub(out_size);

    Ok(ConvertResult {
        filename,
        original_size: bak_size,
        new_size: out_size,
        saved,
    })
}

pub struct ConvertResult {
    pub filename: String,
    pub original_size: u64,
    pub new_size: u64,
    pub saved: u64,
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    for unit in UNITS {
        if size < 1024.0 {
            return format!("{:.1}{}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{:.1}{}", size, "PB")
}
