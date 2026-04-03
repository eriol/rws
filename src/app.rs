use anyhow::Result;

use crate::cli::{Cli, Command, PackageCommand, WorkspaceUpdateArgs};
use crate::workspace;

pub fn run(cli: Cli) -> Result<u8> {
    match cli.command {
        Command::Init(args) => {
            workspace::init_workspace(
                &cli.workspace,
                args.common_justfile.as_deref(),
                args.envrc.as_deref(),
            )?;
            println!("initialized workspace at {}", cli.workspace);
            Ok(0)
        }
        Command::Update(args) => {
            workspace::update_workspace(&cli.workspace, workspace_update(args))?;
            println!("updated workspace at {}", cli.workspace);
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
        Command::Package { command } => match command {
            PackageCommand::Add { source, name } => {
                let package = workspace::add_package(&cli.workspace, &source, name.as_deref())?;
                println!("added package `{}` from {}", package.name, package.source);
                Ok(0)
            }
            PackageCommand::Remove { name } => {
                workspace::remove_package(&cli.workspace, &name)?;
                println!(
                    "removed package `{}`",
                    crate::manifest::normalize_package_name(&name)
                );
                Ok(0)
            }
        },
    }
}

fn workspace_update(args: WorkspaceUpdateArgs) -> workspace::WorkspaceUpdate {
    workspace::WorkspaceUpdate {
        common_justfile: args.common_justfile,
        remove_common_justfile: args.no_common_justfile,
        envrc: args.envrc,
        remove_envrc: args.no_envrc,
    }
}
