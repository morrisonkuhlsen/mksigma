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
        ..Services::default()
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

fn with_steps() -> Harness<'static, SigmaApp> {
    let mut app = SigmaApp::default();
    app.session.run_syntax("NUMERIC bonus.\nCOMPUTE bonus = score / 10.\nSELECT IF (bonus > 7).\nDESCRIPTIVES age.").unwrap();
    harness(app)
}

#[test]
fn the_steps_panel_lists_data_changes_only() {
    let h = with_steps();
    assert!(shows(&h, "Applied steps (3)"), "DESCRIPTIVES is not a step");
    assert!(shows(&h, "SELECT IF (bonus > 7)."));
    assert_eq!(h.state().session.dataset.n_cases(), 5);
}

#[test]
fn switching_a_step_off_and_on_replays_the_data() {
    let mut h = with_steps();
    // The first checkbox is Data View's "Value labels"; the steps' follow in order.
    let filter = || egui::accesskit::Role::CheckBox;
    h.query_all_by_role(filter()).nth(3).unwrap().click();
    h.run_steps(3);
    assert!(!h.state().session.steps()[2].enabled);
    assert_eq!(h.state().session.dataset.n_cases(), 8);
    h.query_all_by_role(filter()).nth(3).unwrap().click();
    h.run_steps(3);
    assert_eq!(h.state().session.dataset.n_cases(), 5);
}

#[test]
fn remove_and_move_steps() {
    let mut h = with_steps();
    h.get_all_by_label("remove").nth(2).unwrap().click();
    h.run_steps(3);
    assert_eq!(h.state().session.steps().len(), 2);
    assert_eq!(h.state().session.dataset.n_cases(), 8);
    // COMPUTE before NUMERIC: COMPUTE makes the variable, so NUMERIC fails and is skipped.
    h.get_all_by_label("up").nth(1).unwrap().click();
    h.run_steps(3);
    let steps = h.state().session.steps();
    assert_eq!(steps[0].label(), "COMPUTE bonus = score / 10.");
    assert!(steps[1].error.as_deref().is_some_and(|e| e.contains("already exists")), "{:?}", steps[1].error);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run_steps(3);
    assert_eq!(h.state().session.steps()[0].label(), "NUMERIC bonus.", "the move is undoable");
}

#[test]
fn rewriting_a_step() {
    let mut h = with_steps();
    edit(&mut h, "SELECT IF (bonus > 7).", "SELECT IF (bonus > 7.5).");
    press(&mut h, egui::Key::Enter);
    assert_eq!(h.state().session.dataset.n_cases(), 3);
    edit(&mut h, "SELECT IF (bonus > 7.5).", "SELECT IF (.");
    press(&mut h, egui::Key::Enter);
    assert!(matches!(h.state().status, Some(Status::Error(_))));
    assert_eq!(h.state().session.steps()[2].label(), "SELECT IF (bonus > 7.5).", "a bad rewrite changes nothing");
}

#[test]
fn steps_paste_into_the_syntax_editor() {
    let mut h = with_steps();
    h.state_mut().syntax.clear();
    h.get_by_label("Paste steps into syntax").click();
    h.run_steps(3);
    assert_eq!(h.state().syntax, "NUMERIC bonus.\nCOMPUTE bonus = score / 10.\nSELECT IF (bonus > 7).");
}

fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("mksigma-{}-{name}", std::process::id()))
}

#[test]
fn save_as_writes_a_file_that_reopens_the_same() {
    let path = temp_path("save-as.sav");
    let picked = path.clone();
    let suggested = Rc::new(std::cell::RefCell::new(String::new()));
    let seen = suggested.clone();
    let services = Services {
        pick_save: Some(Box::new(move |s: &str| {
            *seen.borrow_mut() = s.to_string();
            Some(picked.clone())
        })),
        now: Some(Box::new(|| 951_782_400)),
        ..Services::default()
    };
    let mut h = harness(SigmaApp::new(services));
    h.state_mut().run_edit("VARIABLE LABELS age 'Idade'.");
    h.run_steps(2);
    assert!(h.state().session.modified);
    h.get_by_label("File").click();
    h.run_steps(3);
    h.get_by_label("Save").click();
    h.run_steps(3);

    let app = h.state();
    assert_eq!(*suggested.borrow(), "Untitled.sav", "the sample has no file yet, so Save asks where");
    assert!(!app.session.modified);
    assert_eq!(app.file_path.as_deref(), Some(path.as_path()));
    assert!(matches!(&app.status, Some(Status::Info(s)) if s.starts_with("Saved ")), "{:?}", app.status);
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let mut reopened = SigmaApp::default();
    assert!(reopened.open_bytes("again.sav", &bytes));
    assert_eq!(reopened.session.dataset.variables(), app.session.dataset.variables());
    assert_eq!(reopened.session.dataset.column(1), app.session.dataset.column(1));
}

#[test]
fn save_goes_back_to_the_opened_file_without_asking() {
    let path = temp_path("opened.sav");
    std::fs::write(&path, tiny_sav()).unwrap();
    let asked = Rc::new(Cell::new(0));
    let count = asked.clone();
    let services = Services {
        pick_save: Some(Box::new(move |_: &str| {
            count.set(count.get() + 1);
            None
        })),
        ..Services::default()
    };
    let mut app = SigmaApp::new(services);
    app.open_path(&path);
    app.run_edit("VARIABLE LABELS SCORE 'Nota'.");
    app.save();
    assert_eq!(asked.get(), 0);
    let mut again = SigmaApp::default();
    again.open_path(&path);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(again.session.dataset.variables()[0].label.as_deref(), Some("Nota"));
}

