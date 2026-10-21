//! Files built here byte by byte from TS-140, the first one its own sample session's: the reader
//! is checked against the published layout.

use super::*;

/// One 80-byte record: `text`, then blanks.
fn record(text: &[u8]) -> Vec<u8> {
    let mut r = text.to_vec();
    r.resize(RECORD, b' ');
    r
}

/// A NAMESTR descriptor of 140 bytes.
#[allow(clippy::too_many_arguments)]
fn namestr(numeric: bool, length: i16, number: i16, name: &str, label: &str, format: &str, width: i16, position: i32) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend(if numeric { 1i16 } else { 2 }.to_be_bytes());
    d.extend(0i16.to_be_bytes());
    d.extend(length.to_be_bytes());
    d.extend(number.to_be_bytes());
    let fixed = |d: &mut Vec<u8>, s: &str, n: usize| {
        let mut b = s.as_bytes().to_vec();
        b.resize(n, b' ');
        d.extend(b);
    };
    fixed(&mut d, name, 8);
    fixed(&mut d, label, 40);
    fixed(&mut d, format, 8);
    d.extend(width.to_be_bytes());
    d.extend(0i16.to_be_bytes());
    d.extend(0i16.to_be_bytes());
    d.extend([0u8; 2]);
    fixed(&mut d, "", 8);
    d.extend(0i16.to_be_bytes());
    d.extend(0i16.to_be_bytes());
    d.extend(position.to_be_bytes());
    d.resize(140, 0);
    d
}

/// Bytes streamed in 80-byte records, the last padded with blanks.
fn streamed(bytes: &[u8]) -> Vec<u8> {
    let mut b = bytes.to_vec();
    let over = b.len() % RECORD;
    if over != 0 {
        b.resize(b.len() + RECORD - over, b' ');
    }
    b
}

/// A transport file of one member with `vars` and the observations `data`.
fn file(name: &str, vars: &[Vec<u8>], data: &[u8]) -> Vec<u8> {
    let mut f = Vec::new();
    f.extend(record(LIBRARY));
    f.extend(record(b"SAS     SAS     SASLIB  6.06    bsd4.2                          13APR89:10:20:06"));
    f.extend(record(b"13APR89:10:20:06"));
    f.extend(record(b"HEADER RECORD*******MEMBER  HEADER RECORD!!!!!!!000000000000000001600000000140"));
    f.extend(record(b"HEADER RECORD*******DSCRPTR HEADER RECORD!!!!!!!000000000000000000000000000000"));
    f.extend(record(format!("SAS     {name:<8}SASDATA 6.06    bsd4.2                          13APR89:10:20:06").as_bytes()));
    f.extend(record(b"13APR89:10:20:06                Sample data from TS-140"));
    f.extend(record(format!("HEADER RECORD*******NAMESTR HEADER RECORD!!!!!!!000000{:04}00000000000000000000", vars.len()).as_bytes()));
    f.extend(streamed(&vars.concat()));
    f.extend(record(b"HEADER RECORD*******OBS     HEADER RECORD!!!!!!!000000000000000000000000000000"));
    f.extend(streamed(data));
    f
}

/// TS-140's sample session: X (DATE7.) and Y (labelled), the values 1 a, 2 B, . blank, .A *.
fn sample() -> Vec<u8> {
    let vars = [namestr(true, 8, 1, "X", "", "DATE", 7, 0), namestr(false, 8, 2, "Y", "character variable", "", 0, 8)];
    let mut data = Vec::new();
    for (x, y) in [
        ([0x41u8, 0x10, 0, 0, 0, 0, 0, 0], b"a       "),
        ([0x41, 0x20, 0, 0, 0, 0, 0, 0], b"B       "),
        ([b'.', 0, 0, 0, 0, 0, 0, 0], b"        "),
        ([b'A', 0, 0, 0, 0, 0, 0, 0], b"*       "),
    ] {
        data.extend(x);
        data.extend(y);
    }
    file("ABC", &vars, &data)
}

#[test]
fn the_papers_sample_session() {
    let f = read(&sample()).unwrap();
    let ds = &f.dataset;
    assert_eq!((f.name.as_str(), f.created.as_str(), ds.label.as_deref()), ("ABC", "13APR89:10:20:06", Some("Sample data from TS-140")));
    assert_eq!(ds.n_cases(), 4, "the padding is no observation");
    let (x, y) = (&ds.variables()[0], &ds.variables()[1]);
    assert_eq!((x.name.as_str(), x.format.kind), ("X", FormatKind::Date));
    assert_eq!((y.name.as_str(), y.label.as_deref()), ("Y", Some("character variable")));
    // 1 and 2 are days after 1 January 1960: 02JAN60 and 03JAN60.
    let column = ds.column(0).unwrap();
    assert_eq!(column[0], Value::Num(SAS_EPOCH + 86_400.0));
    assert_eq!(column[1], Value::Num(SAS_EPOCH + 2.0 * 86_400.0));
    assert_eq!((column[2].clone(), column[3].clone()), (Value::SysMis, Value::SysMis), ". and .A");
    let texts: Vec<Value> = ds.column(1).unwrap().to_vec();
    assert_eq!(texts, ["a", "B", "", "*"].map(|s| Value::Str(s.into())));
    assert_eq!(f.warnings, [tr!("xpt-warn-special-missing", n = 1)]);
}

