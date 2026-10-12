//! MK Sigma core types: a dataset is a list of variables (with their dictionary: labels, value
//! labels, measurement level, user-missing values) and their values, stored by column.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use thiserror::Error;

/// One cell of the data matrix.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(f64),
    Str(String),
    /// The system-missing value (shown as `.`).
    SysMis,
}

impl Value {
    pub fn as_num(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarType {
    Numeric,
    /// A string variable of the given width in bytes.
    String(u16),
}

/// Measurement level, as in Variable View.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Measure {
    #[default]
    Nominal,
    Ordinal,
    Scale,
}

/// User-missing values: up to three discrete values, or a range plus one discrete value.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum MissingValues {
    #[default]
    None,
    Discrete(Vec<Value>),
    Range {
        low: f64,
        high: f64,
        discrete: Option<f64>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    pub name: String,
    pub label: Option<String>,
    pub var_type: VarType,
    pub measure: Measure,
    pub value_labels: Vec<(Value, String)>,
    pub missing: MissingValues,
}

impl Variable {
    pub fn numeric(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: None,
            var_type: VarType::Numeric,
            measure: Measure::Scale,
            value_labels: Vec::new(),
            missing: MissingValues::None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The label if there is one, else the name (how output tables title a variable).
    pub fn display_name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }

    /// Whether `v` is system-missing or one of this variable's user-missing values.
    pub fn is_missing(&self, v: &Value) -> bool {
        match (&self.missing, v) {
            (_, Value::SysMis) => true,
            (MissingValues::None, _) => false,
            (MissingValues::Discrete(list), v) => list.contains(v),
            (MissingValues::Range { low, high, discrete }, Value::Num(n)) => (*low..=*high).contains(n) || *discrete == Some(*n),
            (MissingValues::Range { .. }, Value::Str(_)) => false,
        }
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum DatasetError {
    #[error("a variable named {0} already exists")]
    DuplicateName(String),
    #[error("variables can only be added to an empty dataset")]
    NotEmpty,
    #[error("a case has {got} values but the dataset has {expected} variables")]
    CaseWidth { expected: usize, got: usize },
}

/// Variables and their values, stored by column.
#[derive(Debug, Clone, Default)]
pub struct Dataset {
    variables: Vec<Variable>,
    columns: Vec<Vec<Value>>,
}

impl Dataset {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a variable to a dataset with no cases yet. Names are unique ignoring case.
    pub fn add_variable(&mut self, var: Variable) -> Result<usize, DatasetError> {
        if self.n_cases() > 0 {
            return Err(DatasetError::NotEmpty);
        }
        if self.variable(&var.name).is_some() {
            return Err(DatasetError::DuplicateName(var.name));
        }
        self.variables.push(var);
        self.columns.push(Vec::new());
        Ok(self.variables.len() - 1)
    }

    pub fn push_case(&mut self, values: Vec<Value>) -> Result<(), DatasetError> {
        if values.len() != self.variables.len() {
            return Err(DatasetError::CaseWidth { expected: self.variables.len(), got: values.len() });
        }
        for (column, v) in self.columns.iter_mut().zip(values) {
            column.push(v);
        }
        Ok(())
    }

    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }

    pub fn n_cases(&self) -> usize {
        self.columns.first().map_or(0, Vec::len)
    }

    /// Looks a variable up by name, ignoring case.
    pub fn variable(&self, name: &str) -> Option<(usize, &Variable)> {
        self.variables.iter().enumerate().find(|(_, v)| v.name.eq_ignore_ascii_case(name))
    }

    pub fn column(&self, index: usize) -> Option<&[Value]> {
        self.columns.get(index).map(Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_ignoring_case() {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("Age")).unwrap();
        assert_eq!(ds.add_variable(Variable::numeric("AGE")), Err(DatasetError::DuplicateName("AGE".into())));
        assert_eq!(ds.variable("age").map(|(i, _)| i), Some(0));
    }

    #[test]
    fn cases_must_match_the_variable_count() {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("x")).unwrap();
        assert!(ds.push_case(vec![Value::Num(1.0), Value::Num(2.0)]).is_err());
        ds.push_case(vec![Value::Num(1.0)]).unwrap();
        assert_eq!(ds.n_cases(), 1);
        assert_eq!(ds.add_variable(Variable::numeric("y")), Err(DatasetError::NotEmpty));
    }

    #[test]
    fn user_missing_ranges() {
        let mut v = Variable::numeric("income");
        v.missing = MissingValues::Range { low: 900.0, high: 999.0, discrete: Some(-1.0) };
        assert!(v.is_missing(&Value::Num(950.0)));
        assert!(v.is_missing(&Value::Num(-1.0)));
        assert!(v.is_missing(&Value::SysMis));
        assert!(!v.is_missing(&Value::Num(10.0)));
    }
}
