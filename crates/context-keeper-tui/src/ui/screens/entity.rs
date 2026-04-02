//! Entity browser: search/browse entities, structured detail with relations.

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::backend::TuiBackend;
use crate::types::{EntityDetail, EntitySummary, RelationDirection};
use crate::ui::components::input::TextInput;
use crate::ui::event::AppEvent;
use crate::ui::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Input,
    EntityList,
    Detail,
}

pub struct EntityState {
    pub input: TextInput,
    pub entities: Vec<EntitySummary>,
    pub detail: Option<EntityDetail>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub detail_scroll: u16,
    focus: Focus,
}

impl Default for EntityState {
    fn default() -> Self {
        Self {
            input: TextInput::default(),
            entities: Vec::new(),
            detail: None,
            cursor: 0,
            scroll_offset: 0,
            detail_scroll: 0,
            focus: Focus::Input,
        }
    }
}

impl EntityState {
    pub fn apply_event(&mut self, ev: &AppEvent) {
        match ev {
            AppEvent::EntityListReady(Ok(list)) => {
                self.entities = list.clone();
                self.cursor = 0;
                self.scroll_offset = 0;
                if !self.entities.is_empty() {
                    self.focus = Focus::EntityList;
                }
            }
            AppEvent::EntityReady(Ok(detail)) => {
                self.detail = detail.clone();
                self.detail_scroll = 0;
                if detail.is_some() {
                    self.focus = Focus::Detail;
                }
            }
            _ => {}
        }
    }

