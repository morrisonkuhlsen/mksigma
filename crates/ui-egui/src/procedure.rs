//! Procedure dialogs, as in SPSS: the file's variables on the left, the procedure's slots on the
//! right (Variables, Grouping variable, Factor...), options below. OK runs the command the dialog
//! builds and Paste puts it in the syntax editor; either way the command is syntax, in the
//! History. The dialog only builds text: the session runs it.

use mksigma_engine::Session;
use mksigma_engine::core::{Measure, Value, VarType, Variable};
use mksigma_engine::quote;
use mksigma_i18n::tr;

use crate::icons::{self, Icon};
use crate::theme::Tokens;

/// The procedures with a dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Descriptives,
    Frequencies,
    Crosstabs,
    TTest,
    Oneway,
    Correlations,
    Regression,
    Logistic,
    /// Mann-Whitney U or, switched in the dialog, Kruskal-Wallis H.
    Nonparametric,
    Explore,
    Histogram,
    Bar,
    Scatter,
    Boxplot,
}

/// A slot's or an option's name, which is also its id, in the interface's language.
fn shown(name: &str) -> String {
    let id: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let id = id.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    mksigma_i18n::message(&format!("dlg-{id}"), &[])
}

impl Kind {
    fn title(self) -> String {
        match self {
            Kind::Descriptives => tr!("dlg-title-descriptives"),
            Kind::Frequencies => tr!("dlg-title-frequencies"),
            Kind::Crosstabs => tr!("dlg-title-crosstabs"),
            Kind::TTest => tr!("dlg-title-ttest"),
            Kind::Oneway => tr!("dlg-title-oneway"),
            Kind::Correlations => tr!("dlg-title-correlations"),
            Kind::Regression => tr!("dlg-title-regression"),
            Kind::Logistic => tr!("dlg-title-logistic"),
            Kind::Nonparametric => tr!("dlg-title-nonparametric"),
            Kind::Explore => tr!("dlg-title-explore"),
            Kind::Histogram => tr!("dlg-title-histogram"),
            Kind::Bar => tr!("dlg-title-bar"),
            Kind::Scatter => tr!("dlg-title-scatter"),
            Kind::Boxplot => tr!("dlg-title-boxplot"),
        }
    }

    fn is_chart(self) -> bool {
        matches!(self, Kind::Histogram | Kind::Bar | Kind::Scatter | Kind::Boxplot)
    }
}

/// A list of variables the procedure takes.
#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    pub name: &'static str,
    pub vars: Vec<String>,
    /// Takes one variable: another replaces it.
    pub single: bool,
    /// Takes numeric variables only.
    pub numeric: bool,
    /// May stay empty.
    pub optional: bool,
}

impl Slot {
    fn new(name: &'static str, single: bool, numeric: bool) -> Self {
        Self { name, vars: Vec::new(), single, numeric, optional: false }
    }

    fn optional(name: &'static str, single: bool, numeric: bool) -> Self {
        Self { optional: true, ..Slot::new(name, single, numeric) }
    }
}

/// A dialog's state.
#[derive(Debug, Clone, PartialEq)]
pub struct Dialog {
    pub kind: Kind,
    pub slots: Vec<Slot>,
    /// Options, by name (the `FREQUENCIES` statistics, `CROSSTABS` cells, post hoc tests...).
    pub checks: Vec<(&'static str, bool)>,
    /// The two groups of a t-test, as typed.
    pub groups: (String, String),
    /// Confidence level in percent, where the procedure has one.
    pub confidence: f64,
    /// Kruskal-Wallis instead of Mann-Whitney (`groups` are then the lowest and highest).
    pub kruskal: bool,
    /// A chart's title; empty for the usual one.
    pub title: String,
    /// The variable picked in the file's list, then in a slot.
    picked: Option<String>,
    picked_in: Option<(usize, String)>,
}

/// What the dialog's buttons asked for.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Outcome {
    Open,
    Cancel,
    /// Run this syntax.
    Run(String),
    /// Put this syntax in the editor.
    Paste(String),
}

