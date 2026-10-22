//! Files built here byte by byte from StataCorp's description, the first ones its own examples:
//! the reader is checked against the format, not against a writer of ours.

use super::*;

/// A numeric or string variable as the dictionary records it.
struct Var {
    /// The type code of formats 117–119 (65530 byte … 65526 double, 1–2045 str#, 32768 strL).
    code: u16,
    name: &'static str,
    format: &'static str,
    value_labels: &'static str,
    label: &'static str,
}

fn var(code: u16, name: &'static str, format: &'static str) -> Var {
    Var { code, name, format, value_labels: "", label: "" }
}

/// Writes numbers in one byte order.
struct Bytes {
    big: bool,
    out: Vec<u8>,
}

impl Bytes {
    fn new(big: bool) -> Self {
        Bytes { big, out: Vec::new() }
    }
    fn tag(&mut self, s: &str) -> &mut Self {
        self.out.extend_from_slice(s.as_bytes());
        self
    }
    fn raw(&mut self, b: &[u8]) -> &mut Self {
        self.out.extend_from_slice(b);
        self
    }
    fn u8(&mut self, v: u8) -> &mut Self {
        self.out.push(v);
        self
    }
    fn u16(&mut self, v: u16) -> &mut Self {
        let b = if self.big { v.to_be_bytes() } else { v.to_le_bytes() };
        self.raw(&b)
    }
    fn u32(&mut self, v: u32) -> &mut Self {
        let b = if self.big { v.to_be_bytes() } else { v.to_le_bytes() };
        self.raw(&b)
    }
    fn u64(&mut self, v: u64) -> &mut Self {
        let b = if self.big { v.to_be_bytes() } else { v.to_le_bytes() };
        self.raw(&b)
    }
    fn f32(&mut self, v: f32) -> &mut Self {
        self.u32(v.to_bits())
    }
    fn f64(&mut self, v: f64) -> &mut Self {
        self.u64(v.to_bits())
    }
    /// Text in a field of `width` bytes, `\0` after it when it is shorter, then junk.
    fn fixed(&mut self, s: &str, width: usize) -> &mut Self {
        let mut b = s.as_bytes().to_vec();
        if b.len() < width {
            b.push(0);
        }
        b.resize(width, b'.');
        self.raw(&b)
    }
}

/// A value-label table, as section 5.12 lays it out.
fn table(big: bool, entries: &[(i32, &str)]) -> Vec<u8> {
    let mut t = Bytes::new(big);
    let mut text = Vec::new();
    let mut offsets = Vec::new();
    for (_, label) in entries {
        offsets.push(text.len() as u32);
        text.extend_from_slice(label.as_bytes());
        text.push(0);
    }
    t.u32(entries.len() as u32).u32(text.len() as u32);
    for o in offsets {
        t.u32(o);
    }
    for (v, _) in entries {
        t.u32(*v as u32);
    }
    t.raw(&text);
    t.out
}

/// A 117–119 file: the header, the dictionary, `data` and `strls` as given, and value labels.
fn tagged(release: u16, big: bool, vars: &[Var], n: u64, data: &[u8], strls: &[u8], labels: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let (name, format, label) = if release == 117 { (33, 49, 81) } else { (129, 57, 321) };
    let mut b = Bytes::new(big);
    b.tag("<stata_dta><header><release>").tag(&release.to_string()).tag("</release><byteorder>");
    b.tag(if big { "MSF" } else { "LSF" }).tag("</byteorder><K>");
    if release == 119 {
        b.u32(vars.len() as u32);
    } else {
        b.u16(vars.len() as u16);
    }
    b.tag("</K><N>");
    if release == 117 {
        b.u32(n as u32);
    } else {
        b.u64(n);
    }
    b.tag("</N><label>");
    if release == 117 {
        b.u8(11);
    } else {
        b.u16(11);
    }
    b.tag("Sample Data</label><timestamp>").u8(17).tag("10 Mar 2017 14:23</timestamp></header><map>");
    for _ in 0..14 {
        b.u64(0);
    }
    b.tag("</map><variable_types>");
    for v in vars {
        b.u16(v.code);
    }
    b.tag("</variable_types><varnames>");
    for v in vars {
        b.fixed(v.name, name);
    }
    b.tag("</varnames><sortlist>");
    for _ in 0..=vars.len() {
        if release == 119 {
            b.u32(0);
        } else {
            b.u16(0);
        }
    }
    b.tag("</sortlist><formats>");
    for v in vars {
        b.fixed(v.format, format);
    }
    b.tag("</formats><value_label_names>");
    for v in vars {
        b.fixed(v.value_labels, name);
    }
    b.tag("</value_label_names><variable_labels>");
    for v in vars {
        b.fixed(v.label, label);
    }
    b.tag("</variable_labels><characteristics><ch>").u32(5).raw(b"x\0y\0z").tag("</ch></characteristics><data>");
    b.raw(data).tag("</data><strls>").raw(strls).tag("</strls><value_labels>");
    for (labname, t) in labels {
        b.tag("<lbl>").u32(t.len() as u32).fixed(labname, name).raw(b"...").raw(t).tag("</lbl>");
    }
    b.tag("</value_labels></stata_dta>");
    b.out
}

