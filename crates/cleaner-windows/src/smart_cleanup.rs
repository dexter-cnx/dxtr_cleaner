use cleaner_core::{
    AllowedRoot, CancellationToken, CategoryActionPolicy, CleanupBackend, CleanupExecutor,
    CleanupPlan, ExecutionPolicy, ExecutionReport, Planner, SafetyError, ScanItem,
};

use crate::{WindowsScanSet, WindowsTrashBackend};

/// Trash-only cleanup boundary for a discovered Windows Smart Scan session.
///
/// Construct this before scanning so every provider root is pinned by the shared
/// `AllowedRoot` identity checks before any review interval begins.
pub struct WindowsSmartCleanup {
    execution_policy: ExecutionPolicy,
}

impl WindowsSmartCleanup {
    pub fn from_scan_set(scan_set: &WindowsScanSet) -> Self {
        let allowed_roots = scan_set
            .requests()
            .iter()
            .flat_map(|request| {
                request.roots.iter().cloned().map(|root| {
                    AllowedRoot::new(request.category, root)
                })
            })
            .collect();

        Self {
            execution_policy: ExecutionPolicy::enabled(allowed_roots),
        }
    }

    pub fn build_plan(items: Vec<ScanItem>) -> CleanupPlan {
        Planner::build(items)
    }

    pub fn execute(
        &self,
        plan: &CleanupPlan,
        cancellation: &CancellationToken,
    ) -> Result<ExecutionReport, SafetyError> {
        self.execute_with_backend(plan, cancellation, &WindowsTrashBackend)
    }

    fn execute_with_backend(
        &self,
        plan: &CleanupPlan,
        cancellation: &CancellationToken,
        backend: &dyn CleanupBackend,
    ) -> Result<ExecutionReport, SafetyError> {
        CleanupExecutor::execute(
            plan,
            &self.execution_policy,
            &CategoryActionPolicy::trash_only(),
            cancellation,
            backend,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };

    use cleaner_core::{CleanupCategory, PermanentDeleteBackend, TrashBackend};

    use super::*;
    use crate::WindowsPaths;

    #[derive(Default)]
    struct RecordingBackend {
        trashed: Mutex<Vec<PathBuf>>,
    }

    impl TrashBackend for RecordingBackend {
        fn move_to_trash(&self, path: &Path) -> Result<(), String> {
            self.trashed
                .lock()
                .expect("lock trashed paths")
                .push(path.to_path_buf());
            Ok(())
        }
    }

    impl PermanentDeleteBackend for RecordingBackend {
        fn permanent_delete(&self, _path: &Path) -> Result<(), String> {
            panic!("Windows Smart Scan cleanup must remain Trash-only")
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "dxtr-cleaner-smart-cleanup-{name}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn scan_item(path: PathBuf, category: CleanupCategory) -> ScanItem {
        ScanItem {
            path,
            category,
            bytes: 5,
            is_symlink: false,
        }
    }

    fn fixture_scan_set(root: &Path) -> WindowsScanSet {
        let local_app_data = root.join("LocalAppData");
        fs::create_dir_all(local_app_data.join("Packages/Vendor.App/LocalCache"))
            .expect("create package cache");
        fs::create_dir_all(local_app_data.join("Google/Chrome/User Data/Default/Cache"))
            .expect("create browser cache");
        fs::create_dir_all(root.join("Temp")).expect("create temp cache");

        let paths = WindowsPaths {
            user_profile: root.join("Users/tester"),
            local_app_data,
            program_data: root.join("ProgramData"),
            system_root: root.join("Windows"),
            temp: root.join("Temp"),
        };

        WindowsScanSet::discover(&paths).expect("discover scan set")
    }

    #[test]
    fn smart_cleanup_allows_items_only_under_discovered_provider_roots() {
        let root = temp_root("allow-list");
        let scan_set = fixture_scan_set(&root);
        let cleanup = WindowsSmartCleanup::from_scan_set(&scan_set);
        let cache_root = root.join("Temp");
        let cache_file = cache_root.join("cache.bin");
        fs::write(&cache_file, b"cache").expect("write cache fixture");
        let outside = root.join("outside.bin");
        fs::write(&outside, b"outside").expect("write outside fixture");
        let plan = WindowsSmartCleanup::build_plan(vec![
            scan_item(cache_file, CleanupCategory::UserCache),
            scan_item(outside, CleanupCategory::UserCache),
        ]);
        let backend = RecordingBackend::default();

        let report = cleanup
            .execute_with_backend(&plan, &CancellationToken::new(), &backend)
            .expect("execution report");

        assert_eq!(report.succeeded_count(), 1);
        assert_eq!(report.failed_count(), 1);
        assert_eq!(backend.trashed.lock().expect("lock trashed paths").len(), 1);

        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn smart_cleanup_rejects_provider_root_replaced_after_session_pinning() {
        let root = temp_root("root-swap");
        let scan_set = fixture_scan_set(&root);
        let cleanup = WindowsSmartCleanup::from_scan_set(&scan_set);
        let cache_root = root.join("Temp");
        let original_file = cache_root.join("cache.bin");
        fs::write(&original_file, b"original").expect("write original fixture");
        let plan = WindowsSmartCleanup::build_plan(vec![scan_item(
            original_file,
            CleanupCategory::UserCache,
        )]);

        let moved = root.join("Temp-original");
        fs::rename(&cache_root, &moved).expect("move original provider root");
        fs::create_dir_all(&cache_root).expect("create replacement provider root");
        fs::write(cache_root.join("cache.bin"), b"replacement")
            .expect("write replacement fixture");

        let backend = RecordingBackend::default();
        let report = cleanup
            .execute_with_backend(&plan, &CancellationToken::new(), &backend)
            .expect("execution report");

        assert_eq!(report.succeeded_count(), 0);
        assert_eq!(report.failed_count(), 1);
        assert!(backend.trashed.lock().expect("lock trashed paths").is_empty());

        fs::remove_dir_all(root).expect("remove fixture");
    }
}
