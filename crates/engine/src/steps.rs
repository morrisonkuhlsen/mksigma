//! Applied steps: every change to the data since the file was opened, in order, like Power
//! Query's. The current dataset is always the base dataset with the enabled steps applied, so a
//! step can be switched off, removed, moved or rewritten and the data replayed from the base.

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
}

impl AppliedStep {
    pub(crate) fn new(kind: StepKind) -> Self {
        Self { kind, enabled: true, error: None }
    }

    /// One line describing the step.
    pub fn label(&self) -> String {
        match &self.kind {
            StepKind::Syntax(source) => source.clone(),
            StepKind::Cell { case, variable, input } => {
                let shown = if input.trim().is_empty() { "(blank)" } else { input.as_str() };
                format!("Edit case {}, {variable} = {shown}", case + 1)
            }
            StepKind::InsertCase(case) => format!("Insert case {}", case + 1),
            StepKind::DeleteCase(case) => format!("Delete case {}", case + 1),
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
