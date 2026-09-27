//! Where the host writes its diagnostics.
//!
//! Host logs go to \`<DM_PLUGIN_HOME>/dm.log\` so stdout stays machine-readable and
//! stderr keeps only what a human has to see: the progress bar, plugin output and
//! the \`错误\`/\`详情\`/\`提示\` report. The file is opened once at startup and rotated
//! to \`dm.log.1\` when it outgrew the size limit. When it cannot be opened (an
//! unwritable data directory, for example) the host says so on stderr and logs
//! there instead of failing the command.

use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

/// Log file inside the host data directory.
pub const LOG_FILE: &str = "dm.log";
/// Name the full log file is rotated to on the next start.
pub const ROTATED_LOG_FILE: &str = "dm.log.1";
/// Size at which the next start rotates the log file instead of appending.
pub const LOG_FILE_LIMIT: u64 = 5 * 1024 * 1024;

/// Path of the host log file of one data directory.
pub fn log_file_path(home: &Path) -> PathBuf {
    home.join(LOG_FILE)
}

/// Open the log file of one data directory, rotating it when it is full.
pub fn open_log_file(home: &Path) -> io::Result<File> {
    open_log_file_with(home, LOG_FILE_LIMIT)
}

/// Open the log file, rotating it first when it reached \`limit\` bytes.
///
/// Rotation is best effort: a file that cannot be renamed is still appended to,
/// because losing diagnostics must never fail a command.
pub fn open_log_file_with(home: &Path, limit: u64) -> io::Result<File> {
    fs::create_dir_all(home)?;
    let path = log_file_path(home);
    if fs::metadata(&path).is_ok_and(|metadata| metadata.len() >= limit) {
        let rotated = home.join(ROTATED_LOG_FILE);
        let _ = fs::remove_file(&rotated);
        let _ = fs::rename(&path, &rotated);
    }
    OpenOptions::new().create(true).append(true).open(path)
}

/// Start the logging backend of one data directory.
///
/// \`filter\` uses the same syntax as \`DM_LOG\`; \`off\` logs nothing and leaves the
/// data directory untouched.
pub fn init(filter: &str, home: &Path) {
    if logging_is_off(filter) {
        init_stderr(filter);
        return;
    }
    match open_log_file(home) {
        Ok(file) => builder(filter)
            .target(env_logger::Target::Pipe(Box::new(file)))
            .init(),
        Err(error) => {
            init_stderr(filter);
            log::warn!(
                "cannot write the log file {}: {error}; logging to stderr instead",
                log_file_path(home).display()
            );
        }
    }
}

/// Start the logging backend on stderr, used when no data directory is known.
pub fn init_stderr(filter: &str) {
    builder(filter).target(env_logger::Target::Stderr).init();
}

fn builder(filter: &str) -> env_logger::Builder {
    let mut builder = env_logger::Builder::new();
    builder
        .parse_filters(filter)
        .format_timestamp_secs()
        .format_target(false);
    builder
}

fn logging_is_off(filter: &str) -> bool {
    filter.trim().eq_ignore_ascii_case("off")
}
