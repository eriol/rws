use std::io;

use anyhow::Result;

use crate::cli::{Cli, Command};
use crate::manifest::manifest_path;
use crate::workspace;

pub fn run(cli: Cli) -> Result<u8> {
    match cli.command {
        Command::Init {
            common_justfile,
            envrc,
        } => {
            let manifest_path = manifest_path(&cli.workspace);
            if manifest_path.exists() && !confirm_reinitialize(&manifest_path)? {
                println!("init cancelled");
                return Ok(0);
            }

            workspace::init_workspace(
                &cli.workspace,
                common_justfile.as_deref(),
                envrc.as_deref(),
            )?;
            println!("initialized workspace at {}", cli.workspace);
            Ok(0)
        }
        Command::Add { source, name } => {
            let package = workspace::add_package(&cli.workspace, &source, name.as_deref())?;
            println!("added package `{}` from {}", package.name, package.source);
            Ok(0)
        }
        Command::Remove { name } => {
            workspace::remove_package(&cli.workspace, &name)?;
            println!(
                "removed package `{}`",
                crate::manifest::normalize_package_name(&name)
            );
            Ok(0)
        }
        Command::List => {
            for row in workspace::list_packages(&cli.workspace)? {
                let justfile = if row.has_justfile { "yes" } else { "no" };
                println!(
                    "{}\n  source: {}\n  link: {}\n  package justfile: {}",
                    row.name, row.source, row.link, justfile
                );
            }
            Ok(0)
        }
        Command::Sync => {
            let report = workspace::sync_workspace(&cli.workspace)?;
            for warning in report.warnings {
                println!("{warning}");
            }
            println!("synced workspace {}", cli.workspace);
            Ok(0)
        }
        Command::Doctor => {
            let report = workspace::doctor_workspace(&cli.workspace)?;
            for warning in &report.warnings {
                println!("warning: {warning}");
            }
            for error in &report.errors {
                eprintln!("error: {error}");
            }
            if report.errors.is_empty() {
                println!("doctor checks passed");
                Ok(0)
            } else {
                Ok(1)
            }
        }
    }
}

fn confirm_reinitialize(manifest_path: &camino::Utf8Path) -> Result<bool> {
    println!(
        "workspace manifest already exists at {}. Reinitialize and overwrite it? [y/N]",
        manifest_path
    );

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let answer = input.trim().to_ascii_lowercase();
    Ok(matches!(answer.as_str(), "y" | "yes"))
}
