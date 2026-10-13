# Roadmap

MK Sigma covers one workflow end to end: **get data in → prepare it → analyse it → publish an
interactive dashboard of the results**, every step recorded as syntax, so it is reproducible and
drivable by agents. The core is a solid SPSS-style statistics package; the Power BI-style
features (recorded data preparation, cross-filtering, dashboards) are a layer on top of it.

## M0 — skeleton (done)
Workspace, core data model, output model, syntax for `DESCRIPTIVES`, `DESCRIPTIVES` itself,
desktop app (Data View, syntax box, Output Viewer) and CLI on a sample dataset.

## M1 — data in, data preparation
- **Columnar storage on Apache Arrow** (`arrow` or `polars`) behind the `core` API, so large
  datasets, connectors and dashboards share one in-memory format. Decide before the `.sav`
  reader lands.
- `.sav` reader — **started**: both byte orders, uncompressed and bytecode-compressed data,
  variable and value labels, user-missing values, print formats, long names, measurement
  levels, documents, weight, character encoding; `DISPLAY DICTIONARY`. To do: `.zsav` (zlib)
  data, very long strings (> 255 bytes) as one variable, long-string value labels and missing
  values (record 7.22), variable attributes, multiple-response sets.
- `.sav` writer — **done**: little-endian UTF-8, bytecode-compressed or uncompressed; unique
  short names plus long names (7.13), labels, user-missing values (LO/HI ranges), value labels
  including strings over 8 bytes (7.21, read too), measurement levels (7.11), documents,
  weight, file label. Checked by round trip and by reading our files with ReadStat
  (pyreadstat). To do: `.zsav`, strings over 255 bytes, long-string missing values.
- CSV and `.xlsx` import (`GET DATA`)
- **Recorded data preparation** (Power Query style): every transformation is a syntax command
  shown as a list of applied steps that can be edited, reordered and replayed.
  **Done:** `COMPUTE`, `IF`, `RECODE` (values, `LO THRU HI` ranges, `MISSING`, `SYSMIS`, `ELSE`,
  `COPY`, `INTO`), `SELECT IF`, all undoable; expressions with SPSS precedence and missing-value
  rules and functions (`ABS SQRT EXP LN LG10 RND TRUNC MOD SUM MEAN SD VARIANCE MIN MAX` with
  `.n`, `NMISS NVALID MISSING SYSMIS VALUE RANGE ANY`, `CONCAT UPCASE LOWER LTRIM RTRIM LENGTH
  SUBSTR INDEX NUMBER STRING`). Checked against pandas. To do: `SORT CASES`, `AGGREGATE`,
  `MERGE FILES`, `DO IF`, `COUNT`, date functions.
- **Applied steps panel** — **done**: every change to the data since the file was opened (data
  commands, cell edits, inserted and deleted cases) is a step; switch one off, rewrite its
  command, move or remove it, and the data is replayed from the file. Steps that fail on replay
  are skipped and shown in red with the reason. Step changes are undoable. Next: steps saved
  with the file (a `.sps` beside the `.sav`), inserting a step in the middle.
- Desktop app: File ▸ Open (native dialog, Ctrl+O), drag and drop, opening a file given on the
  command line, Data View (virtualized rows, value-labels toggle, missing values dimmed) and a
  Variable View — **done**.
- Editing — **done**: Variable View (name, label, value labels dialog, missing values,
  measurement level, format) turns every edit into syntax (`RENAME VARIABLES`, `VARIABLE LABELS`,
  `VALUE LABELS`, `MISSING VALUES`, `VARIABLE LEVEL`, `FORMATS`), which runs through the engine
  and lands in the session **History** (pastable into the syntax editor): the first recorded
  steps. Data View edits cells in place. Commands are all-or-nothing.
- Saving — **done**: File ▸ Save (Ctrl+S) and Save As… (Ctrl+Shift+S), written to a temporary
  file and renamed over the target; `•` in the title while there are unsaved changes;
  `mksigma-cli run --out`.
- Undo and redo — **done**: every change to the data (syntax commands, cell edits, cases) is
  one step, stored as the smallest reversible record (metadata before/after, old value, the
  removed column); undoing a command also takes it out of the History. Edit menu names the
  step; Ctrl+Z, Ctrl+Shift+Z / Ctrl+Y when no text box has the keyboard.
- Adding and deleting — **done**: `NUMERIC`, `STRING`, `DELETE VARIABLES`; blank last rows in
  Data View (adds a case) and Variable View (adds a numeric variable); right-click a case
  number to insert or delete a case, a variable number to delete it.

## M2 — the introductory-course core
Done: case weights (`WEIGHT BY` from the file), `DESCRIPTIVES` weighted, `FREQUENCIES` (frequency
tables with labels and missing sections, `/FORMAT` order and `NOTABLE`, `/STATISTICS` mean,
SE mean, mode, std. deviation, variance, range, minimum, maximum, sum). To do in `FREQUENCIES`:
median, percentiles, skewness, kurtosis, charts, `TO` and `ALL` in variable lists.

Next: `CROSSTABS` (χ², Fisher), `T-TEST` (one-sample, independent, paired),
`ONEWAY`, `CORRELATIONS`, `REGRESSION` (linear), `LOGISTIC REGRESSION`.

## M3 — Output Viewer
- Pivot tables (transpose, move layers), charts (histogram, bar, scatter, boxplot)
- **Cross-filtering:** items from one dataset share a filter; selecting a bar or a table
  category filters the other items
- Export to HTML and PDF

## M4 — dashboards
- A dashboard is a layout of output items with shared filters (slicers), saved with the
  syntax that produced it
- **Publish as a self-contained interactive page** (HTML + WASM): no server, no account
- MCP server, so agents can run analyses and build dashboards

## Later
- Multiple related tables (star-schema data model) and measures
- Connectors: PostgreSQL, SQLite, Parquet, HTTP/JSON
- `GLM`, `UNIANOVA`, `FACTOR`, `RELIABILITY`, `NPAR TESTS`, `MIXED`
- Web build of the whole app

## Out of scope for now
- A DAX-like formula language (syntax plus expressions covers the common cases)
- Hosted service: workspaces, permissions, scheduled refresh
- Free-form drag-and-drop report designer (dashboards start from analyses)
