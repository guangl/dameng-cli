//! Native SSH transport: no executable lookup or secret-bearing child process.
use crate::{Server, decrypt, load_config, load_servers};
use anyhow::{Context, Result, ensure};
use dm_plugin_sdk::Context as PluginContext;
use russh::{
    client,
    keys::{PrivateKeyWithHashAlg, PublicKeyOrCertificate, load_secret_key},
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

struct Client {
    host: String,
    port: u16,
    known_hosts: PathBuf,
    observed: Arc<Mutex<Option<russh::keys::ssh_key::PublicKey>>>,
}
impl client::Handler for Client {
    type Error = anyhow::Error;
    async fn check_server_key(&mut self, key: &PublicKeyOrCertificate) -> Result<bool> {
        let PublicKeyOrCertificate::PublicKey { key, .. } = key else {
            anyhow::bail!("Host key verification: certificates are not supported");
        };
        if self.known_hosts.exists() {
            std::fs::read_to_string(&self.known_hosts).context("Read SSH known_hosts")?;
        }
        let known = russh::keys::known_hosts::known_host_keys_path(
            &self.host,
            self.port,
            &self.known_hosts,
        )?;
        ensure!(
            known.is_empty() || known.iter().any(|(_, saved)| saved == key),
            "Host key verification failed: remote host identification changed"
        );
        *self.observed.lock().unwrap() = Some(key.clone());
        Ok(true)
    }
}

pub fn server_by_name(context: &PluginContext, name: &str) -> Result<Server> {
    load_servers(context)?
        .into_iter()
        .find(|server| server.name == name)
        .with_context(|| format!("SSH server '{name}' is not configured"))
}

pub(crate) async fn authenticate(
    context: &PluginContext,
    server: &Server,
) -> Result<client::Handle<impl client::Handler>> {
    let seconds = load_config(context)?
        .test
        .connect_timeout
        .unwrap_or(crate::storage::config::DEFAULT_CONNECT_TIMEOUT);
    let timeout = Duration::from_secs(seconds);
    let secret = server
        .secret
        .as_deref()
        .map(|value| decrypt(context, value))
        .transpose()?
        .map(String::from_utf8)
        .transpose()?;
    let known_hosts = context.data_dir.join("known_hosts");
    let observed = Arc::new(Mutex::new(None));
    let handler = Client {
        host: server.host.clone(),
        port: server.port,
        known_hosts: known_hosts.clone(),
        observed: observed.clone(),
    };
    let result = tokio::time::timeout(timeout, async {
        let mut session = client::connect(
            Arc::new(client::Config::default()),
            (server.host.as_str(), server.port),
            handler,
        )
        .await
        .context("SSH connection failed")?;
        let auth = match server.auth_type.as_str() {
            "password" => {
                session
                    .authenticate_password(
                        &server.username,
                        secret
                            .as_deref()
                            .context("SSH password is not configured")?,
                    )
                    .await?
            }
            "key" => {
                let path = server
                    .key_path
                    .as_deref()
                    .context("SSH key path is not configured")?;
                let key = load_secret_key(path, secret.as_deref())
                    .context("Load SSH private key or passphrase failed")?;
                let hash = session.best_supported_rsa_hash().await?.flatten();
                session
                    .authenticate_publickey(
                        &server.username,
                        PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                    )
                    .await?
            }
            _ => anyhow::bail!("Unknown SSH authentication method"),
        };
        ensure!(
            auth.success(),
            "SSH authentication failed for '{}'",
            server.name
        );
        Ok::<_, anyhow::Error>(session)
    })
    .await
    .context("SSH connection/authentication timed out")??;
    // First successful authentication pins the host key; later changes fail.
    let key = observed
        .lock()
        .unwrap()
        .clone()
        .context("Missing SSH host key")?;
    std::fs::create_dir_all(&context.data_dir)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(context.data_dir.join("known_hosts.lock"))?;
    fs2::FileExt::lock_exclusive(&lock)?;
    let known =
        russh::keys::known_hosts::known_host_keys_path(&server.host, server.port, &known_hosts)?;
    ensure!(
        known.is_empty() || known.iter().any(|(_, saved)| saved == &key),
        "Host key verification failed: remote host identification changed"
    );
    if known.is_empty() {
        russh::keys::known_hosts::learn_known_hosts_path(
            &server.host,
            server.port,
            &key,
            &known_hosts,
        )?;
    }
    Ok(result)
}

pub fn test_server(context: &PluginContext, server: &Server) -> Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let session = authenticate(context, server).await?;
        session
            .disconnect(russh::Disconnect::ByApplication, "", "")
            .await?;
        Ok(())
    })
}
