use std::collections::HashMap;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Root {
    pub targets: Vec<Target>,
}

#[derive(Deserialize)]
pub struct Target {
    pub path: std::path::PathBuf,
    #[serde(default)]
    pub variables: HashMap<String, String>,
    #[serde(rename = "to")]
    pub target_location: std::path::PathBuf,
    pub link_type: Option<LinkType>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkType {
    Soft,
    Hard,
}
