//! Analysis procedures with subcommands of their own: `CROSSTABS`, `T-TEST`, `ONEWAY`.

use crate::lexer::Token;
use crate::{Command, Literal, SyntaxError, is_keyword};

/// What `CROSSTABS /CELLS` shows in each cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrosstabCells {
    pub count: bool,
    pub expected: bool,
    pub row: bool,
    pub column: bool,
    pub total: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TTestSpec {
    /// `/TESTVAL=x /VARIABLES=...`
    OneSample { variables: Vec<String>, test_value: f64 },
    /// `/GROUPS=g(a b) /VARIABLES=...`
    Independent { variables: Vec<String>, group: String, first: Literal, second: Literal },
    /// `/PAIRS=...`
    Paired { pairs: Vec<(String, String)> },
}

fn words(tokens: &[Token]) -> Option<Vec<String>> {
    tokens
        .iter()
        .map(|t| match t {
            Token::Word(w) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

/// Subcommands: `NAME = tokens`, `NAME tokens` or, for the first, bare tokens (`None` name).
fn subcommands(rest: &[Token]) -> Vec<(Option<&str>, &[Token])> {
    rest.split(|t| *t == Token::Slash)
        .filter(|g| !g.is_empty())
        .enumerate()
        .map(|(i, group)| match group {
            [Token::Word(name), Token::Equals, rest @ ..] => (Some(name.as_str()), rest),
            [Token::Word(name), rest @ ..] if i > 0 => (Some(name.as_str()), rest),
            group => (None, group),
        })
        .collect()
}

fn unsupported(command: &'static str, what: &str) -> SyntaxError {
    SyntaxError::Unsupported { command, what: what.to_uppercase() }
}

/// `left BY right`: two variable lists.
fn split_by(command: &'static str, tokens: &[Token]) -> Result<(Vec<String>, Vec<String>), SyntaxError> {
    let at = tokens.iter().position(|t| matches!(t, Token::Word(w) if w.eq_ignore_ascii_case("BY")));
    let lists = at.and_then(|at| Some((words(tokens.get(..at)?)?, words(tokens.get(at + 1..)?)?)));
    match lists {
        Some((left, right)) if !left.is_empty() && !right.is_empty() => Ok((left, right)),
        _ => Err(SyntaxError::Expected { command, expected: "variables BY variables" }),
    }
}

pub(crate) fn crosstabs(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "CROSSTABS";
    let mut tables = None;
    let mut cells = CrosstabCells { count: true, expected: false, row: false, column: false, total: false };
    let mut chisq = false;
    for (name, tokens) in subcommands(rest) {
        match name {
            None => tables = Some(split_by(CMD, tokens)?),
            Some(n) if is_keyword(n, "TABLES") => tables = Some(split_by(CMD, tokens)?),
            Some(n) if is_keyword(n, "STATISTICS") => {
                for w in words(tokens).ok_or(SyntaxError::Expected { command: CMD, expected: "statistic names" })? {
                    if is_keyword(&w, "CHISQ") {
                        chisq = true;
                    } else if !is_keyword(&w, "NONE") {
                        return Err(unsupported(CMD, &format!("/STATISTICS={w}")));
                    }
                }
            }
            Some(n) if is_keyword(n, "CELLS") => {
                // Listing cells replaces the default (count only).
                cells = CrosstabCells { count: false, expected: false, row: false, column: false, total: false };
                for w in words(tokens).ok_or(SyntaxError::Expected { command: CMD, expected: "cell statistics" })? {
                    match w.to_ascii_uppercase().as_str() {
                        "COUNT" => cells.count = true,
                        "EXPECTED" => cells.expected = true,
                        "ROW" => cells.row = true,
                        "COLUMN" => cells.column = true,
                        "TOTAL" => cells.total = true,
                        "ALL" => cells = CrosstabCells { count: true, expected: true, row: true, column: true, total: true },
                        "NONE" => {}
                        _ => return Err(unsupported(CMD, &format!("/CELLS={w}"))),
                    }
                }
            }
            Some(n) => return Err(unsupported(CMD, &format!("/{n}"))),
        }
    }
    let (rows, columns) = tables.ok_or(SyntaxError::Expected { command: CMD, expected: "/TABLES=rows BY columns" })?;
    Ok(Command::Crosstabs { rows, columns, cells, chisq })
}

fn literal(token: &Token) -> Option<Literal> {
    match token {
        Token::Str(s) => Some(Literal::Str(s.clone())),
        Token::Word(w) => w.parse().ok().map(Literal::Num),
        _ => None,
    }
}

pub(crate) fn t_test(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "T-TEST";
    let bad = |expected| SyntaxError::Expected { command: CMD, expected };
    let (mut test_value, mut variables, mut groups, mut pairs, mut ci) = (None, None, None, None, 0.95);
    for (name, tokens) in subcommands(rest) {
        let name = name.unwrap_or("VARIABLES");
        if is_keyword(name, "TESTVAL") {
            test_value = Some(match tokens {
                [Token::Word(w)] => w.parse::<f64>().map_err(|_| bad("a number after TESTVAL="))?,
                _ => return Err(bad("a number after TESTVAL=")),
            });
        } else if is_keyword(name, "VARIABLES") {
            variables = Some(words(tokens).filter(|v| !v.is_empty()).ok_or(bad("a variable list"))?);
        } else if is_keyword(name, "GROUPS") {
            let values: Vec<&Token> = tokens.get(2..).unwrap_or_default().iter().filter(|t| !matches!(t, Token::Comma | Token::RParen)).collect();
            groups = Some(match (tokens, values.as_slice()) {
                ([Token::Word(g), Token::LParen, .., Token::RParen], [a, b]) => {
                    (g.clone(), literal(a).ok_or(bad("two group values"))?, literal(b).ok_or(bad("two group values"))?)
                }
                _ => return Err(bad("GROUPS=variable(value value)")),
            });
        } else if is_keyword(name, "PAIRS") {
            let paired = matches!(tokens, [.., Token::LParen, Token::Word(p), Token::RParen] if is_keyword(p, "PAIRED"));
            let tokens = if paired { tokens.get(..tokens.len() - 3).unwrap_or_default() } else { tokens };
            let with = tokens.iter().position(|t| matches!(t, Token::Word(w) if w.eq_ignore_ascii_case("WITH")));
            let list = match with {
                Some(at) => {
                    let left = words(tokens.get(..at).unwrap_or_default()).ok_or(bad("variables WITH variables"))?;
                    let right = words(tokens.get(at + 1..).unwrap_or_default()).ok_or(bad("variables WITH variables"))?;
                    if paired {
                        if left.len() != right.len() {
                            return Err(bad("as many variables after WITH as before it, for (PAIRED)"));
                        }
                        left.into_iter().zip(right).collect::<Vec<_>>()
                    } else {
                        left.iter().flat_map(|a| right.iter().map(move |b| (a.clone(), b.clone()))).collect()
                    }
                }
                // Every pair of the list.
                None => {
                    let vars = words(tokens).ok_or(bad("a variable list"))?;
                    vars.iter().enumerate().flat_map(|(i, a)| vars.iter().skip(i + 1).map(move |b| (a.clone(), b.clone()))).collect()
                }
            };
            if list.is_empty() {
                return Err(bad("at least two variables to pair"));
            }
            pairs = Some(list);
        } else if is_keyword(name, "CRITERIA") {
            ci = match tokens {
                [Token::Word(w), Token::LParen, Token::Word(level), Token::RParen] if is_keyword(w, "CIN") || w.eq_ignore_ascii_case("CI") => {
                    level.parse::<f64>().ok().filter(|l| *l > 0.0 && *l < 1.0).ok_or(bad("a confidence level between 0 and 1, such as CI(.95)"))?
                }
                _ => return Err(bad("CRITERIA=CI(.95)")),
            };
        } else if is_keyword(name, "MISSING") {
            if !matches!(tokens, [Token::Word(w)] if is_keyword(w, "ANALYSIS")) {
                return Err(unsupported(CMD, "/MISSING other than ANALYSIS"));
            }
        } else {
            return Err(unsupported(CMD, &format!("/{name}")));
        }
    }
    let test = match (pairs, groups, test_value, variables) {
        (Some(pairs), None, None, None) => TTestSpec::Paired { pairs },
        (None, Some((group, first, second)), None, Some(variables)) => TTestSpec::Independent { variables, group, first, second },
        (None, None, Some(test_value), Some(variables)) => TTestSpec::OneSample { variables, test_value },
        _ => return Err(bad("one of TESTVAL= with VARIABLES=, GROUPS= with VARIABLES=, or PAIRS=")),
    };
    Ok(Command::TTest { test, ci })
}

pub(crate) fn oneway(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "ONEWAY";
    let (mut lists, mut descriptives, mut homogeneity) = (None, false, false);
    for (name, tokens) in subcommands(rest) {
        match name {
            None => lists = Some(split_by(CMD, tokens)?),
            Some(n) if is_keyword(n, "VARIABLES") => lists = Some(split_by(CMD, tokens)?),
            Some(n) if is_keyword(n, "STATISTICS") => {
                for w in words(tokens).ok_or(SyntaxError::Expected { command: CMD, expected: "statistic names" })? {
                    match w.to_ascii_uppercase().as_str() {
                        "DESCRIPTIVES" => descriptives = true,
                        "HOMOGENEITY" => homogeneity = true,
                        "ALL" => (descriptives, homogeneity) = (true, true),
                        "NONE" => {}
                        _ => return Err(unsupported(CMD, &format!("/STATISTICS={w}"))),
                    }
                }
            }
            Some(n) => return Err(unsupported(CMD, &format!("/{n}"))),
        }
    }
    let (dependents, factor) = lists.ok_or(SyntaxError::Expected { command: CMD, expected: "variables BY factor" })?;
    let [factor] = <[String; 1]>::try_from(factor).map_err(|_| SyntaxError::Expected { command: CMD, expected: "one factor after BY" })?;
    Ok(Command::Oneway { dependents, factor, descriptives, homogeneity })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn one(src: &str) -> Command {
        parse(src).unwrap().remove(0)
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn crosstabs_syntax() {
        let all = CrosstabCells { count: true, expected: true, row: true, column: true, total: true };
        assert_eq!(
            one("CROSSTABS /TABLES=sexo regiao BY voto /STATISTICS=CHISQ /CELLS=ALL."),
            Command::Crosstabs { rows: names(&["sexo", "regiao"]), columns: names(&["voto"]), cells: all, chisq: true }
        );
        let Command::Crosstabs { cells, chisq, .. } = one("CROSSTABS a BY b /CELLS=ROW.") else { panic!() };
        assert!(cells.row && !cells.count, "listing cells replaces the default count");
        assert!(!chisq);
        assert!(parse("CROSSTABS a b.").is_err());
        assert!(parse("CROSSTABS a BY b /STATISTICS=PHI.").is_err());
    }

    #[test]
    fn t_test_syntax() {
        assert_eq!(
            one("T-TEST /TESTVAL=-1.5 /VARIABLES=x y /CRITERIA=CI(.99)."),
            Command::TTest { test: TTestSpec::OneSample { variables: names(&["x", "y"]), test_value: -1.5 }, ci: 0.99 }
        );
        assert_eq!(
            one("T-TEST GROUPS=sexo('F' 'M') /VARIABLES=renda."),
            Command::TTest {
                test: TTestSpec::Independent {
                    variables: names(&["renda"]),
                    group: "sexo".into(),
                    first: Literal::Str("F".into()),
                    second: Literal::Str("M".into())
                },
                ci: 0.95
            }
        );
        let Command::TTest { test: TTestSpec::Independent { first, .. }, .. } = one("T-TEST GROUPS=g(1, 2) /VARIABLES=x.") else { panic!() };
        assert_eq!(first, Literal::Num(1.0));
        assert_eq!(
            one("T-TEST PAIRS=pre1 pre2 WITH pos1 pos2 (PAIRED)."),
            Command::TTest { test: TTestSpec::Paired { pairs: vec![("pre1".into(), "pos1".into()), ("pre2".into(), "pos2".into())] }, ci: 0.95 }
        );
        let Command::TTest { test: TTestSpec::Paired { pairs }, .. } = one("T-TEST PAIRS=a b c.") else { panic!() };
        assert_eq!(pairs.len(), 3, "every pair of the list");
        assert!(parse("T-TEST /VARIABLES=x.").is_err(), "needs TESTVAL, GROUPS or PAIRS");
        assert!(parse("T-TEST PAIRS=a WITH b c (PAIRED).").is_err());
    }

    #[test]
    fn oneway_syntax() {
        assert_eq!(
            one("ONEWAY renda nota BY escola /STATISTICS DESCRIPTIVES HOMOGENEITY."),
            Command::Oneway { dependents: names(&["renda", "nota"]), factor: "escola".into(), descriptives: true, homogeneity: true }
        );
        assert!(parse("ONEWAY y BY a b.").is_err(), "one factor");
        assert!(parse("ONEWAY y /POSTHOC=TUKEY.").is_err());
    }
}
