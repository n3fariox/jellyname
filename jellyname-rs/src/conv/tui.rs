use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;
use tokio::sync::mpsc;

use super::ffmpeg::build_cmd;
use super::profiles::Profile;

fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    for unit in UNITS {
        if size < 1024.0 {
            return format!("{:.1}{}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{:.1}{}", size, "PB")
}

#[derive(Clone, Copy)]
pub struct AudioOption {
    pub label: &'static str,
    pub codec: &'static str,
    pub bitrate: Option<&'static str>,
}

pub const AUDIO_OPTIONS: &[AudioOption] = &[
    AudioOption { label: "AAC 128k",  codec: "aac",  bitrate: Some("128k") },
    AudioOption { label: "AAC 256k",  codec: "aac",  bitrate: Some("256k") },
    AudioOption { label: "AAC 320k",  codec: "aac",  bitrate: Some("320k") },
    AudioOption { label: "AC3 640k",  codec: "ac3",  bitrate: Some("640k") },
    AudioOption { label: "Copy (passthrough)", codec: "copy", bitrate: None },
];

#[derive(Clone)]
struct FileEntry {
    path: PathBuf,
    selected: bool,
    size: u64,
}

enum ConvTuiEvent {
    Tick,
    Key(KeyEvent),
}

pub struct ConvSelection {
    pub files: Vec<PathBuf>,
    pub profile_name: String,
    pub profile: Profile,
    pub audio_option: AudioOption,
}

enum ConvScreen {
    SelectFiles {
        entries: Vec<FileEntry>,
        idx: usize,
    },
    SelectProfile {
        files: Vec<PathBuf>,
        profile_names: Vec<String>,
        profile_descs: Vec<String>,
        idx: usize,
    },
    SelectAudio {
        files: Vec<PathBuf>,
        video_profile_name: String,
        video_profile: Profile,
        idx: usize,
    },
    Confirm {
        files: Vec<PathBuf>,
        profile_name: String,
        profile: Profile,
        audio_option: AudioOption,
    },
    ShowFileInfo {
        entries: Vec<FileEntry>,
        idx: usize,
        info: String,
    },
}

async fn probe_file(path: &PathBuf) -> Result<String> {
    let output = tokio::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-of",
            "json",
            "-show_entries",
            "format=format_name,duration,size,bit_rate",
            "-show_entries",
            "stream=index,codec_name,codec_type,profile,width,height,bit_rate,sample_rate,channels,channel_layout",
        ])
        .arg(path.as_os_str())
        .output()
        .await?;

    let raw = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&raw)?;

    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("?");
    let size_str = human_size(
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
    );

    let mut lines = Vec::new();
    lines.push(format!("{}  ({})", filename, size_str));
    lines.push(String::new());

    if let Some(fmt) = parsed.get("format") {
        if let Some(name) = fmt.get("format_name").and_then(|v| v.as_str()) {
            lines.push(format!("Format: {}", name));
        }
        if let Some(dur) = fmt
            .get("duration")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
        {
            let h = (dur / 3600.0) as u64;
            let m = ((dur % 3600.0) / 60.0) as u64;
            let s = (dur % 60.0) as u64;
            lines.push(format!("Duration: {:02}:{:02}:{:02}", h, m, s));
        }
        if let Some(br) = fmt
            .get("bit_rate")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
        {
            lines.push(format!("Overall bitrate: {:.0} kbps", br / 1000.0));
        }
        lines.push(String::new());
    }

    let streams = parsed
        .get("streams")
        .and_then(|v| v.as_array())
        .map(|a| a.to_vec())
        .unwrap_or_default();

    let fmt_streams = |codec_type: &str, label: &str| -> Vec<String> {
        let typed: Vec<&serde_json::Value> =
            streams.iter().filter(|s| s.get("codec_type").and_then(|v| v.as_str()) == Some(codec_type)).collect();
        if typed.is_empty() {
            return vec![];
        }
        let mut out = vec![format!("{}:", label)];
        for s in &typed {
            let codec = s.get("codec_name").and_then(|v| v.as_str()).unwrap_or("?").to_uppercase();
            let profile = s.get("profile").and_then(|v| v.as_str()).filter(|p| !p.is_empty());
            let w = s.get("width").and_then(|v| v.as_u64()).unwrap_or(0);
            let h = s.get("height").and_then(|v| v.as_u64()).unwrap_or(0);
            let br = s.get("bit_rate").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok());
            let sr = s.get("sample_rate").and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok());
            let ch = s.get("channels").and_then(|v| v.as_u64()).unwrap_or(0);

            match codec_type {
                "video" => {
                    let mut parts = vec![format!("  {}", codec)];
                    if let Some(p) = profile {
                        parts.push(format!("({})", p));
                    }
                    if w > 0 && h > 0 {
                        parts.push(format!("{}x{}", w, h));
                    }
                    if let Some(b) = br {
                        parts.push(format!("@ {:.0} kbps", b / 1000.0));
                    }
                    out.push(parts.join(" "));
                }
                "audio" => {
                    let ch_str = match ch {
                        1 => "Mono",
                        2 => "Stereo",
                        6 => "5.1",
                        8 => "7.1",
                        _ => "",
                    };
                    let mut parts = vec![format!("  {}", codec)];
                    if !ch_str.is_empty() {
                        parts.push(ch_str.to_string());
                    } else if ch > 0 {
                        parts.push(format!("{}ch", ch));
                    }
                    if let Some(r) = sr {
                        parts.push(format!("{:.0}kHz", r / 1000.0));
                    }
                    if let Some(b) = br {
                        parts.push(format!("@ {:.0} kbps", b / 1000.0));
                    }
                    out.push(parts.join(" "));
                }
                "subtitle" => {
                    out.push(format!("  {}", codec));
                }
                _ => {}
            }
        }
        out.push(String::new());
        out
    };

    lines.extend(fmt_streams("video", "Video"));
    lines.extend(fmt_streams("audio", "Audio"));
    lines.extend(fmt_streams("subtitle", "Subtitles"));

    Ok(lines.join("\n"))
}

