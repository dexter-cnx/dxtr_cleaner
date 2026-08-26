use std::io;

use cleaner_core::{CategoryScanTarget, ScanRequest};

use crate::{
    WindowsBrowserCacheScan, WindowsBuildCacheScan, WindowsDevCacheScan, WindowsExplorerCacheScan,
    WindowsLocalAppDataCacheScan, WindowsPaths, WindowsTempScan, WindowsWerCacheScan,
};

#[derive(Debug, Clone)]
pub struct WindowsScanSet {
    requests: Vec<ScanRequest>,
}

impl WindowsScanSet {
    pub fn discover(paths: &WindowsPaths) -> io::Result<Self> {
        let mut requests = vec![
            WindowsTempScan::new(paths).request(),
            WindowsWerCacheScan::new(paths).request(),
            WindowsDevCacheScan::new(paths).request(),
            WindowsBuildCacheScan::new(paths).request(),
        ];

        requests.push(WindowsLocalAppDataCacheScan::discover(paths)?.request());
        requests.push(WindowsExplorerCacheScan::discover(paths)?.request());
        requests.push(WindowsBrowserCacheScan::discover(paths)?.request());

        requests.retain(|request| !request.roots.is_empty());
        Ok(Self { requests })
    }

    pub fn requests(&self) -> &[ScanRequest] {
        &self.requests
    }

    pub fn into_requests(self) -> Vec<ScanRequest> {
        self.requests
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_core::CleanupCategory;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!("dxtr-cleaner-windows-scan-set-{nonce}"))
    }

    #[test]
    fn aggregates_known_windows_scan_requests_without_empty_discovery_targets() {
        let root = temp_root();
        fs::create_dir_all(root.join("Packages/Vendor.App/LocalCache"))
            .expect("create package cache");
        fs::create_dir_all(root.join("Google/Chrome/User Data/Default/Cache"))
            .expect("create browser cache");

        let paths = WindowsPaths {
            user_profile: root.join("Users/tester"),
            local_app_data: root.clone(),
            program_data: root.join("ProgramData"),
            system_root: root.join("Windows"),
            temp: root.join("Temp"),
        };

        let set = WindowsScanSet::discover(&paths).expect("discover Windows scan set");
        let requests = set.requests();

        assert!(
            requests
                .iter()
                .any(|request| request.category == CleanupCategory::Node)
        );
        assert!(requests.iter().any(|request| {
            request.category == CleanupCategory::UserCache
                && request
                    .roots
                    .contains(&root.join("Packages/Vendor.App/LocalCache"))
        }));
        assert!(requests.iter().any(|request| {
            request.category == CleanupCategory::UserCache
                && request
                    .roots
                    .contains(&root.join("Google/Chrome/User Data/Default/Cache"))
        }));
        assert!(requests.iter().all(|request| !request.roots.is_empty()));

        fs::remove_dir_all(root).expect("remove fixture");
    }
}
