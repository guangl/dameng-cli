//! Local checks do not attempt a network connection or decrypt secrets.
use anyhow::Result;
use serde::Serialize;
use std::path::Path;
#[derive(Default, Serialize)]
pub struct DiagnosticReport {
    pub issues: Vec<String>,
    pub checks: Vec<String>,
}
impl DiagnosticReport {
    pub fn tool(&mut self, name: &str) {
        let found = std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let path = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    path.metadata()
                        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                }
                #[cfg(not(unix))]
                {
                    path.is_file()
                }
            })
        });
        if found {
            self.checks.push(format!("已找到 {name}"));
        } else {
            self.issues.push(format!("缺少 {name}，请安装后重试"));
        }
    }
    pub fn key(&mut self, name: &str, path: &Path) {
        if !path.is_file() {
            self.issues
                .push(format!("{name}：私钥不存在：{}", path.display()));
        } else {
            self.checks.push(format!("{name}：私钥文件存在"));
        }
    }
    pub fn print(&self, json: bool) -> Result<i32> {
        if json {
            println!("{}", serde_json::to_string_pretty(self)?);
        } else {
            for check in &self.checks {
                println!("通过：{check}");
            }
            for issue in &self.issues {
                println!("问题：{issue}");
            }
            if self.issues.is_empty() {
                println!("检查完成，未发现问题。");
            }
        }
        Ok(i32::from(!self.issues.is_empty()))
    }
}
