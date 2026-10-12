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
methods (Welford for moments).

## Output (`output`)
Tables, text and warnings as plain data. Frontends render them: text (CLI, tests), egui (app),
later HTML/PDF export.

## Engine (`engine`)
`Session` owns the active dataset and the output, and is the single entry point for syntax,
menus, the CLI and a future MCP server.
