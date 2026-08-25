use std::path::PathBuf;

use cleaner_core::{CategoryScanTarget, CleanupCategory};

use crate::WindowsPaths;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsWerCacheScan {
    roots: Vec<PathBuf>,
}

impl WindowsWerCacheScan {
    pub fn new(paths: &WindowsPaths) -> Self {
        let wer = paths
            .local_app_data
            .join("Microsoft")
            .join("Windows")
            .join("WER");

        Self {
            roots: vec![wer.join("ReportArchive"), wer.join("ReportQueue")],
        }
    }
}

impl CategoryScanTarget for WindowsWerCacheScan {
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
    fn targets_only_user_wer_archive_and_queue() {
        let paths = WindowsPaths {
            user_profile: PathBuf::from(r"C:\Users\tester"),
            local_app_data: PathBuf::from(r"C:\Users\tester\AppData\Local"),
            program_data: PathBuf::from(r"C:\ProgramData"),
            system_root: PathBuf::from(r"C:\Windows"),
            temp: PathBuf::from(r"C:\Users\tester\AppData\Local\Temp"),
        };

        let request = WindowsWerCacheScan::new(&paths).request();
        let wer = paths
            .local_app_data
            .join("Microsoft")
            .join("Windows")
            .join("WER");

        assert_eq!(request.category, CleanupCategory::UserCache);
        assert_eq!(
            request.roots,
            vec![wer.join("ReportArchive"), wer.join("ReportQueue")]
        );
        assert!(request.excluded_roots.is_empty());
        assert!(!request.roots.iter().any(|root| root == &wer));
    }
}
