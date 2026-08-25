use std::{fs, io, path::{Path, PathBuf}};

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

    if metadata.is_file() {
        if let Some(home) = std::env::var_os("HOME") {
            let trash = PathBuf::from(home).join(".Trash");
            if safe_trash_directory(&trash) {
                match move_regular_file_same_volume(path, &trash) {
                    Ok(()) => return Ok(()),
                    Err(FastTrashError::SourceDisappeared) => {
                        return Err("move to Trash skipped: path no longer exists".into());
                    }
                    Err(FastTrashError::FallbackToFinder) => {}
                }
            }
        }
    }

    move_with_finder(path)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn move_path_to_trash(path: &Path) -> Result<(), String> {
    let _ = path;
    Err("macOS only".into())
}

#[cfg(target_os = "macos")]
fn safe_trash_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FastTrashError {
    SourceDisappeared,
    FallbackToFinder,
}

#[cfg(target_os = "macos")]
fn move_regular_file_same_volume(path: &Path, trash: &Path) -> Result<(), FastTrashError> {
    let Some(file_name) = path.file_name() else {
        return Err(FastTrashError::FallbackToFinder);
    };

    for attempt in 0..10_000_u32 {
        let destination = unique_destination(trash, file_name, attempt);
        match fs::hard_link(path, &destination) {
            Ok(()) => {
                match fs::remove_file(path) {
                    Ok(()) => return Ok(()),
                    Err(error) => {
                        let _ = fs::remove_file(&destination);
                        return if error.kind() == io::ErrorKind::NotFound {
                            Err(FastTrashError::SourceDisappeared)
                        } else {
                            Err(FastTrashError::FallbackToFinder)
                        };
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(FastTrashError::SourceDisappeared);
            }
            Err(_) => return Err(FastTrashError::FallbackToFinder),
        }
    }

    Err(FastTrashError::FallbackToFinder)
}

#[cfg(target_os = "macos")]
fn unique_destination(trash: &Path, file_name: &std::ffi::OsStr, attempt: u32) -> PathBuf {
    if attempt == 0 {
        return trash.join(file_name);
    }

    let original = Path::new(file_name);
    let stem = original
        .file_stem()
        .unwrap_or(file_name)
        .to_string_lossy();
    let extension = original.extension().map(|value| value.to_string_lossy());
    let name = match extension {
        Some(extension) if !extension.is_empty() => format!("{stem} {attempt}.{extension}"),
        _ => format!("{stem} {attempt}"),
    };
    trash.join(name)
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
        Err(format!("move to Trash failed with status {}", output.status))
    } else {
        Err(format!(
            "move to Trash failed with status {}: {stderr}",
            output.status
        ))
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dxtr-cleaner-trash-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn same_volume_fast_path_moves_file_without_overwriting_collision() {
        let root = temp_dir("fast-path");
        let source_dir = root.join("source");
        let trash_dir = root.join("trash");
        fs::create_dir_all(&source_dir).expect("create source dir");
        fs::create_dir_all(&trash_dir).expect("create trash dir");
        let source = source_dir.join("cache.bin");
        fs::write(&source, b"new").expect("write source");
        fs::write(trash_dir.join("cache.bin"), b"old").expect("write collision");

        move_regular_file_same_volume(&source, &trash_dir).expect("fast move");

        assert!(!source.exists());
        assert_eq!(fs::read(trash_dir.join("cache.bin")).unwrap(), b"old");
        assert_eq!(fs::read(trash_dir.join("cache 1.bin")).unwrap(), b"new");
        fs::remove_dir_all(root).expect("remove temp dir");
    }

    #[test]
    fn unique_destination_preserves_extension() {
        let trash = Path::new("/tmp/trash");
        assert_eq!(
            unique_destination(trash, std::ffi::OsStr::new("cache.data"), 2),
            trash.join("cache 2.data")
        );
    }
}
