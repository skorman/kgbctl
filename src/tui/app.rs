use std::collections::HashSet;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::widgets::TableState;

use super::splash::Splash;
use crate::db::{Broker, Db};
use crate::portfolio::{BROKERS, Snapshot};
use crate::sync::SyncReport;

/// Reload from the database this often, so syncs run outside the TUI show up.
const AUTO_RELOAD: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Overview,
    Detail,
}

/// What a key press asks the event loop to do beyond updating state.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Reload,
    Sync,
}

pub struct App {
    pub snapshot: Snapshot,
    pub view: View,
    pub tab: usize,
    pub tables: Vec<TableState>,
    pub syncing: HashSet<Broker>,
    pub frame: usize,
    pub error: Option<String>,
    pub should_quit: bool,
    /// Startup animation; shown instead of the dashboard until it finishes.
    pub splash: Option<Splash>,
    /// Host that `s` syncs into, when it isn't the local database.
    pub remote: Option<String>,
    /// Set after a sync into `remote`: the dashboard still shows local data.
    pub synced_remotely: bool,
    loaded_at: Instant,
}

impl App {
    pub fn new(remote: Option<String>) -> Self {
        Self {
            snapshot: Snapshot::default(),
            view: View::Overview,
            tab: 0,
            tables: Vec::new(),
            syncing: HashSet::new(),
            frame: 0,
            error: None,
            should_quit: false,
            splash: None,
            remote,
            synced_remotely: false,
            loaded_at: Instant::now(),
        }
    }

    pub async fn reload(&mut self, db: &Db) {
        match Snapshot::load(db).await {
            Ok(snapshot) => self.set_snapshot(snapshot),
            Err(e) => self.error = Some(format!("Database: {e}")),
        }
        self.loaded_at = Instant::now();
    }

    fn set_snapshot(&mut self, snapshot: Snapshot) {
        self.tables
            .resize_with(snapshot.tabs.len(), TableState::default);
        for (table, tab) in self.tables.iter_mut().zip(&snapshot.tabs) {
            let n = tab.holdings.len();
            let selected = table.selected().map(|i| i.min(n.saturating_sub(1)));
            table.select(if n == 0 { None } else { selected.or(Some(0)) });
        }
        self.tab = self.tab.min(snapshot.tabs.len().saturating_sub(1));
        self.snapshot = snapshot;
    }

    /// Advance the spinner; true when it's time for an automatic reload.
    pub fn tick(&mut self) -> bool {
        if self.splash.as_ref().is_some_and(Splash::done) {
            self.splash = None;
        }
        if !self.syncing.is_empty() {
            self.frame = self.frame.wrapping_add(1);
        }
        self.syncing.is_empty() && self.loaded_at.elapsed() >= AUTO_RELOAD
    }

    pub fn start_sync(&mut self) -> bool {
        if !self.syncing.is_empty() {
            return false;
        }
        self.syncing.extend(BROKERS);
        self.error = None;
        true
    }

    pub fn finish_sync(&mut self, results: &[(Broker, anyhow::Result<SyncReport>)]) {
        self.syncing.clear();
        let errors: Vec<_> = results
            .iter()
            .filter_map(|(b, r)| r.as_ref().err().map(|e| format!("{b:?}: {e:#}")))
            .collect();
        self.error = (!errors.is_empty()).then(|| errors.join("; "));
        self.synced_remotely = self.remote.is_some() && errors.len() < results.len();
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Action {
        if key.kind != KeyEventKind::Press {
            return Action::None;
        }
        // Any key skips the splash; q still quits.
        if self.splash.take().is_some() && key.code != KeyCode::Char('q') {
            return Action::None;
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true
            }
            KeyCode::Char('r') => return Action::Reload,
            KeyCode::Char('s') => return Action::Sync,
            KeyCode::Char('o') => {
                self.view = match self.view {
                    View::Overview => View::Detail,
                    View::Detail => View::Overview,
                }
            }
            KeyCode::Left | KeyCode::Char('h') => self.step_tab(-1),
            KeyCode::Right | KeyCode::Char('l') => self.step_tab(1),
            KeyCode::Down | KeyCode::Char('j') => self.step_row(1),
            KeyCode::Up | KeyCode::Char('k') => self.step_row(-1),
            KeyCode::Home | KeyCode::Char('g') => self.step_row(isize::MIN),
            KeyCode::End | KeyCode::Char('G') => self.step_row(isize::MAX),
            _ => {}
        }
        Action::None
    }

    fn step_tab(&mut self, step: isize) {
        let n = self.snapshot.tabs.len();
        if self.view == View::Detail && n > 0 {
            self.tab = (self.tab as isize + step).rem_euclid(n as isize) as usize;
        }
    }

    fn step_row(&mut self, step: isize) {
        if self.view != View::Detail {
            return;
        }
        let n = self
            .snapshot
            .tabs
            .get(self.tab)
            .map_or(0, |t| t.holdings.len());
        if let (Some(table), true) = (self.tables.get_mut(self.tab), n > 0) {
            let i = table.selected().unwrap_or(0) as isize;
            let next = i.saturating_add(step).clamp(0, n as isize - 1);
            table.select(Some(next as usize));
        }
    }
}
