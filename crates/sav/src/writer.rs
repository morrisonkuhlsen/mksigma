//! Writes a dataset as a system file: little-endian, UTF-8 (record 7.20), uncompressed or
//! bytecode-compressed. Names longer than 8 bytes get a unique short name in the variable record
//! and their full name in record 7.13, as SPSS does.

use mksigma_core::{Dataset, Format, FormatKind, Measure, MissingValues, Value, VarType, Variable};

use crate::{Compression, SavError};

/// What the header says about the file.
#[derive(Debug, Clone)]
pub struct WriteOptions {
    pub compression: Compression,
    /// Creation date (`dd Mon yy`) and time (`hh:mm:ss`); see `timestamp`.
    pub created: (String, String),
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self { compression: Compression::Bytecode, created: ("01 Jan 70".into(), "00:00:00".into()) }
    }
}

const SYSMIS: f64 = -f64::MAX;
const HIGHEST: f64 = f64::MAX;
/// The largest finite negative number above system-missing.
const LOWEST: f64 = f64::from_bits(0xffef_ffff_ffff_fffe);
const BIAS: f64 = 100.0;

/// The header's date and time for seconds since 1970 (UTC).
pub fn timestamp(unix_seconds: i64) -> (String, String) {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let days = unix_seconds.div_euclid(86_400);
    let secs = unix_seconds.rem_euclid(86_400);
    // Days to civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let name = usize::try_from(month - 1).ok().and_then(|m| MONTHS.get(m)).copied().unwrap_or("Jan");
    (format!("{day:02} {name} {:02}", year.rem_euclid(100)), format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60))
}

fn format_code(kind: FormatKind) -> i32 {
    match kind {
        FormatKind::A => 1,
        FormatKind::AHex => 2,
        FormatKind::Comma => 3,
        FormatKind::Dollar => 4,
        FormatKind::F => 5,
        FormatKind::N => 16,
        FormatKind::E => 17,
        FormatKind::Date => 20,
        FormatKind::Time => 21,
        FormatKind::DateTime => 22,
        FormatKind::ADate => 23,
        FormatKind::JDate => 24,
        FormatKind::DTime => 25,
        FormatKind::WkDay => 26,
        FormatKind::Month => 27,
        FormatKind::MoYr => 28,
        FormatKind::QYr => 29,
        FormatKind::WkYr => 30,
        FormatKind::Pct => 31,
        FormatKind::Dot => 32,
        FormatKind::EDate => 38,
        FormatKind::SDate => 39,
        FormatKind::MTime => 40,
        FormatKind::YmdHms => 41,
        FormatKind::Other(code) => i32::from(code),
    }
}

fn packed(f: Format) -> i32 {
    (format_code(f.kind) << 16) | (i32::from(f.width.min(255)) << 8) | i32::from(f.decimals)
}

/// `s` cut to at most `max` bytes on a character boundary.
fn cut(s: &str, max: usize) -> &str {
    let mut end = max.min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s.get(..end).unwrap_or_default()
}

/// `bytes` padded with spaces (or cut) to exactly `n` bytes.
fn padded(bytes: &[u8], n: usize) -> Vec<u8> {
    let mut v = bytes.get(..n.min(bytes.len())).unwrap_or_default().to_vec();
    v.resize(n, b' ');
    v
}

/// Unique short names: the long name upper-cased, restricted to ASCII letters, digits and `_`,
/// cut to 8 bytes; a numeric suffix settles collisions.
fn short_names(vars: &[Variable]) -> Vec<String> {
    let mut used: Vec<String> = Vec::with_capacity(vars.len());
    for var in vars {
        let mut base: String = var.name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').map(|c| c.to_ascii_uppercase()).collect();
        if !base.starts_with(|c: char| c.is_ascii_alphabetic()) {
            base.insert(0, 'V');
        }
        let mut name = cut(&base, 8).to_string();
        let mut n = 1;
        while used.contains(&name) {
            let suffix = format!("_{n}");
            name = format!("{}{suffix}", cut(&base, 8 - suffix.len()));
            n += 1;
        }
        used.push(name);
    }
    used
}

struct Out {
    buf: Vec<u8>,
}