fn num(ds: &Dataset, column: usize, case: usize) -> f64 {
    ds.column(column).unwrap()[case].as_num().unwrap()
}

fn text(ds: &Dataset, column: usize, case: usize) -> String {
    match &ds.column(column).unwrap()[case] {
        Value::Str(s) => s.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_specifications_first_example() {
    // Section 5: myfloat (float) and myint (int), one observation, both 0, MSF.
    let mut data = Bytes::new(true);
    data.f32(0.0).u16(0);
    let vars = [var(65527, "myfloat", "%9.0g"), var(65529, "myint", "%8.0g")];
    let file = read(&tagged(118, true, &vars, 1, &data.out, b"", &[])).unwrap();
    let ds = &file.dataset;
    assert_eq!((file.release, file.timestamp.as_deref()), (118, Some("10 Mar 2017 14:23")));
    assert_eq!(ds.label.as_deref(), Some("Sample Data"));
    assert_eq!(ds.variables().iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), ["myfloat", "myint"]);
    assert_eq!((num(ds, 0, 0), num(ds, 1, 0)), (0.0, 0.0));
    assert_eq!(ds.variables()[0].format, Format::new(FormatKind::F, 9, 0), "%9.0g on whole numbers");
    assert!(file.warnings.is_empty(), "{:?}", file.warnings);
}

/// Section 5.11's dataset: V1 float, V2 byte, V3 double, V4 str6, V5 strL; obs 2's strL
/// crosslinks to obs 1's when `shared`, and obs 1's is empty, (0, 0), when `empty`.
fn strl_example(release: u16, big: bool, shared: bool, empty: bool) -> Vec<u8> {
    let vo = |b: &mut Bytes, v: u64, o: u64| {
        let bits = match release {
            117 => 32,
            118 => 16,
            _ => 24,
        };
        let raw = if big { (v << (64 - bits)) | o } else { v | (o << bits) };
        b.u64(raw);
    };
    let gso = |b: &mut Bytes, v: u32, o: u64, s: &str| {
        b.tag("GSO").u32(v);
        if release == 117 {
            b.u32(o as u32);
        } else {
            b.u64(o);
        }
        b.u8(130).u32(s.len() as u32 + 1).tag(s).u8(0);
    };
    let mut data = Bytes::new(big);
    data.f32(0.0).u8(1).f64(2.0).raw(b"first\0");
    if empty {
        vo(&mut data, 0, 0)
    } else {
        vo(&mut data, 5, 1)
    }
    data.f32(1.0).u8(2).f64(3.0).raw(b"second");
    if shared {
        vo(&mut data, 5, 1)
    } else {
        vo(&mut data, 5, 2)
    }
    let mut strls = Bytes::new(big);
    if !empty {
        gso(&mut strls, 5, 1, "third");
    }
    if !shared {
        gso(&mut strls, 5, 2, "fourth");
    }
    let vars = [var(65527, "V1", "%9.0g"), var(65530, "V2", "%8.0g"), var(65526, "V3", "%10.0g"), var(6, "V4", "%9s"), var(32768, "V5", "%9s")];
    tagged(release, big, &vars, 2, &data.out, &strls.out, &[])
}

#[test]
fn every_tagged_format_in_both_byte_orders() {
    for release in [117, 118, 119] {
        for big in [true, false] {
            let file = read(&strl_example(release, big, false, false)).unwrap_or_else(|e| panic!("{release} {big}: {e}"));
            let ds = &file.dataset;
            assert_eq!(ds.n_cases(), 2);
            assert_eq!((num(ds, 0, 1), num(ds, 1, 1), num(ds, 2, 1)), (1.0, 2.0, 3.0), "{release} {big}");
            assert_eq!((text(ds, 3, 0), text(ds, 3, 1)), ("first".to_string(), "second".to_string()), "a full-width str6 has no \\0");
            assert_eq!((text(ds, 4, 0), text(ds, 4, 1)), ("third".to_string(), "fourth".to_string()), "{release} {big}");
            assert_eq!(ds.variables()[3].var_type, VarType::String(6));
            assert_eq!(ds.variables()[4].var_type, VarType::String(6), "a strL as wide as its longest value");
        }
    }
    // A crosslinked strL, and the empty one (0, 0).
    let ds = read(&strl_example(118, false, true, false)).unwrap().dataset;
    assert_eq!((text(&ds, 4, 0), text(&ds, 4, 1)), ("third".to_string(), "third".to_string()));
    let ds = read(&strl_example(118, true, false, true)).unwrap().dataset;
    assert_eq!((text(&ds, 4, 0), text(&ds, 4, 1)), (String::new(), "fourth".to_string()));
}

#[test]
fn missing_values_and_floats() {
    let mut data = Bytes::new(false);
    // Observation 1: valid; 2: `.`; 3: `.a`.
    data.u8(100).u16(32740).u32(2_147_483_620).f32(0.1).f64(-2.5);
    data.u8(101).u16(32741).u32(2_147_483_621).u32(0x7f00_0000).u64(0x7fe0_0000_0000_0000);
    data.u8(102).u16(32742).u32(2_147_483_622).u32(0x7f00_0800).u64(0x7fe0_0100_0000_0000);
    let vars = [var(65530, "b", "%8.0g"), var(65529, "i", "%8.0g"), var(65528, "l", "%12.0g"), var(65527, "f", "%9.0g"), var(65526, "d", "%10.0g")];
    let file = read(&tagged(118, false, &vars, 3, &data.out, b"", &[])).unwrap();
    let ds = &file.dataset;
    assert_eq!((num(ds, 0, 0), num(ds, 1, 0), num(ds, 2, 0)), (100.0, 32740.0, 2_147_483_620.0));
    assert_eq!(num(ds, 3, 0), 0.1, "a float reads as the decimal Stata shows");
    assert_eq!(num(ds, 4, 0), -2.5);
    for case in [1, 2] {
        for column in 0..5 {
            assert_eq!(ds.column(column).unwrap()[case], Value::SysMis, "{column} {case}");
        }
    }
    assert_eq!(file.warnings, [tr!("dta-warn-extended-missing", n = 5)], "the five .a, not the five .");
    assert_eq!(ds.variables()[3].format, Format::new(FormatKind::F, 9, 2), "%9.0g with a fraction");
}

#[test]
fn value_labels_variable_labels_and_formats() {
    // Section 5.12's table: 1 yes, 2 no, MSF.
    let mut data = Bytes::new(true);
    data.u8(1).f64(1234.5).u8(2).f64(-3.0);
    let vars = [
        Var { code: 65530, name: "answer", format: "%8.0g", value_labels: "yesno", label: "Did it work?" },
        Var { code: 65526, name: "amount", format: "%10.2fc", value_labels: "", label: "" },
    ];
    let file = read(&tagged(118, true, &vars, 2, &data.out, b"", &[("yesno", table(true, &[(1, "yes"), (2, "no")]))])).unwrap();
    let answer = &file.dataset.variables()[0];
    assert_eq!(answer.value_labels, [(Value::Num(1.0), "yes".to_string()), (Value::Num(2.0), "no".to_string())]);
    assert_eq!((answer.label.as_deref(), answer.measure), (Some("Did it work?"), Measure::Nominal));
    let amount = &file.dataset.variables()[1];
    assert_eq!((amount.format, amount.measure), (Format::new(FormatKind::Comma, 10, 2), Measure::Scale));
    // The table's text in another order is the same table.
    let shuffled = {
        let mut t = Bytes::new(true);
        t.u32(2).u32(7).u32(3).u32(0).u32(1).u32(2).raw(b"no\0yes\0");
        t.out
    };
    let file = read(&tagged(118, true, &vars, 2, &data.out, b"", &[("yesno", shuffled)])).unwrap();
    assert_eq!(file.dataset.variables()[0].value_labels[0].1, "yes");
}

#[test]
fn dates_count_from_1582() {
    let mut data = Bytes::new(false);
    // 2 January 1960; 1960-01-01 00:00:01; February 1960; the second quarter of 1960.
    data.u32(1).f64(1000.0).u16(1).u16(1);
    let vars = [var(65528, "day", "%td"), var(65526, "moment", "%tc"), var(65529, "month", "%tm"), var(65529, "quarter", "%tq")];
    let ds = read(&tagged(118, false, &vars, 1, &data.out, b"", &[])).unwrap().dataset;
    let epoch = 137_775.0 * 86_400.0;
    assert_eq!(num(&ds, 0, 0), epoch + 86_400.0);
    assert_eq!(num(&ds, 1, 0), epoch + 1.0);
    assert_eq!(num(&ds, 2, 0), epoch + 31.0 * 86_400.0);
    assert_eq!(num(&ds, 3, 0), epoch + 91.0 * 86_400.0, "1 April 1960, a leap year");
    let kinds: Vec<FormatKind> = ds.variables().iter().map(|v| v.format.kind).collect();
    assert_eq!(kinds, [FormatKind::Date, FormatKind::DateTime, FormatKind::MoYr, FormatKind::QYr]);
    assert_eq!(days_from_1960(2000, 3, 1), 14_670);
}

#[test]
fn dates_out_of_range_are_missing_not_a_crash() {
    // A month and a quarter of 1e300, and a month that is fine (#23).
    let mut data = Bytes::new(false);
    data.f64(1e300).f64(-1e300).f64(1.0);
    let vars = [var(65526, "m", "%tm"), var(65526, "q", "%tq"), var(65526, "ok", "%tm")];
    let file = read(&tagged(118, false, &vars, 1, &data.out, b"", &[])).unwrap();
    assert_eq!(file.dataset.column(0).unwrap()[0], Value::SysMis);
    assert_eq!(file.dataset.column(1).unwrap()[0], Value::SysMis);
    assert_eq!(num(&file.dataset, 2, 0), 137_775.0 * 86_400.0 + 31.0 * 86_400.0);
    assert_eq!(file.warnings, [tr!("dta-warn-date-range", n = 2)]);
    // Writing a MOYR or QYR far out of range.
    let mut ds = Dataset::new();
    for (name, kind) in [("m", FormatKind::MoYr), ("q", FormatKind::QYr)] {
        let mut v = Variable::numeric(name);
        v.format = mksigma_core::Format::new(kind, 8, 0);
        ds.add_variable(v).unwrap();
    }
    ds.push_case(vec![Value::Num(1e24), Value::Num(-1e300)]).unwrap();
    ds.push_case(vec![Value::Num(137_775.0 * 86_400.0), Value::Num(137_775.0 * 86_400.0)]).unwrap();
    let written = write(&ds, "").unwrap();
    assert_eq!(written.warnings, [tr!("dta-warn-date-range-written")]);
    let back = read(&written.bytes).unwrap().dataset;
    assert_eq!(back.column(0).unwrap(), [Value::SysMis, Value::Num(137_775.0 * 86_400.0)]);
    assert_eq!(back.column(1).unwrap(), [Value::SysMis, Value::Num(137_775.0 * 86_400.0)]);
    // No overflow at the extremes of the day count.
    for days in [i64::MIN, i64::MAX] {
        assert!((1..=12).contains(&civil(days).1));
    }
}

#[test]
fn no_variables_and_a_huge_n_reads_no_cases() {
    // Format 118: K = 0, N = 2^62.
    let file = read(&tagged(118, false, &[], 1 << 62, b"", b"", &[])).unwrap();
    assert_eq!((file.dataset.n_cases(), file.dataset.variables().len()), (0, 0));
    // Format 114: nvar = 0, nobs = 0xFFFFFFFF.
    let mut b = Bytes::new(false);
    b.u8(114).u8(2).u8(1).u8(0).u16(0).u32(u32::MAX);
    b.fixed("", 81).fixed("", 18);
    b.u16(0);
    b.u8(0).u32(0);
    assert_eq!(read(&b.out).map(|f| f.dataset.n_cases()), Ok(0));
}

#[test]
fn names_stata_allows_and_spss_style_names_dont() {
    let mut data = Bytes::new(false);
    data.u8(1).u8(2).u8(3);
    let vars = [var(65530, "_merge", "%8.0g"), var(65530, "X", "%8.0g"), var(65530, "x", "%8.0g")];
    let file = read(&tagged(118, false, &vars, 1, &data.out, b"", &[])).unwrap();
    let names: Vec<&str> = file.dataset.variables().iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["v_merge", "X", "x_2"]);
    assert_eq!(file.warnings.len(), 2, "{:?}", file.warnings);
}

