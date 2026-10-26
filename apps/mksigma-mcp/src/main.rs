//! MK Sigma as a Model Context Protocol server over stdio: an agent opens data, reads its
//! dictionary, runs command syntax, reads the Output and exports it, through the same `Session`
//! the app and the CLI use. One session lives as long as the connection.
//!
//! `mksigma-mcp` reads JSON-RPC messages, one per line, on stdin and answers on stdout. Register
//! it with an MCP client, e.g. `claude mcp add mksigma -- mksigma-mcp`.
//!
//! The agent reaches files only inside one folder: the working directory, or the one given with
//! `--root DIR`. Paths outside it (through `..` or symbolic links too) are refused, and each
//! export writes only its own kind of file (`.pdf`, `.sav`...), so an agent can't read private
//! files or overwrite others.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::io::{BufRead, Write};

use mksigma_engine::Session;
use mksigma_engine::core::{Measure, VarType};
use mksigma_engine::output::Output;
use serde_json::{Value, json};

const PROTOCOL: &str = "2025-06-18";

/// The tools, with their arguments as JSON Schema.
fn tools() -> Value {
    json!([
        {
            "name": "open_data",
            "description": "Open a data file (inside the server's folder) as the active dataset: an SPSS system file (.sav), a Stata file (.dta, Stata 8 and later), a SAS transport file (.xpt, version 5) or delimited text (.csv, .tsv, .txt; names from the first line, types inferred, delimiter detected). Replaces the data; the Output stays.",
            "inputSchema": {"type": "object", "properties": {"path": {"type": "string", "description": "Path to the file"}}, "required": ["path"]}
        },
        {
            "name": "describe_data",
            "description": "The active dataset's dictionary: every variable's name, type, label, measurement level, format, value labels and missing values, and the number of cases.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "run_syntax",
            "description": "Run SPSS-style command syntax (one or more commands, each ending with a period): DESCRIPTIVES, FREQUENCIES, CROSSTABS, T-TEST, ONEWAY, CORRELATIONS, REGRESSION, LOGISTIC REGRESSION, NPAR TESTS, EXAMINE, GRAPH; COMPUTE, IF, RECODE, SELECT IF; VARIABLE LABELS, VALUE LABELS, MISSING VALUES, VARIABLE LEVEL, FORMATS; SLICE, OUTPUT STYLE, DASHBOARD. Returns the output the commands produced, as text. Commands before a failing one keep their effect.",
            "inputSchema": {"type": "object", "properties": {"syntax": {"type": "string"}}, "required": ["syntax"]}
        },
        {
            "name": "get_output",
            "description": "The whole Output so far (tables, notes, chart titles), as text, HTML or LaTeX.",
            "inputSchema": {"type": "object", "properties": {"format": {"type": "string", "enum": ["text", "html", "latex"], "default": "text"}}}
        },
        {
            "name": "export",
            "description": "Write a file inside the server's folder, with the format's extension (.html, .pdf, .tex, .sav, .dta, .html, .sps): the Output as html, pdf or latex; the active data as sav or dta (Stata); the dashboard (tiles set with DASHBOARD /TILES) as an interactive dashboard page, which holds the data; or the session as syntax that rebuilds it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "format": {"type": "string", "enum": ["html", "pdf", "latex", "sav", "dta", "dashboard", "syntax"]}
                },
                "required": ["path", "format"]
            }
        },
        {
            "name": "history",
            "description": "Every command that ran in this session, in order: the syntax that reproduces the analysis.",
            "inputSchema": {"type": "object", "properties": {}}
        }
    ])
}

struct Server {
    session: Session,
}

/// The extensions each export format may write.
fn extensions(format: &str) -> &'static [&'static str] {
    match format {
        "html" | "dashboard" => &["html", "htm"],
        "pdf" => &["pdf"],
        "latex" => &["tex"],
        "sav" => &["sav"],
        "dta" => &["dta"],
        "syntax" => &["sps"],
        _ => &[],
    }
}

/// A tool's answer: its text, and whether it is an error.
type Answer = (String, bool);

impl Server {
    /// A server whose files stay inside `root`.
    fn new(root: std::path::PathBuf) -> Self {
        let mut session = Session::new();
        session.file_root = Some(root);
        Self { session }
    }

