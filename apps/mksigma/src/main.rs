//! MK Sigma desktop app.
//!
//! `mksigma [data.sav]` opens the file, or the sample dataset without one.
// A GUI program on Windows: no console window beside the app. Debug builds keep theirs.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, wgpu};
use mksigma_ui_egui::{Services, SigmaApp};

struct App(SigmaApp);

struct Splash(mksigma_ui_egui::splash::SplashScreen);

impl eframe::App for Splash {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
}

/// The graphics backends to try, best first; `None` is wgpu's own choice. On Windows that choice
/// is Vulkan, and some Intel Vulkan drivers crash the app at start (igvk64.dll, #72), so it is
/// DirectX 12, then OpenGL. `WGPU_BACKEND` overrides it everywhere.
fn backends() -> Vec<Option<wgpu::Backends>> {
    if cfg!(target_os = "windows") && wgpu::Backends::from_env().is_none() {
        vec![Some(wgpu::Backends::DX12), Some(wgpu::Backends::GL)]
    } else {
        vec![None]
    }
}

/// A centred window on the given graphics backends.
fn options(viewport: egui::ViewportBuilder, backends: Option<wgpu::Backends>) -> eframe::NativeOptions {
    let mut wgpu_options = WgpuConfiguration::default();
    if let (Some(backends), WgpuSetup::CreateNew(setup)) = (backends, &mut wgpu_options.wgpu_setup) {
        setup.instance_descriptor.backends = backends;
    }
    eframe::NativeOptions { viewport, centered: true, wgpu_options, ..Default::default() }
}

/// The splash screen in a small window of its own, centred and undecorated, until it closes.
fn splash(backends: Option<wgpu::Backends>) -> eframe::Result {
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
    eframe::run_native("MK Sigma splash", options(viewport, backends), Box::new(|_cc| Ok(Box::new(Splash(Default::default())))))
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
    // The splash also finds a graphics backend that starts, for the main window to use; without
    // its splash (any other error) the program still starts.
    let candidates = backends();
    let backends = candidates
        .iter()
        .copied()
        .find(|b| !matches!(splash(*b), Err(eframe::Error::Wgpu(_))))
        .unwrap_or_else(|| candidates.first().copied().flatten());
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
                let _ = mksigma_engine::write_atomic(&file, code.as_bytes());
            }
        })),
        save_recent: Some(Box::new(|files: &[std::path::PathBuf]| {
            if let Some(file) = recent_file() {
                let text: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
                let _ = file.parent().map(std::fs::create_dir_all);
                let _ = mksigma_engine::write_atomic(&file, text.join("\n").as_bytes());
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
    eframe::run_native("MK Sigma", options(viewport, backends), Box::new(|_cc| Ok(Box::new(App(app)))))
}