/// A 113–115 file: two variables (an int labelled by `sex`, a str5), two observations.
fn binary(release: u8, big: bool) -> Vec<u8> {
    let format = if release == 113 { 12 } else { 49 };
    let mut b = Bytes::new(big);
    b.u8(release).u8(if big { 1 } else { 2 }).u8(1).u8(0).u16(2).u32(2);
    b.fixed("Old data", 81).fixed("10 Mar 2005 09:15", 18);
    b.u8(252).u8(5);
    b.fixed("sex", 33).fixed("name", 33);
    b.u16(0).u16(0).u16(0);
    b.fixed("%8.0g", format).fixed("%5s", format);
    b.fixed("sex", 33).fixed("", 33);
    b.fixed("Sex", 81).fixed("Name", 81);
    // An expansion field, then the end of them.
    b.u8(1).u32(3).raw(b"abc").u8(0).u32(0);
    b.u16(1).raw(b"Ana\0\0").u16(2).raw(b"Bruno");
    let t = table(big, &[(1, "female"), (2, "male")]);
    b.u32(t.len() as u32).fixed("sex", 33).raw(b"...").raw(&t);
    b.out
}

#[test]
fn the_binary_formats_of_stata_8_to_12() {
    for release in [113, 114, 115] {
        for big in [true, false] {
            let file = read(&binary(release, big)).unwrap_or_else(|e| panic!("{release} {big}: {e}"));
            let ds = &file.dataset;
            assert_eq!(
                (file.release, ds.label.as_deref(), file.timestamp.as_deref()),
                (u16::from(release), Some("Old data"), Some("10 Mar 2005 09:15"))
            );
            assert_eq!((num(ds, 0, 0), text(ds, 1, 0), text(ds, 1, 1)), (1.0, "Ana".to_string(), "Bruno".to_string()));
            let sex = &ds.variables()[0];
            assert_eq!((sex.label.as_deref(), sex.value_labels.len()), (Some("Sex"), 2));
        }
    }
    // Text before format 118 is in Windows-1252.
    let mut old = binary(115, false);
    let at = old.windows(5).position(|w| w == b"Bruno").unwrap();
    old[at + 4] = 0xE9;
    assert_eq!(text(&read(&old).unwrap().dataset, 1, 1), "Brun\u{e9}");
}

