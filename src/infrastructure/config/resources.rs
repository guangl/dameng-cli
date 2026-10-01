use super::{Config, configured_value};
use anyhow::{Context, Result, ensure};

pub(super) fn validate_workers(workers: usize) -> Result<usize> {
    ensure!(
        (1..=16).contains(&workers),
        "update.check_concurrency must be between 1 and 16"
    );
    Ok(workers)
}

impl Config {
    pub fn update_check_concurrency(&self) -> Result<usize> {
        let workers = match configured_value(std::env::var("DM_UPDATE_CHECK_CONCURRENCY").ok()) {
            Some(value) => value
                .parse()
                .context("Invalid DM_UPDATE_CHECK_CONCURRENCY")?,
            None => self.update.check_concurrency.unwrap_or(4),
        };
        validate_workers(workers)
    }
}
