mod app;
mod theme;
mod view;
mod worker;

use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::config::UiConfig;
use crate::error::Result;

use app::{App, Tone};

const REFRESH: Duration = Duration::from_secs(20);

pub fn run(config: UiConfig) -> Result<()> {
    let (tx, rx) = worker::spawn()?;
    let mut app = App::new(config);
    app.reveal();
    if let Some(warning) = app.config.warnings.first().cloned() {
        app.say(Tone::Bad, warning);
    }
    app.pump(&tx);
    let mut terminal = ratatui::init();
    let _guard = Restore;
    terminal.draw(|frame| view::draw(frame, &app))?;
    let mut last = Instant::now();
    let mut minute = clock_minute();
    loop {
        let mut draw = false;
        loop {
            match rx.try_recv() {
                Ok(reply) => {
                    app.apply_reply(reply);
                    draw = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    app.say(Tone::Bad, "sync stopped");
                    terminal.draw(|frame| view::draw(frame, &app))?;
                    return Ok(());
                }
            }
        }
        if app.quitting && app.pending() == 0 {
            if app.status.tone != Tone::Bad {
                break;
            }
            app.quitting = false;
        }
        app.pump(&tx);
        draw |= app.expire_status() || minute != clock_minute();
        minute = clock_minute();
        if draw {
            terminal.draw(|frame| view::draw(frame, &app))?;
        }
        if event::poll(Duration::from_millis(80))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    if app::handle(&mut terminal, &mut app, &tx, key)? {
                        if app.pending() == 0 || app.quitting {
                            break;
                        }
                        app.quitting = true;
                        let saves = app.pending();
                        let noun = if saves == 1 { "save" } else { "saves" };
                        app.say(
                            Tone::Busy,
                            format!("finishing {saves} {noun}, q again quits now"),
                        );
                    }
                    app.pump(&tx);
                    terminal.draw(|frame| view::draw(frame, &app))?;
                }
                Event::Resize(_, _) => {
                    terminal.draw(|frame| view::draw(frame, &app))?;
                }
                _ => {}
            }
        } else if last.elapsed() >= REFRESH && app.should_poll_refresh() {
            app.arm_refresh();
            app.pump(&tx);
            last = Instant::now();
        }
    }
    Ok(())
}

/// The now-line and relative day labels move with the wall clock, so redraw once a minute.
fn clock_minute() -> i64 {
    chrono::Utc::now().timestamp() / 60
}

struct Restore;

impl Drop for Restore {
    fn drop(&mut self) {
        ratatui::restore();
    }
}
