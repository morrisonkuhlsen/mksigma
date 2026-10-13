//! MK Sigma command syntax. Commands end with a period; keywords are case-insensitive and may be
//! abbreviated to their first three letters, as in the public SPSS Command Syntax Reference.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use thiserror::Error;

mod dictionary;
mod lexer;

pub use dictionary::{Level, Literal, MissingSpec, NewVariables, ValueLabelList};
use lexer::Token;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// `DESCRIPTIVES [VARIABLES=] var...`
    Descriptives { variables: Vec<String> },
    /// `DISPLAY DICTIONARY`
    DisplayDictionary,
    /// `FREQUENCIES [VARIABLES=] var... [/FORMAT=...] [/STATISTICS=...]`
    Frequencies { variables: Vec<String>, table: bool, order: FreqOrder, statistics: Vec<FreqStatistic> },
    /// `VARIABLE LABELS var 'label' [/var 'label']...`
    VariableLabels(Vec<(String, String)>),
    /// `VALUE LABELS varlist value 'label'... [/varlist ...]` (replaces existing labels)
    ValueLabels(Vec<(Vec<String>, ValueLabelList)>),
    /// `MISSING VALUES varlist (values) [varlist (values)]...`
    MissingValues(Vec<(Vec<String>, MissingSpec)>),
    /// `VARIABLE LEVEL varlist (NOMINAL|ORDINAL|SCALE)...`
    VariableLevel(Vec<(Vec<String>, Level)>),
    /// `FORMATS varlist (format)...`, the format as written (`F8.2`)
    Formats(Vec<(Vec<String>, String)>),
    /// `RENAME VARIABLES (old = new)...`
    RenameVariables(Vec<(String, String)>),
    /// `NUMERIC varlist [(format)] [/varlist [(format)]]...`: new numeric variables (F8.2 when no
    /// format is given), system-missing in every case
    Numeric(NewVariables),
    /// `STRING varlist (A<width>) [/...]`: new string variables, blank in every case
    String(Vec<(Vec<String>, String)>),
    /// `DELETE VARIABLES varlist`
    DeleteVariables(Vec<String>),
}

/// Row order of a frequency table (`/FORMAT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FreqOrder {
    #[default]
    AscendingValue,
    DescendingValue,
    AscendingFrequency,
    DescendingFrequency,
}

/// A statistic `FREQUENCIES /STATISTICS` can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreqStatistic {
    Mean,
    SeMean,
    StdDev,
    Variance,
    Range,
    Minimum,
    Maximum,
    Sum,
    Mode,
}

impl FreqStatistic {
    /// What `/STATISTICS` alone (or `=DEFAULT`) asks for.
    pub const DEFAULT: [FreqStatistic; 4] = [FreqStatistic::Mean, FreqStatistic::StdDev, FreqStatistic::Minimum, FreqStatistic::Maximum];
}

#[derive(Debug, Clone, Error, PartialEq)]
pub enum SyntaxError {
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("{command}: expected {expected}")]
    Expected { command: &'static str, expected: &'static str },
    #[error("unexpected character {0:?}")]
    UnexpectedChar(char),
    #[error("a quoted string is not closed")]
    UnterminatedString,
    #[error("{command}: {what} is not supported yet")]
    Unsupported { command: &'static str, what: String },
}

/// A command's variable list (after an optional `VARIABLES=`) and its other `/NAME=values`
/// subcommands, in order.
struct Parts<'a> {
    variables: Vec<String>,
    subcommands: Vec<(&'a str, Vec<&'a str>)>,
}

fn parts<'a>(command: &'static str, rest: &'a [Token]) -> Result<Parts<'a>, SyntaxError> {
    let words = |tokens: &'a [Token]| -> Result<Vec<&'a str>, SyntaxError> {
        tokens
            .iter()
            .map(|t| match t {
                Token::Word(w) => Ok(w.as_str()),
                _ => Err(SyntaxError::Expected { command, expected: "a name or keyword" }),
            })
            .collect()
    };
    let mut variables: Option<Vec<String>> = None;
    let mut subcommands = Vec::new();
    for (i, group) in rest.split(|t| *t == Token::Slash).enumerate() {
        match group {
            [] => {}
            [Token::Word(w), Token::Equals, list @ ..] if is_keyword(w, "VARIABLES") => {
                variables = Some(words(list)?.into_iter().map(str::to_string).collect());
            }
            [Token::Word(w), Token::Equals, values @ ..] => subcommands.push((w.as_str(), words(values)?)),
            [Token::Word(w)] if i > 0 => subcommands.push((w.as_str(), Vec::new())),
            list if i == 0 => variables = Some(words(list)?.into_iter().map(str::to_string).collect()),
            _ => return Err(SyntaxError::Expected { command, expected: "a subcommand" }),
        }
    }
    match variables {
        Some(variables) if !variables.is_empty() => Ok(Parts { variables, subcommands }),
        _ => Err(SyntaxError::Expected { command, expected: "a variable list" }),
    }
}

fn unsupported(command: &'static str, what: &str) -> SyntaxError {
    SyntaxError::Unsupported { command, what: what.to_uppercase() }
}

/// Whether `word` names `keyword`: the whole keyword or a prefix of at least three letters.
fn is_keyword(word: &str, keyword: &str) -> bool {
    let n = word.len();
    n >= 3.min(keyword.len()) && n <= keyword.len() && keyword.get(..n).is_some_and(|k| k.eq_ignore_ascii_case(word))
}

pub fn parse(src: &str) -> Result<Vec<Command>, SyntaxError> {
    Ok(parse_with_source(src)?.into_iter().map(|(command, _)| command).collect())
}

