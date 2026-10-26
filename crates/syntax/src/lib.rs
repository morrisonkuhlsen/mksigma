//! MK Sigma command syntax. Commands end with a period; keywords are case-insensitive and may be
//! abbreviated to their first three letters, as in the public SPSS Command Syntax Reference.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use thiserror::Error;

mod data;
mod dictionary;
mod expr;
mod lexer;
mod procedures;
mod slice;
mod style;
mod transform;

pub use data::GetData;
pub use dictionary::{Level, Literal, MissingSpec, NewVariables, ValueLabelList};
pub use expr::{BinOp, Expr};
use lexer::Token;
pub use procedures::{BarStat, CrosstabCells, GraphSpec, NparSpec, PairList, PostHocTest, TTestSpec};
pub use slice::SliceSpec;
pub use style::StyleSpec;
pub use transform::{RecodeFrom, RecodeGroup, RecodeSpec, RecodeTo};

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// `GET DATA /TYPE=TXT /FILE='...'`: a new active dataset from delimited text.
    GetData(GetData),
    /// `OUTPUT STYLE [/FONT=...] [/SIZE=n] [/PALETTE=...] [/DECIMALS=AUTO|n]`
    OutputStyle(StyleSpec),
    /// `GET FILE='...'`: a new active dataset from a system file.
    GetFile(String),
    /// `GET STATA FILE='...'`: a new active dataset from a Stata file (`.dta`).
    GetStata(String),
    /// `GET SAS DATA='...'`: a new active dataset from a SAS transport file (`.xpt`).
    GetSas(String),
    /// `SAVE TRANSLATE OUTFILE='...' /TYPE=STATA [/REPLACE]`: the active dataset as a Stata
    /// file; an existing file is overwritten only with `/REPLACE`.
    SaveStata { file: String, replace: bool },
    /// `DASHBOARD /TILES=(analysis item [WIDE])...`: the dashboard's tiles, counted from 1, and
    /// whether each takes a whole row.
    Dashboard(Vec<(usize, usize, bool)>),
    /// `SLICE var (values)`, `SLICE var (low THRU high)`, `SLICE var OFF`; `None` is `SLICE OFF`.
    Slice(Option<(String, SliceSpec)>),
    /// `DESCRIPTIVES [VARIABLES=] var...`
    Descriptives { variables: Vec<String> },
    /// `DISPLAY DICTIONARY`
    DisplayDictionary,
    /// `EXECUTE`: runs pending transformations. They already run as they are given, so it
    /// does nothing; syntax pasted from elsewhere has one after almost every transformation.
    Execute,
    /// `FREQUENCIES [VARIABLES=] var... [/FORMAT=...] [/STATISTICS=...]`
    Frequencies {
        variables: Vec<String>,
        table: bool,
        order: FreqOrder,
        statistics: Vec<FreqStatistic>,
        /// `/HISTOGRAM [NORMAL]`: a histogram per variable, with the normal curve if `true`.
        histogram: Option<bool>,
        /// `/BARCHART`: a bar chart of the frequencies per variable.
        barchart: bool,
    },
    /// `GRAPH /HISTOGRAM[(NORMAL)]=x`, `/BAR(SIMPLE)=COUNT BY x`, `/BAR(SIMPLE)=MEAN(y) BY x`,
    /// `/SCATTERPLOT(BIVAR)=x WITH y`, with an optional `/TITLE='...'`.
    Graph { chart: GraphSpec, title: Option<String> },
    /// `EXAMINE VARIABLES=vars [BY group] [/PLOT=BOXPLOT STEMLEAF] [/STATISTICS=DESCRIPTIVES]`
    Examine { variables: Vec<String>, by: Option<String>, descriptives: bool, boxplot: bool, stemleaf: bool },
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
    /// `COMPUTE target = expression`
    Compute { target: String, expr: Expr },
    /// `IF (condition) target = expression`
    If { cond: Expr, target: String, expr: Expr },
    /// `RECODE varlist (old = new)... [INTO varlist] [/...]`
    Recode(Vec<RecodeGroup>),
    /// `SELECT IF condition`: keeps the cases where it is true.
    SelectIf(Expr),
    /// `CROSSTABS [/TABLES=] rows BY columns [/STATISTICS=CHISQ] [/CELLS=...]`
    Crosstabs { rows: Vec<String>, columns: Vec<String>, cells: CrosstabCells, chisq: bool },
    /// `T-TEST` one-sample, independent-samples or paired; `ci` is the confidence level.
    TTest { test: TTestSpec, ci: f64 },
    /// `CORRELATIONS [/VARIABLES=] vars [WITH vars] [/PRINT=...] [/MISSING=...]`
    Correlations { variables: Vec<String>, with: Vec<String>, one_tailed: bool, flag: bool, listwise: bool },
    /// `REGRESSION /DEPENDENT=y /METHOD=ENTER predictors [/STATISTICS ... CI(95)]`
    Regression { dependent: String, predictors: Vec<String>, ci: Option<f64>, tolerance: f64, origin: bool },
    /// `ONEWAY dependents BY factor [/STATISTICS=DESCRIPTIVES HOMOGENEITY]`
    Oneway { dependents: Vec<String>, factor: String, descriptives: bool, homogeneity: bool, posthoc: Vec<PostHocTest>, alpha: f64 },
    /// `NPAR TESTS /M-W=... /WILCOXON=... /K-W=...`
    NparTests(Vec<NparSpec>),
    /// `LOGISTIC REGRESSION [VARIABLES=] y /METHOD=ENTER covariates [/PRINT=CI(95)] [/CRITERIA=...]`
    Logistic { dependent: String, covariates: Vec<String>, ci: Option<f64>, iterations: usize, cut: f64 },
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
    #[error("{}", mksigma_i18n::tr!("err-unknown-command", name = .0.as_str()))]
    UnknownCommand(String),
    #[error("{}", mksigma_i18n::tr!("err-expected", command = *command, expected = mksigma_i18n::phrase(expected)))]
    Expected { command: &'static str, expected: &'static str },
    #[error("{}", mksigma_i18n::tr!("err-unexpected-char", char = format!("{:?}", .0)))]
    UnexpectedChar(char),
    #[error("{}", mksigma_i18n::tr!("err-unclosed-string"))]
    UnterminatedString,
    #[error("{}", mksigma_i18n::tr!("err-expression-too-deep"))]
    TooDeep,
    #[error("{}", mksigma_i18n::tr!("err-unsupported", command = *command, what = mksigma_i18n::phrase(what)))]
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
            [Token::Word(w), values @ ..] if i > 0 => subcommands.push((w.as_str(), words(values)?)),
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

