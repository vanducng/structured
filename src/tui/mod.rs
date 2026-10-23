mod app;
mod view;
mod worker;

use std::io::{self, stdout};
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use crate::config;
use crate::error::Result;

use app::App;

pub fn run() -> Result<()> {
    let (tx, rx) = worker::spawn()?;
    let mut app = App::new(config::load());
    app.reveal();
    app.pump(&tx);
    let mut terminal = setup()?;
    let _guard = RawGuard;
    terminal.draw(|frame| view::draw(frame, &app))?;
    let mut last = Instant::now();
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
                    app.status = "sync stopped".into();
                    terminal.draw(|frame| view::draw(frame, &app))?;
                    return Ok(());
                }
            }
        }
        app.pump(&tx);
        if draw {
            terminal.draw(|frame| view::draw(frame, &app))?;
        }
        if event::poll(Duration::from_millis(80))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    if app::handle(&mut terminal, &mut app, &tx, key.code, key.modifiers)? {
                        break;
                    }
                    app.pump(&tx);
                    terminal.draw(|frame| view::draw(frame, &app))?;
                }
                Event::Resize(_, _) => {
                    terminal.draw(|frame| view::draw(frame, &app))?;
                }
                _ => {}
            }
        } else if last.elapsed() >= Duration::from_secs(20) && app.should_poll_refresh() {
            app.arm_refresh();
            app.pump(&tx);
            last = Instant::now();
        }
    }
    Ok(())
}

fn setup() -> Result<ratatui::DefaultTerminal> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    Ok(ratatui::Terminal::new(
        ratatui::backend::CrosstermBackend::new(stdout()),
    )?)
}

struct RawGuard;

impl Drop for RawGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = io::stdout().execute(LeaveAlternateScreen);
    }
}
