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

/// Display format family, as in Variable View's Type column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatKind {
    /// Plain number (`F`).
    F,
    Comma,
    Dot,
    Dollar,
    Pct,
    /// Scientific notation (`E`).
    E,
    /// Restricted numeric with leading zeros (`N`).
    N,
    Date,
    ADate,
    EDate,
    SDate,
    JDate,
    QYr,
    MoYr,
    WkYr,
    DateTime,
    YmdHms,
    Time,
    DTime,
    MTime,
    WkDay,
    Month,
    /// String (`A`).
    A,
    /// String shown as hex (`AHEX`).
    AHex,
    /// A format MK Sigma doesn't model yet, by its system-file code.
    Other(u8),
}

/// How values display: family, total width and decimal places (`F8.2`, `A20`, `DATE11`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Format {
    pub kind: FormatKind,
    pub width: u16,
    pub decimals: u8,
}

impl Format {
    pub const fn new(kind: FormatKind, width: u16, decimals: u8) -> Self {
        Self { kind, width, decimals }
    }

    /// `v` as this format shows it, without padding. Number formats are covered; date and time
    /// formats show the underlying number until they are implemented.
    pub fn render(&self, v: &Value) -> String {
        let x = match v {
            Value::SysMis => return ".".to_string(),
            Value::Str(s) => return s.clone(),
            Value::Num(x) => *x,
        };
        let d = usize::from(self.decimals);
        match self.kind {
            FormatKind::Comma => grouped(x, d, ',', '.'),
            FormatKind::Dot => grouped(x, d, '.', ','),
            FormatKind::Dollar => {
                let s = grouped(x.abs(), d, ',', '.');
                if x < 0.0 { format!("-${s}") } else { format!("${s}") }
            }
            FormatKind::Pct => format!("{x:.d$}%"),
            FormatKind::E => format!("{x:.d$E}"),
            FormatKind::N => format!("{x:0w$.0}", w = usize::from(self.width)),
            _ => format!("{x:.d$}"),
        }
    }
}

/// Format names as written in syntax, by family.
const FORMAT_NAMES: [(FormatKind, &str); 24] = [
    (FormatKind::F, "F"),
    (FormatKind::Comma, "COMMA"),
    (FormatKind::Dot, "DOT"),
    (FormatKind::Dollar, "DOLLAR"),
    (FormatKind::Pct, "PCT"),
    (FormatKind::E, "E"),
    (FormatKind::N, "N"),
    (FormatKind::Date, "DATE"),
    (FormatKind::ADate, "ADATE"),
    (FormatKind::EDate, "EDATE"),
    (FormatKind::SDate, "SDATE"),
    (FormatKind::JDate, "JDATE"),
    (FormatKind::QYr, "QYR"),
    (FormatKind::MoYr, "MOYR"),
    (FormatKind::WkYr, "WKYR"),
    (FormatKind::DateTime, "DATETIME"),
    (FormatKind::YmdHms, "YMDHMS"),
    (FormatKind::Time, "TIME"),
    (FormatKind::DTime, "DTIME"),
    (FormatKind::MTime, "MTIME"),
    (FormatKind::WkDay, "WKDAY"),
    (FormatKind::Month, "MONTH"),
    (FormatKind::A, "A"),
    (FormatKind::AHex, "AHEX"),
];

impl FormatKind {
    pub fn is_string(self) -> bool {
        matches!(self, FormatKind::A | FormatKind::AHex)
    }
}

impl std::str::FromStr for Format {
    type Err = String;

    /// Parses `F8.2`, `COMMA10.2`, `A20`, `DATE11` (case-insensitive).
    fn from_str(s: &str) -> Result<Self, String> {
        let bad = || format!("{s} is not a format (examples: F8.2, COMMA10.2, A20)");
        let split = s.find(|c: char| c.is_ascii_digit()).ok_or_else(bad)?;
        let (name, numbers) = s.split_at(split);
        let kind = FORMAT_NAMES.iter().find(|(_, n)| n.eq_ignore_ascii_case(name)).map(|(k, _)| *k).ok_or_else(bad)?;
        let (width, decimals) = numbers.split_once('.').unwrap_or((numbers, "0"));
        let width: u16 = width.parse().map_err(|_| bad())?;
        let decimals: u8 = decimals.parse().map_err(|_| bad())?;
        if width == 0 || (decimals > 0 && u16::from(decimals) >= width) || (kind.is_string() && decimals > 0) {
            return Err(bad());
        }
        Ok(Format::new(kind, width, decimals))
    }
}

