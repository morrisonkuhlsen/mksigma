//! Delimited text data (CSV, TSV and the like), as `GET DATA /TYPE=TXT` reads it.
//!
//! Records follow RFC 4180: fields split by a delimiter, optionally enclosed in a qualifier
//! (a doubled qualifier inside is one literal qualifier, and enclosed fields may span lines). A
//! qualifier opens enclosed text only at the start of a field, after any spaces; anywhere else
//! it is a literal character (`TV 42" LED`). Records end at `\n`, `\r\n` or `\r`.
//!
//! A byte-order mark says the encoding: UTF-8, or UTF-16 as Excel's "Unicode Text" saves it.
//! Without one, text is in the encoding asked for, or UTF-8: in text that is otherwise UTF-8 a
//! bad byte is replaced, and text with no UTF-8 beyond ASCII is read as Windows-1252, each with
//! a warning.
//!
//! Without a list of variables, names come from the first record (or are `VAR00001`...) and
//! each column's type is inferred: numeric when every non-empty field is a number, otherwise a
//! string as wide as its longest value. Identifiers stay strings: a column with digits written
//! with a leading zero (a CEP, a CPF) or more than 15 of them, which a double can't hold exactly.
//! Numbers may group thousands, and when the delimiter isn't a comma the decimal separator may
//! be a comma (`1.234,56`). Each column is read one way throughout, the one that reads all its
//! fields; when both do and differ (`1.500`), the way the file's other columns show, or else a
//! comma with `;` as delimiter and a point otherwise, with a warning. Empty numeric fields are
//! system-missing.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mksigma_core::{Dataset, DatasetError, Format, FormatKind, Measure, Value, Variable, is_valid_name};
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum CsvError {
    #[error("{}", mksigma_i18n::tr!("err-csv-empty"))]
    Empty,
    #[error("{}", mksigma_i18n::tr!("err-csv-fields", max = MAX_FIELDS))]
    TooManyFields,
    #[error("{}", mksigma_i18n::tr!("err-csv-encoding", name = .0.as_str()))]
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
    // A byte-order mark says the encoding, whatever the label (#39).
    if let Some((encoding, bom)) = encoding_rs::Encoding::for_bom(bytes) {
        return Ok(decode_with(encoding, bytes.get(bom..).unwrap_or_default(), warnings));
    }
    let encoding = match label {
        Some(l) => encoding_rs::Encoding::for_label(l.trim().as_bytes()).ok_or_else(|| CsvError::Encoding(l.to_string()))?,
        None => match std::str::from_utf8(bytes) {
            Ok(text) => return Ok(text.to_string()),
            // UTF-8 with a stray bad byte stays UTF-8, the byte replaced; text whose only UTF-8 is
            // ASCII is Windows-1252, as spreadsheets save it (#39).
            Err(_) => {
                let (valid, invalid) = utf8_sequences(bytes);
                if valid > 0 && valid >= invalid {
                    encoding_rs::UTF_8
                } else {
                    warnings.push(mksigma_i18n::tr!("csv-warn-windows-1252"));
                    encoding_rs::WINDOWS_1252
                }
            }
        },
    };
    Ok(decode_with(encoding, bytes, warnings))
}

fn decode_with(encoding: &'static encoding_rs::Encoding, bytes: &[u8], warnings: &mut Vec<String>) -> String {
    let (text, malformed) = encoding.decode_without_bom_handling(bytes);
    if malformed {
        warnings.push(mksigma_i18n::tr!("csv-warn-replaced", encoding = encoding.name()));
    }
    text.into_owned()
}

