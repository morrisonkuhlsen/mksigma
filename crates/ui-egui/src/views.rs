//! Data View and Variable View. Both only read the dataset; edits come back as `(Target, text)`
//! pairs and `Commit`s for the app to apply once the frame's layout is done.

use mksigma_engine::core::{Dataset, Measure, Value, VarType, Variable};

use crate::ROW_HEIGHT;
use crate::edit::{Commit, Editing, Field, Target, editable, missing_text};

/// A cell as Data View shows it: the value label when asked and one exists, else the value in
/// the variable's format.
fn cell_text(var: &Variable, v: &Value, labels: bool) -> String {
    match (labels, var.value_label(v)) {
        (true, Some(label)) => label.to_string(),
        _ => var.format.render(v),
    }
}

/// A value as typed when editing it: full precision, empty for system-missing.
fn edit_text(v: Option<&Value>) -> String {
    match v {
        Some(Value::Num(x)) => x.to_string(),
        Some(Value::Str(s)) => s.clone(),
        Some(Value::SysMis) | None => String::new(),
    }
}

/// Columns are as wide as their header text, within limits.
fn column_width(ui: &egui::Ui, text: &str) -> f32 {
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::TextStyle::Body.resolve(ui.style()), egui::Color32::WHITE);
    (galley.size().x + 16.0).clamp(56.0, 220.0)
}

pub(crate) fn data_view(ui: &mut egui::Ui, ds: &Dataset, labels: bool, editing: &mut Option<Editing>, edits: &mut Vec<(Target, String)>) {
    let vars = ds.variables();
    let n = ds.n_cases();
    let case_width = column_width(ui, &n.to_string()).max(48.0);
    let widths: Vec<f32> = vars.iter().map(|v| column_width(ui, &v.name).max(80.0)).collect();
    egui::ScrollArea::horizontal().id_salt("data-h").auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.add_sized([case_width, ROW_HEIGHT], egui::Label::new(""));
            for (v, w) in vars.iter().zip(&widths) {
                ui.add_sized([*w, ROW_HEIGHT], egui::Label::new(egui::RichText::new(&v.name).strong())).on_hover_text(v.display_name());
            }
        });
        ui.separator();
        // Only the visible rows are laid out, so large files stay fast.
        egui::ScrollArea::vertical().id_salt("data-v").auto_shrink([false, false]).show_rows(ui, ROW_HEIGHT, n, |ui, rows| {
            for case in rows {
                ui.horizontal(|ui| {
                    ui.add_sized([case_width, ROW_HEIGHT], egui::Label::new(egui::RichText::new((case + 1).to_string()).weak()));
                    for (i, (var, w)) in vars.iter().zip(&widths).enumerate() {
                        let value = ds.column(i).and_then(|c| c.get(case));
                        let text = value.map_or_else(|| ".".to_string(), |v| cell_text(var, v, labels));
                        let mut rich = egui::RichText::new(text);
                        if value.is_some_and(|v| var.is_missing(v)) {
                            rich = rich.weak();
                        }
                        let right = var.var_type == VarType::Numeric && !(labels && value.is_some_and(|v| var.value_label(v).is_some()));
                        editable(ui, editing, edits, Target::Cell { case, var: i }, rich, || edit_text(value), *w, ROW_HEIGHT, right);
                    }
                });
            }
        });
    });
}

fn values_text(var: &Variable) -> String {
    match var.value_labels.as_slice() {
        [] => "None".to_string(),
        [(v, label), rest @ ..] => {
            let first = format!("{{{}, {label}}}", var.format.render(v));
            if rest.is_empty() { first } else { format!("{first}…") }
        }
    }
}

const MEASURES: [(Measure, &str, &str); 3] =
    [(Measure::Nominal, "Nominal", "NOMINAL"), (Measure::Ordinal, "Ordinal", "ORDINAL"), (Measure::Scale, "Scale", "SCALE")];

