//! Delimited text data (CSV, TSV and the like), as `GET DATA /TYPE=TXT` reads it.
//!
//! Records follow RFC 4180: fields split by a delimiter, optionally enclosed in a qualifier
//! (a doubled qualifier inside is one literal qualifier, and enclosed fields may span lines);
//! records end at `\n`, `\r\n` or `\r`. Text is UTF-8 (a byte-order mark is skipped), or the
//! encoding asked for; bytes that aren't UTF-8 are read as Windows-1252, with a warning.
//!
//! Without a list of variables, names come from the first record (or are `VAR00001`...) and
//! each column's type is inferred: numeric when every non-empty field is a number, with a
//! decimal comma accepted when the delimiter isn't a comma; otherwise a string as wide as its
//! longest value. Empty numeric fields are system-missing.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mksigma_core::{Dataset, DatasetError, Format, FormatKind, Measure, Value, Variable, is_valid_name};
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum CsvError {
    #[error("the file has no data")]
    Empty,
    #[error("a record has more than {MAX_FIELDS} fields")]
    TooManyFields,
    #[error("{0} is not a character encoding")]
    Encoding(String),
    #[error("{0}")]
    Variables(String),
    #[error(transparent)]
    Dataset(#[from] DatasetError),
}

/// The most variables a dataset can have from one file, as in SPSS.
pub const MAX_FIELDS: usize = 32_767;
const MAX_STRING: usize = 32_767;

#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Characters any of which separates fields; empty to detect one of `,` `;` tab `|`.
    pub delimiters: Vec<char>,
    /// Encloses fields that hold delimiters, line breaks or the qualifier itself.
    pub qualifier: Option<char>,
    /// The first record holds the variable names.
    pub read_names: bool,
    /// The record (from 1) where the cases start; `None` for the one after the names.
    pub first_case: Option<usize>,
    /// Names and formats, in column order, instead of inferring them (`/VARIABLES`).
    pub variables: Vec<(String, Format)>,
    /// An encoding label (`UTF-8`, `windows-1252`, `ISO-8859-1`...); `None` for UTF-8.
    pub encoding: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self { delimiters: Vec::new(), qualifier: Some('"'), read_names: true, first_case: None, variables: Vec::new(), encoding: None }
    }
}

#[derive(Debug, Clone)]
pub struct TextFile {
    pub dataset: Dataset,
    /// The delimiter used: given, or detected.
    pub delimiter: char,
    pub warnings: Vec<String>,
}

fn decode(bytes: &[u8], label: Option<&str>, warnings: &mut Vec<String>) -> Result<String, CsvError> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let encoding = match label {
        Some(l) => encoding_rs::Encoding::for_label(l.trim().as_bytes()).ok_or_else(|| CsvError::Encoding(l.to_string()))?,
        None => match std::str::from_utf8(bytes) {
            Ok(text) => return Ok(text.to_string()),
            Err(_) => {
                warnings.push("the file is not UTF-8; it was read as Windows-1252".to_string());
                encoding_rs::WINDOWS_1252
            }
        },
    };
    let (text, _, malformed) = encoding.decode(bytes);
    if malformed {
        warnings.push(format!("some bytes are not valid {}; they were replaced", encoding.name()));
    }
    Ok(text.into_owned())
}

