//! Reading the SQL of "dm db exec" and rendering what a statement returned.

use crate::domain::types::QueryResult;
use anyhow::{Context, Result, ensure};
use std::{fs, io::Read, path::PathBuf};

/// Read the SQL of "dm db exec" from the argument, a file, or stdin.
pub fn read_sql(sql: Option<String>, file: Option<PathBuf>) -> Result<String> {
    let text = match (sql, file) {
        (Some(sql), _) => sql,
        (None, Some(path)) => {
            fs::read_to_string(&path).with_context(|| format!("Read {}", path.display()))?
        }
        (None, None) => {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .context("Read SQL from stdin")?;
            text
        }
    };
    let text = text.trim().to_owned();
    ensure!(
        !text.is_empty(),
        "SQL must not be empty; pass it as an argument, through --file, or on stdin"
    );
    Ok(text)
}

/// Render a result set as tab-separated rows; SQL NULL becomes an empty field.
pub fn format_result(result: &QueryResult) -> String {
    let mut output = String::new();
    output.push_str(&result.columns.join("\t"));
    output.push('\n');
    for row in &result.rows {
        let values: Vec<&str> = row
            .iter()
            .map(|value| value.as_deref().unwrap_or(""))
            .collect();
        output.push_str(&values.join("\t"));
        output.push('\n');
    }
    output
}
