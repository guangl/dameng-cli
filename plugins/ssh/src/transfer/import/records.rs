//! The rules an imported SSH server record must satisfy.

use crate::storage::servers::{Server, validate_name};
use crate::transfer::export::PortableServer;
use anyhow::{Result, ensure};
use std::collections::HashSet;

/// Reject a document whose servers could never have been saved by "dm ssh add".
pub(crate) fn validate(servers: &[PortableServer]) -> Result<()> {
    let mut names = HashSet::new();
    for item in servers {
        validate_record(item)?;
        ensure!(
            names.insert(&item.name),
            "Export contains duplicate SSH server '{}'",
            item.name
        );
    }
    Ok(())
}

fn validate_record(item: &PortableServer) -> Result<()> {
    validate_name(&item.name)?;
    ensure!(
        item.port > 0,
        "SSH server '{}' has an invalid port",
        item.name
    );
    ensure!(
        !item.host.trim().is_empty(),
        "SSH server '{}' has an empty host",
        item.name
    );
    ensure!(
        !item.username.trim().is_empty(),
        "SSH server '{}' has an empty username",
        item.name
    );
    match item.auth_type.as_str() {
        "password" => ensure!(
            item.key_path.is_none(),
            "SSH server '{}' uses password authentication but includes a key path",
            item.name
        ),
        "key" => ensure!(
            item.key_path
                .as_deref()
                .is_some_and(|path| !path.trim().is_empty()),
            "SSH server '{}' uses key authentication without a key path",
            item.name
        ),
        other => anyhow::bail!(
            "SSH server '{}' has unknown authentication method '{other}'; choose 'password' or 'key'",
            item.name
        ),
    }
    Ok(())
}

/// The secret an import without secrets may keep: only a server that still uses
/// the same authentication method (and, for key authentication, the same key)
/// can reuse its stored password or key passphrase.
pub(crate) fn retained_secret(existing: Option<&Server>, item: &PortableServer) -> Option<String> {
    let existing = existing?;
    if existing.auth_type != item.auth_type {
        return None;
    }
    if item.auth_type == "key" && existing.key_path.as_deref() != item.key_path.as_deref() {
        return None;
    }
    existing.secret.clone()
}
