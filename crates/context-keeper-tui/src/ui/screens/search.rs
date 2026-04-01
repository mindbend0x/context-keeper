//! Search screen: query input, results list with scores, detail split pane.

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::backend::TuiBackend;
use crate::types::SearchHit;
use crate::ui::components::input::TextInput;
use crate::ui::event::AppEvent;
use crate::ui::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Input,
    Results,
}

pub struct SearchState {
    pub input: TextInput,
    pub hits: Vec<SearchHit>,
    pub cursor: usize,
    pub scroll_offset: usize,
    focus: Focus,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            input: TextInput::default(),
            hits: Vec::new(),
            cursor: 0,
            scroll_offset: 0,
            focus: Focus::Input,
        }
    }
}

/// Returned when the user presses Enter on a selected search result.
pub enum SearchAction {
    None,
    DrillIntoEntity(String),
}

impl SearchState {
    pub fn apply_event(&mut self, ev: &AppEvent) {
        match ev {
            AppEvent::SearchReady(Ok(h)) | AppEvent::ExpandReady(Ok(h)) => {
                self.hits = h.clone();
                self.cursor = 0;
                self.scroll_offset = 0;
                if !self.hits.is_empty() {
                    self.focus = Focus::Results;
                }
            }
            _ => {}
        }
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        tx: &mpsc::UnboundedSender<AppEvent>,
        backend: &Arc<dyn TuiBackend>,
    ) -> SearchAction {
        if key.kind != KeyEventKind::Press {
            return SearchAction::None;
        }

        match self.focus {
            Focus::Input => match key.code {
                KeyCode::Enter => {
                    self.do_search(tx, backend);
                    SearchAction::None
                }
                KeyCode::Down if !self.hits.is_empty() => {
                    self.focus = Focus::Results;
                    SearchAction::None
                }
                KeyCode::Esc => {
                    if !self.hits.is_empty() {
                        self.focus = Focus::Results;
                    }
                    SearchAction::None
                }
                _ => {
                    self.input.handle_key(key.code);
                    SearchAction::None
                }
            },
            Focus::Results => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.cursor > 0 {
                        self.cursor -= 1;
                        if self.cursor < self.scroll_offset {
                            self.scroll_offset = self.cursor;
                        }
                    }
                    SearchAction::None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.hits.is_empty() && self.cursor < self.hits.len() - 1 {
                        self.cursor += 1;
                    }
                    SearchAction::None
                }
                KeyCode::Enter => {
                    if let Some(hit) = self.hits.get(self.cursor) {
                        SearchAction::DrillIntoEntity(hit.name.clone())
                    } else {
                        SearchAction::None
                    }
                }
                KeyCode::Char('/') | KeyCode::Char('i') => {
                    self.focus = Focus::Input;
                    SearchAction::None
                }
                KeyCode::Char('x') => {
                    self.do_expand(tx, backend);
                    SearchAction::None
                }
                KeyCode::Esc => {
                    self.focus = Focus::Input;
                    SearchAction::None
                }
                _ => SearchAction::None,
            },
        }
    }

    fn do_search(&self, tx: &mpsc::UnboundedSender<AppEvent>, backend: &Arc<dyn TuiBackend>) {
        let q = self.input.value.clone();
        if q.is_empty() {
            return;
        }
        let tx = tx.clone();
        let b = Arc::clone(backend);
        tokio::spawn(async move {
            let r = b.search_memory(&q, 20).await.map_err(|e| e.to_string());
            let _ = tx.send(AppEvent::SearchReady(r));
        });
    }

    fn do_expand(&self, tx: &mpsc::UnboundedSender<AppEvent>, backend: &Arc<dyn TuiBackend>) {
        let q = self.input.value.clone();
        if q.is_empty() {
            return;
        }
        let tx = tx.clone();
        let b = Arc::clone(backend);
        tokio::spawn(async move {
            let r = b.expand_search(&q, 20).await.map_err(|e| e.to_string());
            let _ = tx.send(AppEvent::ExpandReady(r));
        });
    }

    pub fn render(&self, f: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(5)])
            .split(area);

        self.input
            .render(f, chunks[0], " Search (Enter to query, x to expand) ", self.focus == Focus::Input);

        if self.hits.is_empty() {
            let empty = Paragraph::new("  No results. Type a query and press Enter.")
                .style(Style::default().fg(theme::MUTED))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(theme::border_default())
                        .title(Line::from(" Results ").style(theme::title())),
                );
            f.render_widget(empty, chunks[1]);
            return;
        }

        let result_area = chunks[1];
        let detail_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(result_area);

        self.render_results_list(f, detail_split[0]);
        self.render_detail(f, detail_split[1]);
    }

    fn render_results_list(&self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(if self.focus == Focus::Results {
                theme::border_focused()
            } else {
                theme::border_default()
            })
            .title(
                Line::from(format!(" Results ({}) ", self.hits.len()))
                    .style(theme::title()),
            );

        let inner = block.inner(area);
        f.render_widget(block, area);

        let visible_height = inner.height as usize;
        let mut offset = self.scroll_offset;
        if self.cursor >= offset + visible_height {
            offset = self.cursor.saturating_sub(visible_height - 1);
        }
        if self.cursor < offset {
            offset = self.cursor;
        }
        let start = offset;
        let end = (start + visible_height).min(self.hits.len());

        let items: Vec<ListItem> = self.hits[start..end]
            .iter()
            .enumerate()
            .map(|(vi, h)| {
                let idx = start + vi;
                let is_selected = idx == self.cursor;
                let base = if is_selected {
                    theme::selected_row()
                } else {
                    Style::default()
                };

                let bar = theme::score_bar(h.score, 8);
                let bar_color = theme::score_color(h.score);

                ListItem::new(Line::from(vec![
                    Span::styled(&h.name, base.add_modifier(Modifier::BOLD)),
                    Span::raw(" "),
                    Span::styled(
                        format!("({})", h.entity_type),
                        base.fg(theme::ENTITY_TYPE),
                    ),
                    Span::raw("  "),
                    Span::styled(bar, base.fg(bar_color)),
                    Span::styled(format!(" {:.4}", h.score), base.fg(theme::MUTED)),
                ]))
            })
            .collect();

        f.render_widget(List::new(items), inner);
    }

    fn render_detail(&self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_default())
            .title(Line::from(" Detail ").style(theme::title()));

        let inner = block.inner(area);
        f.render_widget(block, area);

        if let Some(hit) = self.hits.get(self.cursor) {
            let lines = vec![
                Line::from(vec![
                    Span::styled("Name: ", Style::default().fg(theme::LABEL)),
                    Span::styled(&hit.name, Style::default().add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("Type: ", Style::default().fg(theme::LABEL)),
                    Span::styled(&hit.entity_type, Style::default().fg(theme::ENTITY_TYPE)),
                ]),
                Line::from(vec![
                    Span::styled("Score: ", Style::default().fg(theme::LABEL)),
                    Span::styled(
                        format!("{:.6}", hit.score),
                        Style::default().fg(theme::score_color(hit.score)),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled("Summary:", Style::default().fg(theme::LABEL))),
                Line::from(""),
            ];

            let mut all_lines = lines;
            for line in textwrap::wrap(&hit.summary, inner.width.saturating_sub(1) as usize) {
                all_lines.push(Line::from(line.to_string()));
            }

            f.render_widget(
                Paragraph::new(all_lines).wrap(Wrap { trim: false }),
                inner,
            );
        }
    }

    pub fn hints(&self) -> Vec<(&'static str, &'static str)> {
        match self.focus {
            Focus::Input => vec![
                ("Enter", "search"),
                ("Esc/Down", "results"),
                ("Tab", "next screen"),
            ],
            Focus::Results => vec![
                ("j/k", "navigate"),
                ("Enter", "view entity"),
                ("x", "expand"),
                ("/", "edit query"),
                ("Tab", "next screen"),
            ],
        }
    }
}
