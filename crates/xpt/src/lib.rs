//! SAS transport files (`.xpt`, the version 5 XPORT format regulators accept). Clean-room: built
//! from SAS's published record layout (technical paper TS-140, "Record Layout of a SAS Version 5
//! or 6 Data Set in SAS Transport (Xport) Format"), never from SAS's code or GPL code; the
//! numbers' IBM floating point is decoded from its definition, not from the paper's routines.
//!
//! A file is 80-byte records: a library header, then for each member (data set) its headers, its
//! variable descriptors (NAMESTR records, 140 bytes each, streamed across records) and its
//! observations, streamed the same way, padded with blanks to a whole record. The first member
//! is read; numbers are IBM doubles (truncated to the variable's length), `.` and `.A`–`.Z`
//! read as system-missing; text is UTF-8 when it is valid, Windows-1252 otherwise (the file
//! doesn't say). SAS date, datetime and time formats become MK Sigma's.
//!
//! Every count and length in the file is checked against the bytes left before anything is
//! allocated for it: a damaged or hostile file is an error, never a crash.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(test)]
mod tests;

use mksigma_core::{Dataset, DatasetError, Format, FormatKind, Measure, Value, Variable, is_valid_name};
use mksigma_i18n::tr;
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum XptError {
    #[error("{}", tr!("err-not-xpt"))]
    NotXpt,
    #[error("{}", tr!("err-xpt-truncated", offset = *offset))]
    Truncated { offset: usize },
    /// `what` is the record, as the layout names it (`NAMESTR`, `MEMBER HEADER`).
    #[error("{}", tr!("err-xpt-invalid", what = *what, offset = *offset))]
    Invalid { offset: usize, what: &'static str },
    /// A version 8 or 9 transport file (`LIBV8`), whose layout TS-140 doesn't describe.
    #[error("{}", tr!("err-xpt-v8"))]
    Version8,
    #[error(transparent)]
    Dataset(#[from] DatasetError),
}

/// A transport file's first member read into a dataset.
#[derive(Debug, Clone)]
pub struct XptFile {
    pub dataset: Dataset,
    /// The member's (data set's) name.
    pub name: String,
    /// When it was created, as written (`13APR89:10:20:06`).
    pub created: String,
    /// Things read leniently or left out, for the user to see.
    pub warnings: Vec<String>,
}

const RECORD: usize = 80;
const LIBRARY: &[u8] = b"HEADER RECORD*******LIBRARY HEADER RECORD!!!!!!!000000000000000000000000000000";
const LIBRARY_V8: &[u8] = b"HEADER RECORD*******LIBV8";
const MEMBER: &[u8] = b"HEADER RECORD*******MEMBER  HEADER RECORD!!!!!!!";
const DESCRIPTOR: &[u8] = b"HEADER RECORD*******DSCRPTR HEADER RECORD!!!!!!!";
const NAMESTR: &[u8] = b"HEADER RECORD*******NAMESTR HEADER RECORD!!!!!!!";
const OBS: &[u8] = b"HEADER RECORD*******OBS     HEADER RECORD!!!!!!!";

/// Seconds from SPSS's day 0 (14 October 1582) to SAS's (1 January 1960).
const SAS_EPOCH: f64 = 137_775.0 * 86_400.0;

/// Whether `bytes` start like a version 5 transport file.
pub fn is_xpt(bytes: &[u8]) -> bool {
    bytes.starts_with(LIBRARY)
}

/// The 80-byte records, one after another.
struct Records<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Records<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], XptError> {
        let end = self.at.checked_add(n).ok_or(XptError::Truncated { offset: self.at })?;
        let out = self.bytes.get(self.at..end).ok_or(XptError::Truncated { offset: self.at })?;
        self.at = end;
        Ok(out)
    }

    fn record(&mut self) -> Result<&'a [u8], XptError> {
        self.take(RECORD)
    }

    /// A header record that must start with `tag`.
    fn header(&mut self, tag: &[u8], what: &'static str) -> Result<&'a [u8], XptError> {
        let at = self.at;
        let record = self.record()?;
        if record.starts_with(tag) { Ok(record) } else { Err(XptError::Invalid { offset: at, what }) }
    }

    /// Skips to the next whole record (streamed data end padded with blanks).
    fn align(&mut self) {
        let over = self.at % RECORD;
        if over != 0 {
            self.at = self.at.saturating_add(RECORD - over).min(self.bytes.len());
        }
    }
}

