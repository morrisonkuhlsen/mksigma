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
    h.get_by_label("Run").click();
    h.run_steps(3);
    assert!(shows(&h, "Descriptive Statistics"));
    assert_eq!(h.state().status, None);

    h.state_mut().syntax = "DESCRIPTIVES VARIABLES=nope.".into();
    h.get_by_label("Run").click();
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
    // File opens the backstage; its Open page browses.
    h.get_by_label("File").click();
    h.run_steps(3);
    h.get_by_label("Open").click();
    h.run_steps(3);
    h.get_by_label("Browse…").click();
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
    // The first checkbox is the title row's AutoSave switch; the steps' follow in order.
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
    // The title row's Save, the first of the window's.
    h.get_all_by_label("Save").next().unwrap().click();
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
    // The title row's Undo names what it undoes.
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

#[test]
fn clicking_a_category_cross_filters_the_other_items() {
    let mut app = SigmaApp::new(Services::default());
    // Jovem (age ≤ 30) are three cases, scores 61, 80 and 77.
    app.session
        .run_syntax(
            "RECODE age (LO THRU 30 = 1) (ELSE = 2) INTO grupo.\nVALUE LABELS grupo 1 'Jovem' 2 'Adulto'.\n\
             FREQUENCIES grupo.\nGRAPH /BAR(SIMPLE)=COUNT BY grupo /TITLE='Grupos'.\nDESCRIPTIVES score.",
        )
        .unwrap();
    let mut h = harness(app);
    assert!(shows(&h, "72.38"), "the mean of all eight scores");

    // The value in a frequency table's row.
    h.get_by_label("Jovem").click();
    h.run_steps(3);
    assert_eq!(h.state().session.cross_filter().map(|c| c.label.as_str()), Some("grupo = Jovem"));
    assert!(shows(&h, "Filtered by grupo = Jovem"));
    assert!(shows(&h, "72.67") && !shows(&h, "72.38"), "DESCRIPTIVES ran again on the three");
    // The cross-filter's card, in the Filters pane.
    h.get_by_label("Show filters").click();
    h.run_steps(3);
    h.get_by_label("Clear filter").click();
    h.run_steps(3);
    assert!(h.state().session.cross_filter().is_none() && shows(&h, "72.38"));

    // A bar: two slots across the plot, which starts 64 points in and ends 16 before the edge.
    let chart = h.get_by_label("Grupos").rect();
    let slot = (chart.width() - 80.0) / 2.0;
    let adulto = egui::pos2(chart.min.x + 64.0 + slot * 1.5, chart.center().y);
    h.hover_at(adulto);
    h.run_steps(2);
    h.drag_at(adulto);
    h.run_steps(1);
    h.drop_at(adulto);
    h.run_steps(3);
    assert_eq!(h.state().session.cross_filter().map(|c| c.label.as_str()), Some("grupo = Adulto"));
    // Adulto: scores 74, 69, 58, 72 and 88, mean 72.2; the frequency table now has one value.
    assert!(shows(&h, "72.20"));
    assert!(!shows(&h, "Jovem"), "the frequency table ran again without them");

    let path = temp_path("filtered.html");
    h.state_mut().export(&path, mksigma_ui_egui::ExportFormat::Html);
    let html = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(html.contains("<h1>MK Sigma Output (filtered by grupo = Adulto)</h1>"), "an export says it is filtered");
}

#[test]
fn the_analyze_tab_runs_a_procedure_and_pastes_its_syntax() {
    let mut h = harness(SigmaApp::default());
    h.get_by_label("Analyze").click();
    h.run_steps(3);
    // Every procedure has its dialog now.
    for (button, title) in [("Correlate", "Bivariate Correlations"), ("Nonparametric", "Nonparametric Tests"), ("Explore", "Explore")] {
        h.get_by_label(button).click();
        h.run_steps(3);
        assert!(shows(&h, title), "{button} opens {title}");
        h.get_by_label("Cancel").click();
        h.run_steps(3);
    }
    h.get_by_label("Graphs").click();
    h.run_steps(3);
    h.get_by_label("Scatter").click();
    h.run_steps(3);
    assert!(shows(&h, "Scatterplot") && shows(&h, "X axis"));
    h.get_by_label("Cancel").click();
    h.run_steps(3);
    // Transformations still paste a command to complete.
    h.get_by_label("Transform").click();
    h.run_steps(3);
    h.get_by_label("Compute").click();
    h.run_steps(3);
    assert!(h.state().syntax.ends_with("COMPUTE new_variable = age."));
    assert!(matches!(&h.state().status, Some(Status::Info(s)) if s.contains("placeholder")));
    h.get_by_label("Variable View").click();
    h.run_steps(3);
    assert_eq!(h.state().view, View::Variables);
}

