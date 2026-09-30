//! Process-level tests for the `dm` host.
//!
//! These tests build the host binary (and tiny Rust plugins) with Cargo and
//! drive it through `Command`, so each module covers one area of the CLI
//! contract. Keep every file under the repository-wide 200-line limit and add
//! a new `mod` line whenever a file has to grow.

mod cli;
mod common;
mod config;
mod config_runtime;
mod doctor;
mod errors;
mod hooks;
mod install;
mod install_source;
mod installer;
mod logging;
mod logging_options;
mod prebuilt;
mod prebuilt_release;
mod self_update;
mod store;
mod update;

mod usability;

mod release_plugins;

mod resources;
