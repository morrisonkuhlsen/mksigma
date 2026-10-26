//! `ALL` and `first TO last` in variable lists, expanded by the dictionary's order, as in SPSS:
//! `DESCRIPTIVES ALL.`, `FREQUENCIES q1 TO q10.` Analyses expand their lists each time they run,
//! so a filtered rerun or a rerun after new variables sees the current dictionary. Lists of new
//! variables (`NUMERIC`, `STRING`, `RECODE … INTO`) take `x1 TO x5` as numbered names.

use mksigma_core::Dataset;
use mksigma_i18n::tr;
use mksigma_syntax::{Command, NparSpec, PairList, SyntaxError, TTestSpec};

use crate::EngineError;

/// The most names one `x1 TO xN` makes, as many as a dataset's variables.
const MAX_NUMBERED: u64 = 32_767;

/// A list of variables that may be new: `first TO last` is the dictionary's range when both
/// exist, or else numbered names with one root, `x1 TO x3` being `x1 x2 x3` and `v08 TO v10`
/// keeping the first's digits, `v08 v09 v10`.
pub(crate) fn new_names(ds: &Dataset, list: &[String]) -> Result<Vec<String>, EngineError> {
    let mut out = Vec::with_capacity(list.len());
    let mut words = list.iter().peekable();
    while let Some(word) = words.next() {
        if word.eq_ignore_ascii_case("TO") {
            return Err(EngineError::Invalid(tr!("err-to-needs-variables")));
        }
        if !words.peek().is_some_and(|w| w.eq_ignore_ascii_case("TO")) {
            out.push(word.clone());
            continue;
        }
        words.next();
        let last = words.next().ok_or_else(|| EngineError::Invalid(tr!("err-to-needs-after", word = word)))?;
        if ds.variable(word).is_some() && ds.variable(last).is_some() {
            out.extend(names(ds, &[word.clone(), "TO".into(), last.clone()])?);
            continue;
        }
        let wrong = || EngineError::Invalid(tr!("err-to-numbered", first = word, last = last));
        let ((root, width, from), (other, _, to)) = numbered(word).zip(numbered(last)).ok_or_else(wrong)?;
        if root.is_empty() || !root.eq_ignore_ascii_case(other) || from > to || to - from >= MAX_NUMBERED {
            return Err(wrong());
        }
        out.extend((from..=to).map(|n| format!("{root}{n:0width$}")));
    }
    Ok(out)
}

/// A name's root, how many digits end it, and their number: `v08` is `("v", 2, 8)`.
fn numbered(name: &str) -> Option<(&str, usize, u64)> {
    let root = name.trim_end_matches(|c: char| c.is_ascii_digit());
    let number = name.get(root.len()..)?;
    Some((root, number.len(), number.parse().ok()?))
}

/// A pair list with `ALL` and `TO` expanded, checked to pair up.
fn pair_list(ds: &Dataset, pairs: &PairList, command: &'static str) -> Result<PairList, EngineError> {
    let with = pairs.with.as_ref().map(|w| names(ds, w)).transpose()?;
    let expanded = PairList { left: names(ds, &pairs.left)?, with, paired: pairs.paired };
    expanded.pairs().map_err(|expected| SyntaxError::Expected { command, expected })?;
    Ok(expanded)
}

/// The names a list stands for, in order.
pub(crate) fn names(ds: &Dataset, list: &[String]) -> Result<Vec<String>, EngineError> {
    let all: Vec<&str> = ds.variables().iter().map(|v| v.name.as_str()).collect();
    let index = |name: &str| all.iter().position(|n| n.eq_ignore_ascii_case(name)).ok_or_else(|| EngineError::UnknownVariable(name.to_string()));
    let mut out = Vec::with_capacity(list.len());
    let mut words = list.iter().peekable();
    while let Some(word) = words.next() {
        if word.eq_ignore_ascii_case("ALL") {
            out.extend(all.iter().map(|n| n.to_string()));
        } else if word.eq_ignore_ascii_case("TO") {
            return Err(EngineError::Invalid(tr!("err-to-needs-variables")));
        } else if words.peek().is_some_and(|w| w.eq_ignore_ascii_case("TO")) {
            words.next();
            let last = words.next().ok_or_else(|| EngineError::Invalid(tr!("err-to-needs-after", word = word)))?;
            let (from, to) = (index(word)?, index(last)?);
            if from > to {
                return Err(EngineError::Invalid(tr!("err-to-empty", first = word, last = last)));
            }
            out.extend(all.get(from..=to).unwrap_or_default().iter().map(|n| n.to_string()));
        } else {
            out.push(word.clone());
        }
    }
    Ok(out)
}

