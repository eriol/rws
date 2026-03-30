use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "rws", about = "Manage ROS workspaces and justfile modules")]
pub struct Cli {
    #[arg(long, global = true, default_value = ".")]
    pub workspace: Utf8PathBuf,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Init {
        #[arg(long)]
        common_justfile: Option<Utf8PathBuf>,
        #[arg(long)]
        envrc: Option<Utf8PathBuf>,
    },
    Add {
        source: Utf8PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    Remove {
        name: String,
    },
    List,
    Sync,
    Doctor,
}
