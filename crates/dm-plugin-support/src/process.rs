//! Bounded output and wall-clock deadlines for noninteractive helper processes.
use std::{
    io,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

pub const OUTPUT_LIMIT: u64 = 64 * 1024;

pub fn capture(command: &mut Command, timeout: Duration) -> io::Result<Output> {
    let stdout = tempfile::NamedTempFile::new()?;
    let stderr = tempfile::NamedTempFile::new()?;
    let mut child = ReapedChild(
        command
            .stdin(Stdio::null())
            .stdout(stdout.reopen()?)
            .stderr(stderr.reopen()?)
            .spawn()?,
    );
    let start = Instant::now();
    let status = loop {
        let oversized = stdout.as_file().metadata()?.len() > OUTPUT_LIMIT
            || stderr.as_file().metadata()?.len() > OUTPUT_LIMIT;
        if oversized || start.elapsed() >= timeout {
            let _ = child.0.kill();
            let _ = child.0.wait();
            return Err(io::Error::new(
                if oversized {
                    io::ErrorKind::InvalidData
                } else {
                    io::ErrorKind::TimedOut
                },
                if oversized {
                    "Helper process output exceeds 64 KiB"
                } else {
                    "Helper process timed out"
                },
            ));
        }
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Ok(Output {
        status,
        stdout: crate::bounded::file(stdout.path(), OUTPUT_LIMIT)?,
        stderr: crate::bounded::file(stderr.path(), OUTPUT_LIMIT)?,
    })
}

struct ReapedChild(std::process::Child);
impl Drop for ReapedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
