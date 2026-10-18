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
- CSV import — **done**: `GET DATA /TYPE=TXT` (`/FILE`, `/DELIMITERS` with `\t`, `/QUALIFIER`,
  `/FIRSTCASE`, `/ENCODING`, `/VARIABLES=name format`), RFC 4180 quoting (doubled qualifiers,
  fields across lines), UTF-8 with a Windows-1252 fallback. MK Sigma extensions: without
  `/VARIABLES` the names come from the first record (`/READNAMES`) and types are inferred
  (numbers, decimal commas when the delimiter isn't a comma, strings as wide as their longest
  value; repeated whole-number codes are Nominal); without `/DELIMITERS` the delimiter is
  detected. Opening `.csv`/`.tsv`/`.txt` in the app or CLI runs `GET DATA`, so the History
  records it. To do: `.xlsx` (`/TYPE=XLSX`), fixed-width text, dates in text.
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
- Window layout — **done**: an office-suite layout (title row with quick-access Save, Undo and
  Redo; File menu and tabs Home, Data, Transform, Analyze, Graphs, View over a ribbon of large
  buttons with icons drawn in code; Data View as a grid with headers and gridlines; Data View and
  Variable View as tabs at the bottom; a status bar with the data's size, weight and filter),
  light and dark, indigo accented. Analyze and Graphs buttons paste the procedure's syntax for
  suggested variables into the editor and run it; when no variable fits they paste a template.
- File backstage — **done**: File opens a page over the window: a column in Morrison
  Kühlsen's ink with its logo, MK Sigma's name and motto, and the places grouped (Start: Home,
  New, Open; This data: Info, Save, Save As, Export, Close), and the page beside it with "Back
  to the data" in its corner. The app's accent is Morrison Kühlsen's blue (#2540D8). Home and New start a blank dataset or the sample; Home
  and Open list recent files (kept between runs in the user's configuration folder); Info shows
  the file, cases, variables, weight, steps and whether it is saved; Export has every export.
  Replacing data with unsaved changes asks first. Esc or Back returns.
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
median, percentiles, skewness, kurtosis. `ALL` and `first TO last` work in every procedure's and
dictionary command's variable lists (expanded by file order each time an analysis runs).

Done: `CROSSTABS` (counts, expected, row/column/total percentages; Pearson, continuity
correction, likelihood ratio, Fisher's exact for 2×2, linear-by-linear; expected-count note),
`T-TEST` (one-sample, independent with Levene and pooled/Welch, paired with correlation;
confidence level), `ONEWAY` (ANOVA, descriptives, Levene). All checked against scipy.

Done too: `CORRELATIONS` (pairwise or listwise, one- or two-tailed, `*`/`**` flags, `WITH`),
`REGRESSION /METHOD=ENTER` (Model Summary, ANOVA, Coefficients with standardized Beta and
confidence intervals; collinearity detected; runs the syntax SPSS pastes). Checked against
statsmodels and scipy.

Done too: `ONEWAY /POSTHOC` (Tukey HSD/Tukey–Kramer with an own studentized range
distribution, Bonferroni), `NPAR TESTS` (Mann-Whitney, Wilcoxon, Kruskal-Wallis, tie
corrections), `LOGISTIC REGRESSION /METHOD=ENTER` (Newton–Raphson; omnibus test, −2LL, Cox & Snell
and Nagelkerke R², classification table, Wald, Exp(B) with CI). Checked against statsmodels and
scipy. **M2 is complete.**

Later in this area: stepwise and blocks in `REGRESSION`/`LOGISTIC REGRESSION`, residuals and
collinearity diagnostics, categorical covariates, exact tests, more post hoc tests. Earlier list, for reference: `CROSSTABS` (χ², Fisher), `T-TEST` (one-sample, independent, paired),
`ONEWAY`, `CORRELATIONS`, `REGRESSION` (linear), `LOGISTIC REGRESSION`.

## M3 — Output Viewer
- **Done (first part):** charts — histogram with optional normal curve, simple bar (count or
  mean by category), scatterplot — via `GRAPH` and `FREQUENCIES /HISTOGRAM /BARCHART`; the
  layout is computed once and drawn by egui on screen and as SVG in exports. Tables pivot
  (transpose) in the viewer. File > Export Output as HTML (and `mksigma-cli --html`): one
  self-contained page, light and dark, printable.
- **Done (second part):** `EXAMINE` (Descriptives with trimmed mean, IQR, skewness and kurtosis
  with standard errors; boxplots on Tukey's hinges with outliers and extremes, by group);
  PDF export (File menu, Output button, `mksigma-cli --pdf`): A4, standard Helvetica (nothing
  embedded) with Windows-1252 text, tables split across pages with their header repeated and
  wrapped headers, the same chart layouts, page numbers.
- **Done (third part): cross-filtering.** Click a bar, a histogram bin, a box of a boxplot by
  group, or a value in a frequency table: every other item runs again on just those cases, the
  clicked item highlights the selection, a bar above the Output names the filter and clears it.
  The filter is a view (not a step) and follows data changes; exports say they are filtered.
  To do: categories in `CROSSTABS`, `ONEWAY` and `T-TEST` group rows; several selections at once
  (Ctrl+click); highlighting instead of filtering inside charts of the same variable.
- To do: clustered bars, pie, stem-and-leaf; pivot layers; chart and table editing.
- Pivot tables (transpose, move layers), charts (histogram, bar, scatter, boxplot)
- **Cross-filtering:** items from one dataset share a filter; selecting a bar or a table
  category filters the other items
- Export to HTML and PDF
- **LaTeX export — done:** File > Export Output as LaTeX (and `mksigma-cli --tex`) writes a
  document that compiles as it is: tables with `booktabs` (no vertical rules, notes inside the
  table), wide tables scaled to the line, charts as `pgfplots` axes drawn from the data (bars,
  histograms with the normal curve, scatterplots, boxplots), captions and unique labels.
  Right-click a table title or a chart: Copy as LaTeX. The button shows the LaTeX name as LaTeX
  typesets it, not the LaTeX Project's logo artwork, which its terms keep off program icons.
- **Output style — done:** `OUTPUT STYLE /FONT=DEFAULT|SANS|SERIF|MONO /SIZE=n
  /PALETTE=DEFAULT|INDIGO|GRAY|BLUE|GREEN|WARM /DECIMALS=AUTO|n` (an MK Sigma command; the
  controls are on the View tab) applies the same look on screen, in HTML (CSS), PDF (Helvetica,
  Times or Courier, the standard fonts every reader has) and LaTeX (class size, `\familydefault`,
  inline `xcolor` colours). Larger sizes scale charts whole. Default keeps each medium's usual
  look. Saved dashboards keep the style. Serif shows in exports only: the screen has the
  interface's fonts.

## M4 — dashboards
- **Slicers — done:** `SLICE var (values)`, `SLICE var (low THRU high)`, `SLICE var OFF`,
  `SLICE OFF` (an MK Sigma command): every analysis in the Output shows only the cases all
  slicers keep, and the cross-filter works within them. In the Output Viewer, "+ Slicer" adds
  one; its chip opens the values with their counts (checkboxes) or, for a Scale variable with
  many values, a range. Slicers are syntax in the History, not applied steps; they follow data
  changes and exports name them.
- **Dashboard — first part done:** a pin beside each Output table and chart puts it on the
  Dashboard tab: cards in a grid (two columns when there is room; the dashboard takes the whole
  window body), the same slicers and cross-filtering, tiles moved earlier or later or unpinned,
  and Export Dashboard writes a self-contained web page of the cards as filtered. Tiles follow
  their items when filters run the analyses again.
- **Saved dashboards — done:** Save Dashboard writes syntax that rebuilds it: `GET FILE` or
  `GET DATA` for the data, the applied steps not already in that file, every analysis, the
  `SLICE`s and `DASHBOARD /TILES=(analysis item)...`. Opening the `.sps` (File > Open) runs it on
  an empty Output and shows the dashboard. Also new: `GET FILE='x.sav'` (File > Open reads system
  files through it), comments (`* ...` and `COMMENT ...`). Analyses rerun on the data as it is
  after every step; Data View edits aren't syntax and are left out, with a note. Tiles are half a row or,
  with Wide (`DASHBOARD /TILES=(n m WIDE)`), a whole row, on screen and in the export. To do:
  free placement and heights, interactive export (below).
- **Publish as a self-contained interactive page — done:** Export Dashboard (and
  `mksigma-cli --dashboard board.html`) writes one HTML file with the engine compiled to
  WebAssembly (about 840 KB, 1.1 MB as base64), the data as a system file and the dashboard's
  syntax. In any browser, offline, clicking a bar or a table value cross-filters and the slicers
  can be changed: the same engine runs the analyses again. It shows the dashboard as exported
  before the engine starts, or if it can't. The page holds every case of every variable.
  `crates/web/build.rs` builds the WebAssembly (`rustup target add wasm32-unknown-unknown`);
  without that target the export is the static page. To do: adding slicers in the page.
- **MCP server — done:** `mksigma-mcp` speaks the Model Context Protocol over stdio (register it
  with `claude mcp add mksigma -- mksigma-mcp`). Tools: `open_data` (.sav, .csv, .tsv),
  `describe_data` (the dictionary), `run_syntax` (returns only the new output, as text),
  `get_output` (text, HTML, LaTeX), `export` (html, pdf, latex, sav, the interactive dashboard,
  or syntax that rebuilds the session) and `history`. One session per connection, through the
  same engine as the app.

## Later
- Multiple related tables (star-schema data model) and measures
- Connectors: PostgreSQL, SQLite, Parquet, HTTP/JSON
- `GLM`, `UNIANOVA`, `FACTOR`, `RELIABILITY`, `NPAR TESTS`, `MIXED`
- Web build of the whole app

## Out of scope for now
- A DAX-like formula language (syntax plus expressions covers the common cases)
- Hosted service: workspaces, permissions, scheduled refresh
- Free-form drag-and-drop report designer (dashboards start from analyses)
