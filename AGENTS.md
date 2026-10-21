# MK Sigma — instructions for agents

MK Sigma is a clean-room, open-source, Rust-native statistics package targeting IBM SPSS Statistics parity — Data View / Variable View, command syntax, procedures and an Output Viewer — and superiority: every figure with its calculation by hand (Σ), a study section (Learn), and on top the best of Power BI: recorded data preparation, cross-filtering and publishable dashboards. Beyond SPSS, it brings together what the other reference packages do — SAS, Stata, JMP, Minitab, JASP and jamovi — in one engine, one command syntax and one Output (see *Beyond SPSS*). It runs natively on Linux, Windows and macOS; its engine also runs in the browser as WebAssembly, inside exported dashboards. A Morrison Kühlsen project.

## Start every session here
1. Read [`docs/roadmap.md`](docs/roadmap.md): what is done, and the **Next:** items, which are the work list; then the relevant section of [`docs/architecture.md`](docs/architecture.md). Behaviour reference: IBM's public documentation (Command Syntax Reference, Algorithms guide), the public `.sav` format description, the other vendors' public manuals (Stata's PDF manuals, SAS's, Minitab's and JMP's documentation), textbooks and papers, and open tools run as oracles (see *Clean-room*).
2. Work autonomously through the next item: build it, test it, look at it, commit it when the gates pass. Stop to ask only for what is the user's to decide: priorities and scope, brand and visual identity, anything published or shared (pushing, releases), and anything destructive.
3. **Issues.** The open GitHub issues (`gh issue list`) are work too: problems and pending items. A problem found and not fixed in the same change becomes an issue at once (`gh issue create`), in English: a short title, the sections *Report* (what happens), *Expected / actual* and *Notes* (where in the code, what a fix needs), labelled by type (`bug`, `enhancement`) and priority (`priority: critical`, `high`, `medium`, `low`; `needs info` when it can't be confirmed). Search the open issues first, so none is filed twice. A commit that fixes one says `Fixes #n`; closing it waits for the push.

`plan/` is gitignored (local only): working notes, including notes on the reference packages' public documentation.

## Assets and iconography — absolute rule
**No IBM, Microsoft, Adobe, SAS, StataCorp, Minitab, JASP, jamovi (or any other vendor's) iconography, images, artwork, fonts, templates, themes or sample files — ever.** This is the most important rule in this repository; breaking it is a serious failure.
- Every asset in the repo is original work by MK Sigma's contributors, or public domain / CC0, or licensed for redistribution, or one of Morrison Kühlsen's brand assets below.
- Every asset file has a row in [`ATTRIBUTION.md`](ATTRIBUTION.md) (author, source, terms). `cargo xtask assets` enforces it. Adding an asset without a row is a bug.
- Prefer art generated in code: the UI icon set (`crates/ui-egui/src/icons.rs`), the charts, the themes (`crates/ui-egui/src/theme.rs`), the Output styles and the sample dataset are drawn or defined in code and original.
- Never copy any reference package's icons, toolbar artwork, Output templates, table looks, sample files (the `.sav` that ship with SPSS, Stata's example `.dta`, Minitab's and JMP's sample data) or wording beyond feature names. Feature and command *names* (`DESCRIPTIVES`, `FREQUENCIES`, "Variable View", "slicer", "Xbar-R chart", "process capability") are fine.
- Screenshots of IBM, Microsoft or any other vendor's or project's software are never committed.
- The Morrison Kühlsen brand files in `crates/ui-egui/assets/` (the σ mark, the wordmark, the spiral staircase) are the house's, used with its permission, listed in `ATTRIBUTION.md`: the one exception.

## Never crash
People trust MK Sigma with their data and their results; a crash loses their work. **This outranks feature work.**
- **No panics in non-test code:** no `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`; no `unsafe` (`unsafe_code = "forbid"`).
- **Errors are `Result<T, E>`** through each crate's error type and `?`; their messages come from the catalogs (see Languages).
- **Input-derived numbers are hostile** (`.sav` and text files, syntax, MCP params, dashboard pages): `get()` instead of indexing, checked/saturating arithmetic, cap input-sized allocations (a file's declared counts, a `TO` range, a slicer's values).
- **Statistics' own hostile inputs** become a `Result` with an explainable, catalogued message, never a panic or a silent NaN: a singular or near-singular matrix, n = 0 or n = 1, zero variance, missing values and NaN, perfect separation in logistic regression, an optimizer that does not converge, a factor with a million levels. Iterative methods carry an iteration cap.
- **Bound recursion** (nested expressions in syntax, `RECODE` and `COMPUTE` chains).
- **Prove it:** every crash fix lands with a regression test that feeds the input that crashed.
- Crates carry `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]`; `cargo xtask panics` enforces it.

## Non-negotiables
- **Clean-room.** Proprietary packages are not run to shape MK Sigma. SAS, JMP, Minitab and Power BI forbid using them to build a competing product or for competitive research (SAS Universal Terms §1.3.2, JMP Software License Agreement §1.4.3, Minitab Software Subscription Agreement §2.5(c), Power BI Desktop licence terms §6); their academic and trial licences are noncommercial or evaluation-only; IBM SPSS Statistics and Stata forbid reverse engineering, which US courts have read to cover black-box study done to recreate a product (*SAS Institute v. World Programming*, 4th Cir. 2017). Brazil's software law (Lei 9.609/98) has no right to observe a program that would override those terms. Any exception (for example observing IBM SPSS Statistics under a commercial licence) is the user's decision, recorded with the licence used and legal sign-off. Behaviour comes from public documentation — its ideas only, never its text, tables, examples or syntax charts, which are copyrighted — textbooks, papers, published worked examples, NIST StRD, and open tools run as oracles (R, PSPP, gretl, JASP, jamovi, scipy, statsmodels: running GPL/AGPL programs is allowed). Never read, disassemble or copy anything inside a vendor's installation, never commit files any vendor's package produced (`.spv`, Stata logs, Minitab projects, JMP tables, their sample datasets), never copy GPL/AGPL/LGPL code (PSPP, gretl, R, jamovi, JASP and their analysis modules) nor code from CRAN packages, whatever their licence: reading their *documentation* is allowed, their code is not. "Clean-room" here means independent implementation from public sources, never from anyone's code; a commit names the sources it was built from.
- **File formats** come from public specifications: `.sav` from PSPP's *System File Format* documentation (IBM publishes no specification; the description is under the GNU FDL 1.3+, so implement from it and copy none of its text), `.dta` from StataCorp's published specification (`help dta` on stata.com, not an installed Stata), SAS transport `.xpt` from SAS's TS-140 record layout (its C conversion routines are SAS's and are not copied). `.sas7bdat` is known only through third parties' reverse engineering: out of scope until the user decides otherwise.
- **Dependencies** are permissively licensed (MIT, Apache-2.0, BSD, Zlib or similar), and below L3 they build for wasm; anything missing — mixed models, structural equations, complex samples, DOE, SPC — is written here, from the literature.
- **Numerical parity is a feature.** Every procedure lands with tests against reference values computed independently (by hand, or with a documented external tool whose output is re-typed as constants, never vendored code), with explicit tolerances. Where NIST's Statistical Reference Datasets (StRD, public domain, certified values) cover a procedure — univariate statistics, ANOVA, linear and nonlinear regression — its tests hold it against them, the datasets built in code.
- **Every figure has its calculation.** A procedure lands with its Σ (`crates/stats/src/explain.rs`): the formula, the data's numbers in it, every sum, down to the table's figure; its tests hold every number of the table against the calculation, and its exported `.tex` compiles with pdflatex.
- **Fonts are never committed.** The app uses egui's defaults; text the fonts lack (→, bold) is drawn in code or done without.
- **Test data is built in code.** Real SPSS-authored files never enter the repository (`/corpus` is gitignored); tests build their datasets and files in code.
- **Everything goes through the engine.** User-visible behaviour is command syntax (`crates/syntax`) run by `Session` (`crates/engine`), or a `Session` method, plus tests. Menus, dialogs, Learn's examples, the CLI and the MCP server all reach the same `Session`; the UI never computes statistics itself.
- **Programmatic calls never open dialogs.** Syntax, the CLI and MCP run with what they are given or return an error; only the ribbon, the search and Learn's links open a dialog, and a dialog only writes syntax (OK runs it, Paste pastes it).
- **Layering** is enforced by `cargo xtask layers`: `core`/`output`/`i18n` (L0) → `syntax`/`sav`/`csv`/`stats`/`learn` (L1) → `engine` (L2) → `ui-egui`/`web` (L3) → `apps/*` (L4). A crate depends only on its layer and below. Nothing below L3 depends on egui/eframe/winit/rfd. **The UI crate is swappable.**
- **The UI is thin**: views read `Session` state and act through syntax or `Session` methods. Colours come from `theme::Tokens`. Platform services (file dialogs, settings) come in through `Services`.
- **Languages.** Every user-visible text — interface, Output, Σ, Learn, errors — comes from the catalogs in `crates/i18n/locales/*.ftl` through `tr!`, in every language (English and Português (Brasil)); their tests fail on a missing message or placeholder. Command syntax is never translated. Numbers shown take the language's decimal mark; syntax and files keep the point.
- **Learn.** An article per procedure and concept, in every language (`crates/learn/articles/`), written for MK Sigma; its examples run on the sample data, its links resolve, both languages give the same examples (tests enforce all three).
- **Rust only** (no handwritten JS/TS). **Never break wasm** (`cargo xtask wasm`): the exported dashboards embed the engine.
- **The one `unsafe` attribute.** `crates/web` exports functions to WebAssembly with `#[unsafe(no_mangle)]`, which `forbid` can't allow: that crate alone has `unsafe_code = "deny"` and `#[allow(unsafe_code)]` on those attributes. It has no `unsafe` block; keep it so.
- **Quality gates** before every commit: `cargo xtask ci` (fmt, clippy -D warnings, tests, assets, layers, panics, wasm). A commit lands only when they all pass. One change per commit, its message saying what and why. Push only when the user asks.