#[test]
fn a_procedure_dialog_builds_and_runs_the_command() {
    let mut h = harness(SigmaApp::default());
    h.get_by_label("Analyze").click();
    h.run_steps(3);
    h.get_by_label("Descriptives").click();
    h.run_steps(3);
    assert!(h.state().procedure_dialog.is_some());
    // income from the file's list (the dialog's, not Data View's header) into Variables.
    h.get_all_by_label("income").last().unwrap().click();
    h.run_steps(3);
    h.get_by_label(">").click();
    h.run_steps(3);
    assert_eq!(h.state().procedure_dialog.as_ref().unwrap().slots[0].vars, ["income"]);
    h.get_by_label("Paste").click();
    h.run_steps(3);
    assert!(h.state().procedure_dialog.is_none());
    assert!(h.state().syntax.ends_with("\nDESCRIPTIVES VARIABLES=income."), "Paste writes, doesn't run");
    assert!(!shows(&h, "Descriptive Statistics"));

    // OK runs it.
    h.get_by_label("Descriptives").click();
    h.run_steps(3);
    h.get_all_by_label("score").last().unwrap().click();
    h.run_steps(3);
    h.get_by_label(">").click();
    h.run_steps(3);
    h.get_by_label("OK").click();
    h.run_steps(3);
    assert!(shows(&h, "Descriptive Statistics") && shows(&h, "Test score"));
    assert!(h.state().session.journal.iter().any(|j| j == "DESCRIPTIVES VARIABLES=score."));
}

#[test]
fn autosave_saves_each_change_to_the_file() {
    let path = temp_path("autosave.sav");
    let picked = path.clone();
    let services = Services { pick_save: Some(Box::new(move |_: &str| Some(picked.clone()))), ..Services::default() };
    let mut h = harness(SigmaApp::new(services));
    // The sample has no file yet: switching AutoSave on asks where to save first.
    h.query_all_by_role(egui::accesskit::Role::CheckBox).next().unwrap().click();
    h.run_steps(3);
    assert!(h.state().autosave && !h.state().session.modified);
    h.state_mut().run_edit("VARIABLE LABELS age 'Idade'.");
    h.run_steps(2);
    assert!(!h.state().session.modified, "saved without asking");
    assert_eq!(h.state().status, Some(Status::Info("VARIABLE LABELS age 'Idade'.".into())), "quietly");
    let mut again = SigmaApp::default();
    again.open_path(&path);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(again.session.dataset.variables()[0].label.as_deref(), Some("Idade"));
}

#[test]
fn search_finds_commands_and_zoom_steps() {
    let mut h = harness(SigmaApp::default());
    h.get_by_label("Search commands and variables (Ctrl+F)").click();
    h.run_steps(3);
    h.state_mut().search = Some("descri".into());
    h.run_steps(3);
    h.get_by_label("Descriptives").click();
    h.run_steps(3);
    assert!(h.state().search.is_none());
    assert!(h.state().procedure_dialog.is_some(), "the command opens its dialog");
    h.get_by_label("Cancel").click();
    h.run_steps(3);

    h.get_by_label("Zoom In").click();
    h.run_steps(3);
    assert_eq!(h.state().zoom, 110);
    h.get_by_label("110%").click();
    h.run_steps(3);
    assert_eq!(h.state().zoom, 100);
}

