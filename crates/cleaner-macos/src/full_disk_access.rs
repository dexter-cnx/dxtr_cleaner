use std::path::PathBuf;

#[cfg(target_os = "macos")]
use std::{
    fs,
    io::ErrorKind,
    path::Path,
};

use crate::PermissionStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullDiskAccessReport {
    pub status: PermissionStatus,
    pub probe_path: Option<PathBuf>,
    pub detail: String,
}

pub(crate) fn probe_full_disk_access(home: Option<PathBuf>) -> FullDiskAccessReport {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = home;
        return FullDiskAccessReport {
            status: PermissionStatus::Unknown,
            probe_path: None,
            detail: "Full Disk Access is only available on macOS".into(),
        };
    }

    #[cfg(target_os = "macos")]
    probe_macos_full_disk_access(home)
}

#[cfg(target_os = "macos")]
fn probe_macos_full_disk_access(home: Option<PathBuf>) -> FullDiskAccessReport {
    let Some(home) = home else {
        return FullDiskAccessReport {
            status: PermissionStatus::Unknown,
            probe_path: None,
            detail: "HOME is not set; Full Disk Access could not be probed".into(),
        };
    };

    for relative in ["Library/Mail", "Library/Messages", "Library/Safari"] {
        let path = home.join(relative);
        match probe_directory(&path) {
            ProbeResult::Missing => continue,
            ProbeResult::Granted => {
                return FullDiskAccessReport {
                    status: PermissionStatus::Granted,
                    probe_path: Some(path),
                    detail: "read-only probe succeeded for a protected user-data directory".into(),
                };
            }
            ProbeResult::Denied => {
                return FullDiskAccessReport {
                    status: PermissionStatus::Denied,
                    probe_path: Some(path),
                    detail: "read-only probe was denied for a protected user-data directory".into(),
                };
            }
            ProbeResult::Inconclusive(error) => {
                return FullDiskAccessReport {
                    status: PermissionStatus::Unknown,
                    probe_path: Some(path),
                    detail: format!("Full Disk Access probe was inconclusive: {error}"),
                };
            }
        }
    }

    FullDiskAccessReport {
        status: PermissionStatus::Unknown,
        probe_path: None,
        detail: "no protected probe directory exists for this user; status is unknown".into(),
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug, PartialEq, Eq)]
enum ProbeResult {
    Missing,
    Granted,
    Denied,
    Inconclusive(String),
}

#[cfg(target_os = "macos")]
fn probe_directory(path: &Path) -> ProbeResult {
    match fs::read_dir(path) {
        Ok(_) => ProbeResult::Granted,
        Err(error) if error.kind() == ErrorKind::NotFound => ProbeResult::Missing,
        Err(error) if error.kind() == ErrorKind::PermissionDenied => ProbeResult::Denied,
        Err(error) => ProbeResult::Inconclusive(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_platform_is_unknown() {
        #[cfg(not(target_os = "macos"))]
        {
            let report = probe_full_disk_access(Some(PathBuf::from("/tmp/Library/Mail")));
            assert_eq!(report.status, PermissionStatus::Unknown);
            assert!(report.probe_path.is_none());
        }
    }

    #[cfg(target_os = "macos")]
    mod macos {
        use super::*;
        use std::time::{SystemTime, UNIX_EPOCH};

        fn temp_home(test_name: &str) -> PathBuf {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock must be after epoch")
                .as_nanos();
            let home = std::env::temp_dir().join(format!(
                "dxtr-cleaner-fda-{test_name}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&home).expect("temp home must be created");
            home
        }

        #[test]
        fn missing_home_is_unknown() {
            let report = probe_full_disk_access(None);
            assert_eq!(report.status, PermissionStatus::Unknown);
            assert!(report.probe_path.is_none());
        }

        #[test]
        fn readable_protected_probe_reports_granted() {
            let home = temp_home("granted");
            let mail = home.join("Library/Mail");
            fs::create_dir_all(&mail).expect("probe directory must be created");

            let report = probe_full_disk_access(Some(home.clone()));

            assert_eq!(report.status, PermissionStatus::Granted);
            assert_eq!(report.probe_path.as_deref(), Some(mail.as_path()));
            fs::remove_dir_all(home).expect("temp home must be removed");
        }

        #[test]
        fn absent_probe_directories_are_unknown() {
            let home = temp_home("missing");
            let report = probe_full_disk_access(Some(home.clone()));
            assert_eq!(report.status, PermissionStatus::Unknown);
            assert!(report.probe_path.is_none());
            fs::remove_dir_all(home).expect("temp home must be removed");
        }
    }
}