/// Text in a fixed field: trailing blanks (and NULs) dropped; UTF-8 when it is, else Windows-1252.
fn text(bytes: &[u8]) -> String {
    let end = bytes.iter().rposition(|b| *b != b' ' && *b != 0).map_or(0, |i| i + 1);
    let bytes = bytes.get(..end).unwrap_or(&[]);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes).0.into_owned(),
    }
}

/// A big-endian (IBM) 2-byte integer at `at` of `b`.
fn short(b: &[u8], at: usize) -> i16 {
    b.get(at..at + 2).and_then(|s| s.try_into().ok()).map_or(0, i16::from_be_bytes)
}

/// A big-endian (IBM) 4-byte integer at `at` of `b`.
fn long(b: &[u8], at: usize) -> i32 {
    b.get(at..at + 4).and_then(|s| s.try_into().ok()).map_or(0, i32::from_be_bytes)
}

/// An IBM hexadecimal floating-point number of 2 to 8 bytes (the rest of the 8 taken as 0): a
/// sign bit, a 7-bit power of 16 biased by 64, and a 56-bit fraction. `None` for SAS's missing
/// values: `.`, `._` and `.A`–`.Z` in the first byte, zeros after it.
pub(crate) fn ibm(bytes: &[u8]) -> Option<f64> {
    let mut b = [0u8; 8];
    for (to, from) in b.iter_mut().zip(bytes) {
        *to = *from;
    }
    let first = b[0];
    let rest_zero = b.iter().skip(1).all(|x| *x == 0);
    if rest_zero && (first == b'.' || first == b'_' || first.is_ascii_uppercase()) {
        return None;
    }
    let fraction = b.iter().skip(1).fold(0u64, |acc, x| (acc << 8) | u64::from(*x));
    if fraction == 0 {
        return Some(0.0);
    }
    let exponent = i32::from(first & 0x7f) - 64;
    let magnitude = fraction as f64 * 2f64.powi(4 * exponent - 56);
    Some(if first & 0x80 != 0 { -magnitude } else { magnitude })
}

/// `len` bytes from `pos` of `row`, if they are there.
fn field(row: &[u8], pos: usize, len: usize) -> Option<&[u8]> {
    row.get(pos..pos.checked_add(len)?)
}

/// A variable's descriptor.
struct Namestr {
    numeric: bool,
    length: usize,
    name: String,
    label: String,
    format: String,
    width: i16,
    decimals: i16,
    position: usize,
}

