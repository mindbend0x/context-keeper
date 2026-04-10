//! Ratatui event loop and screen state.

use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;
use ratatui::Terminal;
use tokio::sync::mpsc;

use crate::backend::TuiBackend;
use crate::ui::components::{status_bar, tabs};
use crate::ui::event::AppEvent;
use crate::ui::screens::{admin, dashboard, entity, ingest, search};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScreenId {
    Dashboard,
    Search,
    Entity,
    Ingest,
    Admin,
}

impl ScreenId {
    fn index(self) -> usize {
        match self {
            Self::Dashboard => 0,
            Self::Search => 1,
            Self::Entity => 2,
            Self::Ingest => 3,
            Self::Admin => 4,
        }
    }

    fn from_index(i: usize, admin_enabled: bool) -> Self {
        match i {
            0 => Self::Dashboard,
            1 => Self::Search,
            2 => Self::Entity,
            3 => Self::Ingest,
            4 if admin_enabled => Self::Admin,
            _ => Self::Dashboard,
        }
    }

    fn count(admin_enabled: bool) -> usize {
        if admin_enabled {
            5
        } else {
            4
        }
    }
}

struct App {
    screen: ScreenId,
    admin_enabled: bool,
    dashboard: dashboard::DashboardState,
    search: search::SearchState,
    entity: entity::EntityState,
    ingest: ingest::IngestState,
    admin: admin::AdminState,
    status: String,
    quit: bool,
}

impl App {
    fn new(admin_enabled: bool) -> Self {
        Self {
            screen: ScreenId::Dashboard,
            admin_enabled,
            dashboard: dashboard::DashboardState::default(),
            search: search::SearchState::default(),
            entity: entity::EntityState::default(),
            ingest: ingest::IngestState::default(),
            admin: admin::AdminState::default(),
            status: String::new(),
            quit: false,
        }
    }