/// In bytes that aren't all UTF-8: the characters beyond ASCII that are valid UTF-8, and the
/// invalid sequences.
fn utf8_sequences(mut bytes: &[u8]) -> (usize, usize) {
    let beyond_ascii = |text: &str| text.chars().filter(|c| !c.is_ascii()).count();
    let (mut valid, mut invalid) = (0, 0);
    loop {
        match std::str::from_utf8(bytes) {
            Ok(text) => return (valid + beyond_ascii(text), invalid),
            Err(e) => {
                let good = bytes.get(..e.valid_up_to()).unwrap_or_default();
                valid += std::str::from_utf8(good).map_or(0, beyond_ascii);
                invalid += 1;
                // A sequence cut short by the end of the file has no length: nothing follows it.
                bytes = bytes.get(e.valid_up_to() + e.error_len().unwrap_or(bytes.len())..).unwrap_or_default();
            }
        }
    }
}

/// The fields of every record. A record with nothing on its line is left out.
fn records(text: &str, delimiters: &[char], qualifier: Option<char>) -> Result<Vec<Vec<String>>, CsvError> {
    let mut out = Vec::new();
    let mut record: Vec<String> = Vec::new();
    let mut field = String::new();
    // Whether the record has anything: a field, a delimiter or a qualifier.
    let mut started = false;
    let mut quoted = false;
    // At the start of a field, where a qualifier opens enclosed text; elsewhere it is literal (#39).
    let mut fresh = true;
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
            _ if Some(c) == qualifier && fresh => {
                // Spaces before it are no part of the field.
                field.clear();
                quoted = true;
                started = true;
                fresh = false;
            }
            _ if delimiters.contains(&c) => {
                record.push(std::mem::take(&mut field));
                if record.len() > MAX_FIELDS {
                    return Err(CsvError::TooManyFields);
                }
                started = true;
                fresh = true;
            }
            '\r' | '\n' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                end_record(&mut record, &mut field, &mut started)?;
                fresh = true;
            }
            _ => {
                field.push(c);
                started = true;
                fresh &= c == ' ';
            }
        }
    }
    end_record(&mut record, &mut field, &mut started)?;
    Ok(out)
}

/// The candidate that splits the first lines most evenly into more than one field, outside
/// qualified text (opened, as `records` reads it, only at the start of a field): the same count
/// on every line wins over a larger but uneven one.
fn detect_delimiter(text: &str, qualifier: Option<char>) -> char {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(20).collect();
    let count = |line: &str, d: char| {
        let (mut n, mut quoted, mut fresh) = (0, false, true);
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if quoted {
                // A doubled qualifier is a literal one; a single one closes.
                if Some(c) == qualifier && chars.next_if(|next| Some(*next) == qualifier).is_none() {
                    quoted = false;
                }
            } else if Some(c) == qualifier && fresh {
                quoted = true;
                fresh = false;
            } else if c == d {
                n += 1;
                fresh = true;
            } else {
                fresh &= c == ' ';
            }
        }
        n
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

/// The decimal separator of a column's numbers; the other one groups thousands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decimal {
    Point,
    Comma,
}

impl Decimal {
    /// The decimal separator and the thousands one.
    fn marks(self) -> (char, char) {
        match self {
            Decimal::Point => ('.', ','),
            Decimal::Comma => (',', '.'),
        }
    }
}

