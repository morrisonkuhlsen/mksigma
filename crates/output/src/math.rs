//! Mathematical notation as a tree, for the calculations shown by hand: fractions, sums with
//! their limits, sub- and superscripts, roots, bars. The same tree becomes LaTeX (`to_latex`)
//! and is drawn on screen by the frontends, so the two always say the same.

/// A piece of mathematics.
#[derive(Debug, Clone, PartialEq)]
pub enum Math {
    /// A variable or name, set in italics (`x`, `n`).
    Var(String),
    /// A number, an operator or a word, upright (`3.5`, `=`, `+`, `min`).
    Text(String),
    /// Pieces side by side.
    Row(Vec<Math>),
    Frac(Box<Math>, Box<Math>),
    Sqrt(Box<Math>),
    Sub(Box<Math>, Box<Math>),
    Sup(Box<Math>, Box<Math>),
    /// A bar over it: a mean, `x̄`.
    Bar(Box<Math>),
    /// `Σ` with its limits under and over it, then what is summed.
    Sum {
        from: Option<Box<Math>>,
        to: Option<Box<Math>>,
        body: Box<Math>,
    },
    /// In parentheses that grow with it.
    Paren(Box<Math>),
    /// A matrix in brackets, its rows of entries; with a bar before column `bar` when it is an
    /// augmented one (`[A | I]`).
    Matrix {
        rows: Vec<Vec<Math>>,
        bar: Option<usize>,
    },
}

/// A shorthand for building trees.
pub mod build {
    use super::Math;

    pub fn var(s: &str) -> Math {
        Math::Var(s.to_string())
    }
    pub fn text(s: &str) -> Math {
        Math::Text(s.to_string())
    }
    /// A number as written for a reader: up to `places` decimals, no trailing zeros.
    pub fn num(x: f64, places: usize) -> Math {
        Math::Text(super::number(x, places))
    }
    pub fn row(parts: Vec<Math>) -> Math {
        Math::Row(parts)
    }
    pub fn frac(a: Math, b: Math) -> Math {
        Math::Frac(Box::new(a), Box::new(b))
    }
    pub fn sqrt(a: Math) -> Math {
        Math::Sqrt(Box::new(a))
    }
    pub fn sub(a: Math, b: Math) -> Math {
        Math::Sub(Box::new(a), Box::new(b))
    }
    pub fn sup(a: Math, b: Math) -> Math {
        Math::Sup(Box::new(a), Box::new(b))
    }
    pub fn bar(a: Math) -> Math {
        Math::Bar(Box::new(a))
    }
    pub fn paren(a: Math) -> Math {
        Math::Paren(Box::new(a))
    }
    pub fn matrix(rows: Vec<Vec<Math>>) -> Math {
        Math::Matrix { rows, bar: None }
    }
    /// `[A | B]`: the bar before column `bar`.
    pub fn augmented(rows: Vec<Vec<Math>>, bar: usize) -> Math {
        Math::Matrix { rows, bar: Some(bar) }
    }
    pub fn sum(from: Option<Math>, to: Option<Math>, body: Math) -> Math {
        Math::Sum { from: from.map(Box::new), to: to.map(Box::new), body: Box::new(body) }
    }
    /// `x_i`.
    pub fn indexed(name: &str, index: &str) -> Math {
        sub(var(name), var(index))
    }
    /// Terms joined by `+`.
    pub fn plus(terms: Vec<Math>) -> Math {
        let mut out = Vec::with_capacity(terms.len() * 2);
        for (k, t) in terms.into_iter().enumerate() {
            if k > 0 {
                out.push(text("+"));
            }
            out.push(t);
        }
        Math::Row(out)
    }
    /// `a = b = c...`
    pub fn equals(parts: Vec<Math>) -> Math {
        let mut out = Vec::with_capacity(parts.len() * 2);
        for (k, t) in parts.into_iter().enumerate() {
            if k > 0 {
                out.push(text("="));
            }
            out.push(t);
        }
        Math::Row(out)
    }
}

