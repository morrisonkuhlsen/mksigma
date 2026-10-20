//! For native builds, compiles this crate to WebAssembly (profile `wasm`, its own target
//! directory) and leaves `mksigma_web.wasm` in `OUT_DIR` for `ENGINE` to embed. Without the
//! `wasm32-unknown-unknown` target the file is empty and pages can't be interactive; the build
//! itself still works.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());
    let wasm = out.join("mksigma_web.wasm");
    let target = std::env::var("TARGET").unwrap_or_default();
    // Everything the engine is built from, so the embedded engine is never stale.
    for dir in [
        "src",
        "../engine/src",
        "../stats/src",
        "../output/src",
        "../syntax/src",
        "../sav/src",
        "../csv/src",
        "../core/src",
        "../i18n/src",
        "../i18n/locales",
    ] {
        println!("cargo:rerun-if-changed={dir}");
    }
    println!("cargo:rerun-if-changed=build.rs");
    if target.starts_with("wasm32") {
        // The build this one starts: nothing to embed in itself.
        let _ = std::fs::write(&wasm, b"");
        return;
    }
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default()).join("Cargo.toml");
    let target_dir = out.join("wasm-target");
    let status = Command::new(cargo)
        .args(["build", "--offline", "--profile", "wasm", "--target", "wasm32-unknown-unknown", "--manifest-path"])
        .arg(&manifest)
        .arg("--target-dir")
        .arg(&target_dir)
        // The outer build's flags are for the host, not for WebAssembly.
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_BUILD_TARGET")
        .status();
    let built = target_dir.join("wasm32-unknown-unknown").join("wasm").join("mksigma_web.wasm");
    match status {
        Ok(s) if s.success() && std::fs::copy(&built, &wasm).is_ok() => {}
        _ => {
            println!(
                "cargo:warning=the dashboard engine wasn't built for WebAssembly (rustup target add wasm32-unknown-unknown); interactive dashboard export is off"
            );
            let _ = std::fs::write(&wasm, b"");
        }
    }
}
