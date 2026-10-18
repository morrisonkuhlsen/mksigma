//! `OUTPUT STYLE`: how the Output looks on screen and in every export (an MK Sigma command).
//!
//! `OUTPUT STYLE [/FONT=DEFAULT|SANS|SERIF|MONO] [/SIZE=points] [/PALETTE=DEFAULT|INDIGO|GRAY|
//! BLUE|GREEN|WARM] [/DECIMALS=AUTO|n]`. What isn't written stays as it is.

use crate::lexer::Token;
use crate::{Command, SyntaxError, is_keyword, unsupported};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct StyleSpec {
    /// One of `FONTS`, upper case.
    pub font: Option<String>,
    pub size: Option<u8>,
    /// One of `PALETTES`, upper case.
    pub palette: Option<String>,
    /// `Some(None)` is `AUTO`.
    pub decimals: Option<Option<u8>>,
}

pub const FONTS: [&str; 4] = ["DEFAULT", "SANS", "SERIF", "MONO"];
pub const PALETTES: [&str; 6] = ["DEFAULT", "INDIGO", "GRAY", "BLUE", "GREEN", "WARM"];
const CMD: &str = "OUTPUT STYLE";

fn one_of(word: &str, list: &[&str], expected: &'static str) -> Result<String, SyntaxError> {
    let up = word.to_ascii_uppercase();
    // GREY is GRAY.
    let up = if up == "GREY" { "GRAY".to_string() } else { up };
    list.iter().find(|k| **k == up).map(|k| k.to_string()).ok_or(SyntaxError::Expected { command: CMD, expected })
}

pub(crate) fn output_style(rest: &[Token]) -> Result<Command, SyntaxError> {
    let mut spec = StyleSpec::default();
    for sub in rest.split(|t| *t == Token::Slash).filter(|s| !s.is_empty()) {
        let (name, value) = match sub {
            [Token::Word(name), Token::Equals, Token::Word(value)] | [Token::Word(name), Token::Word(value)] => (name.as_str(), value.as_str()),
            _ => return Err(SyntaxError::Expected { command: CMD, expected: "/FONT=, /SIZE=, /PALETTE= or /DECIMALS= with one value" }),
        };
        match name {
            n if is_keyword(n, "FONT") => spec.font = Some(one_of(value, &FONTS, "/FONT=DEFAULT, SANS, SERIF or MONO")?),
            n if is_keyword(n, "PALETTE") => {
                spec.palette = Some(one_of(value, &PALETTES, "/PALETTE=DEFAULT, INDIGO, GRAY, BLUE, GREEN or WARM")?);
            }
            n if is_keyword(n, "SIZE") => {
                let size = value.parse::<u8>().ok().filter(|s| (6..=24).contains(s));
                spec.size = Some(size.ok_or(SyntaxError::Expected { command: CMD, expected: "/SIZE=6 to 24 (points)" })?);
            }
            n if is_keyword(n, "DECIMALS") => {
                spec.decimals = Some(if value.eq_ignore_ascii_case("AUTO") {
                    None
                } else {
                    Some(
                        value
                            .parse::<u8>()
                            .ok()
                            .filter(|d| *d <= 10)
                            .ok_or(SyntaxError::Expected { command: CMD, expected: "/DECIMALS=AUTO or 0 to 10" })?,
                    )
                });
            }
            n => return Err(unsupported(CMD, &format!("/{n}"))),
        }
    }
    Ok(Command::OutputStyle(spec))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn parses_output_style() {
        assert_eq!(
            parse("OUTPUT STYLE /FONT=serif /SIZE=12 /PALETTE=grey /DECIMALS=3.").unwrap(),
            [Command::OutputStyle(StyleSpec { font: Some("SERIF".into()), size: Some(12), palette: Some("GRAY".into()), decimals: Some(Some(3)) })]
        );
        assert_eq!(
            parse("OUTPUT STYLE /DECIMALS=AUTO.").unwrap(),
            [Command::OutputStyle(StyleSpec { decimals: Some(None), ..StyleSpec::default() })]
        );
        for bad in
            ["OUTPUT STYLE /FONT=comic.", "OUTPUT STYLE /SIZE=99.", "OUTPUT STYLE /DECIMALS=-1.", "OUTPUT STYLE /COLOR=red.", "OUTPUT STYLE /FONT."]
        {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}