/// `x` with at most `places` decimals and no trailing zeros (`2.5`, `-0.125`, `12`).
pub fn number(x: f64, places: usize) -> String {
    if !x.is_finite() {
        return "—".to_string();
    }
    let s = format!("{x:.places$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".to_string() } else { s }
}

/// Text inside `\text{}`: LaTeX's special characters escaped.
fn latex_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            // Symbols pdflatex has no text glyph for, set as mathematics.
            '−' => out.push_str("$-$"),
            '≈' => out.push_str("$\\approx$"),
            '·' => out.push_str("$\\cdot$"),
            '×' => out.push_str("$\\times$"),
            '≤' => out.push_str("$\\le$"),
            '≥' => out.push_str("$\\ge$"),
            '±' => out.push_str("$\\pm$"),
            'μ' => out.push_str("$\\mu$"),
            'σ' => out.push_str("$\\sigma$"),
            'χ' => out.push_str("$\\chi$"),
            'α' => out.push_str("$\\alpha$"),
            '√' => out.push_str("$\\surd$"),
            '→' => out.push_str("$\\to$"),
            '₁' => out.push_str("$_1$"),
            '₂' => out.push_str("$_2$"),
            '–' => out.push_str("--"),
            '—' => out.push_str("---"),
            '…' | '⋯' => out.push_str("\\ldots{}"),
            c => out.push(c),
        }
    }
    out
}

impl Math {
    /// The LaTeX for it, for math mode.
    pub fn to_latex(&self) -> String {
        match self {
            Math::Var(s) => {
                // Greek letters as LaTeX's own.
                let greek = match s.as_str() {
                    "μ" => Some("\\mu"),
                    "σ" => Some("\\sigma"),
                    "ν" => Some("\\nu"),
                    "α" => Some("\\alpha"),
                    "χ" => Some("\\chi"),
                    _ => None,
                };
                if let Some(g) = greek {
                    g.to_string()
                } else if s.chars().count() == 1 {
                    latex_text(s)
                } else {
                    format!("\\mathit{{{}}}", latex_text(s))
                }
            }
            Math::Text(s) => match s.as_str() {
                "+" | "-" | "=" | "(" | ")" | "," => s.clone(),
                "−" => "-".into(),
                "×" | "·" => "\\cdot".into(),
                "≈" => "\\approx".into(),
                "…" | "⋯" => "\\cdots".into(),
                "≤" => "\\le".into(),
                "≥" => "\\ge".into(),
                "±" => "\\pm".into(),
                "→" => "\\to".into(),
                "|" | "!" => s.clone(),
                s if s.parse::<f64>().is_ok() => s.to_string(),
                s => format!("\\text{{{}}}", latex_text(s)),
            },
            Math::Row(parts) => parts.iter().map(Math::to_latex).collect::<Vec<_>>().join(" "),
            Math::Frac(a, b) => format!("\\frac{{{}}}{{{}}}", a.to_latex(), b.to_latex()),
            Math::Sqrt(a) => format!("\\sqrt{{{}}}", a.to_latex()),
            Math::Sub(a, b) => format!("{{{}}}_{{{}}}", a.to_latex(), b.to_latex()),
            Math::Sup(a, b) => format!("{{{}}}^{{{}}}", a.to_latex(), b.to_latex()),
            Math::Bar(a) => format!("\\bar{{{}}}", a.to_latex()),
            Math::Sum { from, to, body } => {
                let from = from.as_ref().map(|f| format!("_{{{}}}", f.to_latex())).unwrap_or_default();
                let to = to.as_ref().map(|t| format!("^{{{}}}", t.to_latex())).unwrap_or_default();
                format!("\\sum{from}{to} {}", body.to_latex())
            }
            Math::Paren(a) => format!("\\left( {} \\right)", a.to_latex()),
            Math::Matrix { rows, bar } => {
                let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
                let spec: String = (0..columns).map(|c| if Some(c) == *bar && c > 0 { "|c" } else { "c" }).collect();
                let body: Vec<String> = rows.iter().map(|r| r.iter().map(Math::to_latex).collect::<Vec<_>>().join(" & ")).collect();
                format!("\\left[ \\begin{{array}}{{{spec}}} {} \\end{{array}} \\right]", body.join(" \\\\ "))
            }
        }
    }
}

