mod doctor;
mod download;
mod git;
mod home;
mod hooks;
mod install;
mod package;
mod prebuilt;
mod query;
mod run;
mod types;
mod uninstall;
mod update;
mod util;

pub(crate) use self::{
    download::*, git::checkout_git, home::home_from_env, types::InstallMode, util::*,
};
pub use self::{
    home::PluginStore,
    prebuilt::*,
    types::{DoctorReport, PluginInfo, UpdateStatus},
};
