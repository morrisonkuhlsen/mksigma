//! Dashboards that stay interactive in a browser, with no server: the engine compiled to
//! WebAssembly runs inside one HTML file, with the data and the dashboard's syntax. Clicking a
//! bar or a table value cross-filters, slicers can be changed, and the engine runs the analyses
//! again, as in the app.
//!
//! `Board` is the whole logic, plain Rust that tests run natively; `wasm` is the thin layer the
//! page's script calls. `page` writes the file.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use mksigma_engine::Session;
use mksigma_engine::core::{Measure, Value, VarType};
use mksigma_engine::output::html::{Tile, css, escape};
use mksigma_engine::output::{Item, Output};
use mksigma_engine::{Selection, quote};

#[cfg(target_arch = "wasm32")]
mod wasm;

/// This crate as WebAssembly, for `page`; empty when it couldn't be built (see `build.rs`).
#[cfg(not(target_arch = "wasm32"))]
pub const ENGINE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/mksigma_web.wasm"));

/// A dashboard being looked at: the session the page's data and syntax rebuilt.
pub struct Board {
    session: Session,
    error: Option<String>,
}

/// `[u32 data length][data: a system file][the dashboard's syntax]`, little-endian.
pub fn payload(sav: &[u8], program: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + sav.len() + program.len());
    out.extend_from_slice(&u32::try_from(sav.len()).unwrap_or(u32::MAX).to_le_bytes());
    out.extend_from_slice(sav);
    out.extend_from_slice(program.as_bytes());
    out
}

impl Board {
    /// The dashboard from a `payload`.
    pub fn open(payload: &[u8]) -> Result<Board, String> {
        let (len, rest) = payload.split_first_chunk::<4>().ok_or("the page's data is cut short")?;
        let len = usize::try_from(u32::from_le_bytes(*len)).map_err(|e| e.to_string())?;
        let sav = rest.get(..len).ok_or("the page's data is cut short")?;
        let program = std::str::from_utf8(rest.get(len..).unwrap_or_default()).map_err(|e| e.to_string())?;
        let mut session = Session::new();
        session.open_sav(sav).map_err(|e| e.to_string())?;
        session.run_syntax(program).map_err(|e| e.to_string())?;
        Ok(Board { session, error: None })
    }

    /// Does what a click or a slicer asked for and returns the dashboard again:
    /// `select ITEM PART` (a bar, bin or box), `row ITEM ROW` (a table's value), `slice SYNTAX`
    /// (a `SLICE` command) or `clear` (every filter off).
    pub fn call(&mut self, command: &str) -> String {
        self.error = self.act(command.trim()).err();
        self.render()
    }

    fn act(&mut self, command: &str) -> Result<(), String> {
        let (verb, rest) = command.split_once(' ').unwrap_or((command, ""));
        let numbers = || -> Result<(usize, usize), String> {
            let mut n = rest.split_whitespace().map(str::parse::<usize>);
            match (n.next(), n.next()) {
                (Some(Ok(a)), Some(Ok(b))) => Ok((a, b)),
                _ => Err(format!("{verb} needs two numbers")),
            }
        };
        match verb {
            "select" | "row" => {
                let (item, part) = numbers()?;
                let category = match self.session.output.items.get(item) {
                    Some(Item::Chart(c)) if verb == "select" => c.category(part),
                    Some(Item::Table(t)) if verb == "row" => t.category(part),
                    _ => None,
                };
                let category = category.cloned().ok_or("that can't be selected")?;
                self.session.select_category(item, category).map_err(|e| e.to_string())
            }
            // Only slicers: the page changes the view, never the data or the analyses.
            "slice" if rest.trim_start().to_ascii_uppercase().starts_with("SLICE ") => self.session.run_syntax(rest).map_err(|e| e.to_string()),
            "clear" => {
                self.session.clear_cross_filter();
                self.session.run_syntax("SLICE OFF.").map_err(|e| e.to_string())
            }
            _ => Err(format!("{command} is not something a dashboard does")),
        }
    }

    /// The slicers, what filters the dashboard, and its tiles.
    pub fn render(&self) -> String {
        let s = &self.session;
        let mut out = String::new();
        if let Some(e) = &self.error {
            out.push_str(&format!("<p class=\"warning\">{}</p>\n", escape(e)));
        }
        out.push_str("<div class=\"controls\">\n");
        for slicer in s.slicers() {
            out.push_str(&self.slicer(&slicer.variable, Some(&slicer.selection)));
        }
        let filtered = s.filter_description();
        if let Some(f) = &filtered {
            out.push_str(&format!("<p class=\"filters\">Filtered by {} <button type=\"button\" data-clear>Clear filters</button></p>\n", escape(f)));
        }
        out.push_str("</div>\n");
        let ids = s.dashboard();
        let tiles: Vec<Tile> =
            ids.iter().map(|&id| Tile { id, wide: s.is_wide(id), selected: if s.is_filter_source(id) { s.cross_filter() } else { None } }).collect();
        let board = Output { items: ids.iter().filter_map(|i| s.output.items.get(*i).cloned()).collect(), style: s.output.style };
        out.push_str(&board.tiles_html(&tiles));
        out
    }

