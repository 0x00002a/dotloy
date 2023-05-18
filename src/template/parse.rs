use lazy_static::lazy_static;
use regex::Regex;
use thiserror::Error;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("variable segment is empty")]
    EmptyVariableSegment { offset: usize },
}
fn parse_template_inner(input: &[char]) -> Option<Result<(Vec<String>, usize)>> {
    let mut head = 0;
    let mut segments = Vec::new();
    let mut buf = String::new();
    while head < input.len() {
        match input[head..=head + 1] {
            ['}', '}'] => {
                if buf.is_empty() {
                    return Some(Err(Error::EmptyVariableSegment { offset: head }));
                } else {
                    segments.push(buf);
                }
                return Some(Ok((segments, head + 2)));
            }
            _ => {}
        }
        match input[head] {
            '.' => {
                if buf.is_empty() {
                    return Some(Err(Error::EmptyVariableSegment { offset: head }));
                } else {
                    let mut emp = String::new();
                    std::mem::swap(&mut emp, &mut buf);
                    segments.push(emp);
                }
            }
            ' ' => {}
            ch => {
                buf.push(ch);
            }
        }
        head += 1;
    }
    None
}

pub fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut head = 0;
    let mut strbuf = String::new();
    let chars = input.chars().collect::<Vec<_>>();
    while head < input.len() {
        if head >= input.len().saturating_sub(1) {
            break;
        }
        let var = match chars[head..=head + 1].as_ref() {
            ['{', '{'] => match parse_template_inner(&chars[head + 2..]) {
                Some(Ok((var, len))) => {
                    head += len + 2;
                    Some(var)
                }
                Some(Err(e)) => return Err(e),
                None => None,
            },
            _ => None,
        };
        if let Some(var) = var {
            tokens.push(Token::Variable(var));
        } else {
            strbuf.push(chars[head]);
            head += 1;
        }
    }
    if !strbuf.is_empty() {
        tokens.push(Token::Str(strbuf));
    }
    Ok(tokens)
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
    fn parse_template_inner_parses_the_start_of_a_template() {
        let s = "some.txt }}h1";
        let cs = s.chars().collect::<Vec<_>>();
        let (var, offset) = parse_template_inner(&cs).unwrap().unwrap();
        assert_eq!(offset, s.len() - 2);
        assert_eq!(&var, &["some", "txt"]);
    }
    #[test]
    fn parsing_template_extracts_engine_samples() {
        let parsed = tokenize("{{ var }}etc").unwrap();
        assert_eq!(
            parsed.as_slice(),
            &[
                Token::Variable(vec!["var".to_owned()]),
                Token::Str("etc".to_owned())
            ]
        );
    }
}