/// The fields of every record. A record with nothing on its line is left out.
fn records(text: &str, delimiters: &[char], qualifier: Option<char>) -> Result<Vec<Vec<String>>, CsvError> {
    let mut out = Vec::new();
    let mut record: Vec<String> = Vec::new();
    let mut field = String::new();
    // Whether the record has anything: a field, a delimiter or a qualifier.
    let mut started = false;
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    let mut end_record = |record: &mut Vec<String>, field: &mut String, started: &mut bool| -> Result<(), CsvError> {
        if *started {
            record.push(std::mem::take(field));
            if record.len() > MAX_FIELDS {
                return Err(CsvError::TooManyFields);
            }
            out.push(std::mem::take(record));
        }
        *started = false;
        Ok(())
    };
    while let Some(c) = chars.next() {
        if quoted {
            if Some(c) == qualifier {
                if chars.peek() == qualifier.as_ref() {
                    chars.next();
                    field.push(c);
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            _ if Some(c) == qualifier => {
                quoted = true;
                started = true;
            }
            _ if delimiters.contains(&c) => {
                record.push(std::mem::take(&mut field));
                if record.len() > MAX_FIELDS {
                    return Err(CsvError::TooManyFields);
                }
                started = true;
            }
            '\r' | '\n' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                end_record(&mut record, &mut field, &mut started)?;
            }
            _ => {
                field.push(c);
                started = true;
            }
        }
    }
    end_record(&mut record, &mut field, &mut started)?;
    Ok(out)
}

/// The candidate that splits the first lines most evenly into more than one field, outside
/// qualified text: the same count on every line wins over a larger but uneven one.
fn detect_delimiter(text: &str, qualifier: Option<char>) -> char {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(20).collect();
    let count = |line: &str, d: char| {
        let mut quoted = false;
        line.chars()
            .filter(|c| {
                if Some(*c) == qualifier {
                    quoted = !quoted;
                }
                !quoted && *c == d
            })
            .count()
    };
    let mut best = (',', 0usize, false);
    for d in [',', ';', '\t', '|'] {
        let counts: Vec<usize> = lines.iter().map(|l| count(l, d)).collect();
        let min = counts.iter().copied().min().unwrap_or(0);
        let even = counts.iter().all(|c| *c == min);
        if min > 0 && ((even && !best.2) || (even == best.2 && min > best.1)) {
            best = (d, min, even);
        }
    }
    best.0
}

/// A number as written in a field: digits with an optional sign, decimal separator and
/// exponent. Words such as `inf` or `NaN` are not numbers.
fn number(field: &str, decimal_comma: bool) -> Option<f64> {
    let field = field.trim();
    if field.is_empty() || !field.chars().all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | ',' | 'e' | 'E')) {
        return None;
    }
    let normal = match (decimal_comma, field.contains(',')) {
        (_, false) => std::borrow::Cow::Borrowed(field),
        (true, true) if !field.contains('.') && field.matches(',').count() == 1 => std::borrow::Cow::Owned(field.replace(',', ".")),
        _ => return None,
    };
    normal.parse::<f64>().ok().filter(|x| x.is_finite())
}

/// Digits after the decimal separator, ignoring any exponent.
fn decimals(field: &str) -> usize {
    let mantissa = field.trim().split(['e', 'E']).next().unwrap_or_default();
    mantissa.split(['.', ',']).nth(1).map_or(0, str::len)
}

/// A valid, unused variable name from a header; the header becomes the label when it differs.
fn name_from(header: &str, column: usize, used: &mut Vec<String>) -> (String, Option<String>) {
    let raw = header.trim();
    let mut name: String = raw.chars().map(|c| if c.is_alphanumeric() || matches!(c, '_' | '.' | '@' | '#' | '$') { c } else { '_' }).collect();
    if name.starts_with(|c: char| !(c.is_alphabetic() || matches!(c, '@' | '#' | '$'))) {
        name.insert(0, 'v');
    }
    while name.ends_with('.') {
        name.pop();
    }
    while name.len() > 60 {
        name.pop();
    }
    if mksigma_core::RESERVED.iter().any(|r| r.eq_ignore_ascii_case(&name)) {
        name.push('_');
    }
    if !is_valid_name(&name) {
        name = format!("VAR{:05}", column + 1);
    }
    let base = name.clone();
    let mut n = 1;
    while used.iter().any(|u| u.eq_ignore_ascii_case(&name)) {
        n += 1;
        name = format!("{base}_{n}");
    }
    used.push(name.clone());
    let label = (!raw.is_empty() && raw != name).then(|| raw.to_string());
    (name, label)
}

/// A column's variable and values, inferred from its fields.
fn infer(name: String, label: Option<String>, fields: &[&str], decimal_comma: bool) -> (Variable, Vec<Value>) {
    let filled: Vec<&str> = fields.iter().map(|f| f.trim()).filter(|f| !f.is_empty()).collect();
    let as_numbers = |comma: bool| filled.iter().map(|f| number(f, comma)).collect::<Option<Vec<f64>>>();
    let numeric = as_numbers(false).map(|_| false).or_else(|| if decimal_comma { as_numbers(true).map(|_| true) } else { None });
    let mut var = match numeric {
        Some(comma) => {
            let values: Vec<Value> = fields.iter().map(|f| number(f, comma).map_or(Value::SysMis, Value::Num)).collect();
            let places = filled.iter().map(|f| decimals(f)).max().unwrap_or(0).min(16);
            let integer = filled.iter().map(|f| f.split(['.', ',', 'e', 'E']).next().unwrap_or_default().len()).max().unwrap_or(1);
            let width = (integer + if places > 0 { places + 1 } else { 0 }).clamp(8, 40);
            let mut var = Variable::numeric(name);
            var.format = Format::new(FormatKind::F, width as u16, places as u8);
            // Few whole numbers that repeat look like codes for categories.
            let mut distinct: Vec<u64> = values.iter().filter_map(Value::as_num).filter(|x| x.fract() == 0.0).map(f64::to_bits).collect();
            let whole = distinct.len() == filled.len();
            distinct.sort_unstable();
            distinct.dedup();
            if whole && !filled.is_empty() && distinct.len() <= 10 && distinct.len() * 2 <= filled.len() {
                var.measure = Measure::Nominal;
            }
            var.label = label;
            return (var, values);
        }
        None => {
            let width = fields.iter().map(|f| f.trim_end().len()).max().unwrap_or(1).clamp(1, MAX_STRING);
            Variable::string(name, width as u16)
        }
    };
    var.label = label;
    let values = fields.iter().map(|f| Value::Str(f.trim_end().chars().take(MAX_STRING).collect())).collect();
    (var, values)
}

