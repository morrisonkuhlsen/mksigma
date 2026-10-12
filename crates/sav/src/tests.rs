//! Test files are built byte by byte here from the documented layout; no files written by SPSS
//! are committed (see AGENTS.md).

use mksigma_core::{FormatKind, Measure, MissingValues, Value, VarType};

use super::*;

/// Writes a system file in either byte order.
struct Builder {
    big_endian: bool,
    buf: Vec<u8>,
}

/// A variable record: width (0 numeric), name, label, missing-value code and values, print format.
struct Var<'a> {
    width: i32,
    name: &'a str,
    label: Option<&'a str>,
    n_missing: i32,
    missing: &'a [[u8; 8]],
    print: i32,
}

impl Builder {
    fn new(big_endian: bool) -> Self {
        Self { big_endian, buf: Vec::new() }
    }

    fn i32(&mut self, v: i32) -> &mut Self {
        let b = if self.big_endian { v.to_be_bytes() } else { v.to_le_bytes() };
        self.buf.extend_from_slice(&b);
        self
    }

    fn f64(&mut self, v: f64) -> &mut Self {
        let b = self.num(v);
        self.buf.extend_from_slice(&b);
        self
    }

    fn num(&self, v: f64) -> [u8; 8] {
        if self.big_endian { v.to_be_bytes() } else { v.to_le_bytes() }
    }

    fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(b);
        self
    }

    /// `s` padded with spaces to `n` bytes.
    fn padded(&mut self, s: &str, n: usize) -> &mut Self {
        let mut b = s.as_bytes().to_vec();
        b.resize(n, b' ');
        self.bytes(&b)
    }

    fn header(&mut self, magic: &[u8; 4], case_size: i32, compression: i32, weight: i32, n_cases: i32) -> &mut Self {
        self.bytes(magic).padded("@(#) SPSS DATA FILE MK Sigma tests", 60).i32(2).i32(case_size).i32(compression).i32(weight).i32(n_cases).f64(100.0);
        self.padded("12 Oct 26", 9).padded("10:20:30", 8).padded("Survey 2026", 64).padded("", 3)
    }

    fn var(&mut self, v: &Var) -> &mut Self {
        self.i32(2).i32(v.width).i32(i32::from(v.label.is_some())).i32(v.n_missing).i32(v.print).i32(v.print).padded(v.name, 8);
        if let Some(label) = v.label {
            self.i32(label.len() as i32).padded(label, label.len().next_multiple_of(4));
        }
        for m in v.missing {
            self.bytes(m);
        }
        // Continuation records for the rest of a long string.
        for _ in 1..(v.width.max(1) as usize).div_ceil(8) {
            self.i32(2).i32(-1).i32(0).i32(0).i32(0).i32(0).padded("", 8);
        }
        self
    }

    fn value_labels(&mut self, labels: &[([u8; 8], &str)], segments: &[i32]) -> &mut Self {
        self.i32(3).i32(labels.len() as i32);
        for (value, label) in labels {
            self.bytes(value).bytes(&[label.len() as u8]).padded(label, (label.len() + 1).next_multiple_of(8) - 1);
        }
        self.i32(4).i32(segments.len() as i32);
        for s in segments {
            self.i32(*s);
        }
        self
    }

    fn extension(&mut self, subtype: i32, size: i32, body: &[u8]) -> &mut Self {
        self.i32(7).i32(subtype).i32(size).i32(body.len() as i32 / size).bytes(body)
    }

    fn end_dictionary(&mut self) -> &mut Self {
        self.i32(999).i32(0)
    }
}

fn str8(s: &str) -> [u8; 8] {
    let mut b = [b' '; 8];
    b[..s.len()].copy_from_slice(s.as_bytes());
    b
}

const F8_2: i32 = (5 << 16) | (8 << 8) | 2;
const F3_0: i32 = (5 << 16) | (3 << 8);
const A10: i32 = (1 << 16) | (10 << 8);
const DATE11: i32 = (20 << 16) | (11 << 8);

