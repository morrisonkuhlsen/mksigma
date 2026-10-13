//! Dictionary commands (`VARIABLE LABELS`, `VALUE LABELS`, `MISSING VALUES`, `VARIABLE LEVEL`,
//! `FORMATS`, `RENAME VARIABLES`, `NUMERIC`, `STRING`, `DELETE VARIABLES`) and cell and case
//! edits. A command is checked in full before anything changes, so a failing command leaves the
//! dataset as it was. Each returns the `Edit`s that undo and redo it.

use mksigma_core::{Dataset, Format, FormatKind, Measure, MissingValues, Value, VarType, Variable, is_valid_name};
use mksigma_syntax::{Command, Level, Literal, MissingSpec};

use crate::EngineError;
use crate::history::Edit;

enum Change {
    Label(usize, Option<String>),
    ValueLabels(usize, Vec<(Value, String)>),
    Missing(usize, MissingValues),
    Measure(usize, Measure),
    Format(usize, Format),
}

impl Change {
    fn variable(&self) -> usize {
        match self {
            Change::Label(i, _) | Change::ValueLabels(i, _) | Change::Missing(i, _) | Change::Measure(i, _) | Change::Format(i, _) => *i,
        }
    }
}

fn index(ds: &Dataset, name: &str) -> Result<usize, EngineError> {
    ds.variable(name).map(|(i, _)| i).ok_or_else(|| EngineError::UnknownVariable(name.to_string()))
}

fn indexes(ds: &Dataset, names: &[String]) -> Result<Vec<usize>, EngineError> {
    names.iter().map(|n| index(ds, n)).collect()
}

fn variable(ds: &Dataset, i: usize) -> Result<&Variable, EngineError> {
    ds.variables().get(i).ok_or(EngineError::Dataset(mksigma_core::DatasetError::NoVariable(i)))
}

/// A literal as a value of `var`: numbers for numeric variables, text for string variables.
fn value_for(var: &Variable, literal: &Literal) -> Result<Value, EngineError> {
    match (var.var_type, literal) {
        (VarType::Numeric, Literal::Num(x)) => Ok(Value::Num(*x)),
        (VarType::String(_), Literal::Str(s)) => Ok(Value::Str(s.clone())),
        (VarType::Numeric, Literal::Str(s)) => Err(EngineError::Invalid(format!("{} is numeric; '{s}' is text", var.name))),
        (VarType::String(_), Literal::Num(x)) => Err(EngineError::Invalid(format!("{} is a string variable; quote the value ('{x}')", var.name))),
    }
}

fn changes(ds: &Dataset, command: &Command) -> Result<Vec<Change>, EngineError> {
    let mut out = Vec::new();
    match command {
        Command::VariableLabels(list) => {
            for (name, label) in list {
                out.push(Change::Label(index(ds, name)?, Some(label.clone()).filter(|l| !l.is_empty())));
            }
        }
        Command::ValueLabels(groups) => {
            for (names, labels) in groups {
                for i in indexes(ds, names)? {
                    let var = variable(ds, i)?;
                    let labels = labels.iter().map(|(v, l)| Ok((value_for(var, v)?, l.clone()))).collect::<Result<_, EngineError>>()?;
                    out.push(Change::ValueLabels(i, labels));
                }
            }
        }
        Command::MissingValues(groups) => {
            for (names, spec) in groups {
                for i in indexes(ds, names)? {
                    let var = variable(ds, i)?;
                    let missing = match spec {
                        MissingSpec::None => MissingValues::None,
                        MissingSpec::Discrete(list) => MissingValues::Discrete(list.iter().map(|l| value_for(var, l)).collect::<Result<_, _>>()?),
                        MissingSpec::Range { low, high, discrete } => {
                            if var.var_type != VarType::Numeric {
                                return Err(EngineError::Invalid(format!("{} is a string variable; ranges need numbers", var.name)));
                            }
                            let (low, high) = (low.unwrap_or(f64::MIN), high.unwrap_or(f64::MAX));
                            if low > high {
                                return Err(EngineError::Invalid(format!("{low} THRU {high} is an empty range")));
                            }
                            MissingValues::Range { low, high, discrete: *discrete }
                        }
                    };
                    out.push(Change::Missing(i, missing));
                }
            }
        }
        Command::VariableLevel(groups) => {
            for (names, level) in groups {
                for i in indexes(ds, names)? {
                    let var = variable(ds, i)?;
                    let measure = match level {
                        Level::Nominal => Measure::Nominal,
                        Level::Ordinal => Measure::Ordinal,
                        Level::Scale if var.var_type == VarType::Numeric => Measure::Scale,
                        Level::Scale => return Err(EngineError::Invalid(format!("{} is a string variable and can't be Scale", var.name))),
                    };
                    out.push(Change::Measure(i, measure));
                }
            }
        }
        Command::Formats(groups) => {
            for (names, text) in groups {
                let format: Format = text.parse().map_err(EngineError::Invalid)?;
                for i in indexes(ds, names)? {
                    let var = variable(ds, i)?;
                    if var.var_type != VarType::Numeric || format.kind.is_string() {
                        return Err(EngineError::Invalid(format!("FORMATS sets numeric formats; {} can't be {format}", var.name)));
                    }
                    out.push(Change::Format(i, format));
                }
            }
        }
        _ => {}
    }
    Ok(out)
}

