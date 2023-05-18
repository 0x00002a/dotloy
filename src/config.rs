use std::collections::HashMap;

use serde::Deserialize;

use crate::template::Templated;

#[derive(Deserialize, Debug, PartialEq, Eq, Default)]
pub struct Root {
    /// Global variables. Accessible under `config` namespace
    #[serde(default)]
    pub variables: HashMap<String, Templated<String>>,
    /// Targets to deploy
    pub targets: Vec<Target>,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct Target {
    /// Local path
    ///
    /// Config name: `from`
    #[serde(rename = "from")]
    pub path: Templated<String>,
    /// Target specific variables
    ///
    /// Accessible under `target` namespace
    #[serde(default)]
    pub variables: HashMap<String, Templated<String>>,
    /// Location to deploy to
    ///
    /// Config name: `to`
    #[serde(rename = "to")]
    pub target_location: Templated<String>,
    /// Explicit link type to use.
    ///
    /// If not specified defaults to [`Hard`](LinkType::Hard) for files and
    /// [`Soft`](LinkType::Soft) for directories
    pub link_type: Option<LinkType>,
}

impl Target {
    #[cfg(test)]
    pub fn new(path: String, target_location: String) -> Self {
        Self {
            path: Templated::new(path),
            variables: Default::default(),
            target_location: Templated::new(target_location),
            link_type: None,
        }
    }
}

#[derive(Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum LinkType {
    Soft,
    Hard,
}
