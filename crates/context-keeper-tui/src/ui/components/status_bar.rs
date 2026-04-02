//! Bottom status bar: shows status message and context-sensitive keybind hints.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::ui::theme;

pub fn render(f: &mut Frame<'_>, area: Rect, status: &str, hints: &[(&str, &str)]) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let status_line = Paragraph::new(Line::from(Span::styled(
        status,
        ratatui::style::Style::default().fg(theme::MUTED),
    )));
    f.render_widget(status_line, chunks[0]);

    let mut spans = Vec::new();
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", ratatui::style::Style::default()));
        }
        spans.push(Span::styled(
            *key,
            ratatui::style::Style::default()
                .fg(theme::ACCENT)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {desc}"),
            ratatui::style::Style::default().fg(theme::MUTED),
        ));
    }
    let hints_line = Paragraph::new(Line::from(spans)).right_aligned();
    f.render_widget(hints_line, chunks[1]);
}
