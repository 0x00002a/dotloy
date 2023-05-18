use std::collections::HashMap;

use thiserror::Error;

mod parse;

pub struct Context {
    vars: HashMap<String, String>,
}

pub struct Templated {}
