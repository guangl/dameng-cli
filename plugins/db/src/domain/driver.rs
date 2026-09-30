//! Database driver placeholder.
//!
//! The plugin keeps the Database/Session interfaces and the "dm db test" /
//! "dm db exec" commands, but the driver itself is deferred: opening a driver
//! reports that instead of pretending to connect. Saving, listing and removing
//! connections never touch a driver and keep working.

use crate::{Database, DatabaseFactory};
use anyhow::Result;

/// Factory used by the commands until a real driver is wired in.
pub struct PendingFactory;

impl DatabaseFactory for PendingFactory {
    fn open(&self) -> Result<Box<dyn Database>> {
        anyhow::bail!(
            "The database driver is not implemented yet; dm db currently manages saved connections only"
        )
    }
}
