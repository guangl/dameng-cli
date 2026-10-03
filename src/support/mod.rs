//! 宿主自带的实现工具：有界读取、十六进制编码、子进程与并发控制、终端交互、
//! 配置展示与补全候选。
//!
//! 这些工具只服务 dameng-cli 宿主；内置 db、ssh 插件是独立仓库，各自在
//! `src/support/` 内维护同类实现，不跨仓库共享。公开的插件协议仍由
//! dm-plugin-sdk 定义。

pub mod bounded;
pub mod codec;
pub mod completion;
pub mod config;
pub mod interaction;
pub mod parallel;
pub mod process;
mod suggestions;
