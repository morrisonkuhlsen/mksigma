# Architecture

## Data model (`core`)
A `Dataset` is a list of `Variable`s (the dictionary: name, label, type, measurement level,
value labels, user-missing values) and their values stored **by column**. Names are unique
ignoring case. `Value` is a number, a string or system-missing.

## Syntax (`syntax`)
Commands end with a period at the end of a line or of the input. Keywords are case-insensitive
and may be shortened to three letters. `parse` returns typed `Command`s; adding a command means
a variant, a parser and tests.

## Procedures (`stats`)
A procedure takes a `&Dataset` and its parsed options and returns output items. It never
touches the UI.

### Numerical parity
Each procedure documents which public algorithm it follows and is tested against
independently computed values, with explicit tolerances. Accumulations use numerically stable
methods (Welford for moments); least squares are solved by Householder QR, never by forming
`XᵀX`. `crates/stats/tests/nist.rs` audits the engine against NIST's
Statistical Reference Datasets (univariate, ANOVA, linear regression; data built in code,
certified values as constants): each dataset has a floor of correct digits (LRE) that no change
may lower.

## Output (`output`)
Tables, text and warnings as plain data. Frontends render them: text (CLI, tests), egui (app),
later HTML/PDF export.

## Engine (`engine`)
`Session` owns the active dataset and the output, and is the single entry point for syntax,
menus, the CLI and a future MCP server.

## Cross-filtering (`engine`)
Output items can carry **categories**: a bar, a histogram bin, a box of a grouped boxplot, the
valid rows of a frequency table. A category names a variable and the values in it (one value or
a range); procedures fill them in, since only they know which value a label stands for.

The session remembers each analysis command with the items it produced on all the cases (a
*run*). Selecting a category filters the dataset to its cases and runs every other analysis
again on that subset; the run the selection came from keeps its full view and highlights it.
Clearing the filter shows the original items again. The filter is a view, not a data change: it
is not an applied step, not undoable and not journaled. While it is on, data changes re-run the
filtered analyses too.

## Applied steps and undo (`engine`)
The session keeps the dataset as opened (`base`) and the **applied steps**: every change to the
data since, in order — data commands as written, and Data View edits (cell, insert/delete case).
The current dataset is always `base` with the enabled steps applied.

- Normal work is incremental: a command changes the dataset directly and records the smallest
  edits that reverse it (metadata before/after, a column, removed cases) for undo.
- Changing the steps (switch off, rewrite, move, remove) replays them from `base`. A step that
  fails on replay is skipped and keeps its error; replay doesn't run analyses.
- Undo covers both: incremental entries reverse their edits and drop their step; step-list
  entries restore the previous list and replay.
- Invariant, tested: replaying the steps gives exactly the incrementally built dataset.
- Cost: `base` is a second copy of the data as opened. Fine for survey-sized files; large data
  will want a columnar, copy-on-write store (see the Arrow item in the roadmap).
