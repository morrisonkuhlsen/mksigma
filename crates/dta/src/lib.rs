//! Stata data files (`.dta`). Clean-room: built from StataCorp's published description of the
//! format (Stata's `help dta`, `dta_117`, `dta_115`, `dta_114`, `dta_113`), never from Stata's
//! binaries or GPL code.
//!
//! Reads formats 113 to 115 (Stata 8 to 12: a binary header) and 117 to 119 (Stata 13 and later:
//! sections between tags such as `<varnames>`), in either byte order. Numbers keep their value
//! (a `float` as the decimal Stata shows); `.` and the extended missing values `.a` to `.z` read
//! as system-missing. Strings of both kinds (`str#` and `strL`) read as string variables; value
//! labels, variable labels and the dataset label come along. Date formats become SPSS-style
//! dates, counted in seconds from 14 October 1582.
//!
//! Every count and length in the file is checked against the bytes that remain before anything
//! is allocated for it: a damaged or hostile file is an error, never a crash.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use mksigma_core::{Dataset, DatasetError, Format, FormatKind, Measure, Value, VarType, Variable, is_valid_name};
use mksigma_i18n::tr;
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum DtaError {
    #[error("{}", tr!("err-not-dta"))]
    NotDta,
    #[error("{}", tr!("err-dta-truncated", offset = *offset))]
    Truncated { offset: usize },
    /// `what` is the part of the file, as the format names it (`<varnames>`, `value label`).
    #[error("{}", tr!("err-dta-invalid", what = *what, offset = *offset))]
    Invalid { offset: usize, what: &'static str },
    #[error("{}", tr!("err-dta-release", release = *.0))]
    Release(u16),
    #[error(transparent)]
    Dataset(#[from] DatasetError),
}

/// A Stata file read into a dataset, with what its header says.
#[derive(Debug, Clone)]
pub struct DtaFile {
    pub dataset: Dataset,
    /// The format's number: 113 to 115, or 117 to 119.
    pub release: u16,
    /// When the file was written, as Stata wrote it (`10 Mar 2017 14:23`), if it says.
    pub timestamp: Option<String>,
    /// Things read leniently or changed, for the user to see.
    pub warnings: Vec<String>,
}

/// Whether `bytes` start like a Stata file.
pub fn is_dta(bytes: &[u8]) -> bool {
    bytes.starts_with(b"<stata_dta>") || (matches!(bytes.first(), Some(113..=115)) && matches!(bytes.get(1), Some(1 | 2)))
}

/// Seconds from SPSS's day 0 (14 October 1582) to Stata's (1 January 1960).
const STATA_EPOCH: f64 = 137_775.0 * 86_400.0;

/// The longest string variable MK Sigma keeps.
const MAX_STRING: usize = 32_767;

/// A storage type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// `str#`: a fixed field of this many bytes.
    Str(usize),
    /// `strL`: a reference into the `<strls>` section.
    StrL,
    Byte,
    Int,
    Long,
    Float,
    Double,
}

impl Kind {
    fn bytes(self) -> usize {
        match self {
            Kind::Str(w) => w,
            Kind::StrL | Kind::Double => 8,
            Kind::Byte => 1,
            Kind::Int => 2,
            Kind::Long | Kind::Float => 4,
        }
    }
}

/// Reads through the file, every read bounded by what is left.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
    big: bool,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], DtaError> {
        let end = self.at.checked_add(n).ok_or(DtaError::Truncated { offset: self.at })?;
        let out = self.bytes.get(self.at..end).ok_or(DtaError::Truncated { offset: self.at })?;
        self.at = end;
        Ok(out)
    }

    fn left(&self) -> usize {
        self.bytes.len().saturating_sub(self.at)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], DtaError> {
        let mut out = [0; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, DtaError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, DtaError> {
        let b = self.array()?;
        Ok(if self.big { u16::from_be_bytes(b) } else { u16::from_le_bytes(b) })
    }

    fn u32(&mut self) -> Result<u32, DtaError> {
        let b = self.array()?;
        Ok(if self.big { u32::from_be_bytes(b) } else { u32::from_le_bytes(b) })
    }

    fn u64(&mut self) -> Result<u64, DtaError> {
        let b = self.array()?;
        Ok(if self.big { u64::from_be_bytes(b) } else { u64::from_le_bytes(b) })
    }

    /// An unsigned count, as a size that fits in memory.
    fn size(n: u64, at: usize) -> Result<usize, DtaError> {
        usize::try_from(n).map_err(|_| DtaError::Invalid { offset: at, what: "count" })
    }

    /// The tag `tag`, or an error naming it.
    fn expect(&mut self, tag: &'static str) -> Result<(), DtaError> {
        let at = self.at;
        if self.take(tag.len()).ok() == Some(tag.as_bytes()) { Ok(()) } else { Err(DtaError::Invalid { offset: at, what: tag }) }
    }

    /// Whether `tag` comes next (it is read if so).
    fn next_is(&mut self, tag: &str) -> bool {
        let end = self.at.saturating_add(tag.len());
        if self.bytes.get(self.at..end) == Some(tag.as_bytes()) {
            self.at = end;
            true
        } else {
            false
        }
    }
}

/// Text in a fixed field: up to its first `\0`.
fn field(bytes: &[u8]) -> &[u8] {
    bytes.iter().position(|b| *b == 0).map_or(bytes, |end| bytes.get(..end).unwrap_or(bytes))
}

/// The layout of a release: field widths that changed over time.
struct Layout {
    release: u16,
    /// Before 118, text is in the computer's code page (Windows-1252, as Stata wrote it); since,
    /// UTF-8.
    utf8: bool,
    name: usize,
    format: usize,
    label: usize,
}

impl Layout {
    fn of(release: u16) -> Result<Layout, DtaError> {
        let (utf8, name, format, label) = match release {
            113 => (false, 33, 12, 81),
            114 | 115 | 117 => (false, 33, 49, 81),
            118 | 119 => (true, 129, 57, 321),
            other => return Err(DtaError::Release(other)),
        };
        Ok(Layout { release, utf8, name, format, label })
    }

    fn text(&self, bytes: &[u8], warnings: &mut Vec<String>) -> String {
        let bytes = field(bytes);
        if self.utf8 {
            match std::str::from_utf8(bytes) {
                Ok(s) => s.to_string(),
                Err(_) => {
                    warnings.push(tr!("dta-warn-bad-utf8"));
                    String::from_utf8_lossy(bytes).into_owned()
                }
            }
        } else {
            encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes).0.into_owned()
        }
    }

    /// `K` fixed fields of `width` bytes, as text.
    fn texts(&self, c: &mut Cursor, k: usize, width: usize, warnings: &mut Vec<String>) -> Result<Vec<String>, DtaError> {
        let all = c.take(k.checked_mul(width).ok_or(DtaError::Truncated { offset: c.at })?)?;
        Ok(all.chunks(width.max(1)).map(|chunk| self.text(chunk, warnings)).collect())
    }
}

