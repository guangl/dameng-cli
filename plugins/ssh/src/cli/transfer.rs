//! Import and export command handlers; document validation lives in transfer/.

use crate::private_file::write_private_file;
use crate::storage::servers::load_servers;
use crate::transfer::export::{ExportDocument, export_document};
use crate::transfer::import::import_document;
use crate::ui::prompts::Prompter;
use anyhow::{Context, Result};
use dm_plugin_sdk::Context as PluginContext;
use std::{io::Write, path::PathBuf};

pub(super) fn export(
    context: &PluginContext,
    file: Option<PathBuf>,
    include_secrets: bool,
    prompter: Option<&dyn Prompter>,
) -> Result<()> {
    let servers = load_servers(context)?;
    let document = export_document(context, servers, include_secrets, prompter)?;
    let json = serde_json::to_vec_pretty(&document)?;
    match file {
        Some(path) => {
            write_private_file(&path, &json)?;
            println!(
                "Exported {} SSH servers to {}",
                document.count,
                path.display()
            );
        }
        None => {
            std::io::stdout().write_all(&json)?;
            println!();
        }
    }
    Ok(())
}

pub(super) fn import(
    context: &PluginContext,
    file: PathBuf,
    replace: bool,
    prompter: Option<&dyn Prompter>,
) -> Result<()> {
    let document: ExportDocument = serde_json::from_slice(
        &dm_plugin_support::bounded::file(&file, dm_plugin_support::bounded::DOCUMENT_LIMIT)
            .with_context(|| format!("Read {}", file.display()))?,
    )
    .with_context(|| format!("Parse SSH server export {}", file.display()))?;
    let servers = import_document(context, document, replace, prompter)?;
    println!("Imported {servers} SSH servers");
    Ok(())
}
