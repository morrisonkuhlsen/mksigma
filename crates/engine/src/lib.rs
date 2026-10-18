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
mod filter;
mod history;
mod lists;
mod steps;
mod transform;

use filter::{CrossFilter, Run};
pub use filter::{Selection, Slicer};
use history::{Edit, Entry, History};
use mksigma_output::Category;
pub use steps::{AppliedStep, StepKind};

pub use mksigma_core as core;
pub use mksigma_csv as csv;
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
    #[error(transparent)]
    Csv(#[from] mksigma_csv::CsvError),
    #[error("{0}")]
    File(String),
    #[error("variable {0} does not exist")]
    UnknownVariable(String),
    #[error("{0}")]
    Invalid(String),
}

/// Parses a syntax program without running it.
pub fn check_syntax(src: &str) -> Result<(), EngineError> {
    mksigma_syntax::parse(src).map(drop).map_err(EngineError::from)
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
    /// The analyses behind the Output, each with its items on all the cases.
    runs: Vec<Run>,
    /// The run each item of the Output came from, by item index.
    run_of: Vec<usize>,
    cross_filter: Option<CrossFilter>,
    /// One per variable at most, in the order they were set.
    slicers: Vec<Slicer>,
    /// The `GET FILE` or `GET DATA` command that reads the data, if one can.
    data_source: Option<String>,
    /// How many of the first steps `data_source` already holds (a file saved after them).
    steps_in_source: usize,
    /// The dashboard's tiles, in order: a run and the place of the item among the run's items,
    /// so a tile follows its item when filters run the analyses again.
    tiles: Vec<(usize, usize)>,
    /// The tiles that take a whole row.
    wide: Vec<(usize, usize)>,
    /// What loading data had to say (sizes, reader warnings), until `take_notes`.
    notes: Vec<String>,
    loaded_file: Option<String>,
}

