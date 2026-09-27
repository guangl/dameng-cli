//! End-to-end tests: the plugin binary runs under the host protocol environment.
//! The driver behind test/exec is still a placeholder, so these tests only cover
//! the paths that fail before or at the driver, never a real connection.

mod common;
mod config;
mod connections;
mod exec;
mod failures;
mod help;