/// What the dictionary sections hold, before the data.
struct Dictionary {
    kinds: Vec<Kind>,
    names: Vec<String>,
    formats: Vec<String>,
    label_names: Vec<String>,
    labels: Vec<String>,
}

pub fn read(bytes: &[u8]) -> Result<DtaFile, DtaError> {
    if bytes.starts_with(b"<stata_dta>") {
        read_tagged(bytes)
    } else if is_dta(bytes) {
        read_binary(bytes)
    } else if bytes.first().is_some_and(|b| (102..=112).contains(b)) {
        Err(DtaError::Release(bytes.first().copied().map_or(0, u16::from)))
    } else {
        Err(DtaError::NotDta)
    }
}

/// Formats 117 to 119: sections between tags.
fn read_tagged(bytes: &[u8]) -> Result<DtaFile, DtaError> {
    let mut c = Cursor { bytes, at: 0, big: false };
    let mut warnings = Vec::new();
    c.expect("<stata_dta><header><release>")?;
    let at = c.at;
    let release: u16 =
        std::str::from_utf8(c.take(3)?).ok().and_then(|s| s.parse().ok()).ok_or(DtaError::Invalid { offset: at, what: "<release>" })?;
    let layout = Layout::of(release)?;
    c.expect("</release><byteorder>")?;
    c.big = match c.take(3)? {
        b"MSF" => true,
        b"LSF" => false,
        _ => return Err(DtaError::Invalid { offset: c.at - 3, what: "<byteorder>" }),
    };
    c.expect("</byteorder><K>")?;
    let k = if release == 119 { c.u32()? as usize } else { usize::from(c.u16()?) };
    c.expect("</K><N>")?;
    let at = c.at;
    let n = Cursor::size(if release == 117 { u64::from(c.u32()?) } else { c.u64()? }, at)?;
    c.expect("</N><label>")?;
    let length = if release == 117 { usize::from(c.u8()?) } else { usize::from(c.u16()?) };
    let label = layout.text(c.take(length)?, &mut warnings);
    c.expect("</label><timestamp>")?;
    let length = usize::from(c.u8()?);
    let timestamp = layout.text(c.take(length)?, &mut warnings);
    c.expect("</timestamp></header><map>")?;
    c.take(14 * 8)?;
    c.expect("</map><variable_types>")?;
    let mut kinds = Vec::with_capacity(k.min(c.left() / 2));
    for _ in 0..k {
        let at = c.at;
        kinds.push(match c.u16()? {
            w @ 1..=2045 => Kind::Str(usize::from(w)),
            32768 => Kind::StrL,
            65526 => Kind::Double,
            65527 => Kind::Float,
            65528 => Kind::Long,
            65529 => Kind::Int,
            65530 => Kind::Byte,
            _ => return Err(DtaError::Invalid { offset: at, what: "<variable_types>" }),
        });
    }
    c.expect("</variable_types><varnames>")?;
    let names = layout.texts(&mut c, k, layout.name, &mut warnings)?;
    c.expect("</varnames><sortlist>")?;
    let entry = if release == 119 { 4 } else { 2 };
    c.take(k.checked_add(1).and_then(|e| e.checked_mul(entry)).ok_or(DtaError::Truncated { offset: c.at })?)?;
    c.expect("</sortlist><formats>")?;
    let formats = layout.texts(&mut c, k, layout.format, &mut warnings)?;
    c.expect("</formats><value_label_names>")?;
    let label_names = layout.texts(&mut c, k, layout.name, &mut warnings)?;
    c.expect("</value_label_names><variable_labels>")?;
    let labels = layout.texts(&mut c, k, layout.label, &mut warnings)?;
    c.expect("</variable_labels><characteristics>")?;
    while c.next_is("<ch>") {
        let length = c.u32()? as usize;
        c.take(length)?;
        c.expect("</ch>")?;
    }
    c.expect("</characteristics><data>")?;
    let dictionary = Dictionary { kinds, names, formats, label_names, labels };
    let (mut rows, refs) = read_data(&mut c, &layout, &dictionary.kinds, n, &mut warnings)?;
    c.expect("</data><strls>")?;
    let mut strls: HashMap<(u64, u64), String> = HashMap::new();
    while c.next_is("GSO") {
        let v = u64::from(c.u32()?);
        let o = if release == 117 { u64::from(c.u32()?) } else { c.u64()? };
        let kind = c.u8()?;
        let length = c.u32()? as usize;
        let content = c.take(length)?;
        // An ASCII one (130) ends in \0; a binary one (129) is as it is, text if it can be.
        let content = if kind == 130 { field(content) } else { content };
        let text = if layout.utf8 || kind == 129 {
            match std::str::from_utf8(content) {
                Ok(s) => s.to_string(),
                Err(_) => {
                    warnings.push(tr!("dta-warn-bad-utf8"));
                    String::from_utf8_lossy(content).into_owned()
                }
            }
        } else {
            layout.text(content, &mut warnings)
        };
        strls.insert((v, o), text);
    }
    c.expect("</strls><value_labels>")?;
    let mut tables = HashMap::new();
    while c.next_is("<lbl>") {
        let length = c.u32()? as usize;
        let name = layout.text(c.take(layout.name)?, &mut warnings);
        c.take(3)?;
        let table = c.take(length)?;
        tables.insert(name, value_labels(table, c.big, &layout, c.at - length, &mut warnings)?);
        c.expect("</lbl>")?;
    }
    c.expect("</value_labels></stata_dta>")?;
    // Each strL takes the string of its (v, o), which comes before it.
    for (case, row) in refs.iter().enumerate() {
        for &(column, v, o) in row {
            let text = if (v, o) == (0, 0) {
                String::new()
            } else {
                strls.get(&(v, o)).cloned().ok_or(DtaError::Invalid { offset: c.at, what: "<strls>" })?
            };
            if let Some(cell) = rows.get_mut(case).and_then(|r| r.get_mut(column)) {
                *cell = Value::Str(text);
            }
        }
    }
    let dataset = build(dictionary, rows, &tables, label, &mut warnings)?;
    let timestamp = (!timestamp.trim().is_empty()).then_some(timestamp);
    warnings.dedup();
    Ok(DtaFile { dataset, release, timestamp, warnings })
}

