use std::path::PathBuf;

pub enum Mode {
    Movies,
    Shows,
}

pub enum FileState {
    Pending { path: PathBuf },
    ReadMkv { path: PathBuf },
    Searching { path: PathBuf, query: String, loading: bool },
    Selecting,
    Tagging { default: String },
    Confirming { src: PathBuf, dst: PathBuf },
    Done { path: PathBuf },
    Failed { path: PathBuf, error: String },
}

pub struct TuiApp {
    pub mode: Mode,
    pub files: Vec<FileState>,
    pub current: usize,
    pub should_quit: bool,
}

impl TuiApp {
    pub fn new(mode: Mode, files: Vec<PathBuf>) -> Self {
        Self {
            mode,
            files: files.into_iter().map(|p| FileState::Pending { path: p }).collect(),
            current: 0,
            should_quit: false,
        }
    }
}
