# MK Sigma — instructions for agents

MK Sigma is a clean-room, open-source, Rust-native statistics package targeting the IBM SPSS
Statistics workflow: Data View / Variable View, command syntax, procedures and an Output Viewer.
On top of that core it adds the best of Power BI: recorded data preparation, cross-filtering and
publishable dashboards. Scope and order: `docs/roadmap.md`.

## Non-negotiables
- **Clean-room.** SPSS may be *observed* black-box (run it on synthetic data, compare output).
  The same applies to Power BI. Never read, disassemble or copy anything inside an SPSS or
  Power BI installation, never commit files produced by either, never copy GPL/AGPL/LGPL code
  (PSPP, R and its packages…). Behaviour comes from public documentation (Command Syntax
  Reference, Algorithms guide, the public `.sav` format description) and observation.
- **Numerical parity is a feature.** Every procedure lands with tests against reference values
  computed independently (by hand, or with a documented external tool whose output is
  re-typed as constants, never vendored code). State tolerances explicitly.
- **Never crash.** No `unwrap`/`expect`/`panic!`/`todo!`/`unreachable!` outside tests, no
  `unsafe` (`unsafe_code = "forbid"`). Errors are `Result` through each crate's error type.
  Input-derived sizes (files, syntax) are hostile: `get()` over indexing, checked arithmetic,
  capped allocations.
- **Everything goes through the engine.** Menus, syntax, CLI and (later) MCP all call
  `Session`; the UI never computes statistics itself.
- **Layering:** `core`/`output` (L0) → `syntax`/`sav`/`csv`/`stats` (L1) → `engine` (L2) →
  `ui-egui` (L3) → `apps/*` (L4). Nothing below L3 depends on egui/eframe.
- **No vendor assets.** No IBM or Microsoft iconography, artwork, sample files or templates.
  Feature and command names (`DESCRIPTIVES`, `FREQUENCIES`, "Variable View") are fine.

## Looking at the UI
Render it offscreen: `cargo run -p mksigma-ui-egui --example snapshot -- [--in data.sav]
[--variables] [--labels] [--run 'SYNTAX'] out.png`. Never screenshot the desktop: it captures
whatever else the person has open. UI behaviour is tested headless with `egui_kittest`
(`crates/ui-egui/tests/`); platform services such as the file dialog come in through `Services`.

## Quality gates
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` before every
commit.
