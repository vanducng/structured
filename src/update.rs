use std::io::{self, Write};
use std::process::{Command, Stdio};

use serde_json::json;

use crate::auth;
use crate::error::{Error, Result};
use crate::output;

const INSTALLER: &str = "https://raw.githubusercontent.com/vanducng/structured/main/install.sh";

/// Runs install.sh against the directory this binary lives in, so install and update share one path.
pub fn run(json: bool, dry_run: bool) -> Result<()> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let dir = exe
        .parent()
        .ok_or_else(|| Error::usage("cannot tell which directory structured is installed in"))?;
    let from = env!("CARGO_PKG_VERSION");
    if dry_run {
        let plan = json!({"dry_run": true, "installer": INSTALLER, "dir": dir, "version": from});
        return output::value(json, "would run", plan);
    }
    let script = auth::http()?
        .get(INSTALLER)
        .send()?
        .error_for_status()?
        .text()?;
    let mut shell = Command::new("sh")
        .env("STRUCTURED_INSTALL_DIR", dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::from(io::stderr()))
        .spawn()?;
    shell
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("no stdin for sh"))?
        .write_all(script.as_bytes())?;
    if !shell.wait()?.success() {
        return Err(Error::Io(io::Error::other(
            "the installer failed; its output is above",
        )));
    }
    let printed = Command::new(&exe).arg("--version").output()?.stdout;
    let to = String::from_utf8_lossy(&printed)
        .trim()
        .trim_start_matches("structured ")
        .to_owned();
    if json {
        return output::value(
            true,
            "updated",
            json!({"from": from, "to": to, "path": exe}),
        );
    }
    if to == from {
        println!("structured {to} is the latest");
    } else {
        println!("structured {from} -> {to}");
    }
    Ok(())
}
