//! Tests for reading the exec SQL and rendering its result.

use dm_plugin_db::{QueryResult, format_result, read_sql};
use std::{fs, path::PathBuf};
use tempfile::TempDir;

#[test]
fn sql_is_read_from_argument_file_or_rejected() {
    assert_eq!(
        read_sql(Some(" SELECT 1 ".to_owned()), None).unwrap(),
        "SELECT 1"
    );
    assert!(read_sql(Some("   ".to_owned()), None).is_err());

    let temp = TempDir::new().unwrap();
    let file = temp.path().join("query.sql");
    fs::write(&file, "SELECT 2 FROM DUAL;\n").unwrap();
    assert_eq!(read_sql(None, Some(file)).unwrap(), "SELECT 2 FROM DUAL;");

    let missing = temp.path().join("missing.sql");
    assert!(read_sql(None, Some(missing)).is_err());
    // An explicit statement wins over a file that does not exist.
    assert_eq!(
        read_sql(
            Some("SELECT 3".to_owned()),
            Some(PathBuf::from("/missing.sql"))
        )
        .unwrap(),
        "SELECT 3"
    );
}

#[test]
fn results_are_rendered_as_tab_separated_rows() {
    let result = QueryResult {
        columns: vec!["ID".to_owned(), "NAME".to_owned()],
        rows: vec![
            vec![Some("1".to_owned()), Some("DM".to_owned())],
            vec![Some("2".to_owned()), None],
        ],
    };
    assert_eq!(format_result(&result), "ID\tNAME\n1\tDM\n2\t\n");
    assert_eq!(
        format_result(&QueryResult::default()),
        "\n",
        "a result without columns still prints one empty header line"
    );
}

#[test]
fn oversized_sql_is_rejected_before_execution() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("large.sql");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(16 * 1024 * 1024 + 1).unwrap();
    assert!(
        read_sql(None, Some(path))
            .unwrap_err()
            .to_string()
            .contains("Read")
    );
    assert!(
        read_sql(Some("x".repeat(16 * 1024 * 1024 + 1)), None)
            .unwrap_err()
            .to_string()
            .contains("16 MiB")
    );
}

#[test]
fn streaming_results_preserve_format_and_propagate_output_errors() {
    let result = QueryResult {
        columns: vec!["a".into()],
        rows: vec![vec![Some("value".into())], vec![None]],
    };
    let mut output = Vec::new();
    dm_plugin_db::write_result(&mut output, &result).unwrap();
    assert_eq!(output, b"a\nvalue\n\n");
    assert!(dm_plugin_db::write_result(&mut &mut [0_u8; 1][..], &result).is_err());
}
