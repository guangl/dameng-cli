use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::{Context, Error, Result, ensure};
use clap::{Parser, Subcommand};
use dm_plugin_sdk::{Context as PluginContext, Plugin, PluginResult};
use rand::{RngCore, rngs::OsRng};
use rusqlite::{Connection, params};
use std::{ffi::OsString, fs, io::IsTerminal, path::PathBuf, process::Command};

pub struct Server {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub key_path: Option<String>,
    pub secret: Option<String>,
}

#[derive(Parser)]
#[command(name = "dm ssh", about = "Manage saved SSH server connections")]
struct Cli {
    #[command(subcommand)]
    command: SshCommand,
}

#[derive(Subcommand)]
enum SshCommand {
    /// Add or replace a saved SSH server; prompts interactively for any value that is omitted.
    Add {
        /// SSH server name (prompted when omitted on a terminal).
        name: Option<String>,
        /// SSH host (prompted when omitted on a terminal).
        #[arg(long)]
        host: Option<String>,
        /// SSH port, default 22 (prompted when omitted on a terminal).
        #[arg(long)]
        port: Option<u16>,
        /// SSH username (prompted when omitted on a terminal).
        #[arg(long)]
        username: Option<String>,
        /// Password for password authentication; prompted when omitted on a terminal.
        #[arg(long)]
        password: Option<String>,
        /// Private key path for key authentication; prompted when key authentication is chosen.
        #[arg(long)]
        key: Option<PathBuf>,
        /// Key passphrase; prompted when omitted on a terminal.
        #[arg(long)]
        passphrase: Option<String>,
    },
    /// List saved SSH servers.
    List,
    /// Remove a saved SSH server.
    Remove { name: String },
    /// Test an SSH connection non-interactively.
    Test { name: String },
    /// Start an interactive SSH session.
    Ssh { name: String },
}

pub struct SshPlugin;

impl Plugin for SshPlugin {
    fn run(&self, context: PluginContext) -> PluginResult {
        match run_cli(&context) {
            Ok(code) => Ok(code),
            Err(error) => {
                eprintln!("dm ssh: {error:#}");
                eprintln!("提示：{}", ssh_hint(&error));
                Ok(1)
            }
        }
    }
}

const SERVERS_TABLE_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS servers (
    name TEXT PRIMARY KEY,
    host TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 22,
    username TEXT NOT NULL,
    auth_type TEXT NOT NULL DEFAULT 'password',
    secret TEXT,
    key_path TEXT,
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT";

pub fn database_path(context: &PluginContext) -> PathBuf {
    context.data_dir.join("servers.sqlite3")
}

pub fn key_path(context: &PluginContext) -> PathBuf {
    context.data_dir.join(".ssh-key")
}

pub fn open_database(context: &PluginContext) -> Result<Connection> {
    fs::create_dir_all(&context.data_dir).context("Create SSH plugin data directory")?;
    let connection = Connection::open(database_path(context)).context("Open SSH servers store")?;
    connection.execute_batch(SERVERS_TABLE_SCHEMA)?;
    Ok(connection)
}

pub fn machine_key(context: &PluginContext) -> Result<[u8; 32]> {
    fs::create_dir_all(&context.data_dir).context("Create SSH plugin data directory")?;
    let path = key_path(context);
    if path.is_file() {
        let bytes = fs::read(&path).context("Read SSH encryption key")?;
        ensure!(
            bytes.len() == 32,
            "SSH encryption key is invalid; remove {} and retry",
            path.display()
        );
        let mut key = [0_u8; 32];
        key.copy_from_slice(&bytes);
        return Ok(key);
    }
    let mut key = [0_u8; 32];
    OsRng.fill_bytes(&mut key);
    fs::write(&path, key).context("Write SSH encryption key")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(key)
}

pub fn encrypt(context: &PluginContext, plaintext: &[u8]) -> Result<String> {
    let key = machine_key(context)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| anyhow::anyhow!("Encrypt SSH secret"))?;
    Ok(format!("{}{}", hex(&nonce), hex(&ciphertext)))
}

