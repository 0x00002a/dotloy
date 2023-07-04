use std::collections::HashMap;

use super::Templated;
use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq, Eq, Default)]
pub struct Root {
    /// Global variables. Accessible under `config` namespace
    #[serde(default, flatten)]
    pub shared: MultiScopedOptions,
    /// Targets to deploy
    pub targets: Vec<Target>,
}

#[derive(Deserialize, Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Windows,
    #[serde(rename = "macos")]
    MacOs,
    Linux,
    /// Testing platform that will never be matched by the current one
    #[cfg(test)]
    #[doc(hidden)]
    Test,
}

#[derive(Deserialize, Debug, PartialEq, Eq, Hash, Clone)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

#[derive(Deserialize, Debug, PartialEq, Eq, Default, Clone)]
pub struct MultiScopedOptions {
    #[serde(default)]
    pub variables: HashMap<String, Templated<String>>,
    #[serde(default)]
    pub runs_on: Option<OneOrMany<Platform>>,
}

#[derive(Deserialize, Debug, PartialEq, Eq, Clone, Copy, Default)]
#[serde(rename_all = "snake_case")]
pub enum DeployType {
    #[default]
    Auto,
    Copy,
    #[serde(untagged)]
    Link(LinkType),
}

#[derive(Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Target {
    /// Local path
    ///
    /// Config name: `from`
    #[serde(rename = "from")]
    pub path: Templated<String>,
    /// Target specific variables
    ///
    /// Accessible under `target` namespace
    #[serde(default, flatten)]
    pub shared: MultiScopedOptions,
    /// Location to deploy to
    ///
    /// Config name: `to`
    #[serde(rename = "to")]
    pub target_location: Templated<String>,
    /// Explicit link type to use.
    ///
    /// If not specified defaults to [`Hard`](LinkType::Hard) for files and
    /// [`Soft`](LinkType::Soft) for directories
    #[serde(default)]
    pub link_type: DeployType,
    /// Explicit option to expand template or not
    ///
    /// By default it will only be treated as a template if `from` ends with `.in`
    #[serde(default, rename = "template")]
    pub is_template: Option<bool>,
}

impl Target {
    #[cfg(test)]
    pub fn new(path: String, target_location: String) -> Self {
        Self {
            path: Templated::new(path),
            shared: Default::default(),
            target_location: Templated::new(target_location),
            link_type: Default::default(),
            is_template: None,
        }
    }
}

impl MultiScopedOptions {
    pub fn is_platform_supported(&self, target: Platform) -> bool {
        match &self.runs_on {
            Some(OneOrMany::One(p)) => *p == target,
            Some(OneOrMany::Many(ps)) => ps.iter().any(|p| *p == target),
            None => true,
        }
    }
}
impl Platform {
    pub fn current() -> Option<Self> {
        match std::env::consts::OS {
            "linux" => Some(Self::Linux),
            "macos" => Some(Self::MacOs),
            "windows" => Some(Self::Windows),
            _ => None,
        }
    }
}

#[derive(Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum LinkType {
    Soft,
    Hard,
}
