use std::collections::HashMap;
use std::path::Path;

use crate::models::show::{TVSeason, TVShow};
use crate::utils::format::apply_format;

pub fn compute_episode_dst(
    output_dir: &Path,
    template: &str,
    show: &TVShow,
    season: &TVSeason,
    episode_num: u32,
    ext: &str,
) -> Result<std::path::PathBuf, String> {
    let mut ctx = HashMap::new();
    ctx.insert("name".to_string(), show.name.clone());
    ctx.insert("first_year".to_string(), show.first_year.clone());
    ctx.insert("tmdb_id".to_string(), show.tmdb_id.to_string());
    ctx.insert("season_num".to_string(), format!("{:02}", season.season_number));
    ctx.insert("episode_num".to_string(), format!("{:02}", episode_num));
    ctx.insert("ext".to_string(), ext.to_string());
    apply_format(template, &ctx).map(|p| output_dir.join(p)).map_err(|e| e.to_string())
}
