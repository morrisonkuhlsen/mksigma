//! `DISPLAY DICTIONARY`: the Variable Information and Variable Values tables.

use mksigma_core::{Dataset, Measure, MissingValues, Value};
use mksigma_output::{Cell, Table};

fn value_text(v: &Value) -> String {
    match v {
        Value::Num(n) => n.to_string(),
        Value::Str(s) => format!("\"{s}\""),
        Value::SysMis => ".".to_string(),
    }
}

fn missing_text(m: &MissingValues) -> String {
    match m {
        MissingValues::None => String::new(),
        MissingValues::Discrete(list) => list.iter().map(value_text).collect::<Vec<_>>().join(", "),
        MissingValues::Range { low, high, discrete } => {
            let range = format!("{low} thru {high}");
            discrete.map_or(range.clone(), |d| format!("{range}, {d}"))
        }
    }
}

pub(crate) fn tables(ds: &Dataset) -> Vec<Table> {
    let info = Table {
        title: "Variable Information".to_string(),
        columns: ["Position", "Label", "Measurement Level", "Format", "Missing Values"].map(String::from).to_vec(),
        rows: ds
            .variables()
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let measure = match v.measure {
                    Measure::Nominal => "Nominal",
                    Measure::Ordinal => "Ordinal",
                    Measure::Scale => "Scale",
                };
                let cells = vec![
                    Cell::Int(i64::try_from(i + 1).unwrap_or(i64::MAX)),
                    Cell::Text(v.label.clone().unwrap_or_default()),
                    Cell::Text(measure.to_string()),
                    Cell::Text(v.format.to_string()),
                    Cell::Text(missing_text(&v.missing)),
                ];
                (v.name.clone(), cells)
            })
            .collect(),
        categories: Vec::new(),
    };
    let mut tables = vec![info];
    let labelled: Vec<_> = ds.variables().iter().filter(|v| !v.value_labels.is_empty()).collect();
    if !labelled.is_empty() {
        let mut rows = Vec::new();
        for v in labelled {
            for (value, label) in &v.value_labels {
                let missing = if v.is_missing(value) { "a" } else { "" };
                rows.push((v.name.clone(), vec![Cell::Text(format!("{}{missing}", value_text(value))), Cell::Text(label.clone())]));
            }
        }
        tables.push(Table {
            title: "Variable Values".to_string(),
            columns: vec!["Value".to_string(), "Label".to_string()],
            rows,
            categories: Vec::new(),
        });
    }
    tables
}
