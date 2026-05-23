mod cli;
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
use color_eyre::eyre::{Result, eyre};
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

    let filters = Filters { lang: cli.filter_lang };
    let tmdb = Arc::new(TmdbClient::new(&cli.api_key));

    match cli.cmd {
        cli::Command::Movies { format, output, files } => {
            let paths = resolve_files(&files)?;
            if paths.is_empty() {
                tracing::warn!("No MKV files found matching the given patterns");
                return Ok(());
            }
            tracing::info!("Found {} MKV files", paths.len());

            let app = TuiApp::new(Mode::Movies, paths, cli.dry_run);
            run_tui(app, tmdb, output, format, cli.dry_run, filters).await?;
        }
        cli::Command::Shows { format, output, directories, mixed, same_show } => {
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
                    .filter(|p| {
                        p.extension().and_then(|e| e.to_str()) == Some("mkv")
                    })
                    .collect();
                dir_files.sort();
                all_files.extend(dir_files);
            }

            if all_files.is_empty() {
                tracing::warn!("No MKV files found in any directory");
                return Ok(());
            }

            let app = TuiApp::new(Mode::Shows, all_files, cli.dry_run);
            run_tui(app, tmdb, output, format, cli.dry_run, filters).await?;
        }
    }

    Ok(())
}
