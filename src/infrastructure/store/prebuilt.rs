use indicatif::ProgressBar;

use crate::Manifest;

#[doc(hidden)]
pub fn progress_bar_for(len: u64, terminal: bool) -> ProgressBar {
    if terminal {
        ProgressBar::new(len)
    } else {
        ProgressBar::hidden()
    }
}

#[doc(hidden)]
pub fn github_repository(source: &str) -> Option<(&str, &str)> {
    let rest = source.strip_prefix("https://github.com/")?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let (owner, repository) = rest.split_once('/')?;
    if repository.contains('/') {
        return None;
    }
    let valid = |value: &str| {
        !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    };
    (valid(owner) && valid(repository)).then_some((owner, repository))
}

pub fn prebuilt_target_label() -> Option<&'static str> {
    prebuilt_target_label_for(env!("DM_HOST_TARGET"))
}

#[doc(hidden)]
pub fn prebuilt_target_label_for(target: &str) -> Option<&'static str> {
    match target {
        "aarch64-apple-darwin" => Some("aarch64-macos"),
        "x86_64-apple-darwin" => Some("x86_64-macos"),
        "x86_64-unknown-linux-gnu" | "x86_64-unknown-linux-musl" => Some("x86_64-linux"),
        "aarch64-unknown-linux-gnu" | "aarch64-unknown-linux-musl" => Some("aarch64-linux"),
        "x86_64-pc-windows-msvc" => Some("x86_64-windows"),
        _ => None,
    }
}

#[doc(hidden)]
pub fn release_tag_candidates(manifest: &Manifest, revision: Option<&str>) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(revision) = revision {
        if revision.starts_with('v') {
            candidates.push(revision.to_owned());
        } else if semver::Version::parse(revision).is_ok() {
            candidates.push(format!("v{revision}"));
        }
    }
    candidates.push(format!("v{}", manifest.version));
    candidates.dedup();
    candidates
}

#[doc(hidden)]
pub fn versions_differ(installed: &str, available: &str) -> bool {
    match (
        semver::Version::parse(installed),
        semver::Version::parse(available),
    ) {
        (Ok(installed), Ok(available)) => available > installed,
        _ => installed != available,
    }
}
