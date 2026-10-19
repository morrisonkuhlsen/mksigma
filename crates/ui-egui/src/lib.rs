//! MK Sigma egui frontend: a File menu, Data View and Variable View (both editable), a syntax
//! editor with the session history, and the Output Viewer. It is thin: it reads the session and
//! acts through `Session`; nothing below this crate depends on egui. Platform services (the
//! native file dialog) come in through `Services`, so tests drive the UI without them.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::{Path, PathBuf};

use mksigma_engine::Session;
use mksigma_i18n::tr;

mod backstage;
mod edit;
mod explain;
mod icons;
pub mod logo;
pub use mksigma_i18n as i18n;
mod math;
mod output;
mod procedure;
mod ribbon;
pub mod splash;
mod suggest;
mod theme;
mod views;

pub use backstage::Page as BackstagePage;
pub use edit::{Editing, Field, Target};
pub use explain::ExplainView;
pub use output::{ExportFormat, OutputState};
pub use procedure::{Dialog as ProcedureDialog, Kind as ProcedureKind};
pub use ribbon::Tab;
pub use suggest::Procedure;
pub use views::ValueLabelsDialog;

use icons::Icon;
use theme::Tokens;

const SHEET_TABS_HEIGHT: f32 = 30.0;
const STATUS_HEIGHT: f32 = 24.0;

use edit::{Commit, syntax_for, value_labels_syntax};
use views::DialogOutcome;

/// A file dialog: shows it (with a suggested name, for saving) and returns the chosen path, or
/// `None` when cancelled.
pub type PickOpen = Box<dyn Fn() -> Option<PathBuf>>;
pub type PickSave = Box<dyn Fn(&str) -> Option<PathBuf>>;
/// The recent files kept from earlier runs, and keeping them.
pub type LoadRecent = Box<dyn Fn() -> Vec<PathBuf>>;
pub type SaveRecent = Box<dyn Fn(&[PathBuf])>;
pub type LoadLanguage = Box<dyn Fn() -> Option<String>>;
pub type SaveLanguage = Box<dyn Fn(&str)>;

/// What the app provides from the platform.
#[derive(Default)]
pub struct Services {
    /// Shows an open-file dialog; `None` when cancelled.
    pub pick_open: Option<PickOpen>,
    /// Shows a save-file dialog suggesting a file name; `None` when cancelled.
    pub pick_save: Option<PickSave>,
    /// Seconds since 1970, for the saved file's creation time.
    pub now: Option<Box<dyn Fn() -> i64>>,
    /// Shows a save dialog for exporting the Output, suggesting a file name whose extension
    /// (`.html` or `.pdf`) says the format.
    pub pick_export: Option<PickSave>,
    /// The recent files kept from earlier runs, newest first.
    pub load_recent: Option<LoadRecent>,
    /// Keeps the recent files for later runs.
    pub save_recent: Option<SaveRecent>,
    /// The language chosen in an earlier run (its code), if any.
    pub load_language: Option<LoadLanguage>,
    /// Keeps the chosen language for later runs.
    pub save_language: Option<SaveLanguage>,
}

/// How many recent files the backstage lists.
const RECENT: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Data,
    Variables,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Info(String),
    Warning(String),
    Error(String),
}

pub struct SigmaApp {
    pub session: Session,
    pub view: View,
    /// Show value labels instead of values in Data View.
    pub value_labels: bool,
    pub syntax: String,
    pub status: Option<Status>,
    /// Name of the open file, shown in the window title.
    pub file_name: Option<String>,
    /// Where Save writes: the file opened or last saved, if it came from disk.
    pub file_path: Option<PathBuf>,
    /// The cell being edited, if any.
    pub editing: Option<Editing>,
    pub value_labels_dialog: Option<ValueLabelsDialog>,
    /// Which Output tables are pivoted.
    pub output_state: OutputState,
    /// The File backstage's page while it is open.
    pub backstage: Option<BackstagePage>,
    /// Recent files, newest first.
    pub recent: Vec<PathBuf>,
    /// What waits for an answer about unsaved changes.
    pending: Option<backstage::Action>,
    /// The procedure dialog open, if any.
    pub procedure_dialog: Option<procedure::Dialog>,
    /// The ribbon tab shown.
    pub tab: Tab,
    /// Only the ribbon's tab row shows, for more room.
    pub ribbon_collapsed: bool,
    /// Save to the file after every change (only when the data has a file).
    pub autosave: bool,
    /// The search box's text, while it is open.
    pub search: Option<String>,
    /// The window's zoom, in percent.
    pub zoom: u32,
    services: Services,
    title: String,
    themed: bool,
    /// The Σ window: an Output item's calculation by hand.
    pub explain: Option<explain::ExplainView>,
}

impl Default for SigmaApp {
    fn default() -> Self {
        Self::new(Services::default())
    }
}

pub(crate) const ROW_HEIGHT: f32 = 20.0;

impl SigmaApp {
    pub fn new(services: Services) -> Self {
        let (session, status) = match Session::with_sample() {
            Ok(s) => (s, Some(Status::Info(tr!("status-sample-start")))),
            Err(e) => (Session::new(), Some(Status::Error(e.to_string()))),
        };
        // The language of an earlier run.
        if let Some(lang) = services.load_language.as_ref().and_then(|load| load()).and_then(|code| mksigma_i18n::Lang::from_code(&code)) {
            mksigma_i18n::set(lang);
        }
        Self {
            session,
            view: View::Data,
            value_labels: false,
            syntax: "FREQUENCIES VARIABLES=age /FORMAT=NOTABLE /STATISTICS.\nDESCRIPTIVES VARIABLES=age income score.".to_string(),
            status,
            file_name: None,
            file_path: None,
            editing: None,
            value_labels_dialog: None,
            output_state: OutputState::default(),
            tab: Tab::default(),
            procedure_dialog: None,
            backstage: None,
            recent: services.load_recent.as_ref().map(|load| load()).unwrap_or_default(),
            pending: None,
            ribbon_collapsed: false,
            autosave: false,
            search: None,
            zoom: 100,
            services,
            title: String::new(),
            themed: false,
            explain: None,
        }
    }

