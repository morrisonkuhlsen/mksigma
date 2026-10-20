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

/// What a bar chart's bars measure.
#[derive(Debug, Clone, PartialEq)]
pub enum BarStat {
    Count,
    Mean(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum GraphSpec {
    Histogram { variable: String, normal: bool },
    Bar { value: BarStat, by: String },
    Scatter { x: String, y: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum NparSpec {
    MannWhitney { variables: Vec<String>, group: String, first: f64, second: f64 },
    Wilcoxon { pairs: Vec<(String, String)> },
    KruskalWallis { variables: Vec<String>, group: String, low: f64, high: f64 },
}

/// Post hoc tests for `ONEWAY`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostHocTest {
    Tukey,
    Bonferroni,
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

/// `a b WITH c d (PAIRED)` pairs a–c and b–d; without (PAIRED), every left variable with every
/// right one; `a b c` alone, every pair of the list.
fn pair_list(tokens: &[Token]) -> Result<Vec<(String, String)>, &'static str> {
    let paired = matches!(tokens, [.., Token::LParen, Token::Word(p), Token::RParen] if is_keyword(p, "PAIRED"));
    let tokens = if paired { tokens.get(..tokens.len().saturating_sub(3)).unwrap_or_default() } else { tokens };
    let with = tokens.iter().position(|t| matches!(t, Token::Word(w) if w.eq_ignore_ascii_case("WITH")));
    let list: Vec<(String, String)> = match with {
        Some(at) => {
            let left = words(tokens.get(..at).unwrap_or_default()).ok_or("variables WITH variables")?;
            let right = words(tokens.get(at + 1..).unwrap_or_default()).ok_or("variables WITH variables")?;
            if paired {
                if left.len() != right.len() {
                    return Err("as many variables after WITH as before it, for (PAIRED)");
                }
                left.into_iter().zip(right).collect()
            } else {
                left.iter().flat_map(|a| right.iter().map(move |b| (a.clone(), b.clone()))).collect()
            }
        }
        None => {
            let vars = words(tokens).ok_or("a variable list")?;
            vars.iter().enumerate().flat_map(|(i, a)| vars.iter().skip(i + 1).map(move |b| (a.clone(), b.clone()))).collect()
        }
    };
    if list.is_empty() {
        return Err("at least two variables to pair");
    }
    Ok(list)
}

/// `vars BY group(a b)`: the variables, the grouping variable and its two values.
fn by_group(tokens: &[Token]) -> Option<(Vec<String>, String, f64, f64)> {
    let at = tokens.iter().position(|t| matches!(t, Token::Word(w) if w.eq_ignore_ascii_case("BY")))?;
    let vars = words(tokens.get(..at)?).filter(|v| !v.is_empty())?;
    let values: Vec<f64> = match tokens.get(at + 1..)? {
        [Token::Word(_), Token::LParen, inside @ .., Token::RParen] => inside
            .iter()
            .filter(|t| **t != Token::Comma)
            .map(|t| match t {
                Token::Word(w) => w.parse().ok(),
                _ => None,
            })
            .collect::<Option<_>>()?,
        _ => return None,
    };
    let Some(Token::Word(group)) = tokens.get(at + 1) else { return None };
    match values[..] {
        [a, b] => Some((vars, group.clone(), a, b)),
        _ => None,
    }
}

pub(crate) fn examine(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "EXAMINE";
    let bad = |expected| SyntaxError::Expected { command: CMD, expected };
    let (mut lists, mut descriptives, mut boxplot, mut stemleaf) = (None, true, true, true);
    for (name, tokens) in subcommands(rest) {
        let values = || words(tokens).ok_or(bad("keywords"));
        match name {
            None => lists = Some(tokens),
            Some(n) if is_keyword(n, "VARIABLES") => lists = Some(tokens),
            Some(n) if is_keyword(n, "PLOT") => {
                (boxplot, stemleaf) = (false, false);
                for w in values()? {
                    match w.to_ascii_uppercase().as_str() {
                        "BOXPLOT" => boxplot = true,
                        "STEMLEAF" => stemleaf = true,
                        "NONE" => {}
                        _ => return Err(unsupported(CMD, &format!("/PLOT={w}"))),
                    }
                }
            }
            Some(n) if is_keyword(n, "STATISTICS") => {
                descriptives = false;
                for w in values()? {
                    match w.to_ascii_uppercase().as_str() {
                        "DESCRIPTIVES" => descriptives = true,
                        "NONE" => {}
                        _ => return Err(unsupported(CMD, &format!("/STATISTICS={w}"))),
                    }
                }
            }
            Some(n) if is_keyword(n, "CINTERVAL") => {
                if !matches!(tokens, [Token::Word(level)] if level == "95") {
                    return Err(unsupported(CMD, "confidence levels other than 95"));
                }
            }
            // What SPSS pastes by default; the totals and comparisons here match it.
            Some(n) if is_keyword(n, "COMPARE") || is_keyword(n, "MISSING") || is_keyword(n, "NOTOTAL") || is_keyword(n, "TOTAL") => {}
            Some(n) => return Err(unsupported(CMD, &format!("/{n}"))),
        }
    }
    let tokens = lists.ok_or(bad("VARIABLES=variables [BY group]"))?;
    let at = tokens.iter().position(|t| matches!(t, Token::Word(w) if w.eq_ignore_ascii_case("BY")));
    let (variables, by) = match at {
        Some(at) => match (words(tokens.get(..at).unwrap_or_default()), words(tokens.get(at + 1..).unwrap_or_default()).as_deref()) {
            (Some(vars), Some([group])) => (vars, Some(group.clone())),
            _ => return Err(bad("variables BY one grouping variable")),
        },
        None => (words(tokens).ok_or(bad("variable names"))?, None),
    };
    if variables.is_empty() {
        return Err(bad("VARIABLES=variables [BY group]"));
    }
    Ok(Command::Examine { variables, by, descriptives, boxplot, stemleaf })
}

pub(crate) fn graph(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "GRAPH";
    let bad = |expected| SyntaxError::Expected { command: CMD, expected };
    let (mut chart, mut title) = (None, None);
    for group in rest.split(|t| *t == Token::Slash).filter(|g| !g.is_empty()) {
        let Some((Token::Word(name), rest)) = group.split_first() else { return Err(bad("a chart such as /HISTOGRAM=x")) };
        // An optional (option) between the chart name and `=`.
        let (option, rest) = match rest {
            [Token::LParen, Token::Word(o), Token::RParen, rest @ ..] => (Some(o.as_str()), rest),
            rest => (None, rest),
        };
        let rest = match rest {
            [Token::Equals, rest @ ..] => rest,
            rest => rest,
        };
        if is_keyword(name, "TITLE") {
            title = match rest {
                [Token::Str(t)] => Some(t.clone()),
                _ => return Err(bad("/TITLE='a quoted title'")),
            };
            continue;
        }
        if chart.is_some() {
            return Err(unsupported(CMD, "more than one chart per command"));
        }
        chart = Some(if is_keyword(name, "HISTOGRAM") {
            let normal = match option {
                None => false,
                Some(o) if is_keyword(o, "NORMAL") => true,
                Some(o) => return Err(unsupported(CMD, &format!("/HISTOGRAM({o})"))),
            };
            match rest {
                [Token::Word(v)] => GraphSpec::Histogram { variable: v.clone(), normal },
                _ => return Err(bad("/HISTOGRAM=variable")),
            }
        } else if is_keyword(name, "BAR") {
            if option.is_some_and(|o| !is_keyword(o, "SIMPLE")) {
                return Err(unsupported(CMD, "bar charts other than SIMPLE"));
            }
            match rest {
                [Token::Word(c), Token::Word(by), Token::Word(x)] if is_keyword(c, "COUNT") && by.eq_ignore_ascii_case("BY") => {
                    GraphSpec::Bar { value: BarStat::Count, by: x.clone() }
                }
                [Token::Word(f), Token::LParen, Token::Word(y), Token::RParen, Token::Word(by), Token::Word(x)]
                    if is_keyword(f, "MEAN") && by.eq_ignore_ascii_case("BY") =>
                {
                    GraphSpec::Bar { value: BarStat::Mean(y.clone()), by: x.clone() }
                }
                _ => return Err(bad("/BAR(SIMPLE)=COUNT BY x or MEAN(y) BY x")),
            }
        } else if is_keyword(name, "SCATTERPLOT") {
            if option.is_some_and(|o| !is_keyword(o, "BIVAR")) {
                return Err(unsupported(CMD, "scatterplots other than BIVAR"));
            }
            match rest {
                [Token::Word(x), Token::Word(with), Token::Word(y)] if with.eq_ignore_ascii_case("WITH") => {
                    GraphSpec::Scatter { x: x.clone(), y: y.clone() }
                }
                _ => return Err(bad("/SCATTERPLOT(BIVAR)=x WITH y")),
            }
        } else {
            return Err(unsupported(CMD, &format!("/{name}")));
        });
    }
    Ok(Command::Graph { chart: chart.ok_or(bad("a chart such as /HISTOGRAM=x"))?, title })
}

pub(crate) fn npar_tests(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "NPAR TESTS";
    let bad = |expected| SyntaxError::Expected { command: CMD, expected };
    let mut tests = Vec::new();
    for group in rest.split(|t| *t == Token::Slash).filter(|g| !g.is_empty()) {
        // Test names with a hyphen (M-W, K-W) arrive as word, minus, word.
        let (name, tokens) = match group {
            [Token::Word(a), Token::Minus, Token::Word(b), Token::Equals, rest @ ..] | [Token::Word(a), Token::Minus, Token::Word(b), rest @ ..] => {
                (format!("{a}-{b}").to_ascii_uppercase(), rest)
            }
            [Token::Word(a), Token::Equals, rest @ ..] | [Token::Word(a), rest @ ..] => (a.to_ascii_uppercase(), rest),
            _ => return Err(bad("a test such as /M-W= or /WILCOXON=")),
        };
        match name.as_str() {
            "M-W" => {
                let (variables, group, first, second) = by_group(tokens).ok_or(bad("/M-W=variables BY group(value value)"))?;
                tests.push(NparSpec::MannWhitney { variables, group, first, second });
            }
            "K-W" => {
                let (variables, group, low, high) = by_group(tokens).ok_or(bad("/K-W=variables BY group(low high)"))?;
                tests.push(NparSpec::KruskalWallis { variables, group, low: low.min(high), high: low.max(high) });
            }
            n if is_keyword(n, "WILCOXON") => tests.push(NparSpec::Wilcoxon { pairs: pair_list(tokens).map_err(bad)? }),
            n if is_keyword(n, "MISSING") => {
                if !matches!(tokens, [Token::Word(w)] if is_keyword(w, "ANALYSIS")) {
                    return Err(unsupported(CMD, "/MISSING other than ANALYSIS"));
                }
            }
            n => return Err(unsupported(CMD, &format!("/{n}"))),
        }
    }
    if tests.is_empty() {
        return Err(bad("a test such as /M-W=, /WILCOXON= or /K-W="));
    }
    Ok(Command::NparTests(tests))
}

pub(crate) fn logistic(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "LOGISTIC REGRESSION";
    let bad = |expected| SyntaxError::Expected { command: CMD, expected };
    let (mut dependent, mut covariates, mut ci, mut iterations, mut cut) = (None, None, None, 20usize, 0.5);
    for (name, tokens) in subcommands(rest) {
        match name {
            None => {
                // `y` or the older `y WITH covariates`; SPSS pastes `VARIABLES y` without `=`.
                let tokens = match tokens {
                    [Token::Word(v), rest @ ..] if is_keyword(v, "VARIABLES") => rest,
                    tokens => tokens,
                };
                let with = tokens.iter().position(|t| matches!(t, Token::Word(w) if w.eq_ignore_ascii_case("WITH")));
                let head = with.map_or(tokens, |at| tokens.get(..at).unwrap_or_default());
                dependent = Some(match words(head).as_deref() {
                    Some([y]) => y.clone(),
                    _ => return Err(bad("one dependent variable")),
                });
                if let Some(at) = with {
                    covariates = Some(words(tokens.get(at + 1..).unwrap_or_default()).filter(|v| !v.is_empty()).ok_or(bad("covariates after WITH"))?);
                }
            }
            Some(n) if is_keyword(n, "VARIABLES") => {
                dependent = Some(match words(tokens).as_deref() {
                    Some([y]) => y.clone(),
                    _ => return Err(bad("VARIABLES=dependent")),
                });
            }
            Some(n) if is_keyword(n, "METHOD") => {
                if covariates.is_some() {
                    return Err(unsupported(CMD, "more than one block of covariates"));
                }
                covariates = Some(match words(tokens).as_deref() {
                    Some([method, vars @ ..]) if is_keyword(method, "ENTER") && !vars.is_empty() => vars.to_vec(),
                    _ => return Err(unsupported(CMD, "methods other than ENTER with covariates")),
                });
            }
            Some(n) if is_keyword(n, "PRINT") => match tokens {
                [Token::Word(w), Token::LParen, Token::Word(level), Token::RParen] if w.eq_ignore_ascii_case("CI") => {
                    ci = Some(level.parse::<f64>().ok().filter(|l| *l > 0.0 && *l < 100.0).ok_or(bad("CI(level) in percent"))? / 100.0);
                }
                _ => return Err(unsupported(CMD, "/PRINT other than CI(level)")),
            },
            Some(n) if is_keyword(n, "CRITERIA") => {
                let mut i = 0;
                while let Some(Token::Word(w)) = tokens.get(i) {
                    let value = match (tokens.get(i + 1), tokens.get(i + 2), tokens.get(i + 3)) {
                        (Some(Token::LParen), Some(Token::Word(v)), Some(Token::RParen)) => v.parse::<f64>().ok(),
                        _ => None,
                    };
                    let value = value.ok_or(bad("CRITERIA keywords with a value, such as ITERATE(20)"))?;
                    match w.to_ascii_uppercase().as_str() {
                        "ITERATE" if value >= 1.0 => iterations = value as usize,
                        "CUT" if value > 0.0 && value < 1.0 => cut = value,
                        // Entry criteria only matter for stepwise methods.
                        "PIN" | "POUT" | "EPS" | "LCON" | "BCON" => {}
                        other => return Err(unsupported(CMD, &format!("/CRITERIA={other}"))),
                    }
                    i += 4;
                }
            }
            Some(n) => return Err(unsupported(CMD, &format!("/{n}"))),
        }
    }
    let dependent = dependent.ok_or(bad("the dependent variable"))?;
    let covariates = covariates.ok_or(bad("/METHOD=ENTER covariates"))?;
    Ok(Command::Logistic { dependent, covariates, ci, iterations, cut })
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
            pairs = Some(pair_list(tokens).map_err(bad)?);
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
    let (mut lists, mut descriptives, mut homogeneity, mut posthoc, mut alpha) = (None, false, false, Vec::new(), 0.05);
    for (name, tokens) in subcommands(rest) {
        match name {
            None => lists = Some(split_by(CMD, tokens)?),
            Some(n) if is_keyword(n, "VARIABLES") => lists = Some(split_by(CMD, tokens)?),
            Some(n) if is_keyword(n, "POSTHOC") => {
                let mut i = 0;
                while let Some(t) = tokens.get(i) {
                    match t {
                        Token::Word(w) if is_keyword(w, "TUKEY") => posthoc.push(PostHocTest::Tukey),
                        Token::Word(w) if is_keyword(w, "BONFERRONI") => posthoc.push(PostHocTest::Bonferroni),
                        Token::Word(w) if is_keyword(w, "ALPHA") => {
                            alpha = match (tokens.get(i + 1), tokens.get(i + 2), tokens.get(i + 3)) {
                                (Some(Token::LParen), Some(Token::Word(a)), Some(Token::RParen)) => {
                                    a.parse::<f64>().ok().filter(|a| *a > 0.0 && *a < 1.0)
                                }
                                _ => None,
                            }
                            .ok_or(SyntaxError::Expected { command: CMD, expected: "ALPHA(0.05)" })?;
                            i += 3;
                        }
                        Token::Word(w) => return Err(unsupported(CMD, &format!("/POSTHOC={w}"))),
                        _ => return Err(SyntaxError::Expected { command: CMD, expected: "post hoc test names" }),
                    }
                    i += 1;
                }
            }
            Some(n) if is_keyword(n, "MISSING") => {
                if !matches!(tokens, [Token::Word(w)] if is_keyword(w, "ANALYSIS")) {
                    return Err(unsupported(CMD, "/MISSING other than ANALYSIS"));
                }
            }
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
    Ok(Command::Oneway { dependents, factor, descriptives, homogeneity, posthoc, alpha })
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
    let mut tolerance = 0.0001;
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
        } else if is_keyword(name, "CRITERIA") {
            // TOLERANCE(t) decides which predictors ENTER takes; PIN, POUT and the others only
            // matter for stepwise methods.
            let at = tokens.iter().position(|t| matches!(t, Token::Word(w) if w.eq_ignore_ascii_case("TOLERANCE")));
            if let Some(at) = at {
                tolerance = match (tokens.get(at + 1), tokens.get(at + 2), tokens.get(at + 3)) {
                    (Some(Token::LParen), Some(Token::Word(t)), Some(Token::RParen)) => {
                        t.parse::<f64>().ok().filter(|t| *t > 0.0 && *t < 1.0).ok_or(bad("TOLERANCE(t) with 0 < t < 1, such as TOLERANCE(.0001)"))?
                    }
                    _ => return Err(bad("TOLERANCE(t) with 0 < t < 1, such as TOLERANCE(.0001)")),
                };
            }
        } else if is_keyword(name, "NOORIGIN") {
            // The default: a constant in the model.
        } else {
            return Err(unsupported(CMD, &format!("/{name}")));
        }
    }
    let dependent = dependent.ok_or(bad("/DEPENDENT=variable"))?;
    let predictors = predictors.ok_or(bad("/METHOD=ENTER predictors"))?;
    Ok(Command::Regression { dependent, predictors, ci, tolerance })
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
                "REGRESSION /MISSING LISTWISE /STATISTICS COEFF OUTS CI(99) R ANOVA /CRITERIA=PIN(.05) POUT(.10) TOLERANCE(.05) /NOORIGIN /DEPENDENT renda /METHOD=ENTER idade escolaridade."
            ),
            Command::Regression { dependent: "renda".into(), predictors: names(&["idade", "escolaridade"]), ci: Some(0.99), tolerance: 0.05 }
        );
        assert!(parse("REGRESSION /DEPENDENT y /METHOD=STEPWISE x.").is_err());
        assert!(parse("REGRESSION /DEPENDENT y /METHOD=ENTER a /METHOD=ENTER b.").is_err());
        assert!(parse("REGRESSION /METHOD=ENTER a.").is_err());
    }

