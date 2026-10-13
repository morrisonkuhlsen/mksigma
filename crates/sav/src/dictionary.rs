//! The header and the dictionary: everything before the cases. Records are collected raw first,
//! because the character encoding (records 7.20 and 7.3) comes after the names and labels it
//! applies to, then decoded into the dataset's variables.

use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};
use mksigma_core::{Dataset, Format, FormatKind, Measure, MissingValues, Value, VarType, Variable};

use crate::cursor::Cursor;
use crate::{Compression, Flavour, SavError};

/// What the case reader needs: the dataset with its dictionary, and how cases are laid out.
pub(crate) struct Dictionary {
    pub(crate) dataset: Dataset,
    /// Per variable: width in bytes (0 for numeric) and the number of 8-byte segments it spans.
    pub(crate) layout: Vec<(usize, usize)>,
    pub(crate) data_offset: usize,
    pub(crate) big_endian: bool,
    pub(crate) compression: Compression,
    pub(crate) bias: f64,
    pub(crate) sysmis: f64,
    /// Number of cases from the header, if the writer knew it.
    pub(crate) n_cases: Option<usize>,
    pub(crate) encoding: &'static Encoding,
    pub(crate) product: String,
    pub(crate) created: String,
    pub(crate) warnings: Vec<String>,
}

enum RawMissing {
    None,
    Discrete(Vec<[u8; 8]>),
    Range { low: [u8; 8], high: [u8; 8], discrete: Option<[u8; 8]> },
}

struct RawVar<'a> {
    /// 0 for numeric, else the string width (1 to 255).
    width: usize,
    name: &'a [u8],
    label: Option<&'a [u8]>,
    print: i32,
    missing: RawMissing,
    /// Index of the variable's first 8-byte segment in a case.
    segment: usize,
}

struct RawLabels<'a> {
    labels: Vec<([u8; 8], &'a [u8])>,
    /// 1-based segment indexes of the variables they apply to.
    segments: Vec<usize>,
}

/// A format family from its system-file code.
fn format_kind(code: i32) -> FormatKind {
    match code {
        1 => FormatKind::A,
        2 => FormatKind::AHex,
        3 => FormatKind::Comma,
        4 => FormatKind::Dollar,
        5 => FormatKind::F,
        16 => FormatKind::N,
        17 => FormatKind::E,
        20 => FormatKind::Date,
        21 => FormatKind::Time,
        22 => FormatKind::DateTime,
        23 => FormatKind::ADate,
        24 => FormatKind::JDate,
        25 => FormatKind::DTime,
        26 => FormatKind::WkDay,
        27 => FormatKind::Month,
        28 => FormatKind::MoYr,
        29 => FormatKind::QYr,
        30 => FormatKind::WkYr,
        31 => FormatKind::Pct,
        32 => FormatKind::Dot,
        38 => FormatKind::EDate,
        39 => FormatKind::SDate,
        40 => FormatKind::MTime,
        41 => FormatKind::YmdHms,
        other => FormatKind::Other(u8::try_from(other).unwrap_or(0)),
    }
}

/// A packed format: type in bits 16–23, width in 8–15, decimals in 0–7.
fn format(packed: i32) -> Format {
    Format::new(format_kind((packed >> 16) & 0xff), ((packed >> 8) & 0xff) as u16, (packed & 0xff) as u8)
}

/// The encoding named by record 7.20, else by the code page in record 7.3, else Windows-1252
/// (what SPSS used before Unicode mode; a superset of ASCII).
fn encoding(name: Option<&[u8]>, code_page: Option<i32>, warnings: &mut Vec<String>) -> &'static Encoding {
    if let Some(name) = name {
        let name = String::from_utf8_lossy(name);
        match Encoding::for_label(name.trim().as_bytes()) {
            Some(e) => return e,
            None => warnings.push(format!("unknown character encoding {name:?}; reading text as Windows-1252")),
        }
    }
    match code_page {
        None | Some(2 | 3 | 1252 | 28591) => WINDOWS_1252,
        Some(65001) => UTF_8,
        Some(cp) => Encoding::for_label(format!("windows-{cp}").as_bytes())
            .or_else(|| Encoding::for_label(format!("cp{cp}").as_bytes()))
            .unwrap_or_else(|| {
                warnings.push(format!("unknown code page {cp}; reading text as Windows-1252"));
                WINDOWS_1252
            }),
    }
}

