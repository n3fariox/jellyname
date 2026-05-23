use color_eyre::eyre::{Result, eyre};
use tmdb_api::client::reqwest::ReqwestExecutor;
use tmdb_api::client::Client as TmdbApiClient;
use tmdb_api::prelude::Command;

use super::models::*;

fn map_err(e: tmdb_api::error::Error) -> color_eyre::eyre::ErrReport {
    eyre!("TMDB API error: {e}")
}

pub struct TmdbClient {
    client: TmdbApiClient<ReqwestExecutor>,
}

impl TmdbClient {
    pub fn new(api_key: &str) -> Self {
        let client = TmdbApiClient::new(api_key.to_string());
        Self { client }
    }

    pub async fn search_movie(&self, query: &str) -> Result<Vec<MovieSearchResult>> {
        let cmd = tmdb_api::movie::search::MovieSearch::new(query.to_string());
        let results = cmd.execute(&self.client).await.map_err(map_err)?;
        Ok(results
            .results
            .iter()
            .map(|r| MovieSearchResult {
                title: r.inner.title.clone(),
                year: r
                    .inner
                    .release_date
                    .map(|d| d.format("%Y").to_string())
                    .unwrap_or_else(|| "...".to_string()),
                tmdb_id: r.inner.id,
                original_language: Some(r.inner.original_language.clone()),
            })
            .collect())
    }

    pub async fn search_tv(&self, query: &str) -> Result<Vec<TvSearchResult>> {
        let cmd = tmdb_api::tvshow::search::TVShowSearch::new(query.to_string());
        let results = cmd.execute(&self.client).await.map_err(map_err)?;
        let mut out = Vec::new();
        for r in &results.results {
            let info_cmd = tmdb_api::tvshow::details::TVShowDetails::new(r.inner.id);
            let info = info_cmd.execute(&self.client).await.map_err(map_err)?;
            let first_year = info
                .inner
                .first_air_date
                .map(|d| d.format("%Y").to_string())
                .unwrap_or_else(|| "...".to_string());
            let last_year = info
                .last_air_date
                .map(|d| d.format("%Y").to_string())
                .unwrap_or_else(|| "...".to_string());
            out.push(TvSearchResult {
                name: info.inner.name.clone(),
                first_year,
                last_year,
                tmdb_id: info.inner.id,
                seasons: info
                    .seasons
                    .iter()
                    .map(|s| TvSeasonResult {
                        name: s.inner.name.clone(),
                        season_number: s.inner.season_number as u32,
                        episode_count: s.episode_count as u32,
                    })
                    .collect(),
            });
        }
        Ok(out)
    }

    pub async fn season_episodes(
        &self,
        show_id: u64,
        season_num: u32,
    ) -> Result<Vec<EpisodeResult>> {
        let cmd =
            tmdb_api::tvshow::season::details::TVShowSeasonDetails::new(show_id, season_num as u64);
        let season = cmd.execute(&self.client).await.map_err(map_err)?;
        Ok(season
            .episodes
            .iter()
            .map(|ep| EpisodeResult {
                name: ep.inner.name.clone(),
                episode_number: ep.inner.episode_number as u32,
                air_date: ep
                    .inner
                    .air_date
                    .map(|d| d.format("%Y").to_string())
                    .unwrap_or_else(|| "N/A".to_string()),
            })
            .collect())
    }
}
