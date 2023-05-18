use std::{
    io::{BufReader, Write},
    path::PathBuf,
    process::exit,
};

use actions::Actions;
use anyhow::anyhow;
use args::{Args, DeployCmd, ExpandCmd};
use clap::Parser;
use template::{Context, Variable};

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
fn run_deploy(args: DeployCmd) -> anyhow::Result<()> {
    let cfg_file = args.config.or_else(find_cfg_file);
    if cfg_file.as_ref().map(|c| !c.exists()).unwrap_or(true) {
        return Err(anyhow!(
            "No config file not found, rerun with --config or add a dotloy.yaml in the cwd"
        ));
    }
    let cfg_dir = cfg_file.as_ref().unwrap().parent().unwrap();
    std::env::set_current_dir(cfg_dir)?;
    let cfg_file = BufReader::new(std::fs::File::open(cfg_file.unwrap())?);
    let cfg_file = serde_yaml::from_reader(cfg_file)?;
    let template_engine = default_parse_context();
    let actions = Actions::from_config(&cfg_file, &template_engine)?;
    actions.run(args.dry_run)?;
    Ok(())
}
fn run_expand(cmd: ExpandCmd) -> anyhow::Result<()> {
    let target = cmd.target;
    if !target.exists() {
        return Err(anyhow!(
            "Expand target '{target}' does not exist",
            target = target.to_string_lossy()
        ));
    }
    let engine = default_parse_context();
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

fn main() {
    let args = Args::parse();
    let r = match args.cmd {
        args::Command::Expand(cmd) => run_expand(cmd),
        args::Command::Deploy(cmd) => run_deploy(cmd),
    };
    if let Err(e) = r {
        eprintln!("{}", e);
        exit(1);
    }
}