#[test]
fn opening_a_csv_reads_it_through_get_data() {
    let path = temp_path("turma.csv");
    std::fs::write(&path, "nome;nota;turma\nAna;7,5;1\nBia;9;2\nCaio;6,5;1\nDani;8;2\n").unwrap();
    let picked = path.clone();
    let suggested = Rc::new(std::cell::RefCell::new(String::new()));
    let seen = suggested.clone();
    let services = Services {
        pick_open: Some(Box::new(move || Some(picked.clone()))),
        pick_save: Some(Box::new(move |s: &str| {
            *seen.borrow_mut() = s.to_string();
            None
        })),
        ..Services::default()
    };
    let mut h = harness(SigmaApp::new(services));
    h.get_by_label("Open").click();
    h.run_steps(3);
    std::fs::remove_file(&path).unwrap();
    let app = h.state();
    assert_eq!(app.session.dataset.n_cases(), 4);
    assert!(app.session.journal.last().is_some_and(|j| j.starts_with("GET DATA /TYPE=TXT /FILE=")), "{:?}", app.session.journal);
    assert_eq!(app.file_name.as_deref(), path.file_name().and_then(|n| n.to_str()));
    assert!(matches!(&app.status, Some(Status::Info(s)) if s.ends_with("4 cases, 3 variables (delimiter ';')")), "{:?}", app.status);
    assert!(shows(&h, "7.5"), "decimal commas read as numbers, with as many decimals as written (F8.1)");
    h.state_mut().save();
    assert!(suggested.borrow().ends_with("turma.sav"), "saved as a system file beside it: {}", suggested.borrow());
}

#[test]
fn a_slicer_picked_in_the_output_filters_every_item() {
    let mut app = SigmaApp::new(Services::default());
    app.session
        .run_syntax("RECODE age (LO THRU 30 = 1) (ELSE = 2) INTO grupo.\nVALUE LABELS grupo 1 'Jovem' 2 'Adulto'.\nDESCRIPTIVES score.")
        .unwrap();
    let mut h = harness(app);
    h.get_by_label("Show filters").click();
    h.run_steps(3);
    h.get_by_label("+ Slicer").click();
    h.run_steps(3);
    // The menu's grupo, not Data View's column header.
    h.get_all_by_label("grupo").last().unwrap().click();
    h.run_steps(3);
    h.get_by_label("grupo: all").click();
    h.run_steps(3);
    h.get_by_label("Jovem   (3)").click();
    h.run_steps(3);
    assert_eq!(h.state().session.slicers().len(), 1);
    assert!(h.state().session.journal.iter().any(|j| j == "SLICE grupo (1)."), "a slicer is syntax");
    assert!(shows(&h, "72.67"), "DESCRIPTIVES on the three Jovem");
    assert!(shows(&h, "grupo: Jovem"));
    h.get_by_label("Remove slicer").click();
    h.run_steps(3);
    assert!(h.state().session.slicers().is_empty());
    assert!(shows(&h, "72.38"));
}

#[test]
fn pinned_items_make_the_dashboard_and_it_exports() {
    let path = temp_path("dashboard.html");
    let picked = path.clone();
    let services = Services { pick_export: Some(Box::new(move |_: &str| Some(picked.clone()))), ..Services::default() };
    let mut app = SigmaApp::new(services);
    app.session.run_syntax("DESCRIPTIVES age income.\nGRAPH /HISTOGRAM=income /TITLE='Renda'.").unwrap();
    let mut h = harness(app);
    // The table's pin, then the chart's.
    h.get_all_by_label("Pin to the dashboard").next().unwrap().click();
    h.run_steps(3);
    h.get_by_label("Pin to the dashboard").click();
    h.run_steps(3);
    assert_eq!(h.state().session.dashboard().len(), 2);
    h.get_by_label("Dashboard").click();
    h.run_steps(3);
    assert!(h.state().output_state.dashboard);
    assert!(shows(&h, "Descriptive Statistics") && shows(&h, "Renda"), "both tiles");
    h.get_by_label("Export Dashboard").click();
    h.run_steps(3);
    let html = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(html.contains("class=\"tiles\"") && html.contains("<svg") && html.contains("Descriptive Statistics"));
    h.get_all_by_label("Unpin").next().unwrap().click();
    h.run_steps(3);
    assert_eq!(h.state().session.dashboard().len(), 1);
}

