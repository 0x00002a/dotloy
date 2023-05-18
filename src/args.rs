use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version = clap::crate_version!(), author = clap::crate_authors!("\n"))]
pub struct Args {
    #[command(subcommand)]
    pub cmd: Command,
    #[arg(
        long,
        global = true,
        help = "Config file to use. If not provided defaults to dotloy.yaml in cwd"
    )]
    pub config: Option<std::path::PathBuf>,
    #[arg(
        long,
        global = true,
        help = "Level to use for logging",
        default_value = "info"
    )]
    pub log_level: log::LevelFilter,
}

#[derive(Subcommand, Clone)]
pub enum Command {
    #[command(about = "Expand a file using the template engine")]
    Expand(ExpandCmd),
    #[command(about = "Deploy a configuration")]
    Deploy(DeployCmd),
}

#[derive(clap::Args, Clone)]
pub struct ExpandCmd {
    #[arg(help = "File to expand")]
    pub target: std::path::PathBuf,
    #[arg(
        long,
        short,
        help = "Output to write to, writes to stdout if not provided"
    )]
    pub output: Option<std::path::PathBuf>,
}
#[derive(clap::Args, Clone)]
pub struct DeployCmd {
    #[arg(long, help = "Print actions but don't actually do them")]
    pub dry_run: bool,
}