/// A number as written in a field: digits with an optional sign, decimal separator, exponent,
/// and thousands grouped by the other separator (`1,234.5`, or `1.234,5` with a decimal comma).
/// Words such as `inf` or `NaN` are not numbers.
fn number(field: &str, decimal: Decimal) -> Option<f64> {
    let (point, group) = decimal.marks();
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    let field = field.trim();
    let (mantissa, exponent) = match field.split_once(['e', 'E']) {
        Some((m, e)) => (m, Some(e)),
        None => (field, None),
    };
    let negative = mantissa.starts_with('-');
    let mantissa = mantissa.strip_prefix(['+', '-']).unwrap_or(mantissa);
    let (whole, fraction) = mantissa.split_once(point).unwrap_or((mantissa, ""));
    let mut groups = whole.split(group);
    let lead = groups.next().unwrap_or_default();
    // Groups of three after a lead of one to three digits: `1,234,567`, not `12,34`.
    let grouped = whole.contains(group);
    if (whole.is_empty() && fraction.is_empty())
        || !digits(fraction)
        || !digits(lead)
        || (grouped && !(1..=3).contains(&lead.len()))
        || !groups.all(|g| g.len() == 3 && digits(g))
    {
        return None;
    }
    if let Some(e) = exponent {
        let magnitude = e.strip_prefix(['+', '-']).unwrap_or(e);
        if magnitude.is_empty() || !digits(magnitude) {
            return None;
        }
    }
    let mut normal = String::with_capacity(field.len() + 2);
    if negative {
        normal.push('-');
    }
    // The 0 makes `.5` a number.
    normal.push('0');
    normal.extend(whole.chars().filter(|c| *c != group));
    if !fraction.is_empty() {
        normal.push('.');
        normal.push_str(fraction);
    }
    if let Some(e) = exponent {
        normal.push('e');
        normal.push_str(e);
    }
    normal.parse::<f64>().ok().filter(|x| x.is_finite())
}

/// Digits after the decimal separator, ignoring any exponent.
fn decimals(field: &str, decimal: Decimal) -> usize {
    let mantissa = field.trim().split(['e', 'E']).next().unwrap_or_default();
    mantissa.split_once(decimal.marks().0).map_or(0, |(_, fraction)| fraction.len())
}

/// The digits and sign before the decimal separator, without thousands separators.
fn integer_width(field: &str, decimal: Decimal) -> usize {
    let mantissa = field.trim().split(['e', 'E']).next().unwrap_or_default();
    let whole = mantissa.split_once(decimal.marks().0).map_or(mantissa, |(whole, _)| whole);
    whole.chars().filter(|c| c.is_ascii_digit() || *c == '-').count()
}

/// Digits that identify rather than count, which a number would change: a leading zero
/// (`01310100`, a CEP) or more than 15 of them, beyond what a double holds exactly (#39).
fn identifier(field: &str) -> bool {
    let field = field.trim();
    !field.is_empty() && field.bytes().all(|b| b.is_ascii_digit()) && ((field.len() > 1 && field.starts_with('0')) || field.len() > 15)
}

/// Which decimal separators read every field of a column as a number.
#[derive(Debug, PartialEq)]
enum Fit<'a> {
    Neither,
    Point,
    Comma,
    /// Both do; the first field they read differently (`1.500`), if any.
    Either(Option<&'a str>),
}

/// `comma`: whether a decimal comma may be read (the delimiter isn't a comma).
fn fit<'a>(filled: &[&'a str], comma: bool) -> Fit<'a> {
    let all = |d| filled.iter().map(|f| number(f, d)).collect::<Option<Vec<f64>>>();
    match (all(Decimal::Point).is_some(), comma && all(Decimal::Comma).is_some()) {
        (true, true) => Fit::Either(filled.iter().copied().find(|f| number(f, Decimal::Point) != number(f, Decimal::Comma))),
        (true, false) => Fit::Point,
        (false, true) => Fit::Comma,
        (false, false) => Fit::Neither,
    }
}

