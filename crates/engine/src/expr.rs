//! Evaluates transformation expressions case by case. An expression is compiled once against the
//! dictionary (variables resolved, types and function arguments checked), then evaluated per case.
//!
//! Missing values follow SPSS's rules for transformations:
//! - a variable's user-missing values count as missing (`VALUE(x)` reads past that);
//! - arithmetic with a missing operand is system-missing, except `0 * missing`, `missing * 0`,
//!   `0 / missing` and `MOD(0, missing)`, which are 0;
//! - invalid results (division by zero, `LN` of 0, `SQRT` of a negative, overflow) are
//!   system-missing;
//! - comparisons with a missing operand are missing; `AND` is false if either side is false and
//!   `OR` is true if either side is true, whatever the other side is;
//! - `SUM`, `MEAN`, `SD`, `VARIANCE`, `MIN`, `MAX` use the valid arguments; with `.n` they need at
//!   least n of them (otherwise 1, or 2 for `SD` and `VARIANCE`).

use mksigma_core::{Dataset, Format, Value, VarType};
use mksigma_syntax::{BinOp, Expr};

use crate::EngineError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Type {
    Num,
    Str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stat {
    Sum,
    Mean,
    Sd,
    Variance,
    Min,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Math {
    Abs,
    Sqrt,
    Exp,
    Ln,
    Lg10,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Text {
    Upcase,
    Lower,
    Ltrim,
    Rtrim,
}

#[derive(Debug, Clone)]
pub(crate) enum Node {
    Num(f64),
    Str(String),
    /// A numeric variable; its user-missing values read as missing.
    Var(usize),
    /// `VALUE(x)`: the stored number even if it's user-missing.
    RawVar(usize),
    StrVar(usize),
    Neg(Box<Node>),
    Not(Box<Node>),
    Arith(BinOp, Box<Node>, Box<Node>),
    CmpNum(BinOp, Box<Node>, Box<Node>),
    CmpStr(BinOp, Box<Node>, Box<Node>),
    And(Box<Node>, Box<Node>),
    Or(Box<Node>, Box<Node>),
    Math(Math, Box<Node>),
    /// `RND` or `TRUNC` (`round`), with an optional multiple.
    Round {
        round: bool,
        x: Box<Node>,
        multiple: Option<Box<Node>>,
    },
    Mod(Box<Node>, Box<Node>),
    Stat(Stat, usize, Vec<Node>),
    /// `NMISS` (`true`) or `NVALID`.
    Count(bool, Vec<Node>),
    /// `MISSING(var)`: user- or system-missing.
    MissingVar(usize),
    /// `SYSMIS(var)`: system-missing only.
    SysmisVar(usize),
    /// `MISSING(expr)` / `SYSMIS(expr)`: the expression's result is missing.
    IsMissing(Box<Node>),
    Range(Box<Node>, Vec<(Node, Node)>),
    AnyNum(Box<Node>, Vec<Node>),
    AnyStr(Box<Node>, Vec<Node>),
    Concat(Vec<Node>),
    Text(Text, Box<Node>),
    Length(Box<Node>),
    Substr(Box<Node>, Box<Node>, Option<Box<Node>>),
    Index(Box<Node>, Box<Node>),
    Number(Box<Node>),
    ToString(Box<Node>, Format),
}

fn invalid(message: String) -> EngineError {
    EngineError::Invalid(message)
}

pub(crate) struct Compiler<'a> {
    pub(crate) ds: &'a Dataset,
}

impl Compiler<'_> {
    fn variable(&self, name: &str) -> Result<(usize, Type), EngineError> {
        let (i, var) = self.ds.variable(name).ok_or_else(|| EngineError::UnknownVariable(name.to_string()))?;
        Ok((i, if var.var_type == VarType::Numeric { Type::Num } else { Type::Str }))
    }

    fn expect(&self, e: &Expr, want: Type, context: &str) -> Result<Node, EngineError> {
        let (node, got) = self.compile(e)?;
        if got != want {
            let want = if want == Type::Num { "a number" } else { "text" };
            return Err(invalid(format!("{context} needs {want}")));
        }
        Ok(node)
    }

    fn nums(&self, args: &[Expr], name: &str) -> Result<Vec<Node>, EngineError> {
        args.iter().map(|a| self.expect(a, Type::Num, name)).collect()
    }

    pub(crate) fn compile(&self, e: &Expr) -> Result<(Node, Type), EngineError> {
        Ok(match e {
            Expr::Num(x) => (Node::Num(*x), Type::Num),
            Expr::Str(s) => (Node::Str(s.clone()), Type::Str),
            Expr::Var(name) => match self.variable(name)? {
                (i, Type::Num) => (Node::Var(i), Type::Num),
                (i, Type::Str) => (Node::StrVar(i), Type::Str),
            },
            Expr::Neg(a) => (Node::Neg(Box::new(self.expect(a, Type::Num, "-")?)), Type::Num),
            Expr::Not(a) => (Node::Not(Box::new(self.expect(a, Type::Num, "NOT")?)), Type::Num),
            Expr::Binary(op, a, b) => {
                let (left, lt) = self.compile(a)?;
                let (right, rt) = self.compile(b)?;
                let both = |t: Type| lt == t && rt == t;
                let node = match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Pow if both(Type::Num) => {
                        Node::Arith(*op, Box::new(left), Box::new(right))
                    }
                    BinOp::And if both(Type::Num) => Node::And(Box::new(left), Box::new(right)),
                    BinOp::Or if both(Type::Num) => Node::Or(Box::new(left), Box::new(right)),
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge if both(Type::Num) => {
                        Node::CmpNum(*op, Box::new(left), Box::new(right))
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge if both(Type::Str) => {
                        Node::CmpStr(*op, Box::new(left), Box::new(right))
                    }
                    _ if lt != rt => return Err(invalid("can't compare or combine text with a number".into())),
                    _ => return Err(invalid("arithmetic and AND/OR need numbers".into())),
                };
                (node, Type::Num)
            }
            Expr::Call(name, min, args) => self.call(name, *min, args)?,
        })
    }

    fn call(&self, name: &str, min: Option<usize>, args: &[Expr]) -> Result<(Node, Type), EngineError> {
        let n = args.len();
        let arity = |ok: bool, expected: &str| if ok { Ok(()) } else { Err(invalid(format!("{name} takes {expected}"))) };
        if min.is_some() && !matches!(name, "SUM" | "MEAN" | "SD" | "VARIANCE" | "MIN" | "MAX") {
            return Err(invalid(format!("{name} doesn't take a .n suffix")));
        }
        let arg = |i: usize| args.get(i).ok_or_else(|| invalid(format!("{name} is missing an argument")));
        let num = |i: usize| -> Result<Box<Node>, EngineError> { Ok(Box::new(self.expect(arg(i)?, Type::Num, name)?)) };
        let text = |i: usize| -> Result<Box<Node>, EngineError> { Ok(Box::new(self.expect(arg(i)?, Type::Str, name)?)) };
        let var_index = |what: &str| match args {
            [Expr::Var(v)] => self.variable(v),
            _ => Err(invalid(format!("{name} takes {what}"))),
        };
        Ok(match name {
            "ABS" | "SQRT" | "EXP" | "LN" | "LG10" => {
                arity(n == 1, "one number")?;
                let f = match name {
                    "ABS" => Math::Abs,
                    "SQRT" => Math::Sqrt,
                    "EXP" => Math::Exp,
                    "LN" => Math::Ln,
                    _ => Math::Lg10,
                };
                (Node::Math(f, num(0)?), Type::Num)
            }
            "RND" | "TRUNC" => {
                arity(n == 1 || n == 2, "a number and an optional multiple")?;
                let multiple = if n == 2 { Some(num(1)?) } else { None };
                (Node::Round { round: name == "RND", x: num(0)?, multiple }, Type::Num)
            }
            "MOD" => {
                arity(n == 2, "two numbers")?;
                (Node::Mod(num(0)?, num(1)?), Type::Num)
            }
            "SUM" | "MEAN" | "SD" | "VARIANCE" | "MIN" | "MAX" => {
                let stat = match name {
                    "SUM" => Stat::Sum,
                    "MEAN" => Stat::Mean,
                    "SD" => Stat::Sd,
                    "VARIANCE" => Stat::Variance,
                    "MIN" => Stat::Min,
                    _ => Stat::Max,
                };
                let default = if matches!(stat, Stat::Sd | Stat::Variance) { 2 } else { 1 };
                let need = min.unwrap_or(default);
                arity(n >= default.max(need) && need >= 1, &format!("at least {} numbers", default.max(need)))?;
                (Node::Stat(stat, need, self.nums(args, name)?), Type::Num)
            }
            "NMISS" | "NVALID" => {
                arity(n >= 1, "one or more numbers")?;
                (Node::Count(name == "NMISS", self.nums(args, name)?), Type::Num)
            }
            "MISSING" | "SYSMIS" => {
                arity(n == 1, "one argument")?;
                match args {
                    [Expr::Var(_)] => match var_index("a variable")? {
                        (i, Type::Num) if name == "SYSMIS" => (Node::SysmisVar(i), Type::Num),
                        (i, _) => (Node::MissingVar(i), Type::Num),
                    },
                    _ => (Node::IsMissing(num(0)?), Type::Num),
                }
            }
            "VALUE" => match var_index("a numeric variable")? {
                (i, Type::Num) => (Node::RawVar(i), Type::Num),
                _ => return Err(invalid("VALUE takes a numeric variable".into())),
            },
            "RANGE" => {
                arity(n >= 3 && n % 2 == 1, "a value and low, high pairs")?;
                let pairs = args.get(1..).unwrap_or_default().chunks(2).map(|p| match p {
                    [lo, hi] => Ok((self.expect(lo, Type::Num, name)?, self.expect(hi, Type::Num, name)?)),
                    _ => Err(invalid("RANGE takes low, high pairs".into())),
                });
                (Node::Range(num(0)?, pairs.collect::<Result<_, _>>()?), Type::Num)
            }
            "ANY" => {
                arity(n >= 2, "a value and the values to look for")?;
                let (x, t) = self.compile(arg(0)?)?;
                let rest = args.get(1..).unwrap_or_default().iter().map(|a| self.expect(a, t, name)).collect::<Result<_, _>>()?;
                let node = if t == Type::Num { Node::AnyNum(Box::new(x), rest) } else { Node::AnyStr(Box::new(x), rest) };
                (node, Type::Num)
            }
            "CONCAT" => {
                arity(n >= 1, "one or more strings")?;
                (Node::Concat(args.iter().map(|a| self.expect(a, Type::Str, name)).collect::<Result<_, _>>()?), Type::Str)
            }
            "UPCASE" | "LOWER" | "LTRIM" | "RTRIM" => {
                arity(n == 1, "one string")?;
                let f = match name {
                    "UPCASE" => Text::Upcase,
                    "LOWER" => Text::Lower,
                    "LTRIM" => Text::Ltrim,
                    _ => Text::Rtrim,
                };
                (Node::Text(f, text(0)?), Type::Str)
            }
            "LENGTH" | "CHAR.LENGTH" => {
                arity(n == 1, "one string")?;
                (Node::Length(text(0)?), Type::Num)
            }
            "SUBSTR" | "CHAR.SUBSTR" => {
                arity(n == 2 || n == 3, "a string, a position and an optional length")?;
                let len = if n == 3 { Some(num(2)?) } else { None };
                (Node::Substr(text(0)?, num(1)?, len), Type::Str)
            }
            "INDEX" | "CHAR.INDEX" => {
                arity(n == 2, "a string and the text to find")?;
                (Node::Index(text(0)?, text(1)?), Type::Num)
            }
            "NUMBER" => {
                arity(n == 2, "a string and a format")?;
                (Node::Number(text(0)?), Type::Num)
            }
            "STRING" => {
                arity(n == 2, "a number and a format such as F8.2")?;
                let format = match arg(1)? {
                    Expr::Var(f) => f.parse::<Format>().map_err(invalid)?,
                    _ => return Err(invalid("STRING takes a format such as F8.2 as its second argument".into())),
                };
                (Node::ToString(num(0)?, format), Type::Str)
            }
            _ => return Err(invalid(format!("unknown function {name}"))),
        })
    }
}

/// Reads one case.
pub(crate) struct Case<'a> {
    pub(crate) ds: &'a Dataset,
    pub(crate) case: usize,
}

