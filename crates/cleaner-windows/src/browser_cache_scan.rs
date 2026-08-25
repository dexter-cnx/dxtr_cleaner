use std::{fs, io, path::{Path, PathBuf}};

use cleaner_core::{CategoryScanTarget, CleanupCategory};

use crate::WindowsPaths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsBrowserCacheScan {
    roots: Vec<PathBuf>,
}

impl WindowsBrowserCacheScan {
    pub fn discover(paths: &WindowsPaths) -> io::Result<Self> {
        let mut roots = Vec::new();
        collect_chromium_profile_caches(
            &paths.local_app_data.join("Google").join("Chrome").join("User Data"),
            &mut roots,
        )?;
        collect_chromium_profile_caches(
            &paths.local_app_data.join("Microsoft").join("Edge").join("User Data"),
            &mut roots,
        )?;
        roots.sort();
        roots.dedup();
        Ok(Self { roots })
    }
}

impl CategoryScanTarget for WindowsBrowserCacheScan {
    fn category(&self) -> CleanupCategory {
        CleanupCategory::UserCache
    }

    fn roots(&self) -> Vec<PathBuf> {
        self.roots.clone()
    }
}

fn collect_chromium_profile_caches(user_data: &Path, roots: &mut Vec<PathBuf>) -> io::Result<()> {
    let entries = match fs::read_dir(user_data) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };

    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }

        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name != "Default" && !name.starts_with("Profile ") {
            continue;
        }

        for relative in ["Cache", "Code Cache", "GPUCache"] {
            let candidate = entry.path().join(relative);
            match fs::symlink_metadata(&candidate) {
                Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                    roots.push(candidate);
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
    }

    Ok(())
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
        std::env::temp_dir().join(format!("dxtr-cleaner-browser-cache-{nonce}"))
    }

    #[test]
    fn discovers_only_known_chromium_profile_cache_directories() {
        let root = temp_root();
        let chrome = root.join("Google/Chrome/User Data");
        let edge = root.join("Microsoft/Edge/User Data");

        let chrome_cache = chrome.join("Default/Cache");
        let chrome_code_cache = chrome.join("Profile 1/Code Cache");
        let chrome_history_parent = chrome.join("Default");
        let edge_gpu_cache = edge.join("Default/GPUCache");
        let edge_random = edge.join("Guest Profile/Cache");

        fs::create_dir_all(&chrome_cache).expect("create chrome cache");
        fs::create_dir_all(&chrome_code_cache).expect("create chrome code cache");
        fs::write(chrome_history_parent.join("History"), b"history").expect("write history");
        fs::create_dir_all(&edge_gpu_cache).expect("create edge gpu cache");
        fs::create_dir_all(&edge_random).expect("create guest cache");

        let paths = WindowsPaths {
            user_profile: root.join("Users/tester"),
            local_app_data: root.clone(),
            program_data: root.join("ProgramData"),
            system_root: root.join("Windows"),
            temp: root.join("Temp"),
        };

        let target = WindowsBrowserCacheScan::discover(&paths).expect("discover browser caches");
        let request = target.request();

        assert_eq!(request.category, CleanupCategory::UserCache);
        assert_eq!(
            request.roots,
            vec![chrome_cache, chrome_code_cache, edge_gpu_cache]
        );
        assert!(!request.roots.contains(&chrome_history_parent.join("History")));
        assert!(!request.roots.contains(&edge_random));

        fs::remove_dir_all(root).expect("remove fixture");
    }
}