pub fn decrypt(context: &PluginContext, text: &str) -> Result<Vec<u8>> {
    let bytes = unhex(text)?;
    ensure!(bytes.len() >= 12, "Invalid encrypted SSH secret length");
    let (nonce, ciphertext) = bytes.split_at(12);
    let key = machine_key(context)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| anyhow::anyhow!("Decrypt SSH secret"))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn unhex(text: &str) -> Result<Vec<u8>> {
    ensure!(
        text.len() % 2 == 0 && text.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Expected hexadecimal text"
    );
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).map_err(Into::into))
        .collect()
}

pub fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty() && name.len() <= 64,
        "SSH server name must contain 1–64 characters"
    );
    ensure!(
        name.bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')),
        "SSH server name may only contain a-z, A-Z, 0-9, '-', '_' and '.'"
    );
    Ok(())
}

pub fn load_servers(context: &PluginContext) -> Result<Vec<Server>> {
    let connection = open_database(context)?;
    let mut statement = connection.prepare(
        "SELECT name, host, port, username, auth_type, key_path, secret
         FROM servers ORDER BY name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(Server {
            name: row.get(0)?,
            host: row.get(1)?,
            port: row.get(2)?,
            username: row.get(3)?,
            auth_type: row.get(4)?,
            key_path: row.get(5)?,
            secret: row.get(6)?,
        })
    })?;
    rows.collect::<rusqlite::Result<_>>().map_err(Into::into)
}

pub fn upsert_server(context: &PluginContext, server: &Server) -> Result<()> {
    let connection = open_database(context)?;
    connection.execute(
        "INSERT INTO servers (name, host, port, username, auth_type, secret, key_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(name) DO UPDATE SET host = excluded.host,
             port = excluded.port,
             username = excluded.username,
             auth_type = excluded.auth_type,
             secret = excluded.secret,
             key_path = excluded.key_path,
             updated_at = unixepoch()",
        params![
            server.name,
            server.host,
            server.port,
            server.username,
            server.auth_type,
            server.secret,
            server.key_path,
        ],
    )?;
    Ok(())
}

pub fn remove_server(context: &PluginContext, name: &str) -> Result<()> {
    let connection = open_database(context)?;
    ensure!(
        connection.execute("DELETE FROM servers WHERE name = ?1", [name])? == 1,
        "SSH server '{name}' is not configured"
    );
    Ok(())
}

const AUTH_REQUIRED: &str =
    "SSH password or key path is required; pass --password or --key, or run from a terminal";

/// Read one line from stdin, trimming surrounding whitespace. Prompts are
/// written to stdout so they appear before input is read on an interactive
/// terminal.
fn prompt_line(prompt: &str) -> Result<String> {
    use std::io::Write;
    print!("{prompt}");
    std::io::stdout().flush().context("Flush prompt")?;
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .context("Read input")?;
    Ok(input.trim().to_owned())
}

/// Resolve a required plain-text field, prompting interactively when it was not
/// supplied and stdin is attached to a terminal.
pub fn resolve_required(
    value: Option<String>,
    prompt: &str,
    missing: &str,
    interactive: bool,
) -> Result<String> {
    match value {
        Some(value) => Ok(value),
        None if interactive => prompt_line(prompt),
        None => anyhow::bail!("{missing}"),
    }
}

/// Resolve the SSH port, defaulting to 22 when omitted.
pub fn resolve_port(port: Option<u16>, interactive: bool) -> Result<u16> {
    match port {
        Some(port) => Ok(port),
        None if interactive => {
            let value = prompt_line("Port [22]: ")?;
            if value.is_empty() {
                Ok(22)
            } else {
                value
                    .parse::<u16>()
                    .with_context(|| format!("SSH port must be a number, got '{value}'"))
            }
        }
        None => Ok(22),
    }
}

