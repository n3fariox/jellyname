use std::collections::VecDeque;
use std::path::PathBuf;

use crate::tmdb::models::MovieSearchResult;
use crate::tmdb::models::TvSearchResult;
use crate::tmdb::models::TvSeasonResult;
use crate::tmdb::models::EpisodeResult;

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Movies,
    Shows,
}

#[derive(Debug, Clone)]
pub enum FileState {
    Pending { path: PathBuf },
    ReadMkv { path: PathBuf, title: Option<String> },
    SearchInput {
        path: PathBuf,
        mkv_title: Option<String>,
        query: String,
    },
    Searching {
        path: PathBuf,
        mkv_title: Option<String>,
        query: String,
    },
    SelectMovie {
        path: PathBuf,
        results: Vec<MovieSearchResult>,
    },
    SelectShow {
        path: PathBuf,
        results: Vec<TvSearchResult>,
    },
    SelectSeason {
        path: PathBuf,
        show: super::ShowCache,
        seasons: Vec<TvSeasonResult>,
    },
    SelectEpisode {
        path: PathBuf,
        show: super::ShowCache,
        season: TvSeasonResult,
        episodes: Vec<EpisodeResult>,
    },
    TagInput {
        path: PathBuf,
        movie: crate::models::movie::Movie,
        dst: PathBuf,
        default_tag: String,
        tag: String,
    },
    ConfirmMovie {
        path: PathBuf,
        movie: crate::models::movie::Movie,
        src: PathBuf,
        dst: PathBuf,
        exists: bool,
    },
    ConfirmEpisode {
        path: PathBuf,
        show: super::ShowCache,
        episode_num: u32,
        src: PathBuf,
        dst: PathBuf,
        exists: bool,
    },
    Failed { path: PathBuf, error: String },
    Approved { path: PathBuf },
    Skipped { path: PathBuf },
    Deleted { path: PathBuf },
}

#[derive(Debug, Clone)]
pub struct ShowCache {
    pub name: String,
    pub first_year: String,
    pub tmdb_id: u64,
}

pub struct TuiApp {
    pub mode: Mode,
    pub files: Vec<FileState>,
    pub current: usize,
    pub should_quit: bool,
    pub approve_all: bool,
    pub dry_run: bool,
    pub mixed: bool,
    pub show_cache: Option<ShowCache>,
    pub season_cache: Option<TvSeasonResult>,
    pub episode_num: u32,
    pub status_message: Option<String>,
    pub log: VecDeque<String>,
}

impl TuiApp {
    pub fn new(mode: Mode, files: Vec<PathBuf>, dry_run: bool, mixed: bool) -> Self {
        Self {
            mode,
            files: files
                .into_iter()
                .map(|p| FileState::Pending { path: p })
                .collect(),
            current: 0,
            should_quit: false,
            approve_all: false,
            dry_run,
            mixed,
            show_cache: None,
            season_cache: None,
            episode_num: 0,
            status_message: None,
            log: VecDeque::new(),
        }
    }

    pub fn current_file(&self) -> Option<&FileState> {
        self.files.get(self.current)
    }

    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some(msg);
    }

    pub fn push_log(&mut self, msg: String) {
        self.log.push_back(msg);
        if self.log.len() > 500 {
            self.log.pop_front();
        }
    }
}
