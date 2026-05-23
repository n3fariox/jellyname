use std::fmt;

#[derive(Debug, Clone)]
pub struct MovieSearchResult {
    pub title: String,
    pub year: String,
    pub tmdb_id: u64,
    pub original_language: Option<String>,
}

impl fmt::Display for MovieSearchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}) [tmdbid-{}]",
            self.title, self.year, self.tmdb_id
        )
    }
}

#[derive(Debug, Clone)]
pub struct TvSearchResult {
    pub name: String,
    pub first_year: String,
    pub last_year: String,
    pub tmdb_id: u64,
    pub seasons: Vec<TvSeasonResult>,
}

impl fmt::Display for TvSearchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}-{}) [tmdbid-{}]",
            self.name, self.first_year, self.last_year, self.tmdb_id
        )
    }
}

#[derive(Debug, Clone)]
pub struct TvSeasonResult {
    pub name: String,
    pub season_number: u32,
    pub episode_count: u32,
}

impl fmt::Display for TvSeasonResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} Season {} ({} episodes)",
            self.name, self.season_number, self.episode_count
        )
    }
}

#[derive(Debug, Clone)]
pub struct EpisodeResult {
    pub name: String,
    pub episode_number: u32,
    pub air_date: String,
}

impl fmt::Display for EpisodeResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "S{:02} - {} ({})",
            self.episode_number, self.name, self.air_date
        )
    }
}