#[test]
fn a_saved_dashboard_opens_again() {
    let data = temp_path("loja.csv");
    std::fs::write(&data, "regiao,vendas\nNorte,10\nSul,12\nNorte,8\nSul,15\n").unwrap();
    let board = temp_path("loja dashboard.sps");
    let picked = board.clone();
    let services = Services { pick_export: Some(Box::new(move |_: &str| Some(picked.clone()))), ..Services::default() };
    let mut app = SigmaApp::new(services);
    app.open_path(&data);
    app.session.run_syntax("GRAPH /BAR(SIMPLE)=MEAN(vendas) BY regiao /TITLE='Vendas'.\nDESCRIPTIVES vendas.").unwrap();
    app.session.toggle_pin(0).unwrap();
    app.session.toggle_pin(1).unwrap();
    let mut h = harness(app);
    h.get_by_label("Dashboard").click();
    h.run_steps(3);
    h.get_by_label("Save Dashboard").click();
    h.run_steps(3);
    assert!(matches!(&h.state().status, Some(Status::Info(s)) if s.starts_with("Saved the dashboard")), "{:?}", h.state().status);

    let mut again = harness(SigmaApp::default());
    again.state_mut().open_path(&board);
    again.run_steps(3);
    std::fs::remove_file(&board).unwrap();
    std::fs::remove_file(&data).unwrap();
    let app = again.state();
    assert!(app.output_state.dashboard, "{:?}", app.status);
    assert_eq!(app.session.dashboard().len(), 2);
    assert_eq!(app.file_name.as_deref(), Some(data.file_name().unwrap().to_str().unwrap()));
    assert!(shows(&again, "Vendas") && shows(&again, "Descriptive Statistics"));
    assert!(app.syntax.contains("DASHBOARD /TILES=(1 1) (2 1)."), "the syntax is in the editor");
}

#[test]
fn a_dashboard_needs_data_from_a_file() {
    let mut app = SigmaApp::default();
    app.session.run_syntax("DESCRIPTIVES age.").unwrap();
    app.session.toggle_pin(0).unwrap();
    app.save_dashboard(&temp_path("never.sps"));
    assert!(matches!(&app.status, Some(Status::Error(s)) if s.contains("didn't come from a file")));
    assert!(!temp_path("never.sps").exists());
}

#[test]
fn the_ribbon_hides_and_comes_back() {
    let mut h = harness(SigmaApp::default());
    assert!(shows(&h, "Run Syntax"));
    h.get_by_label("Hide the ribbon (Ctrl+F1)").click();
    h.run_steps(3);
    assert!(h.state().ribbon_collapsed && !shows(&h, "Run Syntax"), "only the tabs are left");
    // A tab clicked while it is hidden shows it again, on that tab.
    h.get_by_label("Analyze").click();
    h.run_steps(3);
    assert!(!h.state().ribbon_collapsed && shows(&h, "Descriptives"));
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F1);
    h.run_steps(3);
    assert!(h.state().ribbon_collapsed);
}

#[test]
fn the_output_exports_as_latex() {
    let path = temp_path("output.tex");
    let mut app = SigmaApp::default();
    app.session.run_syntax("DESCRIPTIVES age income.\nGRAPH /HISTOGRAM(NORMAL)=income.").unwrap();
    app.export(&path, mksigma_ui_egui::ExportFormat::Latex);
    let tex = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(tex.contains("\\documentclass[10pt]{article}") && tex.contains("\\toprule") && tex.contains("\\begin{axis}"));
    assert!(matches!(&app.status, Some(Status::Info(s)) if s.starts_with("Exported the Output")));
}

#[test]
fn the_output_style_shows_on_screen() {
    let mut app = SigmaApp::default();
    app.session.run_syntax("DESCRIPTIVES age.").unwrap();
    let mut h = harness(app);
    assert!(shows(&h, "35.50"));
    h.state_mut().run_edit("OUTPUT STYLE /DECIMALS=3.");
    h.run_steps(3);
    assert!(shows(&h, "35.500") && !shows(&h, "35.50"), "every statistic with three decimals");
    h.get_by_label("View").click();
    h.run_steps(3);
    assert!(shows(&h, "Output Style"), "the controls are on the View tab");
}

