use crate::{API_VERSION, Manifest, manifest::validate_name};
use anyhow::{Context, Result, bail, ensure};
use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// An explicit store path makes embedding and tests independent of user state.
pub struct PluginStore {
    home: PathBuf,
}

impl PluginStore {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }

    pub fn from_env() -> Result<Self> {
        let home = if let Some(home) = env::var_os("DM_HOME") {
            ensure!(!home.is_empty(), "DM_HOME must not be empty");
            PathBuf::from(home)
        } else if cfg!(windows) {
            PathBuf::from(env::var_os("LOCALAPPDATA").context("Set DM_HOME or LOCALAPPDATA")?)
                .join("dm")
        } else if let Some(data) = env::var_os("XDG_DATA_HOME").filter(|s| !s.is_empty()) {
            let data = PathBuf::from(data);
            ensure!(data.is_absolute(), "XDG_DATA_HOME must be absolute");
            data.join("dm")
        } else {
            PathBuf::from(env::var_os("HOME").context("Set DM_HOME or HOME")?)
                .join(".local/share/dm")
        };
        Ok(Self::new(if home.is_absolute() {
            home
        } else {
            env::current_dir()?.join(home)
        }))
    }

    fn plugins(&self) -> PathBuf {
        self.home.join("plugins")
    }

    pub fn install(&self, source: &str) -> Result<Manifest> {
        if Path::new(source).is_dir() {
            return self.install_directory(Path::new(source));
        }
        let requested_name = (!source.starts_with("https://")).then_some(source);
        let resolved;
        let source = if source.starts_with("https://") {
            source
        } else {
            validate_name(source)?;
            let path = self.home.join("registry.toml");
            let registry: Registry = toml::from_str(&fs::read_to_string(&path)
                .with_context(|| format!("Unknown plugin '{source}'; use a local directory, HTTPS Git URL, or configure {}", path.display()))?)?;
            resolved = registry
                .plugins
                .get(source)
                .with_context(|| format!("Plugin '{source}' is not registered"))?
                .clone();
            &resolved
        };
        ensure!(
            source.starts_with("https://") && source.len() > 8,
            "Source must be a local plugin directory or HTTPS Git repository URL"
        );
        let checkout = tempfile::tempdir()?;
        let destination = checkout.path().join("source");
        let status = Command::new("git")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "protocol.allow=never",
                "-c",
                "protocol.https.allow=always",
                "clone",
                "--depth",
                "1",
                "--",
                source,
            ])
            .arg(&destination)
            .env("GIT_TERMINAL_PROMPT", "0")
            .status()
            .context("Fetch plugin; HTTPS installation requires Git")?;
        ensure!(status.success(), "Git could not fetch the plugin");
        if let Some(name) = requested_name {
            ensure!(
                Manifest::read(&destination)?.name == name,
                "Registry name '{name}' does not match the fetched plugin manifest"
            );
        }
        self.install_directory(&destination)
    }

    fn install_directory(&self, source: &Path) -> Result<Manifest> {
        let source = fs::canonicalize(source)?;
        let manifest = Manifest::read(&source)?;
        fs::create_dir_all(self.plugins())?;
        let plugins = fs::canonicalize(self.plugins())?;
        ensure!(
            !plugins.starts_with(&source),
            "Plugin source must not contain the plugin store"
        );
        let destination = plugins.join(&manifest.name);
        ensure!(
            fs::symlink_metadata(&destination)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
            "Plugin '{}' is already installed; uninstall it first",
            manifest.name
        );
        let stage = tempfile::Builder::new()
            .prefix(".install-")
            .tempdir_in(&plugins)?;
        let package = stage.path().join("package");
        validate_crate(&source, &manifest)?;
        let build = stage.path().join("build");
        let status = Command::new("cargo")
            .args(["build", "--release", "--locked", "--manifest-path"])
            .arg(source.join("Cargo.toml"))
            .args([
                "--bin",
                &manifest.binary_name(),
                "--target",
                env!("DM_HOST_TARGET"),
                "--target-dir",
            ])
            .arg(&build)
            .current_dir(&source)
            .status()
            .context("Build Rust plugin; install the Rust toolchain and Cargo first")?;
        ensure!(
            status.success(),
            "Rust plugin build failed; no plugin was installed"
        );
        ensure!(
            Manifest::read(&source)? == manifest,
            "Plugin manifest changed during build"
        );
        fs::create_dir(&package)?;
        fs::copy(
            build
                .join(env!("DM_HOST_TARGET"))
                .join("release")
                .join(manifest.executable_name()),
            package.join(manifest.executable_name()),
        )
        .context("Copy compiled Rust plugin")?;
        fs::write(
            package.join(crate::MANIFEST_FILE),
            toml::to_string(&manifest)?,
        )?;
        manifest.entrypoint(&package)?;
        fs::rename(&package, &destination).context("Publish installed plugin")?;
        Ok(manifest)
    }

    pub fn list(&self) -> Result<Vec<Manifest>> {
        if !self.plugins().try_exists()? {
            return Ok(Vec::new());
        }
        let mut result = Vec::new();
        for item in fs::read_dir(self.plugins())? {
            let item = item?;
            if item.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let name = item
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("Invalid installed plugin name"))?;
            let (_, manifest) = self.load(&name)?;
            result.push(manifest);
        }
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    fn load(&self, name: &str) -> Result<(PathBuf, Manifest)> {
        validate_name(name)?;
        let root = self.plugins().join(name);
        let metadata = fs::symlink_metadata(&root).with_context(|| {
            format!("Plugin '{name}' is not installed; use dm install <source>")
        })?;
        ensure!(
            metadata.is_dir(),
            "Installed plugin must be a regular directory"
        );
        let root = fs::canonicalize(root)?;
        let manifest = Manifest::read(&root)?;
        ensure!(
            manifest.name == name,
            "Installed plugin directory and manifest name differ"
        );
        Ok((root, manifest))
    }

    pub fn uninstall(&self, name: &str) -> Result<()> {
        validate_name(name)?;
        let path = self.plugins().join(name);
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("Plugin '{name}' is not installed"))?;
        ensure!(
            metadata.is_dir(),
            "Installed plugin must be a regular directory"
        );
        // Removal must also work for a damaged manifest or missing executable.
        fs::remove_dir_all(path).context("Remove plugin")
    }

    pub fn run(&self, name: &str, args: &[OsString]) -> Result<i32> {
        let (root, manifest) = self.load(name)?;
        let status = Command::new(manifest.entrypoint(&root)?)
            .args(args)
            .env("DM_PLUGIN_API_VERSION", API_VERSION.to_string())
            .env("DM_PLUGIN_DIR", &root)
            .env("DM_HOME", fs::canonicalize(&self.home)?)
            .status()
            .with_context(|| format!("Start plugin '{name}'"))?;
        if let Some(code) = status.code() {
            return Ok(code);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if let Some(signal) = status.signal() {
                return Ok(128 + signal);
            }
        }
        bail!("Plugin terminated without an exit code")
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    plugins: std::collections::BTreeMap<String, String>,
}

fn validate_crate(source: &Path, manifest: &Manifest) -> Result<()> {
    let cargo: toml::Value = toml::from_str(
        &fs::read_to_string(source.join("Cargo.toml"))
            .context("Rust plugins must include Cargo.toml")?,
    )?;
    let package = cargo
        .get("package")
        .context("Plugin must be a Rust package")?;
    ensure!(
        package.get("version").and_then(toml::Value::as_str) == Some(&manifest.version),
        "Cargo package version must match dm-plugin.toml (use an explicit version)"
    );
    ensure!(
        cargo
            .get("dependencies")
            .and_then(|v| v.get("dm-plugin-sdk"))
            .is_some(),
        "Rust plugins must depend on dm-plugin-sdk"
    );
    ensure!(
        cargo
            .get("bin")
            .and_then(toml::Value::as_array)
            .is_some_and(|bins| bins
                .iter()
                .any(|bin| bin.get("name").and_then(toml::Value::as_str)
                    == Some(&manifest.binary_name()))),
        "Rust plugins must declare [[bin]] with name '{}'",
        manifest.binary_name()
    );
    Ok(())
}
