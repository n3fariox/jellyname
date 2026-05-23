use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use super::app::TuiApp;

pub fn render(frame: &mut Frame, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());

    let title = format!("jellyname-rs  [file {}/{}]", app.current + 1, app.files.len());
    let block = Block::default().title(title).borders(Borders::ALL);
    let para = Paragraph::new("TUI placeholder").block(block);
    frame.render_widget(para, chunks[0]);
}
