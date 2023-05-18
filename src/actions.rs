use std::{collections::HashMap, io::Write, path::PathBuf, rc::Rc};

use serde::Deserialize;

use crate::{
    config::{self, LinkType},
    template::{self, Templated, Variable},
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
        target: ResourceLocation,
        output: ResourceLocation,
    },
}

#[derive(Debug, Clone)]
enum ResourceHandle {
    MemStr(String),
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
}

#[derive(Clone)]
pub struct Actions {
    acts: Vec<Action>,
    resources: ResourceStore,
}

impl Actions {
    pub fn from_config(
        cfg: &config::Root,
        engine: &template::Context,
    ) -> Result<Self, template::Error> {
        let mut resources = ResourceStore::new();
        let mut acts = Vec::new();
        for target in &cfg.targets {
            let mut engine = engine.clone();
            for (var, val) in &target.variables {
                engine.define(Variable::from_str(var), val.to_owned());
            }
            let src_path: PathBuf = target.path.render(&engine)?.parse().unwrap();
            let dst_path: PathBuf = target.target_location.render(&engine)?.parse().unwrap();
            let src = ResourceLocation::Path(src_path.clone());
            let dst = ResourceLocation::Path(dst_path.clone());
            if src_path.extension() == Some("in".as_ref()) {
                let template_dst = resources.define_mem();
                acts.push(Action::TemplateExpand {
                    target: src.clone(),
                    output: template_dst.clone(),
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
        default_parse_context,
    };

    use super::Actions;

    #[test]
    fn actions_with_template_does_copy() {
        let cfg = serde_yaml::from_str(
            r"
                                       targets: [ { path: ./src.in, to: ./dst } ]
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
                    output: target.clone()
                },
                Action::Copy {
                    from: target.clone(),
                    to: ResourceLocation::Path("./dst".into())
                }
            ]
        )
    }
    #[test]
    fn actions_on_link_only_expands_to_links() {
        let cfg = serde_yaml::from_str(
            r"
                                       targets: [ { path: ./src, to: ./dst } ]
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
