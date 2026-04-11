//! Admin screen: sub-tabs for namespaces, agents, cross-search, snapshot, activity, notes, runs.

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, Tabs, Wrap};
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::backend::TuiBackend;
use crate::types::{
    AgentInfoRow, AgentRunRow, EpisodeRow, NamespaceInfo, NoteRow, SearchHit, SnapshotResult,
};
use crate::ui::components::input::TextInput;
use crate::ui::event::AppEvent;
use crate::ui::theme;

const SUB_TABS: &[&str] = &[
    "Namespaces",
    "Agents",
    "Cross-Search",
    "Snapshot",
    "Activity",
    "Notes",
    "Runs",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubTab {
    Namespaces,
    Agents,
    CrossSearch,
    Snapshot,
    Activity,
    Notes,
    Runs,
}

impl SubTab {
    fn index(self) -> usize {
        self as usize
    }

    fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Namespaces,
            1 => Self::Agents,
            2 => Self::CrossSearch,
            3 => Self::Snapshot,
            4 => Self::Activity,
            5 => Self::Notes,
            6 => Self::Runs,
            _ => Self::Namespaces,
        }
    }
}

pub struct AdminState {
    sub_tab: SubTab,
    pub input: TextInput,
    pub input_focused: bool,

    pub namespaces: Vec<NamespaceInfo>,
    pub agents: Vec<AgentInfoRow>,
    pub cross_hits: Vec<SearchHit>,
    pub snapshot: Option<SnapshotResult>,
    pub episodes: Vec<EpisodeRow>,
    pub notes: Vec<NoteRow>,
    pub agent_runs: Vec<AgentRunRow>,
    pub scroll_offset: usize,
    pub cursor: usize,
    pub confirm_delete: Option<String>,
}

impl Default for AdminState {
    fn default() -> Self {
        Self {
            sub_tab: SubTab::Namespaces,
            input: TextInput::default(),
            input_focused: false,
            namespaces: Vec::new(),
            agents: Vec::new(),
            cross_hits: Vec::new(),
            snapshot: None,
            episodes: Vec::new(),
            notes: Vec::new(),
            agent_runs: Vec::new(),
            scroll_offset: 0,
            cursor: 0,
            confirm_delete: None,
        }
    }
}

