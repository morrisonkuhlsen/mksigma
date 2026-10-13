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

/// Seconds since 1970, for saved files' creation time.
#[allow(clippy::disallowed_methods)] // the desktop app is native only
fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

fn main() -> eframe::Result {
    let services = Services {
        pick_open: Some(Box::new(|| {
            rfd::FileDialog::new().add_filter("SPSS Statistics data (.sav, .zsav)", &["sav", "zsav"]).add_filter("All files", &["*"]).pick_file()
        })),
        pick_save: Some(Box::new(|suggested: &str| {
            rfd::FileDialog::new().add_filter("SPSS Statistics data (.sav)", &["sav"]).set_file_name(suggested).save_file()
        })),
        now: Some(Box::new(now)),
        pick_export: Some(Box::new(|suggested: &str| {
            let dialog = rfd::FileDialog::new().set_file_name(suggested);
            let dialog = if suggested.ends_with(".pdf") {
                dialog.add_filter("PDF document (.pdf)", &["pdf"])
            } else {
                dialog.add_filter("Web page (.html)", &["html", "htm"])
            };
            dialog.save_file()
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
