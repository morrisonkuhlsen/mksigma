//! For native builds, compiles this crate to WebAssembly (profile `wasm`, its own target
//! directory) and leaves `mksigma_web.wasm` in `OUT_DIR` for `ENGINE` to embed. Without the
//! `wasm32-unknown-unknown` target the file is empty and pages can't be interactive; the build
//! itself still works, unless `MKSIGMA_REQUIRE_WASM` is set (CI sets it), when it fails instead
//! of shipping an empty engine (#47).

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());
    let wasm = out.join("mksigma_web.wasm");
    let target = std::env::var("TARGET").unwrap_or_default();
    // Everything the engine is built from, so the embedded engine is never stale: every crate it
    // links, their manifests, and the lockfile (#47).
    for krate in [".", "../engine", "../stats", "../output", "../syntax", "../sav", "../csv", "../dta", "../xpt", "../core", "../i18n"] {
        println!("cargo:rerun-if-changed={krate}/src");
        println!("cargo:rerun-if-changed={krate}/Cargo.toml");
    }
    for path in ["../i18n/locales", "../../Cargo.toml", "../../Cargo.lock", "build.rs"] {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rerun-if-env-changed=MKSIGMA_REQUIRE_WASM");
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
        // Under `cargo clippy` the outer build runs rustc through clippy-driver with its lints
        // (`-D warnings`): this build makes the engine, it doesn't lint it for wasm32.
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("CLIPPY_ARGS")
        .env_remove("CLIPPY_CONF_DIR")
        .status();
    let built = target_dir.join("wasm32-unknown-unknown").join("wasm").join("mksigma_web.wasm");
    match status {
        Ok(s) if s.success() && std::fs::copy(&built, &wasm).is_ok() => {}
        _ if std::env::var_os("MKSIGMA_REQUIRE_WASM").is_some() => {
            println!(
                "cargo::error=the dashboard engine couldn't be built for WebAssembly (is wasm32-unknown-unknown installed? `-vv` shows that build's errors), and MKSIGMA_REQUIRE_WASM asks for it"
            );
        }
        _ => {
            println!(
                "cargo:warning=the dashboard engine wasn't built for WebAssembly (rustup target add wasm32-unknown-unknown); interactive dashboard export is off"
            );
            let _ = std::fs::write(&wasm, b"");
        }
    }
}