/// Each command with its source text, as written (for the session history).
pub fn parse_with_source(src: &str) -> Result<Vec<(Command, String)>, SyntaxError> {
    lexer::commands(src)?.into_iter().map(|c| Ok((parse_command(&c.tokens)?, c.source))).collect()
}

/// `text` as a quoted syntax string (`it's` → `'it''s'`).
pub fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
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
        let Parts { variables, subcommands } = parts("DESCRIPTIVES", rest)?;
        if let Some((sub, _)) = subcommands.first() {
            return Err(unsupported("DESCRIPTIVES", &format!("/{sub}")));
        }
        return Ok(Command::Descriptives { variables });
    }
    if is_keyword(name, "FREQUENCIES") {
        return frequencies(rest);
    }
    if let Some(result) = dictionary::parse(name, rest) {
        return result;
    }
    Err(SyntaxError::UnknownCommand(name.to_string()))
}

fn frequencies(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "FREQUENCIES";
    let Parts { variables, subcommands } = parts(CMD, rest)?;
    let (mut table, mut order, mut statistics) = (true, FreqOrder::default(), Vec::new());
    for (sub, values) in subcommands {
        if is_keyword(sub, "FORMAT") {
            for v in values {
                match v {
                    v if is_keyword(v, "NOTABLE") => table = false,
                    v if is_keyword(v, "TABLE") => table = true,
                    v if is_keyword(v, "AVALUE") => order = FreqOrder::AscendingValue,
                    v if is_keyword(v, "DVALUE") => order = FreqOrder::DescendingValue,
                    v if is_keyword(v, "AFREQ") => order = FreqOrder::AscendingFrequency,
                    v if is_keyword(v, "DFREQ") => order = FreqOrder::DescendingFrequency,
                    v => return Err(unsupported(CMD, &format!("/FORMAT={v}"))),
                }
            }
        } else if is_keyword(sub, "STATISTICS") {
            if values.is_empty() {
                statistics.extend(FreqStatistic::DEFAULT);
            }
            for v in values {
                let stat = match v {
                    v if is_keyword(v, "DEFAULT") => {
                        statistics.extend(FreqStatistic::DEFAULT);
                        continue;
                    }
                    v if is_keyword(v, "NONE") => {
                        statistics.clear();
                        continue;
                    }
                    v if is_keyword(v, "MEAN") => FreqStatistic::Mean,
                    v if is_keyword(v, "SEMEAN") => FreqStatistic::SeMean,
                    v if is_keyword(v, "STDDEV") => FreqStatistic::StdDev,
                    v if is_keyword(v, "VARIANCE") => FreqStatistic::Variance,
                    v if is_keyword(v, "RANGE") => FreqStatistic::Range,
                    v if is_keyword(v, "MINIMUM") => FreqStatistic::Minimum,
                    v if is_keyword(v, "MAXIMUM") => FreqStatistic::Maximum,
                    v if is_keyword(v, "SUM") => FreqStatistic::Sum,
                    v if is_keyword(v, "MODE") => FreqStatistic::Mode,
                    v => return Err(unsupported(CMD, &format!("/STATISTICS={v}"))),
                };
                statistics.push(stat);
            }
        } else {
            return Err(unsupported(CMD, &format!("/{sub}")));
        }
    }
    // Shown in a fixed order, once each, whatever order they were asked in.
    let all = [
        FreqStatistic::Mean,
        FreqStatistic::SeMean,
        FreqStatistic::Mode,
        FreqStatistic::StdDev,
        FreqStatistic::Variance,
        FreqStatistic::Range,
        FreqStatistic::Minimum,
        FreqStatistic::Maximum,
        FreqStatistic::Sum,
    ];
    let statistics = all.into_iter().filter(|s| statistics.contains(s)).collect();
    Ok(Command::Frequencies { variables, table, order, statistics })
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
    fn frequencies() {
        let freq = |vars: &[&str], table, order, statistics: &[FreqStatistic]| Command::Frequencies {
            variables: vars.iter().map(|v| v.to_string()).collect(),
            table,
            order,
            statistics: statistics.to_vec(),
        };
        assert_eq!(parse("FREQUENCIES sex."), Ok(vec![freq(&["sex"], true, FreqOrder::AscendingValue, &[])]));
        assert_eq!(
            parse("FREQUENCIES /VARIABLES=age sex /FORMAT=NOTABLE DFREQ /STATISTICS=MAX MODE mean."),
            Ok(vec![freq(
                &["age", "sex"],
                false,
                FreqOrder::DescendingFrequency,
                &[FreqStatistic::Mean, FreqStatistic::Mode, FreqStatistic::Maximum]
            )])
        );
        assert_eq!(parse("freq age /statistics."), Ok(vec![freq(&["age"], true, FreqOrder::AscendingValue, &FreqStatistic::DEFAULT)]));
        assert_eq!(
            parse("FREQUENCIES age /STATISTICS=MEDIAN."),
            Err(SyntaxError::Unsupported { command: "FREQUENCIES", what: "/STATISTICS=MEDIAN".into() })
        );
        assert!(matches!(parse("FREQUENCIES age /BARCHART."), Err(SyntaxError::Unsupported { .. })));
        assert!(matches!(parse("FREQUENCIES /FORMAT=NOTABLE."), Err(SyntaxError::Expected { .. })));
    }

    #[test]
    fn descriptives_rejects_subcommands_it_does_not_know() {
        assert!(matches!(parse("DESCRIPTIVES age /SAVE."), Err(SyntaxError::Unsupported { .. })));
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
