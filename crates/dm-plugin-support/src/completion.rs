//! Completion-v1: words exclude the executable and include the current word.
//! Responses contain one plain candidate per line, without descriptions/secrets.
use clap::{Arg, Command};
use std::{collections::BTreeSet, path::Path};

pub fn candidates(mut command: Command, words: &[String], names: &[String]) -> Vec<String> {
    command.build();
    let current = words.last().map(String::as_str).unwrap_or("");
    let completed = &words[..words.len().saturating_sub(1)];
    let mut pending: Option<Arg> = None;
    let mut position = 1;
    let mut flags = true;
    let mut used = BTreeSet::new();
    for word in completed {
        if pending.take().is_some() {
            continue;
        }
        if flags && word == "--" {
            flags = false;
            continue;
        }
        if flags && word.starts_with('-') {
            let flag = word.split('=').next().unwrap_or(word);
            if let Some(arg) = command.get_arguments().find(|arg| {
                arg.get_long()
                    .is_some_and(|long| flag == format!("--{long}"))
                    || arg
                        .get_short()
                        .is_some_and(|short| flag == format!("-{short}"))
            }) {
                used.insert(arg.get_id().to_string());
                if !word.contains('=') && arg.get_action().takes_values() {
                    pending = Some(arg.clone());
                }
            }
            continue;
        }
        if let Some(sub) = command.find_subcommand(word).cloned() {
            command = sub;
            position = 1;
            used.clear();
        } else {
            if let Some(arg) = command
                .get_positionals()
                .find(|arg| arg.get_index() == Some(position))
            {
                used.insert(arg.get_id().to_string());
            }
            position += 1;
        }
    }
    if let Some(arg) = pending {
        return values(&arg, current, names);
    }
    if flags
        && let Some((flag, prefix)) = current.split_once('=')
        && let Some(arg) = command.get_arguments().find(|arg| {
            arg.get_long()
                .is_some_and(|long| flag == format!("--{long}"))
        })
    {
        return values(arg, prefix, names)
            .into_iter()
            .map(|value| format!("{flag}={value}"))
            .collect();
    }
    let blocked: BTreeSet<_> = command
        .get_arguments()
        .filter(|arg| used.contains(arg.get_id().as_str()))
        .flat_map(|arg| command.get_arg_conflicts_with(arg))
        .map(|arg| arg.get_id().to_string())
        .collect();
    let mut result = Vec::new();
    if flags {
        for arg in command.get_arguments().filter(|arg| {
            !arg.is_hide_set()
                && !used.contains(arg.get_id().as_str())
                && !blocked.contains(arg.get_id().as_str())
                && !command
                    .get_arg_conflicts_with(arg)
                    .iter()
                    .any(|conflict| used.contains(conflict.get_id().as_str()))
        }) {
            if let Some(long) = arg.get_long() {
                result.push(format!("--{long}"));
            }
            if let Some(short) = arg.get_short() {
                result.push(format!("-{short}"));
            }
        }
    }
    if !current.starts_with('-') {
        for sub in command.get_subcommands().filter(|sub| !sub.is_hide_set()) {
            result.push(sub.get_name().to_owned());
            result.extend(sub.get_visible_aliases().map(str::to_owned));
        }
        if let Some(arg) = command
            .get_positionals()
            .find(|arg| arg.get_index() == Some(position))
        {
            result.extend(values(arg, current, names));
        }
    }
    result.retain(|value| value.starts_with(current) && !value.chars().any(char::is_control));
    result.sort();
    result.dedup();
    result
}
fn values(arg: &Arg, prefix: &str, names: &[String]) -> Vec<String> {
    let id = arg.get_id().as_str();
    let mut result = if id == "name" {
        names.to_vec()
    } else if matches!(id, "file" | "key" | "source") {
        paths(prefix)
    } else {
        arg.get_possible_values()
            .into_iter()
            .filter(|value| !value.is_hide_set())
            .map(|value| value.get_name().to_owned())
            .collect()
    };
    result.retain(|value| value.starts_with(prefix) && !value.chars().any(char::is_control));
    result.sort();
    result.dedup();
    result
}
fn paths(prefix: &str) -> Vec<String> {
    let split = prefix
        .rfind(['/', '\\'])
        .map(|index| index + 1)
        .unwrap_or(0);
    let parent = &prefix[..split];
    let directory = if parent.is_empty() {
        Path::new(".")
    } else {
        Path::new(parent)
    };
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if prefix[split..].is_empty() && name.starts_with('.') {
                return None;
            }
            Some(format!(
                "{parent}{name}{}",
                if entry.file_type().ok()?.is_dir() {
                    "/"
                } else {
                    ""
                }
            ))
        })
        .collect()
}