#[test]
fn damaged_files_are_errors_not_crashes() {
    // Every prefix of a valid file.
    for file in [strl_example(118, false, false, false), strl_example(117, true, true, false), binary(115, false)] {
        for end in 0..file.len() {
            let _ = read(&file[..end]);
        }
        // Every single byte changed.
        for at in 0..file.len() {
            let mut damaged = file.clone();
            damaged[at] ^= 0xA5;
            let _ = read(&damaged);
        }
    }
    // A header that promises far more observations than the file holds.
    let mut data = Bytes::new(false);
    data.u8(1);
    let huge = tagged(118, false, &[var(65530, "x", "%8.0g")], u64::MAX / 2, &data.out, b"", &[]);
    assert!(matches!(read(&huge), Err(DtaError::Truncated { .. } | DtaError::Invalid { .. })));
    // A value-label table claiming a billion entries.
    let mut t = Bytes::new(false);
    t.u32(1_000_000_000).u32(0);
    assert!(read(&tagged(118, false, &[var(65530, "x", "%8.0g")], 1, &data.out, b"", &[("x", t.out)])).is_err());
    // A strL that points to a string never defined.
    let mut data = Bytes::new(false);
    data.u64(5 | (9 << 16));
    assert!(read(&tagged(118, false, &[var(32768, "s", "%9s")], 1, &data.out, b"", &[])).is_err());
}

