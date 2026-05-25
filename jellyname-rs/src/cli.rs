use clap::{Parser, Subcommand};
use std::path::PathBuf;

const DEFAULT_MOVIE_FORMAT: &str =
    "{title} ({year}) [tmdbid-{tmdb_id}]/{title} ({year}) [tmdbid-{tmdb_id}]{tag}.{ext}";

const DEFAULT_TV_FORMAT: &str = "{name} ({first_year}) [tmdbid-{tmdb_id}]/Season {season_num:02}/{name} S{season_num:02}E{episode_num:02}.{ext}";

#[derive(Debug, Parser)]
#[command(
    name = "jellyname-rs",
    about = "Interactive tool to rename MKV files for Jellyfin"
)]
pub struct Cli {
    #[arg(short = 'd', long)]
    pub dry_run: bool,

    #[arg(short = 'l', long = "filter-lang")]
    pub filter_lang: Option<String>,

    #[arg(short = 'k', long = "api-key", env = "TMDB_API_KEY")]
    pub api_key: Option<String>,

    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Movies {
        #[arg(long, default_value = DEFAULT_MOVIE_FORMAT)]
        format: String,

        #[arg(short = 'o', long = "output", required = true)]
        output: PathBuf,

        #[arg(required = true)]
        files: Vec<String>,
    },
    Shows {
        #[arg(long, default_value = DEFAULT_TV_FORMAT)]
        format: String,

        #[arg(short = 'o', long = "output", required = true)]
        output: PathBuf,

        #[arg(long)]
        mixed: bool,

        #[arg(long = "same-show")]
        same_show: bool,

        #[arg(required = true)]
        directories: Vec<String>,
    },
    Conv {
        /// Directory containing MKV files to convert
        folder: PathBuf,

        /// Path to profiles.yml (uses built-in defaults if not found)
        #[arg(long, default_value = "profiles.yml")]
        profiles: PathBuf,
    },
}