    fn tab_titles(&self) -> Vec<&'static str> {
        let mut v = vec!["Dashboard", "Search", "Entity", "Ingest"];
        if self.admin_enabled {
            v.push("Admin");
        }
        v
    }

    fn apply_event(&mut self, ev: AppEvent) {
        match &ev {
            AppEvent::Quit => {
                self.quit = true;
                return;
            }
            AppEvent::StatsReady(Err(e))
            | AppEvent::RecentReady(Err(e))
            | AppEvent::SearchReady(Err(e))
            | AppEvent::ExpandReady(Err(e))
            | AppEvent::EntityReady(Err(e))
            | AppEvent::EntityListReady(Err(e))
            | AppEvent::IngestReady(Err(e))
            | AppEvent::NamespacesReady(Err(e))
            | AppEvent::AgentsReady(Err(e))
            | AppEvent::CrossSearchReady(Err(e))
            | AppEvent::SnapshotReady(Err(e))
            | AppEvent::ActivityReady(Err(e))
            | AppEvent::NotesReady(Err(e))
            | AppEvent::AgentRunsReady(Err(e)) => {
                self.status = format!("Error: {e}");
            }
            AppEvent::StatsReady(Ok(_)) | AppEvent::RecentReady(Ok(_)) => {
                self.status = String::new();
            }
            AppEvent::SearchReady(Ok(h)) => {
                self.status = format!("{} results", h.len());
            }
            AppEvent::ExpandReady(Ok(h)) => {
                self.status = format!("Expanded: {} results", h.len());
            }
            AppEvent::EntityReady(Ok(Some(d))) => {
                self.status = format!("Entity: {}", d.name);
            }
            AppEvent::EntityReady(Ok(None)) => {
                self.status = "Entity not found".to_string();
            }
            AppEvent::EntityListReady(Ok(l)) => {
                self.status = format!("{} entities", l.len());
            }
            AppEvent::IngestReady(Ok(msg)) => {
                self.status = msg.clone();
            }
            _ => {}
        }

        self.dashboard.apply_event(&ev);
        self.search.apply_event(&ev);
        self.entity.apply_event(&ev);
        self.ingest.apply_event(&ev);
        self.admin.apply_event(&ev);
    }

    fn on_key(
        &mut self,
        key: event::KeyEvent,
        tx: &mpsc::UnboundedSender<AppEvent>,
        backend: &Arc<dyn TuiBackend>,
    ) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        // Global: Ctrl+C always quits
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }

        // Global: Tab / BackTab cycle screens (unless consumed by ingest text or admin input)
        if key.code == KeyCode::Tab
            && self.screen != ScreenId::Ingest
            && (self.screen != ScreenId::Admin || !self.admin.input_focused)
        {
            let n = ScreenId::count(self.admin_enabled);
            self.screen = ScreenId::from_index((self.screen.index() + 1) % n, self.admin_enabled);
            return;
        }
        if key.code == KeyCode::BackTab
            && self.screen != ScreenId::Ingest
            && (self.screen != ScreenId::Admin || !self.admin.input_focused)
        {
            let n = ScreenId::count(self.admin_enabled);
            self.screen =
                ScreenId::from_index((self.screen.index() + n - 1) % n, self.admin_enabled);
            return;
        }

        // Global: q quits from Dashboard, or from screens where input is empty
        if key.code == KeyCode::Char('q') && self.screen == ScreenId::Dashboard {
            self.quit = true;
            return;
        }

        // Dispatch to active screen
        match self.screen {
            ScreenId::Dashboard => self.dashboard.handle_key(key, tx, backend),
            ScreenId::Search => {
                let action = self.search.handle_key(key, tx, backend);
                if let search::SearchAction::DrillIntoEntity(name) = action {
                    self.screen = ScreenId::Entity;
                    self.entity.load_entity_by_name(&name, tx, backend);
                }
            }
            ScreenId::Entity => self.entity.handle_key(key, tx, backend),
            ScreenId::Ingest => {
                if key.code == KeyCode::Tab {
                    let n = ScreenId::count(self.admin_enabled);
                    self.screen =
                        ScreenId::from_index((self.screen.index() + 1) % n, self.admin_enabled);
                } else {
                    self.ingest.handle_key(key, tx, backend);
                }
            }
            ScreenId::Admin => self.admin.handle_key(key, tx, backend),
        }
    }

    fn render(&self, f: &mut Frame<'_>) {
        let area = f.area();

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // tab bar
                Constraint::Min(8),    // main content
                Constraint::Length(1), // status bar
            ])
            .split(area);

        tabs::render(f, layout[0], &self.tab_titles(), self.screen.index());

        match self.screen {
            ScreenId::Dashboard => self.dashboard.render(f, layout[1]),
            ScreenId::Search => self.search.render(f, layout[1]),
            ScreenId::Entity => self.entity.render(f, layout[1]),
            ScreenId::Ingest => self.ingest.render(f, layout[1]),
            ScreenId::Admin => self.admin.render(f, layout[1]),
        }

        let hints = match self.screen {
            ScreenId::Dashboard => self.dashboard.hints(),
            ScreenId::Search => self.search.hints(),
            ScreenId::Entity => self.entity.hints(),
            ScreenId::Ingest => self.ingest.hints(),
            ScreenId::Admin => self.admin.hints(),
        };
        status_bar::render(f, layout[2], &self.status, &hints);
    }
}

/// Run the interactive TUI until the user quits.
pub async fn run_tui(backend: Arc<dyn TuiBackend>, admin_enabled: bool) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(ratatui::backend::CrosstermBackend::new(stdout))?;

    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();
    let mut app = App::new(admin_enabled);

    // Initial data load
    app.dashboard.refresh(&tx, &backend);

    loop {
        terminal.draw(|f| app.render(f))?;

        // Drain pending events from backend tasks
        while let Ok(ev) = rx.try_recv() {
            app.apply_event(ev);
        }

        if app.quit {
            break;
        }

        let key_opt: Option<event::KeyEvent> = tokio::task::spawn_blocking(|| {
            if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(Event::Key(k)) = event::read() {
                    return Some(k);
                }
            }
            None
        })
        .await
        .unwrap_or(None);

        if let Some(k) = key_opt {
            app.on_key(k, &tx, &backend);
        }

        if app.quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