#[test]
fn what_isnt_a_readable_stata_file() {
    assert_eq!(read(b"$FL2 an SPSS file").unwrap_err(), DtaError::NotDta);
    assert_eq!(read(&[105, 2, 1, 0]).unwrap_err(), DtaError::Release(105));
    let mut file = strl_example(118, false, false, false);
    file[28..31].copy_from_slice(b"120");
    assert_eq!(read(&file).unwrap_err(), DtaError::Release(120));
    assert!(is_dta(&strl_example(119, true, false, false)) && is_dta(&binary(114, true)) && !is_dta(b"age,score\n"));
}

/// A dataset with every kind of variable `write` handles.
fn everything() -> Dataset {
    let mut ds = Dataset::new();
    ds.label = Some("Turma de 2024".into());
    let mut grupo = Variable::numeric("grupo").with_label("Grupo");
    grupo.value_labels = vec![(Value::Num(1.0), "Controle".into()), (Value::Num(2.0), "Tratamento".into())];
    grupo.missing = mksigma_core::MissingValues::Discrete(vec![Value::Num(9.0)]);
    let mut dia = Variable::numeric("dia");
    dia.format = Format::new(FormatKind::Date, 11, 0);
    let mut momento = Variable::numeric("momento");
    momento.format = Format::new(FormatKind::DateTime, 20, 0);
    let mut mes = Variable::numeric("mes");
    mes.format = Format::new(FormatKind::MoYr, 8, 0);
    for var in [
        grupo,
        Variable::numeric("contagem"),
        Variable::numeric("populacao"),
        Variable::numeric("renda"),
        Variable::string("nome", 12),
        Variable::string("relato", 3000),
        dia,
        momento,
        mes,
        Variable::numeric("@peso#"),
        Variable::numeric("int"),
    ] {
        ds.add_variable(var).unwrap();
    }
    let epoch = 137_775.0 * 86_400.0;
    let long = "á".repeat(1200);
    for (g, c, p, r, nome, relato, d) in [
        (1.0, 30000.0, 3_000_000.0, 1234.56, "Ana", long.as_str(), 0.0),
        (2.0, -5.0, 7.0, -0.1, "Bárbara", "", 1.0),
        (9.0, 0.0, 0.0, 0.0, "", "curto", 366.0),
    ] {
        ds.push_case(vec![
            Value::Num(g),
            Value::Num(c),
            Value::Num(p),
            Value::Num(r),
            Value::Str(nome.into()),
            Value::Str(relato.into()),
            Value::Num(epoch + d * 86_400.0),
            Value::Num(epoch + d * 86_400.0 + 3_723.0),
            Value::Num(epoch + 31.0 * 86_400.0),
            Value::Num(1.0),
            Value::Num(2.0),
        ])
        .unwrap();
    }
    ds.push_case(
        vec![Value::SysMis; 11].into_iter().enumerate().map(|(j, v)| if j == 4 || j == 5 { Value::Str(String::new()) } else { v }).collect(),
    )
    .unwrap();
    ds
}

