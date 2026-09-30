# dm ssh 插件

由 `plugins/ssh` 提供的 SSH 服务器连接管理插件。宿主只负责安装与转发参数，服务器配置、
凭据加密和 `ssh` 调用都在插件内完成。

## 命令

| 命令 | 说明 |
| --- | --- |
| `dm ssh add <name> --host H [--port 22] [--username U]` | 新增服务器，同名时必须加 `--replace`；密码认证用 `--password P`，密钥认证用 `--key PATH`（可加 `--passphrase S`）；终端下省略任意字段时逐项提示，密码与口令隐藏回显，空口令表示密钥未加密 |
| `dm ssh list [--json]` | 以带边框表格列出名称、主机、端口、用户、认证方式与私钥路径；`--json` 输出同样字段的机器可读 JSON（`auth_type`、`key_path`），空列表为 `[]`，从不包含密码或口令 |
| `dm ssh remove <name>` | 删除服务器，终端下确认，脚本需 `--yes` |
| `dm ssh export [--file PATH] [--include-secrets]` | 导出服务器配置；默认不带密码与私钥口令，省略 `--file` 时输出到 stdout；`--include-secrets` 会要求输入并确认导出加密口令 |
| `dm ssh import <file> [--replace]` | 从 JSON 文件导入；默认遇到同名服务器报错，`--replace` 覆盖；未包含秘密的导入会按认证方式保留同名服务器原有秘密 |
| `dm ssh test <name>` | 非交互探测：密钥认证执行 `ssh -i <key> -p <port> -o BatchMode=yes -o ConnectTimeout=<n> user@host true`，密码认证用 `sshpass` 包裹，并使用 `BatchMode=no` 允许密码应答 |
| `dm ssh connect [name]`（别名 `ssh`） | 启动交互式 SSH 会话，终端交给系统的 `ssh`，其退出码原样返回（被信号终止时为 `128+signal`） |

失败时 stderr 会给出 `错误`、`详情` 与中文 `提示`；缺少必填项、服务器不存在、认证信息缺失或存储异常都会给出可操作建议。

## 前置条件

- `dm ssh test` 和 `dm ssh ssh` 调用系统的 `ssh`，需要本机可以执行 `ssh`。
- 密码认证额外需要系统提供 `sshpass`（Debian/Ubuntu 为 `apt install sshpass`，RHEL 系为 `yum install sshpass`；macOS 通常需要另行安装）；没有安装时命令会明确报错并提示改用密钥认证。
- 密钥认证不会复制私钥：`--key` 只记录路径，**运行 `dm ssh` 的本机**上必须存在该私钥文件；远端服务器只需要对应的公钥（通常放在 `~/.ssh/authorized_keys`）。

## 配置

插件由自己的目录配置：`<DM_PLUGIN_HOME>/config/ssh/config.toml`（默认
`~/.config/dm/config/ssh/config.toml`，可用 `dm info ssh` 查看）。完整示例见
[config.example.toml](config.example.toml)，包含 `[defaults]`（port/username/auth/key）
与 `[test]`（connect_timeout）。优先级为命令行参数 > 配置文件 > 内置默认值；未知表、未知键或非法值会让命令直接失败并指出该文件。

## 数据与安全

- 服务器保存在 `<data dir>/servers.sqlite3`（`dm info ssh` 给出路径）。
- 密码与私钥口令用本机随机密钥 `.ssh-key`（权限 `0600`）做 AES-GCM 加密后存储，`dm ssh list` 不会回显它们。
- 普通导出不包含秘密；包含秘密的导出以口令派生密钥加密，导入时再用目标机器的本地密钥加密保存。请妥善保管加密导出文件和口令。
- 指定 `--file` 的导出文件默认拒绝覆盖，并在 Unix 上以 `0600` 权限创建。
- `dm uninstall ssh` 默认保留配置、连接与缓存，`doctor --repair` 不会清理主动保留的数据。`dm uninstall ssh --purge` 才清空，需确认或显式 `--yes`。

## 源码导航

- `src/cli/`：参数与命令处理。
- `src/domain/`：连接行为与业务接口。
- `src/storage/`：配置、保存记录和机器密钥。
- `src/transfer/`：导出格式、校验和事务导入。
- `src/ui/`：交互输入、列表与错误提示。

通用加密字节、编码和安全文件写入使用 workspace 内部的 `dm-plugin-support`；从仓库根目录构建此插件。公开 Rust 导入路径与原有保存数据保持兼容。空列表会给出新增记录提示，自动化可继续使用 `list --json`。

## 编辑、诊断和补全

- `dm ssh edit <name> [--host H] [--port P] [--username U]`：省略的字段与认证秘密默认保留；终端下回车保留原值，保存前确认摘要，`--yes` 跳过确认。
- `dm ssh config init/show/path`：安全创建示例、查看有效配置与来源、定位文件；`show --json` 用于自动化。
- `dm ssh doctor [--json]`：检查配置与连接存储，发现问题返回非零。
- 动态补全包含插件子命令、参数、文件路径和保存的连接名称；按宿主的 `dm completions <shell>` 安装即可，不需另装插件补全脚本。

完整安装方式见 [补全说明](../../docs/usability.md)。

SSH 诊断还检查 `ssh`、密码认证所需的 `sshpass` 与保存的本机私钥路径，不会连接远端。`connect` 省略名称时，一个连接直接使用，多个连接在终端下可搜索选择。
