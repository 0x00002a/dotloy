#![deny(unused_crate_dependencies)]
use std::{
    io::{BufReader, Write},
    path::PathBuf,
    process::exit,
};

use actions::Actions;
use args::{Args, DeployCmd, ExpandCmd};
use clap::{CommandFactory, Parser};
use colored::{Color, Colorize};
use config::Root;
use handybars::{Context, Object, Variable};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod actions;
mod args;
mod config;

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

fn run_deploy(args: DeployCmd, cfg_file: &Root) -> Result<()> {
    let template_engine = default_parse_context();
    let actions = Actions::from_config(cfg_file, &template_engine)?;
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
    #[error("Config file is required for this command")]
    ConfigFileNeeded,
    #[error("Config file '{path}' does not exist")]
    ConfigFileDoesNotExist { path: String },
    #[error(transparent)]
    Action(#[from] actions::Error),
    #[error(transparent)]
    Template(#[from] handybars::Error),
    #[error("Target does not exist '{0}'")]
    TargetDoesNotExist(String),
    #[error("Shell is not supported for completions")]
    UnsupportedShell,
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

fn run() -> Result<()> {
    let args = Args::parse();
    init_logging(args.log_level);
    let cfg_file = args.config.or_else(find_cfg_file);
    if let Some(p) = &cfg_file {
        if !p.exists() {
            return Err(Error::ConfigFileDoesNotExist {
                path: p.to_string_lossy().into_owned(),
            });
        }
        if let Some(parent) = p.parent() {
            std::env::set_current_dir(parent)?;
        }
    }
    let cfg_file = cfg_file
        .map(|p| Ok::<_, Error>(BufReader::new(std::fs::File::open(p)?)))
        .transpose()?
        .map(serde_yaml::from_reader)
        .transpose()?;
    match args.cmd {
        args::Command::Expand(cmd) => run_expand(cmd, cfg_file.as_ref()),
        args::Command::Deploy(cmd) => {
            run_deploy(cmd, cfg_file.as_ref().ok_or(Error::ConfigFileNeeded)?)
        }
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
