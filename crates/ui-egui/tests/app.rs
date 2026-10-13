//! The app driven headless through its accessibility tree: no window, no file dialog.

use std::cell::Cell;
use std::rc::Rc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use mksigma_ui_egui::{Services, SigmaApp, Status, View};

fn harness(app: SigmaApp) -> Harness<'static, SigmaApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1280.0, 800.0)).build_ui_state(|ui, app: &mut SigmaApp| app.ui(ui), app);
    h.run_steps(3);
    h
}

fn shows(h: &Harness<'static, SigmaApp>, label: &str) -> bool {
    h.query_all_by_label(label).next().is_some()
}

/// A minimal uncompressed system file: numeric `score` (F3.0, labelled) and three cases.
fn tiny_sav() -> Vec<u8> {
    let mut b = Vec::new();
    let pad = |b: &mut Vec<u8>, s: &str, n: usize| {
        let mut bytes = s.as_bytes().to_vec();
        bytes.resize(n, b' ');
        b.extend_from_slice(&bytes);
    };
    let int = |b: &mut Vec<u8>, v: i32| b.extend_from_slice(&v.to_le_bytes());
    b.extend_from_slice(b"$FL2");
    pad(&mut b, "@(#) SPSS DATA FILE MK Sigma tests", 60);
    for v in [2, 1, 0, 0, 3] {
        int(&mut b, v);
    }
    b.extend_from_slice(&100.0f64.to_le_bytes());
    pad(&mut b, "12 Oct 26", 9);
    pad(&mut b, "10:00:00", 8);
    pad(&mut b, "", 64 + 3);
    for v in [2, 0, 1, 0, (5 << 16) | (3 << 8), (5 << 16) | (3 << 8)] {
        int(&mut b, v);
    }
    pad(&mut b, "SCORE", 8);
    int(&mut b, 10);
    pad(&mut b, "Test score", 12);
    int(&mut b, 999);
    int(&mut b, 0);
    for x in [70.0f64, 85.0, 90.0] {
        b.extend_from_slice(&x.to_le_bytes());
    }
    b
}

#[test]
fn starts_on_the_sample_in_data_view() {
    let h = harness(SigmaApp::default());
    assert_eq!(h.state().view, View::Data);
    assert!(shows(&h, "income"), "Data View shows the variable names");
    assert!(shows(&h, "1800.00"), "and the values in their format");
}

#[test]
fn variable_view_lists_the_dictionary() {
    let mut h = harness(SigmaApp::default());
    h.get_by_label("Variable View").click();
    h.run_steps(3);
    assert_eq!(h.state().view, View::Variables);
    assert!(shows(&h, "Monthly income"));
    assert!(shows(&h, "Scale"));
    assert!(shows(&h, "F8.2"));
}

#[test]
fn run_puts_tables_in_the_output() {
    let mut h = harness(SigmaApp::default());
    h.state_mut().syntax = "DESCRIPTIVES VARIABLES=age.".into();
    h.get_by_label("▶ Run").click();
    h.run_steps(3);
    assert!(shows(&h, "Descriptive Statistics"));
    assert_eq!(h.state().status, None);

    h.state_mut().syntax = "DESCRIPTIVES VARIABLES=nope.".into();
    h.get_by_label("▶ Run").click();
    h.run_steps(3);
    assert!(matches!(h.state().status, Some(Status::Error(_))));
}

#[test]
fn file_open_uses_the_dialog_and_loads_the_file() {
    let path = std::env::temp_dir().join(format!("mksigma-ui-test-{}.sav", std::process::id()));
    std::fs::write(&path, tiny_sav()).unwrap();
    let asked = Rc::new(Cell::new(0));
    let (count, picked) = (asked.clone(), path.clone());
    let services = Services {
        pick_open: Some(Box::new(move || {
            count.set(count.get() + 1);
            Some(picked.clone())
        })),
    };
    let mut h = harness(SigmaApp::new(services));
    h.get_by_label("File").click();
    h.run_steps(3);
    h.get_by_label("Open…").click();
    h.run_steps(3);
    std::fs::remove_file(&path).unwrap();

    assert_eq!(asked.get(), 1);
    let app = h.state();
    assert_eq!(app.session.dataset.n_cases(), 3);
    assert_eq!(app.file_name.as_deref(), Some(path.file_name().unwrap().to_str().unwrap()));
    assert!(matches!(&app.status, Some(Status::Info(s)) if s.ends_with("3 cases, 1 variables")), "{:?}", app.status);
    assert!(shows(&h, "SCORE"));
    assert!(shows(&h, "85"), "F3.0 shows no decimals");
}

#[test]
fn a_bad_file_is_an_error_not_a_crash() {
    let mut app = SigmaApp::default();
    app.open_bytes("notes.txt", b"hello");
    assert_eq!(app.status, Some(Status::Error("notes.txt: not an SPSS system file".into())));
    assert_eq!(app.session.dataset.n_cases(), 8, "the open dataset stays");
    app.open_bytes("short.sav", &tiny_sav()[..100]);
    assert!(matches!(app.status, Some(Status::Error(_))));
}
