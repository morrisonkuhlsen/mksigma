//! Splits a program into commands and each command into tokens. A command ends with a period
//! followed by whitespace or the end of the input; periods inside numbers, names and quoted
//! strings don't end it.

use crate::SyntaxError;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    /// A name, keyword or number (`age`, `FREQUENCIES`, `-1.5`, `F8.2`).
    Word(String),
    /// A quoted string, quotes removed (`'it''s'` is `it's`).
    Str(String),
    Equals,
    /// `/`: a subcommand separator, or division inside an expression.
    Slash,
    LParen,
    RParen,
    Comma,
    Plus,
    /// `-` between operands; a `-` that starts a number (`= -1`, `(-2`) is part of the word.
    Minus,
    Star,
    /// `**`
    Power,
    Lt,
    Le,
    Gt,
    Ge,
    /// `<>` or `~=`
    Ne,
    /// `&`
    And,
    /// `|`
    Or,
    /// `~`
    Not,
}

impl Token {
    /// Whether a `-` after this token is subtraction rather than the sign of a number.
    fn ends_operand(&self) -> bool {
        matches!(self, Token::Word(_) | Token::Str(_) | Token::RParen)
    }
}

/// One command's tokens and its source text, period included.
pub(crate) struct Lexed {
    pub(crate) tokens: Vec<Token>,
    pub(crate) source: String,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '@' | '#' | '$' | '_' | '.')
}

pub(crate) fn commands(src: &str) -> Result<Vec<Lexed>, SyntaxError> {
    let mut out = Vec::new();
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;
    let mut chars = src.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        let ends_command = |chars: &std::iter::Peekable<std::str::CharIndices>| chars.clone().peek().is_none_or(|(_, n)| n.is_whitespace());
        if c == '.' && ends_command(&chars) {
            if let Some(s) = start.take() {
                out.push(Lexed { tokens: std::mem::take(&mut tokens), source: src.get(s..=i).unwrap_or_default().to_string() });
            }
            continue;
        }
        start.get_or_insert(i);
        match c {
            '=' => tokens.push(Token::Equals),
            '+' => tokens.push(Token::Plus),
            '*' if chars.peek().is_some_and(|(_, n)| *n == '*') => {
                chars.next();
                tokens.push(Token::Power);
            }
            '*' => tokens.push(Token::Star),
            '&' => tokens.push(Token::And),
            '|' => tokens.push(Token::Or),
            '<' | '>' | '~' => {
                let next = chars.peek().map(|(_, n)| *n);
                let token = match (c, next) {
                    ('<', Some('=')) => Token::Le,
                    ('>', Some('=')) => Token::Ge,
                    ('<', Some('>')) | ('~', Some('=')) => Token::Ne,
                    ('<', _) => Token::Lt,
                    ('>', _) => Token::Gt,
                    _ => Token::Not,
                };
                if matches!(token, Token::Le | Token::Ge | Token::Ne) {
                    chars.next();
                }
                tokens.push(token);
            }
            '-' if tokens.last().is_some_and(Token::ends_operand) || !chars.peek().is_some_and(|(_, n)| n.is_ascii_digit() || *n == '.') => {
                tokens.push(Token::Minus);
            }
            '/' => tokens.push(Token::Slash),
            '(' => tokens.push(Token::LParen),
            ')' => tokens.push(Token::RParen),
            ',' => tokens.push(Token::Comma),
            '\'' | '"' => {
                let mut text = String::new();
                loop {
                    match chars.next() {
                        Some((_, q)) if q == c => {
                            // A doubled quote stands for the quote itself.
                            if chars.peek().is_some_and(|(_, n)| *n == c) {
                                chars.next();
                                text.push(c);
                            } else {
                                break;
                            }
                        }
                        Some((_, ch)) => text.push(ch),
                        None => return Err(SyntaxError::UnterminatedString),
                    }
                }
                tokens.push(Token::Str(text));
            }
            c if is_word_char(c) || (c == '-' && chars.peek().is_some_and(|(_, n)| n.is_ascii_digit() || *n == '.')) => {
                let mut word = String::from(c);
                while let Some(&(_, n)) = chars.peek() {
                    let period_ends = n == '.' && {
                        let mut ahead = chars.clone();
                        ahead.next();
                        ahead.peek().is_none_or(|(_, a)| a.is_whitespace())
                    };
                    if period_ends || !is_word_char(n) {
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
    if let Some(s) = start {
        out.push(Lexed { tokens, source: src.get(s..).unwrap_or_default().trim_end().to_string() });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(src: &str) -> Vec<Token> {
        commands(src).unwrap().into_iter().flat_map(|c| c.tokens).collect()
    }

    #[test]
    fn strings_numbers_and_punctuation() {
        use Token::*;
        let w = |s: &str| Word(s.into());
        assert_eq!(
            tokens("VALUE LABELS q -1 'Don''t know' 2.5 \"a. b\" /x (LO THRU 0, 9)."),
            [
                w("VALUE"),
                w("LABELS"),
                w("q"),
                // After a name, `-` could be subtraction; commands without expressions merge it back.
                Minus,
                w("1"),
                Str("Don't know".into()),
                w("2.5"),
                Str("a. b".into()),
                Slash,
                w("x"),
                LParen,
                w("LO"),
                w("THRU"),
                w("0"),
                Comma,
                w("9"),
                RParen
            ]
        );
    }

    #[test]
    fn source_of_each_command() {
        let c = commands("  DESC age.\nFREQ  sex /FORMAT=NOTABLE.\nDISPLAY").unwrap();
        let sources: Vec<&str> = c.iter().map(|c| c.source.as_str()).collect();
        assert_eq!(sources, ["DESC age.", "FREQ  sex /FORMAT=NOTABLE.", "DISPLAY"]);
    }

    #[test]
    fn operators_and_the_two_minuses() {
        use Token::*;
        let w = |s: &str| Word(s.into());
        assert_eq!(
            tokens("x = -1 + a-1 - (-2) ** 2 >= b <> c ~= d & ~e | f <= g < h > i."),
            [
                w("x"),
                Equals,
                w("-1"),
                Plus,
                w("a"),
                Minus,
                w("1"),
                Minus,
                LParen,
                w("-2"),
                RParen,
                Power,
                w("2"),
                Ge,
                w("b"),
                Ne,
                w("c"),
                Ne,
                w("d"),
                And,
                Not,
                w("e"),
                Or,
                w("f"),
                Le,
                w("g"),
                Lt,
                w("h"),
                Gt,
                w("i")
            ]
        );
        assert_eq!(tokens("a*b/c"), [w("a"), Star, w("b"), Slash, w("c")]);
        assert_eq!(tokens("- x"), [Minus, w("x")]);
    }

    #[test]
    fn unterminated_string() {
        assert!(matches!(commands("VARIABLE LABELS a 'oops."), Err(SyntaxError::UnterminatedString)));
    }
}