    #[test]
    fn oneway_syntax() {
        assert_eq!(
            one("ONEWAY renda nota BY escola /STATISTICS DESCRIPTIVES HOMOGENEITY."),
            Command::Oneway {
                dependents: names(&["renda", "nota"]),
                factor: "escola".into(),
                descriptives: true,
                homogeneity: true,
                posthoc: vec![],
                alpha: 0.05
            }
        );
        assert_eq!(
            one("ONEWAY nota BY escola /MISSING ANALYSIS /POSTHOC=TUKEY BONFERRONI ALPHA(0.01)."),
            Command::Oneway {
                dependents: names(&["nota"]),
                factor: "escola".into(),
                descriptives: false,
                homogeneity: false,
                posthoc: vec![PostHocTest::Tukey, PostHocTest::Bonferroni],
                alpha: 0.01
            }
        );
        assert!(parse("ONEWAY y BY a b.").is_err(), "one factor");
        assert!(parse("ONEWAY y BY g /POSTHOC=SCHEFFE.").is_err());
    }

    #[test]
    fn examine_syntax_as_spss_pastes_it() {
        assert_eq!(
            one(
                "EXAMINE VARIABLES=pos BY regiao /PLOT BOXPLOT STEMLEAF /COMPARE GROUPS /STATISTICS DESCRIPTIVES /CINTERVAL 95 /MISSING LISTWISE /NOTOTAL."
            ),
            Command::Examine { variables: names(&["pos"]), by: Some("regiao".into()), descriptives: true, boxplot: true, stemleaf: true }
        );
        assert_eq!(
            one("EXAMINE renda /PLOT=BOXPLOT /STATISTICS=NONE."),
            Command::Examine { variables: names(&["renda"]), by: None, descriptives: false, boxplot: true, stemleaf: false }
        );
        assert!(parse("EXAMINE x /PLOT=NPPLOT.").is_err());
        assert!(parse("EXAMINE x /CINTERVAL 90.").is_err());
    }

