//! MK Sigma command syntax. Commands end with a period; keywords are case-insensitive and may be
//! abbreviated to their first three letters, as in the public SPSS Command Syntax Reference.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// `DESCRIPTIVES [VARIABLES=] var...`
    Descriptives { variables: Vec<String> },
    /// `DISPLAY DICTIONARY`
    DisplayDictionary,
}

#[derive(Debug, Error, PartialEq)]
pub enum SyntaxError {
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("{command}: expected {expected}")]
    Expected { command: &'static str, expected: &'static str },
    #[error("unexpected character {0:?}")]
    UnexpectedChar(char),
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Word(String),
    Equals,
    Slash,
}

/// Whether `word` names `keyword`: the whole keyword or a prefix of at least three letters.
fn is_keyword(word: &str, keyword: &str) -> bool {
    let n = word.len();
    n >= 3.min(keyword.len()) && n <= keyword.len() && keyword.get(..n).is_some_and(|k| k.eq_ignore_ascii_case(word))
}

/// Splits a program into commands (at a period that ends a line or the input) and lexes each one.
fn commands(src: &str) -> Result<Vec<Vec<Token>>, SyntaxError> {
    let mut out = Vec::new();
    let mut tokens = Vec::new();
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {}
            '=' => tokens.push(Token::Equals),
            '/' => tokens.push(Token::Slash),
            '.' if chars.peek().is_none_or(|n| n.is_whitespace()) => {
                if !tokens.is_empty() {
                    out.push(std::mem::take(&mut tokens));
                }
            }
            c if c.is_alphanumeric() || matches!(c, '@' | '#' | '$' | '_' | '.') => {
                let mut word = String::from(c);
                while let Some(&n) = chars.peek() {
                    let ends_command = n == '.' && {
                        let mut ahead = chars.clone();
                        ahead.next();
                        ahead.peek().is_none_or(|a| a.is_whitespace())
                    };
                    if ends_command || !(n.is_alphanumeric() || matches!(n, '@' | '#' | '$' | '_' | '.')) {
                        break;
                    }
                    word.push(n);
                    chars.next();
                }
                tokens.push(Token::Word(word));
            }
            c => return Err(SyntaxError::UnexpectedChar(c)),
        }
    }
    if !tokens.is_empty() {
        out.push(tokens);
    }
    Ok(out)
}

pub fn parse(src: &str) -> Result<Vec<Command>, SyntaxError> {
    commands(src)?.into_iter().map(|tokens| parse_command(&tokens)).collect()
}

fn parse_command(tokens: &[Token]) -> Result<Command, SyntaxError> {
    let (name, rest) = match tokens.split_first() {
        Some((Token::Word(w), rest)) => (w.as_str(), rest),
        _ => return Err(SyntaxError::UnknownCommand(String::new())),
    };
    if is_keyword(name, "DISPLAY") {
        return match rest {
            [] => Ok(Command::DisplayDictionary),
            [Token::Word(w)] if is_keyword(w, "DICTIONARY") => Ok(Command::DisplayDictionary),
            _ => Err(SyntaxError::Expected { command: "DISPLAY", expected: "DICTIONARY" }),
        };
    }
    if is_keyword(name, "DESCRIPTIVES") {
        let rest = match rest {
            [Token::Word(w), Token::Equals, rest @ ..] if is_keyword(w, "VARIABLES") => rest,
            rest => rest,
        };
        let variables: Vec<String> = rest
            .iter()
            .take_while(|t| **t != Token::Slash)
            .filter_map(|t| match t {
                Token::Word(w) => Some(w.clone()),
                _ => None,
            })
            .collect();
        if variables.is_empty() {
            return Err(SyntaxError::Expected { command: "DESCRIPTIVES", expected: "a variable list" });
        }
        return Ok(Command::Descriptives { variables });
    }
    Err(SyntaxError::UnknownCommand(name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc(vars: &[&str]) -> Command {
        Command::Descriptives { variables: vars.iter().map(|v| v.to_string()).collect() }
    }

    #[test]
    fn descriptives_with_and_without_the_subcommand() {
        assert_eq!(parse("DESCRIPTIVES VARIABLES=age income."), Ok(vec![desc(&["age", "income"])]));
        assert_eq!(parse("desc age income."), Ok(vec![desc(&["age", "income"])]));
        assert_eq!(parse("DES var=age.\nDESCRIPTIVES income"), Ok(vec![desc(&["age"]), desc(&["income"])]));
    }

    #[test]
    fn display_dictionary() {
        assert_eq!(parse("DISPLAY DICTIONARY."), Ok(vec![Command::DisplayDictionary]));
        assert_eq!(parse("disp dic."), Ok(vec![Command::DisplayDictionary]));
        assert!(parse("DISPLAY LABELS.").is_err());
    }

    #[test]
    fn keywords_need_three_letters() {
        assert!(is_keyword("DES", "DESCRIPTIVES"));
        assert!(!is_keyword("DE", "DESCRIPTIVES"));
        assert!(!is_keyword("DESCRIPTIVESX", "DESCRIPTIVES"));
    }

    #[test]
    fn errors() {
        assert_eq!(parse("FOO bar."), Err(SyntaxError::UnknownCommand("FOO".into())));
        assert!(matches!(parse("DESCRIPTIVES."), Err(SyntaxError::Expected { .. })));
        assert_eq!(parse("DESCRIPTIVES a & b."), Err(SyntaxError::UnexpectedChar('&')));
    }
}
