use std::{fs, io, path::PathBuf};

use crate::{CleanupCategory, ScanItem};

#[derive(Debug, Clone)]
pub struct ScanRequest {
    pub category: CleanupCategory,
    pub roots: Vec<PathBuf>,
}

pub trait Scanner {
    fn scan(&self, request: &ScanRequest) -> io::Result<Vec<ScanItem>>;
}

#[derive(Debug, Default)]
pub struct FileSystemScanner;

impl FileSystemScanner {
    fn walk(root: PathBuf, category: CleanupCategory, out: &mut Vec<ScanItem>) -> io::Result<()> {
        let metadata = fs::symlink_metadata(&root)?;
        let file_type = metadata.file_type();

        if file_type.is_symlink() {
            out.push(ScanItem {
                path: root,
                category,
                bytes: 0,
                is_symlink: true,
            });
            return Ok(());
        }

        if metadata.is_file() {
            out.push(ScanItem {
                path: root,
                category,
                bytes: metadata.len(),
                is_symlink: false,
            });
            return Ok(());
        }

        if metadata.is_dir() {
            for entry in fs::read_dir(root)? {
                let entry = entry?;
                if let Err(error) = Self::walk(entry.path(), category, out) {
                    if error.kind() != io::ErrorKind::PermissionDenied {
                        return Err(error);
                    }
                }
            }
        }

        Ok(())
    }
}

impl Scanner for FileSystemScanner {
    fn scan(&self, request: &ScanRequest) -> io::Result<Vec<ScanItem>> {
        let mut items = Vec::new();
        for root in &request.roots {
            if root.exists() {
                Self::walk(root.clone(), request.category, &mut items)?;
            }
        }
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use super::*;

    #[test]
    fn scans_regular_files() {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cleaner-core-{nonce}"));
        fs::create_dir_all(&root).expect("create temp dir");
        fs::write(root.join("cache.bin"), [1_u8, 2, 3, 4]).expect("write fixture");

        let scanner = FileSystemScanner;
        let items = scanner
            .scan(&ScanRequest {
                category: CleanupCategory::UserCache,
                roots: vec![root.clone()],
            })
            .expect("scan succeeds");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].bytes, 4);
        assert!(!items[0].is_symlink);

        fs::remove_dir_all(root).expect("remove temp dir");
    }
}
