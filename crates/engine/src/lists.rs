//! `ALL` and `first TO last` in variable lists, expanded by the dictionary's order, as in SPSS:
//! `DESCRIPTIVES ALL.`, `FREQUENCIES q1 TO q10.` Analyses expand their lists each time they run,
//! so a filtered rerun or a rerun after new variables sees the current dictionary.

use mksigma_core::Dataset;
use mksigma_i18n::tr;
use mksigma_syntax::{Command, NparSpec, TTestSpec};

use crate::EngineError;

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
        Command::Correlations { variables, with, .. } => {
            *variables = list(variables)?;
            *with = list(with)?;
        }
        Command::Regression { predictors, .. } => *predictors = list(predictors)?,
        Command::Logistic { covariates, .. } => *covariates = list(covariates)?,
        Command::Oneway { dependents, .. } => *dependents = list(dependents)?,
        Command::NparTests(specs) => {
            for spec in specs {
                if let NparSpec::MannWhitney { variables, .. } | NparSpec::KruskalWallis { variables, .. } = spec {
                    *variables = list(variables)?;
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
}