pub fn read(bytes: &[u8]) -> Result<XptFile, XptError> {
    if bytes.starts_with(LIBRARY_V8) {
        return Err(XptError::Version8);
    }
    if !is_xpt(bytes) {
        return Err(XptError::NotXpt);
    }
    let mut r = Records { bytes, at: RECORD };
    let mut warnings = Vec::new();
    // The library's two real header records.
    r.record()?;
    r.record()?;
    // The member: its header (the descriptor's size at offset 75), descriptor header, data.
    let member = r.header(MEMBER, "MEMBER HEADER")?;
    let size: usize = std::str::from_utf8(member.get(74..78).unwrap_or(b"0140")).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(140);
    if !(136..=140).contains(&size) {
        return Err(XptError::Invalid { offset: r.at - RECORD, what: "MEMBER HEADER" });
    }
    r.header(DESCRIPTOR, "DSCRPTR HEADER")?;
    let first = r.record()?;
    let name = text(first.get(8..16).unwrap_or(&[]));
    let created = text(first.get(64..80).unwrap_or(&[]));
    let second = r.record()?;
    let label = text(second.get(32..72).unwrap_or(&[]));
    let at = r.at;
    let header = r.header(NAMESTR, "NAMESTR HEADER")?;
    let count: usize = std::str::from_utf8(header.get(54..58).unwrap_or(b"0"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .ok_or(XptError::Invalid { offset: at, what: "NAMESTR HEADER" })?;
    // The descriptors must be in the file before anything is set aside for them.
    let all = r.take(count.checked_mul(size).ok_or(XptError::Truncated { offset: r.at })?)?;
    let mut vars: Vec<Namestr> = Vec::with_capacity(count);
    for (k, d) in all.chunks(size).enumerate() {
        let what = "NAMESTR";
        let offset = at + RECORD + k * size;
        let numeric = match short(d, 0) {
            1 => true,
            2 => false,
            _ => return Err(XptError::Invalid { offset, what }),
        };
        let length = usize::try_from(short(d, 4)).map_err(|_| XptError::Invalid { offset, what })?;
        if length == 0 || (numeric && !(2..=8).contains(&length)) {
            return Err(XptError::Invalid { offset, what });
        }
        vars.push(Namestr {
            numeric,
            length,
            name: text(d.get(8..16).unwrap_or(&[])),
            label: text(d.get(16..56).unwrap_or(&[])),
            format: text(d.get(56..64).unwrap_or(&[])),
            width: short(d, 64),
            decimals: short(d, 66),
            position: usize::try_from(long(d, 84)).map_err(|_| XptError::Invalid { offset, what })?,
        });
    }
    r.align();
    r.header(OBS, "OBS HEADER")?;
    // Each observation is as long as its last value reaches.
    let width = vars.iter().map(|v| v.position.saturating_add(v.length)).max().unwrap_or(0);
    let data_start = r.at;
    // The observations run to the next member's header, or to the end of the file.
    let data_end = bytes
        .get(data_start..)
        .and_then(|rest| rest.chunks(RECORD).position(|rec| rec.starts_with(MEMBER)))
        .map_or(bytes.len(), |k| data_start + k * RECORD);
    let data = bytes.get(data_start..data_end).unwrap_or(&[]);
    if data_end < bytes.len() {
        warnings.push(tr!("xpt-warn-more-members", name = name.as_str()));
    }
    let mut rows: Vec<&[u8]> = if width == 0 { Vec::new() } else { data.chunks_exact(width).collect() };
    // The blanks that pad the last record are no observation.
    while rows.last().is_some_and(|row| row.iter().all(|b| *b == b' ')) && data.len() - rows.len() * width < RECORD {
        rows.pop();
    }

    let mut ds = Dataset::new();
    ds.label = (!label.is_empty()).then_some(label);
    let mut kinds = Vec::with_capacity(vars.len());
    let mut extended = 0usize;
    for (k, v) in vars.iter().enumerate() {
        let var_name = valid_name(&v.name, &ds, k, &mut warnings);
        let mut var = if v.numeric {
            let mut var = Variable::numeric(var_name);
            let whole = rows.iter().filter_map(|row| ibm(field(row, v.position, v.length)?)).all(|x| x.fract() == 0.0);
            var.format = number_format(&v.format, v.width, v.decimals, whole);
            var
        } else {
            Variable::string(var_name, u16::try_from(v.length.max(1)).unwrap_or(u16::MAX))
        };
        if !v.label.is_empty() {
            var.label = Some(v.label.clone());
        }
        var.measure = if v.numeric { Measure::Scale } else { Measure::Nominal };
        kinds.push(var.format.kind);
        ds.add_variable(var)?;
    }
    for row in rows {
        let mut values = Vec::with_capacity(vars.len());
        for (v, kind) in vars.iter().zip(&kinds) {
            let bytes = field(row, v.position, v.length).ok_or(XptError::Invalid { offset: data_start, what: "NAMESTR" })?;
            values.push(if v.numeric {
                match ibm(bytes) {
                    Some(x) => Value::Num(date_value(x, *kind)),
                    None => {
                        extended += usize::from(bytes.first().is_some_and(|b| *b != b'.'));
                        Value::SysMis
                    }
                }
            } else {
                Value::Str(text(bytes))
            });
        }
        ds.push_case(values)?;
    }
    if extended > 0 {
        warnings.push(tr!("xpt-warn-special-missing", n = extended));
    }
    Ok(XptFile { dataset: ds, name, created, warnings })
}

/// `name` if it can name a variable here, else the nearest name that can, with a warning.
fn valid_name(name: &str, ds: &Dataset, k: usize, warnings: &mut Vec<String>) -> String {
    if is_valid_name(name) && ds.variable(name).is_none() {
        return name.to_string();
    }
    let mut base: String = name.chars().map(|c| if c.is_alphanumeric() || matches!(c, '_' | '@' | '#' | '$') { c } else { '_' }).collect();
    if !base.chars().next().is_some_and(char::is_alphabetic) {
        base.insert(0, 'v');
    }
    let base = if is_valid_name(&base) { base } else { format!("v{}", k + 1) };
    let mut candidate = base.clone();
    let mut n = 2;
    while ds.variable(&candidate).is_some() || !is_valid_name(&candidate) {
        candidate = format!("{base}_{n}");
        n += 1;
    }
    warnings.push(tr!("xpt-warn-renamed", from = name, to = candidate.as_str()));
    candidate
}

/// MK Sigma's format for a SAS format name, width and decimals; `whole` when every value is a
/// whole number. Dates (`DATE`, `MMDDYY`, `DDMMYY`, `YYMMDD`…), datetimes and times keep their
/// meaning; `COMMA` and `DOLLAR` theirs; anything else is a plain number.
fn number_format(name: &str, width: i16, decimals: i16, whole: bool) -> Format {
    let name = name.trim().to_ascii_uppercase();
    let w = u16::try_from(width).ok().filter(|w| *w > 0);
    let d = u8::try_from(decimals).ok().filter(|d| *d <= 16).unwrap_or(0);
    match name.as_str() {
        n if n.starts_with("DATETIME") => Format::new(FormatKind::DateTime, 20, 0),
        n if n.starts_with("TIME") || n.starts_with("HHMM") => Format::new(FormatKind::Time, 8, 0),
        "DATE" | "DDMMYY" | "MMDDYY" | "YYMMDD" | "E8601DA" | "IS8601DA" | "WEEKDATE" | "WORDDATE" | "NLDATE" => Format::new(FormatKind::Date, 11, 0),
        "MONYY" => Format::new(FormatKind::MoYr, 8, 0),
        "YYQ" => Format::new(FormatKind::QYr, 8, 0),
        "COMMA" => Format::new(FormatKind::Comma, w.unwrap_or(10).clamp(1, 40), d),
        "DOLLAR" => Format::new(FormatKind::Dollar, w.unwrap_or(10).clamp(1, 40), d),
        _ => {
            let d = if w.is_some() {
                d
            } else if whole {
                0
            } else {
                2
            };
            let width = w.unwrap_or(8).clamp(u16::from(d) + 1, 40);
            Format::new(FormatKind::F, width, d)
        }
    }
}

/// A SAS date (days from 1960), datetime (seconds from 1960) or month in MK Sigma's terms:
/// seconds from 14 October 1582. Times stay seconds.
fn date_value(x: f64, kind: FormatKind) -> f64 {
    match kind {
        FormatKind::Date | FormatKind::MoYr | FormatKind::QYr => SAS_EPOCH + x * 86_400.0,
        FormatKind::DateTime => SAS_EPOCH + x,
        _ => x,
    }
}
