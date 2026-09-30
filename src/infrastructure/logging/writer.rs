use fs2::FileExt;
use jiff::civil::Date;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// One bounded file per local calendar day, with 30 days of retention.
pub struct DailyLogWriter {
    directory: PathBuf,
    lock: File,
    limit: u64,
    cleaned: Option<Date>,
    clock: Box<dyn Fn() -> Date + Send>,
}

impl DailyLogWriter {
    pub fn new(directory: &Path, limit: u64) -> io::Result<Self> {
        Self::with_clock(directory, limit, || jiff::Zoned::now().date())
    }

    /// Deterministic date source for retention and midnight tests.
    #[doc(hidden)]
    pub fn with_clock(
        directory: &Path,
        limit: u64,
        clock: impl Fn() -> Date + Send + 'static,
    ) -> io::Result<Self> {
        if limit == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Log size limit must be positive",
            ));
        }
        fs::create_dir_all(directory)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join(".dm-log.lock"))?;
        if !lock.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Log lock must be a regular file",
            ));
        }
        Ok(Self {
            directory: directory.into(),
            lock,
            limit,
            cleaned: None,
            clock: Box::new(clock),
        })
    }

    fn write_locked(&mut self, bytes: &[u8]) -> io::Result<()> {
        let date = (self.clock)();
        if self.cleaned != Some(date) {
            super::files::cleanup(&self.directory, date)?;
            self.cleaned = Some(date);
        }
        let path = self.directory.join(super::daily_file_name(date));
        // Refuse symlinks and devices, including redirects to stdout/stderr.
        let length = match fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_file() => meta.len(),
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Log must be a regular file",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error),
        };
        // Very large records keep only their final bytes, without allocating a copy.
        let mut start = bytes
            .len()
            .saturating_sub(self.limit.min(usize::MAX as u64) as usize);
        while start < bytes.len() && bytes[start] & 0xc0 == 0x80 {
            start += 1;
        }
        let bytes = &bytes[start..];
        if length > self.limit.saturating_sub(bytes.len() as u64) {
            let keep = (self.limit / 2).min(self.limit - bytes.len() as u64);
            super::files::compact(&path, keep)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Log must be a regular file",
            ));
        }
        file.write_all(bytes)
    }
}

impl Write for DailyLogWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        // Logging is best effort. Never propagate write errors to env_logger,
        // which may otherwise report them on the terminal.
        if FileExt::lock_exclusive(&self.lock).is_ok() {
            let _ = self.write_locked(bytes);
            let _ = FileExt::unlock(&self.lock);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
