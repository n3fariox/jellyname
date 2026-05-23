use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Movie {
    pub title: String,
    pub year: String,
    pub tmdb_id: u64,
}

#[derive(Debug, Clone)]
pub struct ProcessedFile {
    pub src: PathBuf,
    pub dst: PathBuf,
    pub approved: bool,
}

#[derive(Debug, Clone)]
pub struct ProcessedMovieFile {
    pub inner: ProcessedFile,
    pub movie: Movie,
}
