use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;

pub fn render_file_list(frame: &mut Frame, area: Rect, files: &[super::app::FileState], current: usize) {
    let block = Block::default()
        .title("Files")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let max_rows = inner.height as usize;
    let start = current.saturating_sub(max_rows / 2);

    let lines: Vec<Line> = files
        .iter()
        .enumerate()
        .skip(start)
        .take(max_rows)
        .map(|(i, state)| {
            let prefix = if i == current { "▸ " } else { "  " };
            let (icon, path) = match state {
                super::app::FileState::Pending { path }
                | super::app::FileState::ReadMkv { path, .. }
                | super::app::FileState::SearchInput { path, .. }
                | super::app::FileState::Searching { path, .. }
                | super::app::FileState::SelectMovie { path, .. }
                | super::app::FileState::SelectShow { path, .. }
                | super::app::FileState::SelectSeason { path, .. }
                | super::app::FileState::SelectEpisode { path, .. }
                | super::app::FileState::TagInput { path, .. }
                | super::app::FileState::ConfirmMovie { path, .. }
                | super::app::FileState::ConfirmEpisode { path, .. } => (" ◌", path),
                super::app::FileState::Approved { path } => (" ✓", path),
                super::app::FileState::Failed { path, .. } => (" ✗", path),
                super::app::FileState::Skipped { path } => (" –", path),
                super::app::FileState::Deleted { path } => (" ✕", path),
            };

            let style = if i == current {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();

            Line::from(Span::styled(format!("{}{} {}", prefix, icon, name), style))
        })
        .collect();

    let para = Paragraph::new(Text::from(lines));
    frame.render_widget(para, inner);
}

pub fn render_search_input(frame: &mut Frame, area: Rect, query: &str, prompt: &str) {
    let block = Block::default()
        .title("Search TMDB")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = Text::from(vec![
        Line::from(Span::raw(format!("{}\n\n", prompt))),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
            Span::raw(query),
            Span::styled("█", Style::default().fg(Color::Cyan).add_modifier(Modifier::SLOW_BLINK)),
        ]),
    ]);
    let para = Paragraph::new(text).wrap(Wrap { trim: false });
    frame.render_widget(para, inner);
}

pub fn render_select_list<T: std::fmt::Display>(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    items: &[T],
    selected: usize,
    show_none: bool,
) {
    let mut title_str = title.to_string();
    if show_none {
        title_str.push_str(" [Esc: skip]");
    }
    let block = Block::default()
        .title(title_str)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let max_rows = inner.height as usize - 1;
    let start = selected.saturating_sub(max_rows / 2);

    let mut lines = Vec::new();
    for (i, item) in items.iter().enumerate().skip(start).take(max_rows) {
        let marker = if i == selected { "▸ " } else { "  " };
        let style = if i == selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(format!("{}{}", marker, item), style)));
    }

    if show_none {
        let none_marker = if selected >= items.len() { "▸ " } else { "  " };
        lines.push(Line::from(Span::styled(
            format!("{}None of the above", none_marker),
            if selected >= items.len() {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        )));
    }

    let para = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    frame.render_widget(para, inner);
}

pub fn render_confirm_dialog(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    src: &str,
    dst: &str,
    exists: bool,
    show_approve_all: bool,
    selected: usize,
) {
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let exists_text = if exists {
        " ⚠ Destination exists!"
    } else {
        ""
    };

    let buttons = {
        let mut b = vec!["Yes", "Skip", "Delete"];
        if show_approve_all {
            b.insert(1, "Yes to All");
        }
        b
    };

    let button_line: Vec<Span> = buttons
        .iter()
        .enumerate()
        .flat_map(|(i, label)| {
            let style = if i == selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            vec![
                Span::styled(format!("[{}]", label), style),
                Span::raw(" "),
            ]
        })
        .collect();

    let text = Text::from(vec![
        Line::from(Span::raw(format!("src: {}", src))),
        Line::from(Span::raw(format!("dst: {}", dst))),
        Line::from(Span::styled(
            exists_text,
            Style::default().fg(Color::Red),
        )),
        Line::from(""),
        Line::from(button_line),
    ]);

    let para = Paragraph::new(text).wrap(Wrap { trim: false });
    frame.render_widget(para, inner);
}

pub fn render_tag_input(frame: &mut Frame, area: Rect, tag: &str, default_tag: &str) {
    let block = Block::default()
        .title("Optional Tag")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let default_hint = if !default_tag.is_empty() {
        format!(" (default: {})", default_tag)
    } else {
        String::new()
    };

    let text = Text::from(vec![
        Line::from(Span::raw(format!("Enter a tag{}:", default_hint))),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
            Span::raw(tag),
            Span::styled("█", Style::default().fg(Color::Cyan).add_modifier(Modifier::SLOW_BLINK)),
        ]),
        Line::from(Span::styled(
            "[Enter] confirm  [Esc] skip tag",
            Style::default().fg(Color::Gray),
        )),
    ]);

    let para = Paragraph::new(text).wrap(Wrap { trim: false });
    frame.render_widget(para, inner);
}

pub fn render_log_panel(frame: &mut Frame, area: Rect, log: &std::collections::VecDeque<String>) {
    let block = Block::default()
        .title("Log")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let max_rows = inner.height as usize;
    let total = log.len();
    let start = total.saturating_sub(max_rows);
    let lines: Vec<Line> = log
        .iter()
        .skip(start)
        .take(max_rows)
        .map(|msg| Line::from(msg.as_str()))
        .collect();

    let para = Paragraph::new(Text::from(lines));
    frame.render_widget(para, inner);
}

pub fn render_loading(frame: &mut Frame, area: Rect, msg: &str) {
    let block = Block::default()
        .title("Please wait")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = Text::from(Line::from(Span::styled(
        format!("{}...", msg),
        Style::default().fg(Color::Cyan).add_modifier(Modifier::SLOW_BLINK),
    )));

    let para = Paragraph::new(text)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: false });
    frame.render_widget(para, inner);
}
