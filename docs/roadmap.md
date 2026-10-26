# Roadmap

MK Sigma covers one workflow end to end: **get data in → prepare it → analyse it → publish an
interactive dashboard of the results**, every step recorded as syntax, so it is reproducible and
drivable by agents. The core is a solid SPSS-style statistics package; the Power BI-style
features (recorded data preparation, cross-filtering, dashboards) are a layer on top of it.
Beyond SPSS, the strengths of the other reference packages (SAS, Stata, JMP, Minitab, JASP,
jamovi) join as modules of the same engine: see *Beyond SPSS* below.

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
  fields across lines; a qualifier mid-field is literal), the encoding from a byte-order mark
  (UTF-8, UTF-16), else UTF-8 with bad bytes replaced, or Windows-1252 when nothing beyond
  ASCII is UTF-8. MK Sigma extensions: without `/VARIABLES` the names come from the first
  record (`/READNAMES`) and types are inferred (numbers with grouped thousands, and decimal
  commas when the delimiter isn't a comma, read one way per column, the file's other columns
  settling `1.500`; identifiers with leading zeros or over 15 digits, such as CEP and CPF, stay
  strings; strings as wide as their longest value; repeated whole-number codes are Nominal);
  without `/DELIMITERS` the delimiter is detected. Opening `.csv`/`.tsv`/`.txt` in the app or CLI runs `GET DATA`, so the History
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
  are skipped and shown in red with the reason; so is a cell edit or a case inserted or deleted
  when the steps before it that add or remove cases changed, as its row would be another case.
  Step changes are undoable. Next: cell edits that follow their case by a stable case id; steps saved
  with the file (a `.sps` beside the `.sav`), inserting a step in the middle.
- Desktop app: File ▸ Open (native dialog, Ctrl+O), drag and drop, opening a file given on the
  command line, Data View (virtualized rows, value-labels toggle, missing values dimmed) and a
  Variable View — **done**.
- Window layout — **done**: an office-suite layout (title row with quick-access Save, Undo and
  Redo; File menu and tabs Home, Data, Transform, Analyze, Graphs, View over a ribbon of large
  buttons with icons drawn in code, in colour by meaning (duotone objects — a soft tint under a crisp line — and the window's controls in the text colour); Data View as a grid with headers and gridlines; a status bar with the data's size, weight and filter),
  light and dark, indigo accented, square-cornered throughout (panels, buttons, tabs, dialogs).
  As in report designers, a view rail down the left is the one place that switches views
  (Data View and Variable View beside the Output, the Dashboard on the whole window), each with
  an icon of its own, and a Filters pane down the right that starts collapsed to a strip with
  its name and how many filters are on. Analyze and Graphs buttons paste the procedure's syntax for
  suggested variables into the editor and run it; when no variable fits they paste a template.
- File backstage — **done**: File opens a page over the window: a column in Morrison
  Kühlsen's ink with its logo, MK Sigma's name and motto, and the places grouped (Start: Home,
  New, Open; This data: Info, Save, Save As, Export, Close), and the page beside it with "Back
  to the data" in its corner. The app's accent is Morrison Kühlsen's blue (#2540D8). Home and New start a blank dataset or the sample; Home
  and Open list recent files (kept between runs in the user's configuration folder); Info shows
  the file, cases, variables, weight, steps and whether it is saved; Export has every export.
  Replacing data with unsaved changes asks first. Esc or Back returns.
- Procedure dialogs — **done**: every Analyze and Graphs button (Descriptives, Frequencies,
  Explore, Crosstabs, T-Test, One-Way ANOVA, Correlate, Regression, Logistic, Nonparametric with
  Mann-Whitney or Kruskal-Wallis; Histogram, Bar, Scatter, Boxplot with an optional title) opens a
  dialog: the file's variables (with their
  measurement level) on the left, the procedure's slots on the right (`>`/`<` or a double-click
  move a variable; numeric-only slots refuse strings), options below; a t-test's groups start
  as the grouping variable's first two values. OK runs the command, Paste puts it in the syntax
  editor. Optional slots (Explore's factor, a bar chart's "mean of") may stay empty.
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
  removed column, whether it weighted the cases); undoing a command also takes it out of the
  History. A command changes all it was asked to or nothing (a `RECODE` whose second variable
  fails leaves the first as it was). After a save, the dashboard syntax builds on the file only
  while the steps still start with those it holds. Edit menu names the
  step; Ctrl+Z, Ctrl+Shift+Z / Ctrl+Y when no text box has the keyboard.
