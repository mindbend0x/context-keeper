//! Consistent color palette and style helpers for the TUI.

use ratatui::style::{Color, Modifier, Style};

pub const ACCENT: Color = Color::Cyan;
pub const ACCENT_DIM: Color = Color::DarkGray;
pub const ENTITY_TYPE: Color = Color::Green;
pub const TIMESTAMP: Color = Color::Yellow;
pub const RELATION_TYPE: Color = Color::Magenta;
pub const SCORE_HIGH: Color = Color::Green;
pub const SCORE_MID: Color = Color::Yellow;
pub const SCORE_LOW: Color = Color::Red;
pub const MUTED: Color = Color::DarkGray;
pub const ERROR: Color = Color::Red;
pub const SUCCESS: Color = Color::Green;
pub const LABEL: Color = Color::Cyan;
pub const DIM_FG: Color = Color::DarkGray;

pub fn border_default() -> Style {
    Style::default().fg(ACCENT_DIM)
}

pub fn border_focused() -> Style {
    Style::default().fg(ACCENT)
}

pub fn tab_active() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn tab_inactive() -> Style {
    Style::default().fg(MUTED)
}

pub fn title() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn selected_row() -> Style {
    Style::default()
        .bg(Color::Rgb(30, 40, 55))
        .add_modifier(Modifier::BOLD)
}

pub fn stat_value() -> Style {
    Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

pub fn stat_label() -> Style {
    Style::default().fg(MUTED)
}

pub fn score_color(score: f64) -> Color {
    if score >= 0.02 {
        SCORE_HIGH
    } else if score >= 0.01 {
        SCORE_MID
    } else {
        SCORE_LOW
    }
}

pub fn score_bar(score: f64, width: usize) -> String {
    let max_score = 0.033; // typical max RRF score
    let ratio = (score / max_score).clamp(0.0, 1.0);
    let filled = (ratio * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}
