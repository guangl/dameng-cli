---
layout: doc
title: 快速开始
description: 创建、安装并运行第一个 dameng-cli Rust 插件。
---

# 快速开始

## 前置条件

- 当前稳定版 Rust 和 Cargo。
- 已安装的 `dm`，或 dameng-cli 源码检出。
- 远程安装插件时需要 Git。

在 dameng-cli 仓库根目录安装宿主：

```sh
cargo install --path . --locked
dm --version
```

## 1. 创建项目

```sh
cargo new --bin dm-plugin-backup
cd dm-plugin-backup
```

插件仓库根目录最终应包含：

```text
dm-plugin-backup/
├── Cargo.toml
├── Cargo.lock
├── dm-plugin.toml
└── src/
    └── main.rs
```

## 2. 配置 Cargo target 与 SDK

开发阶段可以让插件目录与 dameng-cli 检出并排，并使用本地路径：

```toml
[package]
name = "dm-plugin-backup"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "dm-backup"
path = "src/main.rs"

[dependencies]
dm-plugin-sdk = { path = "../dameng-cli/crates/dm-plugin-sdk" }
```

独立发布仓库不能依赖开发机器上的路径。发布前改为任何用户都能访问且固定到实际提交的 Git 依赖：

```toml
dm-plugin-sdk = { git = "https://github.com/guangl/dameng-cli.git", rev = "<full-commit-sha>" }
```

SDK 尚未承诺发布到 crates.io；不要提交本机绝对路径。

## 3. 添加插件清单

在仓库根目录创建 `dm-plugin.toml`：

```toml
name = "backup"
version = "0.1.0"
description = "Backup a Dameng database"
api_version = 1
```

`name = "backup"` 决定用户命令是 `dm backup`，Cargo binary 必须对应为 `dm-backup`。

## 4. 实现插件

```rust
use dm_plugin_sdk::{Context, Plugin, PluginResult};

struct Backup;

impl Plugin for Backup {
    fn run(&self, context: Context) -> PluginResult {
        println!("received {} arguments", context.args.len());
        Ok(0)
    }
}

fn main() {
    dm_plugin_sdk::run(Backup);
}
```

## 5. 生成锁文件并安装

```sh
cargo generate-lockfile
cargo test --locked
cd ..
dm install ./dm-plugin-backup
dm list
dm backup --help
```

安装成功后，插件运行不再依赖原源码目录和 Cargo 构建目录。修改源码后需要先 `dm uninstall backup`，再重新安装。

下一步阅读[项目结构与清单](manifest.html)。