/// Outside expressions a `-` is always a sign: `VALUE LABELS q -1 'No answer'`.
fn merge_signs(tokens: &[Token]) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut iter = tokens.iter().peekable();
    while let Some(t) = iter.next() {
        if *t == Token::Minus
            && let Some(Token::Word(w)) = iter.peek()
            && w.starts_with(|c: char| c.is_ascii_digit() || c == '.')
        {
            out.push(Token::Word(format!("-{w}")));
            iter.next();
            continue;
        }
        out.push(t.clone());
    }
    out
}

fn parse_command(tokens: &[Token]) -> Result<Command, SyntaxError> {
    let (name, rest) = match tokens.split_first() {
        Some((Token::Word(w), rest)) => (w.as_str(), rest),
        _ => return Err(SyntaxError::UnknownCommand(String::new())),
    };
    if let Some(result) = transform::parse(name, rest) {
        return result;
    }
    let rest = &merge_signs(rest)[..];
    // `T-TEST` reaches here as `T`, `-`, `TEST`.
    if name.eq_ignore_ascii_case("T")
        && let [Token::Minus, Token::Word(test), rest @ ..] = rest
        && test.eq_ignore_ascii_case("TEST")
    {
        return procedures::t_test(rest);
    }
    if is_keyword(name, "CROSSTABS") {
        return procedures::crosstabs(rest);
    }
    if is_keyword(name, "ONEWAY") {
        return procedures::oneway(rest);
    }
    if is_keyword(name, "EXAMINE") {
        return procedures::examine(rest);
    }
    if is_keyword(name, "GRAPH") {
        return procedures::graph(rest);
    }
    if is_keyword(name, "CORRELATIONS") {
        return procedures::correlations(rest);
    }
    if is_keyword(name, "REGRESSION") {
        return procedures::regression(rest);
    }
    let second = |word: &str| matches!(rest.first(), Some(Token::Word(w)) if is_keyword(w, word));
    if is_keyword(name, "NPAR") && second("TESTS") {
        return procedures::npar_tests(rest.get(1..).unwrap_or_default());
    }
    if is_keyword(name, "LOGISTIC") && second("REGRESSION") {
        return procedures::logistic(rest.get(1..).unwrap_or_default());
    }
    if is_keyword(name, "OUTPUT") && second("STYLE") {
        return style::output_style(rest.get(1..).unwrap_or_default());
    }
    if name.eq_ignore_ascii_case("SLICE") {
        return slice::slice(rest);
    }
    if is_keyword(name, "GET") && second("DATA") {
        return data::get_data(rest.get(1..).unwrap_or_default());
    }
    if is_keyword(name, "GET") && second("FILE") {
        return data::get_file(rest);
    }
    if is_keyword(name, "SAVE") && second("TRANSLATE") {
        return data::save_translate(rest.get(1..).unwrap_or_default());
    }
    if is_keyword(name, "GET") && second("SAS") {
        return data::get_sas(rest.get(1..).unwrap_or_default());
    }
    if is_keyword(name, "GET") && second("STATA") {
        return data::get_stata(rest.get(1..).unwrap_or_default());
    }
    if is_keyword(name, "DASHBOARD") {
        return data::dashboard(rest);
    }
    if is_keyword(name, "DISPLAY") {
        return match rest {
            [] => Ok(Command::DisplayDictionary),
            [Token::Word(w)] if is_keyword(w, "DICTIONARY") => Ok(Command::DisplayDictionary),
            _ => Err(SyntaxError::Expected { command: "DISPLAY", expected: "DICTIONARY" }),
        };
    }
    if is_keyword(name, "EXECUTE") {
        return if rest.is_empty() {
            Ok(Command::Execute)
        } else {
            Err(SyntaxError::Expected { command: "EXECUTE", expected: "the end of the command" })
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
    let (mut table, mut order, mut statistics, mut histogram, mut barchart) = (true, FreqOrder::default(), Vec::new(), None, false);
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
        } else if is_keyword(sub, "HISTOGRAM") {
            histogram = Some(match values.as_slice() {
                [] => false,
                [v] if is_keyword(v, "NORMAL") => true,
                [v] if is_keyword(v, "NONORMAL") => false,
                _ => return Err(unsupported(CMD, "/HISTOGRAM options other than NORMAL")),
            });
        } else if is_keyword(sub, "BARCHART") {
            if !values.iter().all(|v| is_keyword(v, "FREQ")) {
                return Err(unsupported(CMD, "/BARCHART options other than FREQ"));
            }
            barchart = true;
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
    Ok(Command::Frequencies { variables, table, order, statistics, histogram, barchart })
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
            histogram: None,
            barchart: false,
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
        assert!(matches!(
            &parse("FREQUENCIES age /HISTOGRAM NORMAL /BARCHART.").unwrap()[..],
            [Command::Frequencies { histogram: Some(true), barchart: true, .. }]
        ));
        assert!(matches!(parse("FREQUENCIES age /PIECHART."), Err(SyntaxError::Unsupported { .. })));
        assert!(matches!(parse("FREQUENCIES /FORMAT=NOTABLE."), Err(SyntaxError::Expected { .. })));
    }

    #[test]
    fn descriptives_rejects_subcommands_it_does_not_know() {
        assert!(matches!(parse("DESCRIPTIVES age /SAVE."), Err(SyntaxError::Unsupported { .. })));
    }

    #[test]
    fn negative_values_outside_expressions() {
        assert_eq!(
            parse("VALUE LABELS q -1 'No answer' -2.5 'Other'."),
            Ok(vec![Command::ValueLabels(vec![(
                vec!["q".to_string()],
                vec![(Literal::Num(-1.0), "No answer".into()), (Literal::Num(-2.5), "Other".into())]
            )])])
        );
        assert_eq!(
            parse("MISSING VALUES x (-1, -9)."),
            Ok(vec![Command::MissingValues(vec![(vec!["x".to_string()], MissingSpec::Discrete(vec![Literal::Num(-1.0), Literal::Num(-9.0)]))])])
        );
    }

    #[test]
    fn display_dictionary() {
        assert_eq!(parse("DISPLAY DICTIONARY."), Ok(vec![Command::DisplayDictionary]));
        assert_eq!(parse("disp dic."), Ok(vec![Command::DisplayDictionary]));
        assert_eq!(parse("EXECUTE.\nexe."), Ok(vec![Command::Execute, Command::Execute]));
        assert!(parse("EXECUTE x.").is_err());
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
        assert_eq!(parse("DESCRIPTIVES a % b."), Err(SyntaxError::UnexpectedChar('%')));
    }
}