/// A column's values under a given format; fields that aren't numbers in a numeric format are
/// system-missing and counted in `bad`.
fn typed(format: Format, fields: &[&str], decimal_comma: bool, bad: &mut usize) -> Vec<Value> {
    fields
        .iter()
        .map(|f| {
            if format.kind.is_string() {
                return Value::Str(f.trim_end().chars().take(usize::from(format.width)).collect());
            }
            match number(f, decimal_comma) {
                Some(x) => Value::Num(x),
                None if f.trim().is_empty() => Value::SysMis,
                None => {
                    *bad += 1;
                    Value::SysMis
                }
            }
        })
        .collect()
}

pub fn read(bytes: &[u8], options: &Options) -> Result<TextFile, CsvError> {
    let mut warnings = Vec::new();
    let text = decode(bytes, options.encoding.as_deref(), &mut warnings)?;
    let delimiters = if options.delimiters.is_empty() { vec![detect_delimiter(&text, options.qualifier)] } else { options.delimiters.clone() };
    let delimiter = delimiters.first().copied().unwrap_or(',');
    let decimal_comma = !delimiters.contains(&',');
    let all = records(&text, &delimiters, options.qualifier)?;
    let header = if options.read_names { all.first() } else { None };
    let first_case = options.first_case.unwrap_or(if options.read_names { 2 } else { 1 }).max(1);
    let rows = all.get(first_case - 1..).unwrap_or_default();
    let columns = match (&options.variables, header) {
        (vars, _) if !vars.is_empty() => vars.len(),
        (_, Some(h)) => h.len(),
        _ => rows.iter().map(Vec::len).max().unwrap_or(0),
    };
    if columns == 0 {
        return Err(CsvError::Empty);
    }
    let longer = rows.iter().filter(|r| r.len() > columns).count();
    if longer > 0 {
        warnings.push(format!("{longer} records have more than {columns} fields; the extra fields were left out"));
    }
    let mut ds = Dataset::new();
    let mut column_values = Vec::with_capacity(columns);
    let mut used = Vec::new();
    let mut bad = 0;
    for c in 0..columns {
        let fields: Vec<&str> = rows.iter().map(|r| r.get(c).map_or("", String::as_str)).collect();
        let (var, values) = match options.variables.get(c) {
            Some((name, format)) => {
                if !is_valid_name(name) || used.iter().any(|u: &String| u.eq_ignore_ascii_case(name)) {
                    return Err(CsvError::Variables(format!("{name} can't name a variable here")));
                }
                used.push(name.clone());
                let mut var = if format.kind.is_string() { Variable::string(name.clone(), format.width) } else { Variable::numeric(name.clone()) };
                var.format = *format;
                (var, typed(*format, &fields, decimal_comma, &mut bad))
            }
            None => {
                let (name, label) = match header.and_then(|h| h.get(c)) {
                    Some(h) => name_from(h, c, &mut used),
                    None => name_from("", c, &mut used),
                };
                infer(name, label, &fields, decimal_comma)
            }
        };
        ds.add_variable(var)?;
        column_values.push(values);
    }
    if bad > 0 {
        warnings.push(format!("{bad} fields are not numbers in numeric variables; they are system-missing"));
    }
    for case in 0..rows.len() {
        let values = column_values.iter().map(|column| column.get(case).cloned().unwrap_or(Value::SysMis)).collect();
        ds.push_case(values)?;
    }
    Ok(TextFile { dataset: ds, delimiter, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_core::VarType;

    fn column(ds: &Dataset, i: usize) -> Vec<Value> {
        ds.column(i).unwrap().to_vec()
    }

    fn nums(xs: &[f64]) -> Vec<Value> {
        xs.iter().map(|x| Value::Num(*x)).collect()
    }

    #[test]
    fn reads_names_infers_types_and_handles_quotes() {
        let csv =
            "id,Nome completo,renda,grupo\n1,\"Silva, Ana\",1800.5,1\n2,\"Diz \"\"oi\"\"\",,2\r\n3,Bia,4200,1\n4,\"linha\nquebrada\",3900.25,2\n\n";
        let file = read(csv.as_bytes(), &Options::default()).unwrap();
        let ds = &file.dataset;
        assert_eq!(file.delimiter, ',');
        let names: Vec<&str> = ds.variables().iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["id", "Nome_completo", "renda", "grupo"]);
        assert_eq!(ds.variables()[1].label.as_deref(), Some("Nome completo"));
        assert_eq!(ds.n_cases(), 4, "the blank line is not a case");
        assert_eq!(column(ds, 1)[..2], [Value::Str("Silva, Ana".into()), Value::Str("Diz \"oi\"".into())]);
        assert_eq!(column(ds, 1)[3], Value::Str("linha\nquebrada".into()));
        assert_eq!(column(ds, 2), [Value::Num(1800.5), Value::SysMis, Value::Num(4200.0), Value::Num(3900.25)]);
        assert_eq!(ds.variables()[2].format, Format::new(FormatKind::F, 8, 2));
        assert_eq!(ds.variables()[2].measure, Measure::Scale);
        assert_eq!(ds.variables()[3].measure, Measure::Nominal, "two codes, each twice");
        assert_eq!(ds.variables()[0].measure, Measure::Scale, "ids don't repeat");
        assert_eq!(ds.variables()[1].var_type, VarType::String(14), "as wide as \"linha\\nquebrada\"");
    }

    #[test]
    fn semicolons_and_decimal_commas() {
        let csv = "cidade;nota;peso\nRecife;7,5;60\nNatal;8,25;72\nSalvador;6;\n";
        let file = read(csv.as_bytes(), &Options::default()).unwrap();
        assert_eq!(file.delimiter, ';', "detected");
        assert_eq!(column(&file.dataset, 1), nums(&[7.5, 8.25, 6.0]));
        assert_eq!(file.dataset.variables()[1].format.decimals, 2);
        assert_eq!(column(&file.dataset, 2)[2], Value::SysMis);
        // With a comma delimiter, "7,5" would be two fields; a decimal comma is only read when
        // the comma isn't the delimiter.
        assert_eq!(number("7,5", false), None);
        assert_eq!(number("1,000,000", true), None);
        assert_eq!(number("inf", false), None);
        assert_eq!(number("-1.5e3", false), Some(-1500.0));
    }

    #[test]
    fn tabs_explicit_variables_and_no_names() {
        let tsv = "1\tSP\t10\n2\tRJ\tx\n3\tMG\n";
        let options = Options {
            delimiters: vec!['\t'],
            read_names: false,
            variables: vec![("id".into(), "F2.0".parse().unwrap()), ("uf".into(), "A2".parse().unwrap()), ("n".into(), "F4.0".parse().unwrap())],
            ..Options::default()
        };
        let file = read(tsv.as_bytes(), &options).unwrap();
        assert_eq!(file.dataset.n_cases(), 3);
        assert_eq!(column(&file.dataset, 1)[2], Value::Str("MG".into()));
        assert_eq!(column(&file.dataset, 2), [Value::Num(10.0), Value::SysMis, Value::SysMis]);
        assert_eq!(file.warnings, ["1 fields are not numbers in numeric variables; they are system-missing"]);

        let file = read(b"5,6\n7,8,9\n", &Options { read_names: false, ..Options::default() }).unwrap();
        let names: Vec<&str> = file.dataset.variables().iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["VAR00001", "VAR00002", "VAR00003"], "as many as the widest record");
        assert_eq!(column(&file.dataset, 2), [Value::SysMis, Value::Num(9.0)]);
    }

    #[test]
    fn awkward_headers_become_valid_unique_names() {
        let file = read(b"1st,a b,A_B,,x.,to\n1,2,3,4,5,6\n", &Options::default()).unwrap();
        let names: Vec<&str> = file.dataset.variables().iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["v1st", "a_b", "A_B_2", "VAR00004", "x", "to_"], "TO is a keyword");
    }

    #[test]
    fn encodings_and_bad_input() {
        // "José" in Windows-1252: é is 0xE9, not UTF-8.
        let file = read(b"nome\nJos\xE9\n", &Options::default()).unwrap();
        assert_eq!(column(&file.dataset, 0), [Value::Str("José".into())]);
        assert_eq!(file.warnings.len(), 1);
        let file = read(b"\xEF\xBB\xBFa\n1\n", &Options::default()).unwrap();
        assert_eq!(file.dataset.variables()[0].name, "a", "the byte-order mark is skipped");
        assert!(matches!(read(b"", &Options::default()), Err(CsvError::Empty)));
        assert!(matches!(read(b"a\n1\n", &Options { encoding: Some("klingon".into()), ..Options::default() }), Err(CsvError::Encoding(_))));
        let wide = ",".repeat(MAX_FIELDS + 1);
        assert!(matches!(read(wide.as_bytes(), &Options::default()), Err(CsvError::TooManyFields)));
        assert!(read(b"\"never closed\n1,2", &Options::default()).is_ok(), "an open quote runs to the end, no crash");
    }
}
