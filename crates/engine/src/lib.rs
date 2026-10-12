//! MK Sigma session: the active dataset plus the output so far. Every user-visible action goes
//! through here (syntax, menus, CLI, later MCP), so all frontends behave the same.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mksigma_core::{Dataset, DatasetError, Value, Variable};
use mksigma_output::{Item, Output};
use mksigma_syntax::{Command, SyntaxError};
use thiserror::Error;

mod dictionary;

pub use mksigma_core as core;
pub use mksigma_output as output;

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
}

#[derive(Debug, Default)]
pub struct Session {
    pub dataset: Dataset,
    pub output: Output,
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
        Ok(Self { dataset: ds, output: Output::default() })
    }

    /// Replaces the active dataset with a system file's. Returns the reader's warnings.
    pub fn open_sav(&mut self, bytes: &[u8]) -> Result<Vec<String>, EngineError> {
        let file = mksigma_sav::read(bytes)?;
        self.dataset = file.dataset;
        Ok(file.warnings)
    }

    /// Runs a syntax program. Commands before a failing one keep their output.
    pub fn run_syntax(&mut self, src: &str) -> Result<(), EngineError> {
        for command in mksigma_syntax::parse(src)? {
            self.execute(&command)?;
        }
        Ok(())
    }

    pub fn execute(&mut self, command: &Command) -> Result<(), EngineError> {
        match command {
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
    fn reports_errors() {
        let mut s = Session::with_sample().unwrap();
        assert!(matches!(s.run_syntax("DESCRIPTIVES nope."), Err(EngineError::Stats(_))));
        assert!(matches!(s.run_syntax("CROSSTABS age."), Err(EngineError::Syntax(_))));
    }
}
