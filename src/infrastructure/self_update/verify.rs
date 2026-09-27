use anyhow::{Context, Result, ensure};
use semver::Version;
use sha2::{Digest, Sha256};

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
    let text = std::str::from_utf8(checksum)?.trim();
    let expected = text
        .split_whitespace()
        .next()
        .context("Empty SHA-256 file")?;
    ensure!(
        expected.len() == 64 && expected.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Invalid SHA-256 file"
    );
    let actual = format!("{:x}", Sha256::digest(archive));
    ensure!(
        actual.eq_ignore_ascii_case(expected),
        "Release SHA-256 mismatch"
    );
    Ok(())
}