    /// The answer to one JSON-RPC message; `None` for a notification.
    fn handle(&mut self, line: &str) -> Option<String> {
        let message: Value = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(e) => return Some(error(Value::Null, -32700, &format!("not JSON: {e}"))),
        };
        let id = message.get("id").cloned();
        let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        // Notifications (no id) get no answer.
        let id = id?;
        let result = match method {
            "initialize" => json!({
                "protocolVersion": PROTOCOL,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "mksigma", "version": env!("CARGO_PKG_VERSION")},
                "instructions": format!("MK Sigma: SPSS-style statistics. Open data with open_data, look at it with describe_data, analyse it with run_syntax (SPSS command syntax), then export. Files are read and written only inside {} (relative paths start there); each export writes only its format's extension.", self.root())
            }),
            "ping" => json!({}),
            "tools/list" => json!({"tools": tools()}),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
                let args = params.get("arguments").cloned().unwrap_or(Value::Null);
                let (text, is_error) = self.call(name, &args);
                json!({"content": [{"type": "text", "text": text}], "isError": is_error})
            }
            other => return Some(error(id, -32601, &format!("no method {other}"))),
        };
        Some(json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string())
    }

    fn call(&mut self, name: &str, args: &Value) -> Answer {
        let text = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
        match name {
            "open_data" => self.open_data(&text("path")),
            "describe_data" => (self.describe(), false),
            "run_syntax" => self.run(&text("syntax")),
            "get_output" => {
                let out = &self.session.output;
                match text("format").as_str() {
                    "html" => (out.to_html("MK Sigma Output"), false),
                    "latex" => (out.to_latex("MK Sigma Output"), false),
                    _ if out.items.is_empty() => ("The Output is empty.".to_string(), false),
                    _ => (out.to_text(), false),
                }
            }
            "export" => self.export(&text("path"), &text("format")),
            "history" => (if self.session.journal.is_empty() { "Nothing has run yet.".to_string() } else { self.session.journal.join("\n") }, false),
            other => (format!("There is no tool {other}."), true),
        }
    }

    fn open_data(&mut self, path: &str) -> Answer {
        let lower = path.to_ascii_lowercase();
        let file = mksigma_engine::quote(path);
        let syntax = if [".csv", ".txt", ".dat"].iter().any(|e| lower.ends_with(e)) {
            format!("GET DATA /TYPE=TXT /FILE={file}.")
        } else if lower.ends_with(".tsv") {
            format!("GET DATA /TYPE=TXT /FILE={file} /DELIMITERS=\"\\t\".")
        } else if lower.ends_with(".dta") {
            format!("GET STATA FILE={file}.")
        } else if lower.ends_with(".xpt") {
            format!("GET SAS DATA={file}.")
        } else {
            format!("GET FILE={file}.")
        };
        let result = self.session.run_syntax(&syntax);
        let notes = self.session.take_notes().join("\n");
        match result {
            Ok(()) => (format!("{notes}\n\n{}", self.describe()), false),
            Err(e) => (e.to_string(), true),
        }
    }

    /// The dictionary as text, one variable a line.
    fn describe(&self) -> String {
        let ds = &self.session.dataset;
        if ds.variables().is_empty() {
            return "No data is open: use open_data.".to_string();
        }
        let mut out = format!("{} cases, {} variables\n", ds.n_cases(), ds.variables().len());
        for v in ds.variables() {
            let kind = match v.var_type {
                VarType::Numeric => "numeric".to_string(),
                VarType::String(w) => format!("string({w})"),
            };
            let measure = match v.measure {
                Measure::Nominal => "nominal",
                Measure::Ordinal => "ordinal",
                Measure::Scale => "scale",
            };
            out.push_str(&format!("- {} ({kind}, {measure}, {})", v.name, v.format));
            if let Some(label) = &v.label {
                out.push_str(&format!(" \"{label}\""));
            }
            if !v.value_labels.is_empty() {
                let labels: Vec<String> = v.value_labels.iter().map(|(value, l)| format!("{} = {l}", v.format.render(value))).collect();
                out.push_str(&format!("; values: {}", labels.join(", ")));
            }
            if v.missing != mksigma_engine::core::MissingValues::None {
                out.push_str("; has user-missing values");
            }
            out.push('\n');
        }
        out
    }

    fn run(&mut self, syntax: &str) -> Answer {
        let before = self.session.output.items.len();
        let result = self.session.run_syntax(syntax);
        let notes = self.session.take_notes();
        let items = &self.session.output.items;
        let new = Output { items: items.get(before..).unwrap_or(items).to_vec(), style: self.session.output.style };
        let mut text = notes.join("\n");
        if !new.items.is_empty() {
            if !text.is_empty() {
                text.push_str("\n\n");
            }
            text.push_str(&new.to_text());
        }
        match result {
            Ok(()) if text.trim().is_empty() => ("Done: the commands ran and produced no output.".to_string(), false),
            Ok(()) => (text, false),
            Err(e) => (format!("{text}\nError: {e}").trim_start().to_string(), true),
        }
    }

    /// The folder the agent's files must be in, as shown to it.
    fn root(&self) -> String {
        self.session.file_root.as_ref().map(|r| r.display().to_string()).unwrap_or_default()
    }

    fn export(&mut self, path: &str, format: &str) -> Answer {
        let allowed = extensions(format);
        if allowed.is_empty() {
            return (format!("{format} is not an export format"), true);
        }
        if !std::path::Path::new(path).extension().and_then(|e| e.to_str()).is_some_and(|e| allowed.iter().any(|a| a.eq_ignore_ascii_case(e))) {
            return (format!("{path}: a {format} export must end in .{}", allowed.join(" or .")), true);
        }
        let target = match self.session.resolve(path) {
            Ok(target) => target,
            Err(e) => return (e.to_string(), true),
        };
        let title = "MK Sigma Output";
        let out = &self.session.output;
        let bytes: Result<Vec<u8>, String> = match format {
            "html" => Ok(out.to_html(title).into_bytes()),
            "pdf" => Ok(out.to_pdf(title)),
            "latex" => Ok(out.to_latex(title).into_bytes()),
            "sav" => self.session.data_sav().map_err(|e| e.to_string()),
            "dta" => self.session.data_dta().map_err(|e| e.to_string()),
            "syntax" => {
                self.session.dashboard_syntax().map(String::into_bytes).ok_or_else(|| "the data didn't come from a file (use open_data)".to_string())
            }
            "dashboard" if self.session.dashboard().is_empty() => {
                Err("the dashboard is empty: set its tiles with DASHBOARD /TILES=(analysis item) ...".into())
            }
            "dashboard" if mksigma_web::ENGINE.is_empty() => {
                let (board, wide) = self.session.dashboard_output();
                Ok(board.to_dashboard_html("MK Sigma Dashboard", self.session.filter_description().as_deref(), &wide).into_bytes())
            }
            "dashboard" => mksigma_web::page(&self.session, "MK Sigma Dashboard", mksigma_web::ENGINE).map(String::into_bytes),
            other => Err(format!("{other} is not an export format")),
        };
        match bytes.and_then(|b| mksigma_engine::write_atomic(&target, &b).map_err(|e| format!("{path}: {e}"))) {
            Ok(()) => (format!("Wrote {path}"), false),
            Err(e) => (e, true),
        }
    }
}

