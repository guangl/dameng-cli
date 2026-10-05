//! Explicit source builds; all artifacts live outside the source and plugin store.
use anyhow::{Context, Result, ensure};
use std::{
    path::Path,
    process::{Command, Stdio},
};

use super::{InstallMode, PluginStore, resolve_source};
use crate::{Manifest, infrastructure::config::build::validate_toolchain};

/// Toolchain selection for an explicitly requested source build.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// CLI override, taking precedence over the plugin's toolchain file.
    pub toolchain: Option<String>,
    /// Host fallback when neither override nor plugin toolchain file exists.
    pub default_toolchain: String,
    /// Allow installing a missing toolchain without changing rustup's default.
    pub install_toolchain: bool,
}
impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            toolchain: None,
            default_toolchain: env!("CARGO_PKG_RUST_VERSION").into(),
            install_toolchain: false,
        }
    }
}

impl PluginStore {
    /// Compile a local or HTTPS source, then use the normal transactional installer.
    /// Existing prebuilt binaries are ignored. `update` keeps its prebuilt behavior;
    /// to rebuild use this method again with `replace = true`.
    pub fn install_from_source(
        &self,
        source: &str,
        revision: Option<&str>,
        replace: bool,
        options: &BuildOptions,
    ) -> Result<Manifest> {
        let resolved = resolve_source(source, revision, self.progress_enabled())?;
        let manifest = Manifest::read(&resolved.path)?;
        if let Ok(plugins) = std::fs::canonicalize(self.plugins()) {
            ensure!(
                !plugins.starts_with(&resolved.path),
                "Plugin source must not contain the plugin store"
            );
        }
        let mode = if replace {
            InstallMode::Replace
        } else {
            InstallMode::New
        };
        super::conflict::resolve(
            self.installed(&manifest.name)?,
            &self.plugins().join(&manifest.name),
            &manifest.name,
            mode,
        )?;
        ensure!(
            resolved.path.join("Cargo.toml").is_file(),
            "Source build requires Cargo.toml"
        );
        ensure!(
            resolved.path.join("Cargo.lock").is_file(),
            "Source build requires a committed Cargo.lock; generate it before installation"
        );
        let toolchain = select_toolchain(&resolved.path, options)?;
        ensure_toolchain(&resolved.path, &toolchain, options.install_toolchain)?;
        let artifacts = tempfile::tempdir().context("Create isolated build directory")?;
        eprintln!("正在编译 {}（Rust {toolchain}）", manifest.name);
        let status = Command::new("rustup")
            .args([
                "run",
                &toolchain,
                "cargo",
                "build",
                "--release",
                "--locked",
                "--bin",
            ])
            .arg(format!("dm-{}", manifest.name))
            .args([
                "--manifest-path",
                "Cargo.toml",
                "--target",
                env!("DM_HOST_TARGET"),
                "--target-dir",
            ])
            .arg(artifacts.path())
            .current_dir(&resolved.path)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("Run Cargo source build")?;
        ensure!(
            status.success(),
            "Plugin source build failed ({status}); no plugin was installed"
        );
        let binary = artifacts
            .path()
            .join(env!("DM_HOST_TARGET"))
            .join("release")
            .join(manifest.executable_name());
        ensure!(
            binary.is_file(),
            "Cargo did not produce {}",
            binary.display()
        );
        self.install_prepared(
            &resolved.path,
            resolved.recorded_source,
            resolved.revision,
            resolved.source_ref,
            mode,
            Some(&binary),
        )
    }
}

fn select_toolchain(source: &Path, options: &BuildOptions) -> Result<String> {
    if let Some(value) = &options.toolchain {
        validate_toolchain(value)?;
        return Ok(value.clone());
    }
    ensure!(
        !source.join("rust-toolchain").exists(),
        "Legacy rust-toolchain is unsupported; use rust-toolchain.toml or --toolchain <version>"
    );
    let file = source.join("rust-toolchain.toml");
    let value = if file.exists() {
        let text = crate::support::bounded::text(&file, crate::support::bounded::CONFIG_LIMIT)?;
        let value: toml::Value =
            toml::from_str(&text).context("Parse plugin rust-toolchain.toml")?;
        let table = value
            .get("toolchain")
            .and_then(toml::Value::as_table)
            .context("rust-toolchain.toml requires [toolchain]")?;
        ensure!(
            !table.contains_key("path"),
            "Custom toolchain paths are unsupported; use --toolchain <version>"
        );
        table
            .get("channel")
            .and_then(toml::Value::as_str)
            .context("rust-toolchain.toml requires toolchain.channel")?
            .to_owned()
    } else {
        options.default_toolchain.clone()
    };
    validate_toolchain(&value)
        .context("Use a fixed stable toolchain in rust-toolchain.toml or --toolchain <version>")?;
    Ok(value)
}

fn ensure_toolchain(source: &Path, toolchain: &str, install: bool) -> Result<()> {
    let output = Command::new("rustup")
        .args(["which", "--toolchain", toolchain, "cargo"])
        .current_dir(source)
        .stdin(Stdio::null())
        .output()
        .context("Source builds require rustup; install rustup and the requested Rust toolchain")?;
    if output.status.success() {
        return Ok(());
    }
    ensure!(
        install,
        "Rust {toolchain} is unavailable; install it with `rustup toolchain install {toolchain} --profile minimal` or add --install-toolchain"
    );
    let status = Command::new("rustup")
        .args(["toolchain", "install", toolchain, "--profile", "minimal"])
        .current_dir(source)
        .stdin(Stdio::null())
        .status()
        .context("Install Rust toolchain")?;
    ensure!(
        status.success(),
        "Failed to install Rust {toolchain} ({status})"
    );
    Ok(())
}