pub async fn run_conv_tui(
    all_files: Vec<PathBuf>,
    profiles: HashMap<String, Profile>,
) -> Result<Option<ConvSelection>> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let mut terminal = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))?;
    terminal.clear()?;

    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<ConvTuiEvent>();

    let tick_tx = evt_tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(16)).await;
            if tick_tx.send(ConvTuiEvent::Tick).is_err() {
                break;
            }
        }
    });

    std::thread::spawn(move || {
        loop {
            match crossterm::event::read() {
                Ok(crossterm::event::Event::Key(key))
                    if evt_tx.send(ConvTuiEvent::Key(key)).is_err() =>
                {
                    break;
                }
                Ok(crossterm::event::Event::Resize(..)) => {
                    let _ = evt_tx.send(ConvTuiEvent::Tick);
                }
                Err(_) => break,
                _ => {}
            }
        }
    });

    let entries: Vec<FileEntry> = all_files
        .iter()
        .map(|f| FileEntry {
            path: f.clone(),
            selected: true,
            size: std::fs::metadata(f).map(|m| m.len()).unwrap_or(0),
        })
        .collect();
    let profile_names: Vec<String> = profiles.keys().cloned().collect();
    let profile_descs: Vec<String> = profiles
        .values()
        .map(|p| p.description.clone().unwrap_or_default())
        .collect();
    let mut screen = ConvScreen::SelectFiles { entries, idx: 0 };
    let mut result = None;

    let res = async {
        loop {
            terminal.draw(|f| render_conv(f, &screen))?;

            let event = evt_rx.recv().await.unwrap_or(ConvTuiEvent::Tick);
            let ConvTuiEvent::Key(key) = event else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if key.code == KeyCode::Char('c')
                && key.modifiers == crossterm::event::KeyModifiers::CONTROL
            {
                break;
            }

            match &mut screen {
                ConvScreen::SelectFiles { entries, idx } => {
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            *idx = idx.saturating_sub(1);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            *idx = (*idx + 1).min(entries.len().saturating_sub(1));
                        }
                        KeyCode::Char(' ') if *idx < entries.len() => {
                            entries[*idx].selected = !entries[*idx].selected;
                        }
                        KeyCode::Enter => {
                            let selected: Vec<PathBuf> = entries
                                .iter()
                                .filter(|e| e.selected)
                                .map(|e| e.path.clone())
                                .collect();
                            if selected.is_empty() {
                                continue;
                            }
                            screen = ConvScreen::SelectProfile {
                                files: selected,
                                profile_names: profile_names.clone(),
                                profile_descs: profile_descs.clone(),
                                idx: 0,
                            };
                        }
                        KeyCode::Char('i') if *idx < entries.len() => {
                            let path = entries[*idx].path.clone();
                            let saved_entries = entries.clone();
                            let saved_idx = *idx;
                            let info = probe_file(&path).await.map_err(|e| {
                                color_eyre::eyre::eyre!("ffprobe failed: {e}")
                            })?;
                            screen = ConvScreen::ShowFileInfo {
                                entries: saved_entries,
                                idx: saved_idx,
                                info,
                            };
                        }
                        KeyCode::Esc => {
                            break;
                        }
                        _ => {}
                    }
                }
                ConvScreen::SelectProfile {
                    files,
                    profile_names,
                    profile_descs: _,
                    idx,
                } => {
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            *idx = idx.saturating_sub(1);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            *idx = (*idx + 1).min(profile_names.len().saturating_sub(1));
                        }
                        KeyCode::Enter if *idx < profile_names.len() => {
                            let name = profile_names[*idx].clone();
                            let profile = profiles[&name].clone();
                            let files = std::mem::take(files);
                            screen = ConvScreen::SelectAudio {
                                files,
                                video_profile_name: name,
                                video_profile: profile,
                                idx: 0,
                            };
                        }
                        KeyCode::Esc => {
                            let entries: Vec<FileEntry> = files
                                .iter()
                                .map(|f| FileEntry {
                                    path: f.clone(),
                                    selected: true,
                                    size: std::fs::metadata(f).map(|m| m.len()).unwrap_or(0),
                                })
                                .collect();
                            screen = ConvScreen::SelectFiles { entries, idx: 0 };
                        }
                        _ => {}
                    }
                }
                ConvScreen::SelectAudio {
                    files,
                    video_profile_name,
                    video_profile,
                    idx,
                } => {
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            *idx = idx.saturating_sub(1);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            *idx = (*idx + 1).min(AUDIO_OPTIONS.len().saturating_sub(1));
                        }
                        KeyCode::Enter if *idx < AUDIO_OPTIONS.len() => {
                            let audio = AUDIO_OPTIONS[*idx];
                            let name = video_profile_name.clone();
                            let profile = video_profile.clone();
                            let files = std::mem::take(files);
                            screen = ConvScreen::Confirm {
                                files,
                                profile_name: name,
                                profile,
                                audio_option: audio,
                            };
                        }
                        KeyCode::Esc => {
                            screen = ConvScreen::SelectProfile {
                                files: std::mem::take(files),
                                profile_names: profile_names.clone(),
                                profile_descs: profile_descs.clone(),
                                idx: 0,
                            };
                        }
                        _ => {}
                    }
                }
                ConvScreen::Confirm {
                    files, profile_name, profile, audio_option,
                } => {
                    match key.code {
                        KeyCode::Enter => {
                            let sel = ConvSelection {
                                files: std::mem::take(files),
                                profile_name: profile_name.clone(),
                                profile: profile.clone(),
                                audio_option: *audio_option,
                            };
                            result = Some(sel);
                            break;
                        }
                        KeyCode::Esc => {
                            let files = std::mem::take(files);
                            let name = profile_name.clone();
                            let prof = profile.clone();
                            screen = ConvScreen::SelectAudio {
                                files,
                                video_profile_name: name,
                                video_profile: prof,
                                idx: 0,
                            };
                        }
                        _ => {}
                    }
                }
                ConvScreen::ShowFileInfo { entries, idx, info: _ } => {
                    if key.code == KeyCode::Esc {
                        let saved = std::mem::take(entries);
                        let saved_idx = *idx;
                        screen = ConvScreen::SelectFiles {
                            entries: saved,
                            idx: saved_idx,
                        };
                    }
                }
            }
        }

        Ok::<_, color_eyre::eyre::ErrReport>(())
    }
    .await;

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    res?;
    Ok(result)
}

