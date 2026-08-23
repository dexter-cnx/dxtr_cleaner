use std::path::{Path, PathBuf};

use cleaner_core::{PermanentDeleteBackend, TrashBackend};

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
    fn permanent_delete(&self, path: &Path) -> Result<(), String>;
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

    fn move_to_trash(&self, path: &Path) -> Result<(), String> {
        move_path_to_trash(path)
    }

    fn permanent_delete(&self, path: &Path) -> Result<(), String> {
        let _ = path;
        Err(
            "permanent delete safety lock: requires anchored no-follow filesystem mutation"
                .into(),
        )
    }

    fn installed_application_paths(&self) -> Result<Vec<PathBuf>, String> {
        let mut roots = vec![PathBuf::from("/Applications")];
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home).join("Applications"));
        }
        Ok(roots)
    }
}

impl TrashBackend for SystemMacPlatform {
    fn move_to_trash(&self, path: &Path) -> Result<(), String> {
        MacPlatform::move_to_trash(self, path)
    }
}

impl PermanentDeleteBackend for SystemMacPlatform {
    fn permanent_delete(&self, path: &Path) -> Result<(), String> {
        MacPlatform::permanent_delete(self, path)
    }
}

#[cfg(target_os = "macos")]
fn move_path_to_trash(path: &Path) -> Result<(), String> {
    let status = std::process::Command::new("osascript")
        .arg("-e")
        .arg("on run argv")
        .arg("-e")
        .arg("tell application \"Finder\" to delete POSIX file (item 1 of argv)")
        .arg("-e")
        .arg("end run")
        .arg(path)
        .status()
        .map_err(|error| error.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("move to Trash failed with status {status}"))
    }
}

#[cfg(not(target_os = "macos"))]
fn move_path_to_trash(path: &Path) -> Result<(), String> {
    let _ = path;
    Err("macOS only".into())
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
