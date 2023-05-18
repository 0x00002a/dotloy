use std::collections::HashMap;

use serde::Deserialize;

use crate::template::Templated;

#[derive(Deserialize, Debug, PartialEq, Eq, Default)]
pub struct Root {
    pub targets: Vec<Target>,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct Target {
    #[serde(rename = "from")]
    pub path: Templated<String>,
    #[serde(default)]
    pub variables: HashMap<String, String>,
    #[serde(rename = "to")]
    pub target_location: Templated<String>,
    pub link_type: Option<LinkType>,
}

impl Target {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn verify_config(cfg: &str, expected: &Root) {
        let parsed: Root = serde_yaml::from_str(cfg).expect("failed to parse config");
        assert_eq!(&parsed, expected);
    }

    /*
    #[test]
    fn parse_basic_config() {
        verify_config(
            r"

        ",
            &Root {
                targets: vec![Target {
                    path: "p1",
                    variables: Default::default(),
                    target_location: "{{ xdg.home }}",
                }],
            },
        );
    }*/
}
