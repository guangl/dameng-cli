//! Short, actionable hints for the errors the commands report.

use anyhow::Error;

/// Return a short, actionable hint for a database plugin error.
#[doc(hidden)]
pub fn db_hint(error: &Error) -> String {
    let text = error
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();

    if text.contains("is not configured") {
        return "请先运行 dm db add <name> 配置连接，或用 dm db list 查看已保存的连接。".into();
    }
    if text.contains("driver is not implemented") {
        return "数据库驱动尚未接入：当前 dm db 只管理连接配置，dm db test 与 dm db exec 会在驱动实现后可用。".into();
    }
    if text.contains("must not be empty") || text.contains(" is required") {
        return "缺少必填项；在终端下运行可交互输入，或显式传入对应参数。".into();
    }
    if text.contains("no such table")
        || text.contains("no such column")
        || text.contains("no column named")
        || text.contains("sql logic error")
        || text.contains("store")
        || text.contains("sqlite")
    {
        return "连接存储异常，请检查插件数据目录中的 connections.sqlite3。".into();
    }

    "使用 dm db --help 查看可用子命令和参数。".into()
}
