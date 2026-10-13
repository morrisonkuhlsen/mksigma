//! MK Sigma session: the active dataset plus the output so far. Every user-visible action goes
//! through here (syntax, menus, CLI, later MCP), so all frontends behave the same.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mksigma_core::{Dataset, DatasetError, Value, Variable};
use mksigma_output::{Item, Output};
use mksigma_syntax::{Command, SyntaxError};
use thiserror::Error;

mod dictionary;
mod edit;
mod history;

use history::{Edit, History, Step};

pub use mksigma_core as core;
pub use mksigma_output as output;
pub use mksigma_sav::timestamp;
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
    /// Whether the data or dictionary changed since the file was opened or saved.
    pub modified: bool,
    history: History,
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
        self.modified = false;
        self.history.clear();
        Ok(file.warnings)
    }

    /// The active dataset as a bytecode-compressed system file, and anything that couldn't be
    /// written. `created` is the header's date and time (`mksigma_sav::timestamp`).
    pub fn save_sav(&mut self, created: (String, String)) -> Result<(Vec<u8>, Vec<String>), EngineError> {
        let options = mksigma_sav::WriteOptions { compression: mksigma_sav::Compression::Bytecode, created };
        let written = mksigma_sav::write(&self.dataset, &options)?;
        self.modified = false;
        Ok(written)
    }

    /// Runs a syntax program. Commands before a failing one keep their effect and output, and
    /// go into the journal. Each command that changes the data is one undo step.
    pub fn run_syntax(&mut self, src: &str) -> Result<(), EngineError> {
        for (command, source) in mksigma_syntax::parse_with_source(src)? {
            let edits = self.apply(&command)?;
            self.journal.push(source.clone());
            if !edits.is_empty() {
                self.record(source.clone(), edits, Some(source));
            }
        }
        Ok(())
    }

    /// Runs one parsed command (not journaled: it has no source text).
    pub fn execute(&mut self, command: &Command) -> Result<(), EngineError> {
        let edits = self.apply(command)?;
        self.record("command".to_string(), edits, None);
        Ok(())
    }

    fn record(&mut self, label: String, edits: Vec<Edit>, journal: Option<String>) {
        if !edits.is_empty() {
            self.modified = true;
            self.history.push(Step { label, edits, journal });
        }
    }

    /// Edits one cell from typed text (Data View); the row after the last case adds a case. Not
    /// a syntax command, so not journaled, but undoable.
    pub fn set_cell(&mut self, case: usize, variable: usize, input: &str) -> Result<(), EngineError> {
        let edits = edit::set_cell(&mut self.dataset, case, variable, input)?;
        self.record("cell edit".to_string(), edits, None);
        Ok(())
    }

    /// Inserts a blank case at `index` (Data View).
    pub fn insert_case(&mut self, index: usize) -> Result<(), EngineError> {
        let values = self.dataset.blank_case();
        self.dataset.insert_case(index, values.clone())?;
        self.record("insert case".to_string(), vec![Edit::InsertCase { index, values }], None);
        Ok(())
    }

    /// Deletes the case at `index` (Data View).
    pub fn delete_case(&mut self, index: usize) -> Result<(), EngineError> {
        let values = self.dataset.remove_case(index)?;
        self.record("delete case".to_string(), vec![Edit::RemoveCase { index, values }], None);
        Ok(())
    }

    /// What Undo would reverse (a command as written, or "cell edit", "delete case"...).
    pub fn undo_label(&self) -> Option<&str> {
        self.history.undo_label()
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.history.redo_label()
    }

    /// Reverses the last change; `false` when there is nothing to undo.
    pub fn undo(&mut self) -> Result<bool, EngineError> {
        let done = self.history.undo(&mut self.dataset, &mut self.journal)?;
        self.modified |= done;
        Ok(done)
    }

    /// Applies the last undone change again; `false` when there is nothing to redo.
    pub fn redo(&mut self) -> Result<bool, EngineError> {
        let done = self.history.redo(&mut self.dataset, &mut self.journal)?;
        self.modified |= done;
        Ok(done)
    }

    /// Runs a command and returns the edits it made to the data (none for analyses).
    fn apply(&mut self, command: &Command) -> Result<Vec<Edit>, EngineError> {
        match command {
            Command::VariableLabels(_)
            | Command::ValueLabels(_)
            | Command::MissingValues(_)
            | Command::VariableLevel(_)
            | Command::Formats(_)
            | Command::RenameVariables(_) => {
                let before = self.dataset.variables().to_vec();
                edit::apply(&mut self.dataset, command)?;
                return Ok(vec![Edit::Dictionary { before, after: self.dataset.variables().to_vec() }]);
            }
            Command::Numeric(_) | Command::String(_) => return edit::new_variables(&mut self.dataset, command),
            Command::DeleteVariables(names) => return edit::delete_variables(&mut self.dataset, names),
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
        Ok(Vec::new())
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
    fn save_and_reopen() {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax("VALUE LABELS age 23 'Youngest'.").unwrap();
        assert!(s.modified);
        let (bytes, warnings) = s.save_sav(mksigma_sav::timestamp(0)).unwrap();
        assert!(warnings.is_empty() && !s.modified);
        let mut t = Session::new();
        t.open_sav(&bytes).unwrap();
        assert_eq!(t.dataset.variables(), s.dataset.variables());
        assert_eq!(t.dataset.column(1), s.dataset.column(1));
        s.run_syntax("DESCRIPTIVES age.").unwrap();
        assert!(!s.modified, "analysis doesn't modify the data");
    }

    #[test]
    fn undo_and_redo_every_kind_of_change() {
        let mut s = Session::with_sample().unwrap();
        let original_vars = s.dataset.variables().to_vec();
        let original_data: Vec<Vec<Value>> = (0..3).map(|i| s.dataset.column(i).unwrap().to_vec()).collect();
        s.run_syntax("VARIABLE LABELS age 'Idade'.\nNUMERIC peso (F3.0).\nSTRING nome (A10).\nDELETE VARIABLES income.").unwrap();
        s.set_cell(0, 2, "2").unwrap();
        s.set_cell(8, 3, "Ana").unwrap();
        s.insert_case(0).unwrap();
        s.delete_case(3).unwrap();
        s.run_syntax("DESCRIPTIVES age.").unwrap();
        assert_eq!(s.dataset.n_cases(), 9);
        assert_eq!(s.dataset.variables().iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), ["age", "score", "peso", "nome"]);
        assert_eq!(s.undo_label(), Some("delete case"), "the analysis is not an undo step");

        let mut undone = 0;
        while s.undo().unwrap() {
            undone += 1;
        }
        assert_eq!(undone, 8);
        assert_eq!(s.dataset.variables(), original_vars.as_slice());
        for (i, column) in original_data.iter().enumerate() {
            assert_eq!(s.dataset.column(i).unwrap(), column.as_slice());
        }
        assert_eq!(s.journal, ["DESCRIPTIVES age."], "undone commands leave the journal");

        while s.redo().unwrap() {}
        assert_eq!(s.dataset.n_cases(), 9);
        assert_eq!(s.dataset.column(3).unwrap()[8], Value::Str("Ana".into()));
        assert_eq!(s.dataset.variables()[0].label.as_deref(), Some("Idade"));
        assert_eq!(s.journal.len(), 5);
    }

    #[test]
    fn a_new_change_clears_redo_and_opening_clears_history() {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax("NUMERIC a.").unwrap();
        s.undo().unwrap();
        assert_eq!(s.redo_label(), Some("NUMERIC a."));
        s.run_syntax("NUMERIC b.").unwrap();
        assert_eq!(s.redo_label(), None);
        let (bytes, _) = s.save_sav(mksigma_sav::timestamp(0)).unwrap();
        s.open_sav(&bytes).unwrap();
        assert_eq!(s.undo_label(), None);
    }

    #[test]
    fn new_variables_are_checked_before_any_is_added() {
        let mut s = Session::with_sample().unwrap();
        assert!(s.run_syntax("NUMERIC x y x.").is_err(), "a name twice");
        assert!(s.run_syntax("NUMERIC z AGE.").is_err(), "an existing name");
        assert!(s.run_syntax("STRING t (F8.2).").is_err());
        assert!(s.run_syntax("NUMERIC n (A8).").is_err());
        assert_eq!(s.dataset.variables().len(), 3);
        s.run_syntax("STRING t (A3).").unwrap();
        assert_eq!(s.dataset.column(3).unwrap()[0], Value::Str(String::new()));
        assert!(s.run_syntax("DELETE VARIABLES age nope.").is_err());
        assert_eq!(s.dataset.variables().len(), 4);
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
