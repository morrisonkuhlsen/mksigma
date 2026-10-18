//! What a ribbon button pastes into the syntax editor: the command for a procedure, on variables
//! picked from the dictionary by measurement level, the way SPSS's dialogs paste syntax. When the
//! data has no suitable variables the command comes back as a template with placeholder names,
//! to be completed before running.

use mksigma_engine::core::{Dataset, Measure, Value, VarType, Variable};
use mksigma_engine::quote;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Procedure {
    Descriptives,
    Frequencies,
    Explore,
    Crosstabs,
    TTest,
    Oneway,
    Correlations,
    Regression,
    Logistic,
    Nonparametric,
    Histogram,
    Bar,
    Scatter,
    Boxplot,
    Dictionary,
    Compute,
    Recode,
    SelectCases,
}

/// The pasted syntax, and whether it is ready to run (`false`: a template to complete).
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub syntax: String,
    pub ready: bool,
}

fn ready(syntax: String) -> Suggestion {
    Suggestion { syntax, ready: true }
}

fn template(syntax: &str) -> Suggestion {
    Suggestion { syntax: syntax.to_string(), ready: false }
}

/// Up to `max` numeric Scale variables.
fn scale(ds: &Dataset, max: usize) -> Vec<&Variable> {
    ds.variables().iter().filter(|v| v.var_type == VarType::Numeric && v.measure == Measure::Scale).take(max).collect()
}

/// Up to `max` Nominal or Ordinal variables.
fn categorical(ds: &Dataset, max: usize) -> Vec<&Variable> {
    ds.variables().iter().filter(|v| v.measure != Measure::Scale).take(max).collect()
}

/// The valid values of a variable, in ascending order, at most `max` of them (more means
/// "more than max").
fn values(ds: &Dataset, var: &Variable, max: usize) -> Vec<Value> {
    let Some((i, _)) = ds.variable(&var.name) else { return Vec::new() };
    let mut out: Vec<Value> = Vec::new();
    for v in ds.column(i).unwrap_or_default() {
        if var.is_missing(v) || out.contains(v) {
            continue;
        }
        out.push(v.clone());
        if out.len() > max {
            break;
        }
    }
    out.sort_by(|a, b| match (a, b) {
        (Value::Num(x), Value::Num(y)) => x.total_cmp(y),
        (Value::Str(x), Value::Str(y)) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
    });
    out
}

fn literal(v: &Value) -> String {
    match v {
        Value::Num(x) => x.to_string(),
        Value::Str(s) => quote(s),
        Value::SysMis => ".".to_string(),
    }
}

fn names(vars: &[&Variable]) -> String {
    vars.iter().map(|v| v.name.as_str()).collect::<Vec<_>>().join(" ")
}

