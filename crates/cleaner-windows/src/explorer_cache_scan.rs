use std::{fs, io, path::PathBuf};

use cleaner_core::{CategoryScanTarget, CleanupCategory};

use crate::WindowsPaths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsExplorerCacheScan {
    root: Option<PathBuf>,
    excluded_roots: Vec<PathBuf>,
}

impl WindowsExplorerCacheScan {
    pub fn discover(paths: &WindowsPaths) -> io::Result<Self> {
        let explorer = paths
            .local_app_data
            .join("Microsoft")
            .join("Windows")
            .join("Explorer");
        let entries = match fs::read_dir(&explorer) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Self {
                    root: None,
                    excluded_roots: Vec::new(),
                });
            }
            Err(error) => return Err(error),
        };

        let mut excluded_roots = Vec::new();
        for entry in entries {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            let is_cache_db = file_type.is_file()
                && !file_type.is_symlink()
                && (name.starts_with("thumbcache_") || name.starts_with("iconcache_"))
                && name.ends_with(".db");

            if !is_cache_db {
                excluded_roots.push(entry.path());
            }
        }

        excluded_roots.sort();
        excluded_roots.dedup();
        Ok(Self {
            root: Some(explorer),
            excluded_roots,
        })
    }
}

impl CategoryScanTarget for WindowsExplorerCacheScan {
    fn category(&self) -> CleanupCategory {
        CleanupCategory::UserCache
    }

    fn roots(&self) -> Vec<PathBuf> {
        self.root.iter().cloned().collect()
    }

    fn excluded_roots(&self) -> Vec<PathBuf> {
        self.excluded_roots.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_core::{FileSystemScanner, Scanner};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!("dxtr-cleaner-explorer-cache-{nonce}"))
    }

    #[test]
    fn scans_only_explorer_cache_databases_under_directory_root() {
        let root = temp_root();
        let explorer = root.join("Microsoft/Windows/Explorer");
        fs::create_dir_all(&explorer).expect("create explorer fixture");

        let thumb = explorer.join("thumbcache_256.db");
        let icon = explorer.join("iconcache_32.db");
        let unrelated = explorer.join("unrelated.db");
        let text = explorer.join("thumbcache_notes.txt");
        let unrelated_dir = explorer.join("other");
        fs::write(&thumb, b"thumb").expect("write thumb cache");
        fs::write(&icon, b"icon").expect("write icon cache");
        fs::write(&unrelated, b"other").expect("write unrelated db");
        fs::write(&text, b"notes").expect("write text file");
        fs::create_dir_all(&unrelated_dir).expect("create unrelated dir");
        fs::write(unrelated_dir.join("nested.db"), b"nested").expect("write nested file");

        let paths = WindowsPaths {
            user_profile: root.join("Users/tester"),
            local_app_data: root.clone(),
            program_data: root.join("ProgramData"),
            system_root: root.join("Windows"),
            temp: root.join("Temp"),
        };

        let target = WindowsExplorerCacheScan::discover(&paths).expect("discover explorer cache");
        let request = target.request();

        assert_eq!(request.category, CleanupCategory::UserCache);
        assert_eq!(request.roots, vec![explorer]);
        assert!(request.excluded_roots.contains(&unrelated));
        assert!(request.excluded_roots.contains(&text));
        assert!(request.excluded_roots.contains(&unrelated_dir));

        let mut items = FileSystemScanner.scan(&request).expect("scan explorer cache");
        items.sort_by(|left, right| left.path.cmp(&right.path));
        assert_eq!(
            items.into_iter().map(|item| item.path).collect::<Vec<_>>(),
            vec![icon, thumb]
        );

        fs::remove_dir_all(root).expect("remove fixture");
    }
}
