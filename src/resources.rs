use std::io::Write;
use std::{collections::HashMap, path::PathBuf};

use fs_err as fs;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Debug, Eq, Clone)]
#[serde(untagged)]
pub enum ResourceLocation {
    InMemory { id: Uuid },
    Path(PathBuf),
}
impl PartialEq for ResourceLocation {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InMemory { id: l_id }, Self::InMemory { id: r_id }) => l_id == r_id,
            (Self::Path(l0), Self::Path(r0)) => {
                match (fs::canonicalize(l0), fs::canonicalize(r0)) {
                    (Ok(l), Ok(r)) => l == r,
                    _ => l0 == r0,
                }
            }
            _ => false,
        }
    }
}
impl From<PathBuf> for ResourceLocation {
    fn from(value: PathBuf) -> Self {
        Self::Path(value)
    }
}

impl ResourceLocation {
    #[must_use]
    pub fn as_path(&self) -> Option<&PathBuf> {
        if let Self::Path(v) = self {
            Some(v)
        } else {
            None
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

#[derive(Debug, Clone)]
pub enum ResourceHandle {
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
pub struct ResourceStore {
    handles: HashMap<Uuid, ResourceHandle>,
}
impl ResourceStore {
    pub fn define(&mut self, handle: ResourceHandle) -> ResourceLocation {
        let id = Uuid::new_v4();
        self.handles.insert(id, handle);
        ResourceLocation::InMemory { id }
    }
    pub fn define_mem(&mut self) -> ResourceLocation {
        self.define(ResourceHandle::MemStr("".to_owned()))
    }
    pub fn set(&mut self, target: Uuid, value: ResourceHandle) {
        self.handles.insert(target, value);
    }
    #[cfg(test)]
    pub fn test_handles(&self) -> &HashMap<Uuid, ResourceHandle> {
        &self.handles
    }
    pub fn set_content(
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

    pub fn get(&self, target: Uuid) -> &ResourceHandle {
        &self.handles[&target]
    }
    pub fn get_content(&self, target: &ResourceLocation) -> std::io::Result<String> {
        match target {
            ResourceLocation::InMemory { id } => self.get(*id).content(),
            ResourceLocation::Path(p) => fs::read_to_string(p),
        }
    }
    pub fn append(&mut self, other: &mut ResourceStore) {
        self.handles.extend(other.handles.drain());
    }
}
