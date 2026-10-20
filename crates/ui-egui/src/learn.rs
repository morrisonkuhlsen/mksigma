//! Learn, in the File backstage: the list of articles, by kind, and an article read on screen,
//! its formulas drawn as the Σ draws them. Links go to other articles or open a procedure's
//! dialog; an example runs on the data, or goes into the syntax editor.

use mksigma_i18n::tr;
use mksigma_learn::{Article, Block, Inline, Kind, Target};

use crate::theme::Tokens;

/// What was asked for on a Learn page.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Action {
    /// Show this article, or the list with `None`.
    Show(Option<String>),
    /// Open a procedure's dialog, by its name.
    Dialog(String),
    Run(String),
    Paste(String),
}

/// Size of the formulas, in points.
const MATH_SIZE: f32 = 17.0;

/// The Learn page: the list, or the article `id`.
pub(crate) fn page(ui: &mut egui::Ui, id: Option<&str>) -> Option<Action> {
    let lang = mksigma_i18n::current();
    match id.and_then(|id| mksigma_learn::article(lang, id)) {
        Some(article) => show_article(ui, &article),
        None => index(ui),
    }
}

fn index(ui: &mut egui::Ui) -> Option<Action> {
    let t = Tokens::get(ui.ctx());
    let mut action = None;
    ui.label(egui::RichText::new(tr!("learn-title")).size(28.0).color(t.text));
    ui.label(egui::RichText::new(tr!("learn-sub")).size(13.5).color(t.text_dim));
    ui.add_space(14.0);
    let articles = mksigma_learn::articles(mksigma_i18n::current());
    for (kind, caption) in [(Kind::Guide, tr!("learn-start")), (Kind::Procedure, tr!("learn-procedures")), (Kind::Concept, tr!("learn-concepts"))] {
        let of_kind: Vec<&Article> = articles.iter().filter(|a| a.kind == kind).collect();
        if of_kind.is_empty() {
            continue;
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new(caption).size(11.5).color(t.text_dim));
        ui.add_space(4.0);
        for article in of_kind {
            let frame = egui::Frame::new().fill(t.ribbon).stroke(egui::Stroke::new(1.0, t.border)).inner_margin(egui::Margin::symmetric(14, 10));
            let response = frame
                .show(ui, |ui| {
                    ui.set_width(ui.available_width().min(720.0));
                    ui.label(egui::RichText::new(&article.title).size(15.5).color(t.text));
                    ui.add(egui::Label::new(egui::RichText::new(&article.summary).size(12.5).color(t.text_dim)).wrap());
                })
                .response
                .interact(egui::Sense::click());
            response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &article.title));
            if response.clicked() {
                action = Some(Action::Show(Some(article.id.to_string())));
            }
            ui.add_space(8.0);
        }
    }
    action
}

fn show_article(ui: &mut egui::Ui, article: &Article) -> Option<Action> {
    let t = Tokens::get(ui.ctx());
    let mut action = None;
    if ui.link(tr!("learn-all")).clicked() {
        action = Some(Action::Show(None));
    }
    ui.add_space(6.0);
    ui.set_max_width(ui.available_width().min(760.0));
    ui.label(egui::RichText::new(&article.title).size(28.0).color(t.text));
    ui.add_space(8.0);
    for block in &article.blocks {
        match block {
            Block::Heading(text) => {
                ui.add_space(12.0);
                ui.label(egui::RichText::new(text).size(18.0).strong().color(t.text));
                ui.add_space(4.0);
            }
            Block::Paragraph(pieces) => {
                inline(ui, pieces, &mut action);
                ui.add_space(8.0);
            }
            Block::List(items) => {
                for item in items {
                    ui.horizontal_top(|ui| {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("•").color(t.accent));
                        ui.vertical(|ui| inline(ui, item, &mut action));
                    });
                    ui.add_space(3.0);
                }
                ui.add_space(6.0);
            }
            Block::Note(pieces) => {
                egui::Frame::new().fill(t.accent_soft).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    inline(ui, pieces, &mut action);
                });
                ui.add_space(8.0);
            }
            Block::Math(m) => {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.vertical(|ui| crate::math::show(ui, m, MATH_SIZE));
                });
                ui.add_space(8.0);
            }
            Block::Example(syntax) => {
                egui::Frame::new().fill(t.ribbon).stroke(egui::Stroke::new(1.0, t.border)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(egui::RichText::new(syntax).monospace().color(t.text));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if crate::ribbon::accent_button(ui, crate::icons::Icon::Run, &tr!("learn-run")).on_hover_text(tr!("learn-run-tip")).clicked()
                        {
                            action = Some(Action::Run(syntax.clone()));
                        }
                        if ui.button(tr!("learn-paste")).on_hover_text(tr!("learn-paste-tip")).clicked() {
                            action = Some(Action::Paste(syntax.clone()));
                        }
                    });
                });
                ui.add_space(8.0);
            }
        }
    }
    action
}

/// A paragraph's pieces, wrapping as text.
fn inline(ui: &mut egui::Ui, pieces: &[Inline], action: &mut Option<Action>) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let size = 14.5;
        for piece in pieces {
            match piece {
                Inline::Text(s) => {
                    ui.label(egui::RichText::new(s).size(size).color(t.text));
                }
                Inline::Strong(s) => {
                    ui.label(egui::RichText::new(s).size(size).strong().color(t.text));
                }
                Inline::Emphasis(s) => {
                    ui.label(egui::RichText::new(s).size(size).italics().color(t.text));
                }
                Inline::Code(s) => {
                    ui.label(egui::RichText::new(s).size(size - 1.5).monospace().color(t.accent));
                }
                Inline::Math(m) => {
                    crate::math::show(ui, m, size);
                }
                Inline::Link { text, target } => {
                    if ui.link(egui::RichText::new(text).size(size)).clicked() {
                        *action = Some(match target {
                            Target::Article(id) => Action::Show(Some(id.clone())),
                            Target::Dialog(kind) => Action::Dialog(kind.clone()),
                        });
                    }
                }
            }
        }
    });
}
