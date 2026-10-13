//! In-place editing: a cell shows its text and becomes a text box when clicked. Enter or clicking
//! elsewhere commits, Escape cancels. Variable View edits turn into syntax commands, so they run
//! through the engine and land in the session's history like typed syntax.

use mksigma_engine::core::{Format, MissingValues, Value, VarType, Variable};
use mksigma_engine::quote;

/// A Variable View column that can be edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Label,
    Missing,
    Format,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Variable(usize, Field),
    Cell { case: usize, var: usize },
}

#[derive(Debug, Clone)]
pub struct Editing {
    pub target: Target,
    pub text: String,
    focused: bool,
}

/// A finished edit, applied after the frame's layout (when the dataset is no longer borrowed).
#[derive(Debug, Clone, PartialEq)]
pub enum Commit {
    Syntax(String),
    OpenValueLabels(usize),
}

/// Shows `display` (left-aligned, or right-aligned for numbers when `right`), or a text box
/// while `target` is being edited. Committed edits go to `commits` as `(target, text)`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn editable(
    ui: &mut egui::Ui,
    editing: &mut Option<Editing>,
    commits: &mut Vec<(Target, String)>,
    target: Target,
    display: egui::RichText,
    initial: impl FnOnce() -> String,
    width: f32,
    height: f32,
    right: bool,
) {
    if let Some(e) = editing.as_mut().filter(|e| e.target == target) {
        let response = ui.add_sized([width, height], egui::TextEdit::singleline(&mut e.text).margin(egui::vec2(2.0, 0.0)));
        if !e.focused {
            response.request_focus();
            e.focused = true;
        } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            *editing = None;
        } else if response.lost_focus() {
            commits.push((target, std::mem::take(&mut e.text)));
            *editing = None;
        }
        return;
    }
    let layout = if right { egui::Layout::right_to_left(egui::Align::Center) } else { egui::Layout::left_to_right(egui::Align::Center) };
    let response = ui
        .allocate_ui_with_layout(egui::vec2(width, height), layout, |ui| {
            ui.set_min_size(egui::vec2(width, height));
            ui.add(egui::Label::new(display).truncate().sense(egui::Sense::click()))
        })
        .inner;
    if response.clicked() {
        // Starting another edit commits the one in progress.
        if let Some(old) = editing.take() {
            commits.push((old.target, old.text));
        }
        *editing = Some(Editing { target, text: initial(), focused: false });
    }
}

fn is_lowest(x: f64) -> bool {
    x <= -1e300
}

fn is_highest(x: f64) -> bool {
    x >= 1e300
}

/// User-missing values as Variable View shows and edits them: `9, 99` or `LO THRU 0, 999`.
pub(crate) fn missing_text(var: &Variable) -> String {
    let num = |x: f64| var.format.render(&Value::Num(x));
    match &var.missing {
        MissingValues::None => String::new(),
        MissingValues::Discrete(list) => list.iter().map(|v| var.format.render(v)).collect::<Vec<_>>().join(", "),
        MissingValues::Range { low, high, discrete } => {
            let low = if is_lowest(*low) { "LO".to_string() } else { num(*low) };
            let high = if is_highest(*high) { "HI".to_string() } else { num(*high) };
            let range = format!("{low} THRU {high}");
            discrete.map_or(range.clone(), |d| format!("{range}, {}", num(d)))
        }
    }
}

/// The syntax for an edited Variable View cell, or `None` when nothing changes.
pub(crate) fn syntax_for(var: &Variable, field: Field, text: &str) -> Option<String> {
    let text = text.trim();
    let name = &var.name;
    match field {
        Field::Name => (!text.is_empty() && text != name).then(|| format!("RENAME VARIABLES ({name} = {text}).")),
        Field::Label => (Some(text) != var.label.as_deref() && !(text.is_empty() && var.label.is_none()))
            .then(|| format!("VARIABLE LABELS {name} {}.", quote(text))),
        Field::Missing => {
            if text == missing_text(var) {
                return None;
            }
            let values = match var.var_type {
                VarType::Numeric => text.to_string(),
                // Text values are typed bare; quote each one.
                VarType::String(_) => text.split(',').map(str::trim).filter(|s| !s.is_empty()).map(quote).collect::<Vec<_>>().join(", "),
            };
            Some(format!("MISSING VALUES {name} ({values})."))
        }
        // `F3` and `F3.0` are the same format: compare what they mean, not how they're written.
        Field::Format => (!text.is_empty() && text.parse::<Format>().ok() != Some(var.format)).then(|| format!("FORMATS {name} ({text}).")),
    }
}