#[test]
fn a_written_file_reads_back() {
    let ds = everything();
    let written = write(&ds, "20 Oct 2026 14:05").unwrap();
    let back = read(&written.bytes).unwrap();
    assert_eq!((back.release, back.timestamp.as_deref(), back.dataset.label.as_deref()), (118, Some("20 Oct 2026 14:05"), Some("Turma de 2024")));
    let b = &back.dataset;
    let names: Vec<&str> = b.variables().iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["grupo", "contagem", "populacao", "renda", "nome", "relato", "dia", "momento", "mes", "v_peso_", "int_"]);
    assert_eq!(b.n_cases(), 4);
    for (j, var) in ds.variables().iter().enumerate() {
        for case in 0..4 {
            let original = &ds.column(j).unwrap()[case];
            let user_missing = var.is_missing(original) && *original != Value::SysMis;
            let expected = if user_missing { Value::SysMis } else { original.clone() };
            assert_eq!(b.column(j).unwrap()[case], expected, "{} case {case}", var.name);
        }
    }
    let grupo = &b.variables()[0];
    assert_eq!((grupo.label.as_deref(), grupo.value_labels.len()), (Some("Grupo"), 2));
    assert_eq!(b.variables()[6].format.kind, FormatKind::Date);
    assert_eq!(b.variables()[7].format.kind, FormatKind::DateTime);
    assert_eq!(b.variables()[8].format.kind, FormatKind::MoYr);
    // Renamed twice, user-missing once.
    let renamed = written.warnings.iter().filter(|w| w.contains("renamed")).count();
    assert_eq!(renamed, 2, "{:?}", written.warnings);
    assert!(written.warnings.contains(&tr!("dta-warn-user-missing")));
}

