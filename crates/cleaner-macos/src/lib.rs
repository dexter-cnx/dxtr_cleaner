mod app_metadata;

use std::{
    fs,
    path::{Path, PathBuf},
};

use app_metadata::extract_application_metadata;
use cleaner_core::{
    ApplicationInventory, ApplicationInventoryIssue, ApplicationInventoryReport,
    ApplicationLocation, InstalledApplication, PermanentDeleteBackend, TrashBackend,
};

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
        Err("permanent delete safety lock: requires anchored no-follow filesystem mutation".into())
    }

    fn installed_application_paths(&self) -> Result<Vec<PathBuf>, String> {
        Ok(self
            .inventory()
            .applications
            .into_iter()
            .map(|application| application.path)
            .collect())
    }
}

impl ApplicationInventory for SystemMacPlatform {
    fn inventory(&self) -> ApplicationInventoryReport {
        #[cfg(target_os = "macos")]
        {
            let mut roots = vec![
                (ApplicationLocation::Local, PathBuf::from("/Applications")),
                (
                    ApplicationLocation::System,
                    PathBuf::from("/System/Applications"),
                ),
            ];
            if let Some(home) = std::env::var_os("HOME") {
                roots.push((
                    ApplicationLocation::User,
                    PathBuf::from(home).join("Applications"),
                ));
            }
            inventory_roots(&roots)
        }
        #[cfg(not(target_os = "macos"))]
        {
            ApplicationInventoryReport {
                applications: Vec::new(),
                issues: vec![ApplicationInventoryIssue::new(
                    PathBuf::from("/Applications"),
                    "installed application inventory is available only on macOS",
                )],
            }
        }
    }
}

fn inventory_roots(roots: &[(ApplicationLocation, PathBuf)]) -> ApplicationInventoryReport {
    let mut report = ApplicationInventoryReport::default();
    for (location, root) in roots {
        collect_applications(root, *location, true, &mut report);
    }
    report.sort_deterministically();
    report
}

fn collect_applications(
    directory: &Path,
    location: ApplicationLocation,
    is_root: bool,
    report: &mut ApplicationInventoryReport,
) {
    let metadata = match fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(error) if is_root && error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            report.issues.push(ApplicationInventoryIssue::new(
                directory.to_path_buf(),
                error.to_string(),
            ));
            return;
        }
    };

    if metadata.file_type().is_symlink() {
        report.issues.push(ApplicationInventoryIssue::new(
            directory.to_path_buf(),
            "application inventory directory is a symlink and was skipped",
        ));
        return;
    }

    if !metadata.is_dir() {
        report.issues.push(ApplicationInventoryIssue::new(
            directory.to_path_buf(),
            "application inventory path is not a directory",
        ));
        return;
    }

    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            report.issues.push(ApplicationInventoryIssue::new(
                directory.to_path_buf(),
                error.to_string(),
            ));
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                report.issues.push(ApplicationInventoryIssue::new(
                    directory.to_path_buf(),
                    error.to_string(),
                ));
                continue;
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                report
                    .issues
                    .push(ApplicationInventoryIssue::new(path, error.to_string()));
                continue;
            }
        };

        if file_type.is_symlink() || !file_type.is_dir() {
            continue;
        }

        if is_app_bundle(&path) {
            let name = path
                .file_stem()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_default();
            let metadata = extract_application_metadata(&path);
            report
                .applications
                .push(InstalledApplication::new(path, name, location).with_metadata(metadata));
        } else {
            collect_applications(&path, location, false, report);
        }
    }
}

fn is_app_bundle(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(test_name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dxtr-cleaner-{test_name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temp root must be created");
        path
    }

    #[test]
    fn inventory_finds_app_bundles_in_nested_application_folders() {
        let root = temp_root("inventory-nested");
        let utilities = root.join("Utilities");
        fs::create_dir_all(utilities.join("Tool.app/Contents"))
            .expect("nested app fixture must be created");
        fs::create_dir_all(root.join("Direct.app/Contents"))
            .expect("direct app fixture must be created");
        fs::create_dir_all(root.join("NotAnApp/Child")).expect("non-app fixture must be created");

        let report = inventory_roots(&[(ApplicationLocation::Local, root.clone())]);

        assert_eq!(report.applications.len(), 2);
        assert_eq!(report.applications[0].path, root.join("Direct.app"));
        assert_eq!(report.applications[1].path, utilities.join("Tool.app"));
        assert!(report.issues.is_empty());

        fs::remove_dir_all(root).expect("temp root must be removed");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn inventory_extracts_info_plist_metadata() {
        let root = temp_root("inventory-metadata");
        let app = root.join("Example.app");
        fs::create_dir_all(app.join("Contents")).expect("app fixture must be created");
        fs::write(
            app.join("Contents/Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.example.cleaner-fixture</string>
<key>CFBundleVersion</key><string>42</string>
<key>CFBundleShortVersionString</key><string>1.2.3</string>
</dict></plist>"#,
        )
        .expect("Info.plist fixture must be written");

        let report = inventory_roots(&[(ApplicationLocation::Local, root.clone())]);
        let metadata = &report.applications[0].metadata;

        assert_eq!(
            metadata.bundle_identifier.as_deref(),
            Some("com.example.cleaner-fixture")
        );
        assert_eq!(metadata.bundle_version.as_deref(), Some("42"));
        assert_eq!(metadata.short_version.as_deref(), Some("1.2.3"));

        fs::remove_dir_all(root).expect("temp root must be removed");
    }

    #[cfg(unix)]
    #[test]
    fn inventory_does_not_follow_symlinked_directories() {
        use std::os::unix::fs::symlink;

        let root = temp_root("inventory-symlink");
        let outside = temp_root("inventory-symlink-outside");
        fs::create_dir_all(outside.join("Escaped.app/Contents"))
            .expect("outside app fixture must be created");
        symlink(&outside, root.join("Linked")).expect("directory symlink fixture must be created");

        let report = inventory_roots(&[(ApplicationLocation::Local, root.clone())]);

        assert!(report.applications.is_empty());
        assert!(report.issues.is_empty());

        fs::remove_dir_all(root).expect("temp root must be removed");
        fs::remove_dir_all(outside).expect("outside temp root must be removed");
    }

    #[cfg(unix)]
    #[test]
    fn inventory_rejects_symlinked_roots() {
        use std::os::unix::fs::symlink;

        let parent = temp_root("inventory-symlink-root");
        let outside = temp_root("inventory-symlink-root-outside");
        fs::create_dir_all(outside.join("Escaped.app/Contents"))
            .expect("outside app fixture must be created");
        let linked_root = parent.join("Applications");
        symlink(&outside, &linked_root).expect("root symlink fixture must be created");

        let report = inventory_roots(&[(ApplicationLocation::User, linked_root.clone())]);

        assert!(report.applications.is_empty());
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].path, linked_root);
        assert!(report.issues[0].message.contains("symlink"));

        fs::remove_dir_all(parent).expect("temp root must be removed");
        fs::remove_dir_all(outside).expect("outside temp root must be removed");
    }
}
