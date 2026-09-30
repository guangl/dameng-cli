//! Reading the SQL of "dm db exec" and rendering what a statement returned.

use crate::domain::types::QueryResult;
use anyhow::{Context, Result, ensure};
use dm_plugin_support::bounded::{self, DOCUMENT_LIMIT};
use std::path::PathBuf;

/// Read the SQL of "dm db exec" from the argument, a file, or stdin.
pub fn read_sql(sql: Option<String>, file: Option<PathBuf>) -> Result<String> {
    let text = match (sql, file) {
        (Some(sql), _) => sql,
        (None, Some(path)) => bounded::text(&path, DOCUMENT_LIMIT)
            .with_context(|| format!("Read {}", path.display()))?,
        (None, None) => String::from_utf8(bounded::read(std::io::stdin().lock(), DOCUMENT_LIMIT)?)
            .context("Read SQL from stdin")?,
    };
    ensure!(text.len() as u64 <= DOCUMENT_LIMIT, "SQL exceeds 16 MiB");
    let text = text.trim().to_owned();
    ensure!(
        !text.is_empty(),
        "SQL must not be empty; pass it as an argument, through --file, or on stdin"
    );
    Ok(text)
}

/// Render a result set as tab-separated rows; SQL NULL becomes an empty field.
pub fn format_result(result: &QueryResult) -> String {
    let mut output = Vec::new();
    write_result(&mut output, result).expect("Writing to memory cannot fail");
    String::from_utf8(output).expect("Results contain UTF-8 strings")
}

/// Write rows incrementally without allocating a second complete result string.
pub fn write_result(output: &mut impl std::io::Write, result: &QueryResult) -> std::io::Result<()> {
    write_fields(output, result.columns.iter().map(String::as_str))?;
    for row in &result.rows {
        write_fields(
            output,
            row.iter().map(|value| value.as_deref().unwrap_or("")),
        )?;
    }
    Ok(())
}

fn write_fields<'a>(
    output: &mut impl std::io::Write,
    fields: impl Iterator<Item = &'a str>,
) -> std::io::Result<()> {
    for (index, value) in fields.enumerate() {
        if index > 0 {
            output.write_all(b"\t")?;
        }
        output.write_all(value.as_bytes())?;
    }
    output.write_all(b"\n")
}