#[test]
fn the_written_file_follows_the_description() {
    let written = write(&everything(), "").unwrap();
    let bytes = &written.bytes;
    assert!(
        bytes.starts_with(
            b"<stata_dta><header><release>118</release><byteorder>LSF</byteorder><K>\x0b\x00</K><N>\x04\x00\x00\x00\x00\x00\x00\x00</N>"
        )
    );
    // The map: where each section starts, then the end of the file.
    let at = bytes.windows(5).position(|w| w == b"<map>").unwrap() + 5;
    let map: Vec<usize> = (0..14).map(|k| u64::from_le_bytes(bytes[at + 8 * k..at + 8 * k + 8].try_into().unwrap()) as usize).collect();
    let tags = [
        "<stata_dta>",
        "<map>",
        "<variable_types>",
        "<varnames>",
        "<sortlist>",
        "<formats>",
        "<value_label_names>",
        "<variable_labels>",
        "<characteristics>",
        "<data>",
        "<strls>",
        "<value_labels>",
        "</stata_dta>",
    ];
    for (position, tag) in map.iter().zip(tags) {
        assert!(bytes[*position..].starts_with(tag.as_bytes()), "{tag} at {position}");
    }
    assert_eq!(map[13], bytes.len());
    // Storage: grupo byte, contagem int (30000), populacao long, renda double (fractions),
    // nome str8 ("Bárbara" is 8 bytes in UTF-8), relato strL (2400 bytes), dia int (366 days),
    // momento double (milliseconds), mes byte (month 1), and the last two byte.
    let types = map[2] + "<variable_types>".len();
    let codes: Vec<u16> = (0..11).map(|k| u16::from_le_bytes([bytes[types + 2 * k], bytes[types + 2 * k + 1]])).collect();
    assert_eq!(codes, [65530, 65529, 65528, 65526, 8, 32768, 65529, 65526, 65530, 65530, 65530]);
}

#[test]
fn time_stamps_as_stata_writes_them() {
    assert_eq!(timestamp(0), "01 Jan 1970 00:00");
    assert_eq!(timestamp(1_792_505_100), "20 Oct 2026 14:05");
    assert_eq!(civil(-3653), (1960, 1, 1));
}

#[test]
fn what_stata_cant_hold() {
    let mut ds = Dataset::new();
    let mut s = Variable::string("s", 4);
    s.value_labels = vec![(Value::Str("a".into()), "A".into())];
    let mut x = Variable::numeric("x").with_label("l".repeat(100));
    x.value_labels = vec![(Value::Num(1.5), "one and a half".into()), (Value::Num(2.0), "two".into())];
    ds.add_variable(s).unwrap();
    ds.add_variable(x).unwrap();
    ds.push_case(vec![Value::Str("a".into()), Value::Num(2.0)]).unwrap();
    let written = write(&ds, "").unwrap();
    assert!(written.warnings.contains(&tr!("dta-warn-labels-dropped")) && written.warnings.contains(&tr!("dta-warn-labels-cut", n = 80)));
    let back = read(&written.bytes).unwrap().dataset;
    assert_eq!(back.variables()[1].value_labels, [(Value::Num(2.0), "two".to_string())]);
    assert_eq!(back.variables()[1].label.as_deref().map(str::len), Some(80));
    // Too many variables for the format.
    assert_eq!(writer::variable_count(32_767), Ok(32_767));
    assert_eq!(writer::variable_count(32_768), Err(DtaError::TooManyVariables(32_768)));
}