pub fn suggest(procedure: Procedure, ds: &Dataset) -> Suggestion {
    let scales = scale(ds, 6);
    let cats = categorical(ds, 2);
    let first_scale = scales.first();
    let first_cat = cats.first();
    match procedure {
        Procedure::Descriptives => match scales.as_slice() {
            [] => template("DESCRIPTIVES VARIABLES=var1 var2."),
            vars => ready(format!("DESCRIPTIVES VARIABLES={}.", names(vars))),
        },
        Procedure::Frequencies => match (first_cat, first_scale) {
            (Some(_), _) => ready(format!("FREQUENCIES VARIABLES={} /BARCHART.", names(&cats))),
            (None, Some(v)) => ready(format!("FREQUENCIES VARIABLES={} /FORMAT=NOTABLE /STATISTICS /HISTOGRAM(NORMAL).", v.name)),
            (None, None) => template("FREQUENCIES VARIABLES=var1."),
        },
        Procedure::Explore => match (first_scale, first_cat) {
            (Some(y), Some(g)) => ready(format!("EXAMINE VARIABLES={} BY {} /PLOT=BOXPLOT.", y.name, g.name)),
            (Some(y), None) => ready(format!("EXAMINE VARIABLES={} /PLOT=BOXPLOT.", y.name)),
            _ => template("EXAMINE VARIABLES=dependent BY factor /PLOT=BOXPLOT."),
        },
        Procedure::Crosstabs => match cats.as_slice() {
            [a, b, ..] => ready(format!("CROSSTABS /TABLES={} BY {} /CELLS=COUNT ROW /STATISTICS=CHISQ.", a.name, b.name)),
            _ => template("CROSSTABS /TABLES=row BY column /CELLS=COUNT ROW /STATISTICS=CHISQ."),
        },
        Procedure::TTest => {
            let groups = first_cat.map(|g| (g, values(ds, g, 2)));
            match (first_scale, groups) {
                (Some(y), Some((g, v))) if v.len() == 2 => {
                    ready(format!("T-TEST GROUPS={}({} {}) /VARIABLES={}.", g.name, literal(&v[0]), literal(&v[1]), y.name))
                }
                _ => template("T-TEST GROUPS=group(1 2) /VARIABLES=dependent."),
            }
        }
        Procedure::Oneway => match (first_scale, first_cat) {
            (Some(y), Some(g)) if g.var_type == VarType::Numeric => {
                ready(format!("ONEWAY {} BY {} /STATISTICS=DESCRIPTIVES HOMOGENEITY /POSTHOC=TUKEY ALPHA(0.05).", y.name, g.name))
            }
            _ => template("ONEWAY dependent BY factor /STATISTICS=DESCRIPTIVES HOMOGENEITY /POSTHOC=TUKEY ALPHA(0.05)."),
        },
        Procedure::Correlations => match scales.as_slice() {
            vars @ [_, _, ..] => ready(format!("CORRELATIONS /VARIABLES={} /PRINT=TWOTAIL NOSIG.", names(vars))),
            _ => template("CORRELATIONS /VARIABLES=var1 var2 /PRINT=TWOTAIL NOSIG."),
        },
        Procedure::Regression => match scales.as_slice() {
            [predictors @ .., dependent] if !predictors.is_empty() => {
                ready(format!("REGRESSION /DEPENDENT {} /METHOD=ENTER {}.", dependent.name, names(predictors)))
            }
            _ => template("REGRESSION /DEPENDENT dependent /METHOD=ENTER predictor1 predictor2."),
        },
        Procedure::Logistic => {
            // A numeric variable with two values, coded 0 and 1.
            let binary = ds.variables().iter().find(|v| v.var_type == VarType::Numeric && values(ds, v, 2) == [Value::Num(0.0), Value::Num(1.0)]);
            let covariates: Vec<&Variable> = scales.iter().copied().filter(|v| Some(v.name.as_str()) != binary.map(|b| b.name.as_str())).collect();
            match binary {
                Some(b) if !covariates.is_empty() => ready(format!("LOGISTIC REGRESSION VARIABLES {} /METHOD=ENTER {}.", b.name, names(&covariates))),
                _ => template("LOGISTIC REGRESSION VARIABLES outcome /METHOD=ENTER covariate1 covariate2."),
            }
        }
        Procedure::Nonparametric => {
            let groups = first_cat.filter(|g| g.var_type == VarType::Numeric).map(|g| (g, values(ds, g, 2)));
            match (first_scale, groups) {
                (Some(y), Some((g, v))) if v.len() == 2 => {
                    ready(format!("NPAR TESTS /M-W={} BY {}({} {}).", y.name, g.name, literal(&v[0]), literal(&v[1])))
                }
                _ => template("NPAR TESTS /M-W=dependent BY group(1 2)."),
            }
        }
        Procedure::Histogram => match first_scale {
            Some(v) => ready(format!("GRAPH /HISTOGRAM(NORMAL)={}.", v.name)),
            None => template("GRAPH /HISTOGRAM(NORMAL)=variable."),
        },
        Procedure::Bar => match (first_cat, first_scale) {
            (Some(g), _) => ready(format!("GRAPH /BAR(SIMPLE)=COUNT BY {}.", g.name)),
            _ => template("GRAPH /BAR(SIMPLE)=COUNT BY category."),
        },
        Procedure::Scatter => match scales.as_slice() {
            [x, y, ..] => ready(format!("GRAPH /SCATTERPLOT(BIVAR)={} WITH {}.", x.name, y.name)),
            _ => template("GRAPH /SCATTERPLOT(BIVAR)=x WITH y."),
        },
        Procedure::Boxplot => match (first_scale, first_cat) {
            (Some(y), Some(g)) => ready(format!("EXAMINE VARIABLES={} BY {} /PLOT=BOXPLOT /STATISTICS=NONE.", y.name, g.name)),
            (Some(y), None) => ready(format!("EXAMINE VARIABLES={} /PLOT=BOXPLOT /STATISTICS=NONE.", y.name)),
            _ => template("EXAMINE VARIABLES=dependent BY factor /PLOT=BOXPLOT /STATISTICS=NONE."),
        },
        Procedure::Dictionary => ready("DISPLAY DICTIONARY.".to_string()),
        // Transformations change the data: always a template to complete.
        Procedure::Compute => template(&format!("COMPUTE new_variable = {}.", first_scale.map_or("expression", |v| v.name.as_str()))),
        Procedure::Recode => {
            let source = first_scale.map_or("variable", |v| v.name.as_str());
            template(&format!("RECODE {source} (LO THRU 10 = 1) (ELSE = 2) INTO {source}_group."))
        }
        Procedure::SelectCases => template(&format!("SELECT IF ({} > 0).", first_scale.map_or("variable", |v| v.name.as_str()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_engine::Session;

    const ALL: [Procedure; 18] = [
        Procedure::Descriptives,
        Procedure::Frequencies,
        Procedure::Explore,
        Procedure::Crosstabs,
        Procedure::TTest,
        Procedure::Oneway,
        Procedure::Correlations,
        Procedure::Regression,
        Procedure::Logistic,
        Procedure::Nonparametric,
        Procedure::Histogram,
        Procedure::Bar,
        Procedure::Scatter,
        Procedure::Boxplot,
        Procedure::Dictionary,
        Procedure::Compute,
        Procedure::Recode,
        Procedure::SelectCases,
    ];

    #[test]
    fn every_ready_suggestion_runs_and_every_template_parses() {
        let mut s = Session::with_sample().unwrap();
        s.run_syntax(
            "RECODE age (LO THRU 30 = 1) (ELSE = 2) INTO grupo.\nRECODE age (LO THRU 35 = 0) (ELSE = 1) INTO velho.\n\
             RECODE income (LO THRU 3000 = 1) (ELSE = 2) INTO faixa.\nVARIABLE LEVEL grupo velho faixa (NOMINAL).",
        )
        .unwrap();
        for procedure in ALL {
            let suggestion = suggest(procedure, &s.dataset);
            if suggestion.ready {
                s.run_syntax(&suggestion.syntax).unwrap_or_else(|e| panic!("{procedure:?}: {} fails: {e}", suggestion.syntax));
            } else {
                assert!(matches!(procedure, Procedure::Compute | Procedure::Recode | Procedure::SelectCases), "{procedure:?} should be ready");
            }
        }
        assert_eq!(suggest(Procedure::TTest, &s.dataset).syntax, "T-TEST GROUPS=grupo(1 2) /VARIABLES=age.");
        assert_eq!(suggest(Procedure::Logistic, &s.dataset).syntax, "LOGISTIC REGRESSION VARIABLES velho /METHOD=ENTER age income score.");
    }

    #[test]
    fn without_categories_the_grouped_procedures_are_templates() {
        let s = Session::with_sample().unwrap();
        assert!(suggest(Procedure::Descriptives, &s.dataset).ready);
        assert_eq!(suggest(Procedure::Descriptives, &s.dataset).syntax, "DESCRIPTIVES VARIABLES=age income score.");
        for procedure in [Procedure::Crosstabs, Procedure::TTest, Procedure::Oneway, Procedure::Bar, Procedure::Logistic] {
            let t = suggest(procedure, &s.dataset);
            assert!(!t.ready, "{procedure:?}");
            assert!(mksigma_engine::check_syntax(&t.syntax).is_ok(), "{procedure:?}: {}", t.syntax);
        }
    }
}
