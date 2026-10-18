//! `SLICE`, MK Sigma's slicers (as in Power BI): every analysis in the Output shows only the
//! cases a slicer keeps. A view of the data, not a change to it.
//!
//! `SLICE var (value value...)` keeps the cases with one of the values; `SLICE var (low THRU
//! high)` the cases in the range (`LO`/`LOWEST` and `HI`/`HIGHEST` for open ends); `SLICE var
//! OFF` removes that variable's slicer and `SLICE OFF` every slicer. One slicer per variable:
//! slicing a variable again replaces its slicer. Missing values are never kept.

use crate::dictionary::Literal;
use crate::lexer::Token;
use crate::{Command, SyntaxError, is_keyword};

#[derive(Debug, Clone, PartialEq)]
pub enum SliceSpec {
    Values(Vec<Literal>),
    Range { low: Option<f64>, high: Option<f64> },
    Off,
}

const CMD: &str = "SLICE";

fn bound(word: &str, open: &[&str]) -> Result<Option<f64>, SyntaxError> {
    if open.iter().any(|k| is_keyword(word, k)) {
        return Ok(None);
    }
    word.parse::<f64>().map(Some).map_err(|_| SyntaxError::Expected { command: CMD, expected: "a number or LO/HI in the range" })
}

pub(crate) fn slice(rest: &[Token]) -> Result<Command, SyntaxError> {
    let expected = SyntaxError::Expected { command: CMD, expected: "var (values), var (low THRU high), var OFF or OFF" };
    match rest {
        [Token::Word(off)] if off.eq_ignore_ascii_case("OFF") => Ok(Command::Slice(None)),
        [Token::Word(var), Token::Word(off)] if off.eq_ignore_ascii_case("OFF") => Ok(Command::Slice(Some((var.clone(), SliceSpec::Off)))),
        [Token::Word(var), Token::LParen, inner @ .., Token::RParen] => {
            let spec = match inner {
                [Token::Word(low), Token::Word(thru), Token::Word(high)] if is_keyword(thru, "THRU") => {
                    let (low, high) = (bound(low, &["LO", "LOWEST"])?, bound(high, &["HI", "HIGHEST"])?);
                    if let (Some(l), Some(h)) = (low, high)
                        && l > h
                    {
                        return Err(SyntaxError::Expected { command: CMD, expected: "a range whose low end is not above its high end" });
                    }
                    SliceSpec::Range { low, high }
                }
                values => {
                    let literals = values
                        .iter()
                        .filter(|t| **t != Token::Comma)
                        .map(|t| match t {
                            Token::Str(s) => Some(Literal::Str(s.clone())),
                            Token::Word(w) => w.parse().ok().map(Literal::Num),
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()
                        .filter(|v| !v.is_empty())
                        .ok_or(expected)?;
                    SliceSpec::Values(literals)
                }
            };
            Ok(Command::Slice(Some((var.clone(), spec))))
        }
        _ => Err(expected),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn one(src: &str) -> Command {
        let mut c = parse(src).unwrap();
        c.remove(0)
    }

    #[test]
    fn parses_slicers() {
        assert_eq!(one("SLICE grupo (1 2)."), Command::Slice(Some(("grupo".into(), SliceSpec::Values(vec![Literal::Num(1.0), Literal::Num(2.0)])))));
        assert_eq!(
            one("SLICE uf ('SP', 'RJ')."),
            Command::Slice(Some(("uf".into(), SliceSpec::Values(vec![Literal::Str("SP".into()), Literal::Str("RJ".into())]))))
        );
        assert_eq!(one("SLICE renda (2000 THRU HI)."), Command::Slice(Some(("renda".into(), SliceSpec::Range { low: Some(2000.0), high: None }))));
        assert_eq!(one("SLICE nota (LO THRU -1.5)."), Command::Slice(Some(("nota".into(), SliceSpec::Range { low: None, high: Some(-1.5) }))));
        assert_eq!(one("SLICE grupo OFF."), Command::Slice(Some(("grupo".into(), SliceSpec::Off))));
        assert_eq!(one("SLICE OFF."), Command::Slice(None));
        for bad in ["SLICE.", "SLICE grupo.", "SLICE grupo ().", "SLICE x (5 THRU 1).", "SLICE x (a b)."] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}
