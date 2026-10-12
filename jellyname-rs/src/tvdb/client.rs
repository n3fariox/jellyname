use color_eyre::eyre::{eyre, Result};

use crate::tmdb::models::{EpisodeResult, TvSeasonResult};

use super::models::*;

const BASE: &str = "https://api.thetvdb.com";

pub struct TvdbClient {
    http: reqwest::Client,
}

impl TvdbClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    pub async fn fetch_show_summary(&self, tvdb_id: u64) -> Result<TvdbShowSummary> {
        let series_url = format!("{BASE}/series/{tvdb_id}");
        let resp = self
            .http
            .get(&series_url)
            .send()
            .await
            .map_err(|e| eyre!("TVDB request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(eyre!(
                "Failed to fetch TVDB show {tvdb_id}: {}",
                resp.status()
            ));
        }
        let series: SeriesResponse = resp
            .json()
            .await
            .map_err(|e| eyre!("TVDB response parse failed: {e}"))?;

        let name = series
            .data
            .series_name
            .unwrap_or_else(|| "Unknown".to_string());
        let first_year = series
            .data
            .first_aired
            .as_deref()
            .and_then(|d| d.split('-').next())
            .filter(|s| !s.is_empty())
            .unwrap_or("...")
            .to_string();

        // Check if the show has a DVD episode order, in addition to aired order.
        let summary_url = format!("{BASE}/series/{tvdb_id}/episodes/summary");
        let has_dvd_order = match self.http.get(&summary_url).send().await {
            Ok(r) if r.status().is_success() => r
                .json::<SummaryResponse>()
                .await
                .map(|s| !s.data.dvd_seasons.is_empty())
                .unwrap_or(false),
            _ => false,
        };

        Ok(TvdbShowSummary {
            name,
            first_year,
            tvdb_id,
            has_dvd_order,
        })
    }

    /// Fetch every episode for the show, following pagination (the TVDB v1
    /// API returns 100 episodes per page via `links.next`).
    async fn fetch_all_episodes(&self, tvdb_id: u64, dvd_order: bool) -> Result<Vec<RawEpisode>> {
        let url = if dvd_order {
            format!("{BASE}/series/{tvdb_id}/episodes/query")
        } else {
            format!("{BASE}/series/{tvdb_id}/episodes")
        };

        let mut all = Vec::new();
        let mut page: i64 = 1;
        loop {
            let mut req = self.http.get(&url).query(&[("page", page)]);
            if dvd_order {
                req = req.query(&[("dvdSeason", 1)]);
            }
            let resp = req
                .send()
                .await
                .map_err(|e| eyre!("TVDB episodes request failed: {e}"))?;
            if !resp.status().is_success() {
                break;
            }
            let body: EpisodesResponse = resp
                .json()
                .await
                .map_err(|e| eyre!("TVDB episodes parse failed: {e}"))?;
            all.extend(body.data);
            match body.links.and_then(|l| l.next) {
                Some(next) => page = next,
                None => break,
            }
        }
        Ok(all)
    }

    /// Fetch all episodes and group them into seasons, ready to drop into
    /// the existing SelectSeason/SelectEpisode TUI flow.
    pub async fn fetch_seasons(
        &self,
        tvdb_id: u64,
        dvd_order: bool,
    ) -> Result<Vec<TvSeasonResult>> {
        let episodes = self.fetch_all_episodes(tvdb_id, dvd_order).await?;

        if dvd_order {
            let mut eps = episodes;
            eps.sort_by(|a, b| {
                a.dvd_episode_number
                    .unwrap_or(0.0)
                    .total_cmp(&b.dvd_episode_number.unwrap_or(0.0))
            });
            let tv_episodes: Vec<EpisodeResult> = eps
                .iter()
                .map(|ep| EpisodeResult {
                    name: ep.episode_name.clone().unwrap_or_default(),
                    episode_number: ep.dvd_episode_number.unwrap_or(0.0).round() as u32,
                    air_date: ep.first_aired.clone().unwrap_or_default(),
                })
                .collect();

            return Ok(vec![TvSeasonResult {
                name: "Season 1 (DVD Order)".to_string(),
                season_number: 1,
                episode_count: tv_episodes.len() as u32,
                episodes: tv_episodes,
            }]);
        }

        let mut by_season: std::collections::BTreeMap<i64, Vec<RawEpisode>> =
            std::collections::BTreeMap::new();
        for ep in episodes {
            if let Some(season_num) = ep.aired_season {
                by_season.entry(season_num).or_default().push(ep);
            }
        }

        let mut seasons = Vec::new();
        for (season_num, mut eps) in by_season {
            eps.sort_by_key(|e| e.aired_episode_number.unwrap_or(0));
            let tv_episodes: Vec<EpisodeResult> = eps
                .iter()
                .map(|ep| EpisodeResult {
                    name: ep.episode_name.clone().unwrap_or_default(),
                    episode_number: ep.aired_episode_number.unwrap_or(0) as u32,
                    air_date: ep.first_aired.clone().unwrap_or_default(),
                })
                .collect();
            if tv_episodes.is_empty() {
                continue;
            }
            seasons.push(TvSeasonResult {
                name: if season_num > 0 {
                    format!("Season {}", season_num)
                } else {
                    "Specials".to_string()
                },
                season_number: season_num as u32,
                episode_count: tv_episodes.len() as u32,
                episodes: tv_episodes,
            });
        }

        Ok(seasons)
    }
}

impl Default for TvdbClient {
    fn default() -> Self {
        Self::new()
    }
}