    /// Speaks `lang` from now on, and remembers it.
    pub fn set_language(&mut self, lang: mksigma_i18n::Lang) {
        mksigma_i18n::set(lang);
        self.session.language_changed();
        if let Some(save) = &self.services.save_language {
            save(lang.code());
        }
    }

    /// Opens a system file from disk.
    pub fn open_path(&mut self, path: &Path) {
        if is_text(path) {
            // Read through syntax, so the History says how the text was read.
            let mut syntax = format!("GET DATA /TYPE=TXT /FILE={}", mksigma_engine::quote(&path.display().to_string()));
            if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("tsv")) {
                syntax.push_str(" /DELIMITERS=\"\\t\"");
            }
            syntax.push('.');
            let result = self.session.run_syntax(&syntax);
            if result.is_ok() {
                self.remember(path);
            }
            self.after_run(result.map_err(|e| e.to_string()), None);
            return;
        }
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("sps")) {
            self.open_syntax(path);
            return;
        }
        let syntax = format!("GET FILE={}.", mksigma_engine::quote(&path.display().to_string()));
        let result = self.session.run_syntax(&syntax);
        if result.is_ok() {
            self.remember(path);
        }
        self.after_run(result.map_err(|e| e.to_string()), None);
    }

    /// Opens a syntax file into the editor. A saved dashboard (it has `DASHBOARD`) runs at once
    /// on an empty Output and shows the dashboard; other syntax waits for Run.
    pub fn open_syntax(&mut self, path: &Path) {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) => {
                self.status = Some(Status::Error(format!("{}: {e}", path.display())));
                return;
            }
        };
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        self.syntax = text.clone();
        let is_dashboard = mksigma_engine::check_syntax(&text).is_ok() && text.to_ascii_uppercase().contains("DASHBOARD");
        if !is_dashboard {
            self.status = Some(Status::Info(tr!("status-in-syntax-editor", name = name.as_str())));
            return;
        }
        self.session.clear_output();
        self.output_state = OutputState::default();
        let result = self.session.run_syntax(&text).map_err(|e| format!("{name}: {e}"));
        let ok = result.is_ok();
        self.after_run(result, None);
        if ok {
            self.remember(path);
            self.output_state.dashboard = true;
            self.status = Some(Status::Info(tr!("status-opened-dashboard", name = name.as_str())));
        }
    }

    fn export_dashboard_dialog(&mut self) {
        let stem = self.file_name.as_deref().and_then(|f| f.rsplit_once('.').map(|(s, _)| s.to_string())).unwrap_or_else(|| "dashboard".to_string());
        if let Some(path) = self.services.pick_export.as_ref().and_then(|pick| pick(&format!("{stem} dashboard.html"))) {
            self.export_dashboard(&path);
        }
    }

    /// Puts `path` first among the recent files and keeps the list.
    fn remember(&mut self, path: &Path) {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        self.recent.retain(|p| *p != path);
        self.recent.insert(0, path);
        self.recent.truncate(RECENT);
        if let Some(save) = &self.services.save_recent {
            save(&self.recent);
        }
    }

    /// Replaces the session with new data (blank or the sample), leaving the backstage.
    fn new_data(&mut self, session: Session, what: &str) {
        self.session = session;
        self.file_name = None;
        self.file_path = None;
        self.editing = None;
        self.value_labels_dialog = None;
        self.output_state = OutputState::default();
        self.backstage = None;
        self.pending = None;
        self.status = Some(Status::Info(what.to_string()));
    }

    /// Does what the backstage asked for. Replacing data with unsaved changes asks first, on
    /// the Info page.
    fn backstage_act(&mut self, action: backstage::Action) {
        use backstage::Action as A;
        let replaces = matches!(action, A::Blank | A::Sample | A::Close);
        if replaces && self.session.modified && self.pending.is_none() {
            self.pending = Some(action);
            self.backstage = Some(backstage::Page::Info);
            return;
        }
        match action {
            A::Back => {
                self.backstage = None;
                self.pending = None;
            }
            A::Page(page) => {
                self.backstage = Some(page);
                self.pending = None;
            }
            A::Blank | A::Close => self.new_data(Session::new(), &tr!("status-new-dataset")),
            A::Sample => match Session::with_sample() {
                Ok(s) => self.new_data(s, &tr!("status-sample")),
                Err(e) => self.status = Some(Status::Error(e.to_string())),
            },
            A::Discard => {
                let then = self.pending.take().unwrap_or(A::Close);
                self.session.modified = false;
                self.backstage_act(then);
            }
            A::Open(path) => {
                self.backstage = None;
                self.open_path(&path);
            }
            A::Browse => {
                self.backstage = None;
                self.open_dialog();
            }
            A::Save => {
                self.save();
                if !self.session.modified {
                    self.pending = None;
                    self.backstage = None;
                }
            }
            A::SaveAs => {
                self.backstage = None;
                self.save_as();
            }
            A::Export(format) => {
                self.backstage = None;
                self.export_dialog(format);
            }
            A::ExportDashboard => {
                self.backstage = None;
                self.export_dashboard_dialog();
            }
            A::SaveDashboard => {
                self.backstage = None;
                self.save_dashboard_dialog();
            }
        }
    }

    /// Writes the dashboard as syntax that rebuilds it: the data's file, the steps, the
    /// analyses, the slicers and the tiles.
    pub fn save_dashboard(&mut self, path: &Path) {
        let Some(syntax) = self.session.dashboard_syntax() else {
            self.status = Some(Status::Error(tr!("status-dashboard-needs-file")));
            return;
        };
        match std::fs::write(path, syntax) {
            Ok(()) => self.status = Some(Status::Info(tr!("status-saved-dashboard", path = path.display().to_string()))),
            Err(e) => self.status = Some(Status::Error(format!("{}: {e}", path.display()))),
        }
    }

    fn save_dashboard_dialog(&mut self) {
        let stem = self.file_name.as_deref().and_then(|f| f.rsplit_once('.').map(|(s, _)| s.to_string())).unwrap_or_else(|| "dashboard".to_string());
        if let Some(path) = self.services.pick_export.as_ref().and_then(|pick| pick(&format!("{stem} dashboard.sps"))) {
            self.save_dashboard(&path);
        }
    }

    /// Writes the Output as a self-contained HTML page or a PDF document.
    pub fn export(&mut self, path: &Path, format: ExportFormat) {
        let mut title = self.file_name.as_deref().map_or_else(|| tr!("export-title-output"), |f| tr!("export-title-output-of", file = f));
        // A filtered Output says so, or a reader would take its items for all the cases.
        if let Some(filters) = self.session.filter_description() {
            title.push(' ');
            title.push_str(&tr!("export-title-filtered", filters = filters));
        }
        let bytes = match format {
            ExportFormat::Html => self.session.output.to_html(&title).into_bytes(),
            ExportFormat::Pdf => self.session.output.to_pdf(&title),
            ExportFormat::Latex => self.session.output.to_latex(&title).into_bytes(),
        };
        match std::fs::write(path, bytes) {
            Ok(()) => self.status = Some(Status::Info(tr!("status-exported-output", path = path.display().to_string()))),
            Err(e) => self.status = Some(Status::Error(format!("{}: {e}", path.display()))),
        }
    }

    /// Writes the dashboard as a self-contained web page: its tiles in a grid, as filtered now.
    pub fn export_dashboard(&mut self, path: &Path) {
        let title = self.file_name.as_deref().map_or_else(|| tr!("export-title-dashboard"), |f| tr!("export-title-dashboard-of", file = f));
        // Interactive when the engine was built for WebAssembly: slicers and cross-filtering
        // work in the page, which holds the data. Otherwise the cards as they are now.
        let interactive = !mksigma_web::ENGINE.is_empty();
        let html = if interactive {
            match mksigma_web::page(&self.session, &title, mksigma_web::ENGINE) {
                Ok(html) => html,
                Err(e) => {
                    self.status = Some(Status::Error(e));
                    return;
                }
            }
        } else {
            let (board, wide) = self.session.dashboard_output();
            board.to_dashboard_html(&title, self.session.filter_description().as_deref(), &wide)
        };
        let what = if interactive { tr!("status-exported-interactive") } else { tr!("status-exported-static") };
        match std::fs::write(path, html) {
            Ok(()) => self.status = Some(Status::Info(tr!("status-exported-dashboard", what = what, path = path.display().to_string()))),
            Err(e) => self.status = Some(Status::Error(format!("{}: {e}", path.display()))),
        }
    }

    fn export_dialog(&mut self, format: ExportFormat) {
        let stem = self.file_name.as_deref().and_then(|f| f.rsplit_once('.').map(|(s, _)| s.to_string())).unwrap_or_else(|| "output".to_string());
        let extension = match format {
            ExportFormat::Html => "html",
            ExportFormat::Pdf => "pdf",
            ExportFormat::Latex => "tex",
        };
        if let Some(path) = self.services.pick_export.as_ref().and_then(|pick| pick(&format!("{stem}.{extension}"))) {
            self.export(&path, format);
        }
    }

    /// Saves to the current file, or asks where when there is none.
    pub fn save(&mut self) {
        match self.file_path.clone() {
            Some(path) => self.save_to(&path),
            None => self.save_as(),
        }
    }

    pub fn save_as(&mut self) {
        // Data read from text is saved as a system file next to it, never over it.
        let suggested = match self.file_name.as_deref() {
            Some(name) => match name.rsplit_once('.') {
                Some((stem, _)) if is_text(Path::new(name)) => format!("{stem}.sav"),
                _ => name.to_string(),
            },
            None => "Untitled.sav".to_string(),
        };
        if let Some(path) = self.services.pick_save.as_ref().and_then(|pick| pick(&suggested)) {
            self.save_to(&path);
        }
    }

    /// Writes next to the target and renames over it, so a failed save never leaves a half-written
    /// file in place of the old one.
    pub fn save_to(&mut self, path: &Path) {
        let now = self.services.now.as_ref().map_or(0, |now| now());
        let (bytes, warnings) = match self.session.save_sav(mksigma_engine::timestamp(now)) {
            Ok(written) => written,
            Err(e) => {
                self.status = Some(Status::Error(e.to_string()));
                return;
            }
        };
        let temporary = path.with_extension("sav.saving");
        let written = std::fs::write(&temporary, &bytes).and_then(|()| std::fs::rename(&temporary, path));
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        match written {
            Ok(()) => {
                let summary = tr!("status-saved", name = name.as_str());
                self.status =
                    Some(if warnings.is_empty() { Status::Info(summary) } else { Status::Warning(format!("{summary}. {}", warnings.join("; "))) });
                self.file_name = Some(name);
                self.file_path = Some(path.to_path_buf());
                self.session.saved_to(&path.display().to_string());
                self.remember(path);
            }
            Err(e) => {
                let _ = std::fs::remove_file(&temporary);
                // The data wasn't written, so it still has unsaved changes.
                self.session.modified = true;
                self.status = Some(Status::Error(format!("{name}: {e}")));
            }
        }
    }

    /// Opens a system file's bytes (a path read, or a file dropped where there is no path).
    /// Returns whether it opened; on failure the open dataset stays.
    pub fn open_bytes(&mut self, name: &str, bytes: &[u8]) -> bool {
        match self.session.open_sav(bytes) {
            Ok(warnings) => {
                let ds = &self.session.dataset;
                let summary = tr!("status-opened", name = name, cases = ds.n_cases(), variables = ds.variables().len());
                self.status =
                    Some(if warnings.is_empty() { Status::Info(summary) } else { Status::Warning(format!("{summary}. {}", warnings.join("; "))) });
                self.file_name = Some(name.to_string());
                self.file_path = None;
                self.editing = None;
                self.value_labels_dialog = None;
                true
            }
            Err(e) => {
                self.status = Some(Status::Error(format!("{name}: {e}")));
                false
            }
        }
    }

    fn open_dialog(&mut self) {
        if let Some(path) = self.services.pick_open.as_ref().and_then(|pick| pick()) {
            self.open_path(&path);
        }
    }

    fn run_syntax(&mut self) {
        let result = self.session.run_syntax(&self.syntax).map_err(|e| e.to_string());
        self.after_run(result, None);
    }

    /// The status after running syntax: an error, what loading data said (`GET DATA` also
    /// makes its file the document), or `done`.
    fn after_run(&mut self, result: Result<(), String>, done: Option<Status>) {
        let notes = self.session.take_notes();
        if let Some(file) = self.session.take_loaded_file() {
            let path = Path::new(&file);
            self.file_name = Some(path.file_name().map_or_else(|| file.clone(), |n| n.to_string_lossy().into_owned()));
            // Save writes a system file back, never over text.
            self.file_path = (!is_text(path)).then(|| path.to_path_buf());
            self.editing = None;
            self.value_labels_dialog = None;
        }
        self.status = match result {
            Err(e) => Some(Status::Error(e)),
            Ok(()) if notes.len() > 1 => Some(Status::Warning(notes.join(". "))),
            Ok(()) if !notes.is_empty() => Some(Status::Info(notes.join(". "))),
            Ok(()) => done,
        };
    }

    /// Shows an engine error in the status line.
    fn report(&mut self, result: Result<(), mksigma_engine::EngineError>) {
        if let Err(e) = result {
            self.status = Some(Status::Error(e.to_string()));
        }
    }

    pub fn undo(&mut self) {
        let label = self.session.undo_label().map(str::to_string);
        self.editing = None;
        match self.session.undo() {
            Ok(true) => self.status = label.map(|l| Status::Info(tr!("status-undid", what = l))),
            Ok(false) => {}
            Err(e) => self.status = Some(Status::Error(e.to_string())),
        }
    }

    pub fn redo(&mut self) {
        let label = self.session.redo_label().map(str::to_string);
        self.editing = None;
        match self.session.redo() {
            Ok(true) => self.status = label.map(|l| Status::Info(tr!("status-redid", what = l))),
            Ok(false) => {}
            Err(e) => self.status = Some(Status::Error(e.to_string())),
        }
    }

    /// Runs syntax generated by an edit; the status line shows what ran, or why it didn't.
    pub fn run_edit(&mut self, syntax: &str) {
        let result = self.session.run_syntax(syntax).map_err(|e| format!("{e} ({syntax})"));
        self.after_run(result, Some(Status::Info(syntax.to_string())));
    }

    /// Opens delimited text from its bytes (a dropped file with no path), with the delimiter
    /// detected. Returns whether it opened.
    pub fn open_text_bytes(&mut self, name: &str, bytes: &[u8]) -> bool {
        let tsv = name.to_ascii_lowercase().ends_with(".tsv");
        let options = mksigma_engine::csv::Options { delimiters: if tsv { vec!['\t'] } else { Vec::new() }, ..Default::default() };
        match self.session.open_text(bytes, &options) {
            Ok(warnings) => {
                let ds = &self.session.dataset;
                let summary = tr!("status-opened", name = name, cases = ds.n_cases(), variables = ds.variables().len());
                self.status =
                    Some(if warnings.is_empty() { Status::Info(summary) } else { Status::Warning(format!("{summary}. {}", warnings.join("; "))) });
                self.file_name = Some(name.to_string());
                self.file_path = None;
                self.editing = None;
                self.value_labels_dialog = None;
                true
            }
            Err(e) => {
                self.status = Some(Status::Error(format!("{name}: {e}")));
                false
            }
        }
    }

    fn apply_edit(&mut self, target: Target, text: &str) {
        match target {
            Target::Step(i) => {
                let unchanged = self.session.steps().get(i).and_then(|s| s.syntax()).is_some_and(|s| s.trim() == text.trim());
                if !unchanged {
                    let result = self.session.replace_step(i, text);
                    self.report(result);
                }
            }
            Target::Cell { case, var } => {
                if let Err(e) = self.session.set_cell(case, var, text) {
                    self.status = Some(Status::Error(e.to_string()));
                }
            }
            Target::Variable(i, Field::Name) if i == self.session.dataset.variables().len() => {
                let name = text.trim();
                if !name.is_empty() {
                    self.run_edit(&format!("NUMERIC {name}."));
                }
            }
            Target::Variable(i, field) => {
                let syntax = self.session.dataset.variables().get(i).and_then(|var| syntax_for(var, field, text));
                if let Some(syntax) = syntax {
                    self.run_edit(&syntax);
                }
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if !self.themed {
            theme::apply(ui.ctx());
            self.themed = true;
        }
        self.handle_input(ui.ctx());
        let untitled = tr!("untitled");
        let title = format!("{}{} — MK Sigma", if self.session.modified { "• " } else { "" }, self.file_name.as_deref().unwrap_or(&untitled));
        if title != self.title {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        let t = Tokens::get(ui.ctx());
        // The window's own background: a bare root Ui (as eframe gives) paints none.
        ui.painter().rect_filled(ui.max_rect().expand(8.0), 0.0, t.window);
        ui.spacing_mut().item_spacing.y = 0.0;

        let document = format!("{}{}", if self.session.modified { "• " } else { "" }, self.file_name.as_deref().unwrap_or(&untitled));
        let state = ribbon::State {
            title: &document,
            undo: self.session.undo_label(),
            redo: self.session.redo_label(),
            has_output: !self.session.output.items.is_empty(),
            value_labels: self.value_labels,
            view: self.view,
            autosave: self.autosave,
            style: self.session.output.style,
        };
        let mut actions = Vec::new();
        actions.extend(ribbon::title_bar(ui, &state));
        // The File backstage takes the whole window under the title row.
        if let Some(page) = self.backstage {
            for action in actions {
                self.act(ui.ctx(), action);
            }
            let ds = &self.session.dataset;
            let view = backstage::View {
                page,
                recent: &self.recent,
                file_name: self.file_name.as_deref(),
                file_path: self.file_path.as_deref(),
                modified: self.session.modified,
                cases: ds.n_cases(),
                variables: ds.variables().len(),
                weight: ds.weight.and_then(|w| ds.variables().get(w)).map(|v| v.name.as_str()),
                steps: self.session.steps().len(),
                has_output: !self.session.output.items.is_empty(),
                has_dashboard: !self.session.dashboard().is_empty(),
                confirm_close: self.pending.is_some(),
            };
            if let Some(action) = backstage::show(ui, &view) {
                self.backstage_act(action);
            }
            self.dialogs(ui.ctx());
            return;
        }
        actions.extend(ribbon::ribbon(ui, &mut self.tab, &mut self.ribbon_collapsed, &state));
        for action in actions {
            self.act(ui.ctx(), action);
        }

        let bottom = SHEET_TABS_HEIGHT + STATUS_HEIGHT;
        let body = (ui.available_height() - bottom - 8.0).max(100.0);
        ui.add_space(6.0);
        // Data and Variable View get 62% of the width, the Output Viewer the rest; the dashboard,
        // like a report canvas, takes it all.
        let gap = 6.0;
        let dashboard = self.output_state.dashboard;
        let left_width = if dashboard { 0.0 } else { ((ui.available_width() - gap) * 0.62).floor() };
        let right_width = ui.available_width() - if dashboard { 0.0 } else { gap + left_width };
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(gap, 0.0);
            let top_down = egui::Layout::top_down(egui::Align::Min);
            if !dashboard {
                ui.allocate_ui_with_layout(egui::vec2(left_width, body), top_down, |ui| {
                    ui.set_width(left_width);
                    ui.set_height(body);
                    self.left_pane(ui);
                });
            }
            ui.allocate_ui_with_layout(egui::vec2(right_width, body), top_down, |ui| {
                ui.set_width(right_width);
                ui.set_height(body);
                theme::card(&t).fill(t.paper).show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    let action = output::output_view(ui, &self.session, &mut self.output_state);
                    self.output_action(action);
                });
            });
        });
        ui.add_space(2.0);
        self.sheet_tabs(ui);
        self.status_line(ui);
        self.dialogs(ui.ctx());
        self.search_box(ui.ctx());
        let zoom = self.zoom as f32 / 100.0;
        if (ui.ctx().zoom_factor() - zoom).abs() > 0.001 {
            ui.ctx().set_zoom_factor(zoom);
        }
        // AutoSave waits for an edit in progress to finish.
        if self.autosave
            && self.session.modified
            && self.editing.is_none()
            && let Some(path) = self.file_path.clone()
        {
            let before = self.status.clone();
            self.save_to(&path);
            // A save that worked says nothing, so it doesn't hide what the last command said.
            if matches!(self.status, Some(Status::Info(_)) | Some(Status::Warning(_))) && !self.session.modified {
                self.status = before;
            }
        }
    }

    /// The search box under the title row: type to find a command or a variable, Enter or a
    /// click picks it, Escape closes.
    fn search_box(&mut self, ctx: &egui::Context) {
        let Some(mut query) = self.search.take() else { return };
        let t = Tokens::get(ctx);
        let needle = query.trim().to_lowercase();
        let mut results: Vec<(String, ribbon::Action)> =
            ribbon::commands().into_iter().filter(|(name, _)| name.to_lowercase().contains(&needle)).map(|(n, a)| (n.to_string(), a)).collect();
        for var in self.session.dataset.variables() {
            if !needle.is_empty()
                && (var.name.to_lowercase().contains(&needle) || var.label.as_deref().is_some_and(|l| l.to_lowercase().contains(&needle)))
            {
                let label = var.label.as_deref().map_or_else(|| var.name.clone(), |l| format!("{} — {l}", var.name));
                results.push((tr!("search-variable", label = label), ribbon::Action::View(View::Variables)));
            }
        }
        results.truncate(10);
        let mut picked = None;
        let mut open = true;
        let width = 380.0;
        let at = egui::pos2(ctx.content_rect().right() - width - 12.0, ribbon::TITLE_HEIGHT);
        egui::Area::new(egui::Id::new("search")).fixed_pos(at).order(egui::Order::Foreground).show(ctx, |ui| {
            theme::card(&t).shadow(ui.visuals().popup_shadow).show(ui, |ui| {
                ui.set_width(width);
                ui.horizontal(|ui| {
                    icons::show(ui, Icon::Search, 16.0);
                    let edit = ui.add(egui::TextEdit::singleline(&mut query).hint_text(tr!("search-hint")).desired_width(f32::INFINITY));
                    if !edit.has_focus() && !edit.lost_focus() {
                        edit.request_focus();
                    }
                    if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        picked = results.first().map(|(_, a)| a.clone());
                    }
                });
                ui.add_space(4.0);
                for (name, action) in &results {
                    if ui.add(egui::Button::new(name.as_str()).frame(false).min_size(egui::vec2(width, 22.0))).clicked() {
                        picked = Some(action.clone());
                    }
                }
                if results.is_empty() {
                    ui.label(egui::RichText::new(tr!("search-nothing")).color(t.text_dim));
                }
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            open = false;
        }
        if let Some(action) = picked {
            open = false;
            self.act(ctx, action);
        }
        if open {
            self.search = Some(query);
        }
    }

    fn output_action(&mut self, action: output::OutputAction) {
        match action {
            output::OutputAction::None => {}
            output::OutputAction::Clear => self.session.clear_output(),
            output::OutputAction::Select(item, category) => {
                // Filtering changes which items there are, so pivots by index would land on others.
                self.output_state.transposed.clear();
                if let Err(e) = self.session.select_category(item, category) {
                    self.status = Some(Status::Error(e.to_string()));
                }
            }
            output::OutputAction::Pin(item) => {
                let result = self.session.toggle_pin(item).map(drop);
                self.report(result);
            }
            output::OutputAction::MoveTile(tile, earlier) => {
                let result = self.session.move_tile(tile, earlier);
                self.report(result);
            }
            output::OutputAction::SaveDashboard => self.save_dashboard_dialog(),
            output::OutputAction::Wide(item, wide) => {
                let result = self.session.set_wide(item, wide);
                self.report(result);
            }
            output::OutputAction::ExportDashboard => self.export_dashboard_dialog(),
            output::OutputAction::Syntax(syntax) => {
                self.output_state.transposed.clear();
                self.run_edit(&syntax);
            }
            output::OutputAction::Explain(item) => self.explain = Some(explain::ExplainView::new(&self.session, item)),
            output::OutputAction::ClearFilter => {
                self.output_state.transposed.clear();
                self.session.clear_cross_filter();
            }
        }
    }

    /// Does what a ribbon or title-row button asked for.
    fn act(&mut self, ctx: &egui::Context, action: ribbon::Action) {
        use ribbon::Action;
        match action {
            Action::Open => self.open_dialog(),
            Action::Save => self.save(),
            Action::SaveAs => self.save_as(),
            Action::Undo => self.undo(),
            Action::Redo => self.redo(),
            Action::Run => self.run_syntax(),
            Action::Export(format) => self.export_dialog(format),
            Action::ClearOutput => {
                self.output_state.transposed.clear();
                self.session.clear_output();
            }
            Action::InsertCase => {
                let n = self.session.dataset.n_cases();
                let result = self.session.insert_case(n);
                self.report(result);
            }
            Action::InsertVariable => {
                // The first free VAR00001-style name, as SPSS names new variables.
                let name = (1..100_000).map(|i| format!("VAR{i:05}")).find(|n| self.session.dataset.variable(n).is_none());
                if let Some(name) = name {
                    self.run_edit(&format!("NUMERIC {name}."));
                }
            }
            Action::ToggleLabels => self.value_labels = !self.value_labels,
            Action::View(view) => self.view = view,
            Action::Suggest(procedure) => self.paste(procedure),
            Action::Theme(preference) => ctx.set_theme(preference),
            Action::Language(lang) => self.set_language(lang),
            Action::SaveDashboard => self.save_dashboard_dialog(),
            Action::File => self.backstage = Some(backstage::Page::Home),
            Action::Dialog(kind) => self.procedure_dialog = Some(procedure::Dialog::new(kind)),
            Action::Syntax(syntax) => self.run_edit(&syntax),
            Action::Search => self.search = if self.search.is_some() { None } else { Some(String::new()) },
            Action::ToggleAutoSave => {
                if self.autosave {
                    self.autosave = false;
                } else {
                    // AutoSave needs a file: ask for one first.
                    if self.file_path.is_none() {
                        self.save_as();
                    }
                    self.autosave = self.file_path.is_some();
                }
            }
        }
    }

    /// Adds a procedure's syntax to the editor, and runs it when it needs nothing completed.
    pub fn paste(&mut self, procedure: suggest::Procedure) {
        let suggestion = suggest::suggest(procedure, &self.session.dataset);
        if !self.syntax.is_empty() && !self.syntax.ends_with('\n') {
            self.syntax.push('\n');
        }
        self.syntax.push_str(&suggestion.syntax);
        self.status = Some(if suggestion.ready {
            match self.session.run_syntax(&suggestion.syntax) {
                Ok(()) => Status::Info(suggestion.syntax),
                Err(e) => Status::Error(format!("{e} ({})", suggestion.syntax)),
            }
        } else {
            Status::Info(tr!("status-template-added"))
        });
    }

    /// Data View and Variable View as tabs under the grid, like a workbook's sheets.
    fn sheet_tabs(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), SHEET_TABS_HEIGHT), egui::Sense::hover());
        ui.painter().hline(rect.x_range(), rect.top(), egui::Stroke::new(1.0, t.border));
        let mut row =
            ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(8.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Min)));
        row.spacing_mut().item_spacing.x = 2.0;
        for (view, icon, name) in [(View::Data, Icon::Grid, tr!("data-view")), (View::Variables, Icon::List, tr!("variable-view"))] {
            let on = self.view == view;
            let galley = row.painter().layout_no_wrap(name.to_string(), egui::FontId::proportional(13.0), if on { t.accent } else { t.text_dim });
            let size = egui::vec2(galley.size().x + 40.0, SHEET_TABS_HEIGHT - 4.0);
            let (tab, response) = row.allocate_exact_size(size, egui::Sense::click());
            response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, &name));
            let painter = row.painter();
            if on {
                painter.rect_filled(tab, egui::CornerRadius { nw: 0, ne: 0, sw: 6, se: 6 }, t.ribbon);
                painter.rect_filled(
                    egui::Rect::from_min_max(tab.left_bottom() - egui::vec2(-10.0, 3.0), tab.right_bottom() - egui::vec2(10.0, 0.0)),
                    1.5,
                    t.accent,
                );
            } else if response.hovered() {
                painter.rect_filled(tab, 6.0, t.hover);
            }
            icons::paint(painter, egui::Rect::from_center_size(egui::pos2(tab.left() + 16.0, tab.center().y), egui::vec2(14.0, 14.0)), icon, &t);
            painter.galley(egui::pos2(tab.left() + 28.0, tab.center().y - galley.size().y / 2.0), galley, t.text);
            if response.clicked() {
                self.view = view;
            }
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let shortcut = |modifiers, key| ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(modifiers, key)));
        // The longer shortcut first, so Ctrl+Shift+S isn't taken as Ctrl+S.
        if shortcut(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::S) {
            self.save_as();
        } else if shortcut(egui::Modifiers::COMMAND, egui::Key::S) {
            self.save();
        }
        if shortcut(egui::Modifiers::COMMAND, egui::Key::O) {
            self.open_dialog();
        }
        if shortcut(egui::Modifiers::COMMAND, egui::Key::F1) {
            self.ribbon_collapsed = !self.ribbon_collapsed;
        }
        if shortcut(egui::Modifiers::COMMAND, egui::Key::F) {
            self.search = Some(String::new());
        }
        // Undo and redo the data only when no text box has the keyboard (they undo their own text).
        if ctx.memory(|m| m.focused().is_none()) {
            if shortcut(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z) || shortcut(egui::Modifiers::COMMAND, egui::Key::Y) {
                self.redo();
            } else if shortcut(egui::Modifiers::COMMAND, egui::Key::Z) {
                self.undo();
            }
        }
        let dropped = ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));
        for file in dropped {
            let path = file.path();
            let name = path.file_name().map_or_else(|| tr!("dropped-file"), |n| n.to_string_lossy().into_owned());
            if is_text(path) && path.exists() {
                self.open_path(path);
                continue;
            }
            match file.bytes() {
                Ok(bytes) if is_text(Path::new(&name)) => {
                    self.open_text_bytes(&name, &bytes);
                }
                Ok(bytes) => {
                    self.open_bytes(&name, &bytes);
                }
                Err(e) => self.status = Some(Status::Error(format!("{name}: {e}"))),
            }
        }
    }

    fn left_pane(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let syntax_height = 300.0;
        let table_height = (ui.available_height() - syntax_height).max(120.0);
        let mut edits = Vec::new();
        let mut commits = Vec::new();
        let grid = egui::Frame::new().fill(t.grid_bg).stroke(egui::Stroke::new(1.0, t.border)).corner_radius(8).inner_margin(egui::Margin::same(1));
        grid.show(ui, |ui| {
            let size = egui::vec2(ui.available_width(), table_height - 2.0);
            ui.allocate_ui(size, |ui| {
                ui.set_min_size(size);
                match self.view {
                    View::Data => views::data_view(ui, &self.session.dataset, self.value_labels, &mut self.editing, &mut edits, &mut commits),
                    View::Variables => views::variable_view(ui, &self.session.dataset, &mut self.editing, &mut edits, &mut commits),
                }
            });
        });
        for (target, text) in edits {
            self.apply_edit(target, &text);
        }
        for commit in commits {
            self.apply_commit(commit);
        }

        ui.add_space(6.0);
        theme::card(&t).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.horizontal(|ui| {
                icons::show(ui, Icon::List, 14.0);
                ui.label(egui::RichText::new(tr!("syntax")).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ribbon::accent_button(ui, Icon::Run, &tr!("run")).on_hover_text(tr!("run-tip")).clicked() {
                        self.run_syntax();
                    }
                });
            });
            ui.add(egui::TextEdit::multiline(&mut self.syntax).code_editor().desired_rows(5).desired_width(f32::INFINITY));
            self.steps_panel(ui);
        });
    }

    fn apply_commit(&mut self, commit: Commit) {
        let result = match commit {
            Commit::Syntax(syntax) => {
                self.run_edit(&syntax);
                Ok(())
            }
            Commit::OpenValueLabels(i) => {
                self.value_labels_dialog = self.session.dataset.variables().get(i).map(|v| ValueLabelsDialog::new(i, v));
                Ok(())
            }
            Commit::InsertCase(case) => self.session.insert_case(case),
            Commit::DeleteCase(case) => self.session.delete_case(case),
            Commit::EnableStep(i, enabled) => self.session.set_step_enabled(i, enabled),
            Commit::MoveStep(i, up) => self.session.move_step(i, up),
            Commit::RemoveStep(i) => self.session.remove_step(i),
        };
        self.report(result);
    }

    /// Applied steps, Power Query style: every change to the data since the file was opened.
    /// Switch a step off, click its command to rewrite it, move or remove it; the data is
    /// replayed from the file.
    fn steps_panel(&mut self, ui: &mut egui::Ui) {
        let mut edits = Vec::new();
        let mut commits = Vec::new();
        let mut paste = false;
        let steps = self.session.steps();
        let count = steps.len();
        egui::CollapsingHeader::new(tr!("steps-title", n = count)).id_salt("steps").default_open(true).show(ui, |ui| {
            if steps.iter().any(|s| s.enabled && s.syntax().is_some()) && ui.small_button(tr!("steps-paste")).clicked() {
                paste = true;
            }
            egui::ScrollArea::vertical().id_salt("steps-list").max_height(110.0).stick_to_bottom(true).show(ui, |ui| {
                let text_width = (ui.available_width() - 210.0).max(120.0);
                for (i, step) in steps.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let mut enabled = step.enabled;
                        if ui.checkbox(&mut enabled, "").on_hover_text(tr!("step-apply")).changed() {
                            commits.push(Commit::EnableStep(i, enabled));
                        }
                        // A step that failed the last replay: red number, the reason on hover.
                        let number = egui::RichText::new(format!("{:>2}", i + 1));
                        match &step.error {
                            Some(error) => {
                                ui.label(number.color(ui.visuals().error_fg_color)).on_hover_text(tr!("step-skipped", error = error.to_string()));
                            }
                            None => {
                                ui.label(number.weak());
                            }
                        }
                        let mut text = egui::RichText::new(step.label()).monospace();
                        if !step.enabled {
                            text = text.weak().strikethrough();
                        } else if step.error.is_some() {
                            text = text.color(ui.visuals().error_fg_color);
                        }
                        if step.syntax().is_some() {
                            let source = step.label();
                            edit::editable(ui, &mut self.editing, &mut edits, Target::Step(i), text, || source, text_width, ROW_HEIGHT, false);
                        } else {
                            ui.add_sized([text_width, ROW_HEIGHT], egui::Label::new(text).truncate());
                        }
                        // Plain words: the default font has no arrow or cross glyphs.
                        if ui.add_enabled(i > 0, egui::Button::new(tr!("step-up")).small()).on_hover_text(tr!("step-up-tip")).clicked() {
                            commits.push(Commit::MoveStep(i, true));
                        }
                        if ui.add_enabled(i + 1 < count, egui::Button::new(tr!("step-down")).small()).on_hover_text(tr!("step-down-tip")).clicked() {
                            commits.push(Commit::MoveStep(i, false));
                        }
                        if ui.small_button(tr!("step-remove")).on_hover_text(tr!("step-remove-tip")).clicked() {
                            commits.push(Commit::RemoveStep(i));
                        }
                    });
                }
            });
        });
        if paste {
            let lines: Vec<&str> = self.session.steps().iter().filter(|s| s.enabled).filter_map(|s| s.syntax()).collect();
            let text = lines.join("\n");
            if !self.syntax.ends_with('\n') && !self.syntax.is_empty() {
                self.syntax.push('\n');
            }
            self.syntax.push_str(&text);
        }
        for (target, text) in edits {
            self.apply_edit(target, &text);
        }
        for commit in commits {
            self.apply_commit(commit);
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(view) = self.explain.as_mut() {
            match explain::show(ctx, view, &self.session) {
                explain::Outcome::Open => {}
                explain::Outcome::Close => self.explain = None,
                explain::Outcome::Export(tex, name) => {
                    if let Some(path) = self.services.pick_export.as_ref().and_then(|pick| pick(&name)) {
                        match std::fs::write(&path, tex) {
                            Ok(()) => self.status = Some(Status::Info(tr!("status-exported-calculation", path = path.display().to_string()))),
                            Err(e) => self.status = Some(Status::Error(format!("{}: {e}", path.display()))),
                        }
                    }
                }
            }
        }
        if let Some(dialog) = self.procedure_dialog.as_mut() {
            match procedure::show(ctx, dialog, &self.session) {
                procedure::Outcome::Open => {}
                procedure::Outcome::Cancel => self.procedure_dialog = None,
                procedure::Outcome::Run(syntax) => {
                    self.procedure_dialog = None;
                    self.run_edit(&syntax);
                }
                procedure::Outcome::Paste(syntax) => {
                    self.procedure_dialog = None;
                    if !self.syntax.is_empty() && !self.syntax.ends_with('\n') {
                        self.syntax.push('\n');
                    }
                    self.syntax.push_str(&syntax);
                    self.status = Some(Status::Info(tr!("status-pasted")));
                }
            }
        }
        let Some(dialog) = self.value_labels_dialog.as_mut() else { return };
        let Some(var) = self.session.dataset.variables().get(dialog.var) else {
            self.value_labels_dialog = None;
            return;
        };
        match views::value_labels_dialog(ctx, dialog, &var.name) {
            DialogOutcome::Open => {}
            DialogOutcome::Cancel => self.value_labels_dialog = None,
            DialogOutcome::Ok => {
                let syntax = value_labels_syntax(var, &dialog.rows);
                self.value_labels_dialog = None;
                self.run_edit(&syntax);
            }
        }
    }

    /// The last message on the left; the data's size, weight and filter on the right.
    fn status_line(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), STATUS_HEIGHT), egui::Sense::hover());
        ui.painter().hline(rect.x_range(), rect.top(), egui::Stroke::new(1.0, t.border));
        let mut row = ui
            .new_child(egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(10.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let ready = tr!("status-ready");
        let (text, color) = match &self.status {
            Some(Status::Info(s)) => (s.as_str(), t.text_dim),
            Some(Status::Warning(s)) => (s.as_str(), t.warn),
            Some(Status::Error(s)) => (s.as_str(), t.danger),
            None => (ready.as_str(), t.text_dim),
        };
        let ds = &self.session.dataset;
        let mut facts = vec![tr!("fact-cases", n = ds.n_cases()), tr!("fact-variables", n = ds.variables().len())];
        if let Some(w) = ds.weight.and_then(|w| ds.variables().get(w)) {
            facts.push(tr!("fact-weighted", name = w.name.as_str()));
        }
        let mut zoom = self.zoom;
        row.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            // Zoom: − slider + and the percentage (click it for 100%).
            let percent = ui.add(egui::Label::new(egui::RichText::new(format!("{zoom}%")).color(t.text_dim).size(12.0)).sense(egui::Sense::click()));
            if percent.on_hover_text(tr!("zoom-reset")).clicked() {
                zoom = 100;
            }
            if zoom_button(ui, Icon::Plus, &tr!("zoom-in")) {
                zoom = (zoom / 10 * 10 + 10).min(200);
            }
            ui.spacing_mut().slider_width = 110.0;
            ui.add(egui::Slider::new(&mut zoom, 50..=200).show_value(false)).on_hover_text(tr!("zoom"));
            if zoom_button(ui, Icon::Minus, &tr!("zoom-out")) {
                zoom = (zoom.div_ceil(10) * 10).saturating_sub(10).max(50);
            }
            ui.add_space(16.0);
            ui.label(egui::RichText::new(facts.join("   ·   ")).color(t.text_dim).size(12.0));
            if let Some(filters) = self.session.filter_description() {
                ui.add_space(12.0);
                ui.label(egui::RichText::new(tr!("fact-filtered", filters = filters.as_str())).color(t.accent).size(12.0));
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(egui::RichText::new(text).color(color).size(12.0)).truncate());
            });
        });
        self.zoom = zoom;
    }
}

/// A small icon button for the status bar.
fn zoom_button(ui: &mut egui::Ui, icon: Icon, name: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    if response.hovered() {
        ui.painter().rect_filled(rect, 3.0, t.hover);
    }
    icons::paint(ui.painter(), rect.shrink(4.0), icon, &t);
    response.on_hover_text(name).clicked()
}

/// Whether a file is delimited text by its extension (`.csv`, `.tsv`, `.txt`, `.dat`).
fn is_text(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| ["csv", "tsv", "txt", "dat"].iter().any(|t| t.eq_ignore_ascii_case(e)))
}
