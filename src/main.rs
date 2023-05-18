use std::path::PathBuf;

use args::Args;
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

fn main() {
    let args = Args::parse();
    let cfg_file = args.config.or_else(find_cfg_file);
}
