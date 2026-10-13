//! Undo and redo. Each change to the data is recorded as the smallest thing that can reverse it:
//! a dictionary change keeps the variables' metadata before and after (no data), a cell edit keeps
//! the old and new value, a deleted variable keeps its column. Undoing a syntax command also
//! takes it out of the journal, so the journal stays a replayable history of what is in effect.

use mksigma_core::{Dataset, DatasetError, Value, Variable};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Edit {
    Dictionary {
        before: Vec<Variable>,
        after: Vec<Variable>,
    },
    Cell {
        case: usize,
        var: usize,
        before: Value,
        after: Value,
    },
    InsertVariable {
        index: usize,
        var: Variable,
        column: Vec<Value>,
    },
    RemoveVariable {
        index: usize,
        var: Variable,
        column: Vec<Value>,
    },
    InsertCase {
        index: usize,
        values: Vec<Value>,
    },
    RemoveCase {
        index: usize,
        values: Vec<Value>,
    },
    /// A variable's values replaced (`COMPUTE`, `RECODE`).
    Column {
        index: usize,
        before: Vec<Value>,
        after: Vec<Value>,
    },
    /// Cases removed at once (`SELECT IF`), with their values.
    RemoveCases {
        indexes: Vec<usize>,
        rows: Vec<Vec<Value>>,
    },
}

impl Edit {
    fn redo(&self, ds: &mut Dataset) -> Result<(), DatasetError> {
        match self {
            Edit::Dictionary { after, .. } => ds.replace_variables(after.clone()),
            Edit::Cell { case, var, after, .. } => ds.set_value(*case, *var, after.clone()),
            Edit::InsertVariable { index, var, column } => ds.insert_variable(*index, var.clone(), column.clone()),
            Edit::RemoveVariable { index, .. } => ds.remove_variable(*index).map(drop),
            Edit::InsertCase { index, values } => ds.insert_case(*index, values.clone()),
            Edit::RemoveCase { index, .. } => ds.remove_case(*index).map(drop),
            Edit::Column { index, after, .. } => ds.replace_column(*index, after.clone()).map(drop),
            Edit::RemoveCases { indexes, .. } => ds.remove_cases(indexes).map(drop),
        }
    }

    fn undo(&self, ds: &mut Dataset) -> Result<(), DatasetError> {
        match self {
            Edit::Dictionary { before, .. } => ds.replace_variables(before.clone()),
            Edit::Cell { case, var, before, .. } => ds.set_value(*case, *var, before.clone()),
            Edit::InsertVariable { index, .. } => ds.remove_variable(*index).map(drop),
            Edit::RemoveVariable { index, var, column } => ds.insert_variable(*index, var.clone(), column.clone()),
            Edit::InsertCase { index, .. } => ds.remove_case(*index).map(drop),
            Edit::RemoveCase { index, values } => ds.insert_case(*index, values.clone()),
            Edit::Column { index, before, .. } => ds.replace_column(*index, before.clone()).map(drop),
            Edit::RemoveCases { indexes, rows } => ds.restore_cases(indexes, rows.clone()),
        }
    }
}

/// One undoable step: the edits it made, in order, and the journal line it added, if any.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Step {
    pub(crate) label: String,
    pub(crate) edits: Vec<Edit>,
    pub(crate) journal: Option<String>,
}

/// How many steps undo remembers.
const LIMIT: usize = 200;

#[derive(Debug, Default)]
pub(crate) struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
}

impl History {
    /// Records a step that just happened; a new step makes the redo list obsolete.
    pub(crate) fn push(&mut self, step: Step) {
        if step.edits.is_empty() {
            return;
        }
        self.redo.clear();
        self.undo.push(step);
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
    }

    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    pub(crate) fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|s| s.label.as_str())
    }

    pub(crate) fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|s| s.label.as_str())
    }

    /// Reverses the last step. The journal loses that step's line (its last occurrence).
    pub(crate) fn undo(&mut self, ds: &mut Dataset, journal: &mut Vec<String>) -> Result<bool, DatasetError> {
        let Some(step) = self.undo.pop() else { return Ok(false) };
        for edit in step.edits.iter().rev() {
            edit.undo(ds)?;
        }
        if let Some(line) = &step.journal
            && let Some(at) = journal.iter().rposition(|j| j == line)
        {
            journal.remove(at);
        }
        self.redo.push(step);
        Ok(true)
    }

    /// Applies the last undone step again, journal line included.
    pub(crate) fn redo(&mut self, ds: &mut Dataset, journal: &mut Vec<String>) -> Result<bool, DatasetError> {
        let Some(step) = self.redo.pop() else { return Ok(false) };
        for edit in &step.edits {
            edit.redo(ds)?;
        }
        journal.extend(step.journal.clone());
        self.undo.push(step);
        Ok(true)
    }
}
