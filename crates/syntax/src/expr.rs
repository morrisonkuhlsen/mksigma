//! Transformation expressions (`COMPUTE`, `IF`, `SELECT IF`). Precedence, from loosest to
//! tightest: `OR`, `AND`, `NOT`, comparisons, `+ -`, `* /`, unary `-`, `**` (right-associative,
//! so `-2**2` is −4 and `2**-1` is 0.5). Keywords `AND OR NOT EQ NE LT LE GT GE` and the symbols
//! `& | ~ = <> ~= < <= > >=` are interchangeable.

use crate::SyntaxError;
use crate::lexer::Token;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Str(String),
    Var(String),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// A function call: upper-cased name without any `.n` suffix, the suffix (minimum valid
    /// arguments, as in `MEAN.2`), and the arguments.
    Call(String, Option<usize>, Vec<Expr>),
    /// `first TO last` among a function's arguments (`SUM(q1 TO q5)`): the variables from
    /// `first` to `last` in the dictionary's order (#36).
    Range(String, String),
}

const ERR: SyntaxError = SyntaxError::Expected { command: "expression", expected: "a value, variable, function or (" };

/// The deepest expression tree: every operator and function call is a level, so `a + b + c`
/// is three deep. Evaluating and dropping a tree recurses once per level.
const MAX_DEPTH: usize = 500;
/// The deepest the parser itself goes: parentheses, function arguments, `NOT`, `-` and `**`
/// each take a level or more.
const MAX_NESTING: usize = 150;

/// An expression and the depth of its tree.
type Parsed = Result<(Expr, usize), SyntaxError>;

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    nesting: usize,
}

fn is_name(word: &str) -> bool {
    word.chars().next().is_some_and(|c| !(c.is_ascii_digit() || c == '-' || c == '.'))
}

/// A node over children `depth` deep, refused past `MAX_DEPTH`.
fn node(e: Expr, depth: usize) -> Parsed {
    let depth = depth.saturating_add(1);
    if depth > MAX_DEPTH { Err(SyntaxError::TooDeep) } else { Ok((e, depth)) }
}