    #[test]
    fn graph_syntax() {
        assert_eq!(
            one("GRAPH /HISTOGRAM(NORMAL)=renda /TITLE='Renda mensal'."),
            Command::Graph { chart: GraphSpec::Histogram { variable: "renda".into(), normal: true }, title: Some("Renda mensal".into()) }
        );
        assert_eq!(
            one("GRAPH /BAR(SIMPLE)=COUNT BY regiao."),
            Command::Graph { chart: GraphSpec::Bar { value: BarStat::Count, by: "regiao".into() }, title: None }
        );
        assert_eq!(
            one("GRAPH /BAR(SIMPLE)=MEAN(renda) BY regiao."),
            Command::Graph { chart: GraphSpec::Bar { value: BarStat::Mean("renda".into()), by: "regiao".into() }, title: None }
        );
        assert_eq!(
            one("GRAPH /SCATTERPLOT(BIVAR)=pre WITH pos."),
            Command::Graph { chart: GraphSpec::Scatter { x: "pre".into(), y: "pos".into() }, title: None }
        );
        assert!(parse("GRAPH /PIE=COUNT BY x.").is_err());
        assert!(parse("GRAPH /BAR(STACKED)=COUNT BY x.").is_err());
        assert!(parse("GRAPH /TITLE='x'.").is_err(), "a title needs a chart");
    }

