//! MK Sigma command-line interface.
//!
//! `mksigma-cli run [--in data.sav|data.dta|data.xpt] [--out result.sav|result.dta] [--html report.html] [--pdf report.pdf]
//! '<syntax>'` runs
//! syntax against a system file or delimited text (`.csv`, `.tsv`, `.txt`, read with `GET DATA`;
//! without `--in`, the built-in sample dataset), prints the
//! output as text and, with `--out`, saves the resulting dataset; with `--html` / `--pdf`, writes the
//! Output (tables and charts) as a web page.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;

use mksigma_engine::Session;

const USAGE: &str = "usage: mksigma-cli run [--in data.sav] [--out result.sav] [--html report.html] [--pdf report.pdf] [--tex report.tex] [--dashboard board.html] '<syntax>'";

struct Args {
    input: Option<String>,
    output: Option<String>,
    html: Option<String>,
    pdf: Option<String>,
    tex: Option<String>,
    dashboard: Option<String>,
    syntax: String,
}

fn parse_args() -> Option<Args> {
    let mut args = std::env::args().skip(1);
    if args.next()? != "run" {
        return None;
    }
    let (mut input, mut output, mut html, mut pdf, mut tex, mut syntax) = (None, None, None, None, None, None);
    let mut dashboard = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--in" => input = Some(args.next()?),
            "--out" => output = Some(args.next()?),
            "--html" => html = Some(args.next()?),
            "--pdf" => pdf = Some(args.next()?),
            "--tex" => tex = Some(args.next()?),
            "--dashboard" => dashboard = Some(args.next()?),
            _ if syntax.is_none() => syntax = Some(arg),
            _ => return None,
        }
    }
    Some(Args { input, output, html, pdf, tex, dashboard, syntax: syntax? })
}

/// Seconds since 1970 for the file header's creation time.
#[allow(clippy::disallowed_methods)] // native only: the CLI never runs on wasm
fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

fn run(args: &Args) -> Result<(), String> {
    let mut session = match &args.input {
        None => Session::with_sample().map_err(|e| e.to_string())?,
        Some(path) if [".csv", ".tsv", ".txt", ".dat"].iter().any(|e| path.to_ascii_lowercase().ends_with(e)) => {
            let mut s = Session::new();
            let tab = if path.to_ascii_lowercase().ends_with(".tsv") { " /DELIMITERS=\"\\t\"" } else { "" };
            s.run_syntax(&format!("GET DATA /TYPE=TXT /FILE={}{tab}.", mksigma_engine::quote(path))).map_err(|e| e.to_string())?;
            for note in s.take_notes() {
                eprintln!("{note}");
            }
            s
        }
        Some(path) if [".dta", ".xpt"].iter().any(|e| path.to_ascii_lowercase().ends_with(e)) => {
            let mut s = Session::new();
            let get = if path.to_ascii_lowercase().ends_with(".dta") { "GET STATA FILE" } else { "GET SAS DATA" };
            s.run_syntax(&format!("{get}={}.", mksigma_engine::quote(path))).map_err(|e| e.to_string())?;
            for note in s.take_notes() {
                eprintln!("{note}");
            }
            s
        }
        Some(path) => {
            let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
            let mut s = Session::new();
            for w in s.open_sav(&bytes).map_err(|e| format!("{path}: {e}"))? {
                eprintln!("warning: {path}: {w}");
            }
            s
        }
    };
    let result = session.run_syntax(&args.syntax);
    print!("{}", session.output.to_text());
    let name = args.input.as_deref().map(|i| std::path::Path::new(i).file_name().map_or_else(|| i.to_string(), |n| n.to_string_lossy().into_owned()));
    let title = name.map_or_else(|| "MK Sigma Output".to_string(), |n| format!("{n} — Output"));
    // Every file asked for is written, each all or nothing; one that fails doesn't keep the
    // others from being written, and the data comes first.
    let mut failures = Vec::new();
    let mut write = |path: &str, bytes: Result<Vec<u8>, String>| {
        if let Err(e) = bytes.and_then(|b| mksigma_engine::write_atomic(std::path::Path::new(path), &b).map_err(|e| e.to_string())) {
            failures.push(format!("{path}: {e}"));
        }
    };
    if let (Some(path), Ok(())) = (&args.output, &result) {
        // A .dta path saves a Stata file; anything else, a system file.
        let stata = path.to_ascii_lowercase().ends_with(".dta");
        let saved = if stata { session.save_dta(now()) } else { session.save_sav(mksigma_engine::timestamp(now())) };
        write(
            path,
            saved.map_err(|e| e.to_string()).map(|(bytes, warnings)| {
                for w in warnings {
                    eprintln!("warning: {path}: {w}");
                }
                bytes
            }),
        );
    }
    if let Some(path) = &args.html {
        write(path, Ok(session.output.to_html(&title).into_bytes()));
    }
    if let Some(path) = &args.pdf {
        write(path, Ok(session.output.to_pdf(&title)));
    }
    if let Some(path) = &args.dashboard {
        // The tiles come from DASHBOARD /TILES in the syntax.
        let page = if mksigma_web::ENGINE.is_empty() {
            Err("this build has no WebAssembly engine (rustup target add wasm32-unknown-unknown, then build again)".to_string())
        } else {
            let board_title =
                if title.ends_with("— Output") { title.replace("— Output", "— Dashboard") } else { "MK Sigma Dashboard".to_string() };
            mksigma_web::page(&session, &board_title, mksigma_web::ENGINE).map(String::into_bytes)
        };
        write(path, page);
    }
    if let Some(path) = &args.tex {
        write(path, Ok(session.output.to_latex(&title).into_bytes()));
    }
    result.map_err(|e| e.to_string())?;
    if !failures.is_empty() {
        return Err(failures.join("\n"));
    }
    Ok(())
}

fn main() -> ExitCode {
    let Some(args) = parse_args() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