/// Resolve the password for `add`, prompting on the terminal when one was not
/// supplied and the process is attached to a terminal.
pub fn resolve_password(password: Option<String>, interactive: bool) -> Result<String> {
    match password {
        Some(password) => {
            ensure!(!password.is_empty(), "SSH password must not be empty");
            Ok(password)
        }
        None if interactive => {
            let password = rpassword::prompt_password("Password: ").context("Read SSH password")?;
            ensure!(!password.is_empty(), "SSH password must not be empty");
            Ok(password)
        }
        None => anyhow::bail!(AUTH_REQUIRED),
    }
}

/// Resolve the optional key passphrase for `add`. An empty passphrase means the
/// key is not protected by one. Prompting only happens on a terminal.
pub fn resolve_passphrase(passphrase: Option<String>, interactive: bool) -> Result<Option<String>> {
    match passphrase {
        Some(passphrase) if passphrase.is_empty() => Ok(None),
        Some(passphrase) => Ok(Some(passphrase)),
        None if interactive => {
            let passphrase = rpassword::prompt_password("Passphrase (leave empty for none): ")
                .context("Read SSH key passphrase")?;
            Ok((!passphrase.is_empty()).then_some(passphrase))
        }
        None => Ok(None),
    }
}

/// Resolve the authentication method and its encrypted secret for `add`.
/// Explicit `--key` or `--password` wins; otherwise an interactive terminal
/// chooses the method and enters the matching secret.
pub fn resolve_auth(
    context: &PluginContext,
    password: Option<String>,
    key: Option<PathBuf>,
    passphrase: Option<String>,
    interactive: bool,
) -> Result<(String, Option<String>, Option<String>)> {
    if let Some(key_path) = key {
        ensure!(
            !key_path.as_os_str().is_empty(),
            "SSH key path must not be empty"
        );
        let passphrase = resolve_passphrase(passphrase, interactive)?;
        return Ok((
            "key".to_owned(),
            Some(key_path.display().to_string()),
            passphrase
                .map(|passphrase| encrypt(context, passphrase.as_bytes()))
                .transpose()?,
        ));
    }
    if let Some(password) = password {
        let password = resolve_password(Some(password), interactive)?;
        return Ok((
            "password".to_owned(),
            None,
            Some(encrypt(context, password.as_bytes())?),
        ));
    }
    if !interactive {
        anyhow::bail!(AUTH_REQUIRED);
    }
    let method = prompt_line("Authentication method [password/key] (password): ")?;
    match method.as_str() {
        "password" | "" => {
            let password = resolve_password(None, true)?;
            Ok((
                "password".to_owned(),
                None,
                Some(encrypt(context, password.as_bytes())?),
            ))
        }
        "key" => {
            let key_path = prompt_line("Key path: ")?;
            ensure!(!key_path.is_empty(), "SSH key path must not be empty");
            let passphrase = resolve_passphrase(None, true)?;
            Ok((
                "key".to_owned(),
                Some(key_path),
                passphrase
                    .map(|passphrase| encrypt(context, passphrase.as_bytes()))
                    .transpose()?,
            ))
        }
        other => {
            anyhow::bail!("Unknown authentication method '{other}'; choose 'password' or 'key'")
        }
    }
}

