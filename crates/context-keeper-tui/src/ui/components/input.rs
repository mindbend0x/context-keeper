//! Reusable single-line text input with cursor position.

use crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::ui::theme;

#[derive(Debug, Clone, Default)]
pub struct TextInput {
    pub value: String,
    cursor: usize,
}

impl TextInput {
    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char(c) => {
                self.value.insert(self.cursor, c);
                self.cursor += c.len_utf8();
                true
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    let prev = self.value[..self.cursor]
                        .chars()
                        .last()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                    self.cursor -= prev;
                    self.value.remove(self.cursor);
                    true
                } else {
                    false
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.value.len() {
                    self.value.remove(self.cursor);
                    true
                } else {
                    false
                }
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    let prev = self.value[..self.cursor]
                        .chars()
                        .last()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                    self.cursor -= prev;
                }
                false
            }
            KeyCode::Right => {
                if self.cursor < self.value.len() {
                    let next = self.value[self.cursor..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                    self.cursor += next;
                }
                false
            }
            KeyCode::Home => {
                self.cursor = 0;
                false
            }
            KeyCode::End => {
                self.cursor = self.value.len();
                false
            }
            _ => false,
        }
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    pub fn set(&mut self, s: &str) {
        self.value = s.to_string();
        self.cursor = self.value.len();
    }

    pub fn render(&self, f: &mut Frame<'_>, area: Rect, title: &str, focused: bool) {
        let border_style = if focused {
            theme::border_focused()
        } else {
            theme::border_default()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Line::from(title).style(if focused {
                theme::title()
            } else {
                Style::default().fg(theme::MUTED)
            }));

        let paragraph = Paragraph::new(self.value.as_str()).block(block);
        f.render_widget(paragraph, area);

        if focused {
            let x = area.x + 1 + self.visual_cursor() as u16;
            let y = area.y + 1;
            if x < area.x + area.width - 1 {
                f.set_cursor_position((x, y));
            }
        }
    }

    fn visual_cursor(&self) -> usize {
        self.value[..self.cursor].chars().count()
    }
}
