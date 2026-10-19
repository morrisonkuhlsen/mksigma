# MK Sigma in English: the reference catalog; every other language has each message.

language-menu = Language

## Ribbon and title row
autosave = AutoSave
autosave-on-tip = AutoSave is on: every change is saved to the file
autosave-off-tip = AutoSave: save the data to its file after every change
save = Save
search-tip = Search commands and variables (Ctrl+F)
export-output-tip = Export the Output
export-html-menu = Export Output as HTML…
export-pdf-menu = Export Output as PDF…
export-latex-menu = Export Output as LaTeX…
theme-tip = Theme
theme-light = Light
theme-dark = Dark
theme-system = System
file = File
file-tip = Open, save, export and recent files
ribbon-show = Show the ribbon (Ctrl+F1)
ribbon-hide = Hide the ribbon (Ctrl+F1)
open = Open
open-tip = Open a data file (Ctrl+O)
save-tip = Save the data (Ctrl+S)
undo = Undo
redo = Redo
nothing-to-undo = Nothing to undo
nothing-to-redo = Nothing to redo
run-syntax = Run Syntax
run-syntax-tip = Run every command in the syntax editor
export-html = Export HTML
export-html-tip = Save the Output as a web page
export-pdf = Export PDF
export-pdf-tip = Save the Output as a PDF document
export-latex-tip = Save the Output as a LaTeX document: booktabs tables, pgfplots charts
export-latex = Export LaTeX
clear-output = Clear Output
clear-output-tip = Remove every item from the Output
insert-case = Insert Case
insert-case-tip = Add a blank case at the end
insert-variable = Insert Variable
insert-variable-tip = Add a numeric variable
value-labels = Value Labels
value-labels-tip = Show value labels instead of values
dictionary = Dictionary
dictionary-tip = Show every variable's definition in the Output
suggest-tip = Adds a command to the syntax editor to complete and run
compute = Compute
recode = Recode
select-cases = Select Cases
dialog-tip = Opens its dialog: pick the variables and options
chart-dialog-tip = Opens its dialog: pick the variables
descriptives = Descriptives
frequencies = Frequencies
explore = Explore
crosstabs = Crosstabs
t-test = T-Test
oneway = One-Way ANOVA
correlate = Correlate
regression = Regression
logistic = Logistic
nonparametric = Nonparametric
histogram = Histogram
bar = Bar
scatter = Scatter
boxplot = Boxplot
data-view = Data View
data-view-tip = Cases and their values
variable-view = Variable View
variable-view-tip = The variables and their definitions
output-style = Output Style
output-style-tip = How tables and charts look on screen and in every export
font = Font
size = Size
palette = Palette
decimals = Decimals
open-ellipsis = Open…
save-as-ellipsis = Save As…
save-dashboard-ellipsis = Save Dashboard…
display-dictionary = Display Dictionary
compute-variable = Compute Variable
t-test-independent = Independent-Samples T-Test
correlations-bivariate = Bivariate Correlations
regression-linear = Linear Regression
logistic-binary = Binary Logistic Regression
nonparametric-tests = Nonparametric Tests (Mann-Whitney)
bar-chart = Bar Chart
scatterplot = Scatterplot
theme-light-full = Light Theme
theme-dark-full = Dark Theme
tab-home = Home
tab-data = Data
tab-transform = Transform
tab-analyze = Analyze
tab-graphs = Graphs
tab-view = View
undo-named = Undo { $what }
redo-named = Redo { $what }
decimals-auto = Auto

## Window and status
status-sample-start = Sample dataset. Use File > Open… or drop a .sav file on the window.
status-in-syntax-editor = { $name } is in the syntax editor: Run runs it
status-opened-dashboard = Opened the dashboard { $name }
status-new-dataset = New dataset: add variables in Variable View, or open a file.
status-sample = Sample dataset.
status-dashboard-needs-file = The data didn't come from a file it could be read again from: open it with File > Open, or save it first
status-saved-dashboard = Saved the dashboard to { $path }
export-title-output = MK Sigma Output
export-title-output-of = { $file } — Output
export-title-filtered = (filtered by { $filters })
status-exported-output = Exported the Output to { $path }
export-title-dashboard = MK Sigma Dashboard
export-title-dashboard-of = { $file } — Dashboard
status-exported-interactive = an interactive dashboard (it holds the data)
status-exported-static = the dashboard
status-exported-dashboard = Exported { $what } to { $path }
status-saved = Saved { $name }
status-opened = { $name }: { $cases ->
    [one] { $cases } case
    *[other] { $cases } cases
    }, { $variables ->
    [one] { $variables } variable
    *[other] { $variables } variables
    }
status-undid = Undid { $what }
status-redid = Redid { $what }
untitled = Untitled
search-variable = Variable: { $label }
search-hint = Search commands and variables
search-nothing = Nothing found
status-template-added = Added a command to the syntax editor: replace the placeholder names, then Run.
dropped-file = dropped file
syntax = Syntax
run = Run
run-tip = Run every command in the editor
steps-paste = Paste steps into syntax
step-apply = Apply this step
step-skipped = Skipped: { $error }
step-up = up
step-up-tip = Move this step earlier
step-down = down
step-down-tip = Move this step later
step-remove = remove
step-remove-tip = Remove this step
status-exported-calculation = Exported the calculation to { $path }
status-pasted = Pasted into the syntax editor: Run runs it.
status-ready = Ready
fact-cases = { $n ->
    [one] { $n } case
    *[other] { $n } cases
    }
fact-variables = { $n ->
    [one] { $n } variable
    *[other] { $n } variables
    }
fact-weighted = Weighted by { $name }
zoom-reset = Zoom to 100%
zoom-in = Zoom In
zoom-out = Zoom Out
zoom = Zoom
fact-filtered = Filtered by { $filters }