    /// A slicer's controls: a box per value (with its count) or, for a Scale variable with many
    /// values, a range.
    fn slicer(&self, name: &str, selection: Option<&Selection>) -> String {
        const SHOWN: usize = 50;
        let s = &self.session;
        let Some((_, var)) = s.dataset.variable(name) else { return String::new() };
        let values = s.values_of(name, SHOWN + 1);
        // A slicer keeps its kind; a new one is by values unless a Scale variable has many.
        let by_values = match selection {
            Some(Selection::Values(_)) => true,
            Some(Selection::Range { .. }) => false,
            None => var.var_type != VarType::Numeric || (var.measure != Measure::Scale && values.len() <= SHOWN) || values.len() <= 12,
        };
        let mut out = format!("<fieldset class=\"slicer\" data-var=\"{}\"><legend>{}</legend>", escape(&var.name), escape(var.display_name()));
        if by_values {
            let checked = match selection {
                Some(Selection::Values(v)) => v.as_slice(),
                _ => &[],
            };
            for (value, label, count) in values.iter().take(SHOWN) {
                let literal = match value {
                    Value::Num(x) => x.to_string(),
                    Value::Str(t) => quote(t),
                    Value::SysMis => continue,
                };
                let on = if checked.contains(value) { " checked" } else { "" };
                out.push_str(&format!(
                    "<label><input type=\"checkbox\" data-lit=\"{}\"{on}> {} <span class=\"count\">({count:.0})</span></label>",
                    escape(&literal),
                    escape(label)
                ));
            }
        } else if let Some((lo, hi)) = s.range_of(name) {
            let (a, b) = match selection {
                Some(Selection::Range { low, high }) => (low.unwrap_or(lo), high.unwrap_or(hi)),
                _ => (lo, hi),
            };
            out.push_str(&format!(
                "<label>From <input type=\"number\" step=\"any\" class=\"lo\" value=\"{a}\"></label> <label>to <input type=\"number\" step=\"any\" class=\"hi\" value=\"{b}\"></label> <button type=\"button\" data-apply>Apply</button>"
            ));
        }
        out.push_str("</fieldset>\n");
        out
    }
}

const SCRIPT: &str = r#"
(async () => {
  const bytes = b => Uint8Array.from(atob(b), c => c.charCodeAt(0));
  const board = document.getElementById('board');
  let x;
  try {
    x = (await WebAssembly.instantiate(bytes(document.getElementById('engine').textContent.trim()), {})).instance.exports;
  } catch (e) {
    board.insertAdjacentHTML('afterbegin', '<p class="warning">This browser could not start the dashboard\'s engine: it shows as exported.</p>');
    return;
  }
  const enc = new TextEncoder(), dec = new TextDecoder();
  const put = data => { const p = x.mks_input(data.length); new Uint8Array(x.memory.buffer, p, data.length).set(data); };
  const show = p => { board.innerHTML = dec.decode(new Uint8Array(x.memory.buffer, p, x.mks_output_len())); };
  put(bytes(document.getElementById('data').textContent.trim()));
  show(x.mks_open());
  const call = cmd => { put(enc.encode(cmd)); show(x.mks_call()); };
  const slice = set => {
    const name = set.dataset.var;
    const lo = set.querySelector('.lo'), hi = set.querySelector('.hi');
    if (lo) { call(`slice SLICE ${name} (${Math.min(lo.value, hi.value)} THRU ${Math.max(lo.value, hi.value)}).`); return; }
    const lits = [...set.querySelectorAll('input:checked')].map(i => i.dataset.lit);
    call(`slice SLICE ${name} ${lits.length ? '(' + lits.join(' ') + ')' : 'OFF'}.`);
  };
  board.addEventListener('click', e => {
    const tile = e.target.closest('[data-item]');
    if (e.target.closest('[data-clear]')) call('clear');
    else if (e.target.closest('[data-apply]')) slice(e.target.closest('.slicer'));
    else if (tile && e.target.dataset.part !== undefined) call(`select ${tile.dataset.item} ${e.target.dataset.part}`);
    else if (tile && e.target.dataset.row !== undefined) call(`row ${tile.dataset.item} ${e.target.dataset.row}`);
  });
  board.addEventListener('change', e => { if (e.target.type === 'checkbox') slice(e.target.closest('.slicer')); });
})();
"#;

