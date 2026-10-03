use anyhow::{Context, Result, ensure};
use semver::Version;
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

pub fn normalize_tag(version: &str) -> Result<String> {
    let value = version.strip_prefix('v').unwrap_or(version);
    let parsed = Version::parse(value)?;
    Ok(format!("v{parsed}"))
}

pub fn validate_repository(repository: &str) -> Result<()> {
    ensure!(
        repository.split('/').count() == 2
            && repository
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric()
                    || matches!(byte, b'/' | b'-' | b'_' | b'.')),
        "Self-update repository '{repository}' must be in owner/repository form"
    );
    Ok(())
}

pub fn verify_checksum(archive: &[u8], checksum: &[u8]) -> Result<()> {
    verify_digest(
        &crate::support::codec::hex(&Sha256::digest(archive)),
        checksum,
    )
}

fn verify_digest(actual: &str, checksum: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(checksum)?.trim();
    let expected = text
        .split_whitespace()
        .next()
        .context("Empty SHA-256 file")?;
    ensure!(
        expected.len() == 64 && expected.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Invalid SHA-256 file"
    );
    ensure!(
        actual.eq_ignore_ascii_case(expected),
        "Release SHA-256 mismatch"
    );
    Ok(())
}

/// Verify an archive using a fixed 64 KiB buffer instead of loading it in memory.
pub fn verify_checksum_file(archive: &Path, checksum: &Path) -> Result<()> {
    let checksum = crate::support::bounded::file(checksum, 4096)?;
    let mut file = std::fs::File::open(archive)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    verify_digest(&crate::support::codec::hex(&hasher.finalize()), &checksum)
}
