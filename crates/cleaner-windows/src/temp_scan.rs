use std::path::PathBuf;

use cleaner_core::{CategoryScanTarget, CleanupCategory};

use crate::WindowsPaths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsTempScan {
    temp: PathBuf,
}

impl WindowsTempScan {
    pub fn new(paths: &WindowsPaths) -> Self {
        Self {
            temp: paths.temp.clone(),
        }
    }
}

impl CategoryScanTarget for WindowsTempScan {
    fn category(&self) -> CleanupCategory {
        CleanupCategory::UserCache
    }

    fn roots(&self) -> Vec<PathBuf> {
        vec![self.temp.clone()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_user_cache_request_from_typed_windows_paths() {
        let paths = WindowsPaths {
            user_profile: PathBuf::from(r"C:\Users\tester"),
            local_app_data: PathBuf::from(r"C:\Users\tester\AppData\Local"),
            program_data: PathBuf::from(r"C:\ProgramData"),
            system_root: PathBuf::from(r"C:\Windows"),
            temp: PathBuf::from(r"C:\Users\tester\AppData\Local\Temp"),
        };

        let request = WindowsTempScan::new(&paths).request();
        assert_eq!(request.category, CleanupCategory::UserCache);
        assert_eq!(request.roots, vec![paths.temp]);
        assert!(request.excluded_roots.is_empty());
    }
}
