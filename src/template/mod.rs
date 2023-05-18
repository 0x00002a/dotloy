use std::collections::HashMap;

use thiserror::Error;

mod parse;

#[derive(Debug, Default)]
pub struct Context {
    vars: HashMap<String, String>,
}

impl Context {
    pub fn new(vars: HashMap<String, String>) -> Self {
        Self { vars }
    }
    pub fn with_define(mut self, name: String, value: String) -> Self {
        self.define(name, value);
        self
    }

    pub fn define(&mut self, name: String, value: String) -> &mut Self {
        self.vars.insert(name, value);
        self
    }

    pub fn render(&self, input: &str) -> Result<String, parse::Error> {
        let tokens = parse::tokenize(input)?;

        todo!()
    }
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Parse(#[from] parse::Error),
}

#[cfg(test)]
mod tests {
    use super::Context;

    #[test]
    fn context_render_replaces_input_vars() {
        let ctx = Context::default().with_define("tvar".to_owned(), "expanded".to_owned());
        let out = ctx.render("{{ tvar }}/smth").unwrap();
        assert_eq!(out, "expanded/smth");
    }
}
