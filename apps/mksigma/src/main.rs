//! MK Sigma desktop app.
//!
//! `mksigma [data.sav]` opens the file, or the sample dataset without one.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mksigma_ui_egui::{Services, SigmaApp};

struct App(SigmaApp);

struct Splash(mksigma_ui_egui::splash::SplashScreen);

impl eframe::App for Splash {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
}

/// The splash screen in a small window of its own, centred and undecorated, until it closes.
fn splash() -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_title("MK Sigma")
        .with_app_id("mksigma")
        .with_inner_size(mksigma_ui_egui::splash::SIZE)
        .with_decorations(false)
        .with_resizable(false)
        .with_taskbar(false);
    let viewport = match mksigma_ui_egui::logo::icon() {
        Some(icon) => viewport.with_icon(icon),
        None => viewport,
    };
    let options = eframe::NativeOptions { viewport, centered: true, ..Default::default() };
    eframe::run_native("MK Sigma splash", options, Box::new(|_cc| Ok(Box::new(Splash(Default::default())))))
}

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

/// Where the recent files are kept: the platform's per-user configuration folder.
fn recent_file() -> Option<std::path::PathBuf> {
    settings_file("recent.txt")
}

/// A file in the platform's per-user configuration folder, under `mksigma`.
#[allow(clippy::disallowed_methods)] // the desktop app is native only
fn settings_file(name: &str) -> Option<std::path::PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(std::path::PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
    };
    base.map(|b| b.join("mksigma").join(name))
}

/// The interface's language: the one chosen in an earlier run, else the system's.
#[allow(clippy::disallowed_methods)] // the desktop app is native only
fn language() -> mksigma_ui_egui::i18n::Lang {
    use mksigma_ui_egui::i18n::Lang;
    let saved = settings_file("language.txt").and_then(|f| std::fs::read_to_string(f).ok()).and_then(|code| Lang::from_code(&code));
    saved.unwrap_or_else(|| {
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty())).unwrap_or_default();
        Lang::from_locale(&locale)
    })
}

fn main() -> eframe::Result {
    mksigma_ui_egui::i18n::set(language());
    // Without its splash the program still starts.
    let _ = splash();
    let services = Services {
        pick_open: Some(Box::new(|| {
            rfd::FileDialog::new()
                .add_filter(
                    "Data and dashboards (.sav, .dta, .xpt, .csv, .tsv, .txt, .sps)",
                    &["sav", "zsav", "dta", "xpt", "csv", "tsv", "txt", "dat", "sps"],
                )
                .add_filter("SPSS Statistics data (.sav, .zsav)", &["sav", "zsav"])
                .add_filter("Stata data (.dta)", &["dta"])
                .add_filter("SAS transport (.xpt)", &["xpt"])
                .add_filter("Delimited text (.csv, .tsv, .txt)", &["csv", "tsv", "txt", "dat"])
                .add_filter("All files", &["*"])
                .pick_file()
        })),
        pick_save: Some(Box::new(|suggested: &str| {
            rfd::FileDialog::new()
                .add_filter("SPSS Statistics data (.sav)", &["sav"])
                .add_filter("Stata data (.dta)", &["dta"])
                .set_file_name(suggested)
                .save_file()
        })),
        now: Some(Box::new(now)),
        pick_export: Some(Box::new(|suggested: &str| {
            let dialog = rfd::FileDialog::new().set_file_name(suggested);
            let dialog = if suggested.ends_with(".pdf") {
                dialog.add_filter("PDF document (.pdf)", &["pdf"])
            } else if suggested.ends_with(".tex") {
                dialog.add_filter("LaTeX document (.tex)", &["tex"])
            } else if suggested.ends_with(".sps") {
                dialog.add_filter("MK Sigma dashboard syntax (.sps)", &["sps"])
            } else {
                dialog.add_filter("Web page (.html)", &["html", "htm"])
            };
            dialog.save_file()
        })),
        load_recent: Some(Box::new(|| {
            recent_file()
                .and_then(|f| std::fs::read_to_string(f).ok())
                .map(|text| text.lines().filter(|l| !l.trim().is_empty()).map(std::path::PathBuf::from).collect())
                .unwrap_or_default()
        })),
        load_language: None,
        save_language: Some(Box::new(|code: &str| {
            if let Some(file) = settings_file("language.txt") {
                let _ = file.parent().map(std::fs::create_dir_all);
                let _ = std::fs::write(file, code);
            }
        })),
        save_recent: Some(Box::new(|files: &[std::path::PathBuf]| {
            if let Some(file) = recent_file() {
                let text: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
                let _ = file.parent().map(std::fs::create_dir_all);
                let _ = std::fs::write(file, text.join("\n"));
            }
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
    let viewport = match mksigma_ui_egui::logo::icon() {
        Some(icon) => viewport.with_icon(icon),
        None => viewport,
    };
    let options = eframe::NativeOptions { viewport, centered: true, ..Default::default() };
    eframe::run_native("MK Sigma", options, Box::new(|_cc| Ok(Box::new(App(app)))))
}
