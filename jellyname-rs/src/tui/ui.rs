use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::mkv::reader::read_mkv;
use crate::tmdb::client::TmdbClient;
use crate::tmdb::models::EpisodeResult;
use crate::utils::title::{fix_title, guess_title};
use crate::utils::file_ops::rename_file;

use super::app::{FileState, Mode, ShowCache, TuiApp};
use super::widgets;

enum TuiEvent {
    Tick,
    Key(KeyEvent),
    TmdbMovieSearch(Vec<crate::tmdb::models::MovieSearchResult>),
    TmdbTvSearch(Vec<crate::tmdb::models::TvSearchResult>),
    TmdbSeasonEpisodes(Vec<EpisodeResult>),
    Error(String),
}

/// Spawn a blocking thread that reads crossterm events and forwards them
/// through an mpsc sender.  This is the standard Ratatui+tokio integration
/// pattern — crossterm's `read()` is synchronous, so we keep it off the
/// async runtime.
fn spawn_input_thread(tx: mpsc::UnboundedSender<TuiEvent>) {
    std::thread::spawn(move || {
        loop {
            match crossterm::event::read() {
                Ok(crossterm::event::Event::Key(key)) => {
                    if tx.send(TuiEvent::Key(key)).is_err() {
                        break;
                    }
                }
                Ok(crossterm::event::Event::Resize(..)) => {
                    // wake render loop on resize
                    let _ = tx.send(TuiEvent::Tick);
                }
                Err(_) => break,
                _ => {}
            }
        }
    });
}