    pub fn load_entity_by_name(
        &mut self,
        name: &str,
        tx: &mpsc::UnboundedSender<AppEvent>,
        backend: &Arc<dyn TuiBackend>,
    ) {
        self.input.set(name);
        let name = name.to_string();
        let tx = tx.clone();
        let b = Arc::clone(backend);
        tokio::spawn(async move {
            let r = b.get_entity(&name).await.map_err(|e| e.to_string());
            let _ = tx.send(AppEvent::EntityReady(r));
        });
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

        match self.focus {
            Focus::Input => match key.code {
                KeyCode::Enter => {
                    let name = self.input.value.clone();
                    if name.is_empty() {
                        self.browse_all(tx, backend);
                    } else {
                        self.lookup(tx, backend);
                    }
                }
                KeyCode::Down if !self.entities.is_empty() => {
                    self.focus = Focus::EntityList;
                }
                KeyCode::Esc if self.detail.is_some() => {
                    self.focus = Focus::Detail;
                }
                _ => {
                    self.input.handle_key(key.code);
                }
            },
            Focus::EntityList => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.cursor > 0 {
                        self.cursor -= 1;
                        if self.cursor < self.scroll_offset {
                            self.scroll_offset = self.cursor;
                        }
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.entities.is_empty() && self.cursor < self.entities.len() - 1 {
                        self.cursor += 1;
                    }
                }
                KeyCode::Enter => {
                    if let Some(e) = self.entities.get(self.cursor) {
                        let name = e.name.clone();
                        self.load_entity_by_name(&name, tx, backend);
                    }
                }
                KeyCode::Char('/') | KeyCode::Char('i') => {
                    self.focus = Focus::Input;
                }
                KeyCode::Esc => {
                    self.focus = Focus::Input;
                }
                _ => {}
            },
            Focus::Detail => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.detail_scroll += 1;
                }
                KeyCode::Char('/') | KeyCode::Char('i') => {
                    self.focus = Focus::Input;
                }
                KeyCode::Esc => {
                    if !self.entities.is_empty() {
                        self.focus = Focus::EntityList;
                    } else {
                        self.focus = Focus::Input;
                    }
                }
                _ => {}
            },
        }
    }

    fn lookup(&self, tx: &mpsc::UnboundedSender<AppEvent>, backend: &Arc<dyn TuiBackend>) {
        let name = self.input.value.clone();
        if name.is_empty() {
            return;
        }
        let tx = tx.clone();
        let b = Arc::clone(backend);
        tokio::spawn(async move {
            let r = b.get_entity(&name).await.map_err(|e| e.to_string());
            let _ = tx.send(AppEvent::EntityReady(r));
        });
    }

    fn browse_all(&self, tx: &mpsc::UnboundedSender<AppEvent>, backend: &Arc<dyn TuiBackend>) {
        let tx = tx.clone();
        let b = Arc::clone(backend);
        tokio::spawn(async move {
            let r = b.list_entities(200).await.map_err(|e| e.to_string());
            let _ = tx.send(AppEvent::EntityListReady(r));
        });
    }

    pub fn render(&self, f: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(5)])
            .split(area);

        self.input.render(
            f,
            chunks[0],
            " Entity name (Enter to lookup, empty Enter to browse all) ",
            self.focus == Focus::Input,
        );

        let body = chunks[1];

        if self.entities.is_empty() && self.detail.is_none() {
            let msg = Paragraph::new(
                "  Enter a name or press Enter with empty input to browse all entities.",
            )
            .style(Style::default().fg(theme::MUTED))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::border_default())
                    .title(Line::from(" Entity ").style(theme::title())),
            );
            f.render_widget(msg, body);
            return;
        }

        if !self.entities.is_empty() && self.detail.is_some() {
            let split = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
                .split(body);
            self.render_entity_list(f, split[0]);
            self.render_detail(f, split[1]);
        } else if !self.entities.is_empty() {
            self.render_entity_list(f, body);
        } else {
            self.render_detail(f, body);
        }
    }

    fn render_entity_list(&self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(if self.focus == Focus::EntityList {
                theme::border_focused()
            } else {
                theme::border_default()
            })
            .title(
                Line::from(format!(" Entities ({}) ", self.entities.len())).style(theme::title()),
            );

        let inner = block.inner(area);
        f.render_widget(block, area);

        let visible = inner.height as usize;
        let mut offset = self.scroll_offset;
        if self.cursor >= offset + visible {
            offset = self.cursor.saturating_sub(visible - 1);
        }
        if self.cursor < offset {
            offset = self.cursor;
        }
        let start = offset;
        let end = (start + visible).min(self.entities.len());

        let items: Vec<ListItem> = self.entities[start..end]
            .iter()
            .enumerate()
            .map(|(vi, e)| {
                let idx = start + vi;
                let is_selected = idx == self.cursor;
                let base = if is_selected {
                    theme::selected_row()
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(&e.name, base.add_modifier(Modifier::BOLD)),
                    Span::raw(" "),
                    Span::styled(format!("({})", e.entity_type), base.fg(theme::ENTITY_TYPE)),
                ]))
            })
            .collect();

        f.render_widget(List::new(items), inner);
    }

    fn render_detail(&self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(if self.focus == Focus::Detail {
                theme::border_focused()
            } else {
                theme::border_default()
            })
            .title(Line::from(" Detail ").style(theme::title()));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let detail = match &self.detail {
            Some(d) => d,
            None => {
                f.render_widget(
                    Paragraph::new("  Select an entity to view details.")
                        .style(Style::default().fg(theme::MUTED)),
                    inner,
                );
                return;
            }
        };

        let valid_until = detail.valid_until.as_deref().unwrap_or("present");

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Name:  ", Style::default().fg(theme::LABEL)),
                Span::styled(&detail.name, Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("Type:  ", Style::default().fg(theme::LABEL)),
                Span::styled(&detail.entity_type, Style::default().fg(theme::ENTITY_TYPE)),
            ]),
            Line::from(vec![
                Span::styled("Valid: ", Style::default().fg(theme::LABEL)),
                Span::styled(
                    format!(
                        "{} .. {}",
                        &detail.valid_from[..detail.valid_from.len().min(19)],
                        &valid_until[..valid_until.len().min(19)]
                    ),
                    Style::default().fg(theme::TIMESTAMP),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled("Summary:", Style::default().fg(theme::LABEL))),
        ];

        let wrap_width = inner.width.saturating_sub(1) as usize;
        for line in textwrap::wrap(&detail.summary, wrap_width.max(20)) {
            lines.push(Line::from(format!("  {line}")));
        }

        if !detail.relations.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("Relations ({}):", detail.relations.len()),
                Style::default().fg(theme::LABEL),
            )));
            lines.push(Line::from(""));

            for rel in &detail.relations {
                let arrow = match rel.direction {
                    RelationDirection::Outgoing => "->",
                    RelationDirection::Incoming => "<-",
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", arrow), Style::default().fg(theme::MUTED)),
                    Span::styled(
                        &rel.relation_type,
                        Style::default().fg(theme::RELATION_TYPE),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        &rel.target_name,
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  (conf: {})", rel.confidence),
                        Style::default().fg(theme::MUTED),
                    ),
                ]));
            }
        }

        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((self.detail_scroll, 0));
        f.render_widget(paragraph, inner);
    }

    pub fn hints(&self) -> Vec<(&'static str, &'static str)> {
        match self.focus {
            Focus::Input => vec![("Enter", "lookup / browse"), ("Tab", "next screen")],
            Focus::EntityList => vec![
                ("j/k", "navigate"),
                ("Enter", "view detail"),
                ("/", "edit query"),
                ("Esc", "back to input"),
            ],
            Focus::Detail => vec![
                ("j/k", "scroll"),
                ("/", "new lookup"),
                ("Esc", "entity list"),
            ],
        }
    }
}
