---
layout: doc
title: 使用体验与自动补全
description: 连接编辑、数据保留、环境诊断和动态插件补全。
---

# 使用体验与自动补全

## 连接操作

`dm db add`、`dm ssh add` 在终端下逐项输入，端口与必填项输错可原地重试；保存前展示不包含密码的摘要并确认。`--yes` 跳过保存确认。非交互脚本必须提供必填参数，添加/编辑无需额外确认。

同名添加默认拒绝覆盖。使用 `edit` 修改已有连接，只更新指定字段并保留原密码；终端下回车保留显示的原值，密码/口令不回显也不重新询问：

```sh
dm db edit prod --port 5300
dm db edit prod --clear-schema
dm ssh edit prod --username deploy
dm ssh edit prod --passphrase ''      # 显式清除密钥口令
```

`add --replace` 用于重新录入同名连接。SSH 切换认证方法或私钥路径时，不会把原有秘密复用到新认证；编辑时重新选择同一私钥、不传新口令则保留旧口令。

`dm ssh connect [name]` 是登录入口，`dm ssh ssh [name]` 仍可用。省略名称时，只有一个连接直接选中；多个连接在终端下可搜索名称或输入完整名称，脚本必须指定名称。不存在的名称会给出候选提示。

删除连接须确认，脚本显式使用 `dm db remove prod --yes` 或 `dm ssh remove prod --yes`。

## 卸载和重新安装

`dm uninstall ssh` 默认仅移除插件程序并保留配置、连接、缓存和备份。重新安装后可继续使用，`dm doctor --repair` 不会清理这些主动保留的数据。

`dm uninstall ssh --purge` 才同时清空数据。执行前列出路径并确认；非交互执行须显式加 `--yes`；已卸载但保留了数据的插件，也可用同一命令清空。清理失败或中断时，登记一直保留到所有目录（包括备份）清理成功；错误列出未清理路径，修复目录问题后重试 `dm uninstall <name> --purge --yes`。期间 `doctor --repair` 不会删除剩余数据。库 API 的兼容方法 `PluginStore::uninstall` 仍表示彻底移除，新的 `uninstall_with_options(name, purge)` 用于明确选择保留或清空。

## 配置与诊断

宿主与内置插件都支持 `config init`（创建示例，不覆盖文件）、`config path`、`config show [--json]`。`show` 列出有效配置及 `default`、`config`、`env:<变量名>` 来源，插件配置显示其自身支持的字段。保存的密码和口令不会出现在配置输出中。

```sh
dm config init
dm config show --json
dm ssh config init
dm db config path
dm doctor ssh                     # 等价于 dm ssh doctor
dm ssh doctor --json
```

宿主 `doctor` 继续检查插件存储；指定插件名则转发插件环境检查，不支持 `--repair`。SSH 诊断检查配置、连接存储、`ssh`、密码连接和使用已保存私钥口令所需 `sshpass` 及本机私钥路径，不发起网络连接。已保存的私钥口令由 `sshpass` 应答 OpenSSH 提示，密码与口令通过子进程环境传递，不进入参数或日志；未保存口令的密钥连接直接使用系统 `ssh`。实际 `ssh test` 使用连接超时，错误保留 SSH 的诊断信息，提示区分 DNS/网络、认证、主机密钥和本机工具问题。数据库插件诊断会明确报告驱动尚未实现，`test/exec` 当前不可用。

## 启用自动补全

补全包括宿主命令、已安装插件、启用补全的插件子命令与参数、文件路径，以及保存的连接名称。无需为内置插件单独安装脚本，安装、卸载或添加连接后自动生效。

### Bash

在 `~/.bashrc` 中加入：

```sh
source <(dm completions bash)
```

### Zsh

在 `~/.zshrc` 的 `compinit` 之后加入：

```sh
autoload -Uz compinit
compinit
source <(dm completions zsh)
```

也可将 `dm completions zsh` 的输出保存为 `$fpath` 中的 `_dm` 文件，让 `compinit` 加载。

### 体验示例

```text
dm <Tab>                       宿主命令与已安装插件
dm ssh <Tab>                   add、edit、connect、doctor、config 等
dm ssh connect pr<Tab>         已保存名称，例如 prod
dm db edit prod --<Tab>        --host、--port、--password 等
dm ssh add prod --key <Tab>    本机文件路径
dm update s<Tab>               已安装插件名称，例如 ssh
```

补全查询不创建或迁移 SQLite、不创建机器密钥、不写日志、不访问网络。插件查询超时或失败时安静返回空候选，旧插件未声明支持时不会被执行。

## 第三方插件协议

宿主从 0.4.0 起支持此字段。插件在清单顶层显式声明：

```toml
completion = true
min_host_version = "0.4.0"
```

宿主运行 `dm-<name> __complete <words...>`。`words` 不包含 executable、`dm` 或插件名，包含正在补全的最后一个词，最后一词可为空。例如 `dm backup restore pro<Tab>` 对应 `__complete restore pro`。

插件应在普通参数解析前识别 `__complete`，以只读方式生成候选：

- 每行一个纯文本候选，成功退出码为 0；不输出描述、提示、秘密、日志或控制字符。
- 当前词是前缀，宿主再次过滤、排序和去重。含空格的路径作为一整行输出，交由 shell 适配脚本处理引用。
- 查询不访问网络、不运行 hook、不写配置/数据库/缓存；存储不存在时仍可返回静态命令与选项。
- 宿主传递 `completion-v1` capability、常规插件目录与声明的环境变量，不创建目录；stdin 和 stderr 被禁用，1 秒 内未返回则终止进程，最多读取 64 KiB 和 1000 项。

未声明 `completion = true` 的插件仍可补全其名称，宿主不会查询其子命令。旧宿主不认识新的清单字段，插件发布时应在 `min_host_version` 中声明实际支持此字段的宿主版本。