impl Out {
    fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn bytes(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }
    fn len_i32(&mut self, n: usize) -> Result<(), SavError> {
        self.i32(i32::try_from(n).map_err(|_| SavError::Unsupported("records over 2 GB"))?);
        Ok(())
    }
    /// An extension record (type 7) whose items are `size` bytes each.
    fn extension(&mut self, subtype: i32, size: usize, body: &[u8]) -> Result<(), SavError> {
        self.i32(7);
        self.i32(subtype);
        self.len_i32(size)?;
        self.len_i32(body.len() / size)?;
        self.bytes(body);
        Ok(())
    }
}

/// Segments a variable occupies in a case: one per 8 bytes of a string, one for a number.
fn segments(var: &Variable) -> usize {
    match var.var_type {
        VarType::Numeric => 1,
        VarType::String(w) => usize::from(w).div_ceil(8).max(1),
    }
}

/// A user-missing bound as written: infinite ends become LOWEST and HIGHEST.
fn bound(x: f64) -> f64 {
    if x <= -1e300 {
        LOWEST
    } else if x >= 1e300 {
        HIGHEST
    } else {
        x
    }
}

fn number(v: &Value) -> f64 {
    match v {
        Value::Num(x) => *x,
        _ => SYSMIS,
    }
}

fn header(out: &mut Out, ds: &Dataset, options: &WriteOptions, case_size: usize, weight_segment: i32) -> Result<(), SavError> {
    out.bytes(b"$FL2");
    out.bytes(&padded(concat!("@(#) SPSS DATA FILE MK Sigma ", env!("CARGO_PKG_VERSION")).as_bytes(), 60));
    out.i32(2);
    out.len_i32(case_size)?;
    out.i32(match options.compression {
        Compression::None => 0,
        Compression::Bytecode => 1,
        Compression::Zlib => return Err(SavError::Unsupported("writing .zsav")),
    });
    out.i32(weight_segment);
    out.len_i32(ds.n_cases())?;
    out.f64(BIAS);
    out.bytes(&padded(options.created.0.as_bytes(), 9));
    out.bytes(&padded(options.created.1.as_bytes(), 8));
    out.bytes(&padded(cut(ds.label.as_deref().unwrap_or(""), 64).as_bytes(), 64));
    out.bytes(&[0; 3]);
    Ok(())
}

fn variable_record(out: &mut Out, var: &Variable, short: &str, warnings: &mut Vec<String>) -> Result<(), SavError> {
    let width = match var.var_type {
        VarType::Numeric => 0,
        VarType::String(w) => i32::from(w),
    };
    let missing: Vec<[u8; 8]> = match (&var.missing, var.var_type) {
        (MissingValues::None, _) => vec![],
        (MissingValues::Discrete(_), VarType::String(w)) if w > 8 => {
            warnings.push(format!("{}: missing values of strings over 8 bytes are not written yet", var.name));
            vec![]
        }
        (MissingValues::Discrete(list), _) => list
            .iter()
            .take(3)
            .map(|v| match v {
                Value::Str(s) => padded(s.as_bytes(), 8).try_into().unwrap_or([b' '; 8]),
                v => number(v).to_le_bytes(),
            })
            .collect(),
        (MissingValues::Range { low, high, discrete }, _) => {
            let mut v = vec![bound(*low).to_le_bytes(), bound(*high).to_le_bytes()];
            v.extend(discrete.map(f64::to_le_bytes));
            v
        }
    };
    let n_missing = match &var.missing {
        MissingValues::Range { .. } => -i32::try_from(missing.len()).unwrap_or(2),
        _ => i32::try_from(missing.len()).unwrap_or(0),
    };
    let format = packed(var.format);
    out.i32(2);
    out.i32(width);
    out.i32(i32::from(var.label.is_some()));
    out.i32(n_missing);
    out.i32(format);
    out.i32(format);
    out.bytes(&padded(short.as_bytes(), 8));
    if let Some(label) = &var.label {
        let label = cut(label, 255).as_bytes();
        out.len_i32(label.len())?;
        out.bytes(label);
        out.bytes(&vec![0; label.len().next_multiple_of(4) - label.len()]);
    }
    for m in &missing {
        out.bytes(m);
    }
    // Continuation records for the rest of a string's segments.
    for _ in 1..segments(var) {
        out.i32(2);
        out.i32(-1);
        out.bytes(&[0; 16]);
        out.bytes(&[b' '; 8]);
    }
    Ok(())
}

