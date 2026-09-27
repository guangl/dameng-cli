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
cargo generate-lockfile
```

手工创建项目后，按第 2、3 节补齐固定 binary target、SDK 依赖和 `dm-plugin.toml`。

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
min_host_version = "0.2.0"
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

## 5. 构建并安装

宿主只安装预编译插件，不执行任何 Cargo 构建，所以安装前要把构建产物放进包目录，与 `dm-plugin.toml` 同级：

```sh
cargo generate-lockfile
cargo test --locked
cargo build --release --locked
cp target/release/dm-backup .          # Windows: copy target\release\dm-backup.exe .
cd ..
dm install ./dm-plugin-backup
dm list
dm backup --help
```

目录里没有 `dm-<name>` 时 `dm install` 会直接报错，提示缺少预编译产物。安装成功后，插件运行不再依赖包目录和 Cargo 构建目录；修改源码并提升版本后，重新构建、覆盖包目录里的二进制，再运行 `dm update backup`，宿主会从该目录重新安装并原子切换到新版本。要分发给别人，则需按[发布与分发](publishing.html)把 `dm-<name>-<target>` 发布到 GitHub Release。

下一步阅读[项目结构与清单](manifest.html)。
