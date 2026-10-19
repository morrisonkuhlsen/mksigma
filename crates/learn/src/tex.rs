//! Formulas written in a small subset of LaTeX, read into the notation the Σ draws (`Math`):
//! `\frac{a}{b}`, `\sqrt{a}`, `\bar{x}`, `\sum_{i=1}^{n} …`, `x_{i}`, `x^{2}`, `\left( … \right)`,
//! `\text{…}`, Greek letters and the usual relations. Anything else is an error, so an article
//! can't show a formula the reader would see garbled.

use mksigma_output::math::Math;
use mksigma_output::math::build::*;

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Command(String),
    Open,
    Close,
    Sub,
    Sup,
    Char(char),
}

fn tokens(src: &str) -> Vec<Token> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        i += 1;
        match c {
            '\\' => {
                let mut name = String::new();
                while let Some(&c) = chars.get(i).filter(|c| c.is_ascii_alphabetic()) {
                    name.push(c);
                    i += 1;
                }
                // A symbol command, such as \{ or \,.
                if name.is_empty()
                    && let Some(&c) = chars.get(i)
                {
                    name.push(c);
                    i += 1;
                }
                out.push(Token::Command(name));
            }
            '{' => out.push(Token::Open),
            '}' => out.push(Token::Close),
            '_' => out.push(Token::Sub),
            '^' => out.push(Token::Sup),
            c if c.is_whitespace() => {}
            c => out.push(Token::Char(c)),
        }
    }
    out
}

struct Reader {
    tokens: Vec<Token>,
    at: usize,
}

/// Reads a formula; the error says what it couldn't read.
pub fn parse(src: &str) -> Result<Math, String> {
    let mut r = Reader { tokens: tokens(src), at: 0 };
    let math = r.sequence(&[])?;
    match r.tokens.get(r.at) {
        None => Ok(math),
        Some(t) => Err(format!("unexpected {t:?} in {src}")),
    }
}

