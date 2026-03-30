use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = rws::cli::Cli::parse();

    match rws::app::run(cli) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