/// Renames in order (`(a = b) (b = c)` is allowed), checked against the names as they change.
fn rename(ds: &mut Dataset, pairs: &[(String, String)]) -> Result<(), EngineError> {
    let mut names: Vec<String> = ds.variables().iter().map(|v| v.name.clone()).collect();
    let mut plan = Vec::with_capacity(pairs.len());
    for (old, new) in pairs {
        let i = names.iter().position(|n| n.eq_ignore_ascii_case(old)).ok_or_else(|| EngineError::UnknownVariable(old.clone()))?;
        if !is_valid_name(new) {
            return Err(EngineError::Dataset(mksigma_core::DatasetError::InvalidName(new.clone())));
        }
        if names.iter().enumerate().any(|(j, n)| j != i && n.eq_ignore_ascii_case(new)) {
            return Err(EngineError::Dataset(mksigma_core::DatasetError::DuplicateName(new.clone())));
        }
        if let Some(n) = names.get_mut(i) {
            n.clone_from(new);
        }
        plan.push(i);
    }
    // Every step was valid against the names at that point; apply them in the same order.
    for (i, (_, new)) in plan.into_iter().zip(pairs) {
        ds.rename_variable(i, new)?;
    }
    Ok(())
}

/// Applies a dictionary command (other commands do nothing here).
pub(crate) fn apply(ds: &mut Dataset, command: &Command) -> Result<(), EngineError> {
    match command {
        Command::RenameVariables(pairs) => rename(ds, pairs)?,
        _ => {
            for change in changes(ds, command)? {
                let Some(mut var) = ds.variable_mut(change.variable()) else { continue };
                match change {
                    Change::Label(_, label) => var.set_label(label),
                    Change::ValueLabels(_, labels) => var.set_value_labels(labels),
                    Change::Missing(_, missing) => var.set_missing(missing),
                    Change::Measure(_, measure) => var.set_measure(measure),
                    Change::Format(_, format) => var.set_format(format),
                }
            }
        }
    }
    Ok(())
}

/// A cell edit as typed: numeric variables take a number, or nothing (or `.`) for
/// system-missing; string variables take the text.
/// Sets a cell from typed text. Typing in the row after the last case adds a case first.
pub(crate) fn set_cell(ds: &mut Dataset, case: usize, var: usize, input: &str) -> Result<Vec<Edit>, EngineError> {
    let v = variable(ds, var)?;
    let value = match v.var_type {
        VarType::Numeric => match input.trim() {
            "" | "." => Value::SysMis,
            text => Value::Num(
                text.parse::<f64>()
                    .ok()
                    .filter(|x| x.is_finite())
                    .ok_or_else(|| EngineError::Invalid(format!("{text} is not a number ({} is numeric)", v.name)))?,
            ),
        },
        VarType::String(_) => Value::Str(input.trim_end().to_string()),
    };
    let mut edits = Vec::new();
    if case == ds.n_cases() {
        let values = ds.blank_case();
        ds.insert_case(case, values.clone())?;
        edits.push(Edit::InsertCase { index: case, values });
    }
    let before = ds.column(var).and_then(|c| c.get(case)).cloned().ok_or(mksigma_core::DatasetError::NoCase(case))?;
    ds.set_value(case, var, value)?;
    let after = ds.column(var).and_then(|c| c.get(case)).cloned().unwrap_or(Value::SysMis);
    edits.push(Edit::Cell { case, var, before, after });
    Ok(edits)
}

/// `NUMERIC` and `STRING`: new variables at the end, blank in every case.
pub(crate) fn new_variables(ds: &mut Dataset, command: &Command) -> Result<Vec<Edit>, EngineError> {
    let mut planned: Vec<Variable> = Vec::new();
    let groups: Vec<(&Vec<String>, Option<&str>, bool)> = match command {
        Command::Numeric(groups) => groups.iter().map(|(v, f)| (v, f.as_deref(), false)).collect(),
        Command::String(groups) => groups.iter().map(|(v, f)| (v, Some(f.as_str()), true)).collect(),
        _ => return Ok(Vec::new()),
    };
    for (names, format, string) in groups {
        let format: Option<Format> = format.map(str::parse).transpose().map_err(EngineError::Invalid)?;
        for name in names {
            if !is_valid_name(name) {
                return Err(EngineError::Dataset(mksigma_core::DatasetError::InvalidName(name.clone())));
            }
            if ds.variable(name).is_some() || planned.iter().any(|v| v.name.eq_ignore_ascii_case(name)) {
                return Err(EngineError::Dataset(mksigma_core::DatasetError::DuplicateName(name.clone())));
            }
            let var = match (string, format) {
                (false, None) => Variable::numeric(name.clone()),
                (false, Some(f)) if !f.kind.is_string() => {
                    let mut v = Variable::numeric(name.clone());
                    v.format = f;
                    v
                }
                (true, Some(f)) if f.kind == FormatKind::A && f.width <= 255 => Variable::string(name.clone(), f.width),
                (true, _) => return Err(EngineError::Invalid(format!("STRING {name} needs a width from A1 to A255"))),
                (false, Some(f)) => return Err(EngineError::Invalid(format!("NUMERIC {name} can't have the string format {f}"))),
            };
            planned.push(var);
        }
    }
    let mut edits = Vec::with_capacity(planned.len());
    for var in planned {
        let column = vec![Dataset::blank(&var); ds.n_cases()];
        let index = ds.variables().len();
        ds.insert_variable(index, var.clone(), column.clone())?;
        edits.push(Edit::InsertVariable { index, var, column });
    }
    Ok(edits)
}

/// `DELETE VARIABLES`: removed from the last to the first, so undo puts them back in place.
pub(crate) fn delete_variables(ds: &mut Dataset, names: &[String]) -> Result<Vec<Edit>, EngineError> {
    let mut targets = indexes(ds, names)?;
    targets.sort_unstable();
    targets.dedup();
    let mut edits = Vec::with_capacity(targets.len());
    for index in targets.into_iter().rev() {
        let (var, column) = ds.remove_variable(index)?;
        edits.push(Edit::RemoveVariable { index, var, column });
    }
    Ok(edits)
}