/// Labels for one long string variable from record 7.21: the variable's name and its
/// `(value, label)` pairs, raw.
type LongLabels<'a> = (String, Vec<(&'a [u8], &'a [u8])>);

/// Record 7.21, value labels of strings over 8 bytes. Per variable: name length and name,
/// width, label count; per label: value length and value, label length and label.
fn long_string_value_labels<'a>(body: &'a [u8], big_endian: bool, text: &dyn Fn(&[u8]) -> String) -> Result<Vec<LongLabels<'a>>, SavError> {
    let mut b = Cursor::new(body, big_endian);
    let mut out = Vec::new();
    while b.remaining() > 0 {
        let len = b.count("long string label variable name")?;
        let name = text(b.take(len)?);
        let _width = b.count("long string label width")?;
        let n = b.count("long string label count")?;
        let mut labels = Vec::with_capacity(n.min(b.remaining() / 8));
        for _ in 0..n {
            let len = b.count("long string label value")?;
            let value = b.take(len)?;
            let len = b.count("long string label")?;
            labels.push((value, b.take(len)?));
        }
        out.push((name, labels));
    }
    Ok(out)
}

pub(crate) fn decode(encoding: &'static Encoding, bytes: &[u8]) -> String {
    let (text, _) = encoding.decode_without_bom_handling(bytes);
    text.trim_end_matches([' ', '\0']).to_string()
}

