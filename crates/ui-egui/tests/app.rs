//! The app driven headless through its accessibility tree: no window, no file dialog.

use std::cell::Cell;
use std::rc::Rc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use mksigma_ui_egui::{Services, SigmaApp, Status, View};

fn harness(app: SigmaApp) -> Harness<'static, SigmaApp> {
    sized(app, 1280.0)
}

/// Variable View's rightmost columns need a wide window to be on screen without scrolling.
fn sized(app: SigmaApp, width: f32) -> Harness<'static, SigmaApp> {
    let mut h = Harness::builder().with_size(egui::vec2(width, 800.0)).build_ui_state(|ui, app: &mut SigmaApp| app.ui(ui), app);
    h.run_steps(3);
    h
}

fn combo_boxes(h: &Harness<'static, SigmaApp>) -> usize {
    h.query_all_by_role(egui::accesskit::Role::ComboBox).count()
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
    assert_eq!(combo_boxes(&h), 3, "one measurement level box per variable");
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

fn press(h: &mut Harness<'static, SigmaApp>, key: egui::Key) {
    h.key_press(key);
    h.run_steps(3);
}

/// Clicks a cell's text and replaces what is being typed.
fn edit(h: &mut Harness<'static, SigmaApp>, shown: &str, typed: &str) {
    h.get_all_by_label(shown).next().unwrap().click();
    h.run_steps(3);
    h.state_mut().editing.as_mut().expect("editing after a click").text = typed.into();
    h.run_steps(1);
}

#[test]
fn editing_a_label_in_variable_view_runs_syntax() {
    use mksigma_ui_egui::{Field, Target};
    let mut h = harness(SigmaApp::default());
    h.state_mut().view = View::Variables;
    h.run_steps(3);
    edit(&mut h, "Monthly income", "Renda mensal");
    assert_eq!(h.state().editing.as_ref().map(|e| e.target), Some(Target::Variable(1, Field::Label)));
    press(&mut h, egui::Key::Enter);

    let app = h.state();
    assert_eq!(app.editing.as_ref().map(|e| e.target), None);
    assert_eq!(app.session.dataset.variables()[1].label.as_deref(), Some("Renda mensal"));
    assert_eq!(app.session.journal, ["VARIABLE LABELS income 'Renda mensal'."]);
    assert_eq!(app.status, Some(Status::Info("VARIABLE LABELS income 'Renda mensal'.".into())));
}

#[test]
fn escape_cancels_an_edit() {
    let mut h = harness(SigmaApp::default());
    h.state_mut().view = View::Variables;
    h.run_steps(3);
    edit(&mut h, "Test score", "changed");
    press(&mut h, egui::Key::Escape);
    assert!(h.state().editing.is_none());
    assert_eq!(h.state().session.dataset.variables()[2].label.as_deref(), Some("Test score"));
    assert!(h.state().session.journal.is_empty());
}

#[test]
fn rename_and_bad_format_from_variable_view() {
    let mut h = sized(SigmaApp::default(), 1800.0);
    h.state_mut().view = View::Variables;
    h.run_steps(3);
    edit(&mut h, "score", "nota");
    press(&mut h, egui::Key::Enter);
    assert_eq!(h.state().session.dataset.variables()[2].name, "nota");

    edit(&mut h, "F8.2", "X9");
    press(&mut h, egui::Key::Enter);
    assert!(matches!(&h.state().status, Some(Status::Error(e)) if e.contains("X9 is not a format")), "{:?}", h.state().status);
    assert_eq!(h.state().session.journal, ["RENAME VARIABLES (score = nota)."]);
}

#[test]
fn measure_combo_box() {
    let mut h = sized(SigmaApp::default(), 1800.0);
    h.state_mut().view = View::Variables;
    h.run_steps(3);
    h.query_all_by_role(egui::accesskit::Role::ComboBox).next().unwrap().click();
    h.run_steps(3);
    h.get_by_label("Ordinal").click();
    h.run_steps(3);
    assert_eq!(h.state().session.dataset.variables()[0].measure, mksigma_engine::core::Measure::Ordinal);
    assert_eq!(h.state().session.journal, ["VARIABLE LEVEL age (ORDINAL)."]);
}

#[test]
fn value_labels_dialog() {
    use mksigma_engine::core::Value;
    let mut h = harness(SigmaApp::default());
    h.state_mut().value_labels_dialog =
        Some(mksigma_ui_egui::ValueLabelsDialog { var: 0, rows: vec![("18".into(), "Adult".into()), ("".into(), "skipped".into())] });
    h.run_steps(3);
    assert!(shows(&h, "Value Labels: age"));
    h.get_by_label("OK").click();
    h.run_steps(3);
    let app = h.state();
    assert!(app.value_labels_dialog.is_none());
    assert_eq!(app.session.dataset.variables()[0].value_label(&Value::Num(18.0)), Some("Adult"));
    assert_eq!(app.session.journal, ["VALUE LABELS age 18 'Adult'."]);
}

#[test]
fn editing_a_data_cell() {
    use mksigma_engine::core::Value;
    let mut h = harness(SigmaApp::default());
    edit(&mut h, "1800.00", "1950.5");
    press(&mut h, egui::Key::Enter);
    assert_eq!(h.state().session.dataset.column(1).unwrap()[0], Value::Num(1950.5));
    assert!(h.state().session.journal.is_empty(), "cell edits aren't syntax");

    edit(&mut h, "4200.00", "lots");
    press(&mut h, egui::Key::Enter);
    assert!(matches!(&h.state().status, Some(Status::Error(e)) if e.contains("lots is not a number")));
    assert_eq!(h.state().session.dataset.column(1).unwrap()[1], Value::Num(4200.0));
}

#[test]
fn history_pastes_into_the_syntax_editor() {
    let mut h = harness(SigmaApp::default());
    h.state_mut().syntax = "DESCRIPTIVES age.".into();
    h.get_by_label("▶ Run").click();
    h.run_steps(3);
    h.get_by_label("History (1)").click();
    h.run_steps(3);
    h.get_by_label("Paste into syntax").click();
    h.run_steps(3);
    assert_eq!(h.state().syntax, "DESCRIPTIVES age.\nDESCRIPTIVES age.");
}
