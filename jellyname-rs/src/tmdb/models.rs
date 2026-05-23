#[derive(Debug, Clone)]
pub struct MovieSearchResult {
    pub title: String,
    pub year: String,
    pub tmdb_id: u64,
    pub original_language: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TvSearchResult {
    pub name: String,
    pub first_year: String,
    pub last_year: String,
    pub tmdb_id: u64,
    pub seasons: Vec<TvSeasonResult>,
    pub episodes: u32,
}

#[derive(Debug, Clone)]
pub struct TvSeasonResult {
    pub name: String,
    pub season_number: u32,
    pub episode_count: u32,
    pub year: String,
    pub tmdb_id: u64,
}

#[derive(Debug, Clone)]
pub struct EpisodeResult {
    pub name: String,
    pub episode_number: u32,
    pub air_date: String,
    pub overview: String,
    pub tmdb_id: u64,
}
