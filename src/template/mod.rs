use std::collections::HashMap;

mod parse;

pub struct Context {
    vars: HashMap<String, String>,
}

pub struct Templated {}
