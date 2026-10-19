//! The Σ window: the calculation behind an Output table, by hand, step by step, as the engine
//! writes it. Every term of each sum on request; copied or exported as a LaTeX document.

use mksigma_engine::Session;
use mksigma_engine::output::Category;
use mksigma_engine::output::math::Explanation;

/// Size of the mathematics, in points.
const MATH_SIZE: f32 = 16.0;

/// What the explanation was made from: when any of it changes, it is made again.
#[derive(Debug, Clone, PartialEq)]
struct Made {
    items: usize,
    commands: usize,
    data: u64,
    cross_filter: Option<Category>,
    full: bool,
}

impl Made {
    fn of(session: &Session, full: bool) -> Self {
        Made {
            items: session.output.items.len(),
            commands: session.journal.len(),
            data: session.data_version(),
            cross_filter: session.cross_filter().cloned(),
            full,
        }
    }
}

/// The open Σ window.
#[derive(Debug)]
pub struct ExplainView {
    pub item: usize,
    /// Every term of each sum, not the first and the last few.
    pub full: bool,
    made: Made,
    explanation: Result<Explanation, String>,
}

impl ExplainView {
    pub fn new(session: &Session, item: usize) -> Self {
        let made = Made::of(session, false);
        ExplainView { item, full: false, made, explanation: session.explain(item, false).map_err(|e| e.to_string()) }
    }

    /// Made again if the Output, the data or the filters changed, or `full` did.
    fn refresh(&mut self, session: &Session) {
        let now = Made::of(session, self.full);
        if now != self.made {
            self.explanation = session.explain(self.item, self.full).map_err(|e| e.to_string());
            self.made = now;
        }
    }
}

pub(crate) enum Outcome {
    Open,
    Close,
    /// Export this LaTeX document; the name to suggest.
    Export(String, String),
}

pub(crate) fn show(ctx: &egui::Context, view: &mut ExplainView, session: &Session) -> Outcome {
    // The item went away (the Output was cleared or filtered to fewer items).
    if !session.can_explain(view.item) {
        return Outcome::Close;
    }
    view.refresh(session);
    let title = match &view.explanation {
        Ok(e) => e.title.clone(),
        Err(_) => "Calculation".to_string(),
    };
    let mut open = true;
    let mut outcome = Outcome::Open;
    let width = (ctx.content_rect().width() - 40.0).clamp(320.0, 760.0);
    egui::Window::new(format!("Σ  {title}, by hand"))
        .id(egui::Id::new("explain"))
        .open(&mut open)
        .collapsible(false)
        // A fixed width, which long equations break to; as tall as one likes.
        .resizable([false, true])
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .default_height(560.0)
        .min_width(width)
        .max_width(width)
        .max_height(ctx.content_rect().height() - 60.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut view.full, "Show every term").on_hover_text("Write out every term of each sum, not the first and the last few");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Ok(e) = &view.explanation {
                        if ui.button("Export .tex…").on_hover_text("A LaTeX document with the whole calculation").clicked() {
                            outcome = Outcome::Export(e.to_latex(), format!("{}.tex", e.title));
                        }
                        if ui.button("Copy LaTeX").on_hover_text("The whole calculation as a LaTeX document").clicked() {
                            ui.ctx().copy_text(e.to_latex());
                        }
                    }
                });
            });
            ui.add_space(6.0);
            ui.separator();
            match &view.explanation {
                Err(e) => {
                    ui.colored_label(ui.visuals().warn_fg_color, e);
                }
                Ok(e) => {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        for (heading, steps) in &e.sections {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(heading).strong().size(15.0));
                            for step in steps {
                                ui.add_space(4.0);
                                if !step.text.is_empty() {
                                    ui.add(egui::Label::new(&step.text).wrap());
                                }
                                if let Some(m) = &step.math {
                                    ui.horizontal(|ui| {
                                        ui.add_space(16.0);
                                        ui.vertical(|ui| crate::math::show(ui, m, MATH_SIZE));
                                    });
                                }
                            }
                        }
                        ui.add_space(8.0);
                    });
                }
            }
        });
    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        return Outcome::Close;
    }
    outcome
}