/// Two numeric variables and a 10-byte string (two segments), three cases, uncompressed.
fn uncompressed(big_endian: bool) -> Vec<u8> {
    let mut b = Builder::new(big_endian);
    let missing_99 = b.num(99.0);
    b.header(b"$FL2", 4, 0, 0, 3)
        .var(&Var { width: 0, name: "AGE", label: Some("Age in years"), n_missing: 1, missing: &[missing_99], print: F3_0 })
        .var(&Var { width: 10, name: "CITY", label: None, n_missing: 0, missing: &[], print: A10 })
        .var(&Var { width: 0, name: "BORN", label: Some("Birth date"), n_missing: 0, missing: &[], print: DATE11 })
        .end_dictionary();
    for (age, city, born) in [(31.0, "Lisboa", 1.0e10), (99.0, "São Paulo", 1.1e10), (-f64::MAX, "", 1.2e10)] {
        let mut text = city.as_bytes().to_vec();
        text.resize(16, b' ');
        b.f64(age).bytes(&text).f64(born);
    }
    b.buf
}

#[test]
fn reads_an_uncompressed_file_in_both_byte_orders() {
    for big_endian in [false, true] {
        let f = read(&uncompressed(big_endian)).unwrap();
        let ds = &f.dataset;
        assert_eq!(f.compression, Compression::None);
        assert_eq!(f.created, "12 Oct 26 10:20:30");
        assert_eq!(f.product, "@(#) SPSS DATA FILE MK Sigma tests");
        assert_eq!(ds.label.as_deref(), Some("Survey 2026"));
        assert!(f.warnings.is_empty(), "{:?}", f.warnings);

        let names: Vec<&str> = ds.variables().iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["AGE", "CITY", "BORN"]);
        let age = &ds.variables()[0];
        assert_eq!(age.label.as_deref(), Some("Age in years"));
        assert_eq!(age.format.kind, FormatKind::F);
        assert_eq!((age.format.width, age.format.decimals), (3, 0));
        assert_eq!(age.missing, MissingValues::Discrete(vec![Value::Num(99.0)]));
        assert_eq!(ds.variables()[1].var_type, VarType::String(10));
        assert_eq!(ds.variables()[2].format.kind, FormatKind::Date);

        assert_eq!(ds.n_cases(), 3);
        assert_eq!(ds.column(0).unwrap(), [Value::Num(31.0), Value::Num(99.0), Value::SysMis]);
        // Windows-1252 by default: the UTF-8 "ã" reads as two characters, which is correct for a
        // file that doesn't declare its encoding.
        assert_eq!(ds.column(1).unwrap(), [Value::Str("Lisboa".into()), Value::Str("SÃ£o Paulo".into()), Value::Str(String::new())]);
        assert_eq!(ds.column(2).unwrap()[2], Value::Num(1.2e10));
    }
}

#[test]
fn reads_bytecode_compressed_data() {
    let mut b = Builder::new(false);
    b.header(b"$FL2", 3, 1, 0, -1)
        .var(&Var { width: 0, name: "X", label: None, n_missing: 0, missing: &[], print: F8_2 })
        .var(&Var { width: 10, name: "S", label: None, n_missing: 0, missing: &[], print: A10 })
        .end_dictionary();
    // Case 1: X = 7 (code 107), S = "abcdefgh" + "ij" (two raw segments).
    // Case 2: X = 2.5 (raw), S = blanks (two 254s).
    // Case 3: X = system-missing (255), S = "hi" + blanks; then end of data (252).
    b.bytes(&[107, 253, 253, 253, 254, 254, 255, 253]);
    b.bytes(b"abcdefgh").bytes(b"ij      ").f64(2.5).bytes(b"hi      ");
    b.bytes(&[254, 252, 0, 0, 0, 0, 0, 0]);
    let f = read(&b.buf).unwrap();
    assert_eq!(f.compression, Compression::Bytecode);
    assert!(f.warnings.is_empty(), "{:?}", f.warnings);
    assert_eq!(f.dataset.column(0).unwrap(), [Value::Num(7.0), Value::Num(2.5), Value::SysMis]);
    assert_eq!(f.dataset.column(1).unwrap(), [Value::Str("abcdefghij".into()), Value::Str(String::new()), Value::Str("hi".into())]);
}

