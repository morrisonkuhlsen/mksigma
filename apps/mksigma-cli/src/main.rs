//! MK Sigma command-line interface.
//!
//! `mksigma-cli run [--in data.sav] '<syntax>'` runs syntax against a system file (or, without
//! `--in`, the built-in sample dataset) and prints the output as text.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;

use mksigma_engine::Session;

const USAGE: &str = "usage: mksigma-cli run [--in data.sav] '<syntax>'";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (input, syntax) = match args.as_slice() {
        [cmd, syntax] if cmd == "run" => (None, syntax),
        [cmd, flag, path, syntax] if cmd == "run" && flag == "--in" => (Some(path), syntax),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let session = match input {
        None => Session::with_sample().map_err(|e| e.to_string()),
        Some(path) => std::fs::read(path).map_err(|e| e.to_string()).and_then(|bytes| {
            let mut s = Session::new();
            let warnings = s.open_sav(&bytes).map_err(|e| e.to_string())?;
            for w in warnings {
                eprintln!("warning: {path}: {w}");
            }
            Ok(s)
        }),
    };
    let mut session = match session {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let result = session.run_syntax(syntax);
    print!("{}", session.output.to_text());
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