impl AdminState {
    pub fn apply_event(&mut self, ev: &AppEvent) {
        match ev {
            AppEvent::NamespacesReady(Ok(ns)) => self.namespaces = ns.clone(),
            AppEvent::AgentsReady(Ok(ag)) => self.agents = ag.clone(),
            AppEvent::CrossSearchReady(Ok(h)) => {
                self.cross_hits = h.clone();
                self.cursor = 0;
            }
            AppEvent::SnapshotReady(Ok(s)) => self.snapshot = Some(s.clone()),
            AppEvent::ActivityReady(Ok(ep)) => {
                self.episodes = ep.clone();
                self.cursor = 0;
            }
            AppEvent::NotesReady(Ok(n)) => {
                self.notes = n.clone();
                self.cursor = 0;
            }
            AppEvent::AgentRunsReady(Ok(r)) => {
                self.agent_runs = r.clone();
                self.cursor = 0;
            }
            AppEvent::NamespaceDeleteReady(Ok(_)) => {
                self.confirm_delete = None;
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

        if self.input_focused {
            match key.code {
                KeyCode::Enter => {
                    self.submit(tx, backend);
                    self.input_focused = false;
                }
                KeyCode::Esc => {
                    self.input_focused = false;
                }
                _ => {
                    self.input.handle_key(key.code);
                }
            }
            return;
        }

        if let Some(ref ns) = self.confirm_delete {
            match key.code {
                KeyCode::Char('y') => {
                    let ns = ns.clone();
                    let tx2 = tx.clone();
                    let b = Arc::clone(backend);
                    tokio::spawn(async move {
                        let r = b.delete_namespace(&ns).await.map_err(|e| e.to_string());
                        let _ = tx2.send(AppEvent::NamespaceDeleteReady(r));
                        let r2 = b.list_namespaces().await.map_err(|e| e.to_string());
                        let _ = tx2.send(AppEvent::NamespacesReady(r2));
                    });
                    self.confirm_delete = None;
                }
                KeyCode::Esc | KeyCode::Char('n') => {
                    self.confirm_delete = None;
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                let i = self.sub_tab.index();
                self.sub_tab = SubTab::from_index(if i == 0 { SUB_TABS.len() - 1 } else { i - 1 });
                self.scroll_offset = 0;
                self.cursor = 0;
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.sub_tab = SubTab::from_index((self.sub_tab.index() + 1) % SUB_TABS.len());
                self.scroll_offset = 0;
                self.cursor = 0;
            }
            KeyCode::Char('r') => self.load_current(tx, backend),
            KeyCode::Char('/') | KeyCode::Char('i') => {
                if self.current_needs_input() {
                    self.input_focused = true;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let len = self.current_list_len();
                if len > 0 && self.cursor < len - 1 {
                    self.cursor += 1;
                }
            }
            KeyCode::Char('d') => {
                if self.sub_tab == SubTab::Namespaces && !self.namespaces.is_empty() {
                    self.confirm_delete = Some(self.namespaces[self.cursor].name.clone());
                }
            }
            _ => {}
        }
    }

    fn current_needs_input(&self) -> bool {
        matches!(
            self.sub_tab,
            SubTab::CrossSearch | SubTab::Snapshot | SubTab::Activity
        )
    }

    fn current_list_len(&self) -> usize {
        match self.sub_tab {
            SubTab::Namespaces => self.namespaces.len(),
            SubTab::Agents => self.agents.len(),
            SubTab::CrossSearch => self.cross_hits.len(),
            SubTab::Snapshot => self.snapshot.as_ref().map_or(0, |s| s.entities.len()),
            SubTab::Activity => self.episodes.len(),
            SubTab::Notes => self.notes.len(),
            SubTab::Runs => self.agent_runs.len(),
        }
    }

    fn load_current(&self, tx: &mpsc::UnboundedSender<AppEvent>, backend: &Arc<dyn TuiBackend>) {
        match self.sub_tab {
            SubTab::Namespaces => {
                let tx = tx.clone();
                let b = Arc::clone(backend);
                tokio::spawn(async move {
                    let r = b.list_namespaces().await.map_err(|e| e.to_string());
                    let _ = tx.send(AppEvent::NamespacesReady(r));
                });
            }
            SubTab::Agents => {
                let tx = tx.clone();
                let b = Arc::clone(backend);
                tokio::spawn(async move {
                    let r = b.list_agents().await.map_err(|e| e.to_string());
                    let _ = tx.send(AppEvent::AgentsReady(r));
                });
            }
            SubTab::Notes => {
                let tx = tx.clone();
                let b = Arc::clone(backend);
                tokio::spawn(async move {
                    let r = b.list_notes(None, 50).await.map_err(|e| e.to_string());
                    let _ = tx.send(AppEvent::NotesReady(r));
                });
            }
            SubTab::Runs => {
                let tx = tx.clone();
                let b = Arc::clone(backend);
                tokio::spawn(async move {
                    let r = b
                        .query_agent_runs(None, None, 50)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = tx.send(AppEvent::AgentRunsReady(r));
                });
            }
            _ => {}
        }
    }

    fn submit(&self, tx: &mpsc::UnboundedSender<AppEvent>, backend: &Arc<dyn TuiBackend>) {
        let val = self.input.value.clone();
        if val.is_empty() {
            return;
        }
        match self.sub_tab {
            SubTab::CrossSearch => {
                let tx = tx.clone();
                let b = Arc::clone(backend);
                tokio::spawn(async move {
                    let r = b
                        .cross_namespace_search(&val, 20)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = tx.send(AppEvent::CrossSearchReady(r));
                });
            }
            SubTab::Snapshot => {
                let tx = tx.clone();
                let b = Arc::clone(backend);
                tokio::spawn(async move {
                    let r = b.snapshot(&val).await.map_err(|e| e.to_string());
                    let _ = tx.send(AppEvent::SnapshotReady(r));
                });
            }
            SubTab::Activity => {
                let tx = tx.clone();
                let b = Arc::clone(backend);
                tokio::spawn(async move {
                    let r = b.agent_activity(&val, 30).await.map_err(|e| e.to_string());
                    let _ = tx.send(AppEvent::ActivityReady(r));
                });
            }
            _ => {}
        }
    }

    pub fn render(&self, f: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(5)])
            .split(area);

        self.render_sub_tabs(f, chunks[0]);
        self.render_panel(f, chunks[1]);
    }

    fn render_sub_tabs(&self, f: &mut Frame<'_>, area: Rect) {
        let titles: Vec<Line> = SUB_TABS
            .iter()
            .enumerate()
            .map(|(i, t)| {
                if i == self.sub_tab.index() {
                    Line::from(Span::styled(format!(" {t} "), theme::tab_active()))
                } else {
                    Line::from(Span::styled(format!(" {t} "), theme::tab_inactive()))
                }
            })
            .collect();

        let tabs = Tabs::new(titles)
            .select(self.sub_tab.index())
            .highlight_style(theme::tab_active().add_modifier(Modifier::UNDERLINED));
        f.render_widget(tabs, area);
    }

    fn render_panel(&self, f: &mut Frame<'_>, area: Rect) {
        match self.sub_tab {
            SubTab::Namespaces => self.render_namespaces(f, area),
            SubTab::Agents => self.render_agents(f, area),
            SubTab::CrossSearch => self.render_cross_search(f, area),
            SubTab::Snapshot => self.render_snapshot(f, area),
            SubTab::Activity => self.render_activity(f, area),
            SubTab::Notes => self.render_notes(f, area),
            SubTab::Runs => self.render_runs(f, area),
        }
    }

    fn render_namespaces(&self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_default())
            .title(
                Line::from(format!(" Namespaces ({}) ", self.namespaces.len()))
                    .style(theme::title()),
            );

        if self.namespaces.is_empty() {
            f.render_widget(
                Paragraph::new("  Press 'r' to load namespaces")
                    .style(Style::default().fg(theme::MUTED))
                    .block(block),
                area,
            );
            return;
        }

        let body_area = if let Some(ref ns) = self.confirm_delete {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(3)])
                .split(area);
            let msg = format!(" Delete namespace '{ns}'? Press 'y' to confirm, Esc to cancel ");
            f.render_widget(
                Paragraph::new(msg).style(Style::default().fg(theme::ERROR)),
                chunks[0],
            );
            chunks[1]
        } else {
            area
        };