/// Whether `name` can name a variable: starts with a letter or `@#$`, continues with letters,
/// digits or `_.@#$`, doesn't end with a period, at most 64 bytes.
pub fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || matches!(c, '@' | '#' | '$'))
        && chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '@' | '#' | '$'))
        && !name.ends_with('.')
        && name.len() <= 64
}

/// `x` with `decimals` places and digits grouped in threes (`1,234,567.89`).
fn grouped(x: f64, decimals: usize, separator: char, point: char) -> String {
    let plain = format!("{:.decimals$}", x.abs());
    let (int, frac) = plain.split_once('.').map_or((plain.as_str(), None), |(i, f)| (i, Some(f)));
    let mut out = String::with_capacity(plain.len() + plain.len() / 3 + 1);
    if x < 0.0 && plain.bytes().any(|b| b.is_ascii_digit() && b != b'0') {
        out.push('-');
    }
    let digits = int.chars().count();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (digits - i) % 3 == 0 {
            out.push(separator);
        }
        out.push(c);
    }
    if let Some(frac) = frac {
        out.push(point);
        out.push_str(frac);
    }
    out
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self.kind {
            FormatKind::F => "F",
            FormatKind::Comma => "COMMA",
            FormatKind::Dot => "DOT",
            FormatKind::Dollar => "DOLLAR",
            FormatKind::Pct => "PCT",
            FormatKind::E => "E",
            FormatKind::N => "N",
            FormatKind::Date => "DATE",
            FormatKind::ADate => "ADATE",
            FormatKind::EDate => "EDATE",
            FormatKind::SDate => "SDATE",
            FormatKind::JDate => "JDATE",
            FormatKind::QYr => "QYR",
            FormatKind::MoYr => "MOYR",
            FormatKind::WkYr => "WKYR",
            FormatKind::DateTime => "DATETIME",
            FormatKind::YmdHms => "YMDHMS",
            FormatKind::Time => "TIME",
            FormatKind::DTime => "DTIME",
            FormatKind::MTime => "MTIME",
            FormatKind::WkDay => "WKDAY",
            FormatKind::Month => "MONTH",
            FormatKind::A => "A",
            FormatKind::AHex => "AHEX",
            FormatKind::Other(code) => return write!(f, "format {code} ({})", self.width),
        };
        write!(f, "{name}{}", self.width)?;
        if self.decimals > 0 || matches!(self.kind, FormatKind::F | FormatKind::Comma | FormatKind::Dot | FormatKind::Dollar | FormatKind::Pct) {
            write!(f, ".{}", self.decimals)?;
        }
        Ok(())
    }
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
    pub format: Format,
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
            format: Format::new(FormatKind::F, 8, 2),
            measure: Measure::Scale,
            value_labels: Vec::new(),
            missing: MissingValues::None,
        }
    }

    /// A string variable `width` bytes wide (1 to 32767).
    pub fn string(name: impl Into<String>, width: u16) -> Self {
        Self {
            name: name.into(),
            label: None,
            var_type: VarType::String(width),
            format: Format::new(FormatKind::A, width, 0),
            measure: Measure::Nominal,
            value_labels: Vec::new(),
            missing: MissingValues::None,
        }
    }

    /// The label attached to `v`, if any.
    pub fn value_label(&self, v: &Value) -> Option<&str> {
        self.value_labels.iter().find(|(value, _)| value == v).map(|(_, label)| label.as_str())
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

#[derive(Debug, Clone, Error, PartialEq)]
pub enum DatasetError {
    #[error("a variable named {0} already exists")]
    DuplicateName(String),
    #[error("variables can only be added to an empty dataset")]
    NotEmpty,
    #[error("a case has {got} values but the dataset has {expected} variables")]
    CaseWidth { expected: usize, got: usize },
    #[error("{0} is not a valid variable name")]
    InvalidName(String),
    #[error("there is no variable {0}")]
    NoVariable(usize),
    #[error("there is no case {0}")]
    NoCase(usize),
    #[error("{0} is a string variable and needs text")]
    NeedsText(String),
    #[error("{0} is a numeric variable and needs a number")]
    NeedsNumber(String),
}

/// Mutable access to a variable's label, format, measurement level, value labels and missing
/// values; not its name or type.
pub struct VariableMut<'a>(&'a mut Variable);

