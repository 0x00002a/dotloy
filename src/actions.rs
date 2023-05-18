use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    rc::Rc,
};

use serde::Deserialize;
use thiserror::Error;

use crate::{
    config::{self, LinkType},
    template::{self, Variable},
};

#[derive(Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(untagged)]
enum ResourceLocation {
    InMemory { id: usize },
    Path(PathBuf),
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum Action {
    Link {
        ty: LinkType,
        from: PathBuf,
        to: PathBuf,
    },
    Copy {
        from: ResourceLocation,
        to: ResourceLocation,
    },
    TemplateExpand {
        ctx: template::Context,
        target: ResourceLocation,
        output: ResourceLocation,
    },
}

impl Action {
    fn run(&self, res: &mut ResourceStore) -> Result<()> {
        match self {
            Action::Link { ty, from, to } => match ty {
                LinkType::Soft => Ok(symlink::symlink_auto(from, to)?),
                LinkType::Hard => {
                    assert!(to.is_file(), "tried to hardlink directory");
                    Ok(fs::hard_link(from, to)?)
                }
            },
            Action::Copy { from, to } => match from {
                ResourceLocation::InMemory { id: fid } => match to {
                    ResourceLocation::InMemory { id: tid } => {
                        res.set(*tid, res.get(*fid).clone());
                        Ok(())
                    }
                    loc => Ok(res.set_content(loc, res.get(*fid).clone())?),
                },
                ResourceLocation::Path(pf) => match to {
                    ResourceLocation::Path(pt) => Ok({
                        fs::copy(pf, pt)?;
                    }),
                    loc => Ok(res.set_content(loc, ResourceHandle::File(pf.to_owned()))?),
                },
            },
            Action::TemplateExpand {
                ctx,
                target,
                output,
            } => {
                let from = ctx.render(&res.get_content(target)?)?;
                res.set_content(&output, ResourceHandle::MemStr(from))?;
                Ok(())
            }
        }
    }
}
impl std::fmt::Display for ResourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceLocation::InMemory { id } => write!(f, "@{id}"),
            ResourceLocation::Path(p) => write!(f, "{}", p.to_string_lossy()),
        }
    }
}
impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Link { ty, from, to } => f.write_fmt(format_args!(
                "{from} -> {to} [{typ}]",
                typ = match ty {
                    LinkType::Hard => "hard",
                    LinkType::Soft => "soft",
                },
                from = from.to_string_lossy(),
                to = to.to_string_lossy(),
            )),
            Action::Copy { from, to } => f.write_fmt(format_args!("[{from}] -> [{to}]")),
            Action::TemplateExpand { target, output, .. } => {
                write!(f, "expand {target} to {output}")
            }
        }
    }
}