        let header = Row::new(vec![
            Cell::from("Namespace").style(
                Style::default()
                    .fg(theme::LABEL)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("Entities").style(
                Style::default()
                    .fg(theme::LABEL)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let rows: Vec<Row> = self
            .namespaces
            .iter()
            .map(|ns| {
                Row::new(vec![
                    Cell::from(ns.name.as_str()),
                    Cell::from(ns.entity_count.to_string())
                        .style(Style::default().fg(theme::ACCENT)),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [Constraint::Percentage(60), Constraint::Percentage(40)],
        )
        .header(header)
        .block(block);

        f.render_widget(table, body_area);
    }

    fn render_agents(&self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_default())
            .title(Line::from(format!(" Agents ({}) ", self.agents.len())).style(theme::title()));

        if self.agents.is_empty() {
            f.render_widget(
                Paragraph::new("  Press 'r' to load agents")
                    .style(Style::default().fg(theme::MUTED))
                    .block(block),
                area,
            );
            return;
        }

        let header = Row::new(vec![
            Cell::from("Agent ID").style(
                Style::default()
                    .fg(theme::LABEL)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("Name").style(
                Style::default()
                    .fg(theme::LABEL)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("Episodes").style(
                Style::default()
                    .fg(theme::LABEL)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let rows: Vec<Row> = self
            .agents
            .iter()
            .map(|a| {
                Row::new(vec![
                    Cell::from(a.agent_id.as_str()),
                    Cell::from(a.agent_name.as_deref().unwrap_or("-")),
                    Cell::from(a.episode_count.to_string())
                        .style(Style::default().fg(theme::ACCENT)),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(40),
                Constraint::Percentage(30),
                Constraint::Percentage(30),
            ],
        )
        .header(header)
        .block(block);

        f.render_widget(table, area);
    }

    fn render_cross_search(&self, f: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(area);

        self.input.render(
            f,
            chunks[0],
            " Cross-namespace query (Enter to search) ",
            self.input_focused,
        );

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_default())
            .title(
                Line::from(format!(" Results ({}) ", self.cross_hits.len())).style(theme::title()),
            );

        if self.cross_hits.is_empty() {
            f.render_widget(
                Paragraph::new("  Enter a query and press Enter")
                    .style(Style::default().fg(theme::MUTED))
                    .block(block),
                chunks[1],
            );
            return;
        }

        let inner = block.inner(chunks[1]);
        f.render_widget(block, chunks[1]);

        let items: Vec<ListItem> = self
            .cross_hits
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let base = if i == self.cursor {
                    theme::selected_row()
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(&h.name, base.add_modifier(Modifier::BOLD)),
                    Span::raw(" "),
                    Span::styled(format!("({})", h.entity_type), base.fg(theme::ENTITY_TYPE)),
                    Span::styled(format!("  {:.4}", h.score), base.fg(theme::MUTED)),
                ]))
            })
            .collect();

        f.render_widget(List::new(items), inner);
    }

    fn render_snapshot(&self, f: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(area);

        self.input.render(
            f,
            chunks[0],
            " ISO timestamp (Enter to snapshot) ",
            self.input_focused,
        );

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_default())
            .title(Line::from(" Snapshot ").style(theme::title()));

        let snap = match &self.snapshot {
            Some(s) => s,
            None => {
                f.render_widget(
                    Paragraph::new(
                        "  Enter an ISO 8601 timestamp to view a point-in-time snapshot",
                    )
                    .style(Style::default().fg(theme::MUTED))
                    .block(block),
                    chunks[1],
                );
                return;
            }
        };

        let inner = block.inner(chunks[1]);
        f.render_widget(block, chunks[1]);

        let mut lines = vec![
            Line::from(vec![
                Span::styled("At: ", Style::default().fg(theme::LABEL)),
                Span::styled(&snap.timestamp, Style::default().fg(theme::TIMESTAMP)),
                Span::styled(
                    format!(
                        "  ({} entities, {} relations)",
                        snap.entity_count, snap.relation_count
                    ),
                    Style::default().fg(theme::MUTED),
                ),
            ]),
            Line::from(""),
        ];

        for e in &snap.entities {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(&e.name, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(
                    format!("({})", e.entity_type),
                    Style::default().fg(theme::ENTITY_TYPE),
                ),
                Span::styled(
                    format!(" - {}", truncate(&e.summary, 60)),
                    Style::default().fg(theme::MUTED),
                ),
            ]));
        }

        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    fn render_activity(&self, f: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(3)])
            .split(area);

        self.input.render(
            f,
            chunks[0],
            " Agent ID (Enter to load activity) ",
            self.input_focused,
        );

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_default())
            .title(
                Line::from(format!(" Activity ({}) ", self.episodes.len())).style(theme::title()),
            );

        if self.episodes.is_empty() {
            f.render_widget(
                Paragraph::new("  Enter an agent ID and press Enter")
                    .style(Style::default().fg(theme::MUTED))
                    .block(block),
                chunks[1],
            );
            return;
        }

        let inner = block.inner(chunks[1]);
        f.render_widget(block, chunks[1]);

        let items: Vec<ListItem> = self
            .episodes
            .iter()
            .enumerate()
            .map(|(i, ep)| {
                let base = if i == self.cursor {
                    theme::selected_row()
                } else {
                    Style::default()
                };
                let ts = if ep.created_at.len() > 19 {
                    &ep.created_at[..19]
                } else {
                    &ep.created_at
                };
                let ns = ep.namespace.as_deref().unwrap_or("-");
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{ts}  "), base.fg(theme::TIMESTAMP)),
                    Span::styled(format!("[{ns}] "), base.fg(theme::ENTITY_TYPE)),
                    Span::styled(truncate(&ep.content, 80), base),
                ]))
            })
            .collect();

