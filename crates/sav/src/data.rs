//! The cases. Each case is a run of 8-byte segments: one per numeric variable, `ceil(width / 8)`
//! per string variable. Uncompressed files store the segments as they are; bytecode-compressed
//! files store blocks of eight one-byte codes, each standing for a segment (a small integer, a
//! run of spaces, system-missing) or saying that the segment follows the block raw.

use mksigma_core::Value;

use crate::cursor::Cursor;
use crate::dictionary::{Dictionary, decode};
use crate::{Compression, SavError, SavFile};

enum Segment {
    /// A number stored in the code itself (code − bias).
    Num(f64),
    Raw([u8; 8]),
    Spaces,
    SysMis,
}

/// Yields segments until the data ends (`None`).
struct Segments<'a> {
    c: Cursor<'a>,
    compressed: bool,
    bias: f64,
    codes: [u8; 8],
    /// Next code to use in `codes`; 8 when a new block is needed.
    next: usize,
    done: bool,
}

impl Segments<'_> {
    fn next(&mut self) -> Result<Option<Segment>, SavError> {
        if self.done {
            return Ok(None);
        }
        if !self.compressed {
            // A trailing partial segment is ignored: the case it belongs to is incomplete anyway.
            return Ok(if self.c.remaining() < 8 { None } else { Some(Segment::Raw(self.c.array()?)) });
        }
        loop {
            let Some(&code) = self.codes.get(self.next) else {
                if self.c.remaining() == 0 {
                    self.done = true;
                    return Ok(None);
                }
                self.codes = self.c.array()?;
                self.next = 0;
                continue;
            };
            self.next += 1;
            match code {
                0 => {}
                252 => {
                    self.done = true;
                    return Ok(None);
                }
                253 => return Ok(Some(Segment::Raw(self.c.array()?))),
                254 => return Ok(Some(Segment::Spaces)),
                255 => return Ok(Some(Segment::SysMis)),
                n => return Ok(Some(Segment::Num(f64::from(n) - self.bias))),
            }
        }
    }
}

pub(crate) fn read(bytes: &[u8], dict: Dictionary) -> Result<SavFile, SavError> {
    let Dictionary { mut dataset, layout, data_offset, big_endian, compression, bias, sysmis, n_cases, encoding, product, created, mut warnings } =
        dict;
    if compression == Compression::Zlib {
        return Err(SavError::Unsupported(".zsav (zlib-compressed) data"));
    }
    let data = bytes.get(data_offset..).unwrap_or_default();
    let c = Cursor::new(data, big_endian);
    let mut segments = Segments { c, compressed: compression == Compression::Bytecode, bias, codes: [0; 8], next: 8, done: false };
    let mut non_finite = 0usize;
    let mut number = |raw: [u8; 8]| {
        let x = if big_endian { f64::from_be_bytes(raw) } else { f64::from_le_bytes(raw) };
        if x.to_bits() == sysmis.to_bits() {
            Value::SysMis
        } else if !x.is_finite() {
            non_finite += 1;
            Value::SysMis
        } else {
            Value::Num(x)
        }
    };

    let mut case_count = 0usize;
    'cases: while n_cases.is_none_or(|n| case_count < n) {
        let mut case = Vec::with_capacity(layout.len());
        for (i, &(width, n_segments)) in layout.iter().enumerate() {
            if width == 0 {
                let Some(segment) = segments.next()? else {
                    if i > 0 {
                        warnings.push("the last case is incomplete and was dropped".into());
                    }
                    break 'cases;
                };
                case.push(match segment {
                    Segment::Num(x) => Value::Num(x),
                    Segment::Raw(raw) => number(raw),
                    Segment::SysMis | Segment::Spaces => Value::SysMis,
                });
            } else {
                let mut text = Vec::with_capacity(n_segments * 8);
                for _ in 0..n_segments {
                    match segments.next()? {
                        Some(Segment::Raw(raw)) => text.extend_from_slice(&raw),
                        // A number or system-missing in a string is invalid; read it as blanks.
                        Some(Segment::Spaces | Segment::Num(_) | Segment::SysMis) => text.extend_from_slice(b"        "),
                        None => {
                            if i > 0 || !text.is_empty() {
                                warnings.push("the last case is incomplete and was dropped".into());
                            }
                            break 'cases;
                        }
                    }
                }
                text.truncate(width);
                case.push(Value::Str(decode(encoding, &text)));
            }
        }
        if layout.is_empty() {
            break;
        }
        dataset.push_case(case)?;
        case_count += 1;
    }
    if let Some(n) = n_cases.filter(|n| *n != case_count) {
        warnings.push(format!("the header says {n} cases, the file holds {case_count}"));
    }
    if non_finite > 0 {
        warnings.push(mksigma_i18n::tr!("sav-warn-non-finite", n = non_finite));
    }

    Ok(SavFile { dataset, product, created, compression, warnings })
}
