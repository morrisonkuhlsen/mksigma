//! `GET DATA /TYPE=TXT`: reads delimited text into a new active dataset.
//!
//! `GET DATA /TYPE=TXT /FILE='data.csv' [/ENCODING='UTF8'] [/ARRANGEMENT=DELIMITED]
//! [/DELIMITERS=",;"] [/QUALIFIER='"'] [/FIRSTCASE=n] [/READNAMES=ON|OFF] [/IMPORTCASE=ALL]
//! [/DELCASE=LINE] [/VARIABLES=name format ...]`. As in SPSS, `/VARIABLES` names the columns
//! and gives their formats. MK Sigma also reads text without it: the names come from the first
//! record (`/READNAMES=ON`, the default then) and the types are inferred; without
//! `/DELIMITERS` the delimiter is detected. `"\t"` in `/DELIMITERS` is a tab.

use crate::lexer::Token;
use crate::{Command, SyntaxError, is_keyword, unsupported};

#[derive(Debug, Clone, PartialEq)]
pub struct GetData {
    pub file: String,
    /// Characters any of which separates fields; empty to detect.
    pub delimiters: Vec<char>,
    /// `None` when fields are never enclosed (`/QUALIFIER=''`).
    pub qualifier: Option<char>,
    pub first_case: Option<usize>,
    /// `None` when not written: on without `/VARIABLES`, off with it.
    pub read_names: Option<bool>,
    pub encoding: Option<String>,
    /// Names and formats as written (`F8.2`, `A20`).
    pub variables: Vec<(String, String)>,
}

const CMD: &str = "GET DATA";

/// `"\t"` is a tab and `"\\"` a backslash, as SPSS reads them in `/DELIMITERS`.
fn delimiter_chars(text: &str) -> Vec<char> {
    let mut out = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match (c, chars.clone().next()) {
            ('\\', Some('t')) => {
                chars.next();
                out.push('\t');
            }
            ('\\', Some('\\')) => {
                chars.next();
                out.push('\\');
            }
            _ => out.push(c),
        }
    }
    out.dedup();
    out
}