fn finite(x: f64) -> Option<f64> {
    x.is_finite().then_some(x)
}

fn truth(x: Option<f64>) -> Option<bool> {
    x.map(|v| v != 0.0)
}

fn flag(b: Option<bool>) -> Option<f64> {
    b.map(|b| if b { 1.0 } else { 0.0 })
}

fn compare<T: PartialOrd>(op: BinOp, a: &T, b: &T) -> bool {
    match op {
        BinOp::Eq => a == b,
        BinOp::Ne => a != b,
        BinOp::Lt => a < b,
        BinOp::Le => a <= b,
        BinOp::Gt => a > b,
        _ => a >= b,
    }
}

impl Case<'_> {
    fn value(&self, var: usize) -> Option<&Value> {
        self.ds.column(var).and_then(|c| c.get(self.case))
    }

    fn var(&self, var: usize) -> Option<f64> {
        let value = self.value(var)?;
        let missing = self.ds.variables().get(var).is_none_or(|v| v.is_missing(value));
        if missing { None } else { value.as_num() }
    }

    pub(crate) fn num(&self, node: &Node) -> Option<f64> {
        match node {
            Node::Num(x) => Some(*x),
            Node::Var(i) => self.var(*i),
            Node::RawVar(i) => self.value(*i).and_then(Value::as_num),
            Node::Neg(a) => self.num(a).map(|x| -x),
            Node::Not(a) => flag(truth(self.num(a)).map(|b| !b)),
            Node::Arith(op, a, b) => {
                let (x, y) = (self.num(a), self.num(b));
                match (op, x, y) {
                    (BinOp::Mul, Some(z), _) | (BinOp::Mul, _, Some(z)) if z == 0.0 => Some(0.0),
                    (BinOp::Div, _, Some(0.0)) => None,
                    (BinOp::Div, Some(0.0), _) => Some(0.0),
                    (_, Some(x), Some(y)) => finite(match op {
                        BinOp::Add => x + y,
                        BinOp::Sub => x - y,
                        BinOp::Mul => x * y,
                        BinOp::Div => x / y,
                        _ => x.powf(y),
                    }),
                    _ => None,
                }
            }
            Node::CmpNum(op, a, b) => match (self.num(a), self.num(b)) {
                (Some(x), Some(y)) => flag(Some(compare(*op, &x, &y))),
                _ => None,
            },
            Node::CmpStr(op, a, b) => flag(Some(compare(*op, &self.text(a), &self.text(b)))),
            Node::And(a, b) => match (truth(self.num(a)), truth(self.num(b))) {
                (Some(false), _) | (_, Some(false)) => Some(0.0),
                (Some(true), Some(true)) => Some(1.0),
                _ => None,
            },
            Node::Or(a, b) => match (truth(self.num(a)), truth(self.num(b))) {
                (Some(true), _) | (_, Some(true)) => Some(1.0),
                (Some(false), Some(false)) => Some(0.0),
                _ => None,
            },
            Node::Math(f, a) => {
                let x = self.num(a)?;
                finite(match f {
                    Math::Abs => x.abs(),
                    Math::Sqrt if x >= 0.0 => x.sqrt(),
                    Math::Exp => x.exp(),
                    Math::Ln if x > 0.0 => x.ln(),
                    Math::Lg10 if x > 0.0 => x.log10(),
                    _ => return None,
                })
            }
            Node::Round { round, x, multiple } => {
                let x = self.num(x)?;
                let m = match multiple {
                    Some(m) => self.num(m).filter(|m| *m != 0.0)?,
                    None => 1.0,
                };
                // Rounds half away from zero, like SPSS's RND.
                finite(if *round { (x / m).round() * m } else { (x / m).trunc() * m })
            }
            Node::Mod(a, b) => match (self.num(a), self.num(b)) {
                (Some(0.0), _) => Some(0.0),
                (Some(x), Some(y)) if y != 0.0 => finite(x % y),
                _ => None,
            },
            Node::Stat(stat, need, args) => {
                let xs: Vec<f64> = args.iter().filter_map(|a| self.num(a)).collect();
                if xs.len() < *need {
                    return None;
                }
                let n = xs.len() as f64;
                let mean = xs.iter().sum::<f64>() / n;
                let variance = || xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
                finite(match stat {
                    Stat::Sum => xs.iter().sum(),
                    Stat::Mean => mean,
                    Stat::Variance if xs.len() > 1 => variance(),
                    Stat::Sd if xs.len() > 1 => variance().sqrt(),
                    Stat::Min => xs.iter().copied().fold(f64::INFINITY, f64::min),
                    Stat::Max => xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    _ => return None,
                })
            }
            Node::Count(missing, args) => {
                let invalid = args.iter().filter(|a| self.num(a).is_none()).count();
                let count = if *missing { invalid } else { args.len() - invalid };
                Some(count as f64)
            }
            Node::MissingVar(i) => {
                let missing = match (self.value(*i), self.ds.variables().get(*i)) {
                    (Some(v), Some(var)) => var.is_missing(v),
                    _ => true,
                };
                flag(Some(missing))
            }
            Node::SysmisVar(i) => flag(Some(matches!(self.value(*i), Some(Value::SysMis) | None))),
            Node::IsMissing(a) => flag(Some(self.num(a).is_none())),
            Node::Range(x, pairs) => {
                let x = self.num(x)?;
                flag(Some(pairs.iter().any(|(lo, hi)| matches!((self.num(lo), self.num(hi)), (Some(lo), Some(hi)) if lo <= x && x <= hi))))
            }
            Node::AnyNum(x, values) => {
                let x = self.num(x)?;
                flag(Some(values.iter().any(|v| self.num(v) == Some(x))))
            }
            Node::AnyStr(x, values) => {
                let x = self.text(x);
                flag(Some(values.iter().any(|v| self.text(v) == x)))
            }
            Node::Length(s) => Some(self.text(s).chars().count() as f64),
            Node::Index(s, needle) => {
                let (s, needle) = (self.text(s), self.text(needle));
                Some(s.find(&needle).map_or(0.0, |byte| s.get(..byte).map_or(0, |p| p.chars().count()) as f64 + 1.0))
            }
            Node::Number(s) => self.text(s).trim().parse::<f64>().ok().and_then(finite),
            Node::Str(_) | Node::StrVar(_) | Node::Concat(_) | Node::Text(..) | Node::Substr(..) | Node::ToString(..) => None,
        }
    }

    pub(crate) fn text(&self, node: &Node) -> String {
        match node {
            Node::Str(s) => s.clone(),
            Node::StrVar(i) => match self.value(*i) {
                Some(Value::Str(s)) => s.clone(),
                _ => String::new(),
            },
            Node::Concat(parts) => parts.iter().map(|p| self.text(p)).collect(),
            Node::Text(f, s) => {
                let s = self.text(s);
                match f {
                    Text::Upcase => s.to_uppercase(),
                    Text::Lower => s.to_lowercase(),
                    Text::Ltrim => s.trim_start_matches(' ').to_string(),
                    Text::Rtrim => s.trim_end_matches(' ').to_string(),
                }
            }
            Node::Substr(s, pos, len) => {
                let s = self.text(s);
                let Some(pos) = self.num(pos).filter(|p| *p >= 1.0) else { return String::new() };
                let skip = (pos as usize).saturating_sub(1);
                match len.as_ref().map(|l| self.num(l)) {
                    Some(Some(l)) if l >= 0.0 => s.chars().skip(skip).take(l as usize).collect(),
                    Some(_) => String::new(),
                    None => s.chars().skip(skip).collect(),
                }
            }
            Node::ToString(x, format) => self.num(x).map_or_else(String::new, |x| format.render(&Value::Num(x))),
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_core::{MissingValues, Variable};

    /// x = 4, y = 9 user-missing in case 0; x = 0, y system-missing in case 1.
    fn data() -> Dataset {
        let mut ds = Dataset::new();
        ds.add_variable(Variable::numeric("x")).unwrap();
        let mut y = Variable::numeric("y");
        y.missing = MissingValues::Discrete(vec![Value::Num(9.0)]);
        ds.add_variable(y).unwrap();
        ds.add_variable(Variable::string("s", 12)).unwrap();
        ds.push_case(vec![Value::Num(4.0), Value::Num(9.0), Value::Str("  São Paulo".into())]).unwrap();
        ds.push_case(vec![Value::Num(0.0), Value::SysMis, Value::Str("ab".into())]).unwrap();
        ds
    }

    fn eval(src: &str, case: usize) -> Option<f64> {
        let ds = data();
        let cmd = mksigma_syntax::parse(&format!("COMPUTE t = {src}.")).unwrap();
        let mksigma_syntax::Command::Compute { expr, .. } = &cmd[0] else { panic!() };
        let (node, t) = Compiler { ds: &ds }.compile(expr).unwrap();
        assert_eq!(t, Type::Num, "{src}");
        Case { ds: &ds, case }.num(&node)
    }

    fn eval_text(src: &str, case: usize) -> String {
        let ds = data();
        let cmd = mksigma_syntax::parse(&format!("COMPUTE t = {src}.")).unwrap();
        let mksigma_syntax::Command::Compute { expr, .. } = &cmd[0] else { panic!() };
        let (node, t) = Compiler { ds: &ds }.compile(expr).unwrap();
        assert_eq!(t, Type::Str, "{src}");
        Case { ds: &ds, case }.text(&node)
    }

    fn compile_error(src: &str) -> String {
        let ds = data();
        let cmd = mksigma_syntax::parse(&format!("COMPUTE t = {src}.")).unwrap();
        let mksigma_syntax::Command::Compute { expr, .. } = &cmd[0] else { panic!() };
        Compiler { ds: &ds }.compile(expr).err().map(|e| e.to_string()).unwrap_or_default()
    }

    #[test]
    fn arithmetic_and_precedence() {
        assert_eq!(eval("x + 2 * 3 ** 2", 0), Some(22.0));
        assert_eq!(eval("-2 ** 2", 0), Some(-4.0));
        assert_eq!(eval("(x - 1) / 2", 0), Some(1.5));
        assert_eq!(eval("x / 0", 0), None, "division by zero is system-missing");
        assert_eq!(eval("SQRT(-1)", 0), None);
        assert_eq!(eval("LN(0)", 0), None);
        assert_eq!(eval("10 ** 400", 0), None, "overflow");
        assert_eq!(eval("RND(2.5) + RND(-2.5)", 0), Some(0.0), "half away from zero");
        assert_eq!(eval("RND(7.3, 0.5) + TRUNC(7.9, 2)", 0), Some(13.5));
        assert_eq!(eval("TRUNC(-2.7)", 0), Some(-2.0));
        assert_eq!(eval("MOD(10, 3)", 0), Some(1.0));
        assert_eq!(eval("ABS(-3) + EXP(0) + LG10(100)", 0), Some(6.0));
    }

    #[test]
    fn missing_values() {
        assert_eq!(eval("y", 0), None, "9 is user-missing");
        assert_eq!(eval("VALUE(y)", 0), Some(9.0));
        assert_eq!(eval("y + 1", 0), None);
        assert_eq!(eval("x * y", 1), Some(0.0), "0 times missing is 0");
        assert_eq!(eval("y * x", 1), Some(0.0));
        assert_eq!(eval("x / y", 1), Some(0.0), "0 divided by missing is 0");
        assert_eq!(eval("MOD(x, y)", 1), Some(0.0));
        assert_eq!(eval("x * y", 0), None);
        assert_eq!(eval("MISSING(y) + SYSMIS(y)", 0), Some(1.0), "user-missing: MISSING yes, SYSMIS no");
        assert_eq!(eval("MISSING(y) + SYSMIS(y)", 1), Some(2.0));
        assert_eq!(eval("MISSING(y + 1)", 0), Some(1.0));
        assert_eq!(eval("NMISS(x, y) * 10 + NVALID(x, y)", 0), Some(11.0));
    }

    #[test]
    fn logic_with_missing() {
        assert_eq!(eval("y > 1", 0), None);
        assert_eq!(eval("y > 1 AND x > 10", 0), Some(0.0), "false AND missing is false");
        assert_eq!(eval("y > 1 OR x > 1", 0), Some(1.0), "missing OR true is true");
        assert_eq!(eval("y > 1 OR x > 10", 0), None);
        assert_eq!(eval("NOT (x = 4)", 0), Some(0.0));
        assert_eq!(eval("x >= 4 & x < 5", 0), Some(1.0));
        assert_eq!(eval("RANGE(x, 1, 3, 4, 6)", 0), Some(1.0));
        assert_eq!(eval("ANY(x, 1, 2, 4)", 0), Some(1.0));
        assert_eq!(eval("ANY(s, 'ab', 'cd')", 1), Some(1.0));
        assert_eq!(eval("s = 'ab'", 1), Some(1.0));
    }

    #[test]
    fn statistics_functions() {
        assert_eq!(eval("SUM(x, y, 1)", 0), Some(5.0), "the missing y is skipped");
        assert_eq!(eval("MEAN(x, y, 2)", 0), Some(3.0));
        assert_eq!(eval("MEAN.3(x, y, 2)", 0), None, "needs three valid values");
        assert_eq!(eval("MIN(x, 2, 7) + MAX(x, 2, 7)", 0), Some(9.0));
        assert_eq!(eval("VARIANCE(2, 4, 6)", 0), Some(4.0));
        assert_eq!(eval("SD(x, y)", 0), None, "one valid value is not enough");
    }

    #[test]
    fn strings() {
        assert_eq!(eval_text("UPCASE(LTRIM(s))", 0), "SÃO PAULO");
        assert_eq!(eval_text("CONCAT(s, '-', 'x')", 1), "ab-x");
        assert_eq!(eval_text("SUBSTR(LTRIM(s), 5)", 0), "Paulo");
        assert_eq!(eval_text("CHAR.SUBSTR(LTRIM(s), 1, 3)", 0), "São");
        assert_eq!(eval("INDEX(s, 'Paulo')", 0), Some(7.0), "positions count characters");
        assert_eq!(eval("LENGTH(LTRIM(s))", 0), Some(9.0));
        assert_eq!(eval("NUMBER('12.5', F8.1) + 1", 0), Some(13.5));
        assert_eq!(eval("NUMBER('abc', F8)", 0), None);
        assert_eq!(eval_text("STRING(x / 3, F8.2)", 0), "1.33");
    }

    #[test]
    fn type_and_argument_errors() {
        assert!(compile_error("s + 1").contains("text with a number"));
        assert!(compile_error("nope + 1").contains("nope does not exist"));
        assert!(compile_error("FOO(x)").contains("unknown function FOO"));
        assert!(compile_error("SQRT(x, 2)").contains("SQRT takes one number"));
        assert!(compile_error("ABS.2(x)").contains("doesn't take a .n"));
        assert!(compile_error("MEAN.3(x, y)").contains("at least 3"));
        assert!(compile_error("UPCASE(x)").contains("needs text"));
        assert!(compile_error("VALUE(x + 1)").contains("VALUE takes"));
    }
}
