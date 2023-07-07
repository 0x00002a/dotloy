use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version = clap::crate_version!(), author = clap::crate_authors!("\n"))]
pub struct Args {
    #[command(subcommand)]
    pub cmd: Command,
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
    #[command(about = "Deploy a configuration", visible_alias = "d")]
    Deploy(DeployCmd),
    #[command(about = "Generate shell completions")]
    GenerateShellCompletions,
}

#[derive(clap::Args, Clone)]
pub struct ExpandCmd {
    #[arg(help = "File to expand", value_hint = clap::ValueHint::FilePath)]
    pub target: std::path::PathBuf,
    #[arg(
        long,
        short,
        help = "Output to write to, writes to stdout if not provided",
        value_hint = clap::ValueHint::FilePath,
    )]
    pub output: Option<std::path::PathBuf>,
    #[arg(
        long,
        global = true,
        help = "Config file to use. If not provided defaults to dotloy.yaml in cwd"
    )]
    pub config: Option<std::path::PathBuf>,
}
#[derive(clap::Args, Clone)]
pub struct DeployCmd {
    #[arg(
        help = "Targets to deploy. Directories are searched for dotloy.ya?ml's while files are treated as dotloy.yaml's directly"
    )]
    pub targets: Vec<std::path::PathBuf>,
    #[arg(long, help = "Print actions but don't actually do them")]
    pub dry_run: bool,
    #[arg(long, short, help = "Watch directory and re-deploy on changes")]
    pub watch: bool,
}
