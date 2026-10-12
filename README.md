# MK Sigma

**Statistics and dashboards; an open-source, clean-room take on the SPSS Statistics workflow,
with the best of Power BI on top, in pure Rust.**

One workflow, end to end and reproducible:

1. **Get data in:** `.sav`, CSV, Excel, later databases and Parquet.
2. **Prepare it:** every transformation is a recorded, replayable step (Power Query style).
3. **Analyse it:** Data View and Variable View, command syntax, statistical procedures with
   tested numerical parity, and an Output Viewer with pivot-style tables and charts.
4. **Publish it:** cross-filtered dashboards, exported as a self-contained interactive page.

Every step is syntax, so analyses can be rerun, reviewed and driven by agents (CLI, later MCP).
Runs natively on macOS, Windows, Linux and BSD, and later on the web via WebAssembly.

> Early days. Today it reads `.sav` files and runs `DESCRIPTIVES` and `DISPLAY DICTIONARY`;
> see [`docs/roadmap.md`](docs/roadmap.md).

## Quick start

```sh
cargo run -p mksigma                                              # desktop app
cargo run -p mksigma-cli -- run 'DESCRIPTIVES VARIABLES=age income score.'   # sample data
cargo run -p mksigma-cli -- run --in data.sav 'DISPLAY DICTIONARY.'
cargo test
```

## Layout

| Crate | Layer | What it holds |
|---|---|---|
| `crates/core` | L0 | Dataset, variables, values, missing values |
| `crates/output` | L0 | Output items: tables, text, warnings |
| `crates/syntax` | L1 | Command syntax lexer and parser |
| `crates/sav` | L1 | `.sav` / `.zsav` reader and writer |
| `crates/stats` | L1 | Statistical procedures |
| `crates/engine` | L2 | `Session`: runs commands, collects output |
| `crates/ui-egui` | L3 | egui frontend (swappable) |
| `apps/mksigma`, `apps/mksigma-cli` | L4 | Desktop app and CLI |

Details in [`docs/architecture.md`](docs/architecture.md); rules for contributors and agents in
[`AGENTS.md`](AGENTS.md).

## License

MIT OR Apache-2.0, at your option. SPSS is a trademark of IBM and Power BI a trademark of
Microsoft; MK Sigma is not affiliated with or endorsed by either.
