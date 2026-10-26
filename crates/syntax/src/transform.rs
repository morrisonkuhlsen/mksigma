//! Data transformations: `COMPUTE`, `IF`, `RECODE`, `SELECT IF`.

use crate::expr::{self, Expr};
use crate::lexer::Token;
use crate::{Command, Literal, SyntaxError, is_keyword};

/// What a `RECODE` specification matches.
#[derive(Debug, Clone, PartialEq)]
pub enum RecodeFrom {
    Value(Literal),
    /// `low THRU high`; `None` is `LO`/`LOWEST` or `HI`/`HIGHEST`.
    Range(Option<f64>, Option<f64>),
    /// `MISSING`: system- or user-missing.
    Missing,
    /// `SYSMIS`
    SysMis,
    /// `ELSE`: anything not matched by an earlier specification.
    Else,
}

/// What a matched value becomes.
#[derive(Debug, Clone, PartialEq)]
pub enum RecodeTo {
    Value(Literal),
    SysMis,
    /// `COPY`: the input value itself.
    Copy,
}

/// One `(values = value)` specification.
#[derive(Debug, Clone, PartialEq)]
pub struct RecodeSpec {
    pub from: Vec<RecodeFrom>,
    pub to: RecodeTo,
}

/// `varlist (specs)... [INTO varlist]`
#[derive(Debug, Clone, PartialEq)]
pub struct RecodeGroup {
    pub variables: Vec<String>,
    pub specs: Vec<RecodeSpec>,
    pub into: Vec<String>,
}

fn word(t: Option<&Token>) -> Option<&str> {
    match t {
        Some(Token::Word(w)) => Some(w.as_str()),
        _ => None,
    }
}

/// `target = expression`
fn assignment(command: &'static str, tokens: &[Token]) -> Result<(String, Expr), SyntaxError> {
    match tokens {
        [Token::Word(target), Token::Equals, rest @ ..] if !rest.is_empty() => Ok((target.clone(), expr::parse(rest)?)),
        _ => Err(SyntaxError::Expected { command, expected: "target = expression" }),
    }
}

/// The tokens between a `(` at `tokens[0]` and its matching `)`, and what follows.
fn parenthesized<'a>(command: &'static str, tokens: &'a [Token]) -> Result<(&'a [Token], &'a [Token]), SyntaxError> {
    if tokens.first() != Some(&Token::LParen) {
        return Err(SyntaxError::Expected { command, expected: "(" });
    }
    let mut depth = 0usize;
    for (i, t) in tokens.iter().enumerate() {
        match t {
            Token::LParen => depth += 1,
            Token::RParen => {
                depth -= 1;
                if depth == 0 {
                    return Ok((tokens.get(1..i).unwrap_or_default(), tokens.get(i + 1..).unwrap_or_default()));
                }
            }
            _ => {}
        }
    }
    Err(SyntaxError::Expected { command, expected: ")" })
}

/// A number or string, with a leading `-` sign token allowed.
fn literal(tokens: &[Token]) -> Option<(Literal, usize)> {
    match tokens {
        [Token::Str(s), ..] => Some((Literal::Str(s.clone()), 1)),
        [Token::Minus, Token::Word(w), ..] => w.parse::<f64>().ok().map(|x| (Literal::Num(-x), 2)),
        [Token::Word(w), ..] if w.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '.') => w.parse().ok().map(|x| (Literal::Num(x), 1)),
        _ => None,
    }
}

fn number(tokens: &[Token]) -> Option<(f64, usize)> {
    match literal(tokens)? {
        (Literal::Num(x), n) => Some((x, n)),
        _ => None,
    }
}

