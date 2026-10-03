mod completion;
mod directories;
mod doctor;
mod download;
mod git;
mod home;
mod hooks;
mod install;
mod package;
mod prebuilt;
mod query;
mod release;
mod run;
mod types;
mod uninstall;
mod update;
mod util;

pub(crate) use self::{
    directories::RESERVED_HOME_ENTRIES, download::*, git::checkout_git, types::InstallMode, util::*,
};
pub use self::{
    home::{PluginStore, home_from_env},
    prebuilt::*,
    types::{DoctorReport, PluginInfo, UpdateStatus},
};