pub async fn run_tui(
    mut app: TuiApp,
    tmdb: Arc<TmdbClient>,
    output_dir: std::path::PathBuf,
    format: String,
    dry_run: bool,
    filters: crate::models::filters::Filters,
) -> Result<()> {
    // Terminal setup
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let mut terminal = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))?;
    terminal.clear()?;

    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<TuiEvent>();

    // Spawn ticker for rendering (~60 fps)
    let tick_tx = evt_tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(16)).await;
            if tick_tx.send(TuiEvent::Tick).is_err() {
                break;
            }
        }
    });

    // Spawn dedicated thread for blocking crossterm reads
    spawn_input_thread(evt_tx.clone());

    // Current selection indices for lists
    let mut list_selection: usize = 0;
    let mut confirm_selection: usize = 0;
    let mut tag_text = String::new();

    let result = async {
        loop {
            // Check if we're done
            if app.should_quit || app.current >= app.files.len() {
                break;
            }

            let current_state = app.files[app.current].clone();

            // Auto-advance states that don't need user input or TMDB
            match &current_state {
                FileState::Pending { path } => {
                    let meta = match read_mkv(path) {
                        Ok(m) => m,
                        Err(e) => {
                            app.files[app.current] =
                                FileState::Failed { path: path.clone(), error: e.to_string() };
                            app.current += 1;
                            continue;
                        }
                    };
                    app.files[app.current] = FileState::ReadMkv {
                        path: path.clone(),
                        title: meta.title,
                    };
                    continue;
                }
                FileState::ReadMkv { path, title } => {
                    // In shows mode with cached show/season: skip to confirm
                    if app.mode == Mode::Shows {
                        if let (Some(show_cache), Some(season_cache)) =
                            (&app.show_cache, &app.season_cache)
                        {
                            app.episode_num += 1;
                            let ext = path
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("mkv");
                            let show = crate::models::show::TVShow {
                                name: show_cache.name.clone(),
                                tmdb_id: show_cache.tmdb_id,
                                first_year: show_cache.first_year.clone(),
                            };
                            let season = crate::models::show::TVSeason {
                                season_number: season_cache.season_number,
                            };
                            let dst = crate::processor::shows::compute_episode_dst(
                                &output_dir,
                                &format,
                                &show,
                                &season,
                                app.episode_num,
                                ext,
                            )
                            .unwrap_or_else(|_| std::path::PathBuf::new());
                            let exists = dst.exists();
                            app.files[app.current] = FileState::ConfirmEpisode {
                                path: path.clone(),
                                show: show_cache.clone(),
                                episode_num: app.episode_num,
                                src: path.clone(),
                                dst,
                                exists,
                            };
                            continue;
                        }
                    }

                    let fixed = title.as_deref().map(fix_title);
                    let default_query = fixed
                        .clone()
                        .or_else(|| {
                            let g = guess_title(path);
                            if g.is_empty() { None } else { Some(g) }
                        })
                        .unwrap_or_default();

                    let tx = evt_tx.clone();
                    let tmdb = tmdb.clone();
                    let q = default_query.clone();

                    if fixed.is_some() && !default_query.is_empty() {
                        app.files[app.current] = FileState::Searching {
                            path: path.clone(),
                            mkv_title: title.clone(),
                            query: default_query.clone(),
                        };
                        if app.mode == Mode::Movies {
                            tokio::spawn(async move {
                                match tmdb.search_movie(&q).await {
                                    Ok(results) => {
                                        let _ = tx.send(TuiEvent::TmdbMovieSearch(results));
                                    }
                                    Err(e) => {
                                        let _ = tx.send(TuiEvent::Error(e.to_string()));
                                    }
                                }
                            });
                        } else {
                            tokio::spawn(async move {
                                match tmdb.search_tv(&q).await {
                                    Ok(results) => {
                                        let _ = tx.send(TuiEvent::TmdbTvSearch(results));
                                    }
                                    Err(e) => {
                                        let _ = tx.send(TuiEvent::Error(e.to_string()));
                                    }
                                }
                            });
                        }
                    } else {
                        app.files[app.current] = FileState::SearchInput {
                            path: path.clone(),
                            mkv_title: title.clone(),
                            query: default_query,
                        };
                    }
                    continue;
                }
                _ => {}
            }

            // Render
            terminal.draw(|f| {
                render(f, &app, list_selection, confirm_selection, &tag_text);
            })?;

            // Wait for next event (from tick, input thread, or TMDB task)
            let event = evt_rx.recv().await.unwrap_or(TuiEvent::Tick);

            match event {
                TuiEvent::Tick => {}
                TuiEvent::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    // Global quit
                    if key.code == KeyCode::Char('c')
                        && key.modifiers == KeyModifiers::CONTROL
                    {
                        app.should_quit = true;
                        break;
                    }

                    match &current_state {
                        FileState::SearchInput { path, mkv_title, query } => {
                            let mut q = query.clone();
                            handle_text_input(key, &mut q);
                            let q_trimmed = q.trim().to_string();
                            app.files[app.current] = FileState::SearchInput {
                                path: path.clone(),
                                mkv_title: mkv_title.clone(),
                                query: q.clone(),
                            };

                            if key.code == KeyCode::Enter && !q_trimmed.is_empty() {
                                let query = q.trim().to_string();
                                app.files[app.current] = FileState::Searching {
                                    path: path.clone(),
                                    mkv_title: mkv_title.clone(),
                                    query: query.clone(),
                                };
                                let tx = evt_tx.clone();
                                let tmdb = tmdb.clone();
                                let mode = app.mode.clone();
                                tokio::spawn(async move {
                                    match mode {
                                        Mode::Movies => {
                                            match tmdb.search_movie(&query).await {
                                                Ok(results) => {
                                                    let _ = tx.send(TuiEvent::TmdbMovieSearch(results));
                                                }
                                                Err(e) => {
                                                    let _ = tx.send(TuiEvent::Error(e.to_string()));
                                                }
                                            }
                                        }
                                        Mode::Shows => {
                                            match tmdb.search_tv(&query).await {
                                                Ok(results) => {
                                                    let _ = tx.send(TuiEvent::TmdbTvSearch(results));
                                                }
                                                Err(e) => {
                                                    let _ = tx.send(TuiEvent::Error(e.to_string()));
                                                }
                                            }
                                        }
                                    }
                                });
                            } else if key.code == KeyCode::Esc {
                                app.files[app.current] =
                                    FileState::Skipped { path: path.clone() };
                                app.current += 1;
                                list_selection = 0;
                            }
                        }
                        FileState::SelectMovie { path, results } => {
                            match key.code {
                                KeyCode::Up | KeyCode::Char('k') => {
                                    list_selection = list_selection.saturating_sub(1);
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    list_selection =
                                        (list_selection + 1).min(results.len()); // +1 for "None"
                                }
                                KeyCode::Enter => {
                                    if list_selection < results.len() {
                                        let movie = crate::processor::movies::to_movie(
                                            &results[list_selection],
                                        );
                                        let ext = path
                                            .extension()
                                            .and_then(|e| e.to_str())
                                            .unwrap_or("mkv");
                                        let dst = crate::processor::movies::compute_movie_dst(
                                            &output_dir,
                                            &format,
                                            &movie,
                                            "",
                                            ext,
                                        )
                                        .unwrap_or_else(|_| std::path::PathBuf::new());

                                        let default_tag =
                                            if dst.as_os_str().is_empty() { String::new() }
                                            else {
                                                crate::processor::movies::suggest_tag(&dst)
                                            };

                                        tag_text = default_tag.clone();
                                        app.files[app.current] = FileState::TagInput {
                                            path: path.clone(),
                                            movie,
                                            dst,
                                            default_tag,
                                            tag: String::new(),
                                        };
                                        list_selection = 0;
                                    } else {
                                        // "None of the above" - go back to search input
                                        app.files[app.current] = FileState::SearchInput {
                                            path: path.clone(),
                                            mkv_title: None,
                                            query: String::new(),
                                        };
                                        list_selection = 0;
                                    }
                                }
                                KeyCode::Esc => {
                                    app.files[app.current] =
                                        FileState::Skipped { path: path.clone() };
                                    app.current += 1;
                                    list_selection = 0;
                                }
                                _ => {}
                            }
                        }
                        FileState::SelectShow { path, results } => {
                            match key.code {
                                KeyCode::Up | KeyCode::Char('k') => {
                                    list_selection = list_selection.saturating_sub(1);
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    list_selection =
                                        (list_selection + 1).min(results.len());
                                }
                                KeyCode::Enter => {
                                    if list_selection < results.len() {
                                        let r = &results[list_selection];
                                        app.show_cache = Some(ShowCache {
                                            name: r.name.clone(),
                                            first_year: r.first_year.clone(),
                                            tmdb_id: r.tmdb_id,
                                        });
                                        app.files[app.current] = FileState::SelectSeason {
                                            path: path.clone(),
                                            show: ShowCache {
                                                name: r.name.clone(),
                                                first_year: r.first_year.clone(),
                                                tmdb_id: r.tmdb_id,
                                            },
                                            seasons: r.seasons.clone(),
                                        };
                                        list_selection = 0;
                                    } else {
                                        app.files[app.current] = FileState::SearchInput {
                                            path: path.clone(),
                                            mkv_title: None,
                                            query: String::new(),
                                        };
                                        list_selection = 0;
                                    }
                                }
                                KeyCode::Esc => {
                                    app.files[app.current] =
                                        FileState::Skipped { path: path.clone() };
                                    app.current += 1;
                                    list_selection = 0;
                                }
                                _ => {}
                            }
                        }
                        FileState::SelectSeason { path, show, seasons } => {
                            match key.code {
                                KeyCode::Up | KeyCode::Char('k') => {
                                    list_selection = list_selection.saturating_sub(1);
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    list_selection =
                                        (list_selection + 1).min(seasons.len());
                                }
                                KeyCode::Enter => {
                                    if list_selection < seasons.len() {
                                        let s = &seasons[list_selection];
                                        app.season_cache = Some(s.clone());
                                        app.episode_num = 0;

                                        if app.mixed {
                                            // Mixed mode: go to episode selection
                                            app.files[app.current] =
                                                FileState::SelectEpisode {
                                                    path: path.clone(),
                                                    show: show.clone(),
                                                    season: s.clone(),
                                                    episodes: Vec::new(),
                                                };
                                            list_selection = 0;
                                            let tx = evt_tx.clone();
                                            let tmdb = tmdb.clone();
                                            let sid = show.tmdb_id;
                                            let snum = s.season_number;
                                            tokio::spawn(async move {
                                                match tmdb.season_episodes(sid, snum).await {
                                                    Ok(episodes) => {
                                                        let _ = tx
                                                            .send(TuiEvent::TmdbSeasonEpisodes(episodes));
                                                    }
                                                    Err(e) => {
                                                        let _ = tx
                                                            .send(TuiEvent::Error(e.to_string()));
                                                    }
                                                }
                                            });
                                        } else {
                                            // Non-mixed: auto-number episodes
                                            app.episode_num += 1;
                                            let ext = path
                                                .extension()
                                                .and_then(|e| e.to_str())
                                                .unwrap_or("mkv");
                                            let tv_show = crate::models::show::TVShow {
                                                name: show.name.clone(),
                                                tmdb_id: show.tmdb_id,
                                                first_year: show.first_year.clone(),
                                            };
                                            let tv_season = crate::models::show::TVSeason {
                                                season_number: s.season_number,
                                            };
                                            let dst = crate::processor::shows::compute_episode_dst(
                                                &output_dir,
                                                &format,
                                                &tv_show,
                                                &tv_season,
                                                app.episode_num,
                                                ext,
                                            )
                                            .unwrap_or_else(|_| std::path::PathBuf::new());
                                            let exists = dst.exists();
                                            app.files[app.current] =
                                                FileState::ConfirmEpisode {
                                                    path: path.clone(),
                                                    show: show.clone(),
                                                    episode_num: app.episode_num,
                                                    src: path.clone(),
                                                    dst,
                                                    exists,
                                                };
                                            confirm_selection = 0;
                                        }
                                    } else {
                                        app.files[app.current] = FileState::SearchInput {
                                            path: path.clone(),
                                            mkv_title: None,
                                            query: String::new(),
                                        };
                                        list_selection = 0;
                                    }
                                }
                                KeyCode::Esc => {
                                    app.files[app.current] =
                                        FileState::Skipped { path: path.clone() };
                                    app.current += 1;
                                    list_selection = 0;
                                }
                                _ => {}
                            }
                        }
                        FileState::SelectEpisode { path, show, season, episodes } => {
                            match key.code {
                                KeyCode::Up | KeyCode::Char('k') => {
                                    list_selection = list_selection.saturating_sub(1);
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    list_selection =
                                        (list_selection + 1).min(episodes.len());
                                }
                                KeyCode::Enter => {
                                    if list_selection < episodes.len() {
                                        let ep = &episodes[list_selection];
                                        let ext = path
                                            .extension()
                                            .and_then(|e| e.to_str())
                                            .unwrap_or("mkv");
                                        let dst = crate::processor::shows::compute_episode_dst(
                                            &output_dir,
                                            &format,
                                            &crate::models::show::TVShow {
                                                 name: show.name.clone(),
                                                 first_year: show.first_year.clone(),
                                                 tmdb_id: show.tmdb_id,
                                             },
                                             &crate::models::show::TVSeason {
                                                 season_number: season.season_number,
                                             },
                                            ep.episode_number,
                                            ext,
                                        )
                                        .unwrap_or_else(|_| std::path::PathBuf::new());
                                        app.episode_num = ep.episode_number;
                                        app.files[app.current] =
                                            FileState::ConfirmEpisode {
                                                path: path.clone(),
                                                show: show.clone(),
                                                episode_num: ep.episode_number,
                                                src: path.clone(),
                                                dst,
                                                exists: false,
                                            };
                                        list_selection = 0;
                                        confirm_selection = 0;
                                    } else {
                                        app.files[app.current] = FileState::SearchInput {
                                            path: path.clone(),
                                            mkv_title: None,
                                            query: String::new(),
                                        };
                                        list_selection = 0;
                                    }
                                }
                                KeyCode::Esc => {
                                    app.files[app.current] =
                                        FileState::Skipped { path: path.clone() };
                                    app.current += 1;
                                    list_selection = 0;
                                }
                                _ => {}
                            }
                        }
                        FileState::TagInput { path, movie, dst, default_tag, tag } => {
                            let mut t = tag.clone();
                            handle_text_input(key, &mut t);
                            let default = default_tag.clone();
                            let movie_owned = movie.clone();
                            let dst_owned = dst.clone();
                            app.files[app.current] = FileState::TagInput {
                                path: path.clone(),
                                movie: movie_owned.clone(),
                                dst: dst_owned.clone(),
                                default_tag: default.clone(),
                                tag: t.clone(),
                            };

                            if key.code == KeyCode::Enter {
                                let final_tag = if t.trim().is_empty() {
                                    default.clone()
                                } else {
                                    t.clone()
                                };
                                let final_tag = if final_tag.is_empty() {
                                    String::new()
                                } else {
                                    format!(" - {}", final_tag)
                                };
                                let ext = path
                                    .extension()
                                    .and_then(|e| e.to_str())
                                    .unwrap_or("mkv");
                                let final_dst = crate::processor::movies::compute_movie_dst(
                                    &output_dir,
                                    &format,
                                    &movie_owned,
                                    &final_tag,
                                    ext,
                                )
                                .unwrap_or_else(|_| dst_owned.clone());
                                let exists = final_dst.exists();
                                app.files[app.current] = FileState::ConfirmMovie {
                                    path: path.clone(),
                                    movie: movie_owned,
                                    src: path.clone(),
                                    dst: final_dst,
                                    exists,
                                };
                                confirm_selection = 0;
                            } else if key.code == KeyCode::Esc {
                                let ext = path
                                    .extension()
                                    .and_then(|e| e.to_str())
                                    .unwrap_or("mkv");
                                let final_dst = crate::processor::movies::compute_movie_dst(
                                    &output_dir,
                                    &format,
                                    &movie_owned,
                                    "",
                                    ext,
                                )
                                .unwrap_or_else(|_| dst_owned.clone());
                                let exists = final_dst.exists();
                                app.files[app.current] = FileState::ConfirmMovie {
                                    path: path.clone(),
                                    movie: movie_owned,
                                    src: path.clone(),
                                    dst: final_dst,
                                    exists,
                                };
                                confirm_selection = 0;
                            }
                        }
                        FileState::ConfirmMovie { path, movie: _, src, dst, exists: _ } => {
                            handle_confirm_input(
                                key,
                                &mut confirm_selection,
                                3,
                                &mut app,
                                path.clone(),
                                src.clone(),
                                dst.clone(),
                                dry_run,
                            );
                            // Log is pushed inside handle_confirm_input
                        }
                        FileState::ConfirmEpisode { path, show: _, episode_num: _, src, dst, exists: _ } => {
                            let has_show_cache = app.show_cache.is_some();
                            let buttons = if app.approve_all || !has_show_cache {
                                vec!["Yes", "Skip", "Delete"]
                            } else {
                                vec!["Yes", "Yes to All", "Skip", "Delete"]
                            };
                            let n_buttons = buttons.len();

                            match key.code {
                                KeyCode::Left | KeyCode::Char('h') => {
                                    confirm_selection = confirm_selection.saturating_sub(1);
                                }
                                KeyCode::Right | KeyCode::Char('l') => {
                                    confirm_selection = (confirm_selection + 1).min(n_buttons - 1);
                                }
                                KeyCode::Enter => {
                                    match confirm_selection {
                                        0 => {
                                            // Yes
                                            rename_file(&src, &dst, dry_run).ok();
                                            app.push_log(format!(
                                                "✓ {} -> {}",
                                                src.to_string_lossy(),
                                                dst.to_string_lossy(),
                                            ));
                                            app.files[app.current] =
                                                FileState::Approved { path: path.clone() };
                                            app.current += 1;
                                            confirm_selection = 0;
                                        }
                                        1 if n_buttons == 4 => {
                                            // Yes to All
                                            app.approve_all = true;
                                            rename_file(&src, &dst, dry_run).ok();
                                            app.push_log(format!(
                                                "✓ {} -> {}",
                                                src.to_string_lossy(),
                                                dst.to_string_lossy(),
                                            ));
                                            app.files[app.current] =
                                                FileState::Approved { path: path.clone() };
                                            app.current += 1;
                                            confirm_selection = 0;
                                        }
                                        1 if n_buttons == 3 => {
                                            // Skip (no "Yes to All")
                                            app.files[app.current] =
                                                FileState::Skipped { path: path.clone() };
                                            app.current += 1;
                                            confirm_selection = 0;
                                        }
                                        2 if n_buttons == 4 => {
                                            // Skip
                                            app.files[app.current] =
                                                FileState::Skipped { path: path.clone() };
                                            app.current += 1;
                                            confirm_selection = 0;
                                        }
                                        _ => {
                                            // Delete
                                            if !dry_run {
                                                std::fs::remove_file(&src).ok();
                                            }
                                            app.push_log(format!("✕ {}", src.to_string_lossy()));
                                            app.files[app.current] =
                                                FileState::Deleted { path: path.clone() };
                                            app.current += 1;
                                            confirm_selection = 0;
                                        }
                                    }
                                }
                                KeyCode::Esc => {
                                    app.files[app.current] =
                                        FileState::Skipped { path: path.clone() };
                                    app.current += 1;
                                    confirm_selection = 0;
                                }
                                _ => {}
                            }
                        }
                        FileState::Searching { .. } => {
                            // Waiting for TMDB - no input handling
                        }
                        _ => {}
                    }
                }
                TuiEvent::TmdbMovieSearch(results) => {
                    if let FileState::Searching { path, mkv_title, .. } = &current_state {
                        let filtered =
                            crate::processor::movies::filter_movie_results(results, &filters);
                        if filtered.is_empty() {
                            // No filtered results, go back to search
                            app.files[app.current] = FileState::SearchInput {
                                path: path.clone(),
                                mkv_title: mkv_title.clone(),
                                query: String::new(),
                            };
                        } else {
                            app.files[app.current] = FileState::SelectMovie {
                                path: path.clone(),
                                results: filtered,
                            };
                            list_selection = 0;
                        }
                    }
                }
                TuiEvent::TmdbTvSearch(results) => {
                    if let FileState::Searching { path, mkv_title, .. } = &current_state {
                        if results.is_empty() {
                            app.files[app.current] = FileState::SearchInput {
                                path: path.clone(),
                                mkv_title: mkv_title.clone(),
                                query: String::new(),
                            };
                        } else {
                            app.files[app.current] = FileState::SelectShow {
                                path: path.clone(),
                                results,
                            };
                            list_selection = 0;
                        }
                    }
                }
                TuiEvent::TmdbSeasonEpisodes(episodes) => {
                    if let FileState::SelectEpisode { path, show, season, .. } = &current_state {
                        if episodes.is_empty() {
                            app.files[app.current] = FileState::SearchInput {
                                path: path.clone(),
                                mkv_title: None,
                                query: String::new(),
                            };
                        } else {
                            app.files[app.current] = FileState::SelectEpisode {
                                path: path.clone(),
                                show: show.clone(),
                                season: season.clone(),
                                episodes,
                            };
                            list_selection = 0;
                        }
                    }
                }
                TuiEvent::Error(e) => {
                    if let FileState::Searching { path, .. } = &current_state {
                        app.files[app.current] =
                            FileState::Failed { path: path.clone(), error: e.clone() };
                        app.current += 1;
                    } else if app.current < app.files.len() {
                        app.set_status(format!("Error: {}", e));
                    }
                }
            }
        }

        Ok::<_, color_eyre::eyre::ErrReport>(())
    }
    .await;

    // Cleanup
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    result
}

