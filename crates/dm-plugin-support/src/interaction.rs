//! Terminal input shared by the official plugins.
use anyhow::{Context, Result, ensure};
use std::io::{IsTerminal, Write};

pub trait Prompter {
    fn line(&self, prompt: &str) -> Result<String>;
    fn secret(&self, prompt: &str) -> Result<String>;
}

pub struct TerminalPrompter;
impl Prompter for TerminalPrompter {
    fn line(&self, prompt: &str) -> Result<String> {
        eprint!("{prompt}");
        std::io::stderr().flush()?;
        let mut input = String::new();
        ensure!(
            std::io::stdin().read_line(&mut input)? > 0,
            "terminal closed"
        );
        Ok(input.trim().to_owned())
    }
    fn secret(&self, prompt: &str) -> Result<String> {
        rpassword::prompt_password(prompt).context("读取隐藏输入")
    }
}
pub fn terminal_prompter() -> Option<&'static dyn Prompter> {
    static TERMINAL: TerminalPrompter = TerminalPrompter;
    std::io::stdin().is_terminal().then_some(&TERMINAL)
}

pub fn confirm(prompter: Option<&dyn Prompter>, yes: bool, message: &str) -> Result<()> {
    if yes {
        return Ok(());
    }
    let prompter = prompter.context("需要确认；请在终端运行，或显式使用 --yes")?;
    let answer = prompter.line(&format!("{message} [y/N]: "))?;
    ensure!(
        matches!(answer.to_lowercase().as_str(), "y" | "yes" | "是"),
        "操作已取消"
    );
    Ok(())
}

pub fn validated<T>(
    prompter: &dyn Prompter,
    prompt: &str,
    parse: impl Fn(&str) -> Result<T>,
) -> Result<T> {
    loop {
        let answer = prompter.line(prompt)?;
        match parse(answer.trim()) {
            Ok(value) => return Ok(value),
            Err(error) => eprintln!("输入无效：{error}，请重新输入。"),
        }
    }
}
pub fn resolve_required(
    value: Option<String>,
    prompt: &str,
    missing: &str,
    prompter: Option<&dyn Prompter>,
) -> Result<String> {
    let parse = |value: &str| {
        ensure!(!value.trim().is_empty(), "{missing}");
        Ok(value.trim().to_owned())
    };
    match value {
        Some(value) => parse(&value),
        None => validated(prompter.context(missing.to_owned())?, prompt, parse),
    }
}
pub fn port(value: Option<u16>, fallback: u16, prompter: Option<&dyn Prompter>) -> Result<u16> {
    if let Some(value) = value {
        ensure!(value > 0, "端口必须在 1–65535 之间");
        return Ok(value);
    }
    match prompter {
        None => Ok(fallback),
        Some(prompter) => validated(prompter, &format!("端口 [{fallback}]: "), |answer| {
            let value = if answer.is_empty() {
                fallback
            } else {
                answer.parse::<u16>().context("端口必须是数字")?
            };
            ensure!(value > 0, "端口必须在 1–65535 之间");
            Ok(value)
        }),
    }
}
pub fn password(
    value: Option<String>,
    prompter: Option<&dyn Prompter>,
    missing: &str,
) -> Result<String> {
    if let Some(value) = value {
        ensure!(!value.is_empty(), "密码不能为空");
        return Ok(value);
    }
    let prompter = prompter.context(missing.to_owned())?;
    loop {
        let value = prompter.secret("密码: ")?;
        if !value.is_empty() {
            return Ok(value);
        }
        eprintln!("密码不能为空，请重新输入。");
    }
}
/// An omitted edit flag prompts with the old value; Enter keeps it.
pub fn edit_field(
    value: Option<String>,
    old: &str,
    label: &str,
    prompter: Option<&dyn Prompter>,
) -> Result<String> {
    if let Some(value) = value {
        ensure!(!value.trim().is_empty(), "{label}不能为空");
        return Ok(value.trim().into());
    }
    match prompter {
        None => Ok(old.to_owned()),
        Some(p) => {
            let value = p.line(&format!("{label} [{old}]: "))?;
            Ok(if value.is_empty() { old.into() } else { value })
        }
    }
}
pub fn select_name(
    name: Option<String>,
    names: &[String],
    prompter: Option<&dyn Prompter>,
) -> Result<String> {
    if let Some(name) = name {
        ensure!(
            names.contains(&name),
            "连接 '{name}' 不存在。可选连接：{}",
            suggestions(&name, names)
        );
        return Ok(name);
    }
    ensure!(!names.is_empty(), "尚无连接，请先运行 add 添加连接");
    if names.len() == 1 {
        return Ok(names[0].clone());
    }
    let p = prompter.context("有多个连接，请指定名称；使用 list 查看连接")?;
    loop {
        let query = p.line("连接名称或搜索关键词（回车列出全部）: ")?;
        if names.contains(&query) {
            return Ok(query);
        }
        let matches: Vec<_> = names
            .iter()
            .filter(|name| name.to_lowercase().contains(&query.to_lowercase()))
            .collect();
        if matches.len() == 1 {
            return Ok(matches[0].clone());
        }
        eprintln!(
            "匹配的连接：{}",
            matches
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

pub use crate::suggestions::suggestions;
