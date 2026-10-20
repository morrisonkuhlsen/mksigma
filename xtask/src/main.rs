//! MK Sigma's project tasks, run with `cargo xtask <task>`:
//!
//! - `ci`: every quality gate before a commit: fmt, clippy (warnings are errors), tests,
//!   assets, layers, wasm. Stops at the first that fails.
//! - `layers`: each crate depends only on crates at its layer or below, and nothing below the
//!   UI layer on egui, eframe, winit or rfd.
//! - `assets`: every asset file has its row in `ATTRIBUTION.md`.
//! - `panics`: every crate's root denies unwrap, expect, panic, todo, unimplemented and
//!   unreachable outside tests.
//! - `wasm`: the engine still builds for WebAssembly (the interactive dashboards run it).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Each internal crate's layer: L0 `core`/`output`/`i18n` → L1 `syntax`/`sav`/`csv`/`stats`/
/// `learn` → L2 `engine` → L3 `ui-egui`/`web` → L4 `apps/*`.
const LAYERS: [(&str, u8); 13] = [
    ("mksigma-i18n", 0),
    ("mksigma-core", 0),
    ("mksigma-output", 0),
    ("mksigma-syntax", 1),
    ("mksigma-sav", 1),
    ("mksigma-csv", 1),
    ("mksigma-stats", 1),
    ("mksigma-learn", 1),
    ("mksigma-engine", 2),
    ("mksigma-ui-egui", 3),
    ("mksigma-web", 3),
    ("mksigma", 4),
    ("mksigma-cli", 4),
];

/// Crates only the UI layer and above may use.
const UI_ONLY: [&str; 4] = ["egui", "eframe", "winit", "rfd"];

/// Files under these folders are assets: each needs a row in ATTRIBUTION.md.
const ASSET_DIRS: [&str; 1] = ["crates/ui-egui/assets"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_default()
}

fn layer(name: &str) -> Option<u8> {
    match name {
        "mksigma-mcp" => Some(4),
        _ => LAYERS.iter().find(|(n, _)| *n == name).map(|(_, l)| *l),
    }
}

/// A manifest's package name and the names in its `[dependencies]` (not dev or build ones).
fn manifest(path: &Path) -> Result<(String, Vec<String>), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut section = String::new();
    let (mut name, mut deps) = (None, Vec::new());
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line.to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim();
        if section == "[package]" && key == "name" {
            name = Some(value.trim().trim_matches('"').to_string());
        } else if section == "[dependencies]" && !key.is_empty() && !key.starts_with('#') {
            deps.push(key.to_string());
        }
    }
    Ok((name.ok_or_else(|| format!("{}: no package name", path.display()))?, deps))
}

fn layers() -> Result<(), String> {
    let root = root();
    let mut problems = Vec::new();
    for group in ["crates", "apps"] {
        let entries = std::fs::read_dir(root.join(group)).map_err(|e| format!("{group}: {e}"))?;
        for entry in entries.flatten() {
            let path = entry.path().join("Cargo.toml");
            if !path.exists() {
                continue;
            }
            let (name, deps) = manifest(&path)?;
            let Some(own) = layer(&name) else {
                problems.push(format!("{name} has no layer: add it to xtask's LAYERS and to AGENTS.md"));
                continue;
            };
            for dep in deps {
                if let Some(theirs) = layer(&dep).filter(|theirs| *theirs > own) {
                    problems.push(format!("{name} (L{own}) depends on {dep} (L{theirs})"));
                }
                if own < 3 && UI_ONLY.contains(&dep.as_str()) {
                    problems.push(format!("{name} (L{own}) depends on {dep}, which only the UI layer may use"));
                }
            }
        }
    }
    if problems.is_empty() { Ok(()) } else { Err(problems.join("\n")) }
}

fn assets() -> Result<(), String> {
    let root = root();
    let attribution = std::fs::read_to_string(root.join("ATTRIBUTION.md")).map_err(|e| format!("ATTRIBUTION.md: {e}"))?;
    let mut missing = Vec::new();
    for dir in ASSET_DIRS {
        let entries = std::fs::read_dir(root.join(dir)).map_err(|e| format!("{dir}: {e}"))?;
        for entry in entries.flatten() {
            let file = format!("{dir}/{}", entry.file_name().to_string_lossy());
            // A row names the file in backticks.
            if !attribution.contains(&format!("`{file}`")) {
                missing.push(file);
            }
        }
    }
    if missing.is_empty() { Ok(()) } else { Err(format!("assets without a row in ATTRIBUTION.md: {}", missing.join(", "))) }
}

/// The attribute every crate's root carries, so clippy refuses a panic outside tests.
const NO_PANICS: &str = "#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]";

fn panics() -> Result<(), String> {
    let root = root();
    let mut missing = Vec::new();
    for (group, file) in [("crates", "src/lib.rs"), ("apps", "src/main.rs")] {
        let entries = std::fs::read_dir(root.join(group)).map_err(|e| format!("{group}: {e}"))?;
        for entry in entries.flatten() {
            let path = entry.path().join(file);
            let Ok(source) = std::fs::read_to_string(&path) else { continue };
            if !source.contains(NO_PANICS) {
                missing.push(path.strip_prefix(&root).unwrap_or(&path).display().to_string());
            }
        }
    }
    if missing.is_empty() { Ok(()) } else { Err(format!("without the no-panic attribute: {}", missing.join(", "))) }
}

/// One gate: it passes, or says why not.
type Task = fn() -> Result<(), String>;

/// Runs cargo with these arguments in the workspace.
fn cargo(args: &[&str]) -> Result<(), String> {
    let program = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(program).args(args).current_dir(root()).status().map_err(|e| format!("cargo {}: {e}", args.join(" ")))?;
    if status.success() { Ok(()) } else { Err(format!("cargo {} failed", args.join(" "))) }
}

fn wasm() -> Result<(), String> {
    cargo(&["build", "--quiet", "--package", "mksigma-web", "--target", "wasm32-unknown-unknown", "--profile", "wasm"])
}

fn ci() -> Result<(), String> {
    let steps: [(&str, Task); 7] = [
        ("fmt", || cargo(&["fmt", "--all", "--check"])),
        ("clippy", || cargo(&["clippy", "--quiet", "--workspace", "--all-targets", "--", "-D", "warnings"])),
        ("tests", || cargo(&["test", "--quiet", "--workspace"])),
        ("assets", assets),
        ("layers", layers),
        ("panics", panics),
        ("wasm", wasm),
    ];
    for (name, step) in steps {
        eprintln!("xtask ci: {name}");
        step().map_err(|e| format!("{name}: {e}"))?;
    }
    eprintln!("xtask ci: every gate passed");
    Ok(())
}

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "ci" => ci(),
        "layers" => layers(),
        "assets" => assets(),
        "panics" => panics(),
        "wasm" => wasm(),
        _ => Err("usage: cargo xtask <ci|layers|assets|panics|wasm>".to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