fn handle_text_input(key: KeyEvent, text: &mut String) {
    match key.code {
        KeyCode::Char(c) => {
            text.push(c);
        }
        KeyCode::Backspace => {
            text.pop();
        }
        _ => {}
    }
}

fn handle_confirm_input(
    key: KeyEvent,
    selection: &mut usize,
    n_buttons: usize,
    app: &mut TuiApp,
    path: std::path::PathBuf,
    src: std::path::PathBuf,
    dst: std::path::PathBuf,
    dry_run: bool,
) {
    match key.code {
        KeyCode::Left | KeyCode::Char('h') => {
            *selection = selection.saturating_sub(1);
        }
        KeyCode::Right | KeyCode::Char('l') => {
            *selection = (*selection + 1).min(n_buttons - 1);
        }
        KeyCode::Enter => {
            match *selection {
                0 => {
                    // Yes
                    let _ = rename_file(&src, &dst, dry_run);
                    app.push_log(format!(
                        "✓ {} -> {}",
                        src.to_string_lossy(),
                        dst.to_string_lossy(),
                    ));
                    app.files[app.current] = FileState::Approved { path };
                    app.current += 1;
                }
                1 => {
                    // Skip
                    app.files[app.current] = FileState::Skipped { path };
                    app.current += 1;
                }
                2 => {
                    // Delete
                    if !dry_run {
                        let _ = std::fs::remove_file(&src);
                    }
                    app.push_log(format!("✕ {}", src.to_string_lossy()));
                    app.files[app.current] = FileState::Deleted { path };
                    app.current += 1;
                }
                _ => {}
            }
            *selection = 0;
        }
        KeyCode::Esc => {
            app.files[app.current] = FileState::Skipped { path };
            app.current += 1;
            *selection = 0;
        }
        _ => {}
    }
}