- Adding and deleting — **done**: `NUMERIC`, `STRING`, `DELETE VARIABLES`; blank last rows in
  Data View (adds a case) and Variable View (adds a numeric variable); right-click a case
  number to insert or delete a case, a variable number to delete it.

## M2 — the introductory-course core
Done: case weights (`WEIGHT BY` from the file), `DESCRIPTIVES` weighted, `FREQUENCIES` (frequency
tables with labels and missing sections, `/FORMAT` order and `NOTABLE`, `/STATISTICS` mean,
SE mean, mode, std. deviation, variance, range, minimum, maximum, sum). To do in `FREQUENCIES`:
median, percentiles, skewness, kurtosis. `ALL` and `first TO last` work in every procedure's and
dictionary command's variable lists (expanded by file order each time an analysis runs), in
`RECODE`, in the pairs of `T-TEST /PAIRS` and `NPAR TESTS /WILCOXON` (paired once expanded), and
among function arguments (`SUM(q1 TO q5)`); lists of new variables (`NUMERIC`, `STRING`,
`RECODE … INTO`) take `x1 TO x5` as numbered names. To do: `TO` in `RENAME VARIABLES`.

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
  slicers keep, and the cross-filter works within them. In the Filters pane, "+ Slicer" adds
  one, each slicer and the cross-filter on its own card; a slicer's chip opens the values with their counts (checkboxes) or, for a Scale variable with
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
  files through it), comments (`* ...` and `COMMENT ...`, ending at a period at the end of a line or a blank line; `/* ... */` anywhere), a blank line ending a command, signed exponents (`1E-5`). Analyses rerun on the data as it is
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

## Next: the calculation shown by hand (Σ)
- A **Σ** button beside every table and chart: it opens the whole calculation as it would be
  done by hand, in mathematical notation (LaTeX, rendered in the app and exportable): the
  formulas from the public algorithm documentation, then each with the data's own numbers
  substituted, every intermediate sum and step, down to the figures in the table. Audited and
  complete: the steps come from the same engine run that produced the result (not recomputed
  apart), so they always agree with it, and a reader can check every number with a calculator.
  Weights, missing values and filters show where they enter. Start with DESCRIPTIVES,
  FREQUENCIES, T-TEST and CORRELATIONS; large data shows the sums, with the terms on demand.
- **First part — done:** the Σ beside DESCRIPTIVES, CORRELATIONS and frequency tables opens a
  window with every step, typeset in the app (fractions, sums with limits, roots, bars; long
  equations break at `=` and `+`). The first five and last two terms of a long sum, or every
  term on request; Copy LaTeX and Export .tex give a document that compiles as it is. The
  steps run on the cases the table shows (slicers, cross-filter), say so in the title, and a
  table made before the data changed is refused rather than explained on other numbers. Tests
  hold each final figure against the table's.
- **T-TEST — done:** every table of a t test opens the whole test: one sample against its test
  value; two groups with each one's mean and variance, Levene's test as the ANOVA on the
  distances from the group means, the pooled version and Welch's with Satterthwaite's degrees
  of freedom; paired samples with the differences case by case and the pair's correlation.
  t's significance and the interval's critical t are the only figures from a distribution.
- **FREQUENCIES' Statistics, CROSSTABS' chi-square, ONEWAY — done:** the statistics from the
  frequencies (Σ f·x); the expected counts, Pearson's chi-square, the likelihood ratio, Yates'
  correction, Fisher's exact test as every table with the same totals in factorials, the
  linear-by-linear association; each group's mean and variance, the sums of squares, F,
  Levene's test for k groups, Tukey's q and Bonferroni's multiplied t for every pair.
- **NPAR TESTS, EXAMINE — done:** every value ranked, ties as the mean of the ranks they span,
  the sums and mean ranks, U and W, T− and T+, H, the correction for ties, z; Explore's
  Descriptives with the 5% trimmed mean (partial weights at the cuts), the median and quartiles
  by the weighted average at (W + 1)p, and G1 and G2 with their standard errors.
- **REGRESSION — done:** the cross-products of the deviations, the slopes from Sxx b = Sxy
  (Sxx inverted by Gauss–Jordan, every row operation shown, with two predictors or more), the
  constant, the analysis of variance, R², and each coefficient's standard error, t, beta and
  interval. As in SPSS, `/METHOD=ENTER` applies the tolerance test (`/CRITERIA=TOLERANCE`,
  .0001 by default): a predictor that is (almost) a combination of those before it is left
  out and listed in Excluded Variables (Beta In, t, partial correlation, tolerance), each
  figure with its Σ. `/ORIGIN` fits without a constant: sums of squares about 0, R² of the
  variability about 0, one more residual degree of freedom; its Σ shows the products about 0.
