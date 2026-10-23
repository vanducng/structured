use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = structured::Cli::parse();
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