pub(crate) fn read(bytes: &[u8], flavour: Flavour) -> Result<Dictionary, SavError> {
    let mut c = Cursor::new(bytes, false);
    c.take(4)?;
    let product = c.take(60)?;
    // The layout code is 2 or 3 in the file's byte order, which is how the order is detected.
    let layout = c.array::<4>()?;
    c.big_endian = match (i32::from_le_bytes(layout), i32::from_be_bytes(layout)) {
        (2 | 3, _) => false,
        (_, 2 | 3) => true,
        _ => return Err(SavError::Invalid { offset: 64, what: "layout code" }),
    };
    let nominal_case_size = c.i32()?;
    let compression = match (c.i32()?, flavour) {
        (0, Flavour::Sav) => Compression::None,
        (1, Flavour::Sav) => Compression::Bytecode,
        (2, Flavour::Zsav) => Compression::Zlib,
        _ => return Err(c.invalid("compression code")),
    };
    let weight_segment = c.i32()?;
    let n_cases = usize::try_from(c.i32()?).ok();
    let bias = c.f64()?;
    let date = c.take(9)?;
    let time = c.take(8)?;
    let file_label = c.take(64)?;
    c.take(3)?;

    let mut warnings = Vec::new();
    let mut vars: Vec<RawVar> = Vec::new();
    let mut segments = 0usize;
    // Segments a string variable still has to cover with continuation records.
    let mut continuations = 0usize;
    let mut label_sets: Vec<RawLabels> = Vec::new();
    let mut documents: Vec<&[u8]> = Vec::new();
    let mut code_page = None;
    let mut sysmis = -f64::MAX;
    let mut measures: Option<Vec<i32>> = None;
    let mut long_names: Option<&[u8]> = None;
    let mut encoding_name: Option<&[u8]> = None;
    let mut long_string_labels: Vec<&[u8]> = Vec::new();

    loop {
        let record_offset = c.pos();
        match c.i32()? {
            2 => {
                let width = c.i32()?;
                let has_label = c.i32()?;
                let n_missing = c.i32()?;
                let print = c.i32()?;
                let _write = c.i32()?;
                let name = c.take(8)?;
                let label = match has_label {
                    0 => None,
                    1 => {
                        let len = c.count("variable label length")?;
                        let padded = len.checked_next_multiple_of(4).ok_or(c.invalid("variable label length"))?;
                        c.take(padded)?.get(..len)
                    }
                    _ => return Err(c.invalid("variable label flag")),
                };
                let n_values = match n_missing {
                    0..=3 => n_missing.unsigned_abs() as usize,
                    -2 => 2,
                    -3 => 3,
                    _ => return Err(c.invalid("missing value count")),
                };
                let mut values = Vec::with_capacity(n_values);
                for _ in 0..n_values {
                    values.push(c.array::<8>()?);
                }
                if width == -1 {
                    // A continuation of the previous string variable's storage.
                    continuations = continuations.checked_sub(1).ok_or(SavError::Invalid { offset: record_offset, what: "continuation record" })?;
                    segments += 1;
                    continue;
                }
                if continuations > 0 {
                    return Err(SavError::Invalid { offset: record_offset, what: "string variable (missing continuation records)" });
                }
                let width =
                    usize::try_from(width).ok().filter(|w| *w <= 255).ok_or(SavError::Invalid { offset: record_offset, what: "variable width" })?;
                let missing = match (n_missing, values.as_slice()) {
                    (-2, [low, high]) => RawMissing::Range { low: *low, high: *high, discrete: None },
                    (-3, [low, high, d]) => RawMissing::Range { low: *low, high: *high, discrete: Some(*d) },
                    (_, []) => RawMissing::None,
                    _ => RawMissing::Discrete(values),
                };
                vars.push(RawVar { width, name, label, print, missing, segment: segments });
                segments += 1;
                continuations = width.div_ceil(8).saturating_sub(1);
            }
            3 => {
                let n = c.count("value label count")?;
                let mut labels = Vec::with_capacity(n.min(c.remaining() / 16));
                for _ in 0..n {
                    let value = c.array::<8>()?;
                    let [len] = c.array::<1>()?;
                    let len = usize::from(len);
                    // The length byte and the label together fill a multiple of 8 bytes.
                    let label = c.take((len + 1).next_multiple_of(8) - 1)?;
                    labels.push((value, label.get(..len).unwrap_or(label)));
                }
                if c.i32()? != 4 {
                    return Err(c.invalid("value labels without a variable list (record 4)"));
                }
                let n = c.count("value label variable count")?;
                let mut indexes = Vec::with_capacity(n.min(c.remaining() / 4));
                for _ in 0..n {
                    indexes.push(c.count("value label variable index")?);
                }
                label_sets.push(RawLabels { labels, segments: indexes });
            }
            6 => {
                let n = c.count("document line count")?;
                let text = c.take(n.checked_mul(80).ok_or(c.invalid("document line count"))?)?;
                documents.extend(text.chunks(80));
            }
            7 => {
                let subtype = c.i32()?;
                let size = c.count("extension record size")?;
                let count = c.count("extension record count")?;
                let body = c.take(size.checked_mul(count).ok_or(c.invalid("extension record size"))?)?;
                let mut b = Cursor::new(body, c.big_endian);
                match subtype {
                    // Integer info: the eighth integer is the character code page.
                    3 => {
                        b.take(28)?;
                        code_page = Some(b.i32()?);
                    }
                    // Floating-point info: the system-missing value comes first.
                    4 => sysmis = b.f64()?,
                    // Variable display parameters: measure, width, alignment (or no width).
                    11 => {
                        let per_var = match (count, vars.len()) {
                            (n, v) if v > 0 && n == v * 3 => 3,
                            (n, v) if v > 0 && n == v * 2 => 2,
                            _ => {
                                warnings.push("ignored variable display parameters that don't match the variables".into());
                                continue;
                            }
                        };
                        let mut list = Vec::with_capacity(vars.len());
                        for _ in 0..vars.len() {
                            list.push(b.i32()?);
                            b.take((per_var - 1) * 4)?;
                        }
                        measures = Some(list);
                    }
                    13 => long_names = Some(body),
                    14 => warnings.push("very long string variables (over 255 bytes) are read as their 255-byte parts".into()),
                    20 => encoding_name = Some(body),
                    21 => long_string_labels.push(body),
                    22 => warnings.push("missing values of long string variables are not read yet".into()),
                    _ => {}
                }
            }
            999 => {
                c.i32()?;
                break;
            }
            _ => return Err(SavError::Invalid { offset: record_offset, what: "record type" }),
        }
    }
    if continuations > 0 {
        return Err(c.invalid("string variable (missing continuation records)"));
    }
    if usize::try_from(nominal_case_size).is_ok_and(|n| n != segments) {
        warnings.push(format!("the header says cases have {nominal_case_size} segments, the dictionary {segments}"));
    }

    let encoding = encoding(encoding_name, code_page, &mut warnings);
    let text = |b: &[u8]| decode(encoding, b);
    let long_names: Vec<(String, String)> = long_names
        .map(|b| {
            text(b).split('\t').filter_map(|pair| pair.split_once('=')).map(|(s, l)| (s.to_string(), l.trim_end_matches('\0').to_string())).collect()
        })
        .unwrap_or_default();

    let mut long_labels = Vec::new();
    for body in long_string_labels {
        long_labels.extend(long_string_value_labels(body, c.big_endian, &text)?);
    }

    let mut dataset = Dataset::new();
    let mut layout = Vec::with_capacity(vars.len());
    for (i, raw) in vars.iter().enumerate() {
        let short = text(raw.name);
        let name = long_names.iter().find(|(s, _)| s.eq_ignore_ascii_case(&short)).map_or(short, |(_, l)| l.clone());
        let width = u16::try_from(raw.width).unwrap_or(u16::MAX);
        let mut var = if raw.width == 0 { Variable::numeric(name) } else { Variable::string(name, width) };
        var.label = raw.label.map(text);
        var.format = format(raw.print);
        let num = |b: [u8; 8]| c.f64_from(b);
        let value = |b: [u8; 8]| if var.var_type == VarType::Numeric { Value::Num(num(b)) } else { Value::Str(text(b.as_slice())) };
        var.missing = match &raw.missing {
            RawMissing::None => MissingValues::None,
            RawMissing::Discrete(list) => MissingValues::Discrete(list.iter().map(|b| value(*b)).collect()),
            RawMissing::Range { low, high, discrete } => MissingValues::Range { low: num(*low), high: num(*high), discrete: discrete.map(num) },
        };
        if let Some(m) = measures.as_ref().and_then(|m| m.get(i)) {
            match m {
                1 => var.measure = Measure::Nominal,
                2 => var.measure = Measure::Ordinal,
                3 => var.measure = Measure::Scale,
                _ => {}
            }
        }
        for set in label_sets.iter().filter(|s| s.segments.contains(&(raw.segment + 1))) {
            for (v, label) in &set.labels {
                var.value_labels.push((value(*v), text(label)));
            }
        }
        for (owner, labels) in &long_labels {
            if owner.eq_ignore_ascii_case(&var.name) || owner.eq_ignore_ascii_case(&text(raw.name)) {
                var.value_labels.extend(labels.iter().map(|(v, l)| (Value::Str(text(v)), text(l))));
            }
        }
        layout.push((raw.width, raw.width.div_ceil(8).max(1)));
        dataset.add_variable(var)?;
    }
    dataset.label = Some(text(file_label)).filter(|l| !l.is_empty());
    dataset.documents = documents.iter().copied().map(text).collect();
    dataset.weight = usize::try_from(weight_segment).ok().filter(|s| *s > 0).and_then(|s| vars.iter().position(|v| v.segment + 1 == s));

    Ok(Dictionary {
        dataset,
        layout,
        data_offset: c.pos(),
        big_endian: c.big_endian,
        compression,
        bias,
        sysmis,
        n_cases,
        encoding,
        product: text(product),
        created: format!("{} {}", text(date), text(time)),
        warnings,
    })
}