## Beyond SPSS: what the others do
SPSS parity comes first; the other packages' strengths join as modules of the same engine, each a set of commands in the one syntax, with its Σ, its Learn articles and its parity tests — never a separate program or a second syntax:
- **Stata** — econometrics: panel data (fixed and random effects), instrumental variables, survey data with complex samples, epidemiology.
- **SAS** — large data, validated and auditable analyses.
- **JMP** — visual, interactive exploration and design of experiments (DOE, response surfaces).
- **Minitab** — quality: control charts (SPC), process capability, Six Sigma.
- **JASP** — Bayesian analyses side by side with the frequentist ones.
- **jamovi** — results that update as the analysis is edited; teaching.

Scope is the main risk: finish one vertical slice before opening the next module, and the order of modules is the user's to decide (`docs/roadmap.md`).

## Running and looking at the app
- `cargo run --release -p mksigma -- [data.sav]` (without a file, the sample dataset). On the maintainer's machine, `mksigma` (or MK Sigma in the applications menu) rebuilds the release build from this checkout and opens it.
- Headless: `mksigma-cli run [--in data.sav] [--out result.sav] [--html report.html] [--pdf report.pdf] [--tex report.tex] [--dashboard board.html] '<syntax>'`; without `--in`, the sample dataset.
- Agents: `mksigma-mcp` is a Model Context Protocol server over stdio (open data, describe it, run syntax, read and export the Output): `claude mcp add mksigma -- mksigma-mcp`.
- **Offscreen UI render** (no window): `cargo run -p mksigma-ui-egui --example snapshot -- [--in data.sav] [--variables] [--labels] [--filters] [--dark] [--run 'SYNTAX'] [--tab NAME] [--backstage] [--learn ID|list] [--dialog KIND] [--explain ITEM [--every-term]] [--splash] [--lang pt-BR] [--size 1280x800] out.png`. Never screenshot the desktop: it captures whatever else the person has open.
- UI behaviour is tested headless with `egui_kittest` (`crates/ui-egui/tests/`); a control below the window's bottom can't be clicked there, so size the harness for it.
- **For UI work, look at the result** (the snapshot PNG), in both languages; for the Σ, compile its `.tex` with pdflatex and look at a page.
- Shell gotchas: `cargo fmt` reformats code, so re-read a file before an exact-text edit of what it touched; a gate run chained with `;` doesn't stop on a failure — use `cargo xtask ci && git commit …`.
- Parallel agents: a separate `CARGO_TARGET_DIR` per agent; edit only the crates you own; delete your target dir when done.

## Roadmap
[`docs/roadmap.md`](docs/roadmap.md) (committed) is the plan and the progress log: the milestones (M0 skeleton, M1 data in, M2 the introductory-course core, M3 Output Viewer, M4 dashboards), then the sections that followed them (the calculation by hand, languages, Learn), each with what is done and its **Next:** items, then Later and Out of scope. [`docs/architecture.md`](docs/architecture.md) describes the crates and how they fit. Update the affected sections whenever work lands: what was done, and what comes next.

## Credits
- MK Sigma is a Morrison Kühlsen project; its name, motto and brand appear in the app (splash screen, File backstage).
- Commits made by an agent end with the attribution lines its session gives.
- Never add, edit, guess or copy anyone's name or credit (not from git config, commit authors or GitHub profiles).
