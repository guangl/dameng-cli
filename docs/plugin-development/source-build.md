---
layout: doc
title: 源码编译与 Rust 版本
description: 显式源码安装、固定工具链、MSRV 与锁文件约定。
---

# 源码编译与 Rust 版本

`dm install` 默认使用预编译插件。显式指定 `--build` 时，宿主从本地目录或 HTTPS Git 仓库构建 Rust 插件，不下载 Release 二进制，也不使用源码目录里已有的 `dm-<name>`。

```sh
dm install ./my-plugin --build --toolchain 1.99.0 --install-toolchain
dm install https://github.com/OWNER/REPO.git --rev v1.2.0 --build --toolchain 1.99.0
# 更新源码后重建并替换，保留配置、数据与缓存
dm install ./my-plugin --build --replace --toolchain 1.99.0
```

需要 rustup、对应 Rust 工具链，以及目标平台所需的链接器或系统库。远程源码还需要 Git。仓库根目录必须包含 `dm-plugin.toml`、`Cargo.toml` 和已提交的 `Cargo.lock`；宿主不搜索子目录、不初始化 Git submodule。独立 crate 应在根目录提交锁文件；工作区成员如果没有自己的根目录锁文件，应先准备可独立构建的源码目录。

## 插件如何规定 Rust 版本

`Cargo.toml` 的 `rust-version` 声明最低支持版本（MSRV），由 Cargo 校验；它不会选择或下载工具链。SDK 与其他锁定依赖也可能要求更高版本，因此必须用实际构建和测试验证 MSRV。

```toml
[package]
rust-version = "1.99.0"
```

插件根目录的 `rust-toolchain.toml` 声明默认构建版本：

```toml
[toolchain]
channel = "1.99.0"
profile = "minimal"
```

工具链选择顺序：

1. 命令行 `--toolchain VERSION`。
2. 插件根目录的 `rust-toolchain.toml` 中 `[toolchain] channel`。
3. 宿主默认值：`DM_BUILD_TOOLCHAIN` → `<DM_PLUGIN_HOME>/config.toml` 的 `[build] toolchain` → 宿主 MSRV（当前 `1.99.0`）。

首版仅接受完整固定版本，例如 `1.99.0`；不接受 `stable`、`beta`、nightly、自定义路径或旧式 `rust-toolchain` 文件。命令行指定固定版本可覆盖这些插件文件。当前官方插件文件使用 `stable`，源码安装时请显式传入 `--toolchain 1.99.0`。宿主只读取插件工具链文件的 `channel`，不自动安装其中的 components 或 targets；构建目标是当前 dm 二进制的平台。

工具链缺失会给出安装指令。加 `--install-toolchain` 后执行 `rustup toolchain install VERSION --profile minimal`，不会修改 rustup 全局默认值。固定工具链通过 `rustup run VERSION cargo ...` 调用，用户的 `RUSTUP_TOOLCHAIN` 和目录 override 不会改变此次选择。

## 构建与安装

构建采用 `--release --locked --bin dm-<name>`，显式指定宿主 target 和临时 target 目录。Cargo 负责处理 package/workspace 的 `rust-version` 与依赖要求；不会使用 `--ignore-rust-version`。缺少锁文件、不一致的锁文件、工具链或编译失败都会阻止安装。临时产物在操作结束后清理，不覆盖源码目录的二进制或 target 目录。锁文件和固定工具链减少版本漂移，系统依赖与构建环境仍需由插件维护者管理。

构建成功后复用现有安装事务及 hook。`pre_install` 在编译之后运行，不能用来准备 Cargo 编译依赖；`post_install` 失败按既有规则回滚。源码构建会执行 Cargo build script 与过程宏，应像运行插件一样选择可信来源。

`--toolchain` 和 `--install-toolchain` 都要求 `--build`。`--build` 不与 `--check`（或 `--dry-run`）、`--release-source` 同用。`--check` 继续验证预编译包，不执行构建或安装工具链。

首版不保存构建选项：`dm update` 仍使用已有的预编译更新流程。需要源码重建时使用 `dm install <source> --build --replace`，重新提供相同工具链和所需 `--rev`。

插件运行通过独立进程与宿主通信，Rust 版本无需与宿主相同；运行兼容性由插件协议版本、最低宿主版本及平台约束决定。建议插件 CI 分别测试 MSRV 与最新 stable，并将发布构建固定在明确版本，提交锁文件。
