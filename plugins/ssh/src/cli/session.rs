use crate::ssh_command;
use anyhow::{Context, Result, ensure};
use dm_plugin_sdk::Context as PluginContext;

pub(super) fn test(context: &PluginContext, name: &str) -> Result<()> {
    let output = ssh_command(context, name, true)?
        .output()
        .context("启动 SSH 测试失败；运行 dm ssh doctor 检查工具与配置")?;
    let detail = String::from_utf8_lossy(&output.stderr);
    ensure!(
        output.status.success(),
        "SSH 连接 '{name}' 测试失败：{detail}（退出码 {:?}）",
        output.status.code()
    );
    println!("SSH 连接 {name} 测试成功");
    Ok(())
}
pub(super) fn connect(context: &PluginContext, name: &str) -> Result<i32> {
    let status = ssh_command(context, name, false)?
        .status()
        .context("启动 SSH 登录失败；运行 dm ssh doctor 检查工具与配置")?;
    match status.code() {
        Some(code) => Ok(code),
        None => {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                Ok(status.signal().map(|signal| 128 + signal).unwrap_or(1))
            }
            #[cfg(not(unix))]
            {
                anyhow::bail!("SSH session terminated without an exit code")
            }
        }
    }
}
