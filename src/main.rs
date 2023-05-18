#![deny(unused_crate_dependencies)]
use std::{
    io::{BufReader, Write},
    path::PathBuf,
    process::exit,
};

use actions::Actions;
use args::{Args, DeployCmd, ExpandCmd};
use clap::Parser;
use config::Root;
use template::{Context, Variable};
use thiserror::Error;

mod actions;
mod args;
mod config;
mod template;

fn xdg_context() -> template::Context {
    let xdg = Variable::single("xdg".to_owned());
    let local = xdg.with_child("local");
    let dirs = directories::BaseDirs::new().expect("failed to get dirs on system");

    Context::new()
        .with_define(
            xdg.with_child("home"),
            dirs.home_dir().to_string_lossy().to_string(),
        )
        .with_define(
            xdg.with_child("config"),
            dirs.config_dir().to_string_lossy().to_string(),
        )
        .with_define(
            local.with_child("config"),
            dirs.config_local_dir().to_string_lossy().to_string(),
        )
}

fn default_parse_context() -> template::Context {
    let mut ctx = template::Context::new();
    ctx.define(
        Variable::single("cwd".to_string()),
        std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string(),
    );
    ctx.append(xdg_context());
    ctx
}
fn find_cfg_file() -> Option<PathBuf> {
    let path = std::env::current_dir()
        .expect("failed to get cwd")
        .join("dotloy.yaml");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}
fn run_deploy(args: DeployCmd, cfg_file: &Root) -> Result<()> {
    let template_engine = default_parse_context();
    let actions = Actions::from_config(&cfg_file, &template_engine)?;
    actions.run(args.dry_run)?;
    Ok(())
}
fn run_expand(cmd: ExpandCmd, cfg: Option<&Root>) -> Result<()> {
    let target = &cmd.target;
    if !target.exists() {
        return Err(Error::TargetDoesNotExist(
            target.to_string_lossy().into_owned(),
        ));
    }
    let mut engine = default_parse_context();
    if let Some(cfg) = cfg {
        engine.add_defines_with_namespace(Variable::config_level(), cfg.variables.iter())?;
        if let Some(target) = cfg.targets.iter().find(|t| {
            t.path
                .render(&engine)
                .map(|p| &p == cmd.target.to_string_lossy().as_ref())
                .unwrap_or(false)
        }) {
            engine.add_defines_with_namespace(Variable::target_level(), target.variables.iter())?;
        }
    }
    let content = std::fs::read_to_string(target)?;
    let rendered = engine.render(&content)?;
    match cmd.output {
        Some(p) => {
            write!(std::fs::File::create(&p)?, "{}", rendered)?;
        }
        None => print!("{}", rendered),
    }

    Ok(())
}
type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Error, Debug)]
enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Parse(#[from] serde_yaml::Error),
    #[error("Config file is required for this command")]
    ConfigFileNeeded,
    #[error("Config file '{path}' does not exist")]
    ConfigFileDoesNotExist { path: String },
    #[error(transparent)]
    Action(#[from] actions::Error),
    #[error(transparent)]
    Template(#[from] template::Error),
    #[error("Target does not exist '{0}'")]
    TargetDoesNotExist(String),
}
fn run() -> Result<()> {
    let args = Args::parse();
    let cfg_file = args.config.or_else(find_cfg_file);
    if let Some(p) = &cfg_file {
        if !p.exists() {
            return Err(Error::ConfigFileDoesNotExist {
                path: p.to_string_lossy().into_owned(),
            });
        }
        std::env::set_current_dir(p)?;
    }
    let cfg_file = cfg_file
        .map(|p| Ok::<_, Error>(BufReader::new(std::fs::File::open(p)?)))
        .transpose()?
        .map(|f| serde_yaml::from_reader(f))
        .transpose()?;
    match args.cmd {
        args::Command::Expand(cmd) => run_expand(cmd, cfg_file.as_ref()),
        args::Command::Deploy(cmd) => {
            run_deploy(cmd, cfg_file.as_ref().ok_or(Error::ConfigFileNeeded)?)
        }
    }?;
    Ok(())
}

fn main() -> Result<()> {
    let r = run();
    if let Err(e) = r {
        eprintln!("{}", e);
        exit(1);
    }
    Ok(())
}
