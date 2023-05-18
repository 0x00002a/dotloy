use std::collections::HashMap;

use thiserror::Error;

mod parse;

#[derive(Debug, Default)]
pub struct Context {
    vars: HashMap<Variable, String>,
}

#[derive(Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Variable {
    segments: Vec<String>,
}
impl Variable {
    pub fn new(segments: Vec<String>) -> Self {
        Self { segments }
    }
    pub fn single(name: String) -> Self {
        Self::new(vec![name])
    }
    pub fn join(mut self, mut other: Self) -> Self {
        self.segments.append(&mut other.segments);
        self
    }
}
impl std::fmt::Display for Variable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.segments.join(".")))
    }
}

impl Context {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_define(mut self, var: Variable, value: String) -> Self {
        self.define(var, value);
        self
    }

    pub fn define(&mut self, var: Variable, value: String) -> &mut Self {
        self.vars.insert(var, value);
        self
    }

    pub fn render(&self, input: &str) -> Result<String> {
        let tokens = parse::tokenize(input)?;
        let rendered = tokens
            .into_iter()
            .map(|tkn| match tkn {
                parse::Token::Variable(v) => self
                    .vars
                    .get(&v)
                    .ok_or_else(|| Error::UnmatchedVariable { var: v })
                    .cloned(),
                parse::Token::Str(s) => Ok(s),
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(rendered.join(""))
    }
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Parse(#[from] parse::Error),
    #[error("unknown variable: {var}")]
    UnmatchedVariable { var: Variable },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn variable_can_used_as_namespaces() {
        let var = Variable::single("test".to_owned());
        let t1 = var.clone().join(Variable::single("t1".to_owned()));
        assert_eq!(t1, Variable::new(vec!["test".to_owned(), "t1".to_owned()]));
    }

    #[test]
    fn context_render_replaces_input_vars() {
        let ctx = Context::default()
            .with_define(Variable::single("tvar".to_owned()), "expanded".to_owned());
        let out = ctx.render("{{ tvar }}/smth").unwrap();
        assert_eq!(out, "expanded/smth");
    }
}
