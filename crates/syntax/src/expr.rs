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
}

const ERR: SyntaxError = SyntaxError::Expected { command: "expression", expected: "a value, variable, function or (" };

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

fn is_name(word: &str) -> bool {
    word.chars().next().is_some_and(|c| !(c.is_ascii_digit() || c == '-' || c == '.'))
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

    fn or(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.and()?;
        while matches!(self.peek(), Some(Token::Or)) || self.word_is("OR") {
            self.pos += 1;
            left = Expr::Binary(BinOp::Or, Box::new(left), Box::new(self.and()?));
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.not()?;
        while matches!(self.peek(), Some(Token::And)) || self.word_is("AND") {
            self.pos += 1;
            left = Expr::Binary(BinOp::And, Box::new(left), Box::new(self.not()?));
        }
        Ok(left)
    }

    fn not(&mut self) -> Result<Expr, SyntaxError> {
        if matches!(self.peek(), Some(Token::Not)) || self.word_is("NOT") {
            self.pos += 1;
            return Ok(Expr::Not(Box::new(self.not()?)));
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

    fn comparison(&mut self) -> Result<Expr, SyntaxError> {
        let left = self.additive()?;
        let Some(op) = self.comparison_op() else { return Ok(left) };
        self.pos += 1;
        Ok(Expr::Binary(op, Box::new(left), Box::new(self.additive()?)))
    }

    fn additive(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.multiplicative()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                _ => return Ok(left),
            };
            self.pos += 1;
            left = Expr::Binary(op, Box::new(left), Box::new(self.multiplicative()?));
        }
    }

    fn multiplicative(&mut self) -> Result<Expr, SyntaxError> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                _ => return Ok(left),
            };
            self.pos += 1;
            left = Expr::Binary(op, Box::new(left), Box::new(self.unary()?));
        }
    }

    fn unary(&mut self) -> Result<Expr, SyntaxError> {
        if matches!(self.peek(), Some(Token::Minus)) {
            self.pos += 1;
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        self.power()
    }

    fn power(&mut self) -> Result<Expr, SyntaxError> {
        let base = self.primary()?;
        if matches!(self.peek(), Some(Token::Power)) {
            self.pos += 1;
            return Ok(Expr::Binary(BinOp::Pow, Box::new(base), Box::new(self.unary()?)));
        }
        Ok(base)
    }

    fn primary(&mut self) -> Result<Expr, SyntaxError> {
        match self.next().cloned() {
            Some(Token::Str(s)) => Ok(Expr::Str(s)),
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
                    let exponent = self.unary()?;
                    return Ok(Expr::Neg(Box::new(Expr::Binary(BinOp::Pow, Box::new(Expr::Num(-x)), Box::new(exponent)))));
                }
                Ok(Expr::Num(x))
            }
            Some(Token::Word(w)) if matches!(self.peek(), Some(Token::LParen)) => {
                self.pos += 1;
                let mut args = Vec::new();
                if !matches!(self.peek(), Some(Token::RParen)) {
                    loop {
                        args.push(self.or()?);
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
                Ok(Expr::Call(name, min, args))
            }
            Some(Token::Word(w)) => Ok(Expr::Var(w)),
            _ => Err(ERR),
        }
    }
}

/// Parses a whole token slice as one expression.
pub(crate) fn parse(tokens: &[Token]) -> Result<Expr, SyntaxError> {
    let mut p = Parser { tokens, pos: 0 };
    let e = p.or()?;
    if p.pos < tokens.len() {
        return Err(SyntaxError::Expected { command: "expression", expected: "an operator or the end of the expression" });
    }
    Ok(e)
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
}