impl VariableMut<'_> {
    pub fn get(&self) -> &Variable {
        self.0
    }
    pub fn set_label(&mut self, label: Option<String>) {
        self.0.label = label;
    }
    pub fn set_format(&mut self, format: Format) {
        self.0.format = format;
    }
    pub fn set_measure(&mut self, measure: Measure) {
        self.0.measure = measure;
    }
    pub fn set_value_labels(&mut self, labels: Vec<(Value, String)>) {
        self.0.value_labels = labels;
    }
    pub fn set_missing(&mut self, missing: MissingValues) {
        self.0.missing = missing;
    }
}

/// Variables and their values, stored by column, plus file-level metadata.
#[derive(Debug, Clone, Default)]
pub struct Dataset {
    variables: Vec<Variable>,
    columns: Vec<Vec<Value>>,
    /// File label (`FILE LABEL`).
    pub label: Option<String>,
    /// Document lines (`DOCUMENT`).
    pub documents: Vec<String>,
    /// Index of the weight variable (`WEIGHT BY`), if cases are weighted.
    pub weight: Option<usize>,
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

    /// A variable to change in place. Its name and type are fixed here: rename through
    /// `rename_variable`, which keeps names unique.
    pub fn variable_mut(&mut self, index: usize) -> Option<VariableMut<'_>> {
        self.variables.get_mut(index).map(VariableMut)
    }

    /// Renames a variable; the new name must be valid and not used by another variable.
    pub fn rename_variable(&mut self, index: usize, name: &str) -> Result<(), DatasetError> {
        if !is_valid_name(name) {
            return Err(DatasetError::InvalidName(name.to_string()));
        }
        if self.variable(name).is_some_and(|(i, _)| i != index) {
            return Err(DatasetError::DuplicateName(name.to_string()));
        }
        let var = self.variables.get_mut(index).ok_or(DatasetError::NoVariable(index))?;
        var.name = name.to_string();
        Ok(())
    }

    /// Sets one value. Numbers go in numeric variables, text in string variables (cut to the
    /// variable's width in bytes, on a character boundary).
    pub fn set_value(&mut self, case: usize, index: usize, value: Value) -> Result<(), DatasetError> {
        let var = self.variables.get(index).ok_or(DatasetError::NoVariable(index))?;
        let value = match (var.var_type, value) {
            (VarType::Numeric, v @ (Value::Num(_) | Value::SysMis)) => v,
            (VarType::String(width), Value::Str(mut s)) => {
                let mut end = usize::from(width).min(s.len());
                while !s.is_char_boundary(end) {
                    end -= 1;
                }
                s.truncate(end);
                Value::Str(s)
            }
            (VarType::Numeric, _) => return Err(DatasetError::NeedsNumber(var.name.clone())),
            (VarType::String(_), _) => return Err(DatasetError::NeedsText(var.name.clone())),
        };
        let cell = self.columns.get_mut(index).and_then(|c| c.get_mut(case)).ok_or(DatasetError::NoCase(case))?;
        *cell = value;
        Ok(())
    }

    /// The weight of `case` under `WEIGHT BY`: 1 when cases aren't weighted. A case whose weight
    /// is missing, zero, negative or not a number weighs 0, which leaves it out of procedures.
    pub fn case_weight(&self, case: usize) -> f64 {
        let Some(index) = self.weight else { return 1.0 };
        let (Some(var), Some(value)) = (self.variables.get(index), self.columns.get(index).and_then(|c| c.get(case))) else {
            return 0.0;
        };
        match value {
            Value::Num(w) if *w > 0.0 && w.is_finite() && !var.is_missing(value) => *w,
            _ => 0.0,
        }
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
    fn renders_number_formats() {
        let v = Value::Num(-1234567.891);
        assert_eq!(Format::new(FormatKind::F, 8, 2).render(&v), "-1234567.89");
        assert_eq!(Format::new(FormatKind::Comma, 12, 2).render(&v), "-1,234,567.89");
        assert_eq!(Format::new(FormatKind::Dot, 12, 1).render(&v), "-1.234.567,9");
        assert_eq!(Format::new(FormatKind::Dollar, 12, 0).render(&v), "-$1,234,568");
        assert_eq!(Format::new(FormatKind::F, 3, 0).render(&Value::Num(4.0)), "4");
        assert_eq!(Format::new(FormatKind::Pct, 6, 1).render(&Value::Num(12.34)), "12.3%");
        assert_eq!(Format::new(FormatKind::N, 5, 0).render(&Value::Num(42.0)), "00042");
        assert_eq!(Format::new(FormatKind::Comma, 8, 0).render(&Value::Num(999.0)), "999");
        assert_eq!(Format::new(FormatKind::F, 8, 2).render(&Value::SysMis), ".");
        assert_eq!(Format::new(FormatKind::A, 8, 0).render(&Value::Str("abc".into())), "abc");
    }

    #[test]
    fn parses_formats() {
        assert_eq!("F8.2".parse(), Ok(Format::new(FormatKind::F, 8, 2)));
        assert_eq!("comma10.2".parse(), Ok(Format::new(FormatKind::Comma, 10, 2)));
        assert_eq!("A20".parse(), Ok(Format::new(FormatKind::A, 20, 0)));
        assert_eq!("DATE11".parse(), Ok(Format::new(FormatKind::Date, 11, 0)));
        for bad in ["F", "X8", "F0", "F2.2", "A8.1", "F8.x", ""] {
            assert!(bad.parse::<Format>().is_err(), "{bad}");
        }
        for f in ["F8.2", "A20", "DOLLAR10.2", "DATE11"] {
            assert_eq!(f.parse::<Format>().map(|f| f.to_string()).as_deref(), Ok(f));
        }
    }

    #[test]
    fn rename_and_set_values() {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("x")).unwrap();
        ds.add_variable(Variable::string("s", 2)).unwrap();
        ds.push_case(vec![Value::Num(1.0), Value::Str("ab".into())]).unwrap();
        assert_eq!(ds.rename_variable(0, "S"), Err(DatasetError::DuplicateName("S".into())));
        assert_eq!(ds.rename_variable(0, "2x"), Err(DatasetError::InvalidName("2x".into())));
        ds.rename_variable(0, "X").unwrap();
        assert_eq!(ds.variables()[0].name, "X");

        ds.set_value(0, 0, Value::SysMis).unwrap();
        ds.set_value(0, 1, Value::Str("São Paulo".into())).unwrap();
        assert_eq!(ds.column(1).unwrap()[0], Value::Str("S".into()), "2 bytes would split the ã, so it stops before it");
        assert_eq!(ds.set_value(0, 0, Value::Str("a".into())), Err(DatasetError::NeedsNumber("X".into())));
        assert_eq!(ds.set_value(5, 0, Value::Num(1.0)), Err(DatasetError::NoCase(5)));
    }

    #[test]
    fn case_weights() {
        let mut ds = Dataset::new();
        let mut w = Variable::numeric("w");
        w.missing = MissingValues::Discrete(vec![Value::Num(9.0)]);
        ds.add_variable(w).unwrap();
        for v in [Value::Num(2.5), Value::Num(0.0), Value::Num(-1.0), Value::Num(9.0), Value::SysMis] {
            ds.push_case(vec![v]).unwrap();
        }
        assert_eq!(ds.case_weight(0), 1.0);
        ds.weight = Some(0);
        let weights: Vec<f64> = (0..ds.n_cases()).map(|c| ds.case_weight(c)).collect();
        assert_eq!(weights, [2.5, 0.0, 0.0, 0.0, 0.0]);
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