/// Formats 113 to 115: a binary header, descriptors, then the data and value labels.
fn read_binary(bytes: &[u8]) -> Result<DtaFile, DtaError> {
    let mut c = Cursor { bytes, at: 0, big: false };
    let mut warnings = Vec::new();
    let release = u16::from(c.u8()?);
    let layout = Layout::of(release)?;
    c.big = c.u8()? == 1;
    c.take(2)?;
    let k = usize::from(c.u16()?);
    let n = c.u32()? as usize;
    let label = layout.text(c.take(81)?, &mut warnings);
    let timestamp = layout.text(c.take(18)?, &mut warnings);
    let mut kinds = Vec::with_capacity(k.min(c.left()));
    for _ in 0..k {
        let at = c.at;
        kinds.push(match c.u8()? {
            w @ 1..=244 => Kind::Str(usize::from(w)),
            251 => Kind::Byte,
            252 => Kind::Int,
            253 => Kind::Long,
            254 => Kind::Float,
            255 => Kind::Double,
            _ => return Err(DtaError::Invalid { offset: at, what: "typlist" }),
        });
    }
    let names = layout.texts(&mut c, k, layout.name, &mut warnings)?;
    c.take(k.checked_add(1).and_then(|e| e.checked_mul(2)).ok_or(DtaError::Truncated { offset: c.at })?)?;
    let formats = layout.texts(&mut c, k, layout.format, &mut warnings)?;
    let label_names = layout.texts(&mut c, k, layout.name, &mut warnings)?;
    let labels = layout.texts(&mut c, k, layout.label, &mut warnings)?;
    // Expansion fields, until a type and length of 0.
    loop {
        let kind = c.u8()?;
        let length = c.u32()? as usize;
        if kind == 0 && length == 0 {
            break;
        }
        c.take(length)?;
    }
    let dictionary = Dictionary { kinds, names, formats, label_names, labels };
    let (rows, _) = read_data(&mut c, &layout, &dictionary.kinds, n, &mut warnings)?;
    // Value labels, until the end of the file.
    let mut tables = HashMap::new();
    while c.left() > 0 {
        let length = c.u32()? as usize;
        let name = layout.text(c.take(33)?, &mut warnings);
        c.take(3)?;
        let table = c.take(length)?;
        tables.insert(name, value_labels(table, c.big, &layout, c.at - length, &mut warnings)?);
    }
    let dataset = build(dictionary, rows, &tables, label, &mut warnings)?;
    let timestamp = (!timestamp.trim().is_empty()).then_some(timestamp);
    warnings.dedup();
    Ok(DtaFile { dataset, release, timestamp, warnings })
}

