//! Ingest screen: text input for adding memories, structured result display.

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::backend::TuiBackend;
use crate::ui::event::AppEvent;
use crate::ui::theme;

#[derive(Default)]
pub struct IngestState {
    pub text: String,
    pub cursor: usize,
    pub status: Option<String>,
}

impl IngestState {
    pub fn apply_event(&mut self, ev: &AppEvent) {
        if let AppEvent::IngestReady(Ok(msg)) = ev {
            self.status = Some(msg.clone());
            self.text.clear();
            self.cursor = 0;
        } else if let AppEvent::IngestReady(Err(e)) = ev {
            self.status = Some(format!("Error: {e}"));
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
            KeyCode::Enter
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::ALT)
                    || key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.text.insert(self.cursor, '\n');
                self.cursor += 1;
            }
            KeyCode::Enter => {
                if self.text.is_empty() {
                    return;
                }
                self.submit(tx, backend);
            }
            KeyCode::Char(c) => {
                self.text.insert(self.cursor, c);
                self.cursor += c.len_utf8();
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    let prev = self.text[..self.cursor]
                        .chars()
                        .last()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                    self.cursor -= prev;
                    self.text.remove(self.cursor);
                }
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    let prev = self.text[..self.cursor]
                        .chars()
                        .last()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                    self.cursor -= prev;
                }
            }
            KeyCode::Right => {
                if self.cursor < self.text.len() {
                    let next = self.text[self.cursor..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                    self.cursor += next;
                }
            }
            _ => {}
        }
    }

    fn submit(&mut self, tx: &mpsc::UnboundedSender<AppEvent>, backend: &Arc<dyn TuiBackend>) {
        let text = self.text.clone();
        let tx = tx.clone();
        let b = Arc::clone(backend);
        tokio::spawn(async move {
            let r = b.add_memory(&text, "tui").await;
            let msg = match r {
                Ok(o) => {
                    let names = if o.entity_names.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", o.entity_names.join(", "))
                    };
                    Ok(format!(
                        "Ingested: {} entities, {} relations, {} memories{}",
                        o.entity_count, o.relation_count, o.memory_count, names
                    ))
                }
                Err(e) => Err(e.to_string()),
            };
            let _ = tx.send(AppEvent::IngestReady(msg));
        });
        self.status = Some("Ingesting...".to_string());
    }

    pub fn render(&self, f: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(8), Constraint::Length(5)])
            .split(area);

        self.render_input(f, chunks[0]);
        self.render_result(f, chunks[1]);
    }

    fn render_input(&self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_focused())
            .title(
                Line::from(" Add Memory (Enter to ingest, Ctrl+Enter for newline) ")
                    .style(theme::title()),
            );

        let inner = block.inner(area);
        f.render_widget(block, area);

        let paragraph = Paragraph::new(self.text.as_str()).wrap(Wrap { trim: false });
        f.render_widget(paragraph, inner);

        let visual_pos = self.text[..self.cursor].chars().count();
        let width = inner.width as usize;
        if width > 0 {
            let row = visual_pos / width;
            let col = visual_pos % width;
            let x = inner.x + col as u16;
            let y = inner.y + row as u16;
            if y < inner.y + inner.height {
                f.set_cursor_position((x, y));
            }
        }
    }

    fn render_result(&self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_default())
            .title(Line::from(" Result ").style(theme::title()));

        let inner = block.inner(area);
        f.render_widget(block, area);

        if let Some(status) = &self.status {
            let color = if status.starts_with("Error") {
                theme::ERROR
            } else if status.starts_with("Ingested") {
                theme::SUCCESS
            } else {
                theme::MUTED
            };
            f.render_widget(
                Paragraph::new(format!("  {status}"))
                    .style(Style::default().fg(color))
                    .wrap(Wrap { trim: false }),
                inner,
            );
        }
    }

    pub fn hints(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("Enter", "ingest"),
            ("C-Enter", "newline"),
            ("Tab", "next screen"),
        ]
    }
}
