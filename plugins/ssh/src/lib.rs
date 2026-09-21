use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use dm_plugin_sdk::{Context as PluginContext, Plugin, PluginResult};
use rand::{RngCore, rngs::OsRng};
use rusqlite::{Connection, params};
use std::{ffi::OsString, fs, path::PathBuf, process::Command};

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
    /// Add or replace a saved SSH server.
    Add {
        name: String,
        #[arg(long)]
        host: String,
        #[arg(long, default_value_t = 22)]
        port: u16,
        #[arg(long)]
        username: String,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        key: Option<PathBuf>,
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
                Ok(1)
            }
        }
    }
}

pub fn database_path(context: &PluginContext) -> PathBuf {
    context.home.join("store.sqlite3")
}

pub fn key_path(context: &PluginContext) -> PathBuf {
    context.home.join(".ssh-key")
}

pub fn open_database(context: &PluginContext) -> Result<Connection> {
    let connection =
        Connection::open(database_path(context)).context("Open shared SQLite store")?;
    connection.execute_batch(dm_plugin_sdk::SSH_SERVERS_TABLE_SCHEMA)?;
    Ok(connection)
}

pub fn machine_key(context: &PluginContext) -> Result<[u8; 32]> {
    let path = key_path(context);
    if path.is_file() {
        let bytes = fs::read(&path).context("Read shared SSH encryption key")?;
        ensure!(
            bytes.len() == 32,
            "Shared SSH encryption key is invalid; remove {} and retry",
            path.display()
        );
        let mut key = [0_u8; 32];
        key.copy_from_slice(&bytes);
        return Ok(key);
    }
    let mut key = [0_u8; 32];
    OsRng.fill_bytes(&mut key);
    fs::write(&path, key).context("Write shared SSH encryption key")?;
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
            validate_name(&name)?;
            ensure!(!host.is_empty(), "SSH host must not be empty");
            ensure!(!username.is_empty(), "SSH username must not be empty");
            let (auth_type, key_path, secret) = if let Some(key_path) = key {
                ensure!(
                    !key_path.as_os_str().is_empty(),
                    "SSH key path must not be empty"
                );
                (
                    "key".to_owned(),
                    Some(key_path.display().to_string()),
                    passphrase
                        .map(|passphrase| encrypt(context, passphrase.as_bytes()))
                        .transpose()?,
                )
            } else {
                let password = password.context("SSH password or key path is required")?;
                ensure!(!password.is_empty(), "SSH password must not be empty");
                (
                    "password".to_owned(),
                    None,
                    Some(encrypt(context, password.as_bytes())?),
                )
            };
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