/// The strL references of a case: its column, then (v, o).
type Refs = Vec<(usize, u64, u64)>;

/// The `n` observations; strLs are left empty, with their (v, o) to look up.
fn read_data(
    c: &mut Cursor,
    layout: &Layout,
    kinds: &[Kind],
    n: usize,
    warnings: &mut Vec<String>,
) -> Result<(Vec<Vec<Value>>, Vec<Refs>), DtaError> {
    let width: usize = kinds.iter().map(|k| k.bytes()).sum();
    // The data must be there before anything is set aside for it.
    let total = width.checked_mul(n).ok_or(DtaError::Truncated { offset: c.at })?;
    if total > c.left() {
        return Err(DtaError::Truncated { offset: c.bytes.len() });
    }
    let mut rows = Vec::with_capacity(n);
    let mut refs = Vec::with_capacity(if kinds.contains(&Kind::StrL) { n } else { 0 });
    let mut extended = 0usize;
    for _ in 0..n {
        let mut row = Vec::with_capacity(kinds.len());
        let mut strl = Vec::new();
        for (column, kind) in kinds.iter().enumerate() {
            let value = match *kind {
                Kind::Str(w) => Value::Str(layout.text(c.take(w)?, warnings)),
                Kind::StrL => {
                    let raw = c.u64()?;
                    // (v, o) split 4+4 in 117, 2+6 in 118, 3+5 in 119, by the byte order.
                    let bits = match layout.release {
                        117 => 32,
                        118 => 16,
                        _ => 24,
                    };
                    let (v, o) =
                        if c.big { (raw >> (64 - bits), raw & ((1u64 << (64 - bits)) - 1)) } else { (raw & ((1u64 << bits) - 1), raw >> bits) };
                    strl.push((column, v, o));
                    Value::Str(String::new())
                }
                Kind::Byte => integer(i64::from(c.u8()? as i8), 100, &mut extended),
                Kind::Int => integer(i64::from(c.u16()? as i16), 32_740, &mut extended),
                Kind::Long => integer(i64::from(c.u32()? as i32), 2_147_483_620, &mut extended),
                Kind::Float => {
                    let x = f32::from_bits(c.u32()?);
                    if x.is_nan() || x > f32::from_bits(0x7eff_ffff) {
                        extended += usize::from(x > f32::from_bits(0x7f00_0000));
                        Value::SysMis
                    } else {
                        // The decimal Stata shows (0.1, not 0.10000000149).
                        Value::Num(x.to_string().parse().unwrap_or(f64::from(x)))
                    }
                }
                Kind::Double => {
                    let x = f64::from_bits(c.u64()?);
                    if x.is_nan() || x > f64::from_bits(0x7fdf_ffff_ffff_ffff) {
                        extended += usize::from(x > f64::from_bits(0x7fe0_0000_0000_0000));
                        Value::SysMis
                    } else {
                        Value::Num(x)
                    }
                }
            };
            row.push(value);
        }
        rows.push(row);
        refs.push(strl);
    }
    if extended > 0 {
        warnings.push(tr!("dta-warn-extended-missing", n = extended));
    }
    Ok((rows, refs))
}

