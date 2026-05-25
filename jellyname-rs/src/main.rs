mod cli;
mod conv;
mod mkv;
mod models;
mod processor;
mod tmdb;
mod tui;
mod utils;

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use cli::Cli;
use color_eyre::eyre::{eyre, Result};
use glob::glob;

use models::filters::Filters;
use tmdb::client::TmdbClient;
use tui::app::{Mode, TuiApp};
use tui::run_tui;

fn resolve_files(patterns: &[String]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for pattern in patterns {
        let p = if !pattern.contains('*') {
            if pattern.ends_with('/') {
                format!("{}*.mkv", pattern)
            } else {
                format!("{}/**/*.mkv", pattern)
            }
        } else {
            pattern.clone()
        };
        for entry in glob(&p).map_err(|e| eyre!("Invalid glob pattern: {e}"))? {
            match entry {
                Ok(path) => {
                    if path.extension().and_then(|e| e.to_str()) == Some("mkv") {
                        files.push(path);
                    }
                }
                Err(e) => tracing::warn!("Glob error: {e}"),
            }
        }
    }
    files.sort();
    Ok(files)
}

fn resolve_directories(dirs: &[String]) -> Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    for d in dirs {
        let p = PathBuf::from(d);
        if p.is_dir() {
            result.push(p);
        } else {
            tracing::warn!("Not a directory: {d}");
        }
    }
    result.sort();
    Ok(result)
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "jellyname_rs=info".into()),
        )
        .init();

    let cli = Cli::parse();
    tracing::info!(?cli, "jellyname-rs started");

    let filters = Filters {
        lang: cli.filter_lang,
    };

    match cli.cmd {
        cli::Command::Movies {
            format,
            output,
            files,
        } => {
            let api_key = cli
                .api_key
                .ok_or_else(|| color_eyre::eyre::eyre!("--api-key or TMDB_API_KEY is required for movies command"))?;
            let tmdb = Arc::new(TmdbClient::new(&api_key));
            let paths = resolve_files(&files)?;
            if paths.is_empty() {
                tracing::warn!("No MKV files found matching the given patterns");
                return Ok(());
            }
            tracing::info!("Found {} MKV files", paths.len());

            let app = TuiApp::new(Mode::Movies, paths, cli.dry_run, false);
            run_tui(app, tmdb, output, format, cli.dry_run, filters).await?;
        }
        cli::Command::Shows {
            format,
            output,
            directories,
            mixed,
            same_show: _,
        } => {
            let api_key = cli
                .api_key
                .ok_or_else(|| color_eyre::eyre::eyre!("--api-key or TMDB_API_KEY is required for shows command"))?;
            let tmdb = Arc::new(TmdbClient::new(&api_key));
            let dirs = resolve_directories(&directories)?;
            if dirs.is_empty() {
                tracing::warn!("No valid directories found");
                return Ok(());
            }

            // Collect all files from all directories
            let mut all_files = Vec::new();
            for dir in &dirs {
                let mut dir_files: Vec<PathBuf> = std::fs::read_dir(dir)
                    .map_err(|e| eyre!("Cannot read directory {dir:?}: {e}"))?
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("mkv"))
                    .collect();
                dir_files.sort();
                all_files.extend(dir_files);
            }

            if all_files.is_empty() {
                tracing::warn!("No MKV files found in any directory");
                return Ok(());
            }

            let app = TuiApp::new(Mode::Shows, all_files, cli.dry_run, mixed);
            run_tui(app, tmdb, output, format, cli.dry_run, filters).await?;
        }
        cli::Command::Conv { folder, profiles } => {
            let files: Vec<PathBuf> = std::fs::read_dir(&folder)
                .map_err(|e| eyre!("Cannot read directory {folder:?}: {e}"))?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("mkv"))
                .collect();

            if files.is_empty() {
                tracing::warn!("No MKV files found in {:?}", folder);
                return Ok(());
            }

            let mut profile_map = conv::profiles::load_profiles(&profiles);
            if profile_map.is_empty() {
                tracing::info!("No profiles.yml found, using built-in defaults");
                profile_map = conv::profiles::builtin_profiles();
            }

            let selection = conv::tui::run_conv_tui(files, profile_map).await?;
            let Some(sel) = selection else {
                println!("Conversion cancelled.");
                return Ok(());
            };

            if sel.files.is_empty() {
                println!("No files selected.");
                return Ok(());
            }

            // Merge the selected audio option into the video profile
            let mut profile = sel.profile.clone();
            profile.audio_codec = Some(sel.audio_option.codec.into());
            profile.audio_bitrate = sel.audio_option.bitrate.map(|s| s.into());

            let n = sel.files.len();
            let mut results = Vec::with_capacity(n);

            // Show sample ffmpeg command for first file
            if let Some(first) = sel.files.first() {
                let bak = first.with_file_name(format!(
                    "{}.bak",
                    first.file_name().unwrap_or_default().to_string_lossy()
                ));
                let sample = conv::ffmpeg::build_cmd(&profile, &bak, first);
                println!("Command: {}", sample.join(" "));
                println!();
            }

            for (i, file) in sel.files.iter().enumerate() {
                println!(
                    "[{}/{}] Converting {}...",
                    i + 1,
                    n,
                    file.file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("?")
                );

                match conv::convert::convert_file(file, &profile).await {
                    Ok(r) => {
                        println!(
                            "  ✓ {} — original: {}, new: {}, saved: {}",
                            r.filename,
                            conv::convert::human_size(r.original_size),
                            conv::convert::human_size(r.new_size),
                            conv::convert::human_size(r.saved),
                        );
                        results.push(r);
                    }
                    Err(e) => {
                        eprintln!(
                            "  ✗ {}: {e}",
                            file.file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("?")
                        );
                    }
                }
            }

            println!();
            let total_saved: u64 = results.iter().map(|r| r.saved).sum();
            let total_original: u64 = results.iter().map(|r| r.original_size).sum();
            let total_new: u64 = results.iter().map(|r| r.new_size).sum();
            println!(
                "Summary: {} files converted (video: {}, audio: {}), {} -> {}, saved {}",
                results.len(),
                sel.profile_name,
                sel.audio_option.label,
                conv::convert::human_size(total_original),
                conv::convert::human_size(total_new),
                conv::convert::human_size(total_saved),
            );
        }
    }

    Ok(())
}
