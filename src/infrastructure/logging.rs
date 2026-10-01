//! File-only daily diagnostics with a fixed 30-day retention window.
mod files;
mod writer;
pub use writer::DailyLogWriter;

use jiff::civil::Date;
use std::{
    io,
    path::{Path, PathBuf},
};

/// Default directory below the host data directory.
pub const LOG_DIRECTORY: &str = "logs";
/// Maximum size of each daily file (5 MiB by default).
pub const LOG_FILE_LIMIT: u64 = 5 * 1024 * 1024;
/// Current day and the preceding 29 local calendar days are retained.
pub const LOG_RETENTION_DAYS: u32 = 30;

pub fn daily_file_name(date: Date) -> String {
    format!("dm-{date}.log")
}

/// Today's default log path.
pub fn log_file_path(home: &Path) -> PathBuf {
    home.join(LOG_DIRECTORY)
        .join(daily_file_name(jiff::Zoned::now().date()))
}

pub fn open_log_file(home: &Path) -> io::Result<DailyLogWriter> {
    open_log_file_with(home, LOG_FILE_LIMIT)
}

pub fn open_log_file_with(home: &Path, limit: u64) -> io::Result<DailyLogWriter> {
    DailyLogWriter::new(&home.join(LOG_DIRECTORY), limit)
}

pub fn init(filter: &str, home: &Path) {
    init_at(filter, &home.join(LOG_DIRECTORY), LOG_FILE_LIMIT);
}

/// Never fall back to stdout/stderr, including when opening the file fails.
pub fn init_at(filter: &str, directory: &Path, limit: u64) {
    if filter.trim().eq_ignore_ascii_case("off") {
        init_disabled();
        return;
    }
    let mut validation = env_filter::Builder::new();
    if validation.try_parse(filter).is_err() {
        init_disabled();
        return;
    }
    let Ok(writer) = DailyLogWriter::new(directory, limit) else {
        init_disabled();
        return;
    };
    env_logger::Builder::new()
        .parse_filters(filter)
        .format_timestamp_secs()
        .format_target(false)
        .target(env_logger::Target::Pipe(Box::new(writer)))
        .init();
}

pub fn init_disabled() {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Off)
        .target(env_logger::Target::Pipe(Box::new(io::sink())))
        .init();
}

/// Compatibility entry point: diagnostics without a destination are discarded.
pub fn init_stderr(_filter: &str) {
    init_disabled();
}