/// An integer, or system-missing above the largest valid one (`.` is one more, `.a` to `.z`
/// the codes after it).
fn integer(x: i64, largest: i64, extended: &mut usize) -> Value {
    if x > largest {
        *extended += usize::from(x > largest + 1);
        Value::SysMis
    } else {
        Value::Num(x as f64)
    }
}

/// A value-label table: `n`, the text's length, `n` offsets, `n` values, the text.
fn value_labels(table: &[u8], big: bool, layout: &Layout, start: usize, warnings: &mut Vec<String>) -> Result<Vec<(Value, String)>, DtaError> {
    let mut c = Cursor { bytes: table, at: 0, big };
    let bad = DtaError::Invalid { offset: start, what: "value label" };
    let count = c.u32()? as usize;
    let length = c.u32()? as usize;
    // Two 4-byte arrays of `count` and the text must fit in the table.
    if count.checked_mul(8).and_then(|a| a.checked_add(length)).is_none_or(|need| need > c.left()) {
        return Err(bad);
    }
    let offsets: Vec<usize> = (0..count).map(|_| c.u32().map(|o| o as usize)).collect::<Result<_, _>>()?;
    let values: Vec<i32> = (0..count).map(|_| c.u32().map(|v| v as i32)).collect::<Result<_, _>>()?;
    let text = c.take(length)?;
    let mut out = Vec::with_capacity(count);
    for (offset, value) in offsets.into_iter().zip(values) {
        let label = text.get(offset..).ok_or(DtaError::Invalid { offset: start, what: "value label" })?;
        out.push((Value::Num(f64::from(value)), layout.text(label, warnings)));
    }
    Ok(out)
}