pub(crate) fn variable_view(
    ui: &mut egui::Ui,
    ds: &Dataset,
    editing: &mut Option<Editing>,
    edits: &mut Vec<(Target, String)>,
    commits: &mut Vec<Commit>,
) {
    const H: f32 = 22.0;
    egui::ScrollArea::both().id_salt("variables").auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("variable-view").striped(true).num_columns(8).spacing([8.0, 2.0]).show(ui, |ui| {
            for h in ["", "Name", "Type", "Label", "Values", "Missing", "Measure", "Format"] {
                ui.strong(h);
            }
            ui.end_row();
            for (i, var) in ds.variables().iter().enumerate() {
                ui.label(egui::RichText::new((i + 1).to_string()).weak());
                let name = egui::RichText::new(&var.name);
                editable(ui, editing, edits, Target::Variable(i, Field::Name), name, || var.name.clone(), 110.0, H, false);
                ui.label(match var.var_type {
                    VarType::Numeric => "Numeric".to_string(),
                    VarType::String(w) => format!("String ({w})"),
                });
                let label = egui::RichText::new(var.label.as_deref().unwrap_or(""));
                editable(ui, editing, edits, Target::Variable(i, Field::Label), label, || var.label.clone().unwrap_or_default(), 160.0, H, false);

                let values = ui.add_sized([120.0, H], egui::Button::new(values_text(var)).truncate()).on_hover_text("Edit value labels");
                if values.clicked() {
                    commits.push(Commit::OpenValueLabels(i));
                }

                let missing = missing_text(var);
                let shown = egui::RichText::new(if missing.is_empty() { "None".to_string() } else { missing.clone() });
                editable(ui, editing, edits, Target::Variable(i, Field::Missing), shown, || missing, 95.0, H, false);

                let current = MEASURES.iter().find(|(m, _, _)| *m == var.measure).map_or("", |(_, label, _)| label);
                egui::ComboBox::from_id_salt(("measure", i)).selected_text(current).width(80.0).show_ui(ui, |ui| {
                    for (measure, label, keyword) in MEASURES {
                        // Strings can't be Scale.
                        if measure == Measure::Scale && var.var_type != VarType::Numeric {
                            continue;
                        }
                        if ui.selectable_label(var.measure == measure, label).clicked() && var.measure != measure {
                            commits.push(Commit::Syntax(format!("VARIABLE LEVEL {} ({keyword}).", var.name)));
                        }
                    }
                });

                let format = egui::RichText::new(var.format.to_string());
                if var.var_type == VarType::Numeric {
                    editable(ui, editing, edits, Target::Variable(i, Field::Format), format, || var.format.to_string(), 64.0, H, false);
                } else {
                    ui.add_sized([64.0, H], egui::Label::new(format.weak()));
                }
                ui.end_row();
            }
        });
    });
}

/// The Value Labels dialog: one row per label, edited freely; OK writes them all at once.
#[derive(Debug, Clone)]
pub struct ValueLabelsDialog {
    pub var: usize,
    pub rows: Vec<(String, String)>,
}

impl ValueLabelsDialog {
    pub fn new(var_index: usize, var: &Variable) -> Self {
        let rows = var
            .value_labels
            .iter()
            .map(|(v, l)| {
                let value = match v {
                    Value::Num(x) => x.to_string(),
                    Value::Str(s) => s.clone(),
                    Value::SysMis => String::new(),
                };
                (value, l.clone())
            })
            .collect();
        Self { var: var_index, rows }
    }
}

pub(crate) enum DialogOutcome {
    Open,
    Ok,
    Cancel,
}

pub(crate) fn value_labels_dialog(ctx: &egui::Context, dialog: &mut ValueLabelsDialog, name: &str) -> DialogOutcome {
    let mut outcome = DialogOutcome::Open;
    egui::Window::new(format!("Value Labels: {name}")).collapsible(false).resizable(false).show(ctx, |ui| {
        egui::Grid::new("value-labels").num_columns(3).show(ui, |ui| {
            ui.strong("Value");
            ui.strong("Label");
            ui.end_row();
            let mut remove = None;
            for (i, (value, label)) in dialog.rows.iter_mut().enumerate() {
                ui.add(egui::TextEdit::singleline(value).desired_width(80.0).hint_text("value"));
                ui.add(egui::TextEdit::singleline(label).desired_width(220.0).hint_text("label"));
                if ui.small_button("✕").on_hover_text("Remove").clicked() {
                    remove = Some(i);
                }
                ui.end_row();
            }
            if let Some(i) = remove {
                dialog.rows.remove(i);
            }
        });
        if ui.button("Add").clicked() {
            dialog.rows.push((String::new(), String::new()));
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("OK").clicked() {
                outcome = DialogOutcome::Ok;
            }
            if ui.button("Cancel").clicked() {
                outcome = DialogOutcome::Cancel;
            }
        });
    });
    outcome
}
