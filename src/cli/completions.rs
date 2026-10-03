//! Shell adapters call the same runtime completion protocol.
use crate::{PluginStore, cli::Cli};
use anyhow::Result;
use clap::CommandFactory;
use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
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
