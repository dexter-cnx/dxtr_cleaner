use std::path::PathBuf;

use cleaner_core::{CategoryScanTarget, CleanupCategory};

use crate::WindowsPaths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsDevCacheScan {
    roots: Vec<PathBuf>,
}

impl WindowsDevCacheScan {
    pub fn new(paths: &WindowsPaths) -> Self {
        let roots = vec![
            paths.local_app_data.join("npm-cache"),
            paths.local_app_data.join("pnpm").join("store"),
            paths.local_app_data.join("Yarn").join("Cache"),
        ];
        Self { roots }
    }
}

impl CategoryScanTarget for WindowsDevCacheScan {
    fn category(&self) -> CleanupCategory {
        CleanupCategory::Node
    }

    fn roots(&self) -> Vec<PathBuf> {
        self.roots.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_only_known_node_package_manager_cache_roots() {
        let local = PathBuf::from(r"C:\Users\tester\AppData\Local");
        let paths = WindowsPaths {
            user_profile: PathBuf::from(r"C:\Users\tester"),
            local_app_data: local.clone(),
            program_data: PathBuf::from(r"C:\ProgramData"),
            system_root: PathBuf::from(r"C:\Windows"),
            temp: PathBuf::from(r"C:\Users\tester\AppData\Local\Temp"),
        };

        let request = WindowsDevCacheScan::new(&paths).request();

        assert_eq!(request.category, CleanupCategory::Node);
        assert_eq!(
            request.roots,
            vec![
                local.join("npm-cache"),
                local.join("pnpm").join("store"),
                local.join("Yarn").join("Cache"),
            ]
        );
        assert!(request.excluded_roots.is_empty());
    }
}