/// Whether a command changes the data (and so is an applied step) rather than analysing it.
fn changes_data(command: &Command) -> bool {
    !matches!(
        command,
        Command::Descriptives { .. }
            | Command::Frequencies { .. }
            | Command::DisplayDictionary
            | Command::Crosstabs { .. }
            | Command::TTest { .. }
            | Command::Oneway { .. }
            | Command::Correlations { .. }
            | Command::Regression { .. }
            | Command::NparTests(_)
            | Command::Logistic { .. }
            | Command::Graph { .. }
            | Command::Examine { .. }
            | Command::Slice(_)
            | Command::Dashboard(_)
            | Command::OutputStyle(_)
            | Command::GetFile(_)
            | Command::GetData(_)
    )
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
        self.load(file.dataset);
        Ok(file.warnings)
    }

    /// Replaces the active dataset with delimited text's (a file with no path, such as one
    /// dropped on a web page; with a path, `GET DATA` records how it was read). Returns the
    /// reader's warnings.
    pub fn open_text(&mut self, bytes: &[u8], options: &mksigma_csv::Options) -> Result<Vec<String>, EngineError> {
        let file = mksigma_csv::read(bytes, options)?;
        self.load(file.dataset);
        Ok(file.warnings)
    }

    /// A new active dataset: the base of new applied steps, with nothing to undo.
    fn load(&mut self, dataset: Dataset) {
        // Syntax that read it sets this again after loading.
        self.data_source = None;
        self.steps_in_source = 0;
        self.base = dataset.clone();
        self.dataset = dataset;
        self.steps.clear();
        self.modified = false;
        self.history.clear();
        // The Output stays, but filters on the old data's cases mean nothing for the new.
        let filtered = self.cross_filter.take().is_some() | !std::mem::take(&mut self.slicers).is_empty();
        if filtered {
            self.show_runs();
        }
    }

    /// `GET DATA /TYPE=TXT`: reads the file into a new active dataset.
    fn get_data(&mut self, spec: &mksigma_syntax::GetData) -> Result<(), EngineError> {
        let bytes = std::fs::read(&spec.file).map_err(|e| EngineError::File(format!("{}: {e}", spec.file)))?;
        let mut variables = Vec::with_capacity(spec.variables.len());
        for (name, format) in &spec.variables {
            variables.push((name.clone(), format.parse().map_err(EngineError::Invalid)?));
        }
        let options = mksigma_csv::Options {
            delimiters: spec.delimiters.clone(),
            qualifier: spec.qualifier,
            read_names: spec.read_names.unwrap_or(spec.variables.is_empty()),
            first_case: spec.first_case,
            variables,
            encoding: spec.encoding.clone(),
        };
        let file = mksigma_csv::read(&bytes, &options)?;
        let ds = &file.dataset;
        let delimiter = if file.delimiter == '\t' { "tab".to_string() } else { format!("'{}'", file.delimiter) };
        self.notes.push(format!("{}: {} cases, {} variables (delimiter {delimiter})", spec.file, ds.n_cases(), ds.variables().len()));
        self.notes.extend(file.warnings.iter().map(|w| format!("{}: {w}", spec.file)));
        self.load(file.dataset);
        self.loaded_file = Some(spec.file.clone());
        Ok(())
    }

    /// `GET FILE`: reads a system file into a new active dataset.
    fn get_file(&mut self, path: &str) -> Result<(), EngineError> {
        let bytes = std::fs::read(path).map_err(|e| EngineError::File(format!("{path}: {e}")))?;
        let warnings = self.open_sav(&bytes)?;
        let ds = &self.dataset;
        self.notes.push(format!("{path}: {} cases, {} variables", ds.n_cases(), ds.variables().len()));
        self.notes.extend(warnings.iter().map(|w| format!("{path}: {w}")));
        self.loaded_file = Some(path.to_string());
        Ok(())
    }

    /// `DASHBOARD /TILES`: tiles by analysis and item, counted from 1.
    fn set_tiles(&mut self, tiles: &[(usize, usize, bool)]) -> Result<(), EngineError> {
        let mut out = Vec::with_capacity(tiles.len());
        let mut wide = Vec::new();
        for &(analysis, item, is_wide) in tiles {
            let (run, offset) = (analysis - 1, item - 1);
            let count = self.run_of.iter().filter(|r| **r == run).count();
            if offset >= count {
                return Err(EngineError::Invalid(format!("analysis {analysis} has no output item {item}")));
            }
            out.push((run, offset));
            if is_wide {
                wide.push((run, offset));
            }
        }
        self.tiles = out;
        self.wide = wide;
        Ok(())
    }

    /// The session as syntax that rebuilds its dashboard: where the data came from, the applied
    /// steps, every analysis with output, the slicers and the tiles. `None` when the data didn't
    /// come from a file through `GET FILE` or `GET DATA` (nothing could read it again).
    /// The data was written to the system file `path`: from now on it is where the data comes
    /// from, with the steps so far in it.
    pub fn saved_to(&mut self, path: &str) {
        self.data_source = Some(format!("GET FILE={}.", mksigma_syntax::quote(path)));
        self.steps_in_source = self.steps.len();
    }

    pub fn dashboard_syntax(&self) -> Option<String> {
        let source = self.data_source.as_ref()?;
        let mut out = String::from("* MK Sigma dashboard: run it to rebuild the analyses and the dashboard.\n");
        out.push_str(source);
        out.push('\n');
        let mut skipped = 0;
        for step in self.steps.iter().skip(self.steps_in_source).filter(|s| s.enabled) {
            match step.syntax() {
                Some(syntax) => {
                    out.push_str(syntax);
                    out.push('\n');
                }
                None => skipped += 1,
            }
        }
        if skipped > 0 {
            out.push_str(&format!("* {skipped} Data View edits are not syntax and were left out.\n"));
        }
        // Runs from a frame of the past (sources lost) can't be written; their numbers shift.
        let written: Vec<usize> = (0..self.runs.len()).filter(|r| self.runs.get(*r).is_some_and(|run| !run.source.is_empty())).collect();
        for r in &written {
            if let Some(run) = self.runs.get(*r) {
                out.push_str(&run.source);
                out.push('\n');
            }
        }
        for slicer in &self.slicers {
            out.push_str(&slicer.syntax());
            out.push('\n');
        }
        if let Some(style) = self.output.style.syntax() {
            out.push_str(&style);
            out.push('\n');
        }
        let tiles: Vec<String> = self
            .tiles
            .iter()
            .filter_map(|tile @ (run, offset)| {
                let wide = if self.wide.contains(tile) { " WIDE" } else { "" };
                written.iter().position(|r| r == run).map(|n| format!("({} {}{wide})", n + 1, offset + 1))
            })
            .collect();
        out.push_str(&format!("DASHBOARD /TILES={}.\n", tiles.join(" ")));
        Some(out)
    }

    /// The file `GET DATA` last read, if it ran since the last call.
    pub fn take_loaded_file(&mut self) -> Option<String> {
        self.loaded_file.take()
    }

    /// What loading data said since the last call, oldest first.
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
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
            // New data, not a step on the old; and the dashboard's layout.
            let special = match &command {
                Command::GetData(spec) => Some(self.get_data(spec)),
                Command::GetFile(path) => Some(self.get_file(path)),
                Command::Dashboard(tiles) => Some(self.set_tiles(tiles)),
                _ => None,
            };
            if let Some(result) = special {
                result?;
                if matches!(command, Command::GetData(_) | Command::GetFile(_)) {
                    self.data_source = Some(source.clone());
                }
                self.journal.push(source);
                continue;
            }
            let runs = self.runs.len();
            let edits = self.apply(&command)?;
            if self.runs.len() > runs
                && let Some(run) = self.runs.last_mut()
            {
                run.source = source.clone();
            }
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
        self.data_changed();
    }

    /// Filtered items follow the data; the unfiltered ones stay as they were produced.
    fn data_changed(&mut self) {
        if self.cross_filter.is_some() || !self.slicers.is_empty() {
            self.show_runs();
        }
    }

    /// The slicers, in the order they were set.
    pub fn slicers(&self) -> &[Slicer] {
        &self.slicers
    }

    /// Every filter on the Output in words (slicers, then the cross-filter), for a heading.
    pub fn filter_description(&self) -> Option<String> {
        let mut parts: Vec<String> = self.slicers.iter().map(|s| s.describe(&self.dataset)).collect();
        parts.extend(self.cross_filter().map(|c| c.label.clone()));
        (!parts.is_empty()).then(|| parts.join("; "))
    }

    /// A variable's valid values with their labels as shown and weighted counts, in ascending
    /// order, at most `max` of them: what a slicer offers to pick from.
    pub fn values_of(&self, variable: &str, max: usize) -> Vec<(Value, String, f64)> {
        let Some((i, var)) = self.dataset.variable(variable) else { return Vec::new() };
        let mut out: Vec<(Value, String, f64)> = Vec::new();
        for (case, v) in self.dataset.column(i).unwrap_or_default().iter().enumerate() {
            if var.is_missing(v) {
                continue;
            }
            let w = self.dataset.case_weight(case);
            match out.iter().position(|(value, _, _)| value == v) {
                Some(at) => {
                    if let Some(entry) = out.get_mut(at) {
                        entry.2 += w;
                    }
                }
                None if out.len() < max => out.push((v.clone(), var.value_label(v).map_or_else(|| var.format.render(v), str::to_string), w)),
                None => {}
            }
        }
        out.sort_by(|a, b| match (&a.0, &b.0) {
            (Value::Num(x), Value::Num(y)) => x.total_cmp(y),
            (Value::Str(x), Value::Str(y)) => x.cmp(y),
            _ => std::cmp::Ordering::Equal,
        });
        out
    }

    /// The smallest and largest valid value of a numeric variable.
    pub fn range_of(&self, variable: &str) -> Option<(f64, f64)> {
        let (i, var) = self.dataset.variable(variable)?;
        self.dataset.column(i)?.iter().filter(|v| !var.is_missing(v)).filter_map(Value::as_num).fold(None, |r, x| match r {
            None => Some((x, x)),
            Some((lo, hi)) => Some((f64::min(lo, x), f64::max(hi, x))),
        })
    }

    /// `OUTPUT STYLE`: changes what is written, keeps the rest.
    fn set_style(&mut self, spec: &mksigma_syntax::StyleSpec) {
        use mksigma_output::style::{Family, Palette};
        let style = &mut self.output.style;
        if let Some(font) = &spec.font {
            style.family = Family::ALL.into_iter().find(|f| f.name() == font).unwrap_or_default();
        }
        if let Some(palette) = &spec.palette {
            style.palette = Palette::ALL.into_iter().find(|p| p.name() == palette).unwrap_or_default();
        }
        if let Some(size) = spec.size {
            style.size = size;
        }
        if let Some(decimals) = spec.decimals {
            style.decimals = decimals;
        }
    }

    /// `SLICE`: sets, replaces or removes a variable's slicer, or removes them all.
    fn set_slicer(&mut self, spec: Option<&(String, mksigma_syntax::SliceSpec)>) -> Result<(), EngineError> {
        use mksigma_syntax::{Literal, SliceSpec};
        let Some((name, spec)) = spec else {
            self.slicers.clear();
            self.show_runs();
            return Ok(());
        };
        let (_, var) = self.dataset.variable(name).ok_or_else(|| EngineError::UnknownVariable(name.clone()))?;
        let variable = var.name.clone();
        let numeric = var.var_type == mksigma_core::VarType::Numeric;
        let selection = match spec {
            SliceSpec::Off => None,
            SliceSpec::Range { .. } if !numeric => return Err(EngineError::Invalid(format!("{variable} is a string variable: slice it by values"))),
            SliceSpec::Range { low, high } => Some(Selection::Range { low: *low, high: *high }),
            SliceSpec::Values(values) => Some(Selection::Values(
                values
                    .iter()
                    .map(|l| match (l, numeric) {
                        (Literal::Num(x), true) => Ok(Value::Num(*x)),
                        (Literal::Str(s), false) => Ok(Value::Str(s.clone())),
                        _ => Err(EngineError::Invalid(format!("{variable} needs {} values", if numeric { "numeric" } else { "quoted string" }))),
                    })
                    .collect::<Result<_, _>>()?,
            )),
        };
        match (self.slicers.iter().position(|s| s.variable == variable), selection) {
            (Some(at), Some(selection)) => {
                if let Some(slicer) = self.slicers.get_mut(at) {
                    slicer.selection = selection;
                }
            }
            (None, Some(selection)) => self.slicers.push(Slicer { variable, selection }),
            (Some(at), None) => {
                self.slicers.remove(at);
            }
            (None, None) => {}
        }
        self.show_runs();
        Ok(())
    }

    /// The category the Output is cross-filtered by, if any.
    pub fn cross_filter(&self) -> Option<&Category> {
        self.cross_filter.as_ref().map(|f| &f.category)
    }

    /// Whether Output item `item` came from the command whose item was selected: it shows all
    /// the cases, with the selection highlighted.
    pub fn is_filter_source(&self, item: usize) -> bool {
        self.cross_filter.as_ref().is_some_and(|f| self.run_of.get(item) == Some(&f.source))
    }

    /// Selects `category` of Output item `item` and filters the other items to its cases.
    /// Selecting the selected category again clears the filter.
    pub fn select_category(&mut self, item: usize, category: Category) -> Result<(), EngineError> {
        let source = *self.run_of.get(item).ok_or_else(|| EngineError::Invalid(format!("there is no output item {}", item + 1)))?;
        let filter = CrossFilter { category, source };
        self.cross_filter = if self.cross_filter.as_ref() == Some(&filter) { None } else { Some(filter) };
        self.show_runs();
        Ok(())
    }

    pub fn clear_cross_filter(&mut self) {
        if self.cross_filter.take().is_some() {
            self.show_runs();
        }
    }

    /// Empties the Output, and with it the filter and the dashboard.
    pub fn clear_output(&mut self) {
        self.runs.clear();
        self.tiles.clear();
        self.wide.clear();
        self.cross_filter = None;
        self.show_runs();
    }

    /// The run of Output item `item` and the item's place among the run's items.
    fn locate(&self, item: usize) -> Option<(usize, usize)> {
        let run = *self.run_of.get(item)?;
        let first = self.run_of.iter().position(|r| *r == run)?;
        Some((run, item - first))
    }

    /// Pins Output item `item` (a table or a chart) to the dashboard, or unpins it. Returns
    /// whether it is pinned now.
    pub fn toggle_pin(&mut self, item: usize) -> Result<bool, EngineError> {
        if !matches!(self.output.items.get(item), Some(Item::Table(_) | Item::Chart(_))) {
            return Err(EngineError::Invalid(format!("output item {} is not a table or a chart", item + 1)));
        }
        let tile = self.locate(item).ok_or_else(|| EngineError::Invalid(format!("there is no output item {}", item + 1)))?;
        match self.tiles.iter().position(|t| *t == tile) {
            Some(at) => {
                self.tiles.remove(at);
                self.wide.retain(|t| *t != tile);
                Ok(false)
            }
            None => {
                self.tiles.push(tile);
                Ok(true)
            }
        }
    }

    pub fn is_pinned(&self, item: usize) -> bool {
        self.locate(item).is_some_and(|tile| self.tiles.contains(&tile))
    }

    /// The Output items on the dashboard, in its order. A tile whose run has fewer items now
    /// (a filter left it a warning) shows nothing.
    pub fn dashboard(&self) -> Vec<usize> {
        self.tiles
            .iter()
            .filter_map(|(run, offset)| {
                let first = self.run_of.iter().position(|r| r == run)?;
                let item = first + offset;
                (self.run_of.get(item) == Some(run)).then_some(item)
            })
            .collect()
    }

    /// Whether Output item `item` is a wide tile: a whole row of the dashboard.
    pub fn is_wide(&self, item: usize) -> bool {
        self.locate(item).is_some_and(|tile| self.wide.contains(&tile))
    }

    /// Makes the tile of Output item `item` take a whole row, or half of one.
    pub fn set_wide(&mut self, item: usize, wide: bool) -> Result<(), EngineError> {
        let tile = self
            .locate(item)
            .filter(|t| self.tiles.contains(t))
            .ok_or_else(|| EngineError::Invalid(format!("output item {} is not on the dashboard", item + 1)))?;
        self.wide.retain(|t| *t != tile);
        if wide {
            self.wide.push(tile);
        }
        Ok(())
    }

    /// Moves tile `tile` one place earlier or later.
    pub fn move_tile(&mut self, tile: usize, earlier: bool) -> Result<(), EngineError> {
        let other = if earlier { tile.checked_sub(1) } else { tile.checked_add(1).filter(|t| *t < self.tiles.len()) };
        match other.filter(|_| tile < self.tiles.len()) {
            Some(other) => {
                self.tiles.swap(tile, other);
                Ok(())
            }
            None => Err(EngineError::Invalid(format!("tile {} can't move {}", tile + 1, if earlier { "earlier" } else { "later" }))),
        }
    }

    /// The dashboard as an Output of its own, for export, with which of its items are wide.
    pub fn dashboard_output(&self) -> (Output, Vec<bool>) {
        let items = self.dashboard();
        let wide = items.iter().map(|i| self.is_wide(*i)).collect();
        (Output { items: items.into_iter().filter_map(|i| self.output.items.get(i).cloned()).collect(), style: self.output.style }, wide)
    }

    fn add_run(&mut self, command: Command, items: Vec<Item>) {
        self.runs.push(Run { command, source: String::new(), items });
        self.show_runs();
    }

    /// Rebuilds the Output from the runs: each on all the cases, or, under a filter, run again
    /// on the filtered cases (except the one the selection came from).
    fn show_runs(&mut self) {
        // Slicers first, for every run; then the cross-filter, for all but the selected one.
        let sliced = (!self.slicers.is_empty()).then(|| filter::slice(&self.dataset, &self.slicers));
        let crossed = self.cross_filter.as_ref().map(|f| {
            let ds = match &sliced {
                Some(Ok(ds)) => filter::subset(ds, &f.category),
                Some(Err(e)) => Err(EngineError::Invalid(e.to_string())),
                None => filter::subset(&self.dataset, &f.category),
            };
            (f.source, ds)
        });
        let run_on = |ds: &Result<Dataset, EngineError>, command: &Command| match ds {
            Ok(ds) => analyse(ds, command).unwrap_or_else(|e| vec![Item::Warning(format!("Filtered: {e}"))]),
            Err(e) => vec![Item::Warning(format!("Filtered: {e}"))],
        };
        let mut items = Vec::new();
        let mut run_of = Vec::new();
        for (i, run) in self.runs.iter().enumerate() {
            let shown = match (&crossed, &sliced) {
                (Some((source, ds)), _) if *source != i => run_on(ds, &run.command),
                (_, Some(ds)) => run_on(ds, &run.command),
                _ => run.items.clone(),
            };
            run_of.extend(std::iter::repeat_n(i, shown.len()));
            items.extend(shown);
        }
        self.output.items = items;
        self.run_of = run_of;
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
        if matches!(command, Command::GetData(_)) {
            return Err(EngineError::Invalid("GET DATA starts new data; run it from the syntax editor".into()));
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
        // The saved file holds the old steps: no syntax can rebuild the new list from it.
        if self.steps_in_source > 0 {
            self.data_source = None;
        }
        let before = std::mem::replace(&mut self.steps, after);
        self.replay();
        self.history.push(Entry::Steps { label, before, after: self.steps.clone() });
        self.modified = true;
        self.data_changed();
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
        self.data_changed();
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
        self.data_changed();
        Ok(true)
    }

    /// Runs a command and returns the edits it made to the data (none for analyses).
    fn apply(&mut self, written: &Command) -> Result<Vec<Edit>, EngineError> {
        let expanded = lists::expand(&self.dataset, written)?;
        let command = &expanded;
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
            Command::Slice(spec) => self.set_slicer(spec.as_ref())?,
            Command::OutputStyle(spec) => self.set_style(spec),
            _ => {
                // The run keeps the command as written, so `ALL` means the variables of the day.
                let items = analyse(&self.dataset, written)?;
                self.add_run(written.clone(), items);
            }
        }
        Ok(Vec::new())
    }
}

