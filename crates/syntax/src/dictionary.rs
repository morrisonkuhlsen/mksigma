//! Commands that change the dictionary: `VARIABLE LABELS`, `VALUE LABELS`, `MISSING VALUES`,
//! `VARIABLE LEVEL`, `FORMATS`, `RENAME VARIABLES`. Variable View edits run through these, so
//! every edit is a reproducible step.

use crate::lexer::Token;
use crate::{Command, SyntaxError, is_keyword};

/// Value labels for one group of variables: `(value, label)` pairs in the order written.
pub type ValueLabelList = Vec<(Literal, String)>;

/// A variable list and the tokens inside the parentheses after it.
type Group<'a> = (Vec<String>, &'a [Token]);

/// A value written in syntax: a number or a quoted string.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Num(f64),
    Str(String),
}

/// `MISSING VALUES` specification for a group of variables.
#[derive(Debug, Clone, PartialEq)]
pub enum MissingSpec {
    /// `()`: no user-missing values.
    None,
    Discrete(Vec<Literal>),
    /// `(low THRU high [, value])`; `None` bounds are `LO`/`LOWEST` and `HI`/`HIGHEST`.
    Range {
        low: Option<f64>,
        high: Option<f64>,
        discrete: Option<f64>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Nominal,
    Ordinal,
    Scale,
}

/// A variable name starts with a letter or `@#$`; numbers start with a digit, `-` or `.`.
fn is_name(word: &str) -> bool {
    word.chars().next().is_some_and(|c| !(c.is_ascii_digit() || c == '-' || c == '.'))
}

fn literal(token: &Token) -> Option<Literal> {
    match token {
        Token::Str(s) => Some(Literal::Str(s.clone())),
        Token::Word(w) if !is_name(w) => w.parse().ok().filter(|x: &f64| x.is_finite()).map(Literal::Num),
        _ => None,
    }
}

/// Leading variable names, and what follows them.
fn var_list(tokens: &[Token]) -> (Vec<String>, &[Token]) {
    let n = tokens.iter().take_while(|t| matches!(t, Token::Word(w) if is_name(w))).count();
    let names = tokens.iter().take(n).filter_map(|t| if let Token::Word(w) = t { Some(w.clone()) } else { None }).collect();
    (names, tokens.get(n..).unwrap_or_default())
}

/// Repeated `varlist (contents)` groups, as in `MISSING VALUES a b (9) c (8)`.
fn parenthesized_groups<'a>(command: &'static str, tokens: &'a [Token]) -> Result<Vec<Group<'a>>, SyntaxError> {
    let mut out = Vec::new();
    for group in tokens.split(|t| *t == Token::Slash) {
        let mut rest = group;
        while !rest.is_empty() {
            let (vars, after) = var_list(rest);
            if vars.is_empty() {
                return Err(SyntaxError::Expected { command, expected: "a variable list" });
            }
            let Some(Token::LParen) = after.first() else {
                return Err(SyntaxError::Expected { command, expected: "(" });
            };
            let close = after.iter().position(|t| *t == Token::RParen).ok_or(SyntaxError::Expected { command, expected: ")" })?;
            out.push((vars, after.get(1..close).unwrap_or_default()));
            rest = after.get(close + 1..).unwrap_or_default();
        }
    }
    if out.is_empty() {
        return Err(SyntaxError::Expected { command, expected: "a variable list" });
    }
    Ok(out)
}

fn variable_labels(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "VARIABLE LABELS";
    let mut labels = Vec::new();
    for group in rest.split(|t| *t == Token::Slash) {
        for pair in group.chunks(2) {
            match pair {
                [Token::Word(name), Token::Str(label)] if is_name(name) => labels.push((name.clone(), label.clone())),
                _ => return Err(SyntaxError::Expected { command: CMD, expected: "a variable name and a quoted label" }),
            }
        }
    }
    if labels.is_empty() {
        return Err(SyntaxError::Expected { command: CMD, expected: "a variable name and a quoted label" });
    }
    Ok(Command::VariableLabels(labels))
}

fn value_labels(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "VALUE LABELS";
    let mut groups = Vec::new();
    for group in rest.split(|t| *t == Token::Slash).filter(|g| !g.is_empty()) {
        let (vars, pairs) = var_list(group);
        if vars.is_empty() {
            return Err(SyntaxError::Expected { command: CMD, expected: "a variable list" });
        }
        let mut labels = Vec::new();
        for pair in pairs.chunks(2) {
            match pair {
                [value, Token::Str(label)] => {
                    let value = literal(value).ok_or(SyntaxError::Expected { command: CMD, expected: "a value (number or quoted string)" })?;
                    labels.push((value, label.clone()));
                }
                _ => return Err(SyntaxError::Expected { command: CMD, expected: "a value and a quoted label" }),
            }
        }
        groups.push((vars, labels));
    }
    if groups.is_empty() {
        return Err(SyntaxError::Expected { command: CMD, expected: "a variable list" });
    }
    Ok(Command::ValueLabels(groups))
}

fn missing_values(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "MISSING VALUES";
    let bound = |t: &Token, keyword: &str| -> Result<Option<f64>, SyntaxError> {
        match t {
            Token::Word(w) if is_keyword(w, keyword) || keyword.get(..2).is_some_and(|k| w.eq_ignore_ascii_case(k)) => Ok(None),
            t => match literal(t) {
                Some(Literal::Num(x)) => Ok(Some(x)),
                _ => Err(SyntaxError::Expected { command: CMD, expected: "a number, LO or HI in a range" }),
            },
        }
    };
    let mut out = Vec::new();
    for (vars, inner) in parenthesized_groups(CMD, rest)? {
        if inner.is_empty() {
            out.push((vars, MissingSpec::None));
            continue;
        }
        let mut discrete = Vec::new();
        let mut range = None;
        for item in inner.split(|t| *t == Token::Comma) {
            match item {
                [low, Token::Word(thru), high] if is_keyword(thru, "THRU") => {
                    if range.is_some() {
                        return Err(SyntaxError::Expected { command: CMD, expected: "at most one range" });
                    }
                    range = Some((bound(low, "LOWEST")?, bound(high, "HIGHEST")?));
                }
                [value] => discrete.push(literal(value).ok_or(SyntaxError::Expected { command: CMD, expected: "a value" })?),
                _ => return Err(SyntaxError::Expected { command: CMD, expected: "values separated by commas" }),
            }
        }
        let spec = match (range, discrete.as_slice()) {
            (None, list) if list.len() <= 3 => MissingSpec::Discrete(discrete),
            (Some((low, high)), []) => MissingSpec::Range { low, high, discrete: None },
            (Some((low, high)), [Literal::Num(x)]) => MissingSpec::Range { low, high, discrete: Some(*x) },
            _ => return Err(SyntaxError::Expected { command: CMD, expected: "up to three values, or a range and one value" }),
        };
        out.push((vars, spec));
    }
    Ok(Command::MissingValues(out))
}

fn variable_level(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "VARIABLE LEVEL";
    let mut out = Vec::new();
    for (vars, inner) in parenthesized_groups(CMD, rest)? {
        let level = match inner {
            [Token::Word(w)] if is_keyword(w, "NOMINAL") => Level::Nominal,
            [Token::Word(w)] if is_keyword(w, "ORDINAL") => Level::Ordinal,
            [Token::Word(w)] if is_keyword(w, "SCALE") => Level::Scale,
            _ => return Err(SyntaxError::Expected { command: CMD, expected: "NOMINAL, ORDINAL or SCALE" }),
        };
        out.push((vars, level));
    }
    Ok(Command::VariableLevel(out))
}

fn formats(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "FORMATS";
    let mut out = Vec::new();
    for (vars, inner) in parenthesized_groups(CMD, rest)? {
        match inner {
            [Token::Word(format)] => out.push((vars, format.clone())),
            _ => return Err(SyntaxError::Expected { command: CMD, expected: "a format such as F8.2" }),
        }
    }
    Ok(Command::Formats(out))
}

fn rename_variables(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "RENAME VARIABLES";
    let expected = SyntaxError::Expected { command: CMD, expected: "(old = new)" };
    let mut pairs = Vec::new();
    let mut add = |tokens: &[Token]| -> Result<(), SyntaxError> {
        let at = tokens.iter().position(|t| *t == Token::Equals).ok_or(expected.clone())?;
        let (old, rest) = var_list(tokens.get(..at).unwrap_or_default());
        let (new, after) = var_list(tokens.get(at + 1..).unwrap_or_default());
        if old.is_empty() || old.len() != new.len() || !rest.is_empty() || !after.is_empty() {
            return Err(expected.clone());
        }
        pairs.extend(old.into_iter().zip(new));
        Ok(())
    };
    let mut rest = rest;
    while !rest.is_empty() {
        if let Some(Token::LParen) = rest.first() {
            let close = rest.iter().position(|t| *t == Token::RParen).ok_or(expected.clone())?;
            add(rest.get(1..close).unwrap_or_default())?;
            rest = rest.get(close + 1..).unwrap_or_default();
        } else {
            add(rest)?;
            rest = &[];
        }
    }
    if pairs.is_empty() {
        return Err(expected);
    }
    Ok(Command::RenameVariables(pairs))
}

/// New variables' names and the format written after them, if any.
pub type NewVariables = Vec<(Vec<String>, Option<String>)>;

/// `NUMERIC` and `STRING`: variable lists, each followed by an optional `(format)`.
fn new_variables(command: &'static str, rest: &[Token]) -> Result<NewVariables, SyntaxError> {
    let mut out = Vec::new();
    for group in rest.split(|t| *t == Token::Slash).filter(|g| !g.is_empty()) {
        let mut tokens = group;
        while !tokens.is_empty() {
            let (vars, after) = var_list(tokens);
            if vars.is_empty() {
                return Err(SyntaxError::Expected { command, expected: "new variable names" });
            }
            match after {
                [Token::LParen, Token::Word(format), Token::RParen, more @ ..] => {
                    out.push((vars, Some(format.clone())));
                    tokens = more;
                }
                [] => {
                    out.push((vars, None));
                    tokens = &[];
                }
                _ => return Err(SyntaxError::Expected { command, expected: "a format in parentheses, such as (F8.2)" }),
            }
        }
    }
    if out.is_empty() {
        return Err(SyntaxError::Expected { command, expected: "new variable names" });
    }
    Ok(out)
}

fn string_variables(rest: &[Token]) -> Result<Command, SyntaxError> {
    let groups = new_variables("STRING", rest)?
        .into_iter()
        .map(|(vars, format)| format.map(|f| (vars, f)).ok_or(SyntaxError::Expected { command: "STRING", expected: "a width such as (A20)" }))
        .collect::<Result<_, _>>()?;
    Ok(Command::String(groups))
}

fn delete_variables(rest: &[Token]) -> Result<Command, SyntaxError> {
    match var_list(rest) {
        (vars, []) if !vars.is_empty() => Ok(Command::DeleteVariables(vars)),
        _ => Err(SyntaxError::Expected { command: "DELETE VARIABLES", expected: "a variable list" }),
    }
}

/// Parses a two-word dictionary command, if `name second` is one.
pub(crate) fn parse(name: &str, rest: &[Token]) -> Option<Result<Command, SyntaxError>> {
    let (second, after) = match rest.split_first() {
        Some((Token::Word(w), after)) => (Some(w.as_str()), after),
        _ => (None, rest),
    };
    let is = |first: &str, word: &str| is_keyword(name, first) && second.is_some_and(|s| is_keyword(s, word));
    Some(if is("VARIABLE", "LABELS") {
        variable_labels(after)
    } else if is("VARIABLE", "LEVEL") {
        variable_level(after)
    } else if is("VALUE", "LABELS") {
        value_labels(after)
    } else if is("MISSING", "VALUES") {
        missing_values(after)
    } else if is("RENAME", "VARIABLES") {
        rename_variables(after)
    } else if is("DELETE", "VARIABLES") {
        delete_variables(after)
    } else if is_keyword(name, "FORMATS") {
        formats(rest)
    } else if is_keyword(name, "NUMERIC") {
        new_variables("NUMERIC", rest).map(Command::Numeric)
    } else if is_keyword(name, "STRING") {
        string_variables(rest)
    } else {
        return None;
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn one(src: &str) -> Command {
        let mut c = parse(src).unwrap();
        assert_eq!(c.len(), 1);
        c.remove(0)
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn variable_labels() {
        assert_eq!(
            one("VARIABLE LABELS age 'Age in years' /income \"Monthly income\"."),
            Command::VariableLabels(vec![("age".into(), "Age in years".into()), ("income".into(), "Monthly income".into())])
        );
        assert!(parse("VARIABLE LABELS age.").is_err());
    }

    #[test]
    fn value_labels() {
        assert_eq!(
            one("VALUE LABELS q1 q2 1 'Low' 5 'High' /city 'SP' 'São Paulo'."),
            Command::ValueLabels(vec![
                (names(&["q1", "q2"]), vec![(Literal::Num(1.0), "Low".into()), (Literal::Num(5.0), "High".into())]),
                (names(&["city"]), vec![(Literal::Str("SP".into()), "São Paulo".into())]),
            ])
        );
        assert_eq!(one("VALUE LABELS q1."), Command::ValueLabels(vec![(names(&["q1"]), vec![])]));
    }

    #[test]
    fn missing_values() {
        assert_eq!(
            one("MISSING VALUES a b (9, 99) c (LO THRU 0, 999) d () e ('NA')."),
            Command::MissingValues(vec![
                (names(&["a", "b"]), MissingSpec::Discrete(vec![Literal::Num(9.0), Literal::Num(99.0)])),
                (names(&["c"]), MissingSpec::Range { low: None, high: Some(0.0), discrete: Some(999.0) }),
                (names(&["d"]), MissingSpec::None),
                (names(&["e"]), MissingSpec::Discrete(vec![Literal::Str("NA".into())])),
            ])
        );
        assert_eq!(
            one("MISSING VALUES x (90 THRU HIGHEST)."),
            Command::MissingValues(vec![(names(&["x"]), MissingSpec::Range { low: Some(90.0), high: None, discrete: None })])
        );
        assert!(parse("MISSING VALUES x (1, 2, 3, 4).").is_err());
        assert!(parse("MISSING VALUES x (1 THRU 2, 3 THRU 4).").is_err());
        assert!(parse("MISSING VALUES x 9.").is_err());
    }

    #[test]
    fn new_and_deleted_variables() {
        assert_eq!(
            one("NUMERIC a b (F3.0) c /d."),
            Command::Numeric(vec![(names(&["a", "b"]), Some("F3.0".into())), (names(&["c"]), None), (names(&["d"]), None)])
        );
        assert_eq!(one("STRING nome (A20) /uf (A2)."), Command::String(vec![(names(&["nome"]), "A20".into()), (names(&["uf"]), "A2".into())]));
        assert!(parse("STRING nome.").is_err(), "a string needs a width");
        assert!(parse("NUMERIC (F8.2).").is_err());
        assert_eq!(one("DELETE VARIABLES a b."), Command::DeleteVariables(names(&["a", "b"])));
        assert!(parse("DELETE VARIABLES.").is_err());
    }

    #[test]
    fn level_formats_rename() {
        assert_eq!(
            one("VARIABLE LEVEL a b (SCALE) /c (NOMINAL)."),
            Command::VariableLevel(vec![(names(&["a", "b"]), Level::Scale), (names(&["c"]), Level::Nominal)])
        );
        assert_eq!(one("FORMATS income (DOLLAR10.2)."), Command::Formats(vec![(names(&["income"]), "DOLLAR10.2".into())]));
        assert_eq!(
            one("RENAME VARIABLES (q1 = satisfaction) (a b = x y)."),
            Command::RenameVariables(vec![("q1".into(), "satisfaction".into()), ("a".into(), "x".into()), ("b".into(), "y".into())])
        );
        assert_eq!(one("RENAME VARIABLES q1 = s."), Command::RenameVariables(vec![("q1".into(), "s".into())]));
        assert!(parse("RENAME VARIABLES (a b = x).").is_err());
        assert!(parse("VARIABLE LEVEL a (RATIO).").is_err());
    }
}