#[test]
fn an_exported_dashboard_is_interactive() {
    let path = temp_path("interactive.html");
    let mut app = SigmaApp::default();
    app.session.run_syntax("GRAPH /BAR(SIMPLE)=COUNT BY age.\nDESCRIPTIVES score.\nDASHBOARD /TILES=(1 1) (2 1).").unwrap();
    app.export_dashboard(&path);
    let html = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    if mksigma_web::ENGINE.is_empty() {
        // A build without the WebAssembly target: the cards as they are.
        assert!(html.contains("class=\"tiles\"") && !html.contains("mks_open"));
    } else {
        assert!(html.contains("mks_open") && html.contains("id=\"engine\">AGFzbQ"), "the engine, as base64 of \\0asm");
        assert!(html.contains("data-item=\"0\"") && html.contains("data-part="), "a first view that can be clicked");
        assert!(matches!(&app.status, Some(Status::Info(s)) if s.contains("interactive")));
    }
}

#[test]
fn the_file_backstage() {
    let data = temp_path("recent.csv");
    std::fs::write(&data, "x,y\n1,2\n3,4\n").unwrap();
    let saved = Rc::new(std::cell::RefCell::new(Vec::new()));
    let keep = saved.clone();
    let services = Services {
        load_recent: Some(Box::new(|| vec![std::path::PathBuf::from("/nowhere/old.sav")])),
        save_recent: Some(Box::new(move |files: &[std::path::PathBuf]| *keep.borrow_mut() = files.to_vec())),
        ..Services::default()
    };
    let mut h = harness(SigmaApp::new(services));
    h.get_by_label("File").click();
    h.run_steps(3);
    assert_eq!(h.state().backstage, Some(mksigma_ui_egui::BackstagePage::Home));
    assert!(shows(&h, "Blank dataset") && shows(&h, "Sample dataset") && shows(&h, "old.sav"), "templates and recent files");
    h.get_by_label("Info").click();
    h.run_steps(3);
    assert!(shows(&h, "Cases") && shows(&h, "8"));

    // Replacing changed data asks first.
    h.state_mut().run_edit("VARIABLE LABELS age 'Idade'.");
    h.get_by_label("Home").click();
    h.run_steps(3);
    h.get_by_label("Blank dataset").click();
    h.run_steps(3);
    assert!(shows(&h, "The data has changes that aren't saved."));
    h.get_by_label("Close without saving").click();
    h.run_steps(3);
    assert_eq!(h.state().backstage, None);
    assert!(h.state().session.dataset.variables().is_empty(), "a blank dataset");

    // Opening a file puts it first among the recent ones, kept for later runs.
    h.state_mut().open_path(&data);
    std::fs::remove_file(&data).unwrap();
    assert_eq!(saved.borrow().first().and_then(|p| p.file_name()), data.file_name());
    assert_eq!(saved.borrow().len(), 2);
    h.get_by_label("File").click();
    h.run_steps(3);
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert_eq!(h.state().backstage, None, "Esc goes back");
}

#[test]
fn the_splash_closes_by_itself_or_on_a_click() {
    use mksigma_ui_egui::splash::{SIZE, SplashScreen};
    let size = egui::vec2(SIZE[0], SIZE[1]);
    // Steps of a quarter second: closed within its five seconds.
    let mut h = Harness::builder().with_size(size).build_ui_state(|ui, s: &mut SplashScreen| s.ui(ui), SplashScreen::default());
    h.run_steps(8);
    assert!(!h.state().closed());
    h.run_steps(14);
    assert!(h.state().closed());
    // A click closes it sooner.
    let mut h =
        Harness::builder().with_size(size).with_step_dt(1.0 / 60.0).build_ui_state(|ui, s: &mut SplashScreen| s.ui(ui), SplashScreen::default());
    h.run_steps(2);
    assert!(!h.state().closed());
    let at = egui::pos2(200.0, 200.0);
    h.event(egui::Event::PointerMoved(at));
    let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    h.event(button(true));
    h.event(button(false));
    h.run_steps(2);
    assert!(h.state().closed());
    // And so does Esc.
    let mut h =
        Harness::builder().with_size(size).with_step_dt(1.0 / 60.0).build_ui_state(|ui, s: &mut SplashScreen| s.ui(ui), SplashScreen::default());
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert!(h.state().closed());
}

