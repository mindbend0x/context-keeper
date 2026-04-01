//! Top tab bar widget.

use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Tabs as RatatuiTabs};
use ratatui::Frame;

use crate::ui::theme;

pub fn render(
    f: &mut Frame<'_>,
    area: Rect,
    titles: &[&str],
    selected: usize,
) {
    let spans: Vec<Line> = titles
        .iter()
        .enumerate()
        .map(|(i, t)| {
            if i == selected {
                Line::from(Span::styled(
                    format!(" {t} "),
                    theme::tab_active(),
                ))
            } else {
                Line::from(Span::styled(
                    format!(" {t} "),
                    theme::tab_inactive(),
                ))
            }
        })
        .collect();

    let tabs = RatatuiTabs::new(spans)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border_default())
                .title(
                    Line::from(vec![
                        Span::styled(" Context Keeper ", theme::title()),
                        Span::styled("TUI ", theme::stat_label()),
                    ]),
                ),
        )
        .select(selected)
        .highlight_style(theme::tab_active().add_modifier(Modifier::UNDERLINED))
        .divider(Span::styled(" │ ", ratatui::style::Style::default().fg(theme::MUTED)));

    f.render_widget(tabs, area);
}
