# Jellyname-rs Conversion Plan

## Overview

Rewrite the Python `jellyname` utility in Rust with a Ratatui TUI. The tool reads MakeMKV ripped `.mkv` files, queries TMDB for metadata, prompts the user interactively to confirm matches, then renames/moves files into a Jellyfin-compatible directory structure.

## Dependencies

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
ratatui = "0.29"
crossterm = "0.28"
tmdb-api = "0.9.1"
matroska = "0.30.1"
tokio = { version = "1", features = ["full"] }
strfmt = "0.2"
glob = "0.3"
chrono = "0.4"
tracing = "0.1"
tracing-subscriber = "0.3"
color-eyre = "0.6"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

No external binaries. MKVToolNix/mkvmerge replaced by pure-Rust `matroska` crate.

## Project Structure

```
jellyname-rs/
├── Cargo.toml
├── PLAN.md
└── src/
    ├── main.rs               # tokio::main, install error hooks, dispatch to TUI
    ├── cli.rs                # clap derive: two subcommands (movies/shows)
    │
    ├── tui/                  # Ratatui TUI app
    │   ├── mod.rs            # TuiApp struct: event loop, state machine tick()
    │   ├── app.rs            # AppState enum (Startup, MovieSingle, ShowDir...)
    │   ├── ui.rs             # fn render(): split layout, delegate to state views
    │   └── widgets.rs        # ModalPopup, FileList, SearchInput, ConfirmDialog
    │
    ├── tmdb/                 # TMDB abstraction layer
    │   ├── mod.rs            # re-exports
    │   ├── client.rs         # Wraps tmdb-api crate: search_movie, search_tv,
    │   │                     #   tv_info, season_episodes
    │   └── models.rs         # Our Movie, TVShow, TVSeason, TVEpisode domain types
    │
    ├── mkv/
    │   └── reader.rs         # matroska::open -> { title: Option<String>,
    │                         #   resolution: Option<(u64,u64)> }
    │
    ├── models/
    │   ├── movie.rs          # Movie, ProcessedMovieFile
    │   ├── show.rs           # TVShow, TVSeason, TVEpisode, ProcessedTvFile
    │   └── filters.rs        # Filters { lang: Option<String> }
    │
    ├── processor/            # Core logic — no I/O, no TUI, just data transformations
    │   ├── movies.rs         # find_match(), process_movie_file()
    │   └── shows.rs          # identify_show(), identify_season(), process_tv_dir()
    │
    └── utils/
        ├── file_ops.rs       # rename_file(), guess_title(), rmdir
        ├── format.rs         # apply_format(template, ctx) -> PathBuf using strfmt
        └── title.rs          # fix_title() normalization
```

## TUI State Machine

```
AppState::Processing {
    files: Vec<FileState>,
    current: usize,
    mode: Mode,  // Movies | Shows
}

enum FileState {
    Pending,
    ReadMkv,                     // spawn_blocking to read matroska
    Searching { query: String, loading: bool },
    Selecting { results: Vec<SearchResult> },
    Tagging { default: String },
    Confirming { src: PathBuf, dst: PathBuf },
    Done,
    Failed { error: String },
}
```

## Implementation Order

1. **Scaffold**: Cargo.toml, cli.rs, main.rs (just parse + print)
2. **Models**: All data types (Movie, TVShow, TVSeason, TVEpisode, Filters, etc.)
3. **MKV reader**: matroska crate, extract title + resolution
4. **TMDB client**: Wrap tmdb-api crate, translate to our models
5. **Utils**: fix_title, format string, file ops
6. **Processor**: movies.rs + shows.rs logic (I/O-free)
7. **TUI**: State machine, layout, widgets
8. **main.rs final**: Glue CLI args -> TUI -> processor -> file ops
