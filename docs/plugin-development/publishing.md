---
layout: doc
title: 发布与分发
description: 独立插件仓库、本地安装、Git 安装、名称安装和版本策略。
---

# 发布与分发

## 发布前清单

- `dm-plugin.toml` 与 Cargo package 版本一致。
- `Cargo.lock` 已生成并提交。
- SDK 使用公开可访问、固定提交的 Git 依赖；不包含本机绝对路径。
- 仓库根目录就是插件 crate，不依赖未初始化的 submodule。
- README 记录用法、配置、退出码、数据库客户端依赖和许可证。
- 所有目标系统的格式、Clippy、测试和文档检查通过。
- 错误消息、fixture 和 CI 日志中没有真实凭证。

## 本地目录安装

适用于开发和审查：

```sh
dm install ./dm-plugin-backup
```

宿主读取当前目录中的源码，但安装后只保留编译出的 binary 和清单。

## HTTPS Git 安装

将 crate 放在仓库根目录并推送后：

```sh
dm install https://github.com/your-org/dm-plugin-backup.git
```

宿主浅克隆远程默认分支并执行 locked release build。`Cargo.lock` 固定依赖，但默认分支仍可能变化；生产安装应固定 tag 或完整 commit：

```sh
dm install https://github.com/your-org/dm-plugin-backup.git --rev v1.2.0
```

宿主会记录解析后的 commit 和已安装二进制 SHA-256。`dm verify` 可检测安装后的文件变化。

## 名称安装

名称安装依赖 SQLite 中的本地注册表。注册表中的名称必须与下载后清单的 `name` 完全一致：

```sh
dm registry add backup https://github.com/your-org/dm-plugin-backup.git
dm install backup
```

可以使用 `dm registry list` 查看条目，使用 `dm registry remove backup` 删除条目。插件仓库不能通过修改自身清单冒充另一个注册名称。

## 版本策略

- 插件业务版本由插件仓库独立维护。
- 破坏性命令行或配置变更应提升主版本并写迁移说明。
- 宿主 API 仍为 v1 时保持 `api_version = 1`。
- `dm update` 在临时目录完成构建和校验，再原子切换安装目录；构建或元数据写入失败会保留旧版本。
- 使用固定 `--rev` 的插件不会被 `dm outdated` 误报为跟踪默认分支；变更固定版本时重新安装或明确选择新 revision。

出现问题时查看[故障排查](troubleshooting.html)。