#[test]
fn sigma_shows_the_calculation_by_hand_and_exports_it() {
    let path = temp_path("calculation.tex");
    let picked = path.clone();
    let services = Services { pick_export: Some(Box::new(move |_: &str| Some(picked.clone()))), ..Services::default() };
    let mut app = SigmaApp::new(services);
    app.session.run_syntax("DESCRIPTIVES age.\nGRAPH /HISTOGRAM=age.").unwrap();
    let mut h = harness(app);
    // The table and the chart each have their Σ.
    assert_eq!(h.get_all_by_label("Σ").count(), 2);
    h.get_all_by_label("Σ").next().unwrap().click();
    h.run_steps(3);
    assert!(shows(&h, "Σ  Descriptive Statistics, by hand"));
    // Each equation is drawn, and named by its LaTeX. Ages 23 35 41 29 52 47 31 26.
    assert!(h.query_by_label_contains("\\frac{284}{8} = 35.5").is_some());
    h.get_by_label("Show every term").click();
    h.run_steps(3);
    assert!(h.state().explain.as_ref().is_some_and(|v| v.full));
    h.get_by_label("Export .tex…").click();
    h.run_steps(3);
    let tex = std::fs::read_to_string(&path).unwrap();
    assert!(tex.contains("\\documentclass{article}") && tex.contains("\\frac{744}{8 - 1} = 106.2857"), "{tex}");
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(h.state().explain.is_none());
}

#[test]
fn learn_shows_articles_runs_their_examples_and_opens_from_a_table() {
    let mut app = SigmaApp::new(Services::default());
    app.session.run_syntax("DESCRIPTIVES age.").unwrap();
    // Tall, so a whole article is on screen.
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 2400.0)).build_ui_state(|ui, app: &mut SigmaApp| app.ui(ui), app);
    h.run_steps(3);
    // The ? beside the Descriptives table opens its article.
    h.get_by_label("?").click();
    h.run_steps(3);
    assert_eq!(h.state().article.as_deref(), Some("descriptives"));
    assert!(h.query_by_label_contains("Descriptives summarizes each Scale variable").is_some(), "the article");
    // Back to the list, then another article, and its example.
    h.get_by_label("‹ All articles").click();
    h.run_steps(3);
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Independent-samples t test").click();
    h.run_steps(3);
    assert_eq!(h.state().article.as_deref(), Some("t-test"));
    h.get_by_label("Run this example").click();
    h.run_steps(3);
    assert!(h.state().backstage.is_none(), "back to the data");
    assert!(h.state().session.journal.iter().any(|c| c.starts_with("T-TEST GROUPS=older")), "{:?}", h.state().session.journal);
    // A link to a dialog opens it.
    h.state_mut().show_article(Some("which-test"));
    h.run_steps(3);
    h.get_by_label("Open Crosstabs").click();
    h.run_steps(3);
    assert!(h.state().backstage.is_none() && h.state().procedure_dialog.is_some());
}

#[test]
fn the_view_rail_is_the_one_place_that_switches_views() {
    let mut h = harness(SigmaApp::default());
    h.get_by_label("Dashboard").click();
    h.run_steps(3);
    assert!(h.state().output_state.dashboard, "the dashboard on the whole window");
    h.get_by_label("Variable View").click();
    h.run_steps(3);
    assert!(!h.state().output_state.dashboard && h.state().view == View::Variables);
    h.get_by_label("Data View").click();
    h.run_steps(3);
    assert!(!h.state().output_state.dashboard && h.state().view == View::Data);
}

#[test]
fn the_filters_pane_starts_as_a_strip_opens_and_collapses_again() {
    let mut app = SigmaApp::new(Services::default());
    app.session.run_syntax("SLICE age (20 THRU 40).\nDESCRIPTIVES age.").unwrap();
    let mut h = harness(app);
    let chip = h.state().session.slicers().first().unwrap().describe(&h.state().session.dataset);
    assert!(!h.state().filters_open && h.query_by_label("+ Slicer").is_none(), "only the strip shows at first");
    h.get_by_label("Show filters").click();
    h.run_steps(3);
    assert!(h.state().filters_open && shows(&h, &chip), "the slicer's card: {chip}");
    h.get_by_label("Hide filters").click();
    h.run_steps(3);
    assert!(!h.state().filters_open && h.query_by_label("+ Slicer").is_none());
    assert_eq!(h.state().session.slicers().len(), 1, "collapsing changes no filter");
}
