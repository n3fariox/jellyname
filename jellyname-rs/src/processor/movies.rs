use std::collections::HashMap;
use std::path::Path;

use crate::models::filters::Filters;
use crate::models::movie::Movie;
use crate::tmdb::models::MovieSearchResult;
use crate::utils::format::apply_format;

pub fn filter_movie_results(
    results: Vec<MovieSearchResult>,
    filters: &Filters,
) -> Vec<MovieSearchResult> {
    match &filters.lang {
        Some(lang) => results
            .into_iter()
            .filter(|r| {
                r.original_language
                    .as_deref()
                    .is_some_and(|l| l.eq_ignore_ascii_case(lang))
            })
            .collect(),
        None => results,
    }
}

pub fn to_movie(r: &MovieSearchResult) -> Movie {
    Movie {
        title: r.title.clone(),
        year: r.year.clone(),
        tmdb_id: r.tmdb_id,
    }
}

pub fn compute_movie_dst(
    output_dir: &Path,
    template: &str,
    movie: &Movie,
    tag: &str,
    ext: &str,
) -> Result<std::path::PathBuf, String> {
    let mut ctx = HashMap::new();
    ctx.insert("title".to_string(), movie.title.clone());
    ctx.insert("year".to_string(), movie.year.clone());
    ctx.insert("tmdb_id".to_string(), movie.tmdb_id.to_string());
    ctx.insert("tag".to_string(), tag.to_string());
    ctx.insert("ext".to_string(), ext.to_string());
    apply_format(template, &ctx).map(|p| output_dir.join(p)).map_err(|e| e.to_string())
}

pub fn suggest_tag(dst: &Path) -> String {
    let parent = dst.parent();
    let stem = dst.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let ext = dst.extension().and_then(|e| e.to_str()).unwrap_or("");
    if let Some(parent) = parent {
        if parent.exists() {
            if let Ok(entries) = std::fs::read_dir(parent) {
                let count = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path().extension().and_then(|e| e.to_str()) == Some(ext)
                            && e.path().file_stem().and_then(|s| s.to_str()) != Some(stem)
                    })
                    .count();
                if count > 0 {
                    return format!("CD{}", count);
                }
            }
        }
    }
    String::new()
}
