//! Splits a program into commands and each command into tokens. A command ends with a period
//! followed by whitespace or the end of the input, or at a blank line; periods inside numbers,
//! names and quoted strings don't end it. A command that starts with `*` or `COMMENT` is a
//! comment, skipped up to a period at the end of a line or a blank line (quotes and periods
//! inside it don't matter). `/* … */` is a comment anywhere, ending at `*/` or the end of the line.

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

type Chars<'a> = std::iter::Peekable<std::str::CharIndices<'a>>;

/// Whether the rest of the current line, after spaces, is empty: a period here ends a comment.
fn at_line_end(chars: &Chars) -> bool {
    let mut ahead = chars.clone();
    loop {
        match ahead.next() {
            None | Some((_, '\n')) => return true,
            Some((_, c)) if c.is_whitespace() => {}
            Some(_) => return false,
        }
    }
}

/// After a line break: whether the next line is blank, which ends a command or a comment.
fn blank_line_ahead(chars: &Chars) -> bool {
    let mut ahead = chars.clone();
    loop {
        match ahead.next() {
            Some((_, '\n')) => return true,
            Some((_, c)) if c.is_whitespace() => {}
            _ => return false,
        }
    }
}

/// Whether `word` is a number's mantissa followed by `E`, so a sign after it is the exponent's
/// (`1E-5`, `2.5e+3`), not subtraction.
fn wants_exponent_sign(word: &str) -> bool {
    let body = word.strip_prefix('-').unwrap_or(word);
    let Some(mantissa) = body.strip_suffix(['e', 'E']) else {
        return false;
    };
    mantissa.chars().next().is_some_and(|c| c.is_ascii_digit() || c == '.')
        && mantissa.chars().all(|c| c.is_ascii_digit() || c == '.')
        && mantissa.chars().any(|c| c.is_ascii_digit())
}

