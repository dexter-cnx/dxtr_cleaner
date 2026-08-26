use std::path::PathBuf;

use cleaner_core::{CategoryScanTarget, CleanupCategory};

use crate::WindowsPaths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsBuildCacheScan {
    roots: Vec<PathBuf>,
}

impl WindowsBuildCacheScan {
    pub fn new(paths: &WindowsPaths) -> Self {
        Self {
            roots: vec![
                paths.local_app_data.join("NuGet").join("v3-cache"),
                paths.user_profile.join(".gradle").join("caches"),
            ],
        }
    }
}

impl CategoryScanTarget for WindowsBuildCacheScan {
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

    #[test]
    fn builds_only_known_build_cache_roots() {
        let user = PathBuf::from(r"C:\Users\tester");
        let local = user.join("AppData").join("Local");
        let paths = WindowsPaths {
            user_profile: user.clone(),
            local_app_data: local.clone(),
            program_data: PathBuf::from(r"C:\ProgramData"),
            system_root: PathBuf::from(r"C:\Windows"),
            temp: local.join("Temp"),
        };

        let request = WindowsBuildCacheScan::new(&paths).request();

        assert_eq!(request.category, CleanupCategory::UserCache);
        assert_eq!(
            request.roots,
            vec![
                local.join("NuGet").join("v3-cache"),
                user.join(".gradle").join("caches"),
            ]
        );
        assert!(request.excluded_roots.is_empty());
    }
}