#[test]
fn a_failed_save_keeps_the_changes_marked_unsaved() {
    let mut app = SigmaApp::default();
    app.run_edit("VARIABLE LABELS age 'Idade'.");
    app.save_to(std::path::Path::new("/nonexistent-dir/x.sav"));
    assert!(matches!(&app.status, Some(Status::Error(_))));
    assert!(app.session.modified);
    assert_eq!(app.file_path, None);
}

#[test]
fn ctrl_z_undoes_and_ctrl_shift_z_redoes() {
    let mut h = harness(SigmaApp::default());
    h.state_mut().run_edit("VARIABLE LABELS age 'Idade'.");
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run_steps(3);
    assert_eq!(h.state().session.dataset.variables()[0].label.as_deref(), Some("Age (years)"));
    assert_eq!(h.state().status, Some(Status::Info("Undid VARIABLE LABELS age 'Idade'.".into())));
    assert!(h.state().session.journal.is_empty());
    h.key_press_modifiers(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z);
    h.run_steps(3);
    assert_eq!(h.state().session.dataset.variables()[0].label.as_deref(), Some("Idade"));
}

#[test]
fn edit_menu_names_what_it_undoes() {
    let mut h = harness(SigmaApp::default());
    h.state_mut().session.set_cell(0, 0, "30").unwrap();
    h.run_steps(2);
    h.get_by_label("Edit").click();
    h.run_steps(3);
    h.get_by_label("Undo cell edit").click();
    h.run_steps(3);
    assert_eq!(h.state().session.dataset.column(0).unwrap()[0], mksigma_engine::core::Value::Num(23.0));
}

#[test]
fn typing_in_the_blank_rows_adds_a_case_and_a_variable() {
    use mksigma_engine::core::Value;
    let mut h = sized(SigmaApp::default(), 1800.0);
    // Data View: the row after case 8 is blank; type into its first cell.
    assert!(shows(&h, "+"), "the blank row is marked");
    let target = mksigma_ui_egui::Target::Cell { case: 8, var: 0 };
    h.state_mut().editing = Some(mksigma_ui_egui::Editing::new(target, "40"));
    h.run_steps(3);
    press(&mut h, egui::Key::Enter);
    assert_eq!(h.state().session.dataset.n_cases(), 9);
    assert_eq!(h.state().session.dataset.column(0).unwrap()[8], Value::Num(40.0));
    assert_eq!(h.state().session.dataset.column(1).unwrap()[8], Value::SysMis);

    // Variable View: a name in the blank last row runs NUMERIC.
    h.state_mut().view = View::Variables;
    h.run_steps(3);
    let target = mksigma_ui_egui::Target::Variable(3, mksigma_ui_egui::Field::Name);
    h.state_mut().editing = Some(mksigma_ui_egui::Editing::new(target, "renda_extra"));
    h.run_steps(3);
    press(&mut h, egui::Key::Enter);
    assert_eq!(h.state().session.dataset.variables()[3].name, "renda_extra");
    assert_eq!(h.state().session.journal, ["NUMERIC renda_extra."]);
}

#[test]
fn right_click_deletes_cases_and_variables() {
    let mut h = sized(SigmaApp::default(), 1800.0);
    h.get_by_label("2").click_secondary();
    h.run_steps(3);
    h.get_by_label("Delete case").click();
    h.run_steps(3);
    assert_eq!(h.state().session.dataset.n_cases(), 7);
    assert_eq!(h.state().session.dataset.column(0).unwrap()[1], mksigma_engine::core::Value::Num(41.0));

    h.state_mut().view = View::Variables;
    h.run_steps(3);
    h.get_all_by_label("1").next().unwrap().click_secondary();
    h.run_steps(3);
    h.get_by_label("Delete age").click();
    h.run_steps(3);
    assert_eq!(h.state().session.dataset.variables()[0].name, "income");
    assert_eq!(h.state().session.journal, ["DELETE VARIABLES age."]);
}

#[test]
fn charts_pivot_and_html_export() {
    let path = temp_path("output.html");
    let picked = path.clone();
    let services = Services { pick_export: Some(Box::new(move |_: &str| Some(picked.clone()))), ..Services::default() };
    let mut app = SigmaApp::new(services);
    app.session.run_syntax("DESCRIPTIVES age income.\nGRAPH /HISTOGRAM(NORMAL)=income.").unwrap();
    let mut h = harness(app);
    assert!(shows(&h, "Descriptive Statistics"));
    // Pivot the table: the variables become columns.
    h.get_by_label("pivot").click();
    h.run_steps(3);
    assert!(h.state().output_state.transposed.contains(&0));
    assert!(shows(&h, "Std. Deviation"), "statistics are now row labels");
    h.get_by_label("Export HTML").click();
    h.run_steps(3);
    let html = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(html.contains("Descriptive Statistics") && html.contains("<svg"));
    assert!(matches!(&h.state().status, Some(Status::Info(s)) if s.starts_with("Exported the Output")));

    // The same Output as a PDF.
    h.get_by_label("Export PDF").click();
    h.run_steps(3);
    let pdf = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(pdf.starts_with(b"%PDF-") && pdf.ends_with(b"%%EOF\n"));
}
