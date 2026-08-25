use std::{fs, io, path::Path};

#[cfg(target_os = "macos")]
pub(crate) fn move_path_to_trash(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            "move to Trash skipped: path no longer exists".to_string()
        } else {
            format!("move to Trash preflight failed: {error}")
        }
    })?;
    if metadata.file_type().is_symlink() {
        return Err("move to Trash rejected: path is a symlink".into());
    }

    move_with_finder(path)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn move_path_to_trash(path: &Path) -> Result<(), String> {
    let _ = path;
    Err("macOS only".into())
}

#[cfg(target_os = "macos")]
fn move_with_finder(path: &Path) -> Result<(), String> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg("on run argv")
        .arg("-e")
        .arg("tell application \"Finder\" to delete POSIX file (item 1 of argv)")
        .arg("-e")
        .arg("end run")
        .arg(path)
        .output()
        .map_err(|error| error.to_string())?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if matches!(
        fs::symlink_metadata(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound
    ) {
        return Err("move to Trash failed: path disappeared before Finder could move it".into());
    }

    if stderr.is_empty() {
        Err(format!(
            "move to Trash failed with status {}",
            output.status
        ))
    } else {
        Err(format!(
            "move to Trash failed with status {}: {stderr}",
            output.status
        ))
    }
}