fn error(id: Value, code: i64, message: &str) -> String {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}).to_string()
}

/// The folder from `--root DIR`, or the working directory.
fn root_from_args() -> Result<std::path::PathBuf, String> {
    let mut args = std::env::args().skip(1);
    let mut root = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = Some(args.next().ok_or("--root needs a folder")?),
            other => return Err(format!("unknown argument {other}; usage: mksigma-mcp [--root DIR]")),
        }
    }
    let root = match root {
        Some(dir) => std::path::PathBuf::from(dir),
        None => std::env::current_dir().map_err(|e| format!("the working directory: {e}"))?,
    };
    root.canonicalize().map_err(|e| format!("{}: {e}", root.display()))
}

fn main() {
    let root = match root_from_args() {
        Ok(root) => root,
        Err(e) => {
            eprintln!("mksigma-mcp: {e}");
            std::process::exit(2);
        }
    };
    let mut server = Server::new(root);
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(answer) = server.handle(&line)
            && writeln!(stdout, "{answer}").and_then(|()| stdout.flush()).is_err()
        {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(server: &mut Server, id: u32, method: &str, params: Value) -> Value {
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string();
        serde_json::from_str(&server.handle(&line).unwrap()).unwrap()
    }

    /// A new, empty folder for one test, as the server's root.
    fn folder(test: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mksigma-mcp-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    fn tool(server: &mut Server, name: &str, arguments: Value) -> (String, bool) {
        let answer = call(server, 9, "tools/call", json!({"name": name, "arguments": arguments}));
        let result = &answer["result"];
        (result["content"][0]["text"].as_str().unwrap().to_string(), result["isError"].as_bool().unwrap())
    }

    #[test]
    fn the_protocol() {
        let mut s = Server::new(folder("protocol"));
        let init =
            call(&mut s, 1, "initialize", json!({"protocolVersion": PROTOCOL, "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}));
        assert_eq!(init["result"]["protocolVersion"], PROTOCOL);
        assert_eq!(init["result"]["serverInfo"]["name"], "mksigma");
        assert_eq!(s.handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#), None, "no answer to a notification");
        let list = call(&mut s, 2, "tools/list", json!({}));
        let names: Vec<&str> = list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["open_data", "describe_data", "run_syntax", "get_output", "export", "history"]);
        assert_eq!(call(&mut s, 3, "nope", json!({}))["error"]["code"], -32601);
        assert!(s.handle("{oops").unwrap().contains("-32700"));
    }

    #[test]
    fn an_agent_analyses_a_csv() {
        let dir = folder("csv");
        let csv = dir.join("vendas.csv");
        std::fs::write(&csv, "regiao,vendas\nNorte,10\nSul,12\nNorte,8\nSul,15\n").unwrap();
        let mut s = Server::new(dir.clone());
        let (text, err) = tool(&mut s, "open_data", json!({"path": csv.display().to_string()}));
        assert!(!err && text.contains("4 cases, 2 variables") && text.contains("- vendas (numeric, scale"), "{text}");
        // Mean of 10, 12, 8, 15.
        let (text, err) = tool(&mut s, "run_syntax", json!({"syntax": "DESCRIPTIVES vendas."}));
        assert!(!err && text.contains("11.25"), "{text}");
        let (text, err) = tool(&mut s, "run_syntax", json!({"syntax": "FREQUENCIES regiao."}));
        assert!(!err && text.contains("Norte") && !text.contains("11.25"), "only the new output: {text}");
        let (text, err) = tool(&mut s, "run_syntax", json!({"syntax": "DESCRIPTIVES nope."}));
        assert!(err && text.contains("nope"), "{text}");
        let (history, _) = tool(&mut s, "history", json!({}));
        assert!(history.starts_with("GET DATA") && history.contains("FREQUENCIES regiao."), "{history}");

        // A relative path starts from the root.
        let (text, err) = tool(&mut s, "export", json!({"path": "report.pdf", "format": "pdf"}));
        assert!(!err, "{text}");
        assert!(std::fs::read(dir.join("report.pdf")).unwrap().starts_with(b"%PDF"));
        let (text, err) = tool(&mut s, "export", json!({"path": "board.html", "format": "dashboard"}));
        assert!(err && text.contains("empty"), "{text}");
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(tool(&mut s, "get_output", json!({"format": "latex"})).0.contains("\\begin{document}"));
        assert!(tool(&mut s, "nope", json!({})).1);
    }

    #[test]
    fn files_stay_inside_the_root() {
        // Issue #26: an agent could read ~/.ssh or overwrite ~/.bashrc.
        let dir = folder("root");
        let inside = dir.join("inside");
        std::fs::create_dir_all(&inside).unwrap();
        std::fs::write(dir.join("secret.csv"), "key\n42\n").unwrap();
        std::fs::write(inside.join("data.csv"), "x\n1\n2\n").unwrap();
        let mut s = Server::new(inside.clone());
        let refused = |(text, err): (String, bool)| err && text.contains("outside the folder");

        assert!(refused(tool(&mut s, "open_data", json!({"path": dir.join("secret.csv").display().to_string()}))));
        assert!(refused(tool(&mut s, "open_data", json!({"path": "../secret.csv"}))));
        assert!(refused(tool(&mut s, "run_syntax", json!({"syntax": "GET DATA /TYPE=TXT /FILE='../secret.csv'."}))));
        assert!(refused(tool(&mut s, "run_syntax", json!({"syntax": "GET FILE='/etc/hosts'."}))));
        let (text, err) = tool(&mut s, "open_data", json!({"path": "data.csv"}));
        assert!(!err, "{text}");

        // Writes: inside the root, and only the format's own kind of file.
        assert!(refused(tool(&mut s, "export", json!({"path": "../report.html", "format": "html"}))));
        let (text, err) = tool(&mut s, "export", json!({"path": ".bashrc", "format": "latex"}));
        assert!(err && text.contains("must end in .tex"), "{text}");
        let (text, err) = tool(&mut s, "export", json!({"path": "notes.txt", "format": "html"}));
        assert!(err && text.contains(".html or .htm"), "{text}");
        let (text, err) = tool(&mut s, "run_syntax", json!({"syntax": "SAVE TRANSLATE OUTFILE='run.sh' /TYPE=STATA."}));
        assert!(err && text.contains(".dta"), "{text}");
        assert!(refused(tool(&mut s, "run_syntax", json!({"syntax": "SAVE TRANSLATE OUTFILE='../x.dta' /TYPE=STATA."}))));
        let (text, err) = tool(&mut s, "run_syntax", json!({"syntax": "SAVE TRANSLATE OUTFILE='x.dta' /TYPE=STATA."}));
        assert!(!err && inside.join("x.dta").exists(), "{text}");
        assert!(!dir.join("report.html").exists() && !inside.join(".bashrc").exists() && !inside.join("run.sh").exists());

        // A symbolic link inside the root to a file outside it is refused too.
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.join("secret.csv"), inside.join("link.csv")).unwrap();
            assert!(refused(tool(&mut s, "open_data", json!({"path": "link.csv"}))));
        }
        let (text, _) = tool(&mut s, "describe_data", json!({}));
        assert!(text.contains("- x (numeric"), "the data stayed the one opened: {text}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_instructions_name_the_root() {
        let dir = folder("instructions");
        let mut s = Server::new(dir.clone());
        let init = call(&mut s, 1, "initialize", json!({}));
        assert!(init["result"]["instructions"].as_str().unwrap().contains(&dir.display().to_string()));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
