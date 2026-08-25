use std::{fs, io, path::PathBuf};

use cleaner_core::{CategoryScanTarget, CleanupCategory};

use crate::WindowsPaths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsLocalAppDataCacheScan {
    roots: Vec<PathBuf>,
}

impl WindowsLocalAppDataCacheScan {
    pub fn discover(paths: &WindowsPaths) -> io::Result<Self> {
        let packages = paths.local_app_data.join("Packages");
        let mut roots = Vec::new();

        let entries = match fs::read_dir(&packages) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Self { roots });
            }
            Err(error) => return Err(error),
        };

        for entry in entries {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_dir() || file_type.is_symlink() {
                continue;
            }

            let candidate = entry.path().join("LocalCache");
            match fs::symlink_metadata(&candidate) {
                Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                    roots.push(candidate);
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }

        roots.sort();
        roots.dedup();
        Ok(Self { roots })
    }
}

impl CategoryScanTarget for WindowsLocalAppDataCacheScan {
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
        std::env::temp_dir().join(format!("dxtr-cleaner-localappdata-{nonce}"))
    }

    #[test]
    fn discovers_only_package_local_cache_directories() {
        let root = temp_root();
        let packages = root.join("Packages");
        let cache_a = packages.join("Vendor.App_a").join("LocalCache");
        let cache_b = packages.join("Vendor.App_b").join("LocalCache");
        let local_state = packages.join("Vendor.App_b").join("LocalState");
        fs::create_dir_all(&cache_a).expect("create cache a");
        fs::create_dir_all(&cache_b).expect("create cache b");
        fs::create_dir_all(&local_state).expect("create local state");

        let paths = WindowsPaths {
            user_profile: root.join("Users/tester"),
            local_app_data: root.clone(),
            program_data: root.join("ProgramData"),
            system_root: root.join("Windows"),
            temp: root.join("Temp"),
        };

        let target = WindowsLocalAppDataCacheScan::discover(&paths).expect("discover caches");
        let request = target.request();

        assert_eq!(request.category, CleanupCategory::UserCache);
        assert_eq!(request.roots, vec![cache_a, cache_b]);
        assert!(!request.roots.contains(&local_state));

        fs::remove_dir_all(root).expect("remove fixture");
    }
}