pub(crate) fn get_data(rest: &[Token]) -> Result<Command, SyntaxError> {
    let mut file = None;
    let mut spec = GetData {
        file: String::new(),
        delimiters: Vec::new(),
        qualifier: Some('"'),
        first_case: None,
        read_names: None,
        encoding: None,
        variables: Vec::new(),
    };
    let mut text = false;
    for sub in rest.split(|t| *t == Token::Slash).filter(|s| !s.is_empty()) {
        let (name, values) = match sub {
            [Token::Word(name), Token::Equals, values @ ..] | [Token::Word(name), values @ ..] => (name.as_str(), values),
            _ => return Err(SyntaxError::Expected { command: CMD, expected: "a subcommand such as /FILE='data.csv'" }),
        };
        let word = |expected: &'static str| match values {
            [Token::Word(w)] => Ok(w.as_str()),
            _ => Err(SyntaxError::Expected { command: CMD, expected }),
        };
        let string = |expected: &'static str| match values {
            [Token::Str(s)] => Ok(s.clone()),
            _ => Err(SyntaxError::Expected { command: CMD, expected }),
        };
        match name {
            n if is_keyword(n, "TYPE") => {
                let kind = word("/TYPE=TXT")?;
                if !kind.eq_ignore_ascii_case("TXT") {
                    return Err(unsupported(CMD, &format!("/TYPE={kind}")));
                }
                text = true;
            }
            n if is_keyword(n, "FILE") => file = Some(string("/FILE='path'")?),
            n if is_keyword(n, "ENCODING") => spec.encoding = Some(string("/ENCODING='UTF8'")?),
            n if is_keyword(n, "ARRANGEMENT") => {
                let a = word("/ARRANGEMENT=DELIMITED")?;
                if !is_keyword(a, "DELIMITED") {
                    return Err(unsupported(CMD, &format!("/ARRANGEMENT={a}")));
                }
            }
            n if is_keyword(n, "DELIMITERS") => spec.delimiters = delimiter_chars(&string("/DELIMITERS=\",\"")?),
            n if is_keyword(n, "QUALIFIER") => {
                let q = string("/QUALIFIER='\"'")?;
                let mut chars = q.chars();
                spec.qualifier = match (chars.next(), chars.next()) {
                    (None, _) => None,
                    (Some(c), None) => Some(c),
                    _ => return Err(SyntaxError::Expected { command: CMD, expected: "one character in /QUALIFIER" }),
                };
            }
            n if is_keyword(n, "FIRSTCASE") => {
                let n = word("/FIRSTCASE=n")?.parse::<usize>().ok().filter(|n| *n >= 1);
                spec.first_case = Some(n.ok_or(SyntaxError::Expected { command: CMD, expected: "a record number from 1 in /FIRSTCASE" })?);
            }
            n if is_keyword(n, "READNAMES") => {
                spec.read_names = Some(match word("/READNAMES=ON or OFF")? {
                    w if w.eq_ignore_ascii_case("ON") => true,
                    w if w.eq_ignore_ascii_case("OFF") => false,
                    _ => return Err(SyntaxError::Expected { command: CMD, expected: "/READNAMES=ON or OFF" }),
                });
            }
            n if is_keyword(n, "IMPORTCASE") => {
                let all = word("/IMPORTCASE=ALL")?;
                if !all.eq_ignore_ascii_case("ALL") {
                    return Err(unsupported(CMD, &format!("/IMPORTCASE={all}")));
                }
            }
            n if is_keyword(n, "DELCASE") => {
                let line = word("/DELCASE=LINE")?;
                if !line.eq_ignore_ascii_case("LINE") {
                    return Err(unsupported(CMD, &format!("/DELCASE={line}")));
                }
            }
            n if is_keyword(n, "VARIABLES") => {
                let words: Vec<&str> = values.iter().filter_map(|t| if let Token::Word(w) = t { Some(w.as_str()) } else { None }).collect();
                if words.len() != values.len() || words.is_empty() || !words.len().is_multiple_of(2) {
                    return Err(SyntaxError::Expected { command: CMD, expected: "/VARIABLES=name format pairs, such as id F4.0 name A20" });
                }
                spec.variables = words
                    .chunks(2)
                    .filter_map(|p| match p {
                        [name, format] => Some((name.to_string(), format.to_string())),
                        _ => None,
                    })
                    .collect();
            }
            n => return Err(unsupported(CMD, &format!("/{n}"))),
        }
    }
    if !text {
        return Err(SyntaxError::Expected { command: CMD, expected: "/TYPE=TXT" });
    }
    spec.file = file.ok_or(SyntaxError::Expected { command: CMD, expected: "/FILE='path'" })?;
    Ok(Command::GetData(spec))
}

/// `GET FILE='data.sav'`: a system file as the new active dataset.
pub(crate) fn get_file(rest: &[Token]) -> Result<Command, SyntaxError> {
    match rest {
        [Token::Word(file), Token::Equals, Token::Str(path)] | [Token::Word(file), Token::Str(path)] if is_keyword(file, "FILE") => {
            Ok(Command::GetFile(path.clone()))
        }
        _ => Err(SyntaxError::Expected { command: "GET", expected: "FILE='path'" }),
    }
}

/// `GET STATA FILE='data.dta'`: a Stata file as the new active dataset. `/ENCODING` is
/// accepted and ignored: a file says how its text is encoded.
pub(crate) fn get_stata(rest: &[Token]) -> Result<Command, SyntaxError> {
    let path = match rest {
        [Token::Word(file), Token::Equals, Token::Str(path), ..] | [Token::Word(file), Token::Str(path), ..] if is_keyword(file, "FILE") => path,
        _ => return Err(SyntaxError::Expected { command: "GET STATA", expected: "FILE='path'" }),
    };
    let tail = rest.iter().skip_while(|t| !matches!(t, Token::Str(_))).skip(1).collect::<Vec<_>>();
    match tail.as_slice() {
        [] => {}
        [Token::Slash, Token::Word(e), Token::Equals, Token::Str(_)] | [Token::Slash, Token::Word(e), Token::Str(_)] if is_keyword(e, "ENCODING") => {
        }
        _ => return Err(SyntaxError::Expected { command: "GET STATA", expected: "FILE='path' [/ENCODING='name']" }),
    }
    Ok(Command::GetStata(path.clone()))
}

