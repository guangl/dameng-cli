use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const MANIFEST_FILE: &str = "dm-plugin.toml";
pub use dm_plugin_sdk::API_VERSION;
pub const SUPPORTED_API_VERSIONS: &[u32] = &[API_VERSION];
const RESERVED_NAMES: &[&str] = &[
    "disable",
    "doctor",
    "enable",
    "completions",
    "help",
    "info",
    "install",
    "list",
    "new",
    "outdated",
    "registry",
    "rollback",
    "search",
    "self-update",
    "uninstall",
    "update",
    "verify",
    "version",
];

#[derive(Debug, Clone, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub api_version: u32,
    #[serde(default)]
    pub min_host_version: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    /// Environment variables explicitly inherited by the plugin process.
    #[serde(default)]
    pub environment: Vec<String>,
    /// Declarative permissions shown to users. Native plugins are not sandboxed.
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Lifecycle hooks run by the host.
    #[serde(default)]
    pub hooks: Hooks,
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Hooks {
    /// Run from the source root before the Rust build.
    #[serde(default)]
    pub pre_install: Option<String>,
    /// Run from the installed plugin directory after a successful install.
    #[serde(default)]
    pub post_install: Option<String>,
    /// Run from the installed plugin directory before uninstall.
    #[serde(default)]
    pub pre_uninstall: Option<String>,
    /// Run from the staged package before it is permanently removed.
    #[serde(default)]
    pub post_uninstall: Option<String>,
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

fn validate_hook(hook: &str) -> Result<()> {
    ensure!(!hook.is_empty(), "Hook path must not be empty");
    ensure!(
        !hook.starts_with('/') && !hook.starts_with('\\') && !hook.contains(':'),
        "Hook path must be relative"
    );
    for component in hook.split(['/', '\\']) {
        ensure!(
            !component.is_empty() && component != "." && component != "..",
            "Hook path must be a relative executable path"
        );
        ensure!(
            component
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')),
            "Hook path contains unsupported characters"
        );
    }
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

    /// Parse and validate a manifest from TOML. Public for integration tests.
    #[doc(hidden)]
    pub fn from_toml(text: &str) -> Result<Self> {
        let manifest: Self = toml::from_str(text)?;
        validate_name(&manifest.name)?;
        ensure!(
            SUPPORTED_API_VERSIONS.contains(&manifest.api_version),
            "Unsupported plugin API {}; this host supports {:?}",
            manifest.api_version,
            SUPPORTED_API_VERSIONS
        );
        ensure!(
            !manifest.version.trim().is_empty() && !manifest.version.chars().any(char::is_control),
            "Version must be nonempty and contain no control characters"
        );
        ensure!(
            !manifest.description.chars().any(char::is_control),
            "Description must be a single line"
        );
        if let Some(version) = &manifest.min_host_version {
            let minimum = semver::Version::parse(version).context("Invalid min_host_version")?;
            let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;
            ensure!(
                current >= minimum,
                "Plugin requires dm {minimum} or newer; this host is {current}"
            );
        }
        for variable in &manifest.environment {
            ensure!(
                !variable.is_empty()
                    && variable.bytes().all(|byte| byte.is_ascii_uppercase()
                        || byte.is_ascii_digit()
                        || byte == b'_')
                    && variable.as_bytes()[0].is_ascii_uppercase(),
                "Environment variable '{variable}' must use uppercase ASCII letters, digits and '_'"
            );
        }
        const PERMISSIONS: &[&str] = &["filesystem", "network", "process"];
        for permission in &manifest.permissions {
            ensure!(
                PERMISSIONS.contains(&permission.as_str()),
                "Unknown permission '{permission}'; supported values are filesystem, network and process"
            );
        }
        for hook in [
            manifest.hooks.pre_install.as_deref(),
            manifest.hooks.post_install.as_deref(),
            manifest.hooks.pre_uninstall.as_deref(),
            manifest.hooks.post_uninstall.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            validate_hook(hook)?;
        }
        Ok(manifest)
    }

    /// True when this manifest requests permissions or environment variables the previous manifest did not.
    pub fn requests_consent_from(&self, previous: Option<&Self>) -> bool {
        let Some(previous) = previous else {
            return !self.permissions.is_empty() || !self.environment.is_empty();
        };
        self.permissions
            .iter()
            .any(|permission| !previous.permissions.contains(permission))
            || self
                .environment
                .iter()
                .any(|variable| !previous.environment.contains(variable))
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
