use lazy_static::lazy_static;
use regex::Regex;

pub fn tokenize(input: &str) -> Vec<Token> {
    lazy_static! {
        static ref VAR_RE: Regex = Regex::new(r"\{\{\s*([a-z|A-Z](\.[a-z|A-Z])*)\s*}}").unwrap();
    }
    let mut tokens = Vec::new();
    let mut head = 0;
    let mut locations = VAR_RE.capture_locations();
    let mut strbuf = String::new();
    let input_b = input.chars().collect::<Vec<_>>();
    while head < input.len() {
        let next = VAR_RE.captures_read_at(&mut locations, input, head);
        match next {
            Some(m) => {
                if !strbuf.is_empty() {
                    let mut buf = String::new();
                    std::mem::swap(&mut buf, &mut strbuf);
                    tokens.push(Token::Str(buf));
                }
                let var = m.as_str().split(".").map(|s| s.to_owned()).collect();
                tokens.push(Token::Variable(var));
                head += m.len();
            }
            None => {
                strbuf.push(input_b[head]);
                head += 1;
            }
        }
    }
    tokens
}

#[derive(Debug, PartialEq, Eq)]
pub enum Token {
    Variable(Vec<String>),
    Str(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parsing_template_extracts_engine_samples() {
        let parsed = tokenize("{{ var }}etc");
        assert_eq!(
            parsed.as_slice(),
            &[
                Token::Variable(vec!["var".to_owned()]),
                Token::Str("etc".to_owned())
            ]
        );
    }
}