#[test]
fn ibm_floating_point() {
    let at = |b: [u8; 8]| ibm(&b).unwrap();
    assert_eq!(at([0x41, 0x10, 0, 0, 0, 0, 0, 0]), 1.0);
    assert_eq!(at([0x41, 0x28, 0, 0, 0, 0, 0, 0]), 2.5);
    assert_eq!(at([0xC1, 0x28, 0, 0, 0, 0, 0, 0]), -2.5);
    assert_eq!(at([0x42, 0x64, 0, 0, 0, 0, 0, 0]), 100.0);
    assert_eq!(at([0x40, 0x19, 0x99, 0x99, 0x99, 0x99, 0x99, 0x9A]), 0.1, "0.1 survives exactly");
    assert_eq!(at([0, 0, 0, 0, 0, 0, 0, 0]), 0.0);
    assert_eq!(ibm(&[0x41, 0x10, 0, 0]), Some(1.0), "a 4-byte number, the rest taken as 0");
    for missing in *b"._AZ" {
        assert_eq!(ibm(&[missing, 0, 0, 0, 0, 0, 0, 0]), None);
    }
}

#[test]
fn numbers_and_text_of_every_kind() {
    let vars = [
        namestr(true, 8, 1, "amount", "Amount paid", "COMMA", 10, 0),
        namestr(true, 4, 2, "count", "", "", 0, 8),
        namestr(false, 6, 3, "city", "", "$", 6, 12),
        namestr(true, 8, 4, "_n", "", "DATETIME", 20, 18),
    ];
    let mut data = Vec::new();
    // 1234.5 = 0x4D2.8 = 16³ · 0.4D28; 3 in 4 bytes; "Ilhéu" in Windows-1252; 60 seconds after 1960.
    data.extend([0x43, 0x4D, 0x28, 0, 0, 0, 0, 0]);
    data.extend([0x41, 0x30, 0, 0]);
    data.extend(b"Ilh\xe9u ");
    data.extend([0x42, 0x3C, 0, 0, 0, 0, 0, 0]);
    let f = read(&file("SALES", &vars, &data)).unwrap();
    let ds = &f.dataset;
    assert_eq!(ds.column(0).unwrap()[0], Value::Num(1234.5));
    assert_eq!(ds.variables()[0].format, Format::new(FormatKind::Comma, 10, 0));
    assert_eq!(ds.column(1).unwrap()[0], Value::Num(3.0));
    assert_eq!(ds.column(2).unwrap()[0], Value::Str("Ilhéu".into()), "Windows-1252 when it isn't UTF-8");
    assert_eq!(ds.column(3).unwrap()[0], Value::Num(SAS_EPOCH + 60.0));
    assert_eq!(ds.variables()[3].name, "v_n", "renamed: SPSS-style names start with a letter");
    assert!(f.warnings.iter().any(|w| w.contains("v_n")));
}

#[test]
fn damaged_files_are_errors_not_crashes() {
    let good = sample();
    for end in 0..good.len() {
        let _ = read(&good[..end]);
    }
    for at in 0..good.len() {
        let mut damaged = good.clone();
        damaged[at] ^= 0xA5;
        let _ = read(&damaged);
    }
    // A NAMESTR header promising 9999 variables.
    let mut many = sample();
    let header = 7 * RECORD;
    many[header + 54..header + 58].copy_from_slice(b"9999");
    assert!(matches!(read(&many), Err(XptError::Truncated { .. })));
    // A variable that claims to sit past every observation.
    let vars = [namestr(true, 8, 1, "X", "", "", 0, i32::MAX)];
    assert!(read(&file("ABC", &vars, &[0x41, 0x10, 0, 0, 0, 0, 0, 0])).is_ok_and(|f| f.dataset.n_cases() == 0));
}

#[test]
fn what_isnt_a_readable_transport_file() {
    assert_eq!(read(b"$FL2 an SPSS file").unwrap_err(), XptError::NotXpt);
    assert_eq!(read(&record(b"HEADER RECORD*******LIBV8   HEADER RECORD!!!!!!!000000000000000000000000000000")).unwrap_err(), XptError::Version8);
    assert!(is_xpt(&sample()) && !is_xpt(b"<stata_dta>"));
}
