#![deny(unused_must_use)]
#![deny(unused_crate_dependencies)]
use std::{
    io::{BufReader, Write},
    path::{Path, PathBuf},
    process::exit,
};

use actions::Actions;
use args::{Args, DeployCmd, ExpandCmd};
use clap::{CommandFactory, Parser};
use colored::{Color, Colorize};
use config::Root;
use handybars::{Context, Object, Variable};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod abspath;
mod actions;
mod args;
mod config;
pub(crate) mod resources;
use fs_err as fs;

use crate::abspath::AbsPathBuf;

mod vars {
    use handybars::Variable;

    pub fn target_level() -> Variable<'static> {
        Variable::single("target")
    }
    pub fn config_level() -> Variable<'static> {
        Variable::single("config")
    }
}

fn xdg_context() -> Context<'static> {
    let dirs = directories::BaseDirs::new().expect("failed to get dirs on system");

    let mut xdg = Object::new()
        .with_property("home", dirs.home_dir().to_string_lossy().into_owned())
        .with_property("config", dirs.config_dir().to_string_lossy().into_owned())
        .with_property(
            "local",
            Object::new().with_property(
                "config",
                dirs.config_local_dir().to_string_lossy().into_owned(),
            ),
        );
    // TODO: This will give an unhelpful variable not defined error on non-linux
    // maybe intercept the error from handybars for this variable and provide our own?
    if let Some(dir) = dirs.executable_dir() {
        xdg.add_property("exec", dir.to_string_lossy().into_owned());
    }
    Context::new().with_define(Variable::single("xdg".to_owned()), xdg)
}

fn default_parse_context() -> Context<'static> {
    let mut ctx = Context::new();
    ctx.define(
        Variable::single("cwd".to_string()),
        std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
    );
    ctx.append(xdg_context());
    ctx
}

#[repr(transparent)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Templated<T>(T);
impl<T> Templated<T> {
    pub fn new(inner: T) -> Self {
        Self(inner)
    }
}
impl Templated<String> {
    pub fn render(&self, ctx: &Context) -> Result<String, handybars::Error> {
        ctx.render(&self.0)
    }
}

fn define_variables<'a, 'b>(
    on: &mut Context<'b>,
    namespace: &Variable<'b>,
    vars: impl Iterator<Item = (&'a String, &'a Templated<String>)>,
) -> Result<(), handybars::Error> {
    for (var, val) in vars {
        on.define(
            namespace.clone().join(var.parse()?),
            handybars::Value::String(val.render(on)?.into()),
        );
    }
    Ok(())
}

fn handle_watch_updates(
    args: DeployCmd,
    actions: Actions,
    rx: std::sync::mpsc::Receiver<notify::Result<notify::Event>>,
) {
    log::info!(
        "watching for file changes on [{}]",
        actions
            .file_roots()
            .map(|p| if !p.is_dir() { p.parent().unwrap() } else { &p }
                .to_string_lossy()
                .into_owned())
            .join(", ")
    );
    for res in rx {
        match res {
            Ok(ev) => match ev.kind {
                notify::EventKind::Create(_)
                | notify::EventKind::Remove(_)
                | notify::EventKind::Any
                | notify::EventKind::Modify(_) => {
                    log::info!("detected file changes");
                    log::debug!("notify event: {ev:#?}");
                    let r = if ev.paths.is_empty() {
                        None
                    } else {
                        Some(ev.paths)
                    }
                    .map(|s| {
                        actions.dependents_of(
                            s.into_iter()
                                .map(|p| {
                                    AbsPathBuf::new(p)
                                        .expect("failed to canonicalize path from notify")
                                })
                                .map(resources::ResourceLocation::Path)
                                .collect(),
                        )
                    })
                    .as_ref()
                    .unwrap_or(&actions)
                    .run(args.dry_run);
                    if let Err(e) = r {
                        log::error!("failed to redeploy: {e}");
                    }
                }
                _ => {}
            },
            Err(e) => log::error!("watch error: {e}"),
        };
    }
}

