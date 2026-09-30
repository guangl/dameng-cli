//! Internal utilities shared by the built-in database and SSH plugins.
//! The public plugin protocol lives in `dm-plugin-sdk`.

pub mod codec;
pub mod completion;
pub mod config;
pub mod diagnostics;
pub mod interaction;
pub mod private_file;
pub mod secrets;
mod suggestions;