/// `command` with `ALL` and `TO` expanded in its variable lists.
pub(crate) fn expand(ds: &Dataset, command: &Command) -> Result<Command, EngineError> {
    let list = |l: &Vec<String>| names(ds, l);
    let mut c = command.clone();
    match &mut c {
        Command::Descriptives { variables } | Command::Frequencies { variables, .. } | Command::Examine { variables, .. } => {
            *variables = list(variables)?;
        }
        Command::DeleteVariables(variables) => *variables = list(variables)?,
        Command::ValueLabels(groups) => {
            for (vars, _) in groups {
                *vars = list(vars)?;
            }
        }
        Command::MissingValues(groups) => {
            for (vars, _) in groups {
                *vars = list(vars)?;
            }
        }
        Command::VariableLevel(groups) => {
            for (vars, _) in groups {
                *vars = list(vars)?;
            }
        }
        Command::Formats(groups) => {
            for (vars, _) in groups {
                *vars = list(vars)?;
            }
        }
        Command::Crosstabs { rows, columns, .. } => {
            *rows = list(rows)?;
            *columns = list(columns)?;
        }
        Command::TTest { test: TTestSpec::OneSample { variables, .. } | TTestSpec::Independent { variables, .. }, .. } => {
            *variables = list(variables)?
        }
        Command::TTest { test: TTestSpec::Paired { pairs }, .. } => *pairs = pair_list(ds, pairs, "T-TEST")?,
        Command::Recode(groups) => {
            for group in groups {
                group.variables = list(&group.variables)?;
                group.into = new_names(ds, &group.into)?;
                if !group.into.is_empty() && group.into.len() != group.variables.len() {
                    return Err(SyntaxError::Expected { command: "RECODE", expected: "as many INTO variables as variables to recode" }.into());
                }
            }
        }
        Command::Numeric(groups) => {
            for (vars, _) in groups {
                *vars = new_names(ds, vars)?;
            }
        }
        Command::String(groups) => {
            for (vars, _) in groups {
                *vars = new_names(ds, vars)?;
            }
        }
        Command::Correlations { variables, with, .. } => {
            *variables = list(variables)?;
            *with = list(with)?;
        }
        Command::Regression { predictors, .. } => *predictors = list(predictors)?,
        Command::Logistic { covariates, .. } => *covariates = list(covariates)?,
        Command::Oneway { dependents, .. } => *dependents = list(dependents)?,
        Command::NparTests(specs) => {
            for spec in specs {
                match spec {
                    NparSpec::MannWhitney { variables, .. } | NparSpec::KruskalWallis { variables, .. } => *variables = list(variables)?,
                    NparSpec::Wilcoxon { pairs } => *pairs = pair_list(ds, pairs, "NPAR TESTS")?,
                }
            }
        }
        _ => {}
    }
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mksigma_core::Variable;

    fn ds() -> Dataset {
        let mut ds = Dataset::new();
        for name in ["id", "q1", "q2", "q3", "renda"] {
            ds.add_variable(Variable::numeric(name)).unwrap();
        }
        ds
    }

    fn list(words: &str) -> Vec<String> {
        words.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn expands_all_and_to() {
        let ds = ds();
        assert_eq!(names(&ds, &list("all")).unwrap(), list("id q1 q2 q3 renda"));
        assert_eq!(names(&ds, &list("renda q1 TO q3")).unwrap(), list("renda q1 q2 q3"));
        assert_eq!(names(&ds, &list("Q2 to q2 id")).unwrap(), list("q2 id"), "a range of one, any case");
        assert!(names(&ds, &list("q3 TO q1")).is_err(), "backwards");
        assert!(names(&ds, &list("q1 TO")).is_err());
        assert!(names(&ds, &list("TO q1")).is_err());
        assert!(matches!(names(&ds, &list("q1 TO nope")), Err(EngineError::UnknownVariable(_))));
    }

    #[test]
    fn new_names_are_numbered_or_existing() {
        let ds = ds();
        assert_eq!(new_names(&ds, &list("x1 TO x3 y")).unwrap(), list("x1 x2 x3 y"));
        assert_eq!(new_names(&ds, &list("v08 to v10")).unwrap(), list("v08 v09 v10"), "the first's digits");
        assert_eq!(new_names(&ds, &list("q1 TO q3")).unwrap(), list("q1 q2 q3"), "existing ones by the dictionary");
        assert_eq!(new_names(&ds, &list("q2 TO q4")).unwrap(), list("q2 q3 q4"), "one new: numbered");
        for bad in ["x TO y", "x1 TO y3", "x3 TO x1", "1 TO 3", "x1 TO", "TO x1", "x1 TO x99999999999999999999", "x0 TO x40000"] {
            assert!(new_names(&ds, &list(bad)).is_err(), "{bad}");
        }
    }
}