fn run_deploy(args: DeployCmd) -> Result<()> {
    let template_engine = default_parse_context();
    let (tx, rx) = std::sync::mpsc::channel();
    let mut actions = Actions::new();
    let mut watcher = if args.watch {
        let watcher = notify::recommended_watcher(tx)?;
        Some(watcher)
    } else {
        None
    };
    let root_dir = fs::canonicalize(std::env::current_dir()?)?;
    for target in args.targets.clone() {
        let target_str = target.to_string_lossy();
        if !target.exists() {
            log::warn!("path '{target_str}' does not exist");
            continue;
        }
        let Ok(target) = fs::canonicalize(&target).map_err(|e| {
            log::warn!("failed to canonicalize path '{target_str}': {e}, skipping...");
        }) else {continue;};
        let Ok(Some(cfg)) = read_config(&target).map_err(|e| {
            log::warn!("failed to load config at '{target}': {e}", target = target.to_string_lossy());
        }).map(|v| {
            if v.is_none() {
                log::warn!("failed to find config file for '{target}'", target = target.to_string_lossy());
        } v}) else {
            continue;
        };
        std::env::set_current_dir(root_dir.join(resolve_config_dir(&target).unwrap()))?;
        let mut acts = Actions::from_config(&cfg, &template_engine)?;
        actions.append(&mut acts);
        std::env::set_current_dir(&root_dir)?;
    }
    std::env::set_current_dir(&root_dir)?;
    if let Some(watcher) = &mut watcher {
        log::debug!("actions: {actions:#?}");
        actions.configure_watcher(watcher)?;
    }
    actions.run(args.dry_run)?;
    if watcher.is_some() {
        handle_watch_updates(args, actions, rx);
    }

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
        define_variables(
            &mut engine,
            &vars::config_level(),
            cfg.shared.variables.iter(),
        )?;
        if let Some(target) = cfg.targets.iter().find(|t| {
            t.path
                .render(&engine)
                .map(|p| p == cmd.target.to_string_lossy().as_ref())
                .unwrap_or(false)
        }) {
            define_variables(
                &mut engine,
                &vars::target_level(),
                target.shared.variables.iter(),
            )?;
        }
    }
    let content = std::fs::read_to_string(target)?;
    let rendered = engine.render(&content)?;
    match cmd.output {
        Some(p) => {
            write!(std::fs::File::create(p)?, "{}", rendered)?;
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
    #[error(transparent)]
    Action(#[from] actions::Error),
    #[error(transparent)]
    Template(#[from] handybars::Error),
    #[error("Target does not exist '{0}'")]
    TargetDoesNotExist(String),
    #[error("Shell is not supported for completions")]
    UnsupportedShell,
    #[error("Watch error '{0}'")]
    Watch(#[from] notify::Error),
}
#[cfg(test)]
fn test_data_path() -> &'static std::path::Path {
    "./test_data".as_ref()
}

fn init_logging(level: log::LevelFilter) {
    fn colour_for_level(level: log::Level) -> Color {
        match level {
            log::Level::Error => Color::Red,
            log::Level::Warn => Color::Yellow,
            log::Level::Info => Color::Green,
            log::Level::Debug => Color::BrightGreen,
            log::Level::Trace => Color::White,
        }
    }
    fern::Dispatch::new()
        .level_for(env!("CARGO_PKG_NAME"), level)
        .level(log::LevelFilter::Off)
        .format(|out, msg, record| {
            if record.target() == "dotloy::actions" {
                out.finish(format_args!(
                    "{}",
                    msg.to_string().color(colour_for_level(record.level()))
                ))
            } else {
                out.finish(format_args!(
                    "[{src}]: {msg}",
                    src = record.target(),
                    msg = msg.to_string().color(colour_for_level(record.level()))
                ))
            }
        })
        .chain(
            fern::Dispatch::new()
                .level(log::LevelFilter::Error)
                .chain(std::io::stderr()),
        )
        .chain(
            fern::Dispatch::new()
                .filter(|m| m.level() > log::Level::Error)
                .chain(std::io::stdout()),
        )
        .apply()
        .expect("failed to init logging");
}

const DOTLOY_CFG_NAMES: [&str; 2] = ["dotloy.yaml", "dotloy.yml"];
fn find_config_in_dir(dir: &Path) -> Option<PathBuf> {
    assert!(dir.is_dir(), "tried to find config in non-directory");
    DOTLOY_CFG_NAMES
        .into_iter()
        .map(Path::new)
        .map(|p| dir.join(p))
        .find(|c| c.exists())
}
fn resolve_config_dir(p: &Path) -> Option<&Path> {
    if p.is_dir() {
        Some(p)
    } else {
        p.parent()
    }
}

fn read_config(p: &Path) -> Result<Option<Root>> {
    let p = if p.is_dir() {
        find_config_in_dir(p)
    } else {
        Some(p.to_owned())
    };
    p.map(|p| {
        let cfg = serde_yaml::from_reader(BufReader::new(fs::File::open(p)?))?;
        Ok(cfg)
    })
    .transpose()
}

fn run() -> Result<()> {
    let args = Args::parse();
    init_logging(args.log_level);
    match args.cmd {
        args::Command::Expand(cmd) => {
            let cfg = cmd
                .config
                .as_ref()
                .map(|c| read_config(c))
                .transpose()?
                .flatten();
            if let Some(p) = &cmd.config {
                std::env::set_current_dir(resolve_config_dir(p).unwrap())?;
            }
            run_expand(cmd, cfg.as_ref())
        }
        args::Command::Deploy(cmd) => run_deploy(cmd),
        args::Command::GenerateShellCompletions => {
            let shell = clap_complete::Shell::from_env().ok_or(Error::UnsupportedShell)?;
            let mut cmd = Args::command();
            let bname = cmd.get_bin_name().unwrap_or(cmd.get_name()).to_owned();
            clap_complete::generate(shell, &mut cmd, &bname, &mut std::io::stdout());
            Ok(())
        }
    }?;
    Ok(())
}

fn main() {
    let r = run();
    if let Err(e) = r {
        log::error!("{}", e);
        exit(1);
    }
}
