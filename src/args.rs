use clap::Parser;

#[derive(Parser)]
pub struct Args {
    #[arg(help = "Config file to use. If not provided defaults to dotoy.yaml in cwd")]
    pub config: Option<std::path::PathBuf>,

    #[arg(long, help = "Print actions but don't actually do them")]
    pub dry_run: bool,
}
