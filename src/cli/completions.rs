//! Shell adapters call the same runtime completion protocol.
use crate::{PluginStore, cli::Cli};
use anyhow::{Context, Result};
use clap::CommandFactory;
use clap::ValueEnum;
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
}

impl Shell {
    /// Human-readable name used in installer output.
    pub fn label(self) -> &'static str {
        match self {
            Shell::Bash => "Bash",
            Shell::Zsh => "Zsh",
        }
    }

    /// File name the shell expects inside its completion directory.
    pub fn completion_file(self) -> &'static str {
        match self {
            Shell::Bash => "dm",
            Shell::Zsh => "_dm",
        }
    }

    /// Directory below the XDG data directory that the shell reads.
    fn completion_subdir(self) -> &'static str {
        match self {
            Shell::Bash => "bash-completion/completions",
            Shell::Zsh => "zsh/site-functions",
        }
    }
}

pub fn complete(store: &PluginStore, words: &[String]) -> Result<()> {
    let names = store.completion_names().unwrap_or_default();
    let mut values = if words.first().is_some_and(|word| names.contains(word)) && words.len() > 1 {
        store
            .complete_plugin(&words[0], &words[1..])
            .unwrap_or_default()
    } else {
        let mut values = crate::support::completion::candidates(Cli::command(), words, &names);
        if words.len() <= 1 {
            values.extend(names);
        }
        values
    };
    let prefix = words.last().map(String::as_str).unwrap_or("");
    values.retain(|value| value.starts_with(prefix) && !value.chars().any(char::is_control));
    values.sort();
    values.dedup();
    for value in values {
        println!("{value}");
    }
    Ok(())
}

pub fn script(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => include_str!("../../completions/dm.bash"),
        Shell::Zsh => include_str!("../../completions/_dm"),
    }
}

/// Default completion directory: XDG_DATA_HOME plus the shell subdirectory,
/// falling back to ~/.local/share. Both shells load these paths without any
/// extra setup.
pub fn completion_dir(shell: Shell, xdg: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    let base = match xdg.filter(|value| !value.is_empty()) {
        Some(value) => PathBuf::from(value),
        None => PathBuf::from(home.filter(|value| !value.is_empty())?)
            .join(".local")
            .join("share"),
    };
    Some(base.join(shell.completion_subdir()))
}

/// Write the completion script where the shell finds it; returns that path.
pub fn install(shell: Shell, dir: Option<&Path>) -> Result<PathBuf> {
    let directory = match dir {
        Some(dir) => dir.to_path_buf(),
        None => completion_dir(
            shell,
            std::env::var_os("XDG_DATA_HOME").as_deref(),
            std::env::var_os("HOME").as_deref(),
        )
        .context("无法确定补全目录：请设置 HOME 或 XDG_DATA_HOME，或用 --dir 指定")?,
    };
    fs::create_dir_all(&directory)
        .with_context(|| format!("创建补全目录 {}", directory.display()))?;
    let path = directory.join(shell.completion_file());
    fs::write(&path, script(shell)).with_context(|| format!("写入补全脚本 {}", path.display()))?;
    Ok(path)
}

/// What the user still has to do before the installed script takes effect.
pub fn activation_hint(shell: Shell, path: &Path) -> String {
    let directory = path.parent().unwrap_or(path);
    match shell {
        Shell::Bash => format!(
            "新开一个 shell 即生效；未生效时确认 {} 已加入 bash-completion 的搜索路径。",
            directory.display()
        ),
        Shell::Zsh => format!(
            "确保 fpath 包含 {}（例如 fpath=({} $fpath)），再运行 compinit。",
            directory.display(),
            directory.display()
        ),
    }
}
