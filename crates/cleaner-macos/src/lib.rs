use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionStatus {
    Unknown,
    Granted,
    Denied,
}

pub trait MacPlatform {
    fn full_disk_access_status(&self) -> PermissionStatus;
    fn open_full_disk_access_settings(&self) -> Result<(), String>;
    fn reveal_in_finder(&self, path: &Path) -> Result<(), String>;
    fn move_to_trash(&self, path: &Path) -> Result<(), String>;
    fn installed_application_paths(&self) -> Result<Vec<PathBuf>, String>;
}

#[derive(Debug, Default)]
pub struct SystemMacPlatform;

impl MacPlatform for SystemMacPlatform {
    fn full_disk_access_status(&self) -> PermissionStatus {
        PermissionStatus::Unknown
    }

    fn open_full_disk_access_settings(&self) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            run_open(["x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles"])
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err("macOS only".into())
        }
    }

    fn reveal_in_finder(&self, path: &Path) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            let status = std::process::Command::new("open")
                .arg("-R")
                .arg(path)
                .status()
                .map_err(|error| error.to_string())?;

            if status.success() {
                Ok(())
            } else {
                Err(format!("open -R failed with status {status}"))
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = path;
            Err("macOS only".into())
        }
    }

    fn move_to_trash(&self, _path: &Path) -> Result<(), String> {
        Err("M0 safety lock: trash execution is not implemented".into())
    }

    fn installed_application_paths(&self) -> Result<Vec<PathBuf>, String> {
        let mut roots = vec![PathBuf::from("/Applications")];
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home).join("Applications"));
        }
        Ok(roots)
    }
}

#[cfg(target_os = "macos")]
fn run_open<const N: usize>(args: [&str; N]) -> Result<(), String> {
    let status = std::process::Command::new("open")
        .args(args)
        .status()
        .map_err(|error| error.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("open failed with status {status}"))
    }
}