/// The dataset: names made valid, types, formats, labels and the values.
fn build(
    d: Dictionary,
    rows: Vec<Vec<Value>>,
    tables: &HashMap<String, Vec<(Value, String)>>,
    label: String,
    warnings: &mut Vec<String>,
) -> Result<Dataset, DtaError> {
    let mut ds = Dataset::new();
    ds.label = (!label.is_empty()).then_some(label);
    // A string variable is as wide as its longest value.
    let widest =
        |column: usize| rows.iter().filter_map(|r| r.get(column)).map(|v| if let Value::Str(s) = v { s.len() } else { 0 }).max().unwrap_or(0);
    let mut truncated = false;
    let mut widths = Vec::with_capacity(d.kinds.len());
    let mut dates = Vec::with_capacity(d.kinds.len());
    for (column, kind) in d.kinds.iter().enumerate() {
        let name = d.names.get(column).cloned().unwrap_or_default();
        let name = valid_name(&name, &ds, column, warnings);
        let format = d.formats.get(column).map(String::as_str).unwrap_or("");
        let mut var = match *kind {
            // Text converted to UTF-8 can take more bytes than its field did.
            Kind::Str(w) => Variable::string(name, w.max(widest(column)).clamp(1, MAX_STRING) as u16),
            Kind::StrL => {
                let w = widest(column).clamp(1, MAX_STRING);
                Variable::string(name, w as u16)
            }
            _ => {
                let mut var = Variable::numeric(name);
                let whole = rows.iter().filter_map(|r| r.get(column)?.as_num()).all(|x| x.fract() == 0.0);
                var.format = number_format(format, *kind, whole);
                var
            }
        };
        dates
            .push(matches!(var.format.kind, FormatKind::Date | FormatKind::DateTime | FormatKind::MoYr | FormatKind::QYr).then_some(var.format.kind));
        widths.push(match var.var_type {
            VarType::String(w) => Some(usize::from(w)),
            VarType::Numeric => None,
        });
        if let Some(text) = d.labels.get(column).filter(|l| !l.is_empty()) {
            var.label = Some(text.clone());
        }
        let table = d.label_names.get(column).filter(|n| !n.is_empty()).and_then(|n| tables.get(n));
        if let (VarType::Numeric, Some(table)) = (var.var_type, table) {
            var.value_labels = table.clone();
        }
        var.measure = if matches!(var.var_type, VarType::String(_)) || !var.value_labels.is_empty() { Measure::Nominal } else { Measure::Scale };
        ds.add_variable(var)?;
    }
    for mut row in rows {
        for ((value, width), date) in row.iter_mut().zip(&widths).zip(&dates) {
            if let (Value::Num(x), Some(kind)) = (&mut *value, date) {
                *x = date_value(*x, *kind);
            }
            if let (Value::Str(s), Some(w)) = (value, width)
                && s.len() > *w
            {
                let mut end = *w;
                while !s.is_char_boundary(end) {
                    end -= 1;
                }
                s.truncate(end);
                truncated = true;
            }
        }
        ds.push_case(row)?;
    }
    if truncated {
        warnings.push(tr!("dta-warn-truncated", width = MAX_STRING));
    }
    Ok(ds)
}

