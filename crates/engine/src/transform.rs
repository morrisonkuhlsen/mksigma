//! `COMPUTE`, `IF`, `RECODE` and `SELECT IF`. Each works out every new value from the data as it
//! was before the command, checks everything, and only then changes the dataset, returning the
//! edits that undo it.

use mksigma_core::{Dataset, Value, VarType, Variable, is_valid_name};
use mksigma_syntax::{Expr, Literal, RecodeFrom, RecodeGroup, RecodeTo};

use crate::EngineError;
use crate::expr::{Case, Compiler, Node, Type};
use crate::history::Edit;

/// `s` cut to `width` bytes on a character boundary.
fn fit(mut s: String, width: u16) -> String {
    let mut end = usize::from(width).min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
    s
}

/// Writes `column` to `target`: replaces an existing variable's values, or adds `new` at the end.
fn store(ds: &mut Dataset, target: Result<usize, Variable>, column: Vec<Value>) -> Result<Edit, EngineError> {
    Ok(match target {
        Ok(index) => {
            let before = ds.replace_column(index, column.clone())?;
            Edit::Column { index, before, after: column }
        }
        Err(var) => {
            let index = ds.variables().len();
            ds.insert_variable(index, var.clone(), column.clone())?;
            Edit::InsertVariable { index, var, column }
        }
    })
}

/// The variable a command writes to: an existing one of the right type, or a new numeric one.
fn target(ds: &Dataset, name: &str, kind: Type, command: &str) -> Result<Result<usize, Variable>, EngineError> {
    match ds.variable(name) {
        Some((i, var)) => match (var.var_type, kind) {
            (VarType::Numeric, Type::Num) | (VarType::String(_), Type::Str) => Ok(Ok(i)),
            (VarType::Numeric, Type::Str) => Err(EngineError::Invalid(format!("{command}: {name} is numeric but gets text"))),
            (VarType::String(_), Type::Num) => Err(EngineError::Invalid(format!("{command}: {name} is a string variable but gets a number"))),
        },
        None if kind == Type::Str => {
            Err(EngineError::Invalid(format!("{command}: declare the new string variable first, e.g. STRING {name} (A20).")))
        }
        None if !is_valid_name(name) => Err(EngineError::Dataset(mksigma_core::DatasetError::InvalidName(name.to_string()))),
        None => Ok(Err(Variable::numeric(name))),
    }
}

/// `COMPUTE target = expr`, or with `cond`, `IF (cond) target = expr`: cases where the condition
/// isn't true keep their value (system-missing or blank for a new variable).
pub(crate) fn compute(ds: &mut Dataset, target_name: &str, expr: &Expr, cond: Option<&Expr>) -> Result<Vec<Edit>, EngineError> {
    let command = if cond.is_some() { "IF" } else { "COMPUTE" };
    let compiler = Compiler { ds };
    let (node, kind) = compiler.compile(expr)?;
    let cond: Option<Node> = match cond {
        Some(c) => match compiler.compile(c)? {
            (node, Type::Num) => Some(node),
            (_, Type::Str) => return Err(EngineError::Invalid("IF needs a true/false condition".into())),
        },
        None => None,
    };
    let target = target(ds, target_name, kind, command)?;
    let width = match &target {
        Ok(i) => match ds.variables().get(*i).map(|v| v.var_type) {
            Some(VarType::String(w)) => w,
            _ => 0,
        },
        Err(_) => 0,
    };
    let old = |case: usize| match &target {
        Ok(i) => ds.column(*i).and_then(|c| c.get(case)).cloned().unwrap_or(Value::SysMis),
        Err(var) => Dataset::blank(var),
    };
    let column: Vec<Value> = (0..ds.n_cases())
        .map(|case| {
            let row = Case { ds, case };
            if let Some(cond) = &cond
                && row.num(cond).is_none_or(|x| x == 0.0)
            {
                return old(case);
            }
            match kind {
                Type::Num => row.num(&node).map_or(Value::SysMis, Value::Num),
                Type::Str => Value::Str(fit(row.text(&node), width)),
            }
        })
        .collect();
    Ok(vec![store(ds, target, column)?])
}

fn literal_value(l: &Literal) -> Value {
    match l {
        Literal::Num(x) => Value::Num(*x),
        Literal::Str(s) => Value::Str(s.trim_end().to_string()),
    }
}

fn matches(from: &RecodeFrom, var: &Variable, v: &Value) -> bool {
    match from {
        RecodeFrom::Value(l) => literal_value(l) == *v,
        RecodeFrom::Range(lo, hi) => {
            matches!(v, Value::Num(x) if lo.is_none_or(|lo| lo <= *x) && hi.is_none_or(|hi| *x <= hi))
        }
        RecodeFrom::Missing => var.is_missing(v),
        RecodeFrom::SysMis => *v == Value::SysMis,
        RecodeFrom::Else => true,
    }
}