fn render_conv(frame: &mut Frame, screen: &ConvScreen) {
    match screen {
        ConvScreen::SelectFiles { entries, idx } => {
            let block = Block::default()
                .title("Select files to convert  [Space: toggle  i: info  Enter: confirm  Esc: cancel]")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded);
            let inner = block.inner(frame.area());
            frame.render_widget(block, frame.area());

            let max_rows = inner.height as usize;
            let start = idx.saturating_sub(max_rows / 2);

            let lines: Vec<Line> = entries
                .iter()
                .enumerate()
                .skip(start)
                .take(max_rows)
                .map(|(i, entry)| {
                    let marker = if i == *idx { "▸ " } else { "  " };
                    let checkbox = if entry.selected { "[x]" } else { "[ ]" };
                    let name = entry
                        .path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("?");
                    let size_str = human_size(entry.size);
                    let style = if i == *idx {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    Line::from(Span::styled(
                        format!("{}{} {}  [{}]", marker, checkbox, name, size_str),
                        style,
                    ))
                })
                .collect();

            frame.render_widget(
                Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
                inner,
            );
        }
        ConvScreen::SelectProfile {
            files: _,
            profile_names,
            profile_descs,
            idx,
        } => {
            let block = Block::default()
                .title("Select profile  [Enter: confirm  Esc: back]")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded);
            let inner = block.inner(frame.area());
            frame.render_widget(block, frame.area());

            let max_rows = inner.height as usize;
            let start = idx.saturating_sub(max_rows / 2);

            let lines: Vec<Line> = profile_names
                .iter()
                .enumerate()
                .zip(profile_descs.iter())
                .skip(start)
                .take(max_rows)
                .map(|((i, name), desc)| {
                    let marker = if i == *idx { "▸ " } else { "  " };
                    let style = if i == *idx {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let display = if desc.is_empty() {
                        name.clone()
                    } else {
                        format!("{} — {}", name, desc)
                    };
                    Line::from(Span::styled(format!("{}{}", marker, display), style))
                })
                .collect();

            frame.render_widget(
                Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
                inner,
            );
        }
        ConvScreen::SelectAudio {
            files: _,
            video_profile_name,
            video_profile: _,
            idx,
        } => {
            let block = Block::default()
                .title(format!(
                    "Select audio option  [Enter: confirm  Esc: back]  Video: {}",
                    video_profile_name
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded);
            let inner = block.inner(frame.area());
            frame.render_widget(block, frame.area());

            let max_rows = inner.height as usize;
            let start = idx.saturating_sub(max_rows / 2);

            let lines: Vec<Line> = AUDIO_OPTIONS
                .iter()
                .enumerate()
                .skip(start)
                .take(max_rows)
                .map(|(i, opt)| {
                    let marker = if i == *idx { "▸ " } else { "  " };
                    let style = if i == *idx {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    Line::from(Span::styled(
                        format!("{}{}", marker, opt.label),
                        style,
                    ))
                })
                .collect();

            frame.render_widget(
                Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
                inner,
            );
        }
        ConvScreen::Confirm {
            files,
            profile_name,
            profile,
            audio_option,
        } => {
            let title = format!(
                "Confirm conversion  [Enter: start  Esc: back]  Profile: {}",
                profile_name
            );
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded);
            let inner = block.inner(frame.area());
            frame.render_widget(block, frame.area());

            let sample_in = PathBuf::from("<input_file>");
            let sample_bak = sample_in.with_file_name(format!(
                "{}.bak",
                sample_in.file_name().unwrap().to_string_lossy()
            ));
            // Build a merged profile with the audio option applied for the sample command
            let mut merged = profile.clone();
            merged.audio_codec = Some(audio_option.codec.into());
            merged.audio_bitrate = audio_option.bitrate.map(|s| s.into());
            let sample_cmd = build_cmd(&merged, &sample_bak, &sample_in);
            let cmd_str = sample_cmd.join(" ");

            let file_list: String = files
                .iter()
                .map(|f| {
                    format!(
                        "  • {}",
                        f.file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("?")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            let text = Text::from(vec![
                Line::from(Span::styled(
                    "Files to convert:",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::raw(file_list)),
                Line::from(""),
                Line::from(Span::styled(
                    format!("Video: {}", profile_name),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    format!("Audio: {}", audio_option.label),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Example ffmpeg command:",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::raw(cmd_str)),
                Line::from(""),
                Line::from(Span::styled(
                    "[Enter] start  [Esc] back",
                    Style::default().fg(Color::Gray),
                )),
            ]);

            frame.render_widget(
                Paragraph::new(text).wrap(Wrap { trim: false }),
                inner,
            );
        }
        ConvScreen::ShowFileInfo { info, .. } => {
            let block = Block::default()
                .title("File info  [Esc: back]")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded);
            let inner = block.inner(frame.area());
            frame.render_widget(block, frame.area());

            let lines: Vec<Line> = info
                .lines()
                .map(|l| {
                    let style = if l.ends_with(':') || (!l.starts_with(' ') && !l.is_empty()) {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    Line::from(Span::styled(l.to_string(), style))
                })
                .collect();

            let text = Text::from(lines);

            frame.render_widget(
                Paragraph::new(text).wrap(Wrap { trim: false }),
                inner,
            );
        }
    }
}
