use thiserror::Error;

use super::Variable;

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
    if input.is_empty() {
        return Ok(Default::default());
    }
    let mut tokens = Vec::new();
    let mut head = 0;
    let mut strbuf = String::new();
    let chars = input.chars().collect::<Vec<_>>();
    while head < input.len() {
        if head >= input.len() {
            break;
        }
        if head == input.len() - 1 {
            strbuf.push(chars[head]);
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
            if !strbuf.is_empty() {
                let mut tmp = String::new();
                std::mem::swap(&mut tmp, &mut strbuf);
                tokens.push(Token::Str(tmp));
            }
            tokens.push(Token::Variable(Variable::new(var)));
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
    Variable(Variable),
    Str(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_with_equals_works() {
        let s = r"SOME_VAR={{ t1 }}
export THING=$SOME_VAR";
        let tkns = tokenize(s).unwrap();
        assert_eq!(
            tkns.as_slice(),
            &[
                Token::Str("SOME_VAR=".to_owned()),
                Token::Variable(Variable::single("t1".to_string())),
                Token::Str(
                    r"
export THING=$SOME_VAR"
                        .to_owned()
                )
            ]
        )
    }
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
                Token::Variable(Variable::new(vec!["var".to_owned()])),
                Token::Str("etc".to_owned())
            ]
        );
    }
}