- **LOGISTIC REGRESSION — done:** the coding of the dependent, the constant-only model (its
  probability, log odds and −2LL), every Newton–Raphson iteration (the first with the gradient
  and the information written out case by case; then g, H, H⁻¹, the step and the new
  estimates), the stopping rule, the final probabilities, −2LL, the chi-square, Cox and
  Snell's and Nagelkerke's R², the classification, and each coefficient's standard error from
  the inverted information (Gauss–Jordan), Wald, Exp(B) and its interval. The iterations are
  the engine's own run, shared with the table.
- **The charts — done:** a Σ beside histograms, bar charts and EXAMINE's boxplots. A
  histogram: the values in order, Sturges' 1 + log₂ N, the round bin width, each bin's count,
  and with /NORMAL the mean, the standard deviation and the curve's peak N·h/(s√(2π)). A bar:
  its count (or sum of weights) or its mean. A box: the depths of the median and of Tukey's
  hinges, their values, the box length, the fences at 1.5 and 3 lengths, the whiskers,
  outliers and extremes. The bins, bars and boxes are the charts' own. Scatterplots compute
  nothing and have no Σ.
- **Next:** the Σ for the remaining tables (Crosstabs' cell percentages, Explore's case
  processing summary) and the Learn articles linking to them.

## Languages — done (English and Português (Brasil))
- The interface, the Output (titles, labels, notes, warnings, exports), the Σ explanations and
  the error messages speak English or Brazilian Portuguese; command syntax stays English, as
  in SPSS. One catalog per language in a subset of Fluent (`crates/i18n/locales/*.ftl`),
  embedded, read by `mksigma-i18n` (L0, no dependencies); its tests fail when a language lacks
  a message or a placeholder, or the code asks for one English lacks.
- View ▸ Language switches at once and the Output is made again in the new language; the
  choice is kept, and the first run follows the system's (LC_ALL, LC_MESSAGES, LANG).
- Numbers shown in the Output, charts and Σ take the language's decimal comma; syntax, saved
  files and Data View keep the point.
- **Next:** Español; Data View with the decimal comma (typing it too); the interactive
  dashboard's page in the exporting language.

## Learn, the study section — first part done
- A Learn page in the File backstage, and a "?" beside every table and in every dialog that
  has an article. Articles in English and Portuguese, Markdown under `crates/learn/articles/`,
  read by `mksigma-learn`: headings, lists, notes, emphasis, links to articles or to a
  procedure's dialog, formulas in a subset of LaTeX drawn as the Σ draws them, and syntax
  examples with Run and Paste. Tests read every article in both languages, check every link,
  that both languages give the same examples, and that every example runs on the sample data.
- Written so far: "Which test?", Descriptives, the independent-samples t test, Crosstabs and
  chi-square, the p-value. Written for MK Sigma, never copied from a manual.
- **Next:** an article for every procedure (Frequencies, Explore, ANOVA, correlation,
  regression, logistic regression, the nonparametric tests, the charts) and for the concepts
  (levels of measurement, distributions, sampling, confidence intervals, effect sizes, power,
  assumptions); sources cited per article; the articles in the exported web pages.