fn binary(op: BinOp, (left, l): (Expr, usize), (right, r): (Expr, usize)) -> Parsed {
    node(Expr::Binary(op, Box::new(left), Box::new(right)), l.max(r))
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos);
        self.pos += 1;
        t
    }

    fn word_is(&self, keyword: &str) -> bool {
        matches!(self.peek(), Some(Token::Word(w)) if w.eq_ignore_ascii_case(keyword))
    }

    /// Runs `f` one level deeper, refused past `MAX_NESTING`.
    fn nested(&mut self, f: impl FnOnce(&mut Self) -> Parsed) -> Parsed {
        if self.nesting >= MAX_NESTING {
            return Err(SyntaxError::TooDeep);
        }
        self.nesting += 1;
        let result = f(self);
        self.nesting -= 1;
        result
    }

    fn or(&mut self) -> Parsed {
        self.nested(|p| {
            let mut left = p.and()?;
            while matches!(p.peek(), Some(Token::Or)) || p.word_is("OR") {
                p.pos += 1;
                left = binary(BinOp::Or, left, p.and()?)?;
            }
            Ok(left)
        })
    }

    fn and(&mut self) -> Parsed {
        let mut left = self.not()?;
        while matches!(self.peek(), Some(Token::And)) || self.word_is("AND") {
            self.pos += 1;
            left = binary(BinOp::And, left, self.not()?)?;
        }
        Ok(left)
    }

    fn not(&mut self) -> Parsed {
        if matches!(self.peek(), Some(Token::Not)) || self.word_is("NOT") {
            self.pos += 1;
            let (inner, depth) = self.nested(Self::not)?;
            return node(Expr::Not(Box::new(inner)), depth);
        }
        self.comparison()
    }

    fn comparison_op(&self) -> Option<BinOp> {
        Some(match self.peek()? {
            Token::Equals => BinOp::Eq,
            Token::Ne => BinOp::Ne,
            Token::Lt => BinOp::Lt,
            Token::Le => BinOp::Le,
            Token::Gt => BinOp::Gt,
            Token::Ge => BinOp::Ge,
            Token::Word(w) => match w.to_ascii_uppercase().as_str() {
                "EQ" => BinOp::Eq,
                "NE" => BinOp::Ne,
                "LT" => BinOp::Lt,
                "LE" => BinOp::Le,
                "GT" => BinOp::Gt,
                "GE" => BinOp::Ge,
                _ => return None,
            },
            _ => return None,
        })
    }

    fn comparison(&mut self) -> Parsed {
        let left = self.additive()?;
        let Some(op) = self.comparison_op() else { return Ok(left) };
        self.pos += 1;
        binary(op, left, self.additive()?)
    }

    fn additive(&mut self) -> Parsed {
        let mut left = self.multiplicative()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                _ => return Ok(left),
            };
            self.pos += 1;
            left = binary(op, left, self.multiplicative()?)?;
        }
    }

    fn multiplicative(&mut self) -> Parsed {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                _ => return Ok(left),
            };
            self.pos += 1;
            left = binary(op, left, self.unary()?)?;
        }
    }

    fn unary(&mut self) -> Parsed {
        self.nested(|p| {
            if matches!(p.peek(), Some(Token::Minus)) {
                p.pos += 1;
                let (inner, depth) = p.unary()?;
                return node(Expr::Neg(Box::new(inner)), depth);
            }
            p.power()
        })
    }

    fn power(&mut self) -> Parsed {
        let base = self.primary()?;
        if matches!(self.peek(), Some(Token::Power)) {
            self.pos += 1;
            return binary(BinOp::Pow, base, self.unary()?);
        }
        Ok(base)
    }

    fn primary(&mut self) -> Parsed {
        match self.next().cloned() {
            Some(Token::Str(s)) => Ok((Expr::Str(s), 1)),
            Some(Token::LParen) => {
                let inner = self.or()?;
                match self.next() {
                    Some(Token::RParen) => Ok(inner),
                    _ => Err(SyntaxError::Expected { command: "expression", expected: ")" }),
                }
            }
            // A number that starts with its sign: `-2**2` is still −(2²).
            Some(Token::Word(w)) if !is_name(&w) => {
                let x: f64 = w.parse().map_err(|_| SyntaxError::Expected { command: "expression", expected: "a number" })?;
                if x < 0.0 && matches!(self.peek(), Some(Token::Power)) {
                    self.pos += 1;
                    let power = binary(BinOp::Pow, (Expr::Num(-x), 1), self.unary()?)?;
                    return node(Expr::Neg(Box::new(power.0)), power.1);
                }
                Ok((Expr::Num(x), 1))
            }
            Some(Token::Word(w)) if matches!(self.peek(), Some(Token::LParen)) => {
                self.pos += 1;
                let mut args = Vec::new();
                let mut depth = 0;
                if !matches!(self.peek(), Some(Token::RParen)) {
                    loop {
                        let (arg, d) = self.or()?;
                        // `q1 TO q5`: a range of variables, expanded when the call is compiled.
                        let arg = match (arg, self.peek(), self.tokens.get(self.pos + 1)) {
                            (Expr::Var(first), Some(Token::Word(to)), Some(Token::Word(last))) if to.eq_ignore_ascii_case("TO") && is_name(last) => {
                                let last = last.clone();
                                self.pos += 2;
                                Expr::Range(first, last)
                            }
                            (arg, ..) => arg,
                        };
                        args.push(arg);
                        depth = depth.max(d);
                        match self.next() {
                            Some(Token::Comma) => {}
                            Some(Token::RParen) => break,
                            _ => return Err(SyntaxError::Expected { command: "expression", expected: ", or ) in a function call" }),
                        }
                    }
                } else {
                    self.pos += 1;
                }
                let upper = w.to_ascii_uppercase();
                // `MEAN.2`: at least two valid arguments. `CHAR.SUBSTR` keeps its dot.
                let (name, min) = match upper.rsplit_once('.') {
                    Some((name, n)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => (name.to_string(), n.parse().ok()),
                    _ => (upper, None),
                };
                node(Expr::Call(name, min, args), depth)
            }
            Some(Token::Word(w)) => Ok((Expr::Var(w), 1)),
            _ => Err(ERR),
        }
    }
}

/// Parses a whole token slice as one expression.
pub(crate) fn parse(tokens: &[Token]) -> Result<Expr, SyntaxError> {
    let (e, rest) = prefix(tokens)?;
    if !rest.is_empty() {
        return Err(SyntaxError::Expected { command: "expression", expected: "an operator or the end of the expression" });
    }
    Ok(e)
}