fn recode_spec(tokens: &[Token]) -> Result<RecodeSpec, SyntaxError> {
    const CMD: &str = "RECODE";
    let bad = |expected| SyntaxError::Expected { command: CMD, expected };
    let at = tokens.iter().position(|t| *t == Token::Equals).ok_or(bad("= in (old = new)"))?;
    let (left, right) = (tokens.get(..at).unwrap_or_default(), tokens.get(at + 1..).unwrap_or_default());
    let mut from = Vec::new();
    let mut rest = left;
    while let Some(first) = rest.first() {
        if *first == Token::Comma {
            rest = rest.get(1..).unwrap_or_default();
            continue;
        }
        let w = word(Some(first)).map(str::to_ascii_uppercase);
        // A range: (LO | LOWEST | number) THRU (HI | HIGHEST | number).
        let (low, used) = match w.as_deref() {
            Some("LO" | "LOWEST") => (Some(None), 1),
            _ => match number(rest) {
                Some((x, n)) => (Some(Some(x)), n),
                None => (None, 0),
            },
        };
        if let Some(low) = low
            && word(rest.get(used)).is_some_and(|t| is_keyword(t, "THRU"))
        {
            let after = rest.get(used + 1..).unwrap_or_default();
            let (high, n) = match word(after.first()).map(str::to_ascii_uppercase).as_deref() {
                Some("HI" | "HIGHEST") => (None, 1),
                _ => number(after).map(|(x, n)| (Some(x), n)).ok_or(bad("a number or HI after THRU"))?,
            };
            from.push(RecodeFrom::Range(low, high));
            rest = after.get(n..).unwrap_or_default();
            continue;
        }
        let (item, n) = match w.as_deref() {
            Some("MISSING") => (RecodeFrom::Missing, 1),
            Some("SYSMIS") => (RecodeFrom::SysMis, 1),
            Some("ELSE") => (RecodeFrom::Else, 1),
            _ => literal(rest).map(|(l, n)| (RecodeFrom::Value(l), n)).ok_or(bad("values, ranges, MISSING, SYSMIS or ELSE"))?,
        };
        from.push(item);
        rest = rest.get(n..).unwrap_or_default();
    }
    if from.is_empty() {
        return Err(bad("old values before ="));
    }
    let to = match right {
        [Token::Word(w)] if w.eq_ignore_ascii_case("SYSMIS") => RecodeTo::SysMis,
        [Token::Word(w)] if w.eq_ignore_ascii_case("COPY") => RecodeTo::Copy,
        tokens => match literal(tokens) {
            Some((l, n)) if n == tokens.len() => RecodeTo::Value(l),
            _ => return Err(bad("one new value, SYSMIS or COPY after =")),
        },
    };
    Ok(RecodeSpec { from, to })
}

fn recode(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "RECODE";
    let mut groups = Vec::new();
    // Slashes inside parentheses can't occur (no division in RECODE), so splitting is safe.
    for group in rest.split(|t| *t == Token::Slash).filter(|g| !g.is_empty()) {
        let n = group.iter().take_while(|t| matches!(t, Token::Word(_))).count();
        let variables: Vec<String> = group.iter().take(n).filter_map(|t| word(Some(t)).map(str::to_string)).collect();
        if variables.is_empty() {
            return Err(SyntaxError::Expected { command: CMD, expected: "a variable list" });
        }
        let mut tokens = group.get(n..).unwrap_or_default();
        let mut specs = Vec::new();
        while tokens.first() == Some(&Token::LParen) {
            let (inside, after) = parenthesized(CMD, tokens)?;
            specs.push(recode_spec(inside)?);
            tokens = after;
        }
        if specs.is_empty() {
            return Err(SyntaxError::Expected { command: CMD, expected: "(old = new) specifications" });
        }
        let into = match tokens {
            [] => Vec::new(),
            [Token::Word(w), names @ ..]
                if w.eq_ignore_ascii_case("INTO") && names.iter().all(|t| matches!(t, Token::Word(_))) && !names.is_empty() =>
            {
                names.iter().filter_map(|t| word(Some(t)).map(str::to_string)).collect()
            }
            _ => return Err(SyntaxError::Expected { command: CMD, expected: "INTO and new variable names, or the end" }),
        };
        // With ALL or TO the counts are known once the lists are expanded (#36).
        let expands = |l: &[String]| l.iter().any(|w| w.eq_ignore_ascii_case("ALL") || w.eq_ignore_ascii_case("TO"));
        if !into.is_empty() && into.len() != variables.len() && !expands(&variables) && !expands(&into) {
            return Err(SyntaxError::Expected { command: CMD, expected: "as many INTO variables as variables to recode" });
        }
        groups.push(RecodeGroup { variables, specs, into });
    }
    if groups.is_empty() {
        return Err(SyntaxError::Expected { command: CMD, expected: "a variable list" });
    }
    Ok(Command::Recode(groups))
}