/// The output of an analysis command on `ds`; nothing for commands that change the data.
fn analyse(ds: &Dataset, command: &Command) -> Result<Vec<Item>, EngineError> {
    let command = &lists::expand(ds, command)?;
    let mut items = Vec::new();
    match command {
        Command::Descriptives { variables } => {
            let table = mksigma_stats::descriptives(ds, variables)?;
            items.push(Item::Table(table));
        }
        Command::Frequencies { variables, table, order, statistics, histogram, barchart } => {
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
            for item in mksigma_stats::frequencies(ds, &options)? {
                items.push(item);
            }
            for variable in variables {
                if *barchart {
                    items.push(mksigma_stats::bar(ds, &mksigma_stats::BarValue::Count, variable)?);
                }
                if let Some(normal) = histogram {
                    items.push(mksigma_stats::histogram(ds, variable, *normal)?);
                }
            }
        }
        Command::Examine { variables, by, descriptives, boxplot, stemleaf } => {
            let options =
                mksigma_stats::ExamineOptions { variables: variables.clone(), by: by.clone(), descriptives: *descriptives, boxplot: *boxplot };
            items.extend(mksigma_stats::examine(ds, &options)?);
            if *stemleaf {
                items.push(Item::Text("Stem-and-leaf plots are not drawn yet.".into()));
            }
        }
        Command::Graph { chart, title } => {
            use mksigma_syntax::{BarStat, GraphSpec};
            let mut item = match chart {
                GraphSpec::Histogram { variable, normal } => mksigma_stats::histogram(ds, variable, *normal)?,
                GraphSpec::Bar { value, by } => {
                    let value = match value {
                        BarStat::Count => mksigma_stats::BarValue::Count,
                        BarStat::Mean(y) => mksigma_stats::BarValue::Mean(y.clone()),
                    };
                    mksigma_stats::bar(ds, &value, by)?
                }
                GraphSpec::Scatter { x, y } => mksigma_stats::scatter(ds, x, y)?,
            };
            if let (Some(title), Item::Chart(chart)) = (title, &mut item) {
                chart.set_title(title);
            }
            items.push(item);
        }
        Command::DisplayDictionary => {
            for table in dictionary::tables(ds) {
                items.push(Item::Table(table));
            }
        }
        Command::Crosstabs { rows, columns, cells, chisq } => {
            let options = mksigma_stats::CrosstabsOptions {
                rows: rows.clone(),
                columns: columns.clone(),
                cells: mksigma_stats::CrosstabsCells {
                    count: cells.count,
                    expected: cells.expected,
                    row: cells.row,
                    column: cells.column,
                    total: cells.total,
                },
                chisq: *chisq,
            };
            items.extend(mksigma_stats::crosstabs(ds, &options)?);
        }
        Command::TTest { test, ci } => {
            use mksigma_syntax::{Literal, TTestSpec};
            let value = |l: &Literal| match l {
                Literal::Num(x) => Value::Num(*x),
                Literal::Str(s) => Value::Str(s.clone()),
            };
            let test = match test {
                TTestSpec::OneSample { variables, test_value } => {
                    mksigma_stats::TTest::OneSample { variables: variables.clone(), test_value: *test_value }
                }
                TTestSpec::Independent { variables, group, first, second } => mksigma_stats::TTest::Independent {
                    variables: variables.clone(),
                    group: group.clone(),
                    first: value(first),
                    second: value(second),
                },
                TTestSpec::Paired { pairs } => mksigma_stats::TTest::Paired { pairs: pairs.clone() },
            };
            items.extend(mksigma_stats::t_test(ds, &test, *ci)?);
        }
        Command::Correlations { variables, with, one_tailed, flag, listwise } => {
            let options = mksigma_stats::CorrelationsOptions {
                variables: variables.clone(),
                with: with.clone(),
                one_tailed: *one_tailed,
                flag: *flag,
                listwise: *listwise,
            };
            items.extend(mksigma_stats::correlations(ds, &options)?);
        }
        Command::NparTests(specs) => {
            use mksigma_stats::NparTest;
            use mksigma_syntax::NparSpec;
            let tests: Vec<NparTest> = specs
                .iter()
                .map(|s| match s {
                    NparSpec::MannWhitney { variables, group, first, second } => {
                        NparTest::MannWhitney { variables: variables.clone(), group: group.clone(), first: *first, second: *second }
                    }
                    NparSpec::Wilcoxon { pairs } => NparTest::Wilcoxon { pairs: pairs.clone() },
                    NparSpec::KruskalWallis { variables, group, low, high } => {
                        NparTest::KruskalWallis { variables: variables.clone(), group: group.clone(), low: *low, high: *high }
                    }
                })
                .collect();
            items.extend(mksigma_stats::npar_tests(ds, &tests)?);
        }
        Command::Logistic { dependent, covariates, ci, iterations, cut } => {
            let options = mksigma_stats::LogisticOptions {
                dependent: dependent.clone(),
                covariates: covariates.clone(),
                ci: *ci,
                iterations: *iterations,
                cut: *cut,
            };
            items.extend(mksigma_stats::logistic(ds, &options)?);
        }
        Command::Regression { dependent, predictors, ci } => {
            let options = mksigma_stats::RegressionOptions { dependent: dependent.clone(), predictors: predictors.clone(), ci: *ci };
            items.extend(mksigma_stats::regression(ds, &options)?);
        }
        Command::Oneway { dependents, factor, descriptives, homogeneity, posthoc, alpha } => {
            let options = mksigma_stats::OnewayOptions {
                dependents: dependents.clone(),
                factor: factor.clone(),
                descriptives: *descriptives,
                homogeneity: *homogeneity,
                posthoc: posthoc
                    .iter()
                    .map(|p| match p {
                        mksigma_syntax::PostHocTest::Tukey => mksigma_stats::PostHoc::Tukey,
                        mksigma_syntax::PostHocTest::Bonferroni => mksigma_stats::PostHoc::Bonferroni,
                    })
                    .collect(),
                alpha: *alpha,
            };
            items.extend(mksigma_stats::oneway(ds, &options)?);
        }
        _ => {}
    }
    Ok(items)
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
    fn charts_go_to_the_output() {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax(
            "GRAPH /HISTOGRAM(NORMAL)=income /TITLE='Renda'.\nGRAPH /BAR(SIMPLE)=MEAN(income) BY score.\n\
             GRAPH /SCATTERPLOT(BIVAR)=age WITH income.\nFREQUENCIES age /FORMAT=NOTABLE /HISTOGRAM /BARCHART.",
        )
        .unwrap();
        let charts: Vec<&str> = s.output.items.iter().filter_map(|i| if let Item::Chart(c) = i { Some(c.title()) } else { None }).collect();
        assert_eq!(charts, ["Renda", "Mean Monthly income by Test score", "Monthly income by Age (years)", "Age (years)", "Age (years)"]);
        assert!(s.steps().is_empty(), "charts are not steps");
        assert!(s.output.to_html("Output").contains("<svg"));
    }

    #[test]
    fn analyses_run_and_are_not_steps() {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax(
            "RECODE age (LO THRU 30 = 1) (ELSE = 2) INTO grupo.\n\
             CROSSTABS grupo BY grupo /STATISTICS=CHISQ.\n\
             T-TEST GROUPS=grupo(1 2) /VARIABLES=income.\n\
             T-TEST /TESTVAL=70 /VARIABLES=score.\n\
             T-TEST PAIRS=age WITH score (PAIRED).\n\
             ONEWAY income BY grupo /STATISTICS=DESCRIPTIVES HOMOGENEITY.",
        )
        .unwrap();
        assert_eq!(s.steps().len(), 1, "only the RECODE changed the data");
        s.run_syntax(
            "CORRELATIONS age income score.\nREGRESSION /DEPENDENT score /METHOD=ENTER age income.\n\
             ONEWAY score BY grupo /POSTHOC=TUKEY BONFERRONI ALPHA(0.05).\n\
             NPAR TESTS /M-W=income BY grupo(1 2) /WILCOXON=age WITH score (PAIRED) /K-W=income BY grupo(1 2).\n\
             COMPUTE alto = score >= 70.\nLOGISTIC REGRESSION VARIABLES alto /METHOD=ENTER income.",
        )
        .unwrap();
        assert_eq!(s.steps().len(), 2, "RECODE and COMPUTE");
        let text = s.output.to_text();
        for title in [
            "Crosstabulation",
            "Chi-Square Tests",
            "Independent Samples Test",
            "One-Sample Test",
            "Paired Samples Test",
            "ANOVA",
            "Correlations",
            "Model Summary",
            "Coefficients",
            "Multiple Comparisons",
            "Mann-Whitney U",
            "Kruskal-Wallis H",
            "Variables in the Equation",
        ] {
            assert!(text.contains(title), "{title} missing from:\n{text}");
        }
    }

    /// N and mean of score in the first "Descriptive Statistics" table of the Output.
    fn score_n_and_mean(s: &Session) -> (i64, f64) {
        let table = s
            .output
            .items
            .iter()
            .find_map(|i| match i {
                Item::Table(t) if t.title == "Descriptive Statistics" => Some(t),
                _ => None,
            })
            .unwrap();
        match &table.rows[0].1[..] {
            [mksigma_output::Cell::Int(n), _, _, mksigma_output::Cell::Num { value, .. }, _] => (*n, *value),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn selecting_a_category_filters_the_other_items() {
        use mksigma_output::{Cases, Chart};
        let mut s = Session::with_sample().unwrap();
        // Ages 23 35 41 29 52 47 31 26: Jovem (≤ 30) are cases 1, 4 and 8, scores 61, 80, 77.
        s.run_syntax(
            "RECODE age (LO THRU 30 = 1) (ELSE = 2) INTO grupo.\nVALUE LABELS grupo 1 'Jovem' 2 'Adulto'.\n\
             GRAPH /BAR(SIMPLE)=COUNT BY grupo.\nDESCRIPTIVES score.",
        )
        .unwrap();
        let Item::Chart(chart) = &s.output.items[0] else { panic!() };
        let jovem = chart.category(0).unwrap().clone();
        assert_eq!((jovem.label.as_str(), &jovem.cases), ("grupo = Jovem", &Cases::Num(1.0)));
        assert_eq!(score_n_and_mean(&s), (8, 72.375));

        s.select_category(0, jovem.clone()).unwrap();
        assert_eq!(s.cross_filter(), Some(&jovem));
        let (n, mean) = score_n_and_mean(&s);
        assert_eq!(n, 3);
        assert!((mean - 218.0 / 3.0).abs() < 1e-12);
        assert!(s.is_filter_source(0) && !s.is_filter_source(1));
        let Item::Chart(Chart::Bar { bars, .. }) = &s.output.items[0] else { panic!() };
        assert_eq!(bars.len(), 2, "the selected chart keeps every bar");
        assert!(s.steps().len() == 2 && s.dataset.n_cases() == 8, "a filter is a view, not a step");

        // New analyses and changes to the data follow the filter.
        s.run_syntax("FREQUENCIES grupo.").unwrap();
        let frequency = s.output.items.iter().rev().find_map(|i| if let Item::Table(t) = i { Some(t) } else { None }).unwrap();
        assert_eq!(frequency.rows[0].1[1], mksigma_output::Cell::Int(3), "only Jovem is left");
        s.set_cell(0, 2, "70").unwrap();
        assert!((score_n_and_mean(&s).1 - 227.0 / 3.0).abs() < 1e-12);

        // Selecting it again clears the filter; the unfiltered items are as they were produced.
        s.select_category(0, jovem).unwrap();
        assert_eq!(s.cross_filter(), None);
        assert_eq!(score_n_and_mean(&s), (8, 72.375));
        assert!(s.select_category(99, chart_category(&s)).is_err());
        s.clear_output();
        assert!(s.output.items.is_empty() && s.cross_filter().is_none());
    }

    fn chart_category(s: &Session) -> Category {
        s.output.items.iter().find_map(|i| if let Item::Chart(c) = i { c.category(1).cloned() } else { None }).unwrap()
    }

    #[test]
    fn tables_histograms_and_boxes_have_categories() {
        use mksigma_output::{Cases, Chart};
        let mut s = Session::with_sample().unwrap();
        s.run_syntax(
            "RECODE age (LO THRU 30 = 1) (ELSE = 2) INTO grupo.\nFREQUENCIES grupo /HISTOGRAM.\n\
             EXAMINE score BY grupo /PLOT=BOXPLOT.",
        )
        .unwrap();
        let frequency = s.output.items.iter().find_map(|i| if let Item::Table(t) = i { (t.title == "grupo").then_some(t) } else { None }).unwrap();
        assert_eq!(frequency.category(1).map(|c| &c.cases), Some(&Cases::Num(2.0)));
        assert!(frequency.category(2).is_none(), "the Total row");
        let histogram = s.output.items.iter().find_map(|i| if let Item::Chart(c @ Chart::Histogram { .. }) = i { Some(c) } else { None }).unwrap();
        let Chart::Histogram { bins, categories, .. } = histogram else { panic!() };
        assert_eq!(bins.len(), categories.len());
        assert!(matches!(categories.last().unwrap().cases, Cases::Range { closed: true, .. }));
        let boxplot = s.output.items.iter().find_map(|i| if let Item::Chart(c @ Chart::Boxplot { .. }) = i { Some(c) } else { None }).unwrap();
        assert_eq!(boxplot.category(1).map(|c| c.label.as_str()), Some("grupo = 2.00"));

        // The first bin of the histogram holds grupo 1: the three youngest.
        let item = s.output.items.iter().position(|i| matches!(i, Item::Chart(Chart::Histogram { .. }))).unwrap();
        let first_bin = histogram.category(0).unwrap().clone();
        s.select_category(item, first_bin).unwrap();
        let frequency = s.output.items.iter().find_map(|i| if let Item::Table(t) = i { (t.title == "grupo").then_some(t) } else { None }).unwrap();
        assert_eq!(frequency.rows.len(), 3, "the same command as the histogram: all the cases");
        let summary = s
            .output
            .items
            .iter()
            .find_map(|i| if let Item::Table(t) = i { (t.title == "Case Processing Summary").then_some(t) } else { None })
            .unwrap();
        assert_eq!(summary.rows[0].1[0], mksigma_output::Cell::Int(3), "EXAMINE ran again on the three in the first bin");
    }

    #[test]
    fn get_data_reads_text_into_new_data() {
        let path = std::env::temp_dir().join(format!("mksigma-engine-{}.csv", std::process::id()));
        // Semicolons and decimal commas, as spreadsheets in Brazil save CSV.
        std::fs::write(&path, "nome;nota;turma\nAna;7,5;1\nBia;9;2\nCaio;6,5;1\nDani;8;2\n").unwrap();
        let mut s = Session::with_sample().unwrap();
        s.run_syntax("COMPUTE x = 1.").unwrap();
        let file = path.display().to_string();
        s.run_syntax(&format!("GET DATA /TYPE=TXT /FILE={}.\nDESCRIPTIVES nota.", quote(&file))).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(s.dataset.n_cases(), 4);
        assert_eq!(nums(&s, "nota"), [7.5, 9.0, 6.5, 8.0].map(Some));
        assert!(s.steps().is_empty() && s.undo_label().is_none() && !s.modified, "new data: no steps, nothing to undo");
        assert_eq!(s.take_notes(), [format!("{file}: 4 cases, 3 variables (delimiter ';')")]);
        assert!(s.take_notes().is_empty());
        assert!(s.journal.iter().any(|j| j.starts_with("GET DATA")));
        // Mean of 7.5, 9, 6.5 and 8.
        assert!(s.output.to_text().contains("7.75"), "{}", s.output.to_text());
        s.run_syntax("COMPUTE dobro = nota * 2.").unwrap();
        assert_eq!(s.steps().len(), 1, "steps start from the text's data");
        s.set_step_enabled(0, false).unwrap();
        assert_eq!(s.dataset.n_cases(), 4, "replay starts from it too");
        assert!(s.replace_step(0, &format!("GET DATA /TYPE=TXT /FILE={}.", quote(&file))).is_err());

        assert!(matches!(s.run_syntax("GET DATA /TYPE=TXT /FILE='/nonexistent/x.csv'."), Err(EngineError::File(_))));
        assert_eq!(s.dataset.n_cases(), 4, "a file that can't be read changes nothing");
    }

    #[test]
    fn all_and_to_in_variable_lists() {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax("DESCRIPTIVES ALL.\nFORMATS age TO score (F6.1).").unwrap();
        assert!(s.output.to_text().contains("Test score"), "ALL takes every variable");
        assert!(s.dataset.variables().iter().all(|v| v.format.to_string() == "F6.1"));
        // ALL is expanded each time: a new variable joins a filtered rerun.
        s.run_syntax("RECODE age (LO THRU 30 = 1) (ELSE = 2) INTO grupo.\nFREQUENCIES grupo /BARCHART.").unwrap();
        let before = s.output.to_text().matches("Descriptive Statistics").count();
        assert_eq!(before, 1);
        let bar = s.output.items.iter().position(|i| matches!(i, Item::Chart(_))).unwrap();
        let Item::Chart(chart) = &s.output.items[bar] else { panic!() };
        s.select_category(bar, chart.category(0).unwrap().clone()).unwrap();
        let rows = s
            .output
            .items
            .iter()
            .find_map(|i| if let Item::Table(t) = i { (t.title == "Descriptive Statistics").then_some(t.rows.len()) } else { None });
        assert_eq!(rows, Some(5), "age, income, score, grupo and Valid N");
        assert!(s.run_syntax("DESCRIPTIVES score TO age.").is_err());
    }

    #[test]
    fn slicers_filter_every_item_and_combine() {
        let mut s = Session::with_sample().unwrap();
        // Ages 23 35 41 29 52 47 31 26; incomes 1800 4200 5100 2600 6900 3900 3100 2200;
        // scores 61 74 69 80 58 72 88 77. Jovem (age ≤ 30): cases 1, 4, 8.
        s.run_syntax("RECODE age (LO THRU 30 = 1) (ELSE = 2) INTO grupo.\nVALUE LABELS grupo 1 'Jovem' 2 'Adulto'.\nDESCRIPTIVES score.").unwrap();
        assert_eq!(s.values_of("grupo", 50), [(Value::Num(1.0), "Jovem".to_string(), 3.0), (Value::Num(2.0), "Adulto".to_string(), 5.0)]);
        assert_eq!(s.range_of("income"), Some((1800.0, 6900.0)));

        s.run_syntax("SLICE grupo (1).").unwrap();
        let (n, mean) = score_n_and_mean(&s);
        assert_eq!(n, 3);
        assert!((mean - 218.0 / 3.0).abs() < 1e-12);
        // Two slicers keep the cases both keep: Jovem with income 2000 to 4000 are cases 4 and 8.
        s.run_syntax("SLICE income (2000 THRU 4000).").unwrap();
        assert_eq!(score_n_and_mean(&s), (2, 78.5));
        assert_eq!(s.filter_description().as_deref(), Some("grupo: Jovem; income 2000.00 to 4000.00"));
        assert_eq!(s.slicers()[1].syntax(), "SLICE income (2000 THRU 4000).");
        assert_eq!(s.slicers()[0].syntax(), "SLICE grupo (1).");
        assert!(s.steps().len() == 2 && s.dataset.n_cases() == 8, "slicers are a view, not steps");
        assert!(s.journal.iter().any(|j| j == "SLICE grupo (1)."), "but they are syntax, in the History");

        // Slicing a variable again replaces its slicer; OFF removes it.
        s.run_syntax("SLICE grupo (2).\nSLICE income OFF.").unwrap();
        assert_eq!(score_n_and_mean(&s).0, 5);
        // New analyses run sliced too.
        s.run_syntax("FREQUENCIES grupo.").unwrap();
        let frequency = s.output.items.iter().rev().find_map(|i| if let Item::Table(t) = i { Some(t) } else { None }).unwrap();
        assert_eq!(frequency.rows[0].1[1], mksigma_output::Cell::Int(5), "only Adulto");
        s.run_syntax("SLICE OFF.").unwrap();
        assert_eq!(score_n_and_mean(&s), (8, 72.375), "the originals again");

        assert!(matches!(s.run_syntax("SLICE nope (1)."), Err(EngineError::UnknownVariable(_))));
        assert!(s.run_syntax("SLICE grupo ('a').").is_err(), "a numeric variable takes numbers");
        assert!(s.slicers().is_empty());
    }

    #[test]
    fn a_slicer_and_a_cross_filter_together() {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax("RECODE age (LO THRU 30 = 1) (ELSE = 2) INTO grupo.\nGRAPH /BAR(SIMPLE)=COUNT BY grupo.\nDESCRIPTIVES score.").unwrap();
        s.run_syntax("SLICE income (LO THRU 4000).").unwrap();
        // Incomes up to 4000: cases 1, 4, 6, 7, 8; of those, grupo 1 are cases 1, 4 and 8.
        let Item::Chart(mksigma_output::Chart::Bar { bars, .. }) = &s.output.items[0] else { panic!() };
        assert_eq!(bars.iter().map(|b| b.1).collect::<Vec<_>>(), [3.0, 2.0], "the selected chart is sliced too");
        let Item::Chart(chart) = &s.output.items[0] else { panic!() };
        s.select_category(0, chart.category(1).unwrap().clone()).unwrap();
        // grupo 2 within the slice: cases 6 and 7, scores 72 and 88.
        assert_eq!(score_n_and_mean(&s), (2, 80.0));
        assert_eq!(s.filter_description().as_deref(), Some("income up to 4000.00; grupo = 2.00"));
    }

    #[test]
    fn pinned_items_make_a_dashboard_that_follows_the_filters() {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax("RECODE age (LO THRU 30 = 1) (ELSE = 2) INTO grupo.\nFREQUENCIES grupo /FORMAT=NOTABLE /BARCHART.\nDESCRIPTIVES score.")
            .unwrap();
        // FREQUENCIES gave a Statistics table and a chart; DESCRIPTIVES a table.
        let chart = s.output.items.iter().position(|i| matches!(i, Item::Chart(_))).unwrap();
        let descriptives = s.output.items.len() - 1;
        assert!(s.toggle_pin(descriptives).unwrap());
        assert!(s.toggle_pin(chart).unwrap());
        assert_eq!(s.dashboard(), [descriptives, chart], "in the order pinned");
        s.move_tile(1, true).unwrap();
        assert_eq!(s.dashboard(), [chart, descriptives]);
        assert!(s.move_tile(0, true).is_err());

        // Tiles follow their items when filters run the analyses again.
        s.run_syntax("SLICE grupo (1).").unwrap();
        assert_eq!(score_n_and_mean(&s).0, 3);
        let (board, wide) = s.dashboard_output();
        assert_eq!(board.items.len(), 2);
        assert_eq!(wide, [false, false]);
        assert!(board.to_text().contains("Descriptive Statistics"));
        assert!(s.is_pinned(descriptives) && !s.is_pinned(0));

        assert!(!s.toggle_pin(chart).unwrap(), "pinning again unpins");
        assert_eq!(s.dashboard(), [descriptives]);
        s.clear_output();
        assert!(s.dashboard().is_empty());
    }

    #[test]
    fn a_dashboard_saved_as_syntax_rebuilds_itself() {
        let path = std::env::temp_dir().join(format!("mksigma-dash-{}.csv", std::process::id()));
        std::fs::write(&path, "regiao,vendas,meta\nNorte,10,12\nSul,12,10\nNorte,8,9\nSul,15,11\nLeste,9,9\n").unwrap();
        let file = quote(&path.display().to_string());
        let mut s = Session::new();
        assert_eq!(s.dashboard_syntax(), None, "no data from a file yet");
        s.run_syntax(&format!(
            "GET DATA /TYPE=TXT /FILE={file}.\nCOMPUTE folga = vendas - meta.\nGRAPH /BAR(SIMPLE)=MEAN(vendas) BY regiao.\n\
             DESCRIPTIVES vendas meta folga.\nSLICE regiao ('Norte' 'Sul')."
        ))
        .unwrap();
        s.toggle_pin(1).unwrap();
        s.toggle_pin(0).unwrap();
        s.set_wide(1, true).unwrap();
        assert!(s.set_wide(2, true).is_err(), "not on the dashboard");
        let syntax = s.dashboard_syntax().unwrap();
        assert!(syntax.starts_with("* MK Sigma dashboard"));
        assert!(syntax.contains("COMPUTE folga = vendas - meta.\nGRAPH"), "{syntax}");
        assert!(syntax.ends_with("SLICE regiao ('Norte' 'Sul').\nDASHBOARD /TILES=(2 1 WIDE) (1 1).\n"), "{syntax}");

        let mut again = Session::new();
        again.run_syntax(&syntax).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(again.dashboard(), [1, 0]);
        assert!(again.is_wide(1) && !again.is_wide(0), "widths come back too");
        assert_eq!(again.dashboard_output().0.to_text(), s.dashboard_output().0.to_text(), "the same dashboard, filtered the same");
        assert!(again.run_syntax("DASHBOARD /TILES=(9 1).").is_err());
    }

    #[test]
    fn get_file_reads_a_system_file() {
        let path = std::env::temp_dir().join(format!("mksigma-get-file-{}.sav", std::process::id()));
        let mut s = Session::with_sample().unwrap();
        let (bytes, _) = s.save_sav(mksigma_sav::timestamp(0)).unwrap();
        std::fs::write(&path, bytes).unwrap();
        let mut t = Session::new();
        t.run_syntax(&format!("GET FILE={}.\nDESCRIPTIVES age.", quote(&path.display().to_string()))).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(t.dataset.n_cases(), 8);
        assert!(t.take_notes()[0].ends_with("8 cases, 3 variables"));
        assert!(t.dashboard_syntax().is_some_and(|d| d.contains("GET FILE=")));
        assert!(matches!(t.run_syntax("GET FILE='/nonexistent.sav'."), Err(EngineError::File(_))));
        // A file saved after steps holds them: the dashboard reads it and adds only later steps.
        t.run_syntax("COMPUTE x = age + 1.").unwrap();
        t.saved_to("/dados/com_x.sav");
        t.run_syntax("COMPUTE y = x * 2.").unwrap();
        let syntax = t.dashboard_syntax().unwrap();
        assert!(syntax.contains("GET FILE='/dados/com_x.sav'.\nCOMPUTE y = x * 2.\n") && !syntax.contains("COMPUTE x"), "{syntax}");
        t.set_step_enabled(0, false).unwrap();
        assert_eq!(t.dashboard_syntax(), None, "the saved file has the step that is now off");
    }

    #[test]
    fn output_style_changes_every_rendering_and_is_saved() {
        use mksigma_output::style::{Family, Palette};
        let mut s = Session::with_sample().unwrap();
        s.run_syntax("DESCRIPTIVES age.\nOUTPUT STYLE /FONT=SERIF /PALETTE=GRAY /DECIMALS=4.").unwrap();
        let style = s.output.style;
        assert_eq!((style.family, style.palette, style.decimals, style.size), (Family::Serif, Palette::Gray, Some(4), 10));
        assert!(s.output.to_html("x").contains("35.5000"), "four decimals everywhere");
        assert!(s.output.to_latex("x").contains("35.5000") && s.output.to_latex("x").contains("rmdefault"));
        s.run_syntax("OUTPUT STYLE /SIZE=12.").unwrap();
        assert_eq!(s.output.style.family, Family::Serif, "what isn't written stays");
        assert!(s.steps().is_empty() && s.journal.iter().any(|j| j.starts_with("OUTPUT STYLE")));
        s.clear_output();
        assert_eq!(s.output.style.size, 12, "clearing the Output keeps its style");
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
