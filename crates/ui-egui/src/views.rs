//! Data View and Variable View. Both only read the dataset; edits come back as `(Target, text)`
//! pairs and `Commit`s for the app to apply once the frame's layout is done.

use mksigma_engine::core::{Dataset, Measure, Value, VarType, Variable};

use crate::ROW_HEIGHT;
use crate::edit::{Commit, Editing, Field, Target, editable, missing_text};
use crate::theme::Tokens;

/// Lines between the cells of a row starting at `min`: under the row, after the case number
/// and after each column.
fn row_lines(ui: &egui::Ui, t: &Tokens, min: egui::Pos2, height: f32, first: f32, widths: &[f32]) {
    let painter = ui.painter();
    let right = min.x + first + widths.iter().sum::<f32>();
    painter.hline(min.x..=right, min.y + height - 0.5, egui::Stroke::new(1.0, t.gridline));
    let mut x = min.x + first;
    painter.vline(x - 0.5, min.y..=min.y + height, egui::Stroke::new(1.0, t.header_line));
    for w in widths {
        x += w;
        painter.vline(x - 0.5, min.y..=min.y + height, egui::Stroke::new(1.0, t.gridline));
    }
}

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

/// Data View. The row after the last case is blank: typing in it adds a case. Right-click a case
/// number to insert or delete a case.
pub(crate) fn data_view(
    ui: &mut egui::Ui,
    ds: &Dataset,
    labels: bool,
    editing: &mut Option<Editing>,
    edits: &mut Vec<(Target, String)>,
    commits: &mut Vec<Commit>,
) {
    let vars = ds.variables();
    let n = ds.n_cases();
    let case_width = column_width(ui, &n.to_string()).max(48.0);
    let widths: Vec<f32> = vars.iter().map(|v| column_width(ui, &v.name).max(80.0)).collect();
    let t = Tokens::get(ui.ctx());
    egui::ScrollArea::horizontal().id_salt("data-h").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        let header_height = ROW_HEIGHT + 4.0;
        let top = ui.cursor().min;
        let header = egui::Rect::from_min_size(top, egui::vec2(ui.available_width().max(case_width + widths.iter().sum::<f32>()), header_height));
        ui.painter().rect_filled(header, 0.0, t.header_bg);
        ui.horizontal(|ui| {
            ui.add_sized([case_width, header_height], egui::Label::new(""));
            for (v, w) in vars.iter().zip(&widths) {
                let name = egui::RichText::new(&v.name).color(t.header_text);
                ui.add_sized([*w, header_height], egui::Label::new(name)).on_hover_text(v.display_name());
            }
        });
        row_lines(ui, &t, top, header_height, case_width, &widths);
        ui.painter().hline(header.x_range(), header.bottom() - 0.5, egui::Stroke::new(1.0, t.header_line));
        // Only the visible rows are laid out, so large files stay fast.
        let rows_shown = if vars.is_empty() { 0 } else { n + 1 };
        egui::ScrollArea::vertical().id_salt("data-v").auto_shrink([false, false]).show_rows(ui, ROW_HEIGHT, rows_shown, |ui, rows| {
            for case in rows {
                let min = ui.cursor().min;
                ui.painter().rect_filled(egui::Rect::from_min_size(min, egui::vec2(case_width, ROW_HEIGHT)), 0.0, t.header_bg);
                ui.horizontal(|ui| {
                    if case == n {
                        ui.add_sized([case_width, ROW_HEIGHT], egui::Label::new(egui::RichText::new("+").weak()))
                            .on_hover_text("Type here to add a case");
                    } else {
                        let number = egui::Label::new(egui::RichText::new((case + 1).to_string()).color(t.header_text)).sense(egui::Sense::click());
                        ui.add_sized([case_width, ROW_HEIGHT], number).context_menu(|ui| {
                            if ui.button("Insert case above").clicked() {
                                commits.push(Commit::InsertCase(case));
                                ui.close();
                            }
                            if ui.button("Delete case").clicked() {
                                commits.push(Commit::DeleteCase(case));
                                ui.close();
                            }
                        });
                    }
                    for (i, (var, w)) in vars.iter().zip(&widths).enumerate() {
                        let value = ds.column(i).and_then(|c| c.get(case));
                        // The blank row after the last case shows nothing.
                        let text = match value {
                            Some(v) => cell_text(var, v, labels),
                            None => String::new(),
                        };
                        let mut rich = egui::RichText::new(text);
                        if value.is_some_and(|v| var.is_missing(v)) {
                            rich = rich.weak();
                        }
                        let right = var.var_type == VarType::Numeric && !(labels && value.is_some_and(|v| var.value_label(v).is_some()));
                        editable(ui, editing, edits, Target::Cell { case, var: i }, rich, || edit_text(value), *w, ROW_HEIGHT, right);
                    }
                });
                row_lines(ui, &t, min, ROW_HEIGHT, case_width, &widths);
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
                let number = ui.add(egui::Label::new(egui::RichText::new((i + 1).to_string()).weak()).sense(egui::Sense::click()));
                number.context_menu(|ui| {
                    if ui.button(format!("Delete {}", var.name)).clicked() {
                        commits.push(Commit::Syntax(format!("DELETE VARIABLES {}.", var.name)));
                        ui.close();
                    }
                });
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
            // Typing a name in the blank last row adds a numeric variable.
            let next = ds.variables().len();
            ui.label(egui::RichText::new("+").weak()).on_hover_text("Type a name to add a variable");
            editable(ui, editing, edits, Target::Variable(next, Field::Name), egui::RichText::new(""), String::new, 110.0, H, false);
            ui.end_row();
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
                if ui.small_button("remove").clicked() {
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
