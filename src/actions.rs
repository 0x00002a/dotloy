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

#[derive(Clone)]
enum Action {
    Link {
        ty: LinkType,
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
            for (var, val) in target.variables {
                engine.define(Variable::from_str(&var), val);
            }
            let src_path: PathBuf = target.path.render(engine)?.parse().unwrap();
            let dst_path: PathBuf = target.target_location.render(engine)?.parse().unwrap();
            let mut src = ResourceLocation::Path(src_path.clone());
            let mut dst = ResourceLocation::Path(dst_path.clone());
            if src_path.extension() == Some("in".as_ref()) {
                dst = resources.define_mem();
                acts.push(Action::TemplateExpand {
                    target: src.clone(),
                    output: dst.clone(),
                })
            }
            acts.push(Action::Link {
                ty: target.link_type.unwrap_or_else(|| {
                    if dst_path.is_dir() {
                        LinkType::Soft
                    } else {
                        LinkType::Hard
                    }
                }),
                from: src,
                to: dst,
            });
        }
        Ok(Self { acts, resources })
    }
}
