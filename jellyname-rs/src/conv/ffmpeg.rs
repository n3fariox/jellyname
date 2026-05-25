use std::path::Path;
use std::time::Duration;

use color_eyre::eyre::{bail, Result};
use indicatif::{ProgressBar, ProgressStyle};
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;

use super::profiles::Profile;

fn is_qsv(encoder: &str) -> bool {
    encoder.ends_with("_qsv")
}

fn video_codec_flags(profile: &Profile) -> Vec<String> {
    let v = match &profile.video_codec {
        Some(v) => v,
        None => return vec![],
    };
    let encoder = match v.as_str() {
        "h264" | "x264" => "libx264",
        "h265" | "x265" => "libx265",
        "nvenc" => "h264_nvenc",
        "qsv_h264" => "h264_qsv",
        "qsv_h265" => "hevc_qsv",
        other => other,
    };
    let mut flags = vec!["-c:v".into(), encoder.into()];

    if let Some(preset) = &profile.preset {
        flags.push("-preset".into());
        flags.push(preset.clone());
    }
    if is_qsv(encoder) {
        if let Some(q) = profile.crf {
            flags.push("-global_quality".into());
            flags.push(q.to_string());
        }
    } else if encoder != "copy" {
        if let Some(crf) = profile.crf {
            flags.push("-crf".into());
            flags.push(crf.to_string());
        }
    }
    if let Some(vb) = &profile.video_bitrate {
        flags.push("-b:v".into());
        flags.push(vb.clone());
    }
    flags
}

fn audio_codec_flags(profile: &Profile) -> Vec<String> {
    let a = match &profile.audio_codec {
        Some(a) => a,
        None => return vec![],
    };
    let encoder = match a.as_str() {
        "copy" => "copy",
        "aac" => "aac",
        "ac3" => "ac3",
        "mp3" => "libmp3lame",
        other => other,
    };
    let mut flags = vec!["-c:a".into(), encoder.into()];
    if let Some(ab) = &profile.audio_bitrate {
        flags.push("-b:a".into());
        flags.push(ab.clone());
    }
    flags
}

fn hwaccel_flags(profile: &Profile) -> Vec<String> {
    // Auto-detect QSV hwaccel if a QSV encoder is set but hwaccel is missing
    let is_qsv_codec = profile
        .video_codec
        .as_deref()
        .is_some_and(|c| c == "qsv_h265" || c == "qsv_h264");
    let effective = match &profile.hwaccel {
        Some(hw) => hw.clone(),
        None if is_qsv_codec => "qsv".into(),
        None => return vec![],
    };
    if effective == "qsv" {
        vec![
            "-hwaccel".into(),
            "qsv".into(),
            "-hwaccel_output_format".into(),
            "qsv".into(),
        ]
    } else {
        vec![
            "-hwaccel".into(),
            effective.clone(),
            "-hwaccel_output_format".into(),
            effective,
        ]
    }
}

fn extra_flags(profile: &Profile) -> Vec<String> {
    match &profile.extra_flags {
        Some(flags) => flags.clone(),
        None => vec![],
    }
}

pub fn build_cmd(profile: &Profile, input: &Path, output: &Path) -> Vec<String> {
    let mut cmd = vec!["ffmpeg".into()];
    cmd.extend(hwaccel_flags(profile));
    cmd.push("-i".into());
    cmd.push(input.to_string_lossy().to_string());
    cmd.push("-map_metadata".into());
    cmd.push("0".into());
    cmd.push("-movflags".into());
    cmd.push("faststart".into());
    cmd.extend(video_codec_flags(profile));
    cmd.extend(audio_codec_flags(profile));
    cmd.extend(extra_flags(profile));
    cmd.push(output.to_string_lossy().to_string());
    cmd
}

pub async fn get_duration(path: &Path) -> Result<Duration> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path.as_os_str())
        .output()
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        bail!("ffprobe returned no duration for {:?}", path);
    }
    let secs: f64 = trimmed.parse()?;
    Ok(Duration::from_secs_f64(secs))
}

pub async fn run_with_progress(
    ffmpeg_args: &[String],
    duration: Duration,
    filename: &str,
) -> Result<()> {
    let total_secs = duration.as_secs().max(1);

    let pb = ProgressBar::new(total_secs);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len}s {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(filename.to_string());

    // Build command with -progress BEFORE the output file (ffmpeg ignores flags after output)
    let mut cmd = Command::new("ffmpeg");
    if let Some(output_file) = ffmpeg_args.last() {
        cmd.args(&ffmpeg_args[..ffmpeg_args.len() - 1]);
        cmd.arg("-progress");
        cmd.arg("pipe:1");
        cmd.arg("-nostats");
        cmd.arg(output_file);
    } else {
        cmd.args(ffmpeg_args);
    }

    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // Drain stderr in background to prevent pipe deadlock
    let stderr = child.stderr.take().unwrap();
    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut buf = [0u8; 4096];
        let mut reader = tokio::io::BufReader::new(stderr);
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                _ => continue,
            }
        }
    });

    let stdout = child.stdout.take().unwrap();
    let reader = tokio::io::BufReader::new(stdout);
    let mut lines = reader.lines();
    let mut last_secs = 0u64;

    loop {
        tokio::select! {
            line = lines.next_line() => {
                match line? {
                    Some(l) => {
                        if let Some(out_time) = l.trim().strip_prefix("out_time=") {
                            if let Ok(secs) = parse_time(out_time) {
                                let current = secs.min(total_secs);
                                while last_secs < current {
                                    pb.inc(1);
                                    last_secs += 1;
                                }
                            }
                        }
                    }
                    None => break,
                }
            }
            _ = tokio::signal::ctrl_c() => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                pb.finish_and_clear();
                bail!("Interrupted by user");
            }
        }
    }

    let status = child.wait().await?;
    pb.finish_and_clear();

    if !status.success() {
        bail!("ffmpeg exited with code {:?}", status.code());
    }
    Ok(())
}

fn parse_time(s: &str) -> Result<u64> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 3 {
        let h: u64 = parts[0].parse()?;
        let m: u64 = parts[1].parse()?;
        let sec_f: f64 = parts[2].parse()?;
        Ok(h * 3600 + m * 60 + sec_f as u64)
    } else {
        let secs: f64 = s.parse()?;
        Ok(secs as u64)
    }
}