/// `GET SAS DATA='data.xpt'`: a SAS transport file as the new active dataset. `/DSET`, which
/// names a member, isn't taken yet: the first member is read.
pub(crate) fn get_sas(rest: &[Token]) -> Result<Command, SyntaxError> {
    match rest {
        [Token::Word(data), Token::Equals, Token::Str(path)] | [Token::Word(data), Token::Str(path)] if is_keyword(data, "DATA") => {
            Ok(Command::GetSas(path.clone()))
        }
        _ => Err(SyntaxError::Expected { command: "GET SAS", expected: "DATA='path'" }),
    }
}

/// `SAVE TRANSLATE OUTFILE='data.dta' /TYPE=STATA [/VERSION=n] [/EDITION=...] [/REPLACE]`:
/// the active dataset written as a Stata file. Stata is the one type so far; the version and
/// edition are accepted and the file is always format 118 (Stata 14 and later).
pub(crate) fn save_translate(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "SAVE TRANSLATE";
    let (mut file, mut stata, mut replace) = (None, false, false);
    for sub in rest.split(|t| *t == Token::Slash).filter(|s| !s.is_empty()) {
        let (name, value) = match sub {
            [Token::Word(n), Token::Equals, v @ ..] | [Token::Word(n), v @ ..] => (n.as_str(), v),
            _ => return Err(SyntaxError::Expected { command: CMD, expected: "OUTFILE='path' /TYPE=STATA" }),
        };
        match (name.to_ascii_uppercase().as_str(), value) {
            ("OUTFILE", [Token::Str(path)]) => file = Some(path.clone()),
            ("TYPE", [Token::Word(t)]) if is_keyword(t, "STATA") => stata = true,
            ("TYPE", _) => return Err(unsupported(CMD, "/TYPE")),
            ("VERSION" | "EDITION", _) => {}
            ("REPLACE", []) => replace = true,
            (n, _) => return Err(unsupported(CMD, &format!("/{n}"))),
        }
    }
    if !stata {
        return Err(SyntaxError::Expected { command: CMD, expected: "/TYPE=STATA" });
    }
    let file = file.ok_or(SyntaxError::Expected { command: CMD, expected: "OUTFILE='path'" })?;
    Ok(Command::SaveStata { file, replace })
}

