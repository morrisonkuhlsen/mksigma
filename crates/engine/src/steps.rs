//! Applied steps: every change to the data since the file was opened, in order, like Power
//! Query's. The current dataset is always the base dataset with the enabled steps applied, so a
//! step can be switched off, removed, moved or rewritten and the data replayed from the base.
use mksigma_i18n::tr;

/// What a step does.
#[derive(Debug, Clone, PartialEq)]
pub enum StepKind {
    /// A data command as written (`COMPUTE x = 1.`, `VARIABLE LABELS ...`).
    Syntax(String),
    /// A Data View edit: the value typed into a case of a variable (by name).
    Cell {
        case: usize,
        variable: String,
        input: String,
    },
    InsertCase(usize),
    DeleteCase(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppliedStep {
    pub kind: StepKind,
    pub enabled: bool,
    /// Why the step failed the last time the steps were replayed; it was skipped.
    pub error: Option<String>,
    /// For a step that names a case by its row (a cell edit, a case inserted or deleted): the
    /// steps before it that added or removed cases when it was made. Replayed after other such
    /// steps, its row would be another case, so it is skipped with an error instead.
    pub(crate) rows: Vec<StepKind>,
}

impl AppliedStep {
    pub(crate) fn new(kind: StepKind) -> Self {
        Self { kind, enabled: true, error: None, rows: Vec::new() }
    }

    /// A step made now, after `before`: a step that names a row remembers which steps placed
    /// the rows.
    pub(crate) fn after(kind: StepKind, before: &[AppliedStep]) -> Self {
        let mut step = Self::new(kind);
        if step.names_a_row() {
            step.rows = before.iter().filter(|s| s.enabled && s.error.is_none() && s.moves_rows()).map(|s| s.kind.clone()).collect();
        }
        step
    }

    /// Whether the step finds its case by row number.
    pub(crate) fn names_a_row(&self) -> bool {
        !matches!(self.kind, StepKind::Syntax(_))
    }

    /// Whether the step adds or removes cases, so the rows after it are numbered differently.
    pub(crate) fn moves_rows(&self) -> bool {
        match &self.kind {
            StepKind::InsertCase(_) | StepKind::DeleteCase(_) => true,
            StepKind::Cell { .. } => false,
            StepKind::Syntax(source) => {
                mksigma_syntax::parse(source).is_ok_and(|commands| commands.iter().any(|c| matches!(c, mksigma_syntax::Command::SelectIf(_))))
            }
        }
    }

    /// One line describing the step.
    pub fn label(&self) -> String {
        match &self.kind {
            StepKind::Syntax(source) => source.clone(),
            StepKind::Cell { case, variable, input } => {
                let shown = if input.trim().is_empty() { "(blank)" } else { input.as_str() };
                tr!("step-edit-case", case = case + 1, variable = variable.as_str(), value = shown)
            }
            StepKind::InsertCase(case) => tr!("step-insert-case", case = case + 1),
            StepKind::DeleteCase(case) => tr!("step-delete-case", case = case + 1),
        }
    }

    /// The step as syntax, if it is a command (Data View edits have no syntax).
    pub fn syntax(&self) -> Option<&str> {
        match &self.kind {
            StepKind::Syntax(source) => Some(source),
            _ => None,
        }
    }
}
