use fs_err as fs;
use std::{io::Write, path::PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::{
    config::{self, LinkType, Platform},
    define_variables, vars,
};
use handybars::{self};

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
    MkDir {
        path: PathBuf,
    },
    TemplateExpand {
        ctx: handybars::Context<'static>,
        target: ResourceLocation,
        output: ResourceLocation,
    },
}

impl Action {
    fn run(&self, res: &mut ResourceStore) -> Result<()> {
        match self {
            Action::MkDir { path } => Ok(fs::create_dir_all(path)?),
            Action::Link { ty, from, to } => {
                if let Ok(m) = fs::symlink_metadata(to) {
                    if !m.is_symlink() {
                        return Err(Error::TargetExists {
                            path: to.to_string_lossy().into_owned(),
                        });
                    } else if fs::canonicalize(to)? != fs::canonicalize(from)? {
                        return Err(Error::TargetSymlinksDiffer {
                            path: to.to_string_lossy().into_owned(),
                            ours: fs::canonicalize(from)?.to_string_lossy().into_owned(),
                            theirs: fs::canonicalize(to)?.to_string_lossy().into_owned(),
                        });
                    } else {
                        return Ok(());
                    }
                }
                match ty {
                    LinkType::Soft => Ok(symlink::symlink_auto(fs::canonicalize(from)?, to)?),
                    LinkType::Hard => {
                        assert!(from.is_file(), "tried to hardlink directory");
                        Ok(fs::hard_link(from, to)?)
                    }
                }
            }
            Action::Copy { from, to } => match from {
                ResourceLocation::InMemory { id: fid } => match to {
                    ResourceLocation::InMemory { id: tid } => {
                        res.set(*tid, res.get(*fid).clone());
                        Ok(())
                    }
                    loc => Ok(res.set_content(loc, res.get(*fid).clone())?),
                },
                ResourceLocation::Path(pf) => match to {
                    ResourceLocation::Path(pt) => {
                        fs::copy(pf, pt)?;
                        Ok(())
                    }
                    loc => Ok(res.set_content(loc, ResourceHandle::File(pf.to_owned()))?),
                },
            },
            Action::TemplateExpand {
                ctx,
                target,
                output,
            } => {
                let from = ctx.render(&res.get_content(target)?)?;
                res.set_content(output, ResourceHandle::MemStr(from))?;
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
            Action::MkDir { path } => {
                f.write_fmt(format_args!("mkdir {path}", path = path.to_string_lossy()))
            }
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
            ResourceLocation::InMemory { id } => {
                self.set(*id, value);
                Ok(())
            }
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
    Template(#[from] handybars::Error),
    #[cfg(not(test))]
    #[error("Source file does not exist: '{path}'")]
    SourceDoesNotExist { path: String },
    #[error("Target file '{path}' already exists")]
    TargetExists { path: String },
    #[error("Target file '{path}' is already a symlink and its source is different to ours ('{ours}' vs '{theirs}')")]
    TargetSymlinksDiffer {
        path: String,
        ours: String,
        theirs: String,
    },
    #[error("This config does not support the current platform")]
    ConfigDoesNotSupportPlatform,
    #[error("Terribly sorry, but dotloy doesn't support this platform/os")]
    UnsupportedPlatform,
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
            if !dry {
                match action.run(&mut res) {
                    Ok(_) => log::info!("{action}"),
                    Err(e) => log::error!("{action} failed. reason: {}", e),
                }
            } else {
                log::info!("{action}");
            }
        }
        Ok(())
    }
    pub fn from_config(cfg: &config::Root, engine: &handybars::Context<'static>) -> Result<Self> {
        let mut resources = ResourceStore::new();
        let mut acts = Vec::new();
        let mut engine = engine.clone();
        let curr_os = Platform::current().ok_or(Error::UnsupportedPlatform)?;
        if !cfg.shared.is_platform_supported(curr_os) {
            return Err(Error::ConfigDoesNotSupportPlatform);
        }
        define_variables(
            &mut engine,
            &vars::config_level(),
            cfg.shared.variables.iter(),
        )?;
        for target in &cfg.targets {
            if !target.shared.is_platform_supported(curr_os) {
                log::info!("skipping target that deploys '{tname}' since it doesn't support the current platform", tname = target.path.0);
                continue;
            }
            let mut engine = engine.clone();
            define_variables(
                &mut engine,
                &vars::target_level(),
                target.shared.variables.iter(),
            )?;
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
            if let Some(p) = dst_path.parent() {
                if !p.exists() {
                    acts.push(Action::MkDir { path: p.to_owned() });
                }
            }
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
                    ty: target.link_type.map(Ok::<_, Error>).unwrap_or_else(|| {
                        Ok(if fs::canonicalize(&src_path)?.is_dir() {
                            LinkType::Soft
                        } else {
                            LinkType::Hard
                        })
                    })?,
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
    use fs_err as fs;

    use tempdir::TempDir;

    use crate::{
        actions::{Action, ResourceLocation},
        config::{Root, Target},
        default_parse_context, xdg_context, Templated,
    };
    use handybars::{Context, Variable};

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
                    from: target,
                    to: ResourceLocation::Path("./dst".into())
                }
            ]
        )
    }

