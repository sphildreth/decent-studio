//! Data export for result sets.
//!
//! Supports CSV, JSON, Markdown tables, and SQL `INSERT` statements. Used by
//! the "Export results" action and by the data-browser save-to-file flow.

use decentdb::Value;

use crate::db::value;
use crate::db::{quote_ident, ResultSet};

/// Supported export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Csv,
    Json,
    Markdown,
    SqlInsert,
}

impl Format {
    pub const ALL: &'static [Format] = &[
        Format::Csv,
        Format::Json,
        Format::Markdown,
        Format::SqlInsert,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Format::Csv => "CSV",
            Format::Json => "JSON",
            Format::Markdown => "Markdown",
            Format::SqlInsert => "SQL INSERT",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Csv => "csv",
            Format::Json => "json",
            Format::Markdown => "md",
            Format::SqlInsert => "sql",
        }
    }
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Render a [`ResultSet`] in the requested [`Format`].
///
/// `table_name` is used only by [`Format::SqlInsert`] to qualify the generated
/// statements; callers may pass `"exported_table"` when unknown.
pub fn export(result: &ResultSet, format: Format, table_name: &str) -> String {
    match format {
        Format::Csv => to_csv(result),
        Format::Json => to_json(result),
        Format::Markdown => to_markdown(result),
        Format::SqlInsert => to_sql_insert(result, table_name),
    }
}

fn to_csv(result: &ResultSet) -> String {
    let mut wtr = csv::Writer::from_writer(Vec::new());
    let _ = wtr.write_record(&result.columns);
    for row in &result.rows {
        let record: Vec<String> = row
            .iter()
            .map(|v| if value::is_null(v) { String::new() } else { value::display(v) })
            .collect();
        let _ = wtr.write_record(&record);
    }
    let bytes = wtr.into_inner().unwrap_or_default();
    String::from_utf8(bytes).unwrap_or_default()
}

fn to_json(result: &ResultSet) -> String {
    let mut out = String::from("[\n");
    for (ri, row) in result.rows.iter().enumerate() {
        out.push_str("  {");
        for (ci, col) in result.columns.iter().enumerate() {
            if ci > 0 {
                out.push_str(", ");
            }
            out.push_str(&json_escape(col));
            out.push_str(": ");
            out.push_str(&json_value(&row[ci]));
        }
        out.push('}');
        if ri + 1 < result.rows.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push(']');
    out
}

fn json_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Int64(v) => v.to_string(),
        Value::Float64(v) => {
            if v.is_finite() {
                v.to_string()
            } else {
                "null".to_string()
            }
        }
        Value::Bool(v) => v.to_string(),
        other => json_escape(&value::display(other)),
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn to_markdown(result: &ResultSet) -> String {
    let mut out = String::new();
    out.push_str("| ");
    out.push_str(&result.columns.join(" | "));
    out.push_str(" |\n|");
    for _ in &result.columns {
        out.push_str(" --- |");
    }
    out.push('\n');
    for row in &result.rows {
        out.push_str("| ");
        let cells: Vec<String> = row
            .iter()
            .map(|v| value::display(v).replace('|', "\\|").replace('\n', " "))
            .collect();
        out.push_str(&cells.join(" | "));
        out.push_str(" |\n");
    }
    out
}

fn to_sql_insert(result: &ResultSet, table_name: &str) -> String {
    let mut out = String::new();
    let columns = result
        .columns
        .iter()
        .map(|c| quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");
    for row in &result.rows {
        let values = row
            .iter()
            .map(value::sql_literal)
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "INSERT INTO {} ({}) VALUES ({});\n",
            quote_ident(table_name),
            columns,
            values
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ResultSet {
        ResultSet {
            columns: vec!["id".into(), "name".into()],
            rows: vec![
                vec![Value::Int64(1), Value::Text("Ada".into())],
                vec![Value::Int64(2), Value::Null],
            ],
            affected_rows: 0,
            is_query: true,
            elapsed_ms: 0.0,
            statement: String::new(),
        }
    }

    #[test]
    fn csv_export() {
        let csv = to_csv(&sample());
        assert!(csv.contains("id,name"));
        assert!(csv.contains("1,Ada"));
    }

    #[test]
    fn json_export() {
        let json = to_json(&sample());
        assert!(json.contains("\"name\": \"Ada\""));
        assert!(json.contains("null"));
    }

    #[test]
    fn sql_insert_export() {
        let sql = to_sql_insert(&sample(), "users");
        assert!(sql.contains("INSERT INTO \"users\""));
        assert!(sql.contains("'Ada'"));
    }
}