/// `VALUE LABELS` for the rows of the Value Labels dialog; empty rows are skipped.
pub(crate) fn value_labels_syntax(var: &Variable, rows: &[(String, String)]) -> String {
    let pairs: Vec<String> = rows
        .iter()
        .filter(|(v, _)| !v.trim().is_empty())
        .map(|(v, label)| {
            let value = match var.var_type {
                VarType::Numeric => v.trim().to_string(),
                VarType::String(_) => quote(v.trim()),
            };
            format!("{value} {}", quote(label))
        })
        .collect();
    if pairs.is_empty() { format!("VALUE LABELS {}.", var.name) } else { format!("VALUE LABELS {} {}.", var.name, pairs.join(" ")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_engine::core::FormatKind;

    fn var() -> Variable {
        let mut v = Variable::numeric("q1").with_label("Satisfaction");
        v.format = Format::new(FormatKind::F, 3, 0);
        v
    }

    #[test]
    fn syntax_for_edits() {
        let v = var();
        assert_eq!(syntax_for(&v, Field::Name, " sat "), Some("RENAME VARIABLES (q1 = sat).".into()));
        assert_eq!(syntax_for(&v, Field::Name, "q1"), None);
        assert_eq!(syntax_for(&v, Field::Label, "It's fine"), Some("VARIABLE LABELS q1 'It''s fine'.".into()));
        assert_eq!(syntax_for(&v, Field::Label, "Satisfaction"), None);
        assert_eq!(syntax_for(&v, Field::Missing, "9, 99"), Some("MISSING VALUES q1 (9, 99).".into()));
        assert_eq!(syntax_for(&v, Field::Missing, ""), None, "already none");
        assert_eq!(syntax_for(&v, Field::Format, "f8.2"), Some("FORMATS q1 (f8.2).".into()));
        assert_eq!(syntax_for(&v, Field::Format, "F3"), None);
        let s = Variable::string("city", 8);
        assert_eq!(syntax_for(&s, Field::Missing, "NA, -"), Some("MISSING VALUES city ('NA', '-').".into()));
    }

    #[test]
    fn missing_values_round_trip_as_text() {
        let mut v = var();
        v.missing = MissingValues::Range { low: f64::MIN, high: 0.0, discrete: Some(999.0) };
        assert_eq!(missing_text(&v), "LO THRU 0, 999");
        v.missing = MissingValues::Discrete(vec![Value::Num(9.0), Value::Num(99.0)]);
        assert_eq!(missing_text(&v), "9, 99");
        assert_eq!(syntax_for(&v, Field::Missing, "9, 99"), None);
        assert_eq!(syntax_for(&v, Field::Missing, ""), Some("MISSING VALUES q1 ().".into()));
    }

    #[test]
    fn value_labels_from_rows() {
        let rows = vec![("1".to_string(), "Low".to_string()), (" ".to_string(), "ignored".to_string()), ("5".to_string(), "High".to_string())];
        assert_eq!(value_labels_syntax(&var(), &rows), "VALUE LABELS q1 1 'Low' 5 'High'.");
        assert_eq!(value_labels_syntax(&Variable::string("uf", 2), &[("SP".into(), "São Paulo".into())]), "VALUE LABELS uf 'SP' 'São Paulo'.");
        assert_eq!(value_labels_syntax(&var(), &[]), "VALUE LABELS q1.");
    }
}