/// `RECODE`: in place, or `INTO` new or existing variables. The first matching specification
/// wins. Unmatched values stay as they are in place; with `INTO` they leave the target as it was
/// (system-missing or blank for a new variable).
pub(crate) fn recode(ds: &mut Dataset, groups: &[RecodeGroup]) -> Result<Vec<Edit>, EngineError> {
    let mut planned: Vec<(Result<usize, Variable>, Vec<Value>)> = Vec::new();
    for group in groups {
        for (k, name) in group.variables.iter().enumerate() {
            let (source, var) = ds.variable(name).ok_or_else(|| EngineError::UnknownVariable(name.clone()))?;
            let numeric = var.var_type == VarType::Numeric;
            for spec in &group.specs {
                for from in &spec.from {
                    let ok = match from {
                        RecodeFrom::Value(Literal::Num(_)) | RecodeFrom::Range(..) | RecodeFrom::SysMis => numeric,
                        RecodeFrom::Value(Literal::Str(_)) => !numeric,
                        RecodeFrom::Missing | RecodeFrom::Else => true,
                    };
                    if !ok {
                        let kind = if numeric { "numbers" } else { "quoted strings" };
                        return Err(EngineError::Invalid(format!("RECODE: {name} takes {kind} on the left of =")));
                    }
                }
            }
            // The target's type: the source's in place; INTO follows the new values.
            let to_text = group.specs.iter().any(|s| matches!(s.to, RecodeTo::Value(Literal::Str(_))));
            let to_number = group.specs.iter().any(|s| matches!(s.to, RecodeTo::Value(Literal::Num(_)) | RecodeTo::SysMis));
            let into = group.into.get(k);
            let kind = match (into, to_text, to_number) {
                (_, true, true) => return Err(EngineError::Invalid("RECODE: new values mix numbers and text".into())),
                (None, _, _) => {
                    if numeric {
                        Type::Num
                    } else {
                        Type::Str
                    }
                }
                (Some(_), true, false) => Type::Str,
                (Some(_), false, true) => Type::Num,
                (Some(_), false, false) => {
                    if numeric {
                        Type::Num
                    } else {
                        Type::Str
                    }
                }
            };
            if group.specs.iter().any(|s| s.to == RecodeTo::Copy) && (kind == Type::Num) != numeric {
                return Err(EngineError::Invalid(format!("RECODE: COPY needs {name} and its target to be the same type")));
            }
            let target = match into {
                None => Ok(source),
                Some(t) if planned.iter().any(|(p, _)| matches!(p, Err(v) if v.name.eq_ignore_ascii_case(t))) => {
                    return Err(EngineError::Dataset(mksigma_core::DatasetError::DuplicateName(t.clone())));
                }
                Some(t) => target(ds, t, kind, "RECODE")?,
            };
            let width = match &target {
                Ok(i) => match ds.variables().get(*i).map(|v| v.var_type) {
                    Some(VarType::String(w)) => w,
                    _ => 0,
                },
                Err(_) => 0,
            };
            let input = ds.column(source).unwrap_or_default();
            let column = input
                .iter()
                .enumerate()
                .map(|(case, v)| {
                    let hit = group.specs.iter().find(|s| s.from.iter().any(|f| matches(f, var, v)));
                    let value = match hit.map(|s| &s.to) {
                        Some(RecodeTo::Value(l)) => literal_value(l),
                        Some(RecodeTo::SysMis) => Value::SysMis,
                        Some(RecodeTo::Copy) => v.clone(),
                        None => match &target {
                            Ok(i) => ds.column(*i).and_then(|c| c.get(case)).cloned().unwrap_or(Value::SysMis),
                            Err(var) => Dataset::blank(var),
                        },
                    };
                    match value {
                        Value::Str(s) if width > 0 => Value::Str(fit(s, width)),
                        value => value,
                    }
                })
                .collect();
            planned.push((target, column));
        }
    }
    planned.into_iter().map(|(target, column)| store(ds, target, column)).collect()
}

/// `SELECT IF`: keeps the cases where the condition is true; false or missing go.
pub(crate) fn select_if(ds: &mut Dataset, cond: &Expr) -> Result<Vec<Edit>, EngineError> {
    let (node, kind) = Compiler { ds }.compile(cond)?;
    if kind != Type::Num {
        return Err(EngineError::Invalid("SELECT IF needs a true/false condition".into()));
    }
    let drop: Vec<usize> = (0..ds.n_cases()).filter(|&case| Case { ds, case }.num(&node).is_none_or(|x| x == 0.0)).collect();
    if drop.is_empty() {
        return Ok(Vec::new());
    }
    let rows = ds.remove_cases(&drop)?;
    Ok(vec![Edit::RemoveCases { indexes: drop, rows }])
}