/// A length as a little-endian 32-bit integer.
fn push_len(buf: &mut Vec<u8>, n: usize) -> Result<(), SavError> {
    buf.extend_from_slice(&i32::try_from(n).map_err(|_| SavError::Unsupported("records over 2 GB"))?.to_le_bytes());
    Ok(())
}

/// Records 3 and 4 for a numeric or short string variable; longer strings use record 7.21.
fn value_labels(out: &mut Out, var: &Variable, segment: usize) -> Result<(), SavError> {
    if var.value_labels.is_empty() || matches!(var.var_type, VarType::String(w) if w > 8) {
        return Ok(());
    }
    out.i32(3);
    out.len_i32(var.value_labels.len())?;
    for (value, label) in &var.value_labels {
        match value {
            Value::Str(s) => out.bytes(&padded(s.as_bytes(), 8)),
            v => out.f64(number(v)),
        }
        let label = cut(label, 255).as_bytes();
        // The length byte and the label fill a multiple of 8 bytes.
        out.bytes(&[u8::try_from(label.len()).unwrap_or(255)]);
        out.bytes(label);
        out.bytes(&vec![b' '; (label.len() + 1).next_multiple_of(8) - label.len() - 1]);
    }
    out.i32(4);
    out.i32(1);
    out.len_i32(segment + 1)?;
    Ok(())
}

fn compress_case(ds: &Dataset, case: usize, codes: &mut Vec<u8>, raw: &mut Vec<[u8; 8]>, out: &mut Out) {
    let mut push = |code: u8, data: Option<[u8; 8]>, codes: &mut Vec<u8>, raw: &mut Vec<[u8; 8]>| {
        codes.push(code);
        raw.extend(data);
        if codes.len() == 8 {
            out.bytes(codes);
            for r in raw.iter() {
                out.bytes(r);
            }
            codes.clear();
            raw.clear();
        }
    };
    for (i, var) in ds.variables().iter().enumerate() {
        let value = ds.column(i).and_then(|c| c.get(case));
        match var.var_type {
            VarType::Numeric => match value {
                Some(Value::Num(x)) if x.fract() == 0.0 && (1.0 - BIAS..=251.0 - BIAS).contains(x) => {
                    push((x + BIAS) as u8, None, codes, raw);
                }
                Some(Value::Num(x)) => push(253, Some(x.to_le_bytes()), codes, raw),
                _ => push(255, None, codes, raw),
            },
            VarType::String(_) => {
                let text = match value {
                    Some(Value::Str(s)) => s.as_bytes(),
                    _ => b"",
                };
                for chunk in padded(text, segments(var) * 8).chunks(8) {
                    let chunk: [u8; 8] = chunk.try_into().unwrap_or([b' '; 8]);
                    if chunk == [b' '; 8] { push(254, None, codes, raw) } else { push(253, Some(chunk), codes, raw) }
                }
            }
        }
    }
}