const PAGE_CSS: &str = r#"
.controls { margin: -8px 0 16px; }
fieldset.slicer { display: inline-block; vertical-align: top; border: 1px solid var(--line); border-radius: 8px; margin: 0 8px 8px 0; padding: 6px 10px; }
fieldset.slicer legend { font-weight: 600; padding: 0 4px; }
fieldset.slicer label { display: inline-block; margin: 2px 10px 2px 0; white-space: nowrap; }
fieldset.slicer input[type=number] { width: 7em; }
.count { color: var(--muted); }
button { font: inherit; }
"#;

/// Base64 (RFC 4648, with padding), for bytes inside an HTML page.
fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk.first().copied().unwrap_or(0), chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18u32, 12, 6, 0].iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(ABC.get(((n >> shift) & 63) as usize).copied().unwrap_or(b'A')));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The dashboard of `session` as one HTML file that stays interactive: the data (every case
/// of every variable, as a system file), the dashboard's syntax and the engine (`wasm`). It
/// shows the dashboard as exported until the engine starts, and if it can't.
pub fn page(session: &Session, title: &str, wasm: &[u8]) -> Result<String, String> {
    let sav = session.data_sav().map_err(|e| e.to_string())?;
    let data = payload(&sav, &session.dashboard_program());
    // What shows before the engine starts: the same board, rendered here.
    let first = Board::open(&data)?.render();
    Ok(format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{t}</title><style>{css}{PAGE_CSS}</style></head><body class=\"dashboard\"><h1>{t}</h1>\n<div id=\"board\">{first}</div>\n\
         <script type=\"application/octet-stream\" id=\"engine\">{engine}</script>\n<script type=\"application/octet-stream\" id=\"data\">{data}</script>\n\
         <script>{SCRIPT}</script></body></html>\n",
        t = escape(title),
        css = css(&session.output.style),
        engine = base64(wasm),
        data = base64(&data),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dashboard() -> Session {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax(
            "RECODE age (LO THRU 30 = 1) (ELSE = 2) INTO grupo.\nVALUE LABELS grupo 1 'Jovem' 2 'Adulto'.\n\
             GRAPH /BAR(SIMPLE)=COUNT BY grupo.\nDESCRIPTIVES score.\nSLICE income (LO THRU 6000).",
        )
        .unwrap();
        s.toggle_pin(0).unwrap();
        s.toggle_pin(1).unwrap();
        s
    }

    #[test]
    fn base64_as_rfc_4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(&[0xFF, 0xEE]), "/+4=");
    }

    #[test]
    fn a_board_rebuilds_the_dashboard_and_filters_like_the_app() {
        let s = dashboard();
        let mut board = Board::open(&payload(&s.data_sav().unwrap(), &s.dashboard_program())).unwrap();
        let html = board.render();
        // Incomes up to 6000 leave out case 5 (6900): 7 cases, scores mean (579 - 58) / 7.
        assert!(html.contains("data-item=\"0\"") && html.contains("data-part=\"1\""), "{html}");
        assert!(html.contains("74.43"), "{html}");
        assert!(html.contains("data-var=\"income\"") && html.contains("class=\"lo\""), "a Scale slicer is a range");

        // Adulto within the slice: cases 2, 3, 6, 7, scores 74 69 72 88, mean 75.75.
        let html = board.call("select 0 1");
        assert!(html.contains("75.75") && html.contains("bar pick sel"), "{html}");
        let html = board.call("slice SLICE income (LO THRU 4000).");
        // Adulto with income up to 4000: cases 6 and 7, scores 72 and 88.
        assert!(html.contains("80.00"), "{html}");
        let html = board.call("clear");
        assert!(html.contains("72.38") && !html.contains("Filtered by"), "{html}");
        assert!(board.call("slice COMPUTE x = 1.").contains("not something a dashboard does"), "only slicers");
        assert!(board.call("select 9 9").contains("can't be selected"));
    }

    #[test]
    fn a_page_holds_the_engine_the_data_and_a_first_view() {
        let s = dashboard();
        let html = page(&s, "Painel <1>", b"\0asm").unwrap();
        assert!(html.contains("<title>Painel &lt;1&gt;</title>"));
        assert!(html.contains("id=\"engine\">AGFzbQ==</script>"), "the engine in base64");
        assert!(html.contains("<div id=\"board\"><div class=\"controls\">"), "a first view without the engine");
        assert!(Board::open(b"\x05\0\0\0abc").is_err(), "cut short");
    }
}
