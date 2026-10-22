//! Writing format 118 (Stata 14 and later), little-endian, UTF-8: the format StataCorp's
//! description documents as current. Each numeric variable takes the smallest storage type
//! that holds its values exactly; strings up to 2045 bytes are `str#`, longer ones `strL`.

use mksigma_core::{Dataset, FormatKind, MissingValues, Value, VarType, Variable};
use mksigma_i18n::tr;

use crate::{DtaError, STATA_EPOCH};

/// What a written file couldn't keep, for the user to see.
#[derive(Debug)]
pub struct Written {
    pub bytes: Vec<u8>,
    pub warnings: Vec<String>,
}

/// The longest `str#`; longer strings are written as `strL`.
const STRF: usize = 2045;
/// Stata's limits on names and labels, in characters.
const NAME: usize = 32;
const LABEL: usize = 80;

/// Words Stata reserves: no variable may take them as a name.
const RESERVED: [&str; 26] = [
    "_all", "_b", "byte", "_coef", "_cons", "double", "float", "if", "in", "int", "long", "_n", "_N", "_pi", "_pred", "_rc", "_skip", "strL",
    "using", "with", "str", "e", "_se", "_weight", "fweight", "aweight",
];

/// Storage chosen for a variable.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Store {
    Byte,
    Int,
    Long,
    Double,
    Str(usize),
    StrL,
}

impl Store {
    fn code(self) -> u16 {
        match self {
            Store::Byte => 65530,
            Store::Int => 65529,
            Store::Long => 65528,
            Store::Double => 65526,
            Store::StrL => 32768,
            Store::Str(w) => w as u16,
        }
    }
}

/// How a numeric variable's values change on the way: Stata's date units, or none.
#[derive(Clone, Copy, PartialEq)]
enum Unit {
    Plain,
    Days,
    Milliseconds,
    Months,
    Quarters,
}

/// The year and month (0–11) of seconds from 14 October 1582; `None` beyond a million years
/// either side of 1960.
fn year_month(x: f64) -> Option<(i64, i64)> {
    let days = (x - STATA_EPOCH) / 86_400.0;
    if days.is_nan() || days.abs() > crate::MAX_MONTHS * 31.0 {
        return None;
    }
    // Days from 14 October 1582 to 1 January 1970.
    let (y, m, _) = crate::civil((x / 86_400.0).floor() as i64 - 141_428);
    Some((y, i64::from(m) - 1))
}

/// A value in the variable's Stata unit; `None` for a month or quarter out of range.
fn convert(x: f64, unit: Unit) -> Option<f64> {
    match unit {
        Unit::Plain => Some(x),
        Unit::Days => Some(((x - STATA_EPOCH) / 86_400.0).floor()),
        Unit::Milliseconds => Some(((x - STATA_EPOCH) * 1000.0).round()),
        Unit::Months | Unit::Quarters => {
            let (y, m) = year_month(x)?;
            let months = (y - 1960) * 12 + m;
            Some(if unit == Unit::Months { months as f64 } else { months.div_euclid(3) as f64 })
        }
    }
}

/// A user-missing value's place among `.a`, `.b`, `.c`, if it is one.
fn user_missing(var: &Variable, x: f64) -> Option<u8> {
    match &var.missing {
        MissingValues::None => None,
        MissingValues::Discrete(values) => values.iter().position(|v| *v == Value::Num(x)).map(|k| k as u8),
        MissingValues::Range { low, high, discrete } => {
            if x >= *low && x <= *high {
                Some(0)
            } else if *discrete == Some(x) {
                Some(1)
            } else {
                None
            }
        }
    }
}

/// Little-endian bytes, with the file positions the map records.
struct Out {
    bytes: Vec<u8>,
    map: Vec<u64>,
}

impl Out {
    fn tag(&mut self, tag: &str) {
        self.bytes.extend_from_slice(tag.as_bytes());
    }
    /// Notes where the section that starts here begins, then writes its tag.
    fn section(&mut self, tag: &str) {
        self.map.push(self.bytes.len() as u64);
        self.tag(tag);
    }
    fn fixed(&mut self, text: &str, width: usize) {
        let mut b = text.as_bytes().to_vec();
        b.truncate(width - 1);
        b.resize(width, 0);
        self.bytes.extend(b);
    }
}

