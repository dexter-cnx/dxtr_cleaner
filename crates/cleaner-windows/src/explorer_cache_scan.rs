use std::{fs, io, path::PathBuf};

use cleaner_core::{CategoryScanTarget, CleanupCategory};

use crate::WindowsPaths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsExplorerCacheScan {
    roots: Vec<PathBuf>,
}

impl WindowsExplorerCacheScan {
    pub fn discover(paths: &WindowsPaths) -> io::Result<Self> {
        let explorer = paths
            .local_app_data
            .join("Microsoft")
            .join("Windows")
            .join("Explorer");
        let mut roots = Vec::new();

        let entries = match fs::read_dir(&explorer) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Self { roots });
            }
            Err(error) => return Err(error),
        };

        for entry in entries {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_file() || file_type.is_symlink() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            let is_cache_db = (name.starts_with("thumbcache_") || name.starts_with("iconcache_"))
                && name.ends_with(".db");
            if is_cache_db {
                roots.push(entry.path());
            }
        }

        roots.sort();
        roots.dedup();
        Ok(Self { roots })
    }
}

impl CategoryScanTarget for WindowsExplorerCacheScan {
    fn category(&self) -> CleanupCategory {
        CleanupCategory::UserCache
    }

    fn roots(&self) -> Vec<PathBuf> {
        self.roots.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!("dxtr-cleaner-explorer-cache-{nonce}"))
    }

    #[test]
    fn discovers_only_explorer_cache_databases() {
        let root = temp_root();
        let explorer = root.join("Microsoft/Windows/Explorer");
        fs::create_dir_all(&explorer).expect("create explorer fixture");

        let thumb = explorer.join("thumbcache_256.db");
        let icon = explorer.join("iconcache_32.db");
        let unrelated = explorer.join("unrelated.db");
        let text = explorer.join("thumbcache_notes.txt");
        fs::write(&thumb, b"thumb").expect("write thumb cache");
        fs::write(&icon, b"icon").expect("write icon cache");
        fs::write(&unrelated, b"other").expect("write unrelated db");
        fs::write(&text, b"notes").expect("write text file");

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
        assert_eq!(request.roots, vec![icon, thumb]);
        assert!(!request.roots.contains(&unrelated));
        assert!(!request.roots.contains(&text));

        fs::remove_dir_all(root).expect("remove fixture");
    }
}