fn render(
    frame: &mut Frame,
    app: &TuiApp,
    list_selection: usize,
    confirm_selection: usize,
    _tag_text: &str,
) {
    // Vertical layout: main content + log panel
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(frame.area());
    let main_area = chunks[0];
    let log_area = chunks[1];

    // --- Main area: file list on left, modal content on right ---
    let mode_label = match app.mode {
        Mode::Movies => "Movies",
        Mode::Shows => "Shows",
    };

    let file_list_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(main_area);

    widgets::render_file_list(frame, file_list_area[0], &app.files, app.current);

    // Right area (with its own block)
    let right_block = Block::default()
        .title(format!(
            "{}  [{}]",
            mode_label,
            if app.dry_run { "DRY RUN" } else { "live" }
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let right_inner = right_block.inner(file_list_area[1]);
    frame.render_widget(right_block, file_list_area[1]);

    // Modals are rendered over the full main_area (so they can overlap both
    // panels if desired), but we keep the right-inner for non-modal text.
    if let Some(state) = app.current_file() {
        match state {
            FileState::Pending { .. } | FileState::ReadMkv { .. } => {
                let para = Paragraph::new("Processing...");
                frame.render_widget(para, right_inner);
            }
            FileState::SearchInput { query, .. } => {
                widgets::render_search_input(frame, right_inner, query, "Enter search text:");
            }
            FileState::Searching { query, .. } => {
                widgets::render_loading(frame, right_inner, &format!("Searching \"{}\"", query));
            }
            FileState::SelectMovie { results, .. } => {
                widgets::render_select_list(
                    frame, right_inner, "Select Movie",
                    results, list_selection, true,
                );
            }
            FileState::SelectShow { results, .. } => {
                widgets::render_select_list(
                    frame, right_inner, "Select TV Show",
                    results, list_selection, true,
                );
            }
            FileState::SelectSeason { seasons, .. } => {
                widgets::render_select_list(
                    frame, right_inner, "Select Season",
                    seasons, list_selection, true,
                );
            }
            FileState::SelectEpisode { episodes, .. } => {
                widgets::render_select_list(
                    frame, right_inner, "Select Episode",
                    episodes, list_selection, true,
                );
            }
            FileState::TagInput { movie: _, dst: _, default_tag, tag, .. } => {
                widgets::render_tag_input(
                    frame, right_inner,
                    if tag.is_empty() { default_tag } else { tag },
                    default_tag,
                );
            }
            FileState::ConfirmMovie { movie, src, dst, exists, .. } => {
                widgets::render_confirm_dialog(
                    frame, right_inner,
                    &format!("{} ({})", movie.title, movie.year),
                    &src.to_string_lossy(),
                    &dst.to_string_lossy(),
                    *exists,
                    false,
                    confirm_selection,
                );
            }
            FileState::ConfirmEpisode { show, episode_num, src, dst, exists, .. } => {
                widgets::render_confirm_dialog(
                    frame, right_inner,
                    &format!("{} S{:02}E{:02}", show.name, 0, episode_num),
                    &src.to_string_lossy(),
                    &dst.to_string_lossy(),
                    *exists,
                    true,
                    confirm_selection,
                );
            }
            FileState::Approved { .. }
            | FileState::Skipped { .. } | FileState::Deleted { .. } => {
                let para = Paragraph::new("Done");
                frame.render_widget(para, right_inner);
            }
            FileState::Failed { error, .. } => {
                let para = Paragraph::new(format!("Error: {}", error));
                frame.render_widget(para, right_inner);
            }
        }
    }

    // --- Log panel ---
    widgets::render_log_panel(frame, log_area, &app.log);
}
