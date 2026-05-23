use camino::Utf8PathBuf;
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "rws", about = "Manage ROS workspaces and justfile modules")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Version,
    Init(InitArgs),
    Update(WorkspaceUpdateArgs),
    List,
    Sync,
    Doctor,
    Package {
        #[command(subcommand)]
        command: PackageCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum PackageCommand {
    Add {
        #[arg(required = true)]
        sources: Vec<Utf8PathBuf>,
    },
    Remove {
        name: String,
    },
}

#[derive(Debug, Args)]
pub struct InitArgs {
    pub workspace: Utf8PathBuf,
    #[arg(long)]
    pub common_justfile: Option<Utf8PathBuf>,
    #[arg(long)]
    pub envrc: Option<Utf8PathBuf>,
}

#[derive(Debug, Args)]
#[command(group = clap::ArgGroup::new("common_justfile_mode").args(["common_justfile", "no_common_justfile"]))]
#[command(group = clap::ArgGroup::new("envrc_mode").args(["envrc", "no_envrc"]))]
pub struct WorkspaceUpdateArgs {
    #[arg(long)]
    pub common_justfile: Option<Utf8PathBuf>,
    #[arg(long)]
    pub no_common_justfile: bool,
    #[arg(long)]
    pub envrc: Option<Utf8PathBuf>,
    #[arg(long)]
    pub no_envrc: bool,
}