impl Reader {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.at).cloned();
        self.at += 1;
        t
    }

    /// Atoms until the end, a `}` or a `\right`.
    fn sequence(&mut self, until: &[&str]) -> Result<Math, String> {
        let mut parts = Vec::new();
        while let Some(t) = self.peek() {
            match t {
                Token::Close => break,
                Token::Command(c) if until.contains(&c.as_str()) => break,
                _ => parts.push(self.scripted()?),
            }
        }
        Ok(if parts.len() == 1 { parts.remove(0) } else { row(parts) })
    }

    /// An atom with its `_` and `^`.
    fn scripted(&mut self) -> Result<Math, String> {
        let mut base = self.atom()?;
        loop {
            match self.peek() {
                Some(Token::Sub) => {
                    self.at += 1;
                    base = sub(base, self.argument()?);
                }
                Some(Token::Sup) => {
                    self.at += 1;
                    base = sup(base, self.argument()?);
                }
                _ => return Ok(base),
            }
        }
    }

    /// `{…}`, or a single atom.
    fn argument(&mut self) -> Result<Math, String> {
        if self.peek() == Some(&Token::Open) {
            self.at += 1;
            let inner = self.sequence(&[])?;
            return match self.next() {
                Some(Token::Close) => Ok(inner),
                _ => Err("a { without its }".into()),
            };
        }
        self.atom()
    }

    /// The raw text of `{…}`, for `\text`.
    fn words(&mut self) -> Result<String, String> {
        if self.next() != Some(Token::Open) {
            return Err("\\text needs {…}".into());
        }
        let mut out = String::new();
        loop {
            match self.next() {
                Some(Token::Close) => return Ok(out),
                Some(Token::Char(c)) => out.push(c),
                Some(Token::Command(c)) => out.push_str(&c),
                Some(Token::Sub) => out.push('_'),
                Some(Token::Sup) => out.push('^'),
                Some(Token::Open) | None => return Err("\\text{…} not closed".into()),
            }
        }
    }

    fn atom(&mut self) -> Result<Math, String> {
        match self.next() {
            None => Err("a formula that ends too early".into()),
            Some(Token::Open) => {
                let inner = self.sequence(&[])?;
                match self.next() {
                    Some(Token::Close) => Ok(inner),
                    _ => Err("a { without its }".into()),
                }
            }
            Some(Token::Char(c)) if c.is_ascii_digit() || c == '.' => {
                let mut number = c.to_string();
                while let Some(Token::Char(c)) = self.peek().filter(|t| matches!(t, Token::Char(c) if c.is_ascii_digit() || *c == '.')) {
                    number.push(*c);
                    self.at += 1;
                }
                Ok(text(&number))
            }
            Some(Token::Char(c)) if c.is_alphabetic() => {
                let mut name = c.to_string();
                while let Some(Token::Char(c)) = self.peek().filter(|t| matches!(t, Token::Char(c) if c.is_alphabetic())) {
                    name.push(*c);
                    self.at += 1;
                }
                Ok(var(&name))
            }
            Some(Token::Char('-')) => Ok(text("−")),
            Some(Token::Char(c)) if "+=<>,()/|!".contains(c) => Ok(text(&c.to_string())),
            Some(Token::Char(c)) => Err(format!("the character {c:?}")),
            Some(Token::Command(name)) => self.command(&name),
            Some(t) => Err(format!("unexpected {t:?}")),
        }
    }

    fn command(&mut self, name: &str) -> Result<Math, String> {
        let symbol = |s: &str| Ok(text(s));
        let greek = |s: &str| Ok(var(s));
        match name {
            "frac" => {
                let a = self.argument()?;
                let b = self.argument()?;
                Ok(frac(a, b))
            }
            "sqrt" => Ok(sqrt(self.argument()?)),
            "bar" | "overline" => Ok(bar(self.argument()?)),
            "text" | "mathrm" => Ok(text(&self.words()?)),
            "sum" => {
                let (mut from, mut to) = (None, None);
                loop {
                    match self.peek() {
                        Some(Token::Sub) => {
                            self.at += 1;
                            from = Some(self.argument()?);
                        }
                        Some(Token::Sup) => {
                            self.at += 1;
                            to = Some(self.argument()?);
                        }
                        _ => break,
                    }
                }
                let body = self.scripted()?;
                Ok(sum(from, to, body))
            }
            "left" => {
                // The delimiter after \left: only parentheses.
                if self.next() != Some(Token::Char('(')) {
                    return Err("\\left needs (".into());
                }
                let inner = self.sequence(&["right"])?;
                if self.next() != Some(Token::Command("right".into())) || self.next() != Some(Token::Char(')')) {
                    return Err("\\left( without \\right)".into());
                }
                Ok(paren(inner))
            }
            "cdot" => symbol("·"),
            "times" => symbol("×"),
            "pm" => symbol("±"),
            "approx" => symbol("≈"),
            "le" | "leq" => symbol("≤"),
            "ge" | "geq" => symbol("≥"),
            "ne" | "neq" => symbol("≠"),
            "to" | "rightarrow" => symbol("→"),
            "cdots" | "ldots" | "dots" => symbol("⋯"),
            "," | ";" | " " => symbol(" "),
            "alpha" => greek("α"),
            "beta" => greek("β"),
            "chi" => greek("χ"),
            "mu" => greek("μ"),
            "nu" => greek("ν"),
            "rho" => greek("ρ"),
            "sigma" => greek("σ"),
            other => Err(format!("the command \\{other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_notation_of_the_calculations() {
        let mean = parse(r"\bar{x} = \frac{\sum_{i=1}^{n} x_{i}}{n}").unwrap();
        assert_eq!(mean.to_latex(), "\\bar{x} = \\frac{\\sum_{i = 1}^{n} {x}_{i}}{n}");
        let t = parse(r"t = \frac{\bar{x}_{1} - \bar{x}_{2}}{SE}").unwrap();
        assert!(t.to_latex().contains("\\mathit{SE}"));
        let s = parse(r"s^{2} = \frac{\sum \left( x_{i} - \bar{x} \right)^{2}}{n - 1}").unwrap();
        assert!(s.to_latex().contains("\\left( {x}_{i} - \\bar{x} \\right)"), "{}", s.to_latex());
        assert_eq!(parse(r"\chi^{2} \approx 3.84").unwrap().to_latex(), "{\\chi}^{2} \\approx 3.84");
        assert_eq!(parse(r"p \le 0.05, \text{df}").unwrap().to_latex(), "p \\le 0.05 , \\text{df}");
        assert!(parse(r"\integral x").is_err());
        assert!(parse(r"\frac{a}{b").is_err());
    }
}
