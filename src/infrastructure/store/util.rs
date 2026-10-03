use anyhow::Result;
use sha2::{Digest, Sha256};
use std::{env, fs, io::Read, path::Path, process::Command};

use crate::Manifest;

pub(crate) fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(crate::support::codec::hex(&hasher.finalize()))
}

pub(crate) fn inherit_safe_environment(
    command: &mut Command,
    manifest: &Manifest,
    extra: &[String],
) {
    const SAFE: &[&str] = &[
        "COLORTERM",
        "COMSPEC",
        "HOME",
        "LANG",
        "NO_COLOR",
        "PATH",
        "PATHEXT",
        "SYSTEMROOT",
        "TEMP",
        "TERM",
        "TMP",
        "TMPDIR",
        "TZ",
        "USERPROFILE",
        "WINDIR",
    ];
    for (name, value) in env::vars_os() {
        let text = name.to_string_lossy();
        if SAFE.contains(&text.as_ref())
            || text.starts_with("LC_")
            || manifest.environment.iter().any(|allowed| allowed == &text)
            || extra.iter().any(|allowed| allowed == &text)
        {
            command.env(name, value);
        }
    }
}
