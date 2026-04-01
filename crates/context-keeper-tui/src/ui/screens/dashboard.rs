//! Dashboard screen: graph stats + scrollable recent memories.

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::backend::TuiBackend;
use crate::types::{GraphStats, MemoryRow};
use crate::ui::event::AppEvent;
use crate::ui::theme;

#[derive(Default)]
pub struct DashboardState {
    pub stats: GraphStats,
    pub recent: Vec<MemoryRow>,
    pub cursor: usize,
    pub scroll_offset: usize,
}

impl DashboardState {
    pub fn apply_event(&mut self, ev: &AppEvent) {
        match ev {
            AppEvent::StatsReady(Ok(s)) => self.stats = s.clone(),
            AppEvent::RecentReady(Ok(rows)) => {
                self.recent = rows.clone();
                self.cursor = 0;
                self.scroll_offset = 0;
            }
            _ => {}
        }
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        tx: &mpsc::UnboundedSender<AppEvent>,
        backend: &Arc<dyn TuiBackend>,
    ) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    if self.cursor < self.scroll_offset {
                        self.scroll_offset = self.cursor;
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.recent.is_empty() && self.cursor < self.recent.len() - 1 {
                    self.cursor += 1;
                }
            }
            KeyCode::Char('r') => {
                self.refresh(tx, backend);
            }
            _ => {}
        }
    }

    pub fn refresh(
        &self,
        tx: &mpsc::UnboundedSender<AppEvent>,
        backend: &Arc<dyn TuiBackend>,
    ) {
        let tx2 = tx.clone();
        let b = Arc::clone(backend);
        tokio::spawn(async move {
            let r = b.list_recent(50).await.map_err(|e| e.to_string());
            let _ = tx2.send(AppEvent::RecentReady(r));
        });
        let tx3 = tx.clone();
        let b2 = Arc::clone(backend);
        tokio::spawn(async move {
            let r = b2.get_stats().await.map_err(|e| e.to_string());
            let _ = tx3.send(AppEvent::StatsReady(r));
        });
    }

    pub fn render(&self, f: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(28), Constraint::Min(30)])
            .split(area);

        self.render_stats(f, chunks[0]);
        self.render_recent(f, chunks[1]);
    }

    fn render_stats(&self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_default())
            .title(Line::from(" Overview ").style(theme::title()));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let stats = &self.stats;
        let lines = vec![
            Line::from(""),
            stat_line("  Entities", stats.entities),
            Line::from(""),
            stat_line("  Memories", stats.memories),
            Line::from(""),
            stat_line("  Namespaces", stats.namespaces),
            Line::from(""),
            stat_line("  Agents", stats.agents),
        ];

        f.render_widget(Paragraph::new(lines), inner);
    }

    fn render_recent(&self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_default())
            .title(Line::from(format!(" Recent Memories ({}) ", self.recent.len())).style(theme::title()));

        let inner = block.inner(area);
        f.render_widget(block, area);

        if self.recent.is_empty() {
            f.render_widget(
                Paragraph::new("  Loading... press 'r' to refresh")
                    .style(Style::default().fg(theme::MUTED)),
                inner,
            );
            return;
        }

        let visible_height = inner.height as usize;
        // Clamp scroll to keep cursor visible
        let mut offset = self.scroll_offset;
        if self.cursor >= offset + visible_height {
            offset = self.cursor.saturating_sub(visible_height - 1);
        }
        if self.cursor < offset {
            offset = self.cursor;
        }
        let start = offset;
        let end = (start + visible_height).min(self.recent.len());

        let items: Vec<ListItem> = self.recent[start..end]
            .iter()
            .enumerate()
            .map(|(vi, m)| {
                let idx = start + vi;
                let is_selected = idx == self.cursor;
                let style = if is_selected {
                    theme::selected_row()
                } else {
                    Style::default()
                };

                let ts = if m.created_at.len() > 19 {
                    &m.created_at[..19]
                } else {
                    &m.created_at
                };

                let content_preview: String = m
                    .content
                    .chars()
                    .take(area.width.saturating_sub(24) as usize)
                    .collect();

                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{ts}  "),
                        style.fg(theme::TIMESTAMP),
                    ),
                    Span::styled(content_preview, style),
                ]))
            })
            .collect();

        f.render_widget(List::new(items), inner);
    }

    pub fn hints(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("j/k", "navigate"),
            ("r", "refresh"),
            ("Tab", "next screen"),
        ]
    }
}

fn stat_line(label: &str, value: usize) -> Line<'_> {
    Line::from(vec![
        Span::styled(label, theme::stat_label()),
        Span::styled(
            format!("  {value}"),
            theme::stat_value(),
        ),
    ])
}