        f.render_widget(List::new(items), inner);
    }

    fn render_notes(&self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_default())
            .title(
                Line::from(format!(" Notes ({}) ", self.notes.len())).style(theme::title()),
            );

        if self.notes.is_empty() {
            f.render_widget(
                Paragraph::new("  Press 'r' to load notes")
                    .style(Style::default().fg(theme::MUTED))
                    .block(block),
                area,
            );
            return;
        }

        let header = Row::new(vec![
            Cell::from("Key").style(
                Style::default()
                    .fg(theme::LABEL)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("Tags").style(
                Style::default()
                    .fg(theme::LABEL)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("Content").style(
                Style::default()
                    .fg(theme::LABEL)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("Updated").style(
                Style::default()
                    .fg(theme::LABEL)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let rows: Vec<Row> = self
            .notes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let base = if i == self.cursor {
                    theme::selected_row()
                } else {
                    Style::default()
                };
                let ts = if n.updated_at.len() > 19 {
                    &n.updated_at[..19]
                } else {
                    &n.updated_at
                };
                Row::new(vec![
                    Cell::from(n.key.as_str()).style(base.add_modifier(Modifier::BOLD)),
                    Cell::from(n.tags.join(", ")).style(base.fg(theme::ENTITY_TYPE)),
                    Cell::from(truncate(&n.content, 50)).style(base),
                    Cell::from(ts).style(base.fg(theme::TIMESTAMP)),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(20),
                Constraint::Percentage(15),
                Constraint::Percentage(45),
                Constraint::Percentage(20),
            ],
        )
        .header(header)
        .block(block);

        f.render_widget(table, area);
    }

    fn render_runs(&self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_default())
            .title(
                Line::from(format!(" Agent Runs ({}) ", self.agent_runs.len()))
                    .style(theme::title()),
            );

        if self.agent_runs.is_empty() {
            f.render_widget(
                Paragraph::new("  Press 'r' to load agent runs")
                    .style(Style::default().fg(theme::MUTED))
                    .block(block),
                area,
            );
            return;
        }

        let header = Row::new(vec![
            Cell::from("Agent").style(
                Style::default()
                    .fg(theme::LABEL)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("Status").style(
                Style::default()
                    .fg(theme::LABEL)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("Summary").style(
                Style::default()
                    .fg(theme::LABEL)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("Time").style(
                Style::default()
                    .fg(theme::LABEL)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let rows: Vec<Row> = self
            .agent_runs
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let base = if i == self.cursor {
                    theme::selected_row()
                } else {
                    Style::default()
                };
                let ts = if r.created_at.len() > 19 {
                    &r.created_at[..19]
                } else {
                    &r.created_at
                };
                let status_style = match r.status.as_str() {
                    "completed" => base.fg(theme::ACCENT),
                    "failed" => base.fg(theme::ERROR),
                    "blocked" => base.fg(theme::SCORE_MID),
                    _ => base.fg(theme::ENTITY_TYPE),
                };
                Row::new(vec![
                    Cell::from(r.agent_id.as_deref().unwrap_or("-")).style(base),
                    Cell::from(r.status.as_str()).style(status_style),
                    Cell::from(truncate(
                        r.summary.as_deref().unwrap_or("-"),
                        50,
                    ))
                    .style(base),
                    Cell::from(ts).style(base.fg(theme::TIMESTAMP)),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(20),
                Constraint::Percentage(12),
                Constraint::Percentage(48),
                Constraint::Percentage(20),
            ],
        )
        .header(header)
        .block(block);

        f.render_widget(table, area);
    }

    pub fn hints(&self) -> Vec<(&'static str, &'static str)> {
        if self.input_focused {
            return vec![("Enter", "submit"), ("Esc", "back")];
        }
        if self.sub_tab == SubTab::Namespaces && self.confirm_delete.is_some() {
            return vec![("y", "confirm delete"), ("Esc", "cancel")];
        }
        let mut h = vec![("h/l", "sub-tab")];
        if self.current_needs_input() {
            h.push(("/", "input"));
        }
        match self.sub_tab {
            SubTab::Namespaces | SubTab::Agents | SubTab::Notes | SubTab::Runs => {
                h.push(("r", "reload"));
            }
            _ => {}
        }
        if self.sub_tab == SubTab::Namespaces && !self.namespaces.is_empty() {
            h.push(("d", "delete"));
        }
        h.push(("j/k", "navigate"));
        h.push(("Tab", "next screen"));
        h
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}