/// The decimal separator the file's columns show where only one reads them (`2,5`, `3.14`),
/// and `true`; with none, or both, a guess and `false`: a comma when `;` delimits, as
/// spreadsheets save in comma-decimal languages, and a point otherwise.
fn file_decimal(columns: &[Vec<&str>], comma: bool, semicolon: bool) -> (Decimal, bool) {
    let (mut points, mut commas) = (0, 0);
    for fields in columns {
        let filled: Vec<&str> = fields.iter().map(|f| f.trim()).filter(|f| !f.is_empty()).collect();
        match fit(&filled, comma) {
            Fit::Point => points += 1,
            Fit::Comma => commas += 1,
            _ => {}
        }
    }
    match (points, commas) {
        _ if !comma => (Decimal::Point, true),
        (0, 1..) => (Decimal::Comma, true),
        (1.., 0) => (Decimal::Point, true),
        _ if semicolon => (Decimal::Comma, false),
        _ => (Decimal::Point, false),
    }
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

/// A column's variable and values, inferred from its fields: numbers with `decimal` as their
/// decimal separator, or strings when it is `None`.
fn infer(name: String, label: Option<String>, fields: &[&str], decimal: Option<Decimal>) -> (Variable, Vec<Value>) {
    let filled: Vec<&str> = fields.iter().map(|f| f.trim()).filter(|f| !f.is_empty()).collect();
    let mut var = match decimal {
        Some(decimal) => {
            let values: Vec<Value> = fields.iter().map(|f| number(f, decimal).map_or(Value::SysMis, Value::Num)).collect();
            let places = filled.iter().map(|f| decimals(f, decimal)).max().unwrap_or(0).min(16);
            let integer = filled.iter().map(|f| integer_width(f, decimal)).max().unwrap_or(1);
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
fn typed(format: Format, fields: &[&str], decimal: Decimal, bad: &mut usize) -> Vec<Value> {
    fields
        .iter()
        .map(|f| {
            if format.kind.is_string() {
                return Value::Str(f.trim_end().chars().take(usize::from(format.width)).collect());
            }
            match number(f, decimal) {
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
    let comma = !delimiters.contains(&',');
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
    let table: Vec<Vec<&str>> = (0..columns).map(|c| rows.iter().map(|r| r.get(c).map_or("", String::as_str)).collect()).collect();
    let (file, shown) = file_decimal(&table, comma, delimiters.contains(&';'));
    let mut ds = Dataset::new();
    let mut column_values = Vec::with_capacity(columns);
    let mut used = Vec::new();
    let mut bad = 0;
    // Columns whose numbers read either way with nothing in the file to tell, and one such number.
    let (mut either, mut example) = (Vec::new(), None);
    for (c, fields) in table.iter().enumerate() {
        let filled: Vec<&str> = fields.iter().map(|f| f.trim()).filter(|f| !f.is_empty()).collect();
        let (var, values) = match options.variables.get(c) {
            Some((name, format)) => {
                if !is_valid_name(name) || used.iter().any(|u: &String| u.eq_ignore_ascii_case(name)) {
                    return Err(CsvError::Variables(format!("{name} can't name a variable here")));
                }
                used.push(name.clone());
                let mut var = if format.kind.is_string() { Variable::string(name.clone(), format.width) } else { Variable::numeric(name.clone()) };
                var.format = *format;
                // The separator that reads more of its fields, the file's on a tie.
                let read = |d| filled.iter().filter(|f| number(f, d).is_some()).count();
                let decimal = match (read(Decimal::Point), if comma { read(Decimal::Comma) } else { 0 }) {
                    (points, commas) if commas > points => Decimal::Comma,
                    (points, commas) if points > commas => Decimal::Point,
                    _ => file,
                };
                (var, typed(*format, fields, decimal, &mut bad))
            }
            None => {
                let (name, label) = match header.and_then(|h| h.get(c)) {
                    Some(h) => name_from(h, c, &mut used),
                    None => name_from("", c, &mut used),
                };
                let decimal = match fit(&filled, comma) {
                    _ if filled.iter().any(|f| identifier(f)) => None,
                    Fit::Neither => None,
                    Fit::Point => Some(Decimal::Point),
                    Fit::Comma => Some(Decimal::Comma),
                    Fit::Either(differ) => {
                        if let Some(f) = differ.filter(|_| !shown) {
                            either.push(name.clone());
                            example.get_or_insert(f);
                        }
                        Some(file)
                    }
                };
                infer(name, label, fields, decimal)
            }
        };
        ds.add_variable(var)?;
        column_values.push(values);
    }
    if bad > 0 {
        warnings.push(format!("{bad} fields are not numbers in numeric variables; they are system-missing"));
    }
    if let Some(example) = example {
        let names = either.join(", ");
        warnings.push(match file {
            Decimal::Point => mksigma_i18n::tr!("csv-warn-decimal-point", names = names.as_str(), example = example),
            Decimal::Comma => mksigma_i18n::tr!("csv-warn-decimal-comma", names = names.as_str(), example = example),
        });
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
        assert_eq!(number("7,5", Decimal::Point), None);
        assert_eq!(number("1,000,000", Decimal::Comma), None);
        assert_eq!(number("inf", Decimal::Point), None);
        assert_eq!(number("-1.5e3", Decimal::Point), Some(-1500.0));
    }

    #[test]
    fn thousands_separators() {
        assert_eq!(number("1.234,56", Decimal::Comma), Some(1234.56));
        assert_eq!(number("-1.234.567", Decimal::Comma), Some(-1_234_567.0));
        assert_eq!(number("1,234.5", Decimal::Point), Some(1234.5));
        assert_eq!(number("1,234,567", Decimal::Point), Some(1_234_567.0));
        assert_eq!(number(".5", Decimal::Point), Some(0.5));
        assert_eq!(number("1e+3", Decimal::Point), Some(1000.0));
        for bad in ["1,23", "12,34,567", "1234,567", ",5", "1.2.3", "1e", "-", "1 000", "--1"] {
            assert_eq!(number(bad, Decimal::Point), None, "{bad}");
        }
        for bad in ["1.23", "1.2345", "1,2,3", "1.234.56"] {
            assert_eq!(number(bad, Decimal::Comma), None, "{bad}");
        }
    }

    #[test]
    fn brazilian_numbers_with_semicolons() {
        // A spreadsheet in Portuguese: `;` delimits, `.` groups thousands, `,` is the decimal.
        // "1.500" alone reads either way; beside "2,5" it is 1500 (#39).
        let csv = "item;valor;peso\nA;1.234,56;1.500\nB;2,5;2.300\nC;1.500;\n";
        let file = read(csv.as_bytes(), &Options::default()).unwrap();
        assert_eq!(column(&file.dataset, 1), nums(&[1234.56, 2.5, 1500.0]));
        assert_eq!(file.dataset.variables()[1].format, Format::new(FormatKind::F, 8, 2));
        assert_eq!(column(&file.dataset, 2)[..2], nums(&[1500.0, 2300.0]), "as the valor column shows");
        assert!(file.warnings.is_empty(), "{:?}", file.warnings);

        // Without another column to go by, `;` means a decimal comma, with a warning.
        let file = read(b"a;b\nx;1.500\n", &Options::default()).unwrap();
        assert_eq!(column(&file.dataset, 1), nums(&[1500.0]));
        assert_eq!(file.warnings.len(), 1);
        // A point decimal elsewhere in the file says how to read it.
        let file = read(b"a;b;c\nx;1.500;3.14\n", &Options::default()).unwrap();
        assert_eq!(column(&file.dataset, 1), nums(&[1.5]));
    }

    #[test]
    fn grouped_thousands_with_tabs_and_quoted_commas() {
        // A US TSV: "1,234" is a thousand and more, not 1.234 (#39).
        let file = read(b"n\tv\n1\t1,234\n2\t2,000.5\n", &Options::default()).unwrap();
        assert_eq!(file.delimiter, '\t');
        assert_eq!(column(&file.dataset, 1), nums(&[1234.0, 2000.5]));
        let file = read(b"n\tv\n1\t1,234\n", &Options::default()).unwrap();
        assert_eq!(column(&file.dataset, 1), nums(&[1234.0]), "either way: a point with tabs");
        assert_eq!(file.warnings.len(), 1);
        // A CSV with the grouped numbers quoted.
        let file = read(b"n,v\n1,\"1,234.5\"\n2,7\n", &Options::default()).unwrap();
        assert_eq!(column(&file.dataset, 1), nums(&[1234.5, 7.0]));
    }

    #[test]
    fn identifiers_stay_strings() {
        // A CEP and a CPF keep their leading zeros; 17 digits keep every one (#39).
        let csv = "cep;cpf;conta;n\n01310100;01234567890;12345678901234567;1\n20040002;12345678909;12345678901234568;2\n";
        let file = read(csv.as_bytes(), &Options::default()).unwrap();
        let ds = &file.dataset;
        assert_eq!(column(ds, 0), [Value::Str("01310100".into()), Value::Str("20040002".into())]);
        assert_eq!(column(ds, 1)[0], Value::Str("01234567890".into()));
        assert_eq!(column(ds, 2)[1], Value::Str("12345678901234568".into()));
        assert_eq!(column(ds, 3), nums(&[1.0, 2.0]));
        assert!(!identifier("0") && !identifier("0.5") && !identifier("123456789012345"));
    }

    #[test]
    fn a_qualifier_inside_a_field_is_literal() {
        // RFC 4180 opens quoted text only at a field's start; a quote mid-field used to swallow
        // the rest of the file (#39).
        let file = read(b"id,desc\n1,TV 42\" LED\n2,Radio\n", &Options::default()).unwrap();
        assert_eq!(file.dataset.n_cases(), 2);
        assert_eq!(column(&file.dataset, 1), [Value::Str("TV 42\" LED".into()), Value::Str("Radio".into())]);
        // Spaces before an opening qualifier are dropped; text after a closing one is kept.
        let file = read(b"a,b\n1, \"x,y\"\n2,\"p\"q\n", &Options::default()).unwrap();
        assert_eq!(column(&file.dataset, 1), [Value::Str("x,y".into()), Value::Str("pq".into())]);
        // Detection reads the quote the same way.
        let file = read(b"a;b\n5\" disco;1\nlivro;2\n", &Options::default()).unwrap();
        assert_eq!(file.delimiter, ';');
        assert_eq!(column(&file.dataset, 1), nums(&[1.0, 2.0]));
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
        // Excel's "Unicode Text": UTF-16LE with a byte-order mark, tab-delimited (#39).
        let utf16: Vec<u8> = [0xFEFF].into_iter().chain("nome\tn\nJoão\t1\n".encode_utf16()).flat_map(u16::to_le_bytes).collect();
        let file = read(&utf16, &Options::default()).unwrap();
        assert_eq!(column(&file.dataset, 0), [Value::Str("João".into())]);
        assert!(file.warnings.is_empty());
        let utf16be: Vec<u8> = [0xFEFF].into_iter().chain("a\n1\n".encode_utf16()).flat_map(u16::to_be_bytes).collect();
        assert_eq!(column(&read(&utf16be, &Options::default()).unwrap().dataset, 0), nums(&[1.0]));
        // UTF-8 with one bad byte: the byte is replaced, the rest stays UTF-8 (#39).
        let file = read(b"nome\nJo\xC3\xA3o\nAna\xFF\n", &Options::default()).unwrap();
        assert_eq!(column(&file.dataset, 0), [Value::Str("João".into()), Value::Str("Ana\u{FFFD}".into())]);
        assert_eq!(file.warnings.len(), 1);
        assert_eq!(utf8_sequences(b"Jo\xC3\xA3o \xE9 \xFF\xC3"), (1, 3), "a sequence cut short at the end counts");
        assert!(matches!(read(b"", &Options::default()), Err(CsvError::Empty)));
        assert!(matches!(read(b"a\n1\n", &Options { encoding: Some("klingon".into()), ..Options::default() }), Err(CsvError::Encoding(_))));
        let wide = ",".repeat(MAX_FIELDS + 1);
        assert!(matches!(read(wide.as_bytes(), &Options::default()), Err(CsvError::TooManyFields)));
        assert!(read(b"\"never closed\n1,2", &Options::default()).is_ok(), "an open quote runs to the end, no crash");
    }
}