#[test]
fn reads_labels_long_names_measures_encoding_documents_and_weight() {
    let mut b = Builder::new(false);
    let (low, high, minus_one, one, two) = (b.num(90.0), b.num(99.0), b.num(-1.0), b.num(1.0), b.num(2.0));
    b.header(b"$FL2", 2, 0, 1, 2)
        .var(&Var { width: 0, name: "Q1", label: Some("Satisfação"), n_missing: -3, missing: &[low, high, minus_one], print: F8_2 })
        .var(&Var { width: 2, name: "UF", label: None, n_missing: 1, missing: &[str8("XX")], print: (1 << 16) | (2 << 8) })
        .value_labels(&[(one, "Muito satisfeito"), (two, "Insatisfeito")], &[1])
        .value_labels(&[(str8("SP"), "São Paulo")], &[2])
        .bytes(&{
            let mut doc = Builder::new(false);
            doc.i32(6).i32(1).padded("Coleta de outubro.", 80);
            doc.buf
        })
        .extension(11, 4, &[3i32, 8, 1, 1, 2, 0].iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>())
        .extension(13, 1, b"Q1=Satisfacao_geral\tUF=Estado")
        .extension(20, 1, b"UTF-8")
        .end_dictionary();
    b.f64(1.0).padded("SP", 8).f64(95.0).padded("RJ", 8);

    let f = read(&b.buf).unwrap();
    let ds = &f.dataset;
    let [q1, uf] = ds.variables() else { panic!("two variables") };
    assert_eq!(q1.name, "Satisfacao_geral");
    assert_eq!(uf.name, "Estado");
    assert_eq!(q1.label.as_deref(), Some("Satisfação"));
    assert_eq!(q1.missing, MissingValues::Range { low: 90.0, high: 99.0, discrete: Some(-1.0) });
    assert_eq!(uf.missing, MissingValues::Discrete(vec![Value::Str("XX".into())]));
    assert_eq!(q1.value_label(&Value::Num(1.0)), Some("Muito satisfeito"));
    assert_eq!(uf.value_label(&Value::Str("SP".into())), Some("São Paulo"));
    assert_eq!((q1.measure, uf.measure), (Measure::Scale, Measure::Nominal));
    assert_eq!(ds.documents, ["Coleta de outubro."]);
    assert_eq!(ds.weight, Some(0));
    assert!(q1.is_missing(&ds.column(0).unwrap()[1]));
    assert_eq!(ds.column(1).unwrap(), [Value::Str("SP".into()), Value::Str("RJ".into())]);
}

#[test]
fn rejects_what_is_not_a_system_file() {
    assert_eq!(read(b"PK\x03\x04 not a sav").unwrap_err(), SavError::NotSav);
    assert_eq!(detect(b"$FL3").unwrap(), Flavour::Zsav);
    let mut zsav = uncompressed(false);
    zsav[..4].copy_from_slice(b"$FL3");
    // A $FL3 file must say zlib compression.
    assert!(matches!(read(&zsav), Err(SavError::Invalid { what: "compression code", .. })));
}

#[test]
fn a_truncated_file_never_panics() {
    for file in [uncompressed(false), uncompressed(true)] {
        for len in 0..file.len() {
            let _ = read(&file[..len]);
        }
    }
}

#[test]
fn corrupted_bytes_never_panic() {
    let file = uncompressed(false);
    // A small deterministic generator, so a failure replays.
    let mut state = 0x2545_f491_4f6c_dd1du64;
    for _ in 0..5_000 {
        let mut f = file.clone();
        for _ in 0..4 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let at = (state % f.len() as u64) as usize;
            f[at] = (state >> 32) as u8;
        }
        let _ = read(&f);
    }
}

#[test]
fn an_incomplete_last_case_is_dropped_with_a_warning() {
    let mut file = uncompressed(false);
    file.truncate(file.len() - 8);
    let f = read(&file).unwrap();
    assert_eq!(f.dataset.n_cases(), 2);
    assert_eq!(f.warnings.len(), 2, "{:?}", f.warnings);
}
