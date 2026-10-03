//! 宿主使用的终端确认输入与名称建议。

use anyhow::{Context, Result, ensure};
use std::io::{IsTerminal, Write};

pub trait Prompter {
    fn line(&self, prompt: &str) -> Result<String>;
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

pub use super::suggestions::suggestions;