#[derive(Debug, Clone)]
enum ResourceHandle {
    MemStr(String),
    File(PathBuf),
}
impl ResourceHandle {
    fn content(&self) -> std::io::Result<String> {
        match self {
            ResourceHandle::MemStr(s) => Ok(s.clone()),
            ResourceHandle::File(f) => fs::read_to_string(f),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ResourceStore {
    handles: Vec<ResourceHandle>,
}
impl ResourceStore {
    fn new() -> Self {
        Self::default()
    }

    fn define(&mut self, handle: ResourceHandle) -> ResourceLocation {
        let id = self.handles.len();
        self.handles.push(handle);
        ResourceLocation::InMemory { id }
    }
    fn define_mem(&mut self) -> ResourceLocation {
        self.define(ResourceHandle::MemStr("".to_owned()))
    }
    fn set(&mut self, target: usize, value: ResourceHandle) {
        self.handles[target] = value;
    }
    fn set_content(
        &mut self,
        target: &ResourceLocation,
        value: ResourceHandle,
    ) -> std::io::Result<()> {
        match target {
            ResourceLocation::InMemory { id } => Ok(self.set(*id, value)),
            ResourceLocation::Path(p) => {
                write!(fs::File::create(p)?, "{}", value.content()?)
            }
        }
    }

    fn get(&self, target: usize) -> &ResourceHandle {
        &self.handles[target]
    }
    fn get_content(&self, target: &ResourceLocation) -> std::io::Result<String> {
        match target {
            ResourceLocation::InMemory { id } => self.get(*id).content(),
            ResourceLocation::Path(p) => fs::read_to_string(p),
        }
    }
}
type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Template(#[from] template::Error),
    #[error("Source file does not exist: '{path}'")]
    SourceDoesNotExist { path: String },
}

#[derive(Clone, Debug)]
pub struct Actions {
    acts: Vec<Action>,
    resources: ResourceStore,
}

impl Actions {
    pub fn run(&self, dry: bool) -> Result<()> {
        let mut res = self.resources.clone();
        for action in &self.acts {
            println!("{action}");
            if !dry {
                action.run(&mut res)?;
            }
        }
        Ok(())
    }
    pub fn from_config(cfg: &config::Root, engine: &template::Context) -> Result<Self> {
        let mut resources = ResourceStore::new();
        let mut acts = Vec::new();
        let mut engine = engine.clone();
        engine.add_defines_with_namespace(Variable::config_level(), cfg.variables.iter())?;
        for target in &cfg.targets {
            let mut engine = engine.clone();
            engine.add_defines_with_namespace(Variable::target_level(), target.variables.iter())?;
            let src_path: PathBuf = target.path.render(&engine)?.parse().unwrap();
            #[cfg(not(test))]
            if !src_path.exists() {
                return Err(Error::SourceDoesNotExist {
                    path: src_path.to_string_lossy().into_owned(),
                });
            }
            let dst_path: PathBuf = target.target_location.render(&engine)?.parse().unwrap();
            let src = ResourceLocation::Path(src_path.clone());
            let dst = ResourceLocation::Path(dst_path.clone());
            if src_path.extension() == Some("in".as_ref()) {
                let template_dst = resources.define_mem();
                acts.push(Action::TemplateExpand {
                    target: src.clone(),
                    output: template_dst.clone(),
                    ctx: engine,
                });
                acts.push(Action::Copy {
                    from: template_dst,
                    to: dst,
                });
            } else {
                acts.push(Action::Link {
                    ty: target.link_type.unwrap_or_else(|| {
                        if dst_path.is_dir() {
                            LinkType::Soft
                        } else {
                            LinkType::Hard
                        }
                    }),
                    from: src_path,
                    to: dst_path,
                });
            }
        }
        Ok(Self { acts, resources })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        actions::{Action, ResourceLocation},
        config::{Root, Target},
        default_parse_context,
        template::Templated,
        xdg_context,
    };

    use super::Actions;

    #[test]
    fn actions_with_template_does_copy() {
        let cfg = serde_yaml::from_str(
            r"
                                       targets: [ { from: ./src.in, to: ./dst } ]
        ",
        )
        .unwrap();
        let acts = Actions::from_config(&cfg, &default_parse_context()).unwrap();
        let target = ResourceLocation::InMemory { id: 0 };
        assert_eq!(
            &acts.acts,
            &[
                Action::TemplateExpand {
                    target: ResourceLocation::Path("./src.in".into()),
                    output: target.clone(),
                    ctx: default_parse_context(),
                },
                Action::Copy {
                    from: target.clone(),
                    to: ResourceLocation::Path("./dst".into())
                }
            ]
        )
    }

    #[test]
    fn variable_expansions_on_root_are_placed_in_config() {
        let mut cfg: Root = Default::default();
        let tgt = Target::new("{{ config.t1 }}".to_string(), "dst".to_string());
        let t1val = "{{ xdg.home }}/t".to_owned();
        cfg.variables
            .insert("t1".to_owned(), Templated::new(t1val.clone()));
        cfg.targets.push(tgt);

        let acts = Actions::from_config(&cfg, &default_parse_context()).unwrap();
        assert_eq!(
            &acts.acts,
            &[Action::Link {
                ty: crate::config::LinkType::Hard,
                from: xdg_context().render(&t1val).unwrap().into(),
                to: "dst".into()
            }]
        )
    }

    #[test]
    fn variable_expansions_are_placed_in_target_and_can_include_other_expands() {
        let mut cfg: Root = Default::default();
        let mut tgt = Target::new("{{ target.t1 }}".to_string(), "dst".to_string());
        let t1val = "{{ xdg.home }}/t".to_owned();
        tgt.variables
            .insert("t1".to_owned(), Templated::new(t1val.clone()));
        cfg.targets.push(tgt);

        let acts = Actions::from_config(&cfg, &default_parse_context()).unwrap();
        assert_eq!(
            &acts.acts,
            &[Action::Link {
                ty: crate::config::LinkType::Hard,
                from: xdg_context().render(&t1val).unwrap().into(),
                to: "dst".into()
            }]
        )
    }
    #[test]
    fn actions_on_link_only_expands_to_links() {
        let cfg = serde_yaml::from_str(
            r"
                                       targets: [ { from: ./src, to: ./dst } ]
        ",
        )
        .unwrap();
        let acts = Actions::from_config(&cfg, &default_parse_context()).unwrap();
        assert_eq!(
            &acts.acts,
            &[Action::Link {
                ty: crate::config::LinkType::Hard,
                from: "./src".into(),
                to: "./dst".into()
            }]
        )
    }
}
