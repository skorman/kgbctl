//! Positions dashboard, read from the database. A port of `poc/kgbpoc.py`,
//! except that brokers are only contacted when a sync is asked for (`s`).

mod app;
mod data;
mod format;
mod ui;

use std::time::Duration;

use ratatui::crossterm::event::{self, Event};
use tokio::sync::mpsc;

use crate::db::{Broker, Db};
use crate::sync::{self, SyncOptions, SyncReport};
use app::{Action, App};

const TICK: Duration = Duration::from_millis(100);

pub async fn run(db: Db, sync_config: sync::Config) -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, db, sync_config).await;
    ratatui::restore();
    result
}

async fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    db: Db,
    sync_config: sync::Config,
) -> anyhow::Result<()> {
    // crossterm's blocking reader lives on its own thread; the loop below only awaits.
    let (key_tx, mut keys) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        while let Ok(ev) = event::read() {
            if key_tx.send(ev).is_err() {
                break;
            }
        }
    });

    let (sync_tx, mut synced) = mpsc::unbounded_channel::<Vec<(Broker, anyhow::Result<SyncReport>)>>();
    let mut ticker = tokio::time::interval(TICK);
    let mut app = App::new();
    app.reload(&db).await;

    while !app.should_quit {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        tokio::select! {
            Some(ev) = keys.recv() => {
                let Event::Key(key) = ev else { continue };
                match app.on_key(key) {
                    Action::None => {}
                    Action::Reload => app.reload(&db).await,
                    Action::Sync => {
                        if app.start_sync() {
                            let (db, config, tx) = (db.clone(), sync_config.clone(), sync_tx.clone());
                            tokio::spawn(async move {
                                let brokers = [Broker::Ibkr, Broker::Kraken];
                                let results =
                                    sync::run_all(&db, &brokers, &config, SyncOptions::default()).await;
                                let _ = tx.send(results);
                            });
                        }
                    }
                }
            }
            Some(results) = synced.recv() => {
                app.finish_sync(&results);
                app.reload(&db).await;
            }
            _ = ticker.tick() => {
                if app.tick() {
                    app.reload(&db).await;
                }
            }
        }
    }
    Ok(())
}
