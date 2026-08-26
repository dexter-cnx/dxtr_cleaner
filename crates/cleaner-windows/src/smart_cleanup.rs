use std::{collections::HashMap, path::PathBuf, sync::Arc};

use cleaner_core::{
    AllowedRoot, CancellationToken, CategoryActionPolicy, CleanupAction, CleanupBackend,
    CleanupExecutor, CleanupPlan, ExecutionFailure, ExecutionPolicy, ExecutionRecord,
    ExecutionReport, Planner, SafetyError, ScanItem,
};
use same_file::Handle;

use crate::{WindowsScanSet, WindowsTrashBackend};

/// Reviewed Smart Scan plan bound to the exact filesystem identities seen at review time.
pub struct WindowsSmartCleanupPlan {
    plan: CleanupPlan,
    reviewed_identities: HashMap<PathBuf, Arc<Handle>>,
}

impl WindowsSmartCleanupPlan {
    pub fn plan(&self) -> &CleanupPlan {
        &self.plan
    }
}

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
                request
                    .roots
                    .iter()
                    .cloned()
                    .map(|root| AllowedRoot::new(request.category, root))
            })
            .collect();

        Self {
            execution_policy: ExecutionPolicy::enabled(allowed_roots),
        }
    }

    pub fn build_plan(items: Vec<ScanItem>) -> WindowsSmartCleanupPlan {
        let reviewed_identities = items
            .iter()
            .filter(|item| !item.is_symlink)
            .filter_map(|item| {
                Handle::from_path(&item.path)
                    .ok()
                    .map(|handle| (item.path.clone(), Arc::new(handle)))
            })
            .collect();

        WindowsSmartCleanupPlan {
            plan: Planner::build(items),
            reviewed_identities,
        }
    }

    pub fn execute(
        &self,
        reviewed: &WindowsSmartCleanupPlan,
        cancellation: &CancellationToken,
    ) -> Result<ExecutionReport, SafetyError> {
        self.execute_with_backend(reviewed, cancellation, &WindowsTrashBackend)
    }

    fn execute_with_backend(
        &self,
        reviewed: &WindowsSmartCleanupPlan,
        cancellation: &CancellationToken,
        backend: &dyn CleanupBackend,
    ) -> Result<ExecutionReport, SafetyError> {
        let mut executable = CleanupPlan::default();
        let mut report = ExecutionReport::default();

        for entry in reviewed.plan.items.iter().filter(|entry| entry.selected) {
            let identity_matches = reviewed
                .reviewed_identities
                .get(&entry.item.path)
                .and_then(|pinned| {
                    Handle::from_path(&entry.item.path)
                        .ok()
                        .map(|current| current == **pinned)
                })
                .unwrap_or(false);

            if identity_matches {
                executable.items.push(entry.clone());
            } else {
                report.records.push(ExecutionRecord {
                    path: entry.item.path.clone(),
                    category: entry.item.category,
                    action: CleanupAction::MoveToTrash,
                    bytes: entry.item.bytes,
                    result: Err(ExecutionFailure::Safety(
                        SafetyError::PathRevalidationFailed,
                    )),
                });
            }
        }

        let mut executed = CleanupExecutor::execute(
            &executable,
            &self.execution_policy,
            &CategoryActionPolicy::trash_only(),
            cancellation,
            backend,
        )?;
        report.records.append(&mut executed.records);
        report.cancelled = executed.cancelled;
        Ok(report)
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
    fn smart_cleanup_allows_reviewed_items_only_under_discovered_provider_roots() {
        let root = temp_root("allow-list");
        let scan_set = fixture_scan_set(&root);
        let cleanup = WindowsSmartCleanup::from_scan_set(&scan_set);
        let cache_file = root.join("Temp/cache.bin");
        fs::write(&cache_file, b"cache").expect("write cache fixture");
        let outside = root.join("outside.bin");
        fs::write(&outside, b"outside").expect("write outside fixture");
        let reviewed = WindowsSmartCleanup::build_plan(vec![
            scan_item(cache_file, CleanupCategory::UserCache),
            scan_item(outside, CleanupCategory::UserCache),
        ]);
        let backend = RecordingBackend::default();

        let report = cleanup
            .execute_with_backend(&reviewed, &CancellationToken::new(), &backend)
            .expect("execution report");

        assert_eq!(report.succeeded_count(), 1);
        assert_eq!(report.failed_count(), 1);
        assert_eq!(backend.trashed.lock().expect("lock trashed paths").len(), 1);

        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn smart_cleanup_rejects_item_replaced_after_review() {
        let root = temp_root("item-swap");
        let scan_set = fixture_scan_set(&root);
        let cleanup = WindowsSmartCleanup::from_scan_set(&scan_set);
        let cache_file = root.join("Temp/cache.bin");
        fs::write(&cache_file, b"reviewed").expect("write reviewed fixture");
        let reviewed = WindowsSmartCleanup::build_plan(vec![scan_item(
            cache_file.clone(),
            CleanupCategory::UserCache,
        )]);

        fs::remove_file(&cache_file).expect("remove reviewed file");
        fs::write(&cache_file, b"replacement").expect("write replacement fixture");

        let backend = RecordingBackend::default();
        let report = cleanup
            .execute_with_backend(&reviewed, &CancellationToken::new(), &backend)
            .expect("execution report");

        assert_eq!(report.succeeded_count(), 0);
        assert_eq!(report.failed_count(), 1);
        assert!(
            backend
                .trashed
                .lock()
                .expect("lock trashed paths")
                .is_empty()
        );

        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn smart_cleanup_cannot_authorize_an_unreviewed_new_descendant() {
        let root = temp_root("new-descendant");
        let scan_set = fixture_scan_set(&root);
        let cleanup = WindowsSmartCleanup::from_scan_set(&scan_set);
        let reviewed_file = root.join("Temp/reviewed.bin");
        fs::write(&reviewed_file, b"reviewed").expect("write reviewed fixture");
        let reviewed = WindowsSmartCleanup::build_plan(vec![scan_item(
            reviewed_file,
            CleanupCategory::UserCache,
        )]);

        let late_file = root.join("Temp/late.bin");
        fs::write(&late_file, b"late").expect("write late fixture");
        assert!(!reviewed.reviewed_identities.contains_key(&late_file));

        let backend = RecordingBackend::default();
        let report = cleanup
            .execute_with_backend(&reviewed, &CancellationToken::new(), &backend)
            .expect("execution report");

        assert_eq!(report.succeeded_count(), 1);
        assert_eq!(backend.trashed.lock().expect("lock trashed paths").len(), 1);
        assert!(
            late_file.exists(),
            "unreviewed descendant must remain untouched"
        );

        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn smart_cleanup_rejects_or_blocks_provider_root_replacement() {
        let root = temp_root("root-swap");
        let scan_set = fixture_scan_set(&root);
        let cleanup = WindowsSmartCleanup::from_scan_set(&scan_set);
        let cache_root = root.join("Temp");
        let original_file = cache_root.join("cache.bin");
        fs::write(&original_file, b"original").expect("write original fixture");
        let reviewed = WindowsSmartCleanup::build_plan(vec![scan_item(
            original_file,
            CleanupCategory::UserCache,
        )]);

        let moved = root.join("Temp-original");
        match fs::rename(&cache_root, &moved) {
            Ok(()) => {
                fs::create_dir_all(&cache_root).expect("create replacement provider root");
                fs::write(cache_root.join("cache.bin"), b"replacement")
                    .expect("write replacement fixture");

                let backend = RecordingBackend::default();
                let report = cleanup
                    .execute_with_backend(&reviewed, &CancellationToken::new(), &backend)
                    .expect("execution report");

                assert_eq!(report.succeeded_count(), 0);
                assert_eq!(report.failed_count(), 1);
                assert!(
                    backend
                        .trashed
                        .lock()
                        .expect("lock trashed paths")
                        .is_empty()
                );
            }
            Err(error) => {
                assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
            }
        }

        drop(reviewed);
        drop(cleanup);
        drop(scan_set);
        fs::remove_dir_all(root).expect("remove fixture");
    }
}
