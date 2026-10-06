//! GitHub-provided release asset digests; no checksum attachments are required.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{path::Path, process::Command, time::Duration};

#[derive(Deserialize)]
struct Release {
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    digest: Option<String>,
}

/// Fetch a release and find the digest for the exact selected asset.
/// Only a genuine HTTP 404 or absent asset permits trying another candidate.
pub(crate) fn asset_digest(repository: &str, tag: &str, name: &str) -> Result<Option<String>> {
    let temp = tempfile::tempdir()?;
    let metadata = temp.path().join("release.json");
    let url = format!("https://api.github.com/repos/{repository}/releases/tags/{tag}");
    let output = crate::support::process::capture(
        Command::new("curl")
            .args([
                "-q",
                "-sSL",
                "--connect-timeout",
                "15",
                "--max-time",
                "120",
                "-H",
                "Accept: application/vnd.github+json",
                "--output",
            ])
            .arg(&metadata)
            .args(["-w", "%{http_code}"])
            .arg(&url),
        Duration::from_secs(180),
    )
    .context("Read GitHub Release metadata requires curl")?;
    ensure!(
        output.status.success(),
        "GitHub Release metadata download failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let status = String::from_utf8_lossy(&output.stdout);
    if status.trim() == "404" {
        return Ok(None);
    }
    ensure!(
        status.trim() == "200",
        "GitHub Release metadata HTTP {}",
        status.trim()
    );
    digest_from_metadata(
        &crate::support::bounded::file(&metadata, 1024 * 1024)?,
        name,
    )
}

fn digest_from_metadata(metadata: &[u8], name: &str) -> Result<Option<String>> {
    let release: Release =
        serde_json::from_slice(metadata).context("Parse GitHub Release metadata")?;
    let mut matching = release
        .assets
        .into_iter()
        .filter(|asset| asset.name == name);
    let Some(asset) = matching.next() else {
        return Ok(None);
    };
    ensure!(
        matching.next().is_none(),
        "Duplicate GitHub Release asset {name}"
    );
    let digest = asset
        .digest
        .context("GitHub Release asset has no SHA-256 digest")?;
    let hash = digest
        .strip_prefix("sha256:")
        .context("GitHub Release asset digest must use sha256")?;
    ensure!(
        hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Invalid GitHub Release SHA-256 digest"
    );
    Ok(Some(hash.to_owned()))
}

pub(crate) fn verify_file(path: &Path, hash: &str) -> Result<()> {
    // Reuse the bounded-memory file verifier with the API-provided expected hash.
    crate::infrastructure::self_update::verify_checksum_hash(path, hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_asset_and_valid_digest_are_required() {
        let hash = "a".repeat(64);
        let metadata = format!(r#"{{"assets":[{{"name":"wanted","digest":"sha256:{hash}"}}]}}"#);
        assert_eq!(
            digest_from_metadata(metadata.as_bytes(), "wanted").unwrap(),
            Some(hash)
        );
        assert_eq!(
            digest_from_metadata(metadata.as_bytes(), "absent").unwrap(),
            None
        );
        for digest in [
            serde_json::Value::Null,
            serde_json::json!("md5:abcd"),
            serde_json::json!("sha256:bad"),
        ] {
            let metadata = serde_json::json!({"assets":[{"name":"wanted","digest":digest}]});
            assert!(
                digest_from_metadata(&serde_json::to_vec(&metadata).unwrap(), "wanted").is_err()
            );
        }
        assert!(digest_from_metadata(b"not json", "wanted").is_err());
        assert!(
            digest_from_metadata(
                br#"{"assets":[{"name":"wanted"},{"name":"wanted"}]}"#,
                "wanted"
            )
            .is_err()
        );
    }
    #[test]
    fn downloaded_bytes_must_match_digest() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("binary");
        std::fs::write(&file, b"hello").unwrap();
        assert!(
            verify_file(
                &file,
                "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
            )
            .is_ok()
        );
        assert!(verify_file(&file, &"0".repeat(64)).is_err());
    }
}
