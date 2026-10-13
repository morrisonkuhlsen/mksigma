//! MK Sigma desktop app.
//!
//! `mksigma [data.sav]` opens the file, or the sample dataset without one.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mksigma_ui_egui::{Services, SigmaApp};

struct App(SigmaApp);

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
}

fn main() -> eframe::Result {
    let services = Services {
        pick_open: Some(Box::new(|| {
            rfd::FileDialog::new().add_filter("SPSS Statistics data (.sav, .zsav)", &["sav", "zsav"]).add_filter("All files", &["*"]).pick_file()
        })),
    };
    let mut app = SigmaApp::new(services);
    if let Some(path) = std::env::args_os().nth(1) {
        app.open_path(std::path::Path::new(&path));
    }
    let viewport = egui::ViewportBuilder::default()
        .with_title("MK Sigma")
        .with_app_id("mksigma")
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([720.0, 480.0])
        .with_drag_and_drop(true);
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("MK Sigma", options, Box::new(|_cc| Ok(Box::new(App(app)))))
}
