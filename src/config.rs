use std::collections::HashMap;

use serde::Deserialize;

use crate::template::Templated;

#[derive(Deserialize, Debug, PartialEq, Eq, Default)]
pub struct Root {
    #[serde(default)]
    pub variables: HashMap<String, Templated<String>>,
    pub targets: Vec<Target>,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct Target {
    #[serde(rename = "from")]
    pub path: Templated<String>,
    #[serde(default)]
    pub variables: HashMap<String, Templated<String>>,
    #[serde(rename = "to")]
    pub target_location: Templated<String>,
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