pub fn run_cli(context: &PluginContext) -> Result<i32> {
    let mut argv = vec![OsString::from("dm ssh")];
    argv.extend(context.args.iter().cloned());
    let cli = Cli::parse_from(argv);
    match cli.command {
        SshCommand::Add {
            name,
            host,
            port,
            username,
            password,
            key,
            passphrase,
        } => {
            let interactive = std::io::stdin().is_terminal();
            let name =
                resolve_required(name, "Name: ", "SSH server name is required", interactive)?;
            validate_name(&name)?;
            let host = resolve_required(host, "Host: ", "SSH host is required", interactive)?;
            ensure!(!host.is_empty(), "SSH host must not be empty");
            let port = resolve_port(port, interactive)?;
            let username = resolve_required(
                username,
                "Username: ",
                "SSH username is required",
                interactive,
            )?;
            ensure!(!username.is_empty(), "SSH username must not be empty");
            let (auth_type, key_path, secret) =
                resolve_auth(context, password, key, passphrase, interactive)?;
            upsert_server(
                context,
                &Server {
                    name: name.clone(),
                    host,
                    port,
                    username,
                    auth_type,
                    key_path,
                    secret,
                },
            )?;
            println!("Saved SSH server {name}");
        }
        SshCommand::List => {
            for server in load_servers(context)? {
                println!(
                    "{}\t{}@{}:{}\t{}\t{}",
                    server.name,
                    server.username,
                    server.host,
                    server.port,
                    server.auth_type,
                    server.key_path.as_deref().unwrap_or("")
                );
            }
        }
        SshCommand::Remove { name } => {
            remove_server(context, &name)?;
            println!("Removed SSH server {name}");
        }
        SshCommand::Test { name } => {
            let mut command = ssh_command(context, &name, true)?;
            let status = command
                .status()
                .context("Test SSH server; password authentication requires sshpass")?;
            ensure!(status.success(), "SSH connection failed for '{name}'");
            println!("SSH server {name} is reachable");
        }
        SshCommand::Ssh { name } => {
            let mut command = ssh_command(context, &name, false)?;
            let status = command
                .status()
                .context("Start SSH session; password authentication requires sshpass")?;
            let code = match status.code() {
                Some(code) => code,
                None => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::process::ExitStatusExt;
                        status.signal().map(|signal| 128 + signal).unwrap_or(0)
                    }
                    #[cfg(not(unix))]
                    {
                        anyhow::bail!("SSH session terminated without an exit code")
                    }
                }
            };
            return Ok(code);
        }
    }
    Ok(0)
}

/// Return a short, actionable hint for an SSH plugin error.
fn ssh_hint(error: &Error) -> String {
    let text = error
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();

    if text.contains("not configured") {
        return "请先运行 `dm ssh add <name>` 配置服务器，或用 `dm ssh list` 查看已保存的连接。"
            .into();
    }
    if text.contains("sshpass") {
        return "密码认证的测试/登录需要安装 `sshpass`；也可改用密钥认证。".into();
    }
    if text.contains("password") || text.contains("key") {
        return "请通过 `--password` 或 `--key` 提供认证，或在终端下运行以交互输入。".into();
    }
    if text.contains("sqlite") || text.contains("table") || text.contains("store") {
        return "SSH 数据存储异常，请检查插件数据目录中的 servers.sqlite3。".into();
    }
    if text.contains("required") {
        return "缺少必填项；在终端下运行可交互输入，或显式传入对应参数。".into();
    }

    "使用 `dm ssh --help` 查看可用子命令和参数。".into()
}

pub fn ssh_command(context: &PluginContext, name: &str, test: bool) -> Result<Command> {
    let server = load_servers(context)?
        .into_iter()
        .find(|server| server.name == name)
        .with_context(|| format!("SSH server '{name}' is not configured"))?;
    let destination = format!("{}@{}", server.username, server.host);
    let mut common: Vec<String> = vec!["-p".to_owned(), server.port.to_string()];
    if test {
        common.extend([
            "-o".to_owned(),
            "BatchMode=yes".to_owned(),
            "-o".to_owned(),
            "ConnectTimeout=10".to_owned(),
        ]);
    }
    let mut command;
    match server.auth_type.as_str() {
        "key" => {
            let key_path = server
                .key_path
                .as_deref()
                .context("SSH key path is not configured")?;
            command = Command::new("ssh");
            command.args(["-i", key_path]);
            command.args(&common);
        }
        _ => {
            let password = server
                .secret
                .as_deref()
                .map(|secret| decrypt(context, secret))
                .transpose()?
                .map(String::from_utf8)
                .transpose()?
                .context("SSH password is not configured")?;
            command = Command::new("sshpass");
            command.args(["-p", &password, "ssh"]);
            command.args(&common);
        }
    }
    command.arg(&destination);
    if test {
        command.arg("true");
    }
    Ok(command)
}
