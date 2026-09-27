//! Unit tests for the host log file.

use dameng_cli::logging::{LOG_FILE, ROTATED_LOG_FILE, log_file_path, open_log_file_with};
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::TempDir;

fn append(home: &Path, limit: u64, text: &str) {
    let mut file = open_log_file_with(home, limit).unwrap();
    file.write_all(text.as_bytes()).unwrap();
}

#[test]
fn log_file_is_created_appended_and_rotated() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    assert_eq!(log_file_path(&home), home.join(LOG_FILE));

    // The first write creates the data directory and the file.
    append(&home, 64, "first\n");
    assert_eq!(fs::read_to_string(home.join(LOG_FILE)).unwrap(), "first\n");

    // A file that still fits is appended to.
    append(&home, 64, "second\n");
    assert_eq!(
        fs::read_to_string(home.join(LOG_FILE)).unwrap(),
        "first\nsecond\n"
    );

    // Past the limit the next open rotates the previous content away.
    append(&home, 4, "third\n");
    assert_eq!(
        fs::read_to_string(home.join(ROTATED_LOG_FILE)).unwrap(),
        "first\nsecond\n"
    );
    assert_eq!(fs::read_to_string(home.join(LOG_FILE)).unwrap(), "third\n");

    // A later rotation replaces the rotated file instead of failing.
    append(&home, 1, "fourth\n");
    assert_eq!(
        fs::read_to_string(home.join(ROTATED_LOG_FILE)).unwrap(),
        "third\n"
    );
    assert_eq!(fs::read_to_string(home.join(LOG_FILE)).unwrap(), "fourth\n");
}

#[test]
fn log_file_reports_an_unusable_data_directory() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("store.sqlite3");
    fs::write(&file, b"not a directory").unwrap();
    assert!(open_log_file_with(&file, 64).is_err());
}
