use super::movie::ProcessedFile;

#[derive(Debug, Clone)]
pub struct TVSeason {
    pub name: String,
    pub season_number: u32,
    pub episode_count: u32,
    pub year: String,
    pub tmdb_id: u64,
}

#[derive(Debug, Clone)]
pub struct TVShow {
    pub name: String,
    pub seasons: Vec<TVSeason>,
    pub episodes: u32,
    pub tmdb_id: u64,
    pub first_year: String,
    pub last_year: String,
}

#[derive(Debug, Clone)]
pub struct TVEpisode {
    pub name: String,
    pub episode_number: u32,
    pub air_date: String,
    pub overview: String,
    pub tmdb_id: u64,
}

#[derive(Debug, Clone)]
pub struct ProcessedTvFile {
    pub inner: ProcessedFile,
    pub show: TVShow,
}
