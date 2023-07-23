use fs_err as fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::{
    abspath::AbsPathBuf,
    config::{self, DeployType, LinkType, Platform},
    define_variables,
    resources::{ResourceHandle, ResourceLocation, ResourceStore},
    vars,
};
use handybars::{self};

#[derive(Clone, PartialEq, Eq, Debug)]
enum Action {
    Link {
        ty: LinkType,
        from: AbsPathBuf,
        to: AbsPathBuf,
    },
    Copy {
        from: ResourceLocation,
        to: ResourceLocation,
    },
    MkDir {
        path: AbsPathBuf,
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
    pub fn dependency(&self) -> Option<ResourceLocation> {
        match self {
            Action::Link { from, .. } => Some(ResourceLocation::Path(from.to_owned())),
            Action::Copy { from, .. } => Some(from.to_owned()),
            Action::TemplateExpand { target, .. } => Some(target.to_owned()),
            Action::MkDir { .. } => None,
        }
    }
    pub fn output(&self) -> ResourceLocation {
        match self {
            Action::Link { to, .. } => ResourceLocation::Path(to.to_owned()),
            Action::Copy { to, .. } => to.to_owned(),
            Action::MkDir { path } => ResourceLocation::Path(path.to_owned()),
            Action::TemplateExpand { output, .. } => output.to_owned(),
        }
    }

    pub fn configure_watcher(&self, watcher: &mut dyn notify::Watcher) -> notify::Result<()> {
        let src = match self {
            Action::Link { .. } | Action::MkDir { .. } => None,
            Action::Copy { from, .. } => from.as_path(),
            Action::TemplateExpand { target, .. } => target.as_path(),
        };
        if let Some(src) = src {
            watcher.watch(
                if !src.is_dir() {
                    src.parent().unwrap()
                } else {
                    src
                },
                notify::RecursiveMode::NonRecursive,
            )?;
        }
        Ok(())
    }

    /// Returns `true` if the action is [`Copy`].
    ///
    /// [`Copy`]: Action::Copy
    #[must_use]
    #[cfg(test)]
    fn is_copy(&self) -> bool {
        matches!(self, Self::Copy { .. })
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

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Template(#[from] handybars::Error),
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
    #[error("No actions to perform, did you not define any targets in your config?")]
    NoActions,
}

#[derive(Clone, Debug, Default)]
struct ActionsBuilder {
    acts: Vec<Action>,
    res: ResourceStore,
}
impl ActionsBuilder {
    fn copy(
        &mut self,
        from: impl Into<ResourceLocation>,
        to: impl Into<ResourceLocation>,
    ) -> &mut Self {
        self.acts.push(Action::Copy {
            from: from.into(),
            to: to.into(),
        });
        self
    }
    fn link(
        &mut self,
        from: impl AsRef<Path>,
        to: impl AsRef<Path>,
        ty: LinkType,
    ) -> std::io::Result<&mut Self> {
        self.acts.push(Action::Link {
            ty,
            from: AbsPathBuf::new(from)?,
            to: AbsPathBuf::new(to)?,
        });
        Ok(self)
    }
    fn template(
        &mut self,
        ctx: handybars::Context<'static>,
        src: impl Into<ResourceLocation>,
        dst: impl Into<ResourceLocation>,
    ) -> &mut Self {
        self.acts.push(Action::TemplateExpand {
            ctx,
            target: src.into(),
            output: dst.into(),
        });
        self
    }
    fn template_expand(
        &mut self,
        ctx: handybars::Context<'static>,
        src: impl AsRef<Path>,
        dst: impl AsRef<Path>,
    ) -> std::io::Result<&mut Self> {
        let resource = self.res.define_mem();
        self.template(
            ctx,
            ResourceLocation::Path(AbsPathBuf::new(src)?),
            resource.clone(),
        )
        .copy(resource, ResourceLocation::Path(AbsPathBuf::new(dst)?));
        Ok(self)
    }
    fn mkdir(&mut self, dir: impl AsRef<Path>) -> std::io::Result<&mut Self> {
        self.acts.push(Action::MkDir {
            path: AbsPathBuf::new(dir)?,
        });
        Ok(self)
    }

    fn build(self) -> Actions {
        Actions {
            acts: self.acts,
            resources: self.res,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Actions {
    acts: Vec<Action>,
    resources: ResourceStore,
}

impl Actions {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn append(&mut self, other: &mut Actions) {
        self.acts.append(&mut other.acts);
        self.resources.append(&mut other.resources);
    }

    pub fn run(&self, dry: bool) -> Result<()> {
        if self.acts.is_empty() {
            return Err(Error::NoActions);
        }
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
    pub fn configure_watcher(&self, watcher: &mut dyn notify::Watcher) -> notify::Result<()> {
        for act in &self.acts {
            act.configure_watcher(watcher)?;
        }
        Ok(())
    }
    /// Get all the paths that the filesystem uses
    pub fn file_roots(&self) -> impl Iterator<Item = AbsPathBuf> + '_ {
        self.acts
            .iter()
            .filter_map(|act| Some(act.dependency()?.as_path()?.to_owned()))
    }
    pub fn dependents_of(&self, roots: Vec<ResourceLocation>) -> Self {
        let mut todo = roots;
        let mut dependents: Vec<Action> = Vec::new();
        while let Some(resource) = todo.pop() {
            let to_add = self
                .acts
                .iter()
                .filter(|a| a.dependency().as_ref() == Some(&resource) && !dependents.contains(a))
                .cloned()
                .collect::<Vec<_>>();
            for dep in to_add {
                todo.push(dep.output());
                dependents.push(dep);
            }
        }
        Self {
            acts: dependents,
            resources: self.resources.clone(),
        }
    }
    pub fn from_config(cfg: &config::Root, engine: &handybars::Context<'static>) -> Result<Self> {
        let mut engine = engine.clone();
        let mut builder = ActionsBuilder::default();
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
            if !src_path.exists() {
                return Err(Error::SourceDoesNotExist {
                    path: src_path.to_string_lossy().into_owned(),
                });
            }
            let dst_path: PathBuf = target.target_location.render(&engine)?.parse().unwrap();
            if let Some(p) = dst_path.parent() {
                if !p.exists() {
                    builder.mkdir(p)?;
                }
            }
            let is_template = target
                .is_template
                .unwrap_or_else(|| src_path.extension() == Some("in".as_ref()));
            if is_template {
                builder.template_expand(engine, src_path, dst_path)?;
            } else {
                match target.link_type {
                    DeployType::Copy => {
                        builder.copy(AbsPathBuf::new(src_path)?, AbsPathBuf::new(dst_path)?);
                    }
                    DeployType::Auto => {
                        let ty = if fs::canonicalize(&src_path)?.is_dir() {
                            LinkType::Soft
                        } else {
                            LinkType::Hard
                        };
                        builder.link(src_path, dst_path, ty)?;
                    }
                    DeployType::Link(ty) => {
                        builder.link(src_path, dst_path, ty)?;
                    }
                }
            }
        }
        Ok(builder.build())
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use assert_matches::assert_matches;
    use fs_err as fs;

    use itertools::Itertools;
    use tempdir::TempDir;

    use crate::{
        abspath::AbsPathBuf,
        actions::{Action, ResourceLocation},
        config::{Root, Target},
        default_parse_context, test_data_path, xdg_context, Templated,
    };
    use handybars::{Context, Variable};

    use super::{Actions, ActionsBuilder};

    #[test]
    fn explicit_is_template_causes_expansion_even_if_not_ending_with_in() {
        let src = test_data_path().join("softlinks.yaml");
        let dst = test_data_path().join("softlinks-out.yaml");
        let mut cfg = Root::default();
        let mut tgt = Target::new(
            src.to_string_lossy().into_owned(),
            dst.to_string_lossy().into_owned(),
        );
        tgt.is_template = Some(true);
        cfg.targets.push(tgt);
        let acts = Actions::from_config(&cfg, &default_parse_context()).unwrap();
        assert_matches!(acts.acts.as_slice(), [Action::TemplateExpand { .. }, ..])
    }

    #[test]
    fn resource_locations_are_equal_based_on_canonical_path() {
        assert_eq!(
            ResourceLocation::Path(AbsPathBuf::new("./src/..").unwrap()),
            ResourceLocation::Path(AbsPathBuf::new(".").unwrap())
        );
    }
    #[test]
    fn dependents_filters_all_actions_that_depend_on_resource() {
        let mut b = ActionsBuilder::default();
        let resources = (0..10).map(|_| b.res.define_mem()).collect::<Vec<_>>();
        let mut dests = Vec::new();
        for res in &resources {
            let dst = b.res.define_mem();
            dests.push(dst.clone());
            b.copy(res.to_owned(), dst);
        }
        let acts = b.build();
        for roots in resources.into_iter().powerset() {
            let root_len = roots.len();
            let deps = acts.dependents_of(roots);
            assert_eq!(deps.acts.len(), root_len);
            for act in deps.acts {
                assert!(act.is_copy());
            }
        }
    }

    #[test]
    fn actions_with_template_does_copy() {
        let src = AbsPathBuf::new(test_data_path().join("actions_with_test_data.in")).unwrap();
        let dst = AbsPathBuf::new(test_data_path().join("actions_with_test_data")).unwrap();
        let mut cfg = Root::default();
        cfg.targets.push(Target::new(
            src.to_string_lossy().into_owned(),
            dst.to_string_lossy().into_owned(),
        ));
        let acts = Actions::from_config(&cfg, &default_parse_context()).unwrap();
        let target = ResourceLocation::InMemory {
            id: acts
                .resources
                .test_handles()
                .keys()
                .next()
                .unwrap()
                .to_owned(),
        };
        assert_eq!(
            &acts.acts,
            &[
                Action::TemplateExpand {
                    target: ResourceLocation::Path(src),
                    output: target.clone(),
                    ctx: default_parse_context(),
                },
                Action::Copy {
                    from: target,
                    to: ResourceLocation::Path(dst)
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
                to: AbsPathBuf::new(xdg_context().render(&t1val).unwrap()).unwrap(),
                from: AbsPathBuf::new("src/actions.rs").unwrap()
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
                    path: "/home/nonexistant".try_into().unwrap(),
                },
                Action::Link {
                    ty: crate::config::LinkType::Hard,
                    from: "src/actions.rs".try_into().unwrap(),
                    to: "/home/nonexistant/hello.txt".try_into().unwrap()
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
                to: xdg_context().render(&t1val).unwrap().try_into().unwrap(),
                from: "src/actions.rs".try_into().unwrap()
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
                from: "src/actions.rs".try_into().unwrap(),
                to: "./dst".try_into().unwrap()
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

    struct TestDataMgr {
        acts: Actions,
        _ctx: Context<'static>,
        dir: TempDir,
    }
    impl TestDataMgr {
        fn resolve_path(&self, p: &Path) -> PathBuf {
            self.dir.path().join(p)
        }
        fn new(name: &'static str) -> Self {
            let (acts, ctx, dir) = load_test_data(name);
            Self {
                acts,
                _ctx: ctx,
                dir,
            }
        }
    }

    fn load_test_data(name: &'static str) -> (Actions, Context, TempDir) {
        let data = fs::read_to_string(format!("test_data/{name}.yaml")).unwrap();
        let cfg: Root = serde_yaml::from_str(&data).unwrap();
        let (ctx, dir) = test_ctx_with_dir(name);
        let acts = Actions::from_config(&cfg, &ctx).unwrap();
        (acts, ctx, dir)
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

    #[test]
    fn explicit_copying_link_type() {
        let mgr = TestDataMgr::new("copying");
        mgr.acts.run(false).unwrap();
        let created = fs::symlink_metadata(mgr.resolve_path("actions.rs".as_ref())).unwrap();
        assert!(created.is_file());
    }
    #[test]
    fn trying_to_run_an_empty_actions_is_an_error() {
        let acts = Actions::new();
        assert_matches!(acts.run(false), Err(crate::actions::Error::NoActions));
    }
}
