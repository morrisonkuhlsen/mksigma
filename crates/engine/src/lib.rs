//! MK Sigma session: the active dataset plus the output so far. Every user-visible action goes
//! through here (syntax, menus, CLI, later MCP), so all frontends behave the same.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mksigma_core::{Dataset, DatasetError, Value, Variable};
use mksigma_output::{Item, Output};
use mksigma_syntax::{Command, SyntaxError};
use thiserror::Error;

mod dictionary;
mod edit;

pub use mksigma_core as core;
pub use mksigma_output as output;
pub use mksigma_syntax::quote;

#[derive(Debug, Error, PartialEq)]
pub enum EngineError {
    #[error(transparent)]
    Syntax(#[from] SyntaxError),
    #[error(transparent)]
    Stats(#[from] mksigma_stats::StatsError),
    #[error(transparent)]
    Dataset(#[from] DatasetError),
    #[error(transparent)]
    Sav(#[from] mksigma_sav::SavError),
    #[error("variable {0} does not exist")]
    UnknownVariable(String),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Default)]
pub struct Session {
    pub dataset: Dataset,
    pub output: Output,
    /// Every command that ran successfully, as written: the session's reproducible history.
    pub journal: Vec<String>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// A session with a small built-in dataset, for demos and tests.
    pub fn with_sample() -> Result<Self, EngineError> {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("age").with_label("Age (years)"))?;
        ds.add_variable(Variable::numeric("income").with_label("Monthly income"))?;
        ds.add_variable(Variable::numeric("score").with_label("Test score"))?;
        let cases = [
            (23.0, 1800.0, 61.0),
            (35.0, 4200.0, 74.0),
            (41.0, 5100.0, 69.0),
            (29.0, 2600.0, 80.0),
            (52.0, 6900.0, 58.0),
            (47.0, 3900.0, 72.0),
            (31.0, 3100.0, 88.0),
            (26.0, 2200.0, 77.0),
        ];
        for (age, income, score) in cases {
            ds.push_case(vec![Value::Num(age), Value::Num(income), Value::Num(score)])?;
        }
        Ok(Self { dataset: ds, ..Self::default() })
    }

    /// Replaces the active dataset with a system file's. Returns the reader's warnings.
    pub fn open_sav(&mut self, bytes: &[u8]) -> Result<Vec<String>, EngineError> {
        let file = mksigma_sav::read(bytes)?;
        self.dataset = file.dataset;
        Ok(file.warnings)
    }

    /// Runs a syntax program. Commands before a failing one keep their effect and output, and
    /// go into the journal.
    pub fn run_syntax(&mut self, src: &str) -> Result<(), EngineError> {
        for (command, source) in mksigma_syntax::parse_with_source(src)? {
            self.execute(&command)?;
            self.journal.push(source);
        }
        Ok(())
    }

    /// Edits one cell from typed text (Data View). Not a syntax command, so not journaled.
    pub fn set_cell(&mut self, case: usize, variable: usize, input: &str) -> Result<(), EngineError> {
        edit::set_cell(&mut self.dataset, case, variable, input)
    }

    pub fn execute(&mut self, command: &Command) -> Result<(), EngineError> {
        match command {
            Command::VariableLabels(_)
            | Command::ValueLabels(_)
            | Command::MissingValues(_)
            | Command::VariableLevel(_)
            | Command::Formats(_)
            | Command::RenameVariables(_) => edit::apply(&mut self.dataset, command)?,
            Command::Descriptives { variables } => {
                let table = mksigma_stats::descriptives(&self.dataset, variables)?;
                self.output.push(Item::Table(table));
            }
            Command::Frequencies { variables, table, order, statistics } => {
                use mksigma_stats::{FrequenciesOrder as O, FrequenciesStatistic as S};
                use mksigma_syntax::{FreqOrder, FreqStatistic};
                let options = mksigma_stats::FrequenciesOptions {
                    variables: variables.clone(),
                    table: *table,
                    order: match order {
                        FreqOrder::AscendingValue => O::AscendingValue,
                        FreqOrder::DescendingValue => O::DescendingValue,
                        FreqOrder::AscendingFrequency => O::AscendingFrequency,
                        FreqOrder::DescendingFrequency => O::DescendingFrequency,
                    },
                    statistics: statistics
                        .iter()
                        .map(|s| match s {
                            FreqStatistic::Mean => S::Mean,
                            FreqStatistic::SeMean => S::SeMean,
                            FreqStatistic::Mode => S::Mode,
                            FreqStatistic::StdDev => S::StdDev,
                            FreqStatistic::Variance => S::Variance,
                            FreqStatistic::Range => S::Range,
                            FreqStatistic::Minimum => S::Minimum,
                            FreqStatistic::Maximum => S::Maximum,
                            FreqStatistic::Sum => S::Sum,
                        })
                        .collect(),
                };
                for item in mksigma_stats::frequencies(&self.dataset, &options)? {
                    self.output.push(item);
                }
            }
            Command::DisplayDictionary => {
                for table in dictionary::tables(&self.dataset) {
                    self.output.push(Item::Table(table));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_descriptives_on_the_sample() {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax("DESCRIPTIVES VARIABLES=age income score.").unwrap();
        let text = s.output.to_text();
        assert!(text.starts_with("Descriptive Statistics\n"), "{text}");
        assert!(text.contains("Valid N (listwise)  8"), "{text}");
    }

    #[test]
    fn displays_the_dictionary() {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax("DISPLAY DICTIONARY.").unwrap();
        let text = s.output.to_text();
        assert!(text.contains("income         2  Monthly income  Scale              F8.2"), "{text}");
    }

    #[test]
    fn dictionary_commands_change_the_variables() {
        use mksigma_core::{Format, FormatKind, Measure, MissingValues};
        let mut s = Session::with_sample().unwrap();
        s.run_syntax(
            "VARIABLE LABELS score 'Score (0–100)'.\n\
             VALUE LABELS score 0 'None' 100 'Perfect'.\n\
             MISSING VALUES age (LO THRU 0, 999) income (-1).\n\
             VARIABLE LEVEL score (ORDINAL).\n\
             FORMATS income (COMMA10.2).\n\
             RENAME VARIABLES (age = idade) (idade = idade_anos).",
        )
        .unwrap();
        let v = s.dataset.variables();
        assert_eq!(v[2].label.as_deref(), Some("Score (0–100)"));
        assert_eq!(v[2].value_label(&Value::Num(100.0)), Some("Perfect"));
        assert_eq!(v[2].measure, Measure::Ordinal);
        assert_eq!(v[0].missing, MissingValues::Range { low: f64::MIN, high: 0.0, discrete: Some(999.0) });
        assert_eq!(v[1].missing, MissingValues::Discrete(vec![Value::Num(-1.0)]));
        assert_eq!(v[1].format, Format::new(FormatKind::Comma, 10, 2));
        assert_eq!(v[0].name, "idade_anos");
        assert_eq!(s.journal.len(), 6);
        assert_eq!(s.journal[3], "VARIABLE LEVEL score (ORDINAL).");
    }

    #[test]
    fn a_failing_command_changes_nothing_and_is_not_journaled() {
        let mut s = Session::with_sample().unwrap();
        let err = s.run_syntax("VARIABLE LABELS age 'Idade'.\nVARIABLE LABELS income 'Renda' /nope 'X'.").unwrap_err();
        assert_eq!(err, EngineError::UnknownVariable("nope".into()));
        assert_eq!(s.dataset.variables()[0].label.as_deref(), Some("Idade"), "the first command ran");
        assert_eq!(s.dataset.variables()[1].label.as_deref(), Some("Monthly income"), "the failing one changed nothing");
        assert_eq!(s.journal, ["VARIABLE LABELS age 'Idade'."]);

        assert!(s.run_syntax("RENAME VARIABLES (age = x) (income = x).").is_err());
        assert_eq!(s.dataset.variables()[0].name, "age", "renames are all or nothing");
        assert!(s.run_syntax("VALUE LABELS age 'a' 'b'.").is_err(), "numeric values must be numbers");
        assert!(s.run_syntax("FORMATS age (A8).").is_err());
        assert!(s.run_syntax("MISSING VALUES age (5 THRU 1).").is_err());
    }

    #[test]
    fn cell_edits() {
        let mut s = Session::with_sample().unwrap();
        s.set_cell(0, 0, " 24.5 ").unwrap();
        s.set_cell(1, 0, ".").unwrap();
        assert_eq!(s.dataset.column(0).unwrap()[..2], [Value::Num(24.5), Value::SysMis]);
        assert!(matches!(s.set_cell(0, 0, "abc"), Err(EngineError::Invalid(_))));
        assert!(matches!(s.set_cell(99, 0, "1"), Err(EngineError::Dataset(_))));
        assert!(s.journal.is_empty());
    }

    #[test]
    fn reports_errors() {
        let mut s = Session::with_sample().unwrap();
        assert!(matches!(s.run_syntax("DESCRIPTIVES nope."), Err(EngineError::Stats(_))));
        assert!(matches!(s.run_syntax("CROSSTABS age."), Err(EngineError::Syntax(_))));
    }
}