pub fn write(ds: &Dataset, options: &WriteOptions) -> Result<(Vec<u8>, Vec<String>), SavError> {
    let vars = ds.variables();
    if vars.iter().any(|v| matches!(v.var_type, VarType::String(w) if w > 255)) {
        return Err(SavError::Unsupported("writing strings over 255 bytes"));
    }
    let mut warnings = Vec::new();
    let mut out = Out { buf: Vec::new() };
    let starts: Vec<usize> = vars.iter().scan(0, |next, v| Some(std::mem::replace(next, *next + segments(v)))).collect();
    let case_size = vars.iter().map(segments).sum();
    let weight = ds.weight.and_then(|w| starts.get(w)).map_or(0, |s| i32::try_from(s + 1).unwrap_or(0));
    header(&mut out, ds, options, case_size, weight)?;

    let shorts = short_names(vars);
    for (var, short) in vars.iter().zip(&shorts) {
        variable_record(&mut out, var, short, &mut warnings)?;
    }
    for (var, start) in vars.iter().zip(&starts) {
        value_labels(&mut out, var, *start)?;
    }
    if !ds.documents.is_empty() {
        out.i32(6);
        out.len_i32(ds.documents.len())?;
        for line in &ds.documents {
            out.bytes(&padded(cut(line, 80).as_bytes(), 80));
        }
    }

    // 7.3 integer info: version, machine, IEEE floats, compression, little-endian, UTF-8.
    let compression_code = if options.compression == Compression::Bytecode { 1 } else { 0 };
    let ints: Vec<u8> = [0i32, 1, 0, -1, 1, compression_code, 2, 65001].iter().flat_map(|v| v.to_le_bytes()).collect();
    out.extension(3, 4, &ints)?;
    // 7.4 floating-point info: system-missing, highest, lowest.
    let floats: Vec<u8> = [SYSMIS, HIGHEST, LOWEST].iter().flat_map(|v| v.to_le_bytes()).collect();
    out.extension(4, 8, &floats)?;
    // 7.11 display: measurement level, column width, alignment (numbers right, text left).
    let display: Vec<u8> = vars
        .iter()
        .flat_map(|v| {
            let measure = match v.measure {
                Measure::Nominal => 1,
                Measure::Ordinal => 2,
                Measure::Scale => 3,
            };
            let numeric = v.var_type == VarType::Numeric;
            [measure, i32::from(v.format.width).clamp(1, 64), i32::from(numeric)].map(i32::to_le_bytes)
        })
        .flatten()
        .collect();
    if !display.is_empty() {
        out.extension(11, 4, &display)?;
    }
    // 7.13 long names.
    let long: Vec<String> = vars.iter().zip(&shorts).map(|(v, s)| format!("{s}={}", v.name)).collect();
    if !long.is_empty() {
        out.extension(13, 1, long.join("\t").as_bytes())?;
    }
    // 7.21 value labels of strings over 8 bytes: name, width, then (value, label) pairs.
    let mut long_labels = Vec::new();
    for var in vars {
        let VarType::String(width) = var.var_type else { continue };
        if width <= 8 || var.value_labels.is_empty() {
            continue;
        }
        push_len(&mut long_labels, var.name.len())?;
        long_labels.extend_from_slice(var.name.as_bytes());
        push_len(&mut long_labels, usize::from(width))?;
        push_len(&mut long_labels, var.value_labels.len())?;
        for (value, label) in &var.value_labels {
            let value = padded(if let Value::Str(s) = value { s.as_bytes() } else { b"" }, usize::from(width));
            let label = cut(label, 255).as_bytes();
            push_len(&mut long_labels, value.len())?;
            long_labels.extend_from_slice(&value);
            push_len(&mut long_labels, label.len())?;
            long_labels.extend_from_slice(label);
        }
    }
    if !long_labels.is_empty() {
        out.extension(21, 1, &long_labels)?;
    }
    // 7.20 encoding.
    out.extension(20, 1, b"UTF-8")?;
    out.i32(999);
    out.i32(0);

    match options.compression {
        Compression::Bytecode => {
            let (mut codes, mut raw) = (Vec::with_capacity(8), Vec::with_capacity(8));
            for case in 0..ds.n_cases() {
                compress_case(ds, case, &mut codes, &mut raw, &mut out);
            }
            if !codes.is_empty() {
                codes.resize(8, 0);
                out.bytes(&codes);
                for r in &raw {
                    out.bytes(r);
                }
            }
        }
        _ => {
            for case in 0..ds.n_cases() {
                for (i, var) in vars.iter().enumerate() {
                    match (var.var_type, ds.column(i).and_then(|c| c.get(case))) {
                        (VarType::Numeric, v) => out.f64(v.map_or(SYSMIS, number)),
                        (VarType::String(_), Some(Value::Str(s))) => out.bytes(&padded(s.as_bytes(), segments(var) * 8)),
                        (VarType::String(_), _) => out.bytes(&padded(b"", segments(var) * 8)),
                    }
                }
            }
        }
    }
    Ok((out.buf, warnings))
}
