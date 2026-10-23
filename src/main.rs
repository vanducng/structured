use std::process::ExitCode;

use clap::Parser;
use clap::error::ErrorKind;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cli = match structured::Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(err)
            if matches!(
                err.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            err.exit()
        }
        Err(err) if structured::wants_json_raw(&args) => {
            let help = format!("{} --help", command_path(&args));
            let usage =
                structured::Error::usage_with(summary(&err.render().to_string()), &[help.as_str()]);
            structured::report(true, &usage);
            return usage.exit_code();
        }
        Err(err) => err.exit(),
    };
    match structured::execute(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            if !err.quiet() {
                structured::report(structured::wants_json(&cli), &err);
            }
            err.exit_code()
        }
    }
}

/// Clap's message without the usage block, on one line, so an agent sees which argument is wrong.
fn summary(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with("Usage:") && !line.starts_with("For more information"))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .trim_start_matches("error: ")
        .to_owned()
}

fn command_path(args: &[String]) -> String {
    let mut command = <structured::Cli as clap::CommandFactory>::command();
    let mut path = vec!["structured".to_owned()];
    for arg in args.iter().skip(1) {
        let Some(next) = command.find_subcommand(arg).cloned() else {
            continue;
        };
        path.push(next.get_name().to_owned());
        command = next;
    }
    path.join(" ")
}
