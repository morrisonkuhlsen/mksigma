//! MK Sigma session: the active dataset plus the output so far. Every user-visible action goes
//! through here (syntax, menus, CLI, later MCP), so all frontends behave the same.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mksigma_core::{Dataset, DatasetError, Value, Variable};
use mksigma_output::{Item, Output};
use mksigma_syntax::{Command, SyntaxError};
use thiserror::Error;

mod dictionary;
mod edit;
mod expr;
mod history;
mod steps;
mod transform;

use history::{Edit, Entry, History};
pub use steps::{AppliedStep, StepKind};

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
    /// Every command that ran successfully, as written, analyses included.
    pub journal: Vec<String>,
    /// Whether the data or dictionary changed since the file was opened or saved.
    pub modified: bool,
    /// The dataset as opened; with the applied steps replayed on it, it gives `dataset`.
    base: Dataset,
    steps: Vec<AppliedStep>,
    history: History,
}

/// Whether a command changes the data (and so is an applied step) rather than analysing it.
fn changes_data(command: &Command) -> bool {
    !matches!(command, Command::Descriptives { .. } | Command::Frequencies { .. } | Command::DisplayDictionary)
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
        Ok(Self { base: ds.clone(), dataset: ds, ..Self::default() })
    }

    /// Replaces the active dataset with a system file's. Returns the reader's warnings.
    pub fn open_sav(&mut self, bytes: &[u8]) -> Result<Vec<String>, EngineError> {
        let file = mksigma_sav::read(bytes)?;
        self.base = file.dataset.clone();
        self.dataset = file.dataset;
        self.steps.clear();
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
    /// go into the journal. Each command that changes the data is an applied step and one undo
    /// action.
    pub fn run_syntax(&mut self, src: &str) -> Result<(), EngineError> {
        for (command, source) in mksigma_syntax::parse_with_source(src)? {
            let edits = self.apply(&command)?;
            self.journal.push(source.clone());
            if changes_data(&command) {
                let step = AppliedStep::new(StepKind::Syntax(source.clone()));
                self.record(source.clone(), edits, Some(source), step);
            }
        }
        Ok(())
    }

    fn record(&mut self, label: String, edits: Vec<Edit>, journal: Option<String>, step: AppliedStep) {
        self.modified = true;
        self.steps.push(step.clone());
        self.history.push(Entry::Edits { label, edits, journal, step: Some(step) });
    }

    /// Edits one cell from typed text (Data View); the row after the last case adds a case. Not
    /// a syntax command, so not journaled, but an applied step and undoable.
    pub fn set_cell(&mut self, case: usize, variable: usize, input: &str) -> Result<(), EngineError> {
        let name = self.dataset.variables().get(variable).map(|v| v.name.clone()).ok_or(DatasetError::NoVariable(variable))?;
        let edits = edit::set_cell(&mut self.dataset, case, variable, input)?;
        let step = AppliedStep::new(StepKind::Cell { case, variable: name, input: input.to_string() });
        self.record("cell edit".to_string(), edits, None, step);
        Ok(())
    }

    /// Inserts a blank case at `index` (Data View).
    pub fn insert_case(&mut self, index: usize) -> Result<(), EngineError> {
        let values = self.dataset.blank_case();
        self.dataset.insert_case(index, values.clone())?;
        self.record("insert case".to_string(), vec![Edit::InsertCase { index, values }], None, AppliedStep::new(StepKind::InsertCase(index)));
        Ok(())
    }

    /// Deletes the case at `index` (Data View).
    pub fn delete_case(&mut self, index: usize) -> Result<(), EngineError> {
        let values = self.dataset.remove_case(index)?;
        self.record("delete case".to_string(), vec![Edit::RemoveCase { index, values }], None, AppliedStep::new(StepKind::DeleteCase(index)));
        Ok(())
    }

    /// Every change to the data since the file was opened, in order.
    pub fn steps(&self) -> &[AppliedStep] {
        &self.steps
    }

    /// Switches a step on or off and replays the steps.
    pub fn set_step_enabled(&mut self, index: usize, enabled: bool) -> Result<(), EngineError> {
        let mut steps = self.steps.clone();
        let step = steps.get_mut(index).ok_or_else(|| EngineError::Invalid(format!("there is no step {}", index + 1)))?;
        step.enabled = enabled;
        let label = format!("{} step {}", if enabled { "enable" } else { "disable" }, index + 1);
        self.change_steps(label, steps);
        Ok(())
    }

    /// Removes a step and replays the rest.
    pub fn remove_step(&mut self, index: usize) -> Result<(), EngineError> {
        let mut steps = self.steps.clone();
        if index >= steps.len() {
            return Err(EngineError::Invalid(format!("there is no step {}", index + 1)));
        }
        steps.remove(index);
        self.change_steps(format!("remove step {}", index + 1), steps);
        Ok(())
    }

    /// Moves a step one place earlier (`up`) or later and replays the steps.
    pub fn move_step(&mut self, index: usize, up: bool) -> Result<(), EngineError> {
        let other = if up { index.checked_sub(1) } else { index.checked_add(1).filter(|i| *i < self.steps.len()) };
        let Some(other) = other.filter(|_| index < self.steps.len()) else {
            return Err(EngineError::Invalid(format!("step {} can't move {}", index + 1, if up { "up" } else { "down" })));
        };
        let mut steps = self.steps.clone();
        steps.swap(index, other);
        self.change_steps(format!("move step {}", index + 1), steps);
        Ok(())
    }

    /// Rewrites a syntax step: `source` must be exactly one data command.
    pub fn replace_step(&mut self, index: usize, source: &str) -> Result<(), EngineError> {
        let parsed = mksigma_syntax::parse_with_source(source)?;
        let [(command, source)] = parsed.as_slice() else {
            return Err(EngineError::Invalid("a step is one command".into()));
        };
        if !changes_data(command) {
            return Err(EngineError::Invalid("a step changes the data; run analyses from the syntax editor".into()));
        }
        let mut steps = self.steps.clone();
        let step = steps.get_mut(index).ok_or_else(|| EngineError::Invalid(format!("there is no step {}", index + 1)))?;
        if step.syntax().is_none() {
            return Err(EngineError::Invalid("only command steps can be rewritten".into()));
        }
        step.kind = StepKind::Syntax(source.clone());
        self.change_steps(format!("edit step {}", index + 1), steps);
        Ok(())
    }

    /// Sets the step list, replays it, and records the change for undo.
    fn change_steps(&mut self, label: String, after: Vec<AppliedStep>) {
        let before = std::mem::replace(&mut self.steps, after);
        self.replay();
        self.history.push(Entry::Steps { label, before, after: self.steps.clone() });
        self.modified = true;
    }

    /// Rebuilds the dataset from the base with the enabled steps. A step that fails is skipped
    /// and keeps its error; analyses don't run, so the Output stays as it is.
    fn replay(&mut self) {
        self.dataset = self.base.clone();
        let mut steps = std::mem::take(&mut self.steps);
        for step in &mut steps {
            step.error = None;
            if !step.enabled {
                continue;
            }
            let result = match &step.kind {
                StepKind::Syntax(source) => mksigma_syntax::parse(source)
                    .map_err(EngineError::from)
                    .and_then(|commands| commands.iter().try_for_each(|c| self.apply(c).map(drop))),
                StepKind::Cell { case, variable, input } => match self.dataset.variable(variable) {
                    Some((i, _)) => edit::set_cell(&mut self.dataset, *case, i, input).map(drop),
                    None => Err(EngineError::UnknownVariable(variable.clone())),
                },
                StepKind::InsertCase(index) => {
                    let values = self.dataset.blank_case();
                    self.dataset.insert_case(*index, values).map_err(EngineError::from)
                }
                StepKind::DeleteCase(index) => self.dataset.remove_case(*index).map(drop).map_err(EngineError::from),
            };
            step.error = result.err().map(|e| e.to_string());
        }
        self.steps = steps;
    }

    /// What Undo would reverse (a command as written, "cell edit", "disable step 2"...).
    pub fn undo_label(&self) -> Option<&str> {
        self.history.undo_label()
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.history.redo_label()
    }

    /// Reverses the last action; `false` when there is nothing to undo.
    pub fn undo(&mut self) -> Result<bool, EngineError> {
        let Some(entry) = self.history.take_undo() else { return Ok(false) };
        match &entry {
            Entry::Edits { edits, journal, step, .. } => {
                for edit in edits.iter().rev() {
                    edit.undo(&mut self.dataset)?;
                }
                if let Some(line) = journal
                    && let Some(at) = self.journal.iter().rposition(|j| j == line)
                {
                    self.journal.remove(at);
                }
                if step.is_some() {
                    self.steps.pop();
                }
            }
            Entry::Steps { before, .. } => {
                self.steps = before.clone();
                self.replay();
            }
        }
        self.history.undone(entry);
        self.modified = true;
        Ok(true)
    }

    /// Applies the last undone action again; `false` when there is nothing to redo.
    pub fn redo(&mut self) -> Result<bool, EngineError> {
        let Some(entry) = self.history.take_redo() else { return Ok(false) };
        match &entry {
            Entry::Edits { edits, journal, step, .. } => {
                for edit in edits {
                    edit.redo(&mut self.dataset)?;
                }
                self.journal.extend(journal.clone());
                self.steps.extend(step.clone());
            }
            Entry::Steps { after, .. } => {
                self.steps = after.clone();
                self.replay();
            }
        }
        self.history.redone(entry);
        self.modified = true;
        Ok(true)
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
            Command::Compute { target, expr } => return transform::compute(&mut self.dataset, target, expr, None),
            Command::If { cond, target, expr } => return transform::compute(&mut self.dataset, target, expr, Some(cond)),
            Command::Recode(groups) => return transform::recode(&mut self.dataset, groups),
            Command::SelectIf(cond) => return transform::select_if(&mut self.dataset, cond),
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

    fn nums(s: &Session, var: &str) -> Vec<Option<f64>> {
        let (i, _) = s.dataset.variable(var).unwrap();
        s.dataset.column(i).unwrap().iter().map(Value::as_num).collect()
    }

    #[test]
    fn compute_and_if() {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax("COMPUTE renda_k = income / 1000.\nCOMPUTE age = age + 1.\nIF (score >= 75) aprovado = 1.\nIF (score < 75) aprovado = 0.")
            .unwrap();
        assert_eq!(nums(&s, "renda_k")[..3], [Some(1.8), Some(4.2), Some(5.1)]);
        assert_eq!(nums(&s, "age")[..2], [Some(24.0), Some(36.0)], "a variable can use its own old value");
        // Scores 61 74 69 80 58 72 88 77.
        assert_eq!(nums(&s, "aprovado"), [Some(0.0), Some(0.0), Some(0.0), Some(1.0), Some(0.0), Some(0.0), Some(1.0), Some(1.0)]);
        s.run_syntax("STRING faixa (A6).\nIF (age >= 40) faixa = 'senior'.\nIF (age < 40) faixa = CONCAT('jr-', STRING(age, F2.0)).").unwrap();
        let (f, _) = s.dataset.variable("faixa").unwrap();
        assert_eq!(s.dataset.column(f).unwrap()[..3], [Value::Str("jr-24".into()), Value::Str("jr-36".into()), Value::Str("senior".into())]);
        assert!(s.run_syntax("COMPUTE nome = 'x'.").is_err(), "new string variables need STRING first");
        assert!(s.run_syntax("COMPUTE faixa = 1.").is_err(), "a number into a string variable");
        assert!(s.run_syntax("IF ('x') a = 1.").is_err());
    }

    #[test]
    fn recode_in_place_and_into() {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax("MISSING VALUES score (88).\nRECODE age (LO THRU 29 = 1) (30 THRU 44 = 2) (45 THRU HI = 3) INTO faixa.").unwrap();
        // Ages 23 35 41 29 52 47 31 26.
        assert_eq!(nums(&s, "faixa"), [1.0, 2.0, 2.0, 1.0, 3.0, 3.0, 2.0, 1.0].map(Some));
        assert_eq!(nums(&s, "age")[0], Some(23.0), "INTO leaves the source alone");
        s.run_syntax("RECODE score (MISSING = 0) (80 THRU HI = 3) (ELSE = COPY).").unwrap();
        assert_eq!(nums(&s, "score"), [61.0, 74.0, 69.0, 3.0, 58.0, 72.0, 0.0, 77.0].map(Some), "88 is user-missing: MISSING wins, it comes first");
        s.run_syntax("RECODE income (2200 = SYSMIS).").unwrap();
        assert_eq!(nums(&s, "income")[7], None);
        assert_eq!(nums(&s, "income")[0], Some(1800.0), "unmatched values stay in place");
        s.run_syntax("STRING uf (A2).\nRECODE faixa (1 = 'SP') (ELSE = 'RJ') INTO uf.").unwrap();
        let (u, _) = s.dataset.variable("uf").unwrap();
        assert_eq!(s.dataset.column(u).unwrap()[..2], [Value::Str("SP".into()), Value::Str("RJ".into())]);
        assert!(s.run_syntax("RECODE uf ('SP' = 1).").is_err(), "in place, a string stays a string");
        assert!(s.run_syntax("RECODE age ('a' = 1).").is_err(), "a numeric variable takes numbers on the left");
        assert!(s.run_syntax("RECODE age (1 = 'x') INTO novo.").is_err(), "a new text target needs STRING first");
    }

    #[test]
    fn select_if_and_undoing_transformations() {
        let mut s = Session::with_sample().unwrap();
        let original: Vec<Vec<Value>> = (0..3).map(|i| s.dataset.column(i).unwrap().to_vec()).collect();
        s.run_syntax("COMPUTE dobro = age * 2.\nRECODE score (LO THRU 69 = 0) (ELSE = 1).\nSELECT IF (age < 40 AND income > 2000).").unwrap();
        // Kept: 35/4200, 29/2600, 31/3100, 26/2200.
        assert_eq!(nums(&s, "age"), [35.0, 29.0, 31.0, 26.0].map(Some));
        assert_eq!(nums(&s, "dobro"), [70.0, 58.0, 62.0, 52.0].map(Some));
        assert_eq!(s.undo_label(), Some("SELECT IF (age < 40 AND income > 2000)."));
        while s.undo().unwrap() {}
        assert_eq!(s.dataset.variables().len(), 3);
        for (i, column) in original.iter().enumerate() {
            assert_eq!(s.dataset.column(i).unwrap(), column.as_slice());
        }
        while s.redo().unwrap() {}
        assert_eq!(s.dataset.n_cases(), 4);
        s.run_syntax("SELECT IF (age > 0).").unwrap();
        assert_eq!(s.undo_label(), Some("SELECT IF (age > 0)."), "a step even when it keeps every case: earlier steps may change");
    }

    fn snapshot(s: &Session) -> (Vec<mksigma_core::Variable>, Vec<Vec<Value>>) {
        let vars = s.dataset.variables().to_vec();
        let columns = (0..vars.len()).map(|i| s.dataset.column(i).unwrap().to_vec()).collect();
        (vars, columns)
    }

    fn labels(s: &Session) -> Vec<String> {
        s.steps().iter().map(AppliedStep::label).collect()
    }

    #[test]
    fn every_change_to_the_data_is_a_step() {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax("COMPUTE dobro = age * 2.\nDESCRIPTIVES age.\nVARIABLE LABELS dobro 'Dobro'.").unwrap();
        s.set_cell(0, 0, "30").unwrap();
        s.insert_case(1).unwrap();
        s.delete_case(2).unwrap();
        assert_eq!(
            labels(&s),
            ["COMPUTE dobro = age * 2.", "VARIABLE LABELS dobro 'Dobro'.", "Edit case 1, age = 30", "Insert case 2", "Delete case 3"],
            "analyses are not steps"
        );
        s.undo().unwrap();
        assert_eq!(s.steps().len(), 4, "undo takes the step back");
    }

    #[test]
    fn replaying_the_steps_gives_the_same_data() {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax(
            "NUMERIC peso (F3.0).\nSTRING nome (A8).\nCOMPUTE peso = RND(income / 100).\n\
             RECODE age (LO THRU 30 = 1) (ELSE = 2) INTO grupo.\nVALUE LABELS grupo 1 'Jovem' 2 'Adulto'.\n\
             SELECT IF (score > 60).\nRENAME VARIABLES (score = nota).",
        )
        .unwrap();
        s.set_cell(0, 4, "Ana").unwrap();
        s.insert_case(2).unwrap();
        s.set_cell(2, 0, "99").unwrap();
        s.delete_case(5).unwrap();
        let incremental = snapshot(&s);
        // Switching the first step off and on replays everything from the base.
        s.set_step_enabled(0, false).unwrap();
        s.set_step_enabled(0, true).unwrap();
        assert_eq!(snapshot(&s), incremental);
        assert!(s.steps().iter().all(|st| st.error.is_none()));
    }

    #[test]
    fn disable_remove_move_and_rewrite_steps() {
        let mut s = Session::with_sample().unwrap();
        // Bonuses 6.1 7.4 6.9 8.0 5.8 7.2 8.8 7.7: five over 7, three over 7.5.
        s.run_syntax("NUMERIC bonus.\nCOMPUTE bonus = score / 10.\nSELECT IF (bonus > 7).").unwrap();
        assert_eq!(s.dataset.n_cases(), 5);

        s.set_step_enabled(2, false).unwrap();
        assert_eq!(s.dataset.n_cases(), 8, "the filter is off");
        s.set_step_enabled(2, true).unwrap();
        assert_eq!(s.dataset.n_cases(), 5);

        s.replace_step(2, "SELECT IF (bonus > 7.5).").unwrap();
        assert_eq!(s.dataset.n_cases(), 3);
        assert!(s.replace_step(2, "DESCRIPTIVES age.").is_err(), "analyses aren't steps");
        assert!(s.replace_step(2, "SELECT IF (.").is_err());
        assert_eq!(s.dataset.n_cases(), 3, "a rejected rewrite changes nothing");

        // Computing before the variable exists: that step fails and is skipped.
        s.move_step(1, true).unwrap();
        assert_eq!(labels(&s)[0], "COMPUTE bonus = score / 10.");
        assert!(s.steps()[0].error.is_some() || s.steps()[1].error.is_some());
        s.undo().unwrap();
        assert!(s.steps().iter().all(|st| st.error.is_none()), "undo puts the order back");
        assert_eq!(s.dataset.n_cases(), 3);

        // Without NUMERIC, COMPUTE creates the variable itself: nothing fails.
        s.remove_step(0).unwrap();
        assert!(s.steps().iter().all(|st| st.error.is_none()));
        assert_eq!(s.dataset.n_cases(), 3);
        // Without COMPUTE, bonus stays system-missing and the filter keeps nothing.
        s.remove_step(0).unwrap();
        assert_eq!(s.steps()[0].error.as_deref(), Some("variable bonus does not exist"));
        assert_eq!(s.dataset.n_cases(), 8, "the failing filter was skipped");
        s.undo().unwrap();
        s.undo().unwrap();
        assert!(s.move_step(0, true).is_err());
        assert!(s.move_step(2, false).is_err());
        assert!(s.remove_step(9).is_err());
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