impl Dialog {
    pub fn new(kind: Kind) -> Self {
        let (slots, checks): (Vec<Slot>, Vec<(&'static str, bool)>) = match kind {
            Kind::Descriptives => (vec![Slot::new("Variables", false, true)], vec![]),
            Kind::Frequencies => (
                vec![Slot::new("Variables", false, false)],
                vec![
                    ("Frequency tables", true),
                    ("Mean", false),
                    ("Std. deviation", false),
                    ("Minimum", false),
                    ("Maximum", false),
                    ("Bar charts", false),
                    ("Histograms", false),
                ],
            ),
            Kind::Crosstabs => (
                vec![Slot::new("Rows", false, false), Slot::new("Columns", false, false)],
                vec![
                    ("Counts", true),
                    ("Expected", false),
                    ("Row percentages", false),
                    ("Column percentages", false),
                    ("Total percentages", false),
                    ("Chi-square", true),
                ],
            ),
            Kind::TTest => (vec![Slot::new("Test variables", false, true), Slot::new("Grouping variable", true, false)], vec![]),
            Kind::Oneway => (
                vec![Slot::new("Dependent list", false, true), Slot::new("Factor", true, true)],
                vec![("Descriptives", true), ("Homogeneity of variances", false), ("Tukey", false), ("Bonferroni", false)],
            ),
            Kind::Correlations => {
                (vec![Slot::new("Variables", false, true)], vec![("One-tailed", false), ("Flag significant", true), ("Listwise", false)])
            }
            Kind::Regression => {
                (vec![Slot::new("Dependent", true, true), Slot::new("Independents", false, true)], vec![("Confidence intervals", false)])
            }
            Kind::Logistic => (vec![Slot::new("Dependent", true, true), Slot::new("Covariates", false, true)], vec![("CI for Exp(B)", false)]),
            Kind::Nonparametric => (vec![Slot::new("Test variables", false, true), Slot::new("Grouping variable", true, true)], vec![]),
            Kind::Explore => (
                vec![Slot::new("Dependent list", false, true), Slot::optional("Factor", true, false)],
                vec![("Descriptives", true), ("Boxplots", true)],
            ),
            Kind::Histogram => (vec![Slot::new("Variable", true, true)], vec![("Normal curve", true)]),
            Kind::Bar => (vec![Slot::new("Category axis", true, false), Slot::optional("Mean of (empty: counts)", true, true)], vec![]),
            Kind::Scatter => (vec![Slot::new("X axis", true, true), Slot::new("Y axis", true, true)], vec![]),
            Kind::Boxplot => (vec![Slot::new("Variable", true, true), Slot::optional("Category axis", true, false)], vec![]),
        };
        Self {
            kind,
            slots,
            checks,
            groups: (String::new(), String::new()),
            confidence: 95.0,
            kruskal: false,
            title: String::new(),
            picked: None,
            picked_in: None,
        }
    }

    fn check(&self, name: &str) -> bool {
        self.checks.iter().any(|(n, on)| *n == name && *on)
    }

    fn used(&self, name: &str) -> bool {
        self.slots.iter().any(|s| s.vars.iter().any(|v| v == name))
    }

    /// Puts `var` in slot `slot` if it fits; a single slot gives back what it held. On a
    /// t-test's grouping variable, the groups start as its first two values.
    pub fn add(&mut self, session: &Session, slot: usize, var: &str) -> bool {
        let Some((_, v)) = session.dataset.variable(var) else { return false };
        let numeric = v.var_type == VarType::Numeric;
        let Some(target) = self.slots.get_mut(slot) else { return false };
        if target.numeric && !numeric {
            return false;
        }
        if target.single {
            target.vars.clear();
        }
        if !target.vars.iter().any(|x| x == var) {
            target.vars.push(var.to_string());
        }
        if self.kind == Kind::Nonparametric && slot == 1 {
            self.fill_groups(session);
        }
        if self.kind == Kind::TTest && slot == 1 {
            let values = session.values_of(var, 2);
            let text = |v: Option<&(Value, String, f64)>| match v.map(|(v, _, _)| v) {
                Some(Value::Num(x)) => x.to_string(),
                Some(Value::Str(s)) => s.clone(),
                _ => String::new(),
            };
            self.groups = (text(values.first()), text(values.get(1)));
        }
        true
    }

    /// The nonparametric test's groups from its grouping variable: its first two values, or for
    /// Kruskal-Wallis its lowest and highest.
    pub fn fill_groups(&mut self, session: &Session) {
        let Some(var) = self.slots.get(1).and_then(|s| s.vars.first()).cloned() else { return };
        self.groups = if self.kruskal {
            session.range_of(&var).map_or_else(Default::default, |(lo, hi)| (lo.to_string(), hi.to_string()))
        } else {
            let values: Vec<String> = session.values_of(&var, 2).iter().filter_map(|(v, _, _)| v.as_num()).map(|x| x.to_string()).collect();
            (values.first().cloned().unwrap_or_default(), values.get(1).cloned().unwrap_or_default())
        };
    }

    pub fn remove(&mut self, slot: usize, var: &str) {
        if let Some(s) = self.slots.get_mut(slot) {
            s.vars.retain(|v| v != var);
        }
    }

    /// The command, or what is missing for it.
    pub fn syntax(&self, session: &Session) -> Result<String, String> {
        let list = |k: usize| self.slots.get(k).map(|s| s.vars.join(" ")).unwrap_or_default();
        let need = |k: usize| {
            let s = self.slots.get(k);
            if s.is_some_and(|s| s.vars.is_empty() && !s.optional) {
                Err(tr!("dlg-err-put-variable", slot = shown(s.map_or("", |s| s.name))))
            } else {
                Ok(list(k))
            }
        };
        let title = if self.title.trim().is_empty() { String::new() } else { format!(" /TITLE={}", quote(self.title.trim())) };
        let number = |x: &str, what: &str| x.trim().parse::<f64>().map(|n| n.to_string()).map_err(|_| tr!("dlg-err-number", what = shown(what)));
        let ci = if (self.confidence - 95.0).abs() < 1e-9 { String::new() } else { format!(" /CRITERIA=CI({})", self.confidence / 100.0) };
        Ok(match self.kind {
            Kind::Descriptives => format!("DESCRIPTIVES VARIABLES={}.", need(0)?),
            Kind::Frequencies => {
                let mut s = format!("FREQUENCIES VARIABLES={}", need(0)?);
                if !self.check("Frequency tables") {
                    s.push_str(" /FORMAT=NOTABLE");
                }
                let stats: Vec<&str> = [("Mean", "MEAN"), ("Std. deviation", "STDDEV"), ("Minimum", "MINIMUM"), ("Maximum", "MAXIMUM")]
                    .iter()
                    .filter(|(n, _)| self.check(n))
                    .map(|(_, k)| *k)
                    .collect();
                if !stats.is_empty() {
                    s.push_str(&format!(" /STATISTICS={}", stats.join(" ")));
                }
                if self.check("Bar charts") {
                    s.push_str(" /BARCHART");
                }
                if self.check("Histograms") {
                    s.push_str(" /HISTOGRAM");
                }
                s + "."
            }
            Kind::Crosstabs => {
                let (rows, columns) = (need(0)?, need(1)?);
                let cells: Vec<&str> = [
                    ("Counts", "COUNT"),
                    ("Expected", "EXPECTED"),
                    ("Row percentages", "ROW"),
                    ("Column percentages", "COLUMN"),
                    ("Total percentages", "TOTAL"),
                ]
                .iter()
                .filter(|(n, _)| self.check(n))
                .map(|(_, k)| *k)
                .collect();
                let mut s = format!("CROSSTABS /TABLES={rows} BY {columns}");
                if !cells.is_empty() {
                    s.push_str(&format!(" /CELLS={}", cells.join(" ")));
                }
                if self.check("Chi-square") {
                    s.push_str(" /STATISTICS=CHISQ");
                }
                s + "."
            }
            Kind::TTest => {
                let (tests, group) = (need(0)?, need(1)?);
                let (a, b) = (self.groups.0.trim(), self.groups.1.trim());
                if a.is_empty() || b.is_empty() {
                    return Err(tr!("dlg-err-groups"));
                }
                // A string grouping variable takes quoted values.
                let string = session.dataset.variable(&group).is_some_and(|(_, v)| v.var_type != VarType::Numeric);
                let value = |x: &str| -> Result<String, String> {
                    if string {
                        Ok(quote(x))
                    } else {
                        x.parse::<f64>().map(|n| n.to_string()).map_err(|_| tr!("dlg-err-not-number", value = x, group = group.as_str()))
                    }
                };
                format!("T-TEST GROUPS={group}({} {}) /VARIABLES={tests}{ci}.", value(a)?, value(b)?)
            }
            Kind::Oneway => {
                let (deps, factor) = (need(0)?, need(1)?);
                let stats: Vec<&str> = [("Descriptives", "DESCRIPTIVES"), ("Homogeneity of variances", "HOMOGENEITY")]
                    .iter()
                    .filter(|(n, _)| self.check(n))
                    .map(|(_, k)| *k)
                    .collect();
                let posthoc: Vec<&str> =
                    [("Tukey", "TUKEY"), ("Bonferroni", "BONFERRONI")].iter().filter(|(n, _)| self.check(n)).map(|(_, k)| *k).collect();
                let mut s = format!("ONEWAY {deps} BY {factor}");
                if !stats.is_empty() {
                    s.push_str(&format!(" /STATISTICS={}", stats.join(" ")));
                }
                if !posthoc.is_empty() {
                    s.push_str(&format!(" /POSTHOC={} ALPHA(0.05)", posthoc.join(" ")));
                }
                s + "."
            }
            Kind::Correlations => {
                let vars = need(0)?;
                if self.slots.first().is_some_and(|s| s.vars.len() < 2) {
                    return Err(tr!("dlg-err-two-variables"));
                }
                let tails = if self.check("One-tailed") { "ONETAIL" } else { "TWOTAIL" };
                let flag = if self.check("Flag significant") { " NOSIG" } else { "" };
                let missing = if self.check("Listwise") { " /MISSING=LISTWISE" } else { "" };
                format!("CORRELATIONS /VARIABLES={vars} /PRINT={tails}{flag}{missing}.")
            }
            Kind::Regression => {
                let (dependent, predictors) = (need(0)?, need(1)?);
                let ci = if self.check("Confidence intervals") { " /STATISTICS COEFF OUTS R ANOVA CI(95)" } else { "" };
                format!("REGRESSION{ci} /DEPENDENT {dependent} /METHOD=ENTER {predictors}.")
            }
            Kind::Logistic => {
                let (dependent, covariates) = (need(0)?, need(1)?);
                let ci = if self.check("CI for Exp(B)") { " /PRINT=CI(95)" } else { "" };
                format!("LOGISTIC REGRESSION VARIABLES {dependent} /METHOD=ENTER {covariates}{ci}.")
            }
            Kind::Nonparametric => {
                let (tests, group) = (need(0)?, need(1)?);
                let names = if self.kruskal { ("The minimum", "The maximum") } else { ("Group 1", "Group 2") };
                let (a, b) = (number(&self.groups.0, names.0)?, number(&self.groups.1, names.1)?);
                let test = if self.kruskal { "K-W" } else { "M-W" };
                format!("NPAR TESTS /{test}={tests} BY {group}({a} {b}).")
            }
            Kind::Explore | Kind::Boxplot => {
                let deps = need(0)?;
                let by = need(1)?;
                let by = if by.is_empty() { String::new() } else { format!(" BY {by}") };
                let (descriptives, boxplots) =
                    if self.kind == Kind::Boxplot { (false, true) } else { (self.check("Descriptives"), self.check("Boxplots")) };
                if !descriptives && !boxplots {
                    return Err(tr!("dlg-err-explore"));
                }
                let plot = if boxplots { "BOXPLOT" } else { "NONE" };
                let stats = if descriptives { "DESCRIPTIVES" } else { "NONE" };
                format!("EXAMINE VARIABLES={deps}{by} /PLOT={plot} /STATISTICS={stats}.")
            }
            Kind::Histogram => {
                let normal = if self.check("Normal curve") { "(NORMAL)" } else { "" };
                format!("GRAPH /HISTOGRAM{normal}={}{title}.", need(0)?)
            }
            Kind::Bar => {
                let (by, mean) = (need(0)?, need(1)?);
                let value = if mean.is_empty() { "COUNT".to_string() } else { format!("MEAN({mean})") };
                format!("GRAPH /BAR(SIMPLE)={value} BY {by}{title}.")
            }
            Kind::Scatter => format!("GRAPH /SCATTERPLOT(BIVAR)={} WITH {}{title}.", need(0)?, need(1)?),
        })
    }
}

fn level_icon(var: &Variable) -> Icon {
    match var.measure {
        Measure::Scale => Icon::Scale,
        Measure::Ordinal => Icon::Ordinal,
        Measure::Nominal => Icon::Nominal,
    }
}

/// A variable in a list: its level's icon and its name (label on hover). Returns (clicked,
/// double-clicked).
fn row(ui: &mut egui::Ui, var: &Variable, on: bool, enabled: bool) -> (bool, bool) {
    let t = Tokens::get(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, on, &var.name));
    if on {
        ui.painter().rect_filled(rect, 4.0, t.accent_soft);
    } else if response.hovered() && enabled {
        ui.painter().rect_filled(rect, 4.0, t.hover);
    }
    let mut painter = ui.painter().clone();
    if !enabled {
        painter.set_opacity(0.4);
    }
    icons::paint(
        &painter,
        egui::Rect::from_center_size(egui::pos2(rect.left() + 12.0, rect.center().y), egui::vec2(13.0, 13.0)),
        level_icon(var),
        &t,
    );
    painter.text(egui::pos2(rect.left() + 26.0, rect.center().y), egui::Align2::LEFT_CENTER, &var.name, egui::FontId::proportional(13.0), t.text);
    let tip = var.label.as_deref().unwrap_or(&var.name);
    let response = response.on_hover_text(tip);
    (response.clicked(), response.double_clicked())
}

