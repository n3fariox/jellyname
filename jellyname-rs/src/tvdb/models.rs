use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct TvdbShowSummary {
    pub name: String,
    pub first_year: String,
    pub tvdb_id: u64,
    pub has_dvd_order: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct SeriesResponse {
    pub data: SeriesData,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct SeriesData {
    #[serde(rename = "seriesName")]
    pub series_name: Option<String>,
    #[serde(rename = "firstAired")]
    pub first_aired: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SummaryResponse {
    pub data: SummaryData,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct SummaryData {
    #[serde(rename = "dvdSeasons", default)]
    pub dvd_seasons: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct EpisodesResponse {
    #[serde(default)]
    pub data: Vec<RawEpisode>,
    pub links: Option<Links>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Links {
    pub next: Option<i64>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub(super) struct RawEpisode {
    #[serde(rename = "episodeName")]
    pub episode_name: Option<String>,
    #[serde(rename = "firstAired")]
    pub first_aired: Option<String>,
    #[serde(rename = "airedSeason")]
    pub aired_season: Option<i64>,
    #[serde(rename = "airedEpisodeNumber")]
    pub aired_episode_number: Option<i64>,
    #[serde(rename = "dvdEpisodeNumber")]
    pub dvd_episode_number: Option<f64>,
}