    #[test]
    fn variable_expansions_on_root_are_placed_in_config() {
        let mut cfg: Root = Default::default();
        let tgt = Target::new("src/actions.rs".to_string(), "{{ config.t1 }}".to_string());
        let t1val = "{{ xdg.home }}/t".to_owned();
        cfg.shared
            .variables
            .insert("t1".to_owned(), Templated::new(t1val.clone()));
        cfg.targets.push(tgt);

        let acts = Actions::from_config(&cfg, &default_parse_context()).unwrap();
        assert_eq!(
            &acts.acts,
            &[Action::Link {
                ty: crate::config::LinkType::Hard,
                to: xdg_context().render(&t1val).unwrap().into(),
                from: "src/actions.rs".into()
            }]
        )
    }

    #[test]
    fn trying_to_link_into_non_existant_dirs_creates_needed_ones() {
        let mut cfg: Root = Default::default();
        let tgt = Target::new(
            "src/actions.rs".to_string(),
            "/home/nonexistant/hello.txt".to_string(),
        );
        cfg.targets.push(tgt);

        let acts = Actions::from_config(&cfg, &default_parse_context()).unwrap();
        assert_eq!(
            &acts.acts,
            &[
                Action::MkDir {
                    path: "/home/nonexistant".into(),
                },
                Action::Link {
                    ty: crate::config::LinkType::Hard,
                    from: "src/actions.rs".into(),
                    to: "/home/nonexistant/hello.txt".into()
                }
            ]
        )
    }
    #[test]
    fn variable_expansions_are_placed_in_target_and_can_include_other_expands() {
        let mut cfg: Root = Default::default();
        let mut tgt = Target::new("src/actions.rs".to_string(), "{{ target.t1 }}".to_string());
        let t1val = "{{ xdg.home }}/t".to_owned();
        tgt.shared
            .variables
            .insert("t1".to_owned(), Templated::new(t1val.clone()));
        cfg.targets.push(tgt);

        let acts = Actions::from_config(&cfg, &default_parse_context()).unwrap();
        assert_eq!(
            &acts.acts,
            &[Action::Link {
                ty: crate::config::LinkType::Hard,
                to: xdg_context().render(&t1val).unwrap().into(),
                from: "src/actions.rs".into()
            }]
        )
    }
    #[test]
    fn actions_on_link_only_expands_to_links() {
        let cfg = serde_yaml::from_str(
            r"
                                       targets: [ { from: src/actions.rs, to: ./dst } ]
        ",
        )
        .unwrap();
        let acts = Actions::from_config(&cfg, &default_parse_context()).unwrap();
        assert_eq!(
            &acts.acts,
            &[Action::Link {
                ty: crate::config::LinkType::Hard,
                from: "src/actions.rs".into(),
                to: "./dst".into()
            }]
        )
    }
    fn test_ctx_with_dir(prefix: &str) -> (Context, tempdir::TempDir) {
        let dir = TempDir::new(prefix).unwrap();
        let ns = Variable::single("test");
        let ctx = default_parse_context().with_define(
            ns,
            handybars::Object::new()
                .with_property("dir", dir.path().to_string_lossy().into_owned())
                .with_property("data", "./test_data"),
        );
        (ctx, dir)
    }

    #[test]
    fn non_matching_platform_causes_target_to_be_skipped() {
        const DATA: &str = include_str!("../test_data/nonmatch_platform.yaml");
        let cfg: Root = serde_yaml::from_str(DATA).unwrap();
        let ctx = default_parse_context();
        let acts = Actions::from_config(&cfg, &ctx).unwrap();
        assert_eq!(acts.acts.as_slice(), &[]);
    }

    #[test]
    fn softlinks_work() {
        const DATA: &str = include_str!("../test_data/softlinks.yaml");
        let cfg: Root = serde_yaml::from_str(DATA).unwrap();
        let (ctx, dir) = test_ctx_with_dir("softlinks");
        let acts = Actions::from_config(&cfg, &ctx).unwrap();
        acts.run(false).unwrap();
        let created = fs::symlink_metadata(dir.path().join("softlink-folder")).unwrap();
        assert!(created.is_symlink());
    }
}
