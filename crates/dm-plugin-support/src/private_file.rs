//! Writing export files without ever overwriting an existing file.

use anyhow::{Context, Result};
use std::{fs, io::Write, path::Path};

/// Implementation detail exposed for the tests; writes an export file.
#[doc(hidden)]
pub fn write_private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    write_private_file_with(path, bytes, |output, contents| output.write_all(contents))
}

/// Implementation detail exposed for the tests; writes an export file by step.
#[doc(hidden)]
pub fn write_private_file_with(
    path: &Path,
    bytes: &[u8],
    write: impl FnOnce(&mut fs::File, &[u8]) -> std::io::Result<()>,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("Create {}", parent.display()))?;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut output = options.open(path).with_context(|| {
        format!(
            "Create export file {} without overwriting an existing file",
            path.display()
        )
    })?;
    if let Err(error) = write(&mut output, bytes) {
        drop(output);
        fs::remove_file(path)
            .with_context(|| format!("Remove incomplete export file {}", path.display()))?;
        return Err(error).with_context(|| format!("Write {}", path.display()));
    }
    Ok(())
}