/// `DASHBOARD /TILES=(analysis item [WIDE]) ...`: the dashboard's tiles, in order; a `WIDE`
/// tile takes a whole row. An analysis is
/// counted from 1 among the commands with output since the Output was last cleared, and an
/// item among that command's output items (notes included), in order.
pub(crate) fn dashboard(rest: &[Token]) -> Result<Command, SyntaxError> {
    const CMD: &str = "DASHBOARD";
    let expected = SyntaxError::Expected { command: CMD, expected: "/TILES=(analysis item [WIDE]) ..., numbers from 1" };
    let list = match rest {
        [Token::Slash, Token::Word(w), Token::Equals, list @ ..] | [Token::Slash, Token::Word(w), list @ ..] if is_keyword(w, "TILES") => list,
        _ => return Err(expected),
    };
    let mut tiles = Vec::new();
    let mut rest = list;
    while !rest.is_empty() {
        match rest {
            [Token::LParen, Token::Word(a), Token::Comma, Token::Word(b), Token::RParen, after @ ..]
            | [Token::LParen, Token::Word(a), Token::Word(b), Token::RParen, after @ ..] => {
                let n = |w: &str| w.parse::<usize>().ok().filter(|n| *n >= 1);
                tiles.push((n(a).ok_or(expected.clone())?, n(b).ok_or(expected.clone())?, false));
                rest = after;
            }
            [Token::LParen, Token::Word(a), Token::Word(b), Token::Word(wide), Token::RParen, after @ ..] if wide.eq_ignore_ascii_case("WIDE") => {
                let n = |w: &str| w.parse::<usize>().ok().filter(|n| *n >= 1);
                tiles.push((n(a).ok_or(expected.clone())?, n(b).ok_or(expected.clone())?, true));
                rest = after;
            }
            _ => return Err(expected),
        }
    }
    Ok(Command::Dashboard(tiles))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn one(src: &str) -> GetData {
        match parse(src).unwrap().as_slice() {
            [Command::GetData(g)] => g.clone(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_get_data() {
        let g = one("GET DATA /TYPE=TXT /FILE='dados.csv'.");
        assert_eq!((g.file.as_str(), g.delimiters.is_empty(), g.qualifier, g.read_names), ("dados.csv", true, Some('"'), None));
        let g = one("GET DATA /TYPE=TXT /FILE='C:\\dados\\x.txt' /ENCODING='UTF8' /ARRANGEMENT=DELIMITED /DELIMITERS=\"\\t;\" \
             /QUALIFIER='' /FIRSTCASE=3 /READNAMES=OFF /IMPORTCASE=ALL /DELCASE=LINE /VARIABLES=id F4.0 nome A20.");
        assert_eq!(g.file, "C:\\dados\\x.txt");
        assert_eq!(g.delimiters, ['\t', ';']);
        assert_eq!((g.qualifier, g.first_case, g.read_names), (None, Some(3), Some(false)));
        assert_eq!(g.variables, [("id".to_string(), "F4.0".to_string()), ("nome".to_string(), "A20".to_string())]);
        assert_eq!(g.encoding.as_deref(), Some("UTF8"));
    }

    #[test]
    fn get_file_and_dashboard() {
        assert_eq!(parse("GET FILE='/dados/turma.sav'.").unwrap(), [Command::GetFile("/dados/turma.sav".into())]);
        assert_eq!(parse("GET STATA FILE='/dados/pnad.dta'.").unwrap(), [Command::GetStata("/dados/pnad.dta".into())]);
        assert_eq!(parse("GET SAS DATA='/dados/adsl.xpt'.").unwrap(), [Command::GetSas("/dados/adsl.xpt".into())]);
        assert!(parse("GET SAS FILE='x.xpt'.").is_err());
        assert_eq!(parse("get stata file 'a.dta' /encoding='UTF8'.").unwrap(), [Command::GetStata("a.dta".into())]);
        assert!(parse("GET STATA 'a.dta'.").is_err() && parse("GET STATA FILE='a.dta' /KEEP=x.").is_err());
        assert_eq!(
            parse("SAVE TRANSLATE OUTFILE='/dados/turma.dta' /TYPE=STATA /VERSION=14 /REPLACE.").unwrap(),
            [Command::SaveStata { file: "/dados/turma.dta".into(), replace: true }]
        );
        assert_eq!(parse("save translate /type=stata /outfile 'x.dta'.").unwrap(), [Command::SaveStata { file: "x.dta".into(), replace: false }]);
        assert!(parse("SAVE TRANSLATE OUTFILE='x.xlsx' /TYPE=XLS.").is_err() && parse("SAVE TRANSLATE /TYPE=STATA.").is_err());
        assert_eq!(
            parse("DASHBOARD /TILES=(3 1) (1, 2) (2 1 wide).").unwrap(),
            [Command::Dashboard(vec![(3, 1, false), (1, 2, false), (2, 1, true)])]
        );
        assert_eq!(parse("DASHBOARD /TILES.").unwrap(), [Command::Dashboard(Vec::new())], "an empty dashboard");
        for bad in ["GET FILE.", "GET FILE=x.", "DASHBOARD (1 1).", "DASHBOARD /TILES=(0 1).", "DASHBOARD /TILES=(1).", "DASHBOARD /TILES=1 1."] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn rejects_what_it_cannot_read() {
        for bad in [
            "GET DATA /FILE='x.csv'.",
            "GET DATA /TYPE=TXT.",
            "GET DATA /TYPE=XLSX /FILE='x.xlsx'.",
            "GET DATA /TYPE=TXT /FILE='x' /ARRANGEMENT=FIXED.",
            "GET DATA /TYPE=TXT /FILE='x' /FIRSTCASE=0.",
            "GET DATA /TYPE=TXT /FILE='x' /VARIABLES=a.",
            "GET DATA /TYPE=TXT /FILE='x' /QUALIFIER='ab'.",
            "GET DATA /TYPE=TXT /FILE='x' /SHEET=name 'a'.",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}
