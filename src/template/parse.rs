use thiserror::Error;

use super::Variable;

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum ErrorType {
    #[error("variable segment is empty")]
    EmptyVariableSegment,
}

#[derive(Error, Debug)]
pub struct Error {
    pub offset: (usize, usize),
    #[source]
    pub ty: ErrorType,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (col, line) = self.offset;
        write!(
            f,
            "{} at line {line} column {col}",
            self.ty,
            line = line + 1,
            col = col + 1
        )
    }
}

impl Error {
    pub fn new(offset: (usize, usize), ty: ErrorType) -> Self {
        Self { offset, ty }
    }
    pub fn add_offset(mut self, offset: (usize, usize)) -> Self {
        self.offset.0 += offset.0;
        self.offset.1 += offset.1;
        self
    }
}
fn parse_template_inner(input: &[char]) -> Option<Result<(Vec<String>, usize)>> {
    let mut head = 0;
    let mut segments = Vec::new();
    let mut buf = String::new();
    let mut row = 0;
    let mut col = 0;
    while head < input.len() {
        let offset = (col as usize, row as usize);
        match input[head..=head + 1] {
            ['}', '}'] => {
                if buf.is_empty() {
                    return Some(Err(Error::new(offset, ErrorType::EmptyVariableSegment)));
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
                    return Some(Err(Error::new(offset, ErrorType::EmptyVariableSegment)));
                } else {
                    let mut emp = String::new();
                    std::mem::swap(&mut emp, &mut buf);
                    segments.push(emp);
                }
            }
            ' ' => {}
            '\n' => {
                row += 1;
                col = -1;
            }
            ch => {
                buf.push(ch);
            }
        }
        head += 1;
        col += 1;
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
    let mut row = 0;
    let mut col = 0;
    while head < input.len() {
        let pos = (col, row);
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
                Some(Err(e)) => return Err(e.add_offset((pos.0 + 2, pos.1))),
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
            if chars[head] == '\n' {
                col = 0;
                row += 1;
            } else {
                col += 1;
            }
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