pub(crate) fn show(ctx: &egui::Context, dialog: &mut Dialog, session: &Session) -> Outcome {
    let t = Tokens::get(ctx);
    let mut outcome = Outcome::Open;
    let mut error = None;
    egui::Window::new(dialog.kind.title())
        .collapsible(false)
        .resizable(false)
        .default_width(560.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                // The file's variables, those not in a slot.
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(shown("Variables")).color(t.text_dim));
                    egui::Frame::new()
                        .fill(t.input_bg)
                        .stroke(egui::Stroke::new(1.0, t.border))
                        .corner_radius(6)
                        .inner_margin(egui::Margin::same(4))
                        .show(ui, |ui| {
                            ui.set_width(200.0);
                            ui.set_min_height(240.0);
                            egui::ScrollArea::vertical().id_salt("dialog-source").max_height(240.0).show(ui, |ui| {
                                for var in session.dataset.variables() {
                                    if dialog.used(&var.name) {
                                        continue;
                                    }
                                    let fits = dialog.slots.iter().any(|s| !s.numeric || var.var_type == VarType::Numeric);
                                    let on = dialog.picked.as_deref() == Some(var.name.as_str());
                                    let (clicked, double) = row(ui, var, on, fits);
                                    if double {
                                        // Into the first slot that takes it.
                                        let name = var.name.clone();
                                        if let Some(k) = (0..dialog.slots.len())
                                            .find(|k| dialog.slots.get(*k).is_some_and(|s| !s.numeric || var.var_type == VarType::Numeric))
                                        {
                                            dialog.add(session, k, &name);
                                        }
                                        dialog.picked = None;
                                    } else if clicked {
                                        dialog.picked = Some(var.name.clone());
                                        dialog.picked_in = None;
                                    }
                                }
                            });
                        });
                });
                ui.add_space(6.0);
                // The slots, each with its arrow.
                ui.vertical(|ui| {
                    for k in 0..dialog.slots.len() {
                        let Some(slot) = dialog.slots.get(k).cloned() else { continue };
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.add_space(22.0);
                                let back = dialog.picked_in.as_ref().is_some_and(|(s, _)| *s == k);
                                let (label, tip) = if back {
                                    ("<", tr!("dlg-take-out", slot = shown(slot.name)))
                                } else {
                                    (">", tr!("dlg-put-in", slot = shown(slot.name)))
                                };
                                let button = egui::Button::new(label).min_size(egui::vec2(28.0, 24.0));
                                if ui.add(button).on_hover_text(tip).clicked() {
                                    if let Some((s, name)) = dialog.picked_in.take().filter(|(s, _)| *s == k) {
                                        dialog.remove(s, &name);
                                    } else if let Some(name) = dialog.picked.take() {
                                        dialog.add(session, k, &name);
                                    }
                                }
                            });
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(shown(slot.name)).color(t.text_dim));
                                let height = if slot.single { 26.0 } else { 90.0 };
                                egui::Frame::new()
                                    .fill(t.input_bg)
                                    .stroke(egui::Stroke::new(1.0, t.border))
                                    .corner_radius(6)
                                    .inner_margin(egui::Margin::same(4))
                                    .show(ui, |ui| {
                                        ui.set_width(220.0);
                                        ui.set_min_height(height);
                                        for name in &slot.vars {
                                            let Some((_, var)) = session.dataset.variable(name) else { continue };
                                            let on = dialog.picked_in.as_ref().is_some_and(|(s, n)| *s == k && n == name);
                                            let (clicked, double) = row(ui, var, on, true);
                                            if double {
                                                dialog.remove(k, name);
                                                dialog.picked_in = None;
                                            } else if clicked {
                                                dialog.picked_in = Some((k, name.clone()));
                                                dialog.picked = None;
                                            }
                                        }
                                    });
                            });
                        });
                        ui.add_space(4.0);
                    }
                    if dialog.kind == Kind::Nonparametric {
                        ui.horizontal(|ui| {
                            let before = dialog.kruskal;
                            ui.radio_value(&mut dialog.kruskal, false, tr!("dlg-mann-whitney"));
                            ui.radio_value(&mut dialog.kruskal, true, tr!("dlg-kruskal-wallis"));
                            if dialog.kruskal != before {
                                dialog.fill_groups(session);
                            }
                        });
                    }
                    if matches!(dialog.kind, Kind::TTest | Kind::Nonparametric) {
                        let names =
                            if dialog.kruskal && dialog.kind == Kind::Nonparametric { ("Minimum", "Maximum") } else { ("Group 1", "Group 2") };
                        ui.horizontal(|ui| {
                            ui.label(shown(names.0));
                            ui.add(egui::TextEdit::singleline(&mut dialog.groups.0).desired_width(60.0));
                            ui.label(shown(names.1));
                            ui.add(egui::TextEdit::singleline(&mut dialog.groups.1).desired_width(60.0));
                        });
                    }
                    if dialog.kind.is_chart() {
                        ui.horizontal(|ui| {
                            ui.label(tr!("dlg-title"));
                            ui.add(egui::TextEdit::singleline(&mut dialog.title).hint_text(tr!("dlg-title-hint")).desired_width(180.0));
                        });
                    }
                });
            });
            if !dialog.checks.is_empty() || dialog.kind == Kind::TTest {
                ui.add_space(8.0);
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    for (name, on) in &mut dialog.checks {
                        ui.checkbox(on, shown(name));
                    }
                    if dialog.kind == Kind::TTest {
                        ui.label(tr!("dlg-confidence"));
                        ui.add(egui::DragValue::new(&mut dialog.confidence).range(50.0..=99.9).speed(0.5).suffix(" %"));
                    }
                });
            }
            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                let built = dialog.syntax(session);
                if let Err(e) = &built {
                    error = Some(e.clone());
                }
                ui.add_enabled_ui(built.is_ok(), |ui| {
                    let ok = egui::Button::new(egui::RichText::new(tr!("dlg-ok")).color(egui::Color32::WHITE))
                        .fill(t.accent)
                        .min_size(egui::vec2(64.0, 26.0));
                    let run = ui.add(ok).clicked();
                    let paste =
                        ui.add(egui::Button::new(tr!("dlg-paste")).min_size(egui::vec2(64.0, 26.0))).on_hover_text(tr!("dlg-paste-tip")).clicked();
                    if let Ok(s) = &built {
                        if run {
                            outcome = Outcome::Run(s.clone());
                        } else if paste {
                            outcome = Outcome::Paste(s.clone());
                        }
                    }
                });
                if ui.button(tr!("dlg-reset")).clicked() {
                    *dialog = Dialog::new(dialog.kind);
                }
                if ui.button(tr!("dlg-cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    outcome = Outcome::Cancel;
                }
                if let Some(e) = &error {
                    ui.label(egui::RichText::new(e).color(t.text_dim));
                }
            });
        });
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_slot_and_option_has_its_name_in_every_language() {
        let kinds = [
            Kind::Descriptives,
            Kind::Frequencies,
            Kind::Crosstabs,
            Kind::TTest,
            Kind::Oneway,
            Kind::Correlations,
            Kind::Regression,
            Kind::Logistic,
            Kind::Nonparametric,
            Kind::Explore,
            Kind::Histogram,
            Kind::Bar,
            Kind::Scatter,
            Kind::Boxplot,
        ];
        let mut names: Vec<&str> = vec!["Variables", "Group 1", "Group 2", "Minimum", "Maximum", "The minimum", "The maximum"];
        for kind in kinds {
            let d = Dialog::new(kind);
            names.extend(d.slots.iter().map(|s| s.name));
            names.extend(d.checks.iter().map(|(n, _)| *n));
        }
        for lang in mksigma_i18n::Lang::ALL {
            mksigma_i18n::set(lang);
            for name in &names {
                assert!(!shown(name).starts_with("dlg-"), "{name} in {}", lang.code());
            }
        }
        mksigma_i18n::set(mksigma_i18n::Lang::En);
    }

    fn session() -> Session {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax("RECODE age (LO THRU 30 = 1) (ELSE = 2) INTO grupo.\nSTRING uf (A2).\nCOMPUTE x = 1.").unwrap();
        s
    }

    #[test]
    fn dialogs_build_runnable_syntax() {
        let mut s = session();
        let mut d = Dialog::new(Kind::Descriptives);
        assert_eq!(d.syntax(&s), Err("Put a variable in Variables.".into()));
        assert!(!d.add(&s, 0, "uf"), "Descriptives takes numbers");
        assert!(d.add(&s, 0, "age") && d.add(&s, 0, "score") && d.add(&s, 0, "age"));
        assert_eq!(d.syntax(&s).unwrap(), "DESCRIPTIVES VARIABLES=age score.");

        let mut d = Dialog::new(Kind::TTest);
        d.add(&s, 0, "income");
        d.add(&s, 1, "grupo");
        assert_eq!(d.groups, ("1".to_string(), "2".to_string()), "the grouping variable's first two values");
        d.confidence = 90.0;
        assert_eq!(d.syntax(&s).unwrap(), "T-TEST GROUPS=grupo(1 2) /VARIABLES=income /CRITERIA=CI(0.9).");
        d.groups.1 = "dois".into();
        assert!(d.syntax(&s).is_err());

        let mut d = Dialog::new(Kind::Oneway);
        d.add(&s, 0, "score");
        d.add(&s, 1, "grupo");
        d.add(&s, 1, "age");
        assert_eq!(d.slots[1].vars, ["age"], "a single slot keeps the last");
        d.remove(1, "age");
        d.add(&s, 1, "grupo");
        for c in &mut d.checks {
            c.1 = c.0 != "Bonferroni";
        }
        assert_eq!(d.syntax(&s).unwrap(), "ONEWAY score BY grupo /STATISTICS=DESCRIPTIVES HOMOGENEITY /POSTHOC=TUKEY ALPHA(0.05).");

        let mut d = Dialog::new(Kind::Crosstabs);
        d.add(&s, 0, "grupo");
        d.add(&s, 1, "x");
        assert_eq!(d.syntax(&s).unwrap(), "CROSSTABS /TABLES=grupo BY x /CELLS=COUNT /STATISTICS=CHISQ.");

        let mut d = Dialog::new(Kind::Frequencies);
        d.add(&s, 0, "grupo");
        for c in &mut d.checks {
            c.1 = matches!(c.0, "Mean" | "Bar charts");
        }
        assert_eq!(d.syntax(&s).unwrap(), "FREQUENCIES VARIABLES=grupo /FORMAT=NOTABLE /STATISTICS=MEAN /BARCHART.");

        // Every command a dialog builds runs.
        let all = [
            Kind::Descriptives,
            Kind::Frequencies,
            Kind::Crosstabs,
            Kind::TTest,
            Kind::Oneway,
            Kind::Correlations,
            Kind::Regression,
            Kind::Logistic,
            Kind::Nonparametric,
            Kind::Explore,
            Kind::Histogram,
            Kind::Bar,
            Kind::Scatter,
            Kind::Boxplot,
        ];
        s.run_syntax("COMPUTE alto = score >= 70.").unwrap();
        for kind in all {
            let mut d = Dialog::new(kind);
            let vars: &[&str] = match kind {
                Kind::Crosstabs => &["grupo", "x"],
                Kind::Frequencies | Kind::Descriptives | Kind::Histogram => &["score"],
                Kind::Correlations => &["score", "income"],
                Kind::Regression => &["score", "income"],
                Kind::Logistic => &["alto", "income"],
                Kind::Scatter => &["age", "income"],
                Kind::Bar => &["grupo", "income"],
                _ => &["score", "grupo"],
            };
            for (slot, var) in vars.iter().enumerate() {
                if kind == Kind::Correlations {
                    d.add(&s, 0, var);
                } else {
                    d.add(&s, slot, var);
                }
            }
            for check in &mut d.checks {
                check.1 = true;
            }
            d.title = "Título".into();
            let syntax = d.syntax(&s).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
            s.run_syntax(&syntax).unwrap_or_else(|e| panic!("{syntax}: {e}"));
        }
        let mut d = Dialog::new(Kind::Nonparametric);
        d.kruskal = true;
        d.add(&s, 0, "score");
        d.add(&s, 1, "grupo");
        assert_eq!(d.syntax(&s).unwrap(), "NPAR TESTS /K-W=score BY grupo(1 2).", "the grouping variable's range");
        let mut d = Dialog::new(Kind::Explore);
        d.add(&s, 0, "score");
        assert_eq!(d.syntax(&s).unwrap(), "EXAMINE VARIABLES=score /PLOT=BOXPLOT /STATISTICS=DESCRIPTIVES.", "Factor may stay empty");
        let mut d = Dialog::new(Kind::Bar);
        d.add(&s, 0, "grupo");
        assert_eq!(d.syntax(&s).unwrap(), "GRAPH /BAR(SIMPLE)=COUNT BY grupo.", "without Mean of, counts");
    }
}