/// Text cut to `chars` characters.
fn cut(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}

/// A name Stata takes: letters, digits and `_`, a letter or `_` first, 32 characters at most,
/// not reserved, unique.
fn stata_name(name: &str, taken: &[String]) -> String {
    let mut base: String = name.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if !base.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') {
        base.insert(0, '_');
    }
    let reserved = |n: &str| {
        RESERVED.contains(&n) || (n.starts_with("str") && n.get(3..).is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit())))
    };
    let base = cut(&base, NAME);
    let base = if reserved(&base) { cut(&format!("{base}_"), NAME) } else { base };
    let mut candidate = base.clone();
    let mut k = 2;
    while taken.contains(&candidate) {
        let suffix = format!("_{k}");
        candidate = format!("{}{suffix}", cut(&base, NAME - suffix.chars().count()));
        k += 1;
    }
    candidate
}

/// `K` as format 118 records it: up to 32,767 variables.
pub(crate) fn variable_count(k: usize) -> Result<u16, DtaError> {
    u16::try_from(k).ok().filter(|k| *k <= 32_767).ok_or(DtaError::TooManyVariables(k))
}

/// The dataset as a Stata file (format 118). `timestamp` is `dd Mon yyyy hh:mm`, or empty.
pub fn write(ds: &Dataset, timestamp: &str) -> Result<Written, DtaError> {
    let mut warnings = Vec::new();
    let vars = ds.variables();
    let k = variable_count(vars.len())?;
    let n = ds.n_cases();
    let column = |j: usize| ds.column(j).unwrap_or(&[]);

    // Names, storage, formats and units.
    let mut names: Vec<String> = Vec::with_capacity(vars.len());
    let mut stores = Vec::with_capacity(vars.len());
    let mut formats = Vec::with_capacity(vars.len());
    let mut units = Vec::with_capacity(vars.len());
    let mut dropped_user_missing = false;
    for (j, var) in vars.iter().enumerate() {
        let name = stata_name(&var.name, &names);
        if name != var.name {
            warnings.push(tr!("dta-warn-renamed", from = var.name.as_str(), to = name.as_str()));
        }
        names.push(name);
        match var.var_type {
            VarType::String(_) => {
                let longest = column(j).iter().map(|v| if let Value::Str(s) = v { s.len() } else { 0 }).max().unwrap_or(0).max(1);
                let store = if longest <= STRF { Store::Str(longest) } else { Store::StrL };
                stores.push(store);
                formats.push(format!("%{}s", longest.min(STRF)));
                units.push(Unit::Plain);
            }
            VarType::Numeric => {
                let unit = match var.format.kind {
                    FormatKind::Date | FormatKind::ADate | FormatKind::EDate | FormatKind::SDate | FormatKind::JDate | FormatKind::WkYr => Unit::Days,
                    FormatKind::DateTime | FormatKind::YmdHms => Unit::Milliseconds,
                    FormatKind::MoYr => Unit::Months,
                    FormatKind::QYr => Unit::Quarters,
                    _ => Unit::Plain,
                };
                let values: Vec<f64> =
                    column(j).iter().filter_map(Value::as_num).filter(|x| user_missing(var, *x).is_none()).filter_map(|x| convert(x, unit)).collect();
                dropped_user_missing |= column(j).iter().filter_map(Value::as_num).any(|x| user_missing(var, x).is_some());
                let whole = values.iter().all(|x| x.fract() == 0.0);
                let within = |lo: f64, hi: f64| whole && values.iter().all(|x| *x >= lo && *x <= hi);
                let store = if unit == Unit::Milliseconds {
                    Store::Double
                } else if within(-127.0, 100.0) {
                    Store::Byte
                } else if within(-32_767.0, 32_740.0) {
                    Store::Int
                } else if within(-2_147_483_647.0, 2_147_483_620.0) {
                    Store::Long
                } else {
                    Store::Double
                };
                stores.push(store);
                let f = var.format;
                formats.push(match (unit, f.kind) {
                    (Unit::Days, _) => "%td".to_string(),
                    (Unit::Milliseconds, _) => "%tc".to_string(),
                    (Unit::Months, _) => "%tm".to_string(),
                    (Unit::Quarters, _) => "%tq".to_string(),
                    (_, FormatKind::Comma) => format!("%{}.{}fc", f.width, f.decimals),
                    (_, FormatKind::E) => format!("%{}.{}e", f.width, f.decimals),
                    _ if f.decimals == 0 && whole => format!("%{}.0f", f.width.max(1)),
                    _ => format!("%{}.{}f", f.width.max(u16::from(f.decimals) + 2), f.decimals),
                });
                units.push(unit);
            }
        }
    }
    if dropped_user_missing {
        warnings.push(tr!("dta-warn-user-missing"));
    }

    // Value labels: integer values of numeric variables, a label named after its variable.
    let mut tables: Vec<(String, Vec<(i32, String)>)> = Vec::new();
    let mut label_names = vec![String::new(); vars.len()];
    let mut dropped_labels = false;
    for (j, var) in vars.iter().enumerate() {
        if var.value_labels.is_empty() {
            continue;
        }
        if matches!(var.var_type, VarType::String(_)) {
            dropped_labels = true;
            continue;
        }
        let mut entries: Vec<(i32, String)> = Vec::new();
        for (value, label) in &var.value_labels {
            match value.as_num().filter(|x| x.fract() == 0.0 && *x >= f64::from(i32::MIN) && *x <= f64::from(i32::MAX)) {
                Some(x) => entries.push((x as i32, label.clone())),
                None => dropped_labels = true,
            }
        }
        entries.sort_by_key(|(v, _)| *v);
        entries.dedup_by_key(|(v, _)| *v);
        if !entries.is_empty() {
            let name = names.get(j).cloned().unwrap_or_default();
            label_names[j] = name.clone();
            tables.push((name, entries));
        }
    }
    if dropped_labels {
        warnings.push(tr!("dta-warn-labels-dropped"));
    }
    let mut cut_labels = false;

    let mut o = Out { bytes: Vec::with_capacity(1024 + n * stores.len() * 8), map: Vec::with_capacity(14) };
    o.section("<stata_dta>");
    o.tag("<header><release>118</release><byteorder>LSF</byteorder><K>");
    o.bytes.extend(k.to_le_bytes());
    o.tag("</K><N>");
    o.bytes.extend((n as u64).to_le_bytes());
    o.tag("</N><label>");
    let label = ds.label.as_deref().map(|l| cut(l, LABEL)).unwrap_or_default();
    cut_labels |= ds.label.as_deref().is_some_and(|l| l.chars().count() > LABEL);
    o.bytes.extend((label.len() as u16).to_le_bytes());
    o.tag(&label);
    o.tag("</label><timestamp>");
    let stamp = if timestamp.len() == 17 { timestamp } else { "" };
    o.bytes.push(stamp.len() as u8);
    o.tag(stamp);
    o.tag("</timestamp></header>");
    o.section("<map>");
    let map_at = o.bytes.len();
    o.bytes.extend([0u8; 14 * 8]);
    o.tag("</map>");
    o.section("<variable_types>");
    for s in &stores {
        o.bytes.extend(s.code().to_le_bytes());
    }
    o.tag("</variable_types>");
    o.section("<varnames>");
    for name in &names {
        o.fixed(name, 129);
    }
    o.tag("</varnames>");
    o.section("<sortlist>");
    o.bytes.extend(std::iter::repeat_n(0u8, 2 * (vars.len() + 1)));
    o.tag("</sortlist>");
    o.section("<formats>");
    for f in &formats {
        o.fixed(f, 57);
    }
    o.tag("</formats>");
    o.section("<value_label_names>");
    for name in &label_names {
        o.fixed(name, 129);
    }
    o.tag("</value_label_names>");
    o.section("<variable_labels>");
    for var in vars {
        let text = var.label.as_deref().unwrap_or("");
        cut_labels |= text.chars().count() > LABEL;
        o.fixed(&cut(text, LABEL), 321);
    }
    o.tag("</variable_labels>");
    o.section("<characteristics>");
    o.tag("</characteristics>");
    o.section("<data>");
    let mut strls: Vec<(u16, u64, &str)> = Vec::new();
    let mut out_of_range = false;
    for case in 0..n {
        for (j, (var, store)) in vars.iter().zip(&stores).enumerate() {
            let value = column(j).get(case).unwrap_or(&Value::SysMis);
            match *store {
                Store::Str(w) => {
                    let text = if let Value::Str(s) = value { s.as_str() } else { "" };
                    let mut b = text.as_bytes().to_vec();
                    b.resize(w, 0);
                    o.bytes.extend(b);
                }
                Store::StrL => {
                    let text = if let Value::Str(s) = value { s.as_str() } else { "" };
                    if text.is_empty() {
                        o.bytes.extend(0u64.to_le_bytes());
                    } else {
                        let (v, obs) = (j as u16 + 1, case as u64 + 1);
                        o.bytes.extend((u64::from(v) | (obs << 16)).to_le_bytes());
                        strls.push((v, obs, text));
                    }
                }
                _ => {
                    // `.` for system-missing, `.a`, `.b`… for user-missing values.
                    let converted = value.as_num().and_then(|x| convert(x, units.get(j).copied().unwrap_or(Unit::Plain)));
                    let missing = match value {
                        Value::Num(x) => user_missing(var, *x).map(|k| k + 1).or_else(|| {
                            // A date out of range is written as `.`.
                            out_of_range |= converted.is_none();
                            converted.is_none().then_some(0)
                        }),
                        _ => Some(0),
                    };
                    let x = converted.unwrap_or(0.0);
                    match (*store, missing) {
                        (Store::Byte, None) => o.bytes.push(x as i8 as u8),
                        (Store::Byte, Some(m)) => o.bytes.push(101 + m),
                        (Store::Int, None) => o.bytes.extend((x as i16).to_le_bytes()),
                        (Store::Int, Some(m)) => o.bytes.extend((32_741 + i16::from(m)).to_le_bytes()),
                        (Store::Long, None) => o.bytes.extend((x as i32).to_le_bytes()),
                        (Store::Long, Some(m)) => o.bytes.extend((2_147_483_621 + i32::from(m)).to_le_bytes()),
                        (_, None) => o.bytes.extend(x.to_le_bytes()),
                        (_, Some(m)) => o.bytes.extend((0x7fe0_0000_0000_0000u64 + (u64::from(m) << 40)).to_le_bytes()),
                    }
                }
            }
        }
    }
    o.tag("</data>");
    if out_of_range {
        warnings.push(tr!("dta-warn-date-range-written"));
    }
    o.section("<strls>");
    for (v, obs, text) in strls {
        o.tag("GSO");
        o.bytes.extend(u32::from(v).to_le_bytes());
        o.bytes.extend(obs.to_le_bytes());
        o.bytes.push(130);
        o.bytes.extend((text.len() as u32 + 1).to_le_bytes());
        o.tag(text);
        o.bytes.push(0);
    }
    o.tag("</strls>");
    o.section("<value_labels>");
    for (name, entries) in &tables {
        let mut text: Vec<u8> = Vec::new();
        let mut offsets = Vec::with_capacity(entries.len());
        for (_, label) in entries {
            offsets.push(text.len() as u32);
            cut_labels |= label.len() > 32_000;
            let mut end = label.len().min(32_000);
            while !label.is_char_boundary(end) {
                end -= 1;
            }
            text.extend(label.get(..end).unwrap_or("").as_bytes());
            text.push(0);
        }
        let mut table: Vec<u8> = Vec::new();
        table.extend((entries.len() as u32).to_le_bytes());
        table.extend((text.len() as u32).to_le_bytes());
        for off in offsets {
            table.extend(off.to_le_bytes());
        }
        for (v, _) in entries {
            table.extend(v.to_le_bytes());
        }
        table.extend(text);
        o.tag("<lbl>");
        o.bytes.extend((table.len() as u32).to_le_bytes());
        o.fixed(name, 129);
        o.bytes.extend([0u8; 3]);
        o.bytes.extend(table);
        o.tag("</lbl>");
    }
    o.tag("</value_labels>");
    o.section("</stata_dta>");
    o.map.push(o.bytes.len() as u64);
    // The map: where each section starts, then the end of the file.
    for (k, position) in o.map.iter().enumerate() {
        if let Some(slot) = o.bytes.get_mut(map_at + 8 * k..map_at + 8 * k + 8) {
            slot.copy_from_slice(&position.to_le_bytes());
        }
    }
    if cut_labels {
        warnings.push(tr!("dta-warn-labels-cut", n = LABEL));
    }
    Ok(Written { bytes: o.bytes, warnings })
}