/// Parses a transformation command, if `name` starts one. These see the raw tokens: a `-`
/// before a number is subtraction or a sign depending on the expression.
pub(crate) fn parse(name: &str, rest: &[Token]) -> Option<Result<Command, SyntaxError>> {
    if is_keyword(name, "COMPUTE") {
        return Some(assignment("COMPUTE", rest).map(|(target, expr)| Command::Compute { target, expr }));
    }
    if is_keyword(name, "RECODE") {
        return Some(recode(rest));
    }
    if name.eq_ignore_ascii_case("IF") {
        return Some(parenthesized("IF", rest).and_then(|(cond, after)| {
            let cond = expr::parse(cond)?;
            assignment("IF", after).map(|(target, expr)| Command::If { cond, target, expr })
        }));
    }
    if is_keyword(name, "SELECT") && word(rest.first()).is_some_and(|w| w.eq_ignore_ascii_case("IF")) {
        return Some(expr::parse(rest.get(1..).unwrap_or_default()).map(Command::SelectIf));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::BinOp;
    use crate::parse;

    fn one(src: &str) -> Command {
        let mut c = parse(src).unwrap();
        c.remove(0)
    }

    #[test]
    fn compute_if_select() {
        assert_eq!(
            one("COMPUTE imc = peso / altura ** 2."),
            Command::Compute {
                target: "imc".into(),
                expr: Expr::Binary(
                    BinOp::Div,
                    Box::new(Expr::Var("peso".into())),
                    Box::new(Expr::Binary(BinOp::Pow, Box::new(Expr::Var("altura".into())), Box::new(Expr::Num(2.0))))
                )
            }
        );
        assert!(matches!(one("IF (idade >= 60) idoso = 1."), Command::If { target, .. } if target == "idoso"));
        assert!(matches!(one("SELECT IF (sexo = 'F' AND idade > 17)."), Command::SelectIf(Expr::Binary(BinOp::And, _, _))));
        assert!(parse("COMPUTE x.").is_err());
        assert!(parse("IF idade > 60 x = 1.").is_err());
    }

    #[test]
    fn recode() {
        use RecodeFrom::*;
        let c = one(
            "RECODE idade (LO THRU 17 = 1) (18 THRU 59=2) (60 THRU HI = 3) (MISSING = 9) INTO faixa /q1 q2 (1 2 = 0) (-1, 99 = SYSMIS) (ELSE = COPY).",
        );
        let Command::Recode(groups) = c else { panic!("not RECODE") };
        assert_eq!(groups[0].variables, ["idade"]);
        assert_eq!(groups[0].into, ["faixa"]);
        assert_eq!(groups[0].specs[0], RecodeSpec { from: vec![Range(None, Some(17.0))], to: RecodeTo::Value(Literal::Num(1.0)) });
        assert_eq!(groups[0].specs[2].from, [Range(Some(60.0), None)]);
        assert_eq!(groups[0].specs[3].from, [Missing]);
        assert_eq!(groups[1].variables, ["q1", "q2"]);
        assert_eq!(groups[1].specs[0].from, [Value(Literal::Num(1.0)), Value(Literal::Num(2.0))]);
        assert_eq!(groups[1].specs[1], RecodeSpec { from: vec![Value(Literal::Num(-1.0)), Value(Literal::Num(99.0))], to: RecodeTo::SysMis });
        assert_eq!(groups[1].specs[2], RecodeSpec { from: vec![Else], to: RecodeTo::Copy });
        let Command::Recode(s) = one("RECODE uf ('SP' 'RJ' = 'Sudeste') (ELSE = 'Outra') INTO regiao.") else { panic!() };
        assert_eq!(s[0].specs[0].to, RecodeTo::Value(Literal::Str("Sudeste".into())));
        assert!(parse("RECODE x (1 = ) .").is_err());
        assert!(parse("RECODE x y (1 = 2) INTO z.").is_err(), "one INTO name per variable");
        assert!(parse("RECODE x.").is_err());
    }
}
