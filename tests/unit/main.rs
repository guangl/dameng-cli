//! Library-level tests for the `dm` host.
//!
//! Every module is listed here so the whole target is one test binary; keep
//! each file under the repository-wide 200-line limit and add a new `mod`
//! line whenever a file has to grow.

mod cli_report;
mod cli_table;
mod config;
mod logging;
mod manifest;
mod self_update;
mod store;