    #[test]
    fn npar_tests_syntax() {
        assert_eq!(
            one("NPAR TESTS /M-W= renda nota BY sexo(1 2) /WILCOXON=pre WITH pos (PAIRED) /K-W=nota BY escola(3 1) /MISSING ANALYSIS."),
            Command::NparTests(vec![
                NparSpec::MannWhitney { variables: names(&["renda", "nota"]), group: "sexo".into(), first: 1.0, second: 2.0 },
                NparSpec::Wilcoxon { pairs: vec![("pre".into(), "pos".into())] },
                NparSpec::KruskalWallis { variables: names(&["nota"]), group: "escola".into(), low: 1.0, high: 3.0 },
            ])
        );
        assert!(parse("NPAR TESTS /M-W=x BY g(1).").is_err());
        assert!(parse("NPAR TESTS /RUNS(MEAN)=x.").is_err());
    }

    #[test]
    fn logistic_syntax_as_spss_pastes_it() {
        assert_eq!(
            one("LOGISTIC REGRESSION VARIABLES aprovado /METHOD=ENTER pre sexo /PRINT=CI(95) /CRITERIA=PIN(.05) POUT(.10) ITERATE(30) CUT(.4)."),
            Command::Logistic { dependent: "aprovado".into(), covariates: names(&["pre", "sexo"]), ci: Some(0.95), iterations: 30, cut: 0.4 }
        );
        assert_eq!(
            one("LOGISTIC REGRESSION aprovado WITH pre."),
            Command::Logistic { dependent: "aprovado".into(), covariates: names(&["pre"]), ci: None, iterations: 20, cut: 0.5 }
        );
        assert!(parse("LOGISTIC REGRESSION VARIABLES y /METHOD=FSTEP(LR) x.").is_err());
        assert!(parse("LOGISTIC REGRESSION VARIABLES y /METHOD=ENTER x /CATEGORICAL=x.").is_err());
    }
}