/// `name` if it can name a variable here, else the nearest name that can, with a warning.
fn valid_name(name: &str, ds: &Dataset, column: usize, warnings: &mut Vec<String>) -> String {
    if is_valid_name(name) && ds.variable(name).is_none() {
        return name.to_string();
    }
    // Stata allows a leading underscore; SPSS-style names start with a letter.
    let mut base: String = name.chars().map(|c| if c.is_alphanumeric() || matches!(c, '_' | '.' | '@' | '#' | '$') { c } else { '_' }).collect();
    if !base.chars().next().is_some_and(char::is_alphabetic) {
        base.insert(0, 'v');
    }
    let base = base.trim_end_matches('.').chars().take(60).collect::<String>();
    let base = if is_valid_name(&base) { base } else { format!("v{}", column + 1) };
    let mut candidate = base.clone();
    let mut k = 2;
    while ds.variable(&candidate).is_some() || !is_valid_name(&candidate) {
        candidate = format!("{base}_{k}");
        k += 1;
    }
    warnings.push(tr!("dta-warn-renamed", from = name, to = candidate.as_str()));
    candidate
}

/// MK Sigma's format for a Stata numeric format (`%9.2f`, `%td`, `%10.0g`); `whole` when every
/// value is a whole number.
fn number_format(stata: &str, kind: Kind, whole: bool) -> Format {
    let spec = stata.trim_start_matches('%').trim_start_matches('-');
    // Dates and times: %td (days), %tc / %tC (milliseconds), %tm (months), %tq (quarters).
    let date = spec.strip_prefix('t').or_else(|| spec.strip_prefix('d').map(|_| "d"));
    if let Some(unit) = date {
        return match unit.chars().next() {
            Some('d') => Format::new(FormatKind::Date, 11, 0),
            Some('c' | 'C') => Format::new(FormatKind::DateTime, 20, 0),
            Some('m') => Format::new(FormatKind::MoYr, 8, 0),
            Some('q') => Format::new(FormatKind::QYr, 8, 0),
            _ => Format::new(FormatKind::F, 8, 0),
        };
    }
    let digits: String = spec.chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | ',')).collect();
    let letter = spec.get(digits.len()..).unwrap_or("");
    let (w, d) = digits.split_once(['.', ',']).unwrap_or((digits.as_str(), ""));
    let width: u16 = w.parse().unwrap_or(8).clamp(1, 40);
    let integer = matches!(kind, Kind::Byte | Kind::Int | Kind::Long);
    let decimals: u8 = match letter.chars().next() {
        // %g shows what the number needs: none for whole numbers, two otherwise.
        Some('g') | None => {
            if integer || whole {
                0
            } else {
                2
            }
        }
        _ => d.parse().unwrap_or(0),
    };
    let decimals = decimals.min(width.saturating_sub(1).min(16) as u8);
    let kind = match letter {
        l if l.starts_with('e') => FormatKind::E,
        l if l.ends_with('c') => FormatKind::Comma,
        _ => FormatKind::F,
    };
    Format::new(kind, width.max(u16::from(decimals) + 1), decimals)
}

/// A Stata date in MK Sigma's terms, seconds from 14 October 1582: from days (`%td`),
/// milliseconds (`%tc`), months (`%tm`, its first day) or quarters (`%tq`, its first day).
fn date_value(x: f64, kind: FormatKind) -> f64 {
    match kind {
        FormatKind::Date => STATA_EPOCH + x * 86_400.0,
        FormatKind::DateTime => STATA_EPOCH + x / 1000.0,
        FormatKind::MoYr => month_start(x),
        FormatKind::QYr => month_start(x * 3.0),
        _ => x,
    }
}

/// The first day of month `m` counted from January 1960, in seconds from 14 October 1582.
fn month_start(m: f64) -> f64 {
    let m = m.floor() as i64;
    let year = 1960 + m.div_euclid(12);
    let month = m.rem_euclid(12) + 1;
    STATA_EPOCH + days_from_1960(year, month, 1) as f64 * 86_400.0
}

/// Days from 1 January 1960 to a date of the proleptic Gregorian calendar.
fn days_from_1960(year: i64, month: i64, day: i64) -> i64 {
    // Days from 1 March of year 0, the usual civil-date arithmetic.
    let days = |y: i64, m: i64, d: i64| {
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy
    };
    days(year, month, day) - days(1960, 1, 1)
}
