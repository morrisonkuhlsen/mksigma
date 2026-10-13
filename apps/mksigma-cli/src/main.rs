//! MK Sigma command-line interface.
//!
//! `mksigma-cli run [--in data.sav] [--out result.sav] [--html output.html] '<syntax>'` runs
//! syntax against a system file (or, without `--in`, the built-in sample dataset), prints the
//! output as text and, with `--out`, saves the resulting dataset; with `--html`, writes the
//! Output (tables and charts) as a web page.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;

use mksigma_engine::Session;

const USAGE: &str = "usage: mksigma-cli run [--in data.sav] [--out result.sav] [--html output.html] '<syntax>'";

struct Args {
    input: Option<String>,
    output: Option<String>,
    html: Option<String>,
    syntax: String,
}

fn parse_args() -> Option<Args> {
    let mut args = std::env::args().skip(1);
    if args.next()? != "run" {
        return None;
    }
    let (mut input, mut output, mut html, mut syntax) = (None, None, None, None);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--in" => input = Some(args.next()?),
            "--out" => output = Some(args.next()?),
            "--html" => html = Some(args.next()?),
            _ if syntax.is_none() => syntax = Some(arg),
            _ => return None,
        }
    }
    Some(Args { input, output, html, syntax: syntax? })
}

/// Seconds since 1970 for the file header's creation time.
#[allow(clippy::disallowed_methods)] // native only: the CLI never runs on wasm
fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

fn run(args: &Args) -> Result<(), String> {
    let mut session = match &args.input {
        None => Session::with_sample().map_err(|e| e.to_string())?,
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
    if let Some(path) = &args.html {
        let name =
            args.input.as_deref().map(|i| std::path::Path::new(i).file_name().map_or_else(|| i.to_string(), |n| n.to_string_lossy().into_owned()));
        let title = name.map_or_else(|| "MK Sigma Output".to_string(), |n| format!("{n} — Output"));
        std::fs::write(path, session.output.to_html(&title)).map_err(|e| format!("{path}: {e}"))?;
    }
    result.map_err(|e| e.to_string())?;
    if let Some(path) = &args.output {
        let (bytes, warnings) = session.save_sav(mksigma_engine::timestamp(now())).map_err(|e| e.to_string())?;
        for w in warnings {
            eprintln!("warning: {path}: {w}");
        }
        std::fs::write(path, bytes).map_err(|e| format!("{path}: {e}"))?;
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
