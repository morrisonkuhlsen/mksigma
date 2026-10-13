//! Undo and redo. A change to the data is recorded as the smallest thing that can reverse it: a
//! dictionary change keeps the variables' metadata before and after (no data), a cell edit keeps
//! the old and new value, a deleted variable keeps its column. Changing the applied steps
//! themselves (disable, reorder, edit, remove) keeps the step lists before and after; undoing it
//! replays the steps from the base dataset.

use mksigma_core::{Dataset, DatasetError, Value, Variable};

use crate::steps::AppliedStep;

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
    pub(crate) fn redo(&self, ds: &mut Dataset) -> Result<(), DatasetError> {
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

    pub(crate) fn undo(&self, ds: &mut Dataset) -> Result<(), DatasetError> {
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

/// One undoable action.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Entry {
    /// A command or edit applied incrementally: its edits, the journal line and the applied step
    /// it added.
    Edits { label: String, edits: Vec<Edit>, journal: Option<String>, step: Option<AppliedStep> },
    /// The applied steps rearranged; the data follows by replaying them.
    Steps { label: String, before: Vec<AppliedStep>, after: Vec<AppliedStep> },
}

impl Entry {
    fn label(&self) -> &str {
        match self {
            Entry::Edits { label, .. } | Entry::Steps { label, .. } => label,
        }
    }
}

/// How many actions undo remembers.
const LIMIT: usize = 200;

#[derive(Debug, Default)]
pub(crate) struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
}

impl History {
    /// Records an action that just happened; a new action makes the redo list obsolete.
    pub(crate) fn push(&mut self, entry: Entry) {
        self.redo.clear();
        self.undo.push(entry);
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
    }

    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    pub(crate) fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(Entry::label)
    }

    pub(crate) fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(Entry::label)
    }

    pub(crate) fn take_undo(&mut self) -> Option<Entry> {
        self.undo.pop()
    }

    pub(crate) fn take_redo(&mut self) -> Option<Entry> {
        self.redo.pop()
    }

    /// Files an action that was just undone, so redo can apply it again.
    pub(crate) fn undone(&mut self, entry: Entry) {
        self.redo.push(entry);
    }

    /// Files an action that was just redone, keeping the rest of the redo list.
    pub(crate) fn redone(&mut self, entry: Entry) {
        self.undo.push(entry);
    }
}
