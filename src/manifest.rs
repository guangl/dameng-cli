use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const MANIFEST_FILE: &str = "dm-plugin.toml";
pub use dm_plugin_sdk::API_VERSION;
const RESERVED_NAMES: &[&str] = &["install", "uninstall", "list", "help", "version"];

#[derive(Debug, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub api_version: u32,
}

pub fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty() && name.len() <= 64,
        "Plugin name must contain 1–64 characters"
    );
    ensure!(
        name.as_bytes()[0].is_ascii_lowercase()
            && name
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'),
        "Plugin name must start with a lowercase letter and contain only a-z, 0-9 or '-'"
    );
    ensure!(
        !RESERVED_NAMES.contains(&name),
        "Plugin name '{name}' is reserved"
    );
    ensure!(
        !matches!(name, "con" | "prn" | "aux" | "nul")
            && !(name.len() == 4
                && (name.starts_with("com") || name.starts_with("lpt"))
                && matches!(name.as_bytes()[3], b'1'..=b'9')),
        "Plugin name '{name}' is not portable"
    );
    Ok(())
}

impl Manifest {
    pub fn read(root: &Path) -> Result<Self> {
        let path = root.join(MANIFEST_FILE);
        let metadata =
            fs::symlink_metadata(&path).with_context(|| format!("Read {}", path.display()))?;
        ensure!(metadata.is_file(), "Manifest must be a regular file");
        ensure!(metadata.len() <= 64 * 1024, "Manifest exceeds 64 KiB");
        Self::from_toml(&fs::read_to_string(&path)?)
            .with_context(|| format!("Invalid manifest {}", path.display()))
    }

    pub(crate) fn from_toml(text: &str) -> Result<Self> {
        let manifest: Self = toml::from_str(text)?;
        validate_name(&manifest.name)?;
        ensure!(
            manifest.api_version == API_VERSION,
            "Unsupported plugin API {}; this host supports {API_VERSION}",
            manifest.api_version
        );
        ensure!(
            !manifest.version.trim().is_empty() && !manifest.version.chars().any(char::is_control),
            "Version must be nonempty and contain no control characters"
        );
        ensure!(
            !manifest.description.chars().any(char::is_control),
            "Description must be a single line"
        );
        Ok(manifest)
    }

    pub fn binary_name(&self) -> String {
        format!("dm-{}", self.name)
    }

    pub fn executable_name(&self) -> String {
        format!("{}{}", self.binary_name(), std::env::consts::EXE_SUFFIX)
    }

    pub fn entrypoint(&self, root: &Path) -> Result<PathBuf> {
        let path = root.join(self.executable_name());
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("Missing executable {}", path.display()))?;
        ensure!(
            metadata.is_file(),
            "Executable must be a regular file, not a symlink"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            ensure!(
                metadata.permissions().mode() & 0o111 != 0,
                "Plugin is not executable"
            );
        }
        Ok(path)
    }
}
