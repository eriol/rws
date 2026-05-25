use anyhow::Result;
use camino::Utf8PathBuf;
use clap::CommandFactory;
use clap_complete::{Shell, generate};

use crate::cli::{Cli, Command, CompletionShell, PackageCommand, WorkspaceUpdateArgs};
use crate::errors::RwsError;
use crate::fs_utils::{path_exists, utf8_from_path_buf};
use crate::manifest::manifest_path;
use crate::workspace;

pub fn run(cli: Cli) -> Result<u8> {
    match cli.command {
        Command::Version => {
            println!("rws {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        Command::Init(args) => {
            workspace::init_workspace(
                &args.workspace,
                args.common_justfile.as_deref(),
                args.envrc.as_deref(),
            )?;
            println!("initialized workspace at {}", args.workspace);
            Ok(0)
        }
        Command::Update(args) => {
            let workspace_root = discover_workspace_root()?;
            workspace::update_workspace(&workspace_root, workspace_update(args))?;
            println!("updated workspace at {workspace_root}");
            Ok(0)
        }
        Command::List => {
            let workspace_root = discover_workspace_root()?;
            for row in workspace::list_packages(&workspace_root)? {
                let justfile = if row.has_justfile { "yes" } else { "no" };
                println!(
                    "{}\n  source: {}\n  link: {}\n  package justfile: {}",
                    row.name, row.source, row.link, justfile
                );
            }
            Ok(0)
        }
        Command::Sync => {
            let workspace_root = discover_workspace_root()?;
            let report = workspace::sync_workspace(&workspace_root)?;
            for warning in report.warnings {
                println!("{warning}");
            }
            println!("synced workspace {workspace_root}");
            Ok(0)
        }
        Command::Doctor => {
            let workspace_root = discover_workspace_root()?;
            let report = workspace::doctor_workspace(&workspace_root)?;
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
        Command::Completion { shell } => {
            let mut command = Cli::command();
            generate(
                completion_shell(shell),
                &mut command,
                "rws",
                &mut std::io::stdout(),
            );
            Ok(0)
        }
        Command::Package { command } => match command {
            PackageCommand::Add { sources } => {
                let workspace_root = discover_workspace_root()?;
                for package in workspace::add_packages(&workspace_root, &sources)? {
                    println!("added package `{}` from {}", package.name, package.source);
                }
                Ok(0)
            }
            PackageCommand::Remove { name } => {
                let workspace_root = discover_workspace_root()?;
                workspace::remove_package(&workspace_root, &name)?;
                println!(
                    "removed package `{}`",
                    crate::manifest::normalize_package_name(&name)
                );
                Ok(0)
            }
        },
    }
}

fn completion_shell(shell: CompletionShell) -> Shell {
    match shell {
        CompletionShell::Bash => Shell::Bash,
        CompletionShell::Zsh => Shell::Zsh,
    }
}

fn discover_workspace_root() -> Result<Utf8PathBuf> {
    let cwd = utf8_from_path_buf(std::env::current_dir()?)?;
    let mut candidate = cwd.as_path();

    loop {
        if path_exists(&manifest_path(candidate)) {
            return Ok(candidate.to_path_buf());
        }

        let Some(parent) = candidate.parent() else {
            break;
        };
        candidate = parent;
    }

    Err(RwsError::WorkspaceNotFound(cwd).into())
}

fn workspace_update(args: WorkspaceUpdateArgs) -> workspace::WorkspaceUpdate {
    workspace::WorkspaceUpdate {
        common_justfile: args.common_justfile,
        remove_common_justfile: args.no_common_justfile,
        envrc: args.envrc,
        remove_envrc: args.no_envrc,
    }
}
