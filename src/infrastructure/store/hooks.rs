use anyhow::{Context, Result, ensure};
use log::info;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::Manifest;

use super::{PluginStore, inherit_safe_environment};

impl PluginStore {
    pub(crate) fn copy_hooks(
        &self,
        source: &Path,
        package: &Path,
        manifest: &Manifest,
    ) -> Result<()> {
        for hook in [
            manifest.hooks.post_install.as_deref(),
            manifest.hooks.pre_uninstall.as_deref(),
            manifest.hooks.post_uninstall.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            let from = resolve_hook_path(source, hook)?;
            let to = package.join(normalize_hook_path(hook));
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&from, &to).with_context(|| format!("Copy hook '{hook}'"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = fs::metadata(&from)?.permissions().mode();
                fs::set_permissions(&to, fs::Permissions::from_mode(mode | 0o111))?;
            }
        }
        Ok(())
    }

    pub(crate) fn run_hook(
        &self,
        root: &Path,
        hook: &str,
        phase: &str,
        manifest: &Manifest,
    ) -> Result<()> {
        let root = fs::canonicalize(root).context("Resolve plugin hook directory")?;
        let executable = resolve_hook_path(&root, hook)?;
        let metadata =
            fs::symlink_metadata(&executable).with_context(|| format!("Missing hook '{hook}'"))?;
        ensure!(metadata.is_file(), "Hook '{hook}' must be a regular file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            ensure!(
                metadata.permissions().mode() & 0o111 != 0,
                "Hook '{hook}' is not executable"
            );
        }
        info!("running {phase} hook '{hook}' for plugin {}", manifest.name);
        let mut command = Command::new(&executable);
        command.env_clear();
        inherit_safe_environment(&mut command, manifest, &self.plugin_environment);
        command
            .arg(phase)
            .current_dir(&root)
            .env("DM_PLUGIN_HOME", fs::canonicalize(&self.home)?)
            .env("DM_PLUGIN_DIR", &root)
            .env("DM_HOOK_PHASE", phase);
        let status = command
            .status()
            .with_context(|| format!("Run {phase} hook '{hook}'"))?;
        ensure!(status.success(), "Plugin hook '{hook}' failed for {phase}");
        Ok(())
    }
}

fn resolve_hook_path(root: &Path, hook: &str) -> Result<PathBuf> {
    let root = fs::canonicalize(root).context("Resolve plugin hook directory")?;
    let mut path = root.clone();
    let components: Vec<_> = hook.split(['/', '\\']).collect();
    for (index, component) in components.iter().enumerate() {
        path.push(component);
        let metadata =
            fs::symlink_metadata(&path).with_context(|| format!("Missing hook '{hook}'"))?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "Hook '{hook}' must not traverse symlinks"
        );
        if index + 1 == components.len() {
            ensure!(metadata.is_file(), "Hook '{hook}' must be a regular file");
        } else {
            ensure!(
                metadata.is_dir(),
                "Hook '{hook}' parent must be a directory"
            );
        }
    }
    let canonical = fs::canonicalize(&path).context("Resolve plugin hook path")?;
    ensure!(
        canonical.starts_with(&root),
        "Hook '{hook}' escapes plugin directory"
    );
    Ok(canonical)
}

fn normalize_hook_path(hook: &str) -> PathBuf {
    let mut path = PathBuf::new();
    for component in hook.split(['/', '\\']) {
        path.push(component);
    }
    path
}
