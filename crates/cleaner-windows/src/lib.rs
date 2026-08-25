mod paths;

use std::path::Path;

use cleaner_core::{PermanentDeleteBackend, TrashBackend};

pub use paths::WindowsPaths;

#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsTrashBackend;

impl TrashBackend for WindowsTrashBackend {
    fn move_to_trash(&self, path: &Path) -> Result<(), String> {
        move_to_recycle_bin(path)
    }
}

impl PermanentDeleteBackend for WindowsTrashBackend {
    fn permanent_delete(&self, _path: &Path) -> Result<(), String> {
        Err("permanent delete remains safety-locked on Windows".into())
    }
}

#[cfg(windows)]
fn move_to_recycle_bin(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("Recycle Bin path must be absolute".into());
    }
    if !path.exists() {
        return Err(format!(
            "Recycle Bin path does not exist: {}",
            path.display()
        ));
    }

    trash::delete(path).map_err(|error| error.to_string())
}

#[cfg(not(windows))]
fn move_to_recycle_bin(_path: &Path) -> Result<(), String> {
    Err("Windows Recycle Bin backend is unavailable on this platform".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permanent_delete_fails_closed() {
        let error = WindowsTrashBackend
            .permanent_delete(Path::new("unused"))
            .expect_err("permanent delete must remain locked");
        assert!(error.contains("safety-locked"));
    }

    #[cfg(windows)]
    #[test]
    fn moves_disposable_file_to_recycle_bin() {
        use std::{fs, time::SystemTime};

        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("dxtr-cleaner-recycle-{nonce}"));
        fs::create_dir_all(&root).expect("create disposable root");
        let file = root.join("disposable.txt");
        fs::write(&file, b"dxtr-cleaner recycle-bin smoke").expect("write disposable file");
        let file = fs::canonicalize(&file).expect("canonical disposable path");

        WindowsTrashBackend
            .move_to_trash(&file)
            .expect("move disposable file to Recycle Bin");

        assert!(!file.exists(), "source path should be gone after recycling");
        let _ = fs::remove_dir(&root);
    }
}