/// The longest expression `tokens` starts with, and the tokens after it (`IF`'s condition, then
/// its assignment).
pub(crate) fn prefix(tokens: &[Token]) -> Result<(Expr, &[Token]), SyntaxError> {
    let mut p = Parser { tokens, pos: 0, nesting: 0 };
    let (e, _) = p.or()?;
    Ok((e, tokens.get(p.pos..).unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::commands;

    fn expr(src: &str) -> Expr {
        let lexed = commands(src).unwrap();
        parse(&lexed[0].tokens).unwrap()
    }

    fn num(x: f64) -> Box<Expr> {
        Box::new(Expr::Num(x))
    }

    fn var(n: &str) -> Box<Expr> {
        Box::new(Expr::Var(n.into()))
    }

    #[test]
    fn precedence() {
        use BinOp::*;
        assert_eq!(expr("1 + 2 * 3"), Expr::Binary(Add, num(1.0), Box::new(Expr::Binary(Mul, num(2.0), num(3.0)))));
        assert_eq!(expr("a - b - c"), Expr::Binary(Sub, Box::new(Expr::Binary(Sub, var("a"), var("b"))), var("c")));
        assert_eq!(expr("-2**2"), Expr::Neg(Box::new(Expr::Binary(Pow, num(2.0), num(2.0)))));
        assert_eq!(expr("- x ** 2"), Expr::Neg(Box::new(Expr::Binary(Pow, var("x"), num(2.0)))));
        assert_eq!(expr("2 ** 3 ** 2"), Expr::Binary(Pow, num(2.0), Box::new(Expr::Binary(Pow, num(3.0), num(2.0)))));
        assert_eq!(expr("2 ** -1"), Expr::Binary(Pow, num(2.0), num(-1.0)));
        assert_eq!(
            expr("NOT a = 1 AND b > 2 OR c"),
            Expr::Binary(
                Or,
                Box::new(Expr::Binary(
                    And,
                    Box::new(Expr::Not(Box::new(Expr::Binary(Eq, var("a"), num(1.0))))),
                    Box::new(Expr::Binary(Gt, var("b"), num(2.0)))
                )),
                var("c")
            )
        );
        assert_eq!(expr("a GE 1 & ~(b LT 2)"), expr("a >= 1 AND NOT (b < 2)"));
    }

    #[test]
    fn functions() {
        assert_eq!(
            expr("MEAN.2(a, b, c)"),
            Expr::Call("MEAN".into(), Some(2), vec![Expr::Var("a".into()), Expr::Var("b".into()), Expr::Var("c".into())])
        );
        assert_eq!(expr("char.substr(s, 2)"), Expr::Call("CHAR.SUBSTR".into(), None, vec![Expr::Var("s".into()), Expr::Num(2.0)]));
        assert_eq!(expr("rnd(x)"), Expr::Call("RND".into(), None, vec![Expr::Var("x".into())]));
        assert_eq!(expr("concat('a', \"b\")"), Expr::Call("CONCAT".into(), None, vec![Expr::Str("a".into()), Expr::Str("b".into())]));
        // A range of variables as an argument (#36).
        assert_eq!(
            expr("SUM(q1 TO q5, extra)"),
            Expr::Call("SUM".into(), None, vec![Expr::Range("q1".into(), "q5".into()), Expr::Var("extra".into())])
        );
        assert_eq!(expr("mean.3(a to c)"), Expr::Call("MEAN".into(), Some(3), vec![Expr::Range("a".into(), "c".into())]));
        let bad = |src: &str| parse(&commands(src).unwrap()[0].tokens).is_err();
        assert!(bad("SUM(1 TO 5)") && bad("SUM(a TO)") && bad("a TO b"), "TO joins two names, inside a call");
    }

    #[test]
    fn errors() {
        let bad = |src: &str| parse(&commands(src).unwrap()[0].tokens).is_err();
        assert!(bad("1 +"));
        assert!(bad("(1 + 2"));
        assert!(bad("f(1 2)"));
        assert!(bad("1 2"));
        assert!(bad("a = = b"));
    }

    /// Each shape that used to overflow the stack (#20) is an error, and a long but reasonable
    /// expression still parses.
    #[test]
    fn deep_expressions_are_refused_not_a_crash() {
        let too_deep = |src: String| {
            let lexed = commands(&src).unwrap();
            assert_eq!(parse(&lexed[0].tokens), Err(SyntaxError::TooDeep), "{}", &src[..40.min(src.len())]);
        };
        let n = 20_000;
        too_deep(format!("{}1{}", "(".repeat(n), ")".repeat(n)));
        too_deep(format!("{}1", "~".repeat(n)));
        too_deep(format!("{}1", "NOT ".repeat(n)));
        too_deep(format!("{}1", "- ".repeat(n)));
        too_deep(format!("2{}", "**2".repeat(n)));
        too_deep(format!("{}1{}", "abs(".repeat(n), ")".repeat(n)));
        too_deep((0..n).map(|i| format!("q{i}")).collect::<Vec<_>>().join(" + "));
        too_deep((0..n).map(|i| format!("q{i} > 0")).collect::<Vec<_>>().join(" AND "));
        // Room for real work: a sum of a hundred terms, fifty parentheses.
        assert!(matches!(expr(&(0..100).map(|i| format!("q{i}")).collect::<Vec<_>>().join(" + ")), Expr::Binary(BinOp::Add, ..)));
        assert_eq!(expr(&format!("{}1{}", "(".repeat(50), ")".repeat(50))), Expr::Num(1.0));
    }
}
