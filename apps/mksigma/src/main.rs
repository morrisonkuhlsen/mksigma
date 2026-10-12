//! MK Sigma desktop app.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mksigma_ui_egui::SigmaApp;

struct App(SigmaApp);

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
}

fn main() -> eframe::Result {
    let viewport = egui::ViewportBuilder::default().with_title("MK Sigma").with_inner_size([1200.0, 760.0]).with_min_inner_size([640.0, 420.0]);
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("MK Sigma", options, Box::new(|_cc| Ok(Box::new(App(SigmaApp::new())))))
}