## Beyond SPSS: what the other packages bring — not started
SPSS parity comes first (its remaining procedures are under Later); then each package's
strength joins as a module of the same engine: commands in the one syntax (SPSS's names where
SPSS has the procedure, MK Sigma's own otherwise), dialogs that only write syntax, a Σ for every
figure, Learn articles in both languages, parity tests against independently computed values,
and nothing that breaks wasm. Clean-room (see `AGENTS.md`): proprietary packages are not run to
shape MK Sigma; behaviour comes from public documentation (ideas only), the literature and open
tools run as oracles, with no code from the GPL ones (jamovi, JASP, R and CRAN, PSPP, gretl). One
vertical slice at a time; **the order of the modules is the user's to decide.** The order chosen:
the noncentral distributions, quality, power and sample size, then design of experiments (classical
first, the optimal designs after); the other modules follow.

- **Foundations, before any module:**
  - *NIST StRD audit — first part done* (`crates/stats/tests/nist.rs`): 9 univariate, 11
    ANOVA and 9 linear-regression datasets, built in code, certified values as constants, each
    with a floor of correct digits (LRE) no change may lower. DESCRIPTIVES reaches 8.3–15
    digits (NumAcc4 the lowest); ONEWAY 8.8–14.8, except SmLs07–09 at 3.3, the limit of the
    data themselves in a double (10¹² plus tenths). REGRESSION now solves by a Householder QR
    decomposition instead of the normal equations: Norris, Pontius and Longley 11.9–12.8, the
    Wampler polynomials 9.0–12.9 (5.7 and 7.7 on the noisy Wampler5 and 4, from 4.5), and
    Filip's tenth-degree polynomial, refused before, to 7.6; through the origin (`/ORIGIN`),
    NoInt1 and NoInt2 to 14.5–15. Its Σ still shows the normal equations, and says the table
    takes the QR route. **Next:** the nonlinear datasets once there is a nonlinear regression.
  - *Stata files — done:* `GET STATA FILE='x.dta'` (`crates/dta`, from StataCorp's published
    description) reads formats 113–115 and 117–119 (Stata 8 and later) in either byte order:
    numeric types, `str#` and `strL` (crosslinked too), value labels, variable and dataset
    labels; `.a`–`.z` read as system-missing with a warning; `%td`, `%tc`, `%tm`, `%tq` become
    dates; names Stata allows and MK Sigma doesn't (`_merge`) are renamed, with a warning.
    `SAVE TRANSLATE OUTFILE='x.dta' /TYPE=STATA [/REPLACE]` writes format 118: each numeric
    variable in the smallest type that holds it exactly, strings as `str#` up to 2045 bytes and
    `strL` beyond, dates back to Stata's units, user-missing values as `.a`, `.b`, `.c`, value
    labels of whole numbers, names and labels cut or renamed to Stata's limits, each change
    a warning. File › Open and Save As, dropped files, `mksigma-cli --in/--out` and MCP's
    `open_data`/`export` take `.dta`. Dates show as dates and are typed in Data View as their
    format writes them. **Next:** `SET EPOCH` for two-digit years (#13).
  - *SAS transport files — reading done:* `GET SAS DATA='x.xpt'` (`crates/xpt`, from SAS's
    published record layout, TS-140) reads version 5 transport files, the format regulators
    accept: the first member's variables, labels, numbers (IBM floating point, decoded from its
    definition), text (UTF-8 or Windows-1252), `.` and `.A`–`.Z` as system-missing, SAS dates,
    datetimes and times as MK Sigma's. File › Open, dropped files, `mksigma-cli --in` and MCP's
    `open_data` take `.xpt`. **Next:** version 8/9 transport files (`LIBV8`) and `/DSET` for a
    member other than the first. `.sas7bdat` stays out: it is known only through third parties'
    reverse engineering.
  - *A parity matrix:* a table in code of every procedure against SPSS, SAS, Stata, JMP,
    Minitab, JASP and jamovi (has it, part of it, not yet), printed by an xtask and in the
    docs, so the unification is visible and measured.
  - *Noncentral distributions (#52):* t, F, χ² and beta, with CDF, quantile and a solver for the
    noncentrality, from the published series (Lenth 1989, Ding 1992, Frick 1990). One piece
    behind power and sample size, the run length of control charts, the power of a design and
    confidence intervals for effect sizes.
- **Quality: control charts, capability, measurement systems (Minitab's niche) — next:**
  industry uses it every day, and automotive suppliers are required to (IATF 16949). In the UI
  it is "Quality", not "Six Sigma", a Motorola trademark.
  - *First part:* `SPCHART` (SPSS's name) with X̄-R, X̄-S and I-MR (#53) and p, np, c and u (#54):
    subgroups of varying size, σ by R̄/d₂, S̄/c₄, pooled SD or MR̄/d₂, Phase I and II, stages.
    Nelson's tests for special causes (#55). Capability for normal data (#56): Cp, Cpk, Pp,
    Ppk and Cpm, expected and observed ppm, confidence intervals, the normality check. Pareto and
    run charts (#57). The Σ shows every limit, with d₂, d₃ and c₄ computed, not tabled; control
    charts cross-filter and go on dashboards like every chart.
  - *Second part:* gage R&R by ANOVA and by X̄-R, attribute agreement (kappa), capability for
    non-normal data (Box-Cox, Johnson, Weibull), EWMA and CUSUM, Laney's p′ and u′.
  - *Out:* DMAIC project templates, value stream maps, a real-time SPC server.
  - Free tools exist (R's `qcc`, JASP's quality module), so what sets MK Sigma apart is the Σ,
    Portuguese, dashboards and a native app without R.
- **Power and sample size (what researchers use G*Power for):** `POWER`, as SPSS names it, for the
  t tests (#58), the F tests of ANOVA and regression (#59), proportions, χ², correlation and
  McNemar (#60). Each test has five analyses: a priori, post hoc, sensitivity, criterion and
  compromise. Observed power is offered only with a warning (Hoenig and Heisey 2001). Power
  curves, and "Plan a study" beside a result, pasting `POWER` with its effect size (#61). The
  Σ shows the noncentrality, the critical value and the power. The noncentrality convention for
  F (Cohen's f²(u + v + 1) or f²·N) is stated in the Σ. Sources: Cohen (1988), Faul et al. (2007);
  G*Power is free but not open source and is treated as proprietary until its licence is checked.
- **JMP — exploration and design of experiments:**
  - *Classical designs, after power:* generated designs as datasets (full and fractional
    factorials with their generators, resolution and alias structure, Plackett–Burman, central
    composite, Box–Behnken; blocks, centre points, randomized run order), their analysis
    (effects, a Pareto of effects, Daniel's half-normal plot and Lenth's PSE for unreplicated
    designs, curvature and lack of fit, the response surface and its stationary point), and a
    profiler with Derringer–Suich desirability whose factors are slicers on a dashboard. The Σ
    shows the alias structure.
  - *Optimal designs, after that:* D- and I-optimal designs by coordinate exchange, with
    constraints and the random seed recorded in the syntax; definitive screening designs (Jones
    and Nachtsheim 2011); mixtures; split-plot designs once `MIXED` (REML) exists; the power of a
    design.
  - A one-click distribution view of a variable (histogram, boxplot, quantiles and moments
    together). Linked views extend cross-filtering: selecting in one chart highlights the same
    cases in the others.
  - Sources: Montgomery, *Design and Analysis of Experiments*; Box, Hunter and Hunter; the NIST
    e-Handbook, ch. 5. Oracles: R's `FrF2`, `rsm` and `AlgDesign`.
- **Stata — econometrics and epidemiology:** robust (HC) and cluster-robust standard errors;
  panel data (fixed, random and between effects, the Hausman test); instrumental variables by
  two-stage least squares with the first-stage F; probit, ordered and multinomial logit,
  Poisson and negative binomial; marginal effects; complex survey designs (strata, clusters,
  weights, Taylor linearization) for means, proportions, totals and regressions; risk and odds
  ratios with Mantel–Haenszel. Time series after these.
- **SAS — large and auditable:** data larger than memory (columnar, read in chunks, procedures
  that stream), and an audit trail: every Output item carries its syntax, a checksum of the
  data it ran on and MK Sigma's version, so a result can be run again and verified, as
  regulated work needs.
- **JASP — Bayesian beside frequentist:** Bayes factors for t tests, correlation, contingency
  tables, ANOVA and regression, with prior and posterior plots and a robustness check over
  the prior's width, the Σ showing the integrals' numbers; an APA-style Output style of MK
  Sigma's own design.
- **jamovi — live results:** a dialog's results update in the Output as its choices change
  (the dialog still only writes syntax), and editing an earlier analysis runs it again in
  place.

What MK Sigma brings that none of them has together: every figure with its calculation by
hand (Σ), exportable as LaTeX; a Learn article beside every table; recorded data preparation;
dashboards published as one offline page with the engine inside; one engine for the window,
the command line and agents (MCP); Brazilian Portuguese throughout; open source, natively on
Linux, Windows and macOS.

The first modules, in order: the noncentral distributions (#52), quality (#53–#57), power and
sample size (#58–#61), classical design of experiments, optimal designs. Quality comes first for
the industry that pays for it and for its short formulas, which suit the Σ; power reaches most
users for little work and could be swapped ahead of it.

## Later
- Multiple related tables (star-schema data model) and measures
- Connectors: PostgreSQL, SQLite, Parquet, HTTP/JSON
- SPSS's remaining procedures: `GLM`, `UNIANOVA`, `FACTOR`, `RELIABILITY`, `MIXED`
- Web build of the whole app

## Out of scope for now
- A DAX-like formula language (syntax plus expressions covers the common cases)
- Hosted service: workspaces, permissions, scheduled refresh
- Free-form drag-and-drop report designer (dashboards start from analyses)
