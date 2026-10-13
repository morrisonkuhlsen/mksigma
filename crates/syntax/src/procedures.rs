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
    // Number the groups before dropping empty ones: anything after a slash is a subcommand,
    // even the first one written (`REGRESSION /DEPENDENT y ...`).
    rest.split(|t| *t == Token::Slash)
        .enumerate()
        .filter(|(_, g)| !g.is_empty())
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

pub(crate) fn correlations(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "CORRELATIONS";
    let bad = |expected| SyntaxError::Expected { command: CMD, expected };
    let (mut lists, mut one_tailed, mut flag, mut listwise) = (None, false, true, false);
    for (name, tokens) in subcommands(rest) {
        match name {
            None => lists = Some(tokens),
            Some(n) if is_keyword(n, "VARIABLES") => lists = Some(tokens),
            Some(n) if is_keyword(n, "PRINT") => {
                for w in words(tokens).ok_or(bad("PRINT keywords"))? {
                    match w.to_ascii_uppercase().as_str() {
                        "TWOTAIL" => one_tailed = false,
                        "ONETAIL" => one_tailed = true,
                        // NOSIG flags significant correlations; SIG shows them without flags.
                        "NOSIG" => flag = true,
                        "SIG" => flag = false,
                        _ => return Err(unsupported(CMD, &format!("/PRINT={w}"))),
                    }
                }
            }
            Some(n) if is_keyword(n, "MISSING") => {
                for w in words(tokens).ok_or(bad("PAIRWISE or LISTWISE"))? {
                    match w.to_ascii_uppercase().as_str() {
                        "PAIRWISE" => listwise = false,
                        "LISTWISE" => listwise = true,
                        "EXCLUDE" => {}
                        _ => return Err(unsupported(CMD, &format!("/MISSING={w}"))),
                    }
                }
            }
            Some(n) => return Err(unsupported(CMD, &format!("/{n}"))),
        }
    }
    let tokens = lists.ok_or(bad("a variable list"))?;
    let at = tokens.iter().position(|t| matches!(t, Token::Word(w) if w.eq_ignore_ascii_case("WITH")));
    let (variables, with) = match at {
        Some(at) => (words(tokens.get(..at).unwrap_or_default()), words(tokens.get(at + 1..).unwrap_or_default())),
        None => (words(tokens), Some(Vec::new())),
    };
    let (Some(variables), Some(with)) = (variables, with) else { return Err(bad("variable names")) };
    if variables.is_empty() || (at.is_some() && with.is_empty()) || (at.is_none() && variables.len() < 2) {
        return Err(bad("at least two variables, or variables WITH variables"));
    }
    Ok(Command::Correlations { variables, with, one_tailed, flag, listwise })
}

pub(crate) fn regression(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "REGRESSION";
    let bad = |expected| SyntaxError::Expected { command: CMD, expected };
    let (mut dependent, mut predictors, mut ci) = (None, None, None);
    for (name, tokens) in subcommands(rest) {
        let name = name.ok_or(bad("/DEPENDENT= and /METHOD=ENTER"))?;
        if is_keyword(name, "DEPENDENT") {
            dependent = Some(match words(tokens).as_deref() {
                Some([y]) => y.clone(),
                _ => return Err(unsupported(CMD, "more than one dependent variable")),
            });
        } else if is_keyword(name, "METHOD") {
            if predictors.is_some() {
                return Err(unsupported(CMD, "more than one /METHOD (hierarchical models)"));
            }
            predictors = Some(match words(tokens).as_deref() {
                Some([method, vars @ ..]) if is_keyword(method, "ENTER") && !vars.is_empty() => vars.to_vec(),
                Some([method, ..]) if is_keyword(method, "ENTER") => return Err(bad("the predictors after ENTER")),
                _ => return Err(unsupported(CMD, "methods other than ENTER")),
            });
        } else if is_keyword(name, "STATISTICS") {
            let mut i = 0;
            while let Some(t) = tokens.get(i) {
                match t {
                    Token::Word(w) if w.eq_ignore_ascii_case("CI") => {
                        // CI or CI(95): the level in percent.
                        ci = Some(match (tokens.get(i + 1), tokens.get(i + 2), tokens.get(i + 3)) {
                            (Some(Token::LParen), Some(Token::Word(level)), Some(Token::RParen)) => {
                                i += 3;
                                level
                                    .parse::<f64>()
                                    .ok()
                                    .filter(|l| *l > 0.0 && *l < 100.0)
                                    .ok_or(bad("CI(level) with a level in percent, such as CI(95)"))?
                                    / 100.0
                            }
                            _ => 0.95,
                        });
                    }
                    Token::Word(w) if ["COEFF", "OUTS", "R", "ANOVA", "DEFAULTS"].iter().any(|k| w.eq_ignore_ascii_case(k)) => {}
                    Token::Word(w) => return Err(unsupported(CMD, &format!("/STATISTICS={w}"))),
                    _ => return Err(bad("statistic names")),
                }
                i += 1;
            }
        } else if is_keyword(name, "MISSING") {
            if !matches!(tokens, [Token::Word(w)] if is_keyword(w, "LISTWISE")) {
                return Err(unsupported(CMD, "/MISSING other than LISTWISE"));
            }
        } else if is_keyword(name, "CRITERIA") || is_keyword(name, "NOORIGIN") {
            // Entry criteria only matter for stepwise methods; NOORIGIN is the default.
        } else {
            return Err(unsupported(CMD, &format!("/{name}")));
        }
    }
    let dependent = dependent.ok_or(bad("/DEPENDENT=variable"))?;
    let predictors = predictors.ok_or(bad("/METHOD=ENTER predictors"))?;
    Ok(Command::Regression { dependent, predictors, ci })
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
    fn correlations_syntax() {
        assert_eq!(
            one("CORRELATIONS /VARIABLES=a b c /PRINT=ONETAIL NOSIG /MISSING=LISTWISE."),
            Command::Correlations { variables: names(&["a", "b", "c"]), with: vec![], one_tailed: true, flag: true, listwise: true }
        );
        assert_eq!(
            one("CORRELATIONS a b WITH c."),
            Command::Correlations { variables: names(&["a", "b"]), with: names(&["c"]), one_tailed: false, flag: true, listwise: false }
        );
        assert!(parse("CORRELATIONS a.").is_err());
        assert!(parse("CORRELATIONS a b /STATISTICS=DESCRIPTIVES.").is_err());
    }

    #[test]
    fn regression_syntax_as_spss_pastes_it() {
        assert_eq!(
            one(
                "REGRESSION /MISSING LISTWISE /STATISTICS COEFF OUTS CI(99) R ANOVA /CRITERIA=PIN(.05) POUT(.10) /NOORIGIN /DEPENDENT renda /METHOD=ENTER idade escolaridade."
            ),
            Command::Regression { dependent: "renda".into(), predictors: names(&["idade", "escolaridade"]), ci: Some(0.99) }
        );
        assert!(parse("REGRESSION /DEPENDENT y /METHOD=STEPWISE x.").is_err());
        assert!(parse("REGRESSION /DEPENDENT y /METHOD=ENTER a /METHOD=ENTER b.").is_err());
        assert!(parse("REGRESSION /METHOD=ENTER a.").is_err());
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
