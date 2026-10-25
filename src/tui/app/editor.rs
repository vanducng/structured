use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::Sender;

use crate::error::Result;

use super::keys::send;
use super::{App, Tone};
use crate::tui::worker::Request;

pub(super) fn edit_note(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    tx: &Sender<Request>,
) -> Result<()> {
    let Some((index, _)) = app.selected_ref() else {
        return Ok(());
    };
    let previous = app.tasks[index].note.trim_end_matches('\n').to_owned();
    let Some(next) = edit_text(terminal, app, &previous)? else {
        return Ok(());
    };
    if next == previous {
        app.say(Tone::Quiet, "note unchanged");
        return Ok(());
    }
    let recurring = app.tasks[index].is_recurring;
    send(tx, app.update_note(next, previous));
    if recurring {
        app.say(Tone::Busy, "saving the note for every repeat…");
    }
    Ok(())
}

/// Runs the editor on `body`. None when nothing came back to use; the status says why.
pub(super) fn edit_text(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    body: &str,
) -> Result<Option<String>> {
    let path = scratch_file(body)?;
    ratatui::restore();
    let edited = run_editor(&path);
    resume(terminal)?;
    let read = fs::read_to_string(&path);
    let _ = fs::remove_file(&path);
    match edited {
        Ok(true) => {}
        Ok(false) => {
            app.say(Tone::Quiet, "editor closed without saving");
            return Ok(None);
        }
        Err(err) => {
            app.say(
                Tone::Bad,
                format!("could not start {}: {err}", editor_program()),
            );
            return Ok(None);
        }
    }
    let Ok(text) = read else {
        app.say(Tone::Bad, "could not read the note back, it is unchanged");
        return Ok(None);
    };
    Ok(Some(text.trim_end_matches('\n').to_owned()))
}

/// create_new refuses a planted symlink in a shared temp dir, and 0600 keeps the note private.
fn scratch_file(body: &str) -> Result<PathBuf> {
    let nonce: u64 = rand::random();
    let path = std::env::temp_dir().join(format!(
        "structured-note-{}-{nonce:x}.md",
        std::process::id()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(&path)?.write_all(body.as_bytes())?;
    Ok(path)
}

fn resume(terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    ratatui::crossterm::terminal::enable_raw_mode()?;
    ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::terminal::EnterAlternateScreen
    )?;
    terminal.clear()?;
    Ok(())
}

fn editor_program() -> String {
    ["VISUAL", "EDITOR"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "vi".into())
}

fn run_editor(path: &Path) -> Result<bool> {
    let editor = editor_program();
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("vi");
    let status = Command::new(program).args(parts).arg(path).status()?;
    Ok(status.success())
}
