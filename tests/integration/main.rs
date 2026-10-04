//! Process-level tests for the `dm` host.
//!
//! These tests build the host binary (and tiny Rust plugins) with Cargo and
//! drive it through `Command`, so each module covers one area of the CLI
//! contract. Keep every file under the repository-wide 200-line limit and add
//! a new `mod` line whenever a file has to grow.

mod cli;
mod common;
mod completion_install;
mod config;
mod config_runtime;
mod doctor;
mod doctor_logs;
mod errors;
mod hooks;
mod host_directory_protection;
mod install;
mod install_check;
mod install_source;
mod installer;
mod legacy;
mod logging;
mod logging_options;
mod plugin_dirs;
mod plugin_migration;
mod plugin_names;
mod prebuilt;
mod prebuilt_release;
mod purge_retry;
mod self_update;
mod store;
mod store_tables;
mod update;

mod usability;

mod release_plugins;
mod reserved_plugin_recovery;

mod resources;

mod sqllog2db_release;