/// One step of a calculation: what it does, in words, then the mathematics.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub text: String,
    pub math: Option<Math>,
}

/// A calculation shown by hand: sections (a variable, a pair of variables) of steps.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Explanation {
    pub title: String,
    pub sections: Vec<(String, Vec<Step>)>,
}

impl Explanation {
    /// A LaTeX document that compiles as it is (`amsmath`).
    pub fn to_latex(&self) -> String {
        let mut body = String::new();
        for (heading, steps) in &self.sections {
            body.push_str(&format!("\\subsection*{{{}}}\n", latex_text(heading)));
            for step in steps {
                if !step.text.is_empty() {
                    body.push_str(&format!("\\noindent {}\n", latex_text(&step.text)));
                }
                if let Some(m) = &step.math {
                    body.push_str(&format!("\\begin{{dmath*}}\n{}\n\\end{{dmath*}}\n", m.to_latex()));
                }
                body.push_str("\\medskip\n\n");
            }
        }
        format!(
            "% Written by MK Sigma: the calculation behind a table, by hand.\n\\documentclass{{article}}\n\\usepackage[T1]{{fontenc}}\n\\usepackage[margin=2.5cm]{{geometry}}\n\\usepackage{{amsmath}}\n\\usepackage{{breqn}}\n\\begin{{document}}\n\\section*{{{}}}\n\n{body}\\end{{document}}\n",
            latex_text(&self.title)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::*;

    #[test]
    fn numbers_for_readers() {
        assert_eq!(number(2.5, 4), "2.5");
        assert_eq!(number(72.375, 2), "72.38");
        assert_eq!(number(12.0, 4), "12");
        assert_eq!(number(-0.00001, 3), "0");
        assert_eq!(number(f64::NAN, 3), "—");
    }

    #[test]
    fn latex_of_a_mean() {
        let mean =
            equals(vec![bar(var("x")), frac(text("1"), var("n")), sum(Some(equals(vec![var("i"), text("1")])), Some(var("n")), indexed("x", "i"))]);
        assert_eq!(mean.to_latex(), "\\bar{x} = \\frac{1}{n} = \\sum_{i = 1}^{n} {x}_{i}");
        assert_eq!(sqrt(sup(var("s"), num(2.0, 0))).to_latex(), "\\sqrt{{s}^{2}}");
        assert_eq!(row(vec![sub(var("μ"), text("0")), text("±"), text("|")]).to_latex(), "{\\mu}_{0} \\pm |");
        assert_eq!(
            augmented(vec![vec![num(2.0, 0), num(1.0, 0)], vec![num(1.0, 0), num(3.0, 0)]], 1).to_latex(),
            "\\left[ \\begin{array}{c|c} 2 & 1 \\\\ 1 & 3 \\end{array} \\right]"
        );
        // The symbols the explanations write in their sentences: none left for pdflatex to refuse.
        let words = latex_text("N − 1, ≈ · × ≤ ≥ … ± μ σ χ α √2 Welch–Satterthwaite");
        assert!(words.is_ascii(), "{words}");
        assert_eq!(row(vec![var("score"), text("≈"), text("Test & score")]).to_latex(), "\\mathit{score} \\approx \\text{Test \\& score}");
        let e = Explanation {
            title: "Descriptive Statistics".into(),
            sections: vec![("age".into(), vec![Step { text: "50% of it, by N − 1".into(), math: Some(var("n")) }])],
        };
        let tex = e.to_latex();
        assert!(tex.contains("\\subsection*{age}") && tex.contains("50\\% of it, by N $-$ 1") && tex.contains("\\begin{dmath*}\nn\n\\end{dmath*}"));
    }
}
