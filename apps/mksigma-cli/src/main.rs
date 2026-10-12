//! MK Sigma command-line interface.
//!
//! `mksigma-cli run 'DESCRIPTIVES VARIABLES=age income.'` runs syntax against the sample dataset
//! (file input arrives with the `.sav` reader) and prints the output as text.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;

use mksigma_engine::Session;

const USAGE: &str = "usage: mksigma-cli run '<syntax>'";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [cmd, syntax] = args.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    if cmd != "run" {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let mut session = match Session::with_sample() {
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