pub(crate) fn commands(src: &str) -> Result<Vec<Lexed>, SyntaxError> {
    let mut out = Vec::new();
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;
    let mut chars = src.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c.is_whitespace() {
            // A blank line ends the command, period or not.
            if c == '\n'
                && blank_line_ahead(&chars)
                && let Some(s) = start.take()
            {
                out.push(Lexed { tokens: std::mem::take(&mut tokens), source: src.get(s..i).unwrap_or_default().trim_end().to_string() });
            }
            continue;
        }
        if c == '/' && chars.peek().is_some_and(|(_, n)| *n == '*') {
            chars.next();
            while let Some(&(_, n)) = chars.peek() {
                if n == '\n' {
                    break;
                }
                chars.next();
                if n == '*' && chars.peek().is_some_and(|(_, a)| *a == '/') {
                    chars.next();
                    break;
                }
            }
            continue;
        }
        let ends_command = |chars: &Chars| chars.clone().peek().is_none_or(|(_, n)| n.is_whitespace());
        // Bytes, not a `str` slice: the text ahead may be any UTF-8 ("Comentário"), and byte 7
        // can fall inside a character.
        let comment = start.is_none()
            && (c == '*'
                || src.get(i..).is_some_and(|rest| {
                    rest.as_bytes().get(..7).is_some_and(|w| w.eq_ignore_ascii_case(b"COMMENT"))
                        && rest.get(7..).and_then(|r| r.chars().next()).is_some_and(char::is_whitespace)
                }));
        if comment {
            while let Some((_, n)) = chars.next() {
                if (n == '.' && at_line_end(&chars)) || (n == '\n' && blank_line_ahead(&chars)) {
                    break;
                }
            }
            continue;
        }
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
                    let exponent_sign = matches!(n, '+' | '-') && wants_exponent_sign(&word) && {
                        let mut ahead = chars.clone();
                        ahead.next();
                        ahead.peek().is_some_and(|(_, a)| a.is_ascii_digit())
                    };
                    if period_ends || !(is_word_char(n) || exponent_sign) {
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
    fn comments_are_skipped() {
        let c = commands("* A comment, with 'quotes and a 2.5 inside.\nDESC age.\nCOMMENT another\n  over lines.\n*.\nFREQ x.").unwrap();
        let sources: Vec<&str> = c.iter().map(|c| c.source.as_str()).collect();
        assert_eq!(sources, ["DESC age.", "FREQ x."]);
        assert_eq!(
            tokens("COMPUTE y = a * b."),
            [Token::Word("COMPUTE".into()), Token::Word("y".into()), Token::Equals, Token::Word("a".into()), Token::Star, Token::Word("b".into())]
        );
    }

    #[test]
    fn non_ascii_where_comment_is_checked() {
        // Issue #19: these panicked slicing inside "á" and "😀".
        let c = commands("Comentário sobre a análise.\nDESC age.").unwrap();
        let sources: Vec<&str> = c.iter().map(|c| c.source.as_str()).collect();
        assert_eq!(sources, ["Comentário sobre a análise.", "DESC age."]);
        assert!(matches!(commands("abcd😀 x."), Err(SyntaxError::UnexpectedChar('😀'))));
        assert_eq!(tokens("COMMENTÁRIO x."), [Token::Word("COMMENTÁRIO".into()), Token::Word("x".into())]);
        // COMMENT before a non-ASCII space is still a comment.
        assert!(commands("COMMENT\u{a0}x.\n").unwrap().is_empty());
    }

    #[test]
    fn any_utf8_lexes_without_panicking() {
        // Every suffix of a mixed text, so each multibyte character lands at every offset the
        // lexer looks ahead from; the result may be an error, never a panic.
        let text = "COMMENTá Comentário * 'olá' é ≥ − α 😀 x.\nCOMMENT😀 y.\nCOMMEN\u{301}T z. ñ";
        for (i, _) in text.char_indices() {
            for prefix in ["", " ", "a", "ab", "abc", "abcd", "abcde", "abcdef", "COMMENT", "COMMENT "] {
                let _ = commands(&format!("{prefix}{}", text.get(i..).unwrap_or_default()));
            }
        }
    }

    fn sources(src: &str) -> Vec<String> {
        commands(src).unwrap().into_iter().map(|c| c.source).collect()
    }

    #[test]
    fn a_blank_line_ends_a_command_or_a_comment() {
        // Issue #35: the FREQ was swallowed into the unterminated comment.
        assert_eq!(sources("* recode ages\n\nFREQ x."), ["FREQ x."]);
        assert_eq!(sources("COMMENT no period\n   \t\nDESC age."), ["DESC age."]);
        assert_eq!(sources("DESC age\n\nFREQ x."), ["DESC age", "FREQ x."]);
        assert_eq!(sources("DESC age\r\n\r\nFREQ x."), ["DESC age", "FREQ x."]);
        // One line break continues the command.
        assert_eq!(sources("FREQ x\n  /FORMAT=NOTABLE."), ["FREQ x\n  /FORMAT=NOTABLE."]);
    }

    #[test]
    fn a_comment_ends_at_a_period_at_the_end_of_a_line() {
        // Issue #35: "e.g." ended the comment, and "age into groups." was a command.
        assert_eq!(sources("* Recode e.g. age into groups.\nFREQ x."), ["FREQ x."]);
        assert_eq!(sources("* Two lines. Still the comment\n  until here.  \nDESC age."), ["DESC age."]);
        assert_eq!(sources("*.\nFREQ x."), ["FREQ x."]);
    }

    #[test]
    fn slash_star_comments_anywhere() {
        use Token::*;
        let w = |s: &str| Word(s.into());
        assert_eq!(tokens("FREQ x /* the age */."), [w("FREQ"), w("x")]);
        assert_eq!(tokens("COMPUTE y = x /* half */ / 2."), [w("COMPUTE"), w("y"), Equals, w("x"), Slash, w("2")]);
        // To the end of the line when there is no `*/`, and a line of its own makes no command.
        assert_eq!(sources("/* a note. with periods.\nDESC age. /* done */\nFREQ x /* no close\n ."), ["DESC age.", "FREQ x /* no close\n ."]);
        assert_eq!(tokens("a/*b*/*c"), [w("a"), Star, w("c")]);
    }

    #[test]
    fn signed_exponents_are_part_of_the_number() {
        use Token::*;
        let w = |s: &str| Word(s.into());
        assert_eq!(tokens("COMPUTE x = 1E-5 + 2.5e+3 - -1e-2."), [w("COMPUTE"), w("x"), Equals, w("1E-5"), Plus, w("2.5e+3"), Minus, w("-1e-2")]);
        // A name ending in E is still subtracted from.
        assert_eq!(
            tokens("x = rate-1 + e-1 + .e-1"),
            [w("x"), Equals, w("rate"), Minus, w("1"), Plus, w("e"), Minus, w("1"), Plus, w(".e"), Minus, w("1")]
        );
        assert_eq!(tokens("1E-x"), [w("1E"), Minus, w("x")]);
    }

    #[test]
    fn unterminated_string() {
        assert!(matches!(commands("VARIABLE LABELS a 'oops."), Err(SyntaxError::UnterminatedString)));
    }
}
